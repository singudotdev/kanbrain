#!/usr/bin/env bash
# Decides which jobs of a workflow (ci or mutants, the first argument) a push or pull request needs, and writes them
# to $GITHUB_OUTPUT for the jobs' `if:`. A tree a passing run of the workflow already checked needs none: a squash
# into develop or main, or a commit re-signed, holds the tree its pull request passed with. Otherwise each job runs
# when the change touches what it checks; a release branch, a change to the workflows, the toolchain or the
# dependencies, or a change with nothing to compare against runs them all.
#
# Each run leaves an artifact named after its workflow and tree (the workflow's upload after this), which the next
# run of the same tree finds here.
set -euo pipefail
workflow=$1
out=${GITHUB_OUTPUT:-/dev/stdout}
tree=$(git rev-parse 'HEAD^{tree}')
echo "tree=$tree" >>"$out"

# What the change is measured from: the last commit on the branch whose push run of this workflow passed, so what a
# failing push brought is still checked, by the push after it or by a pull request into that branch. A pull request
# is checked out as its merge into the base; with no passing commit there (a branch CI doesn't push-run), all jobs
# run. A push without one is measured from the push before it.
# Whether the last passing push run on branch `$1` checked an ancestor of HEAD: its commit.
passed_on() {
  local sha
  sha=$(gh run list --repo "$GITHUB_REPOSITORY" --workflow "$workflow.yml" --branch "$1" --event push \
    --status success --limit 1 --json headSha --jq '.[0].headSha // empty')
  if [ -n "$sha" ] && git merge-base --is-ancestor "$sha" HEAD 2>/dev/null; then echo "$sha"; fi
}
if [ "${GITHUB_EVENT_NAME:-}" = pull_request ]; then
  base=$(passed_on "$GITHUB_BASE_REF")
else
  base=$(passed_on "$GITHUB_REF_NAME")
  base=${base:-${BEFORE:-}}
fi
echo "base=$base" >>"$out"

checked=false
while read -r run; do
  if [ -z "$run" ] || [ "$run" = "${GITHUB_RUN_ID:-}" ]; then continue; fi
  if [ "$(gh run view "$run" --repo "$GITHUB_REPOSITORY" --json conclusion --jq .conclusion)" = success ]; then
    checked=true
    echo "tree $tree passed in run $run"
    break
  fi
  # Only this repository's runs: a fork's pull request runs its own workflow, which could leave the artifact unchecked.
done < <(gh api "repos/$GITHUB_REPOSITORY/actions/artifacts?name=$workflow-tree-$tree&per_page=20" \
  --jq '.artifacts[].workflow_run | select(.head_repository_id == .repository_id) | .id')

all=false
case "${GITHUB_HEAD_REF:-${GITHUB_REF_NAME:-}}" in release/*) all=true ;; esac
if [ -n "$base" ] && git cat-file -e "$base^{commit}" 2>/dev/null; then
  # Both sides of a rename: the old path is what a job may check (a file built in, moved away).
  changed=$(git diff --name-only --no-renames "$base" HEAD)
  echo "changed since $base:"
  while IFS= read -r file; do echo "  $file"; done <<<"$changed"
else
  all=true
  changed=""
  echo "nothing to compare with: all jobs"
fi
# A first push has no base: every changed line is new, Rust ones included.
rust_changed=$all
if [ -n "$changed" ]; then
  if grep -qE '\.rs$' <<<"$changed"; then rust_changed=true; else rust_changed=false; fi
fi

# Whether the change touches a file matching the extended regular expression $1 (or everything, see above).
touches() { [ "$all" = true ] || grep -qE "$1" <<<"$changed"; }
if touches '^(\.github/(workflows/|plan\.sh$)|Cargo\.(toml|lock)$|rust-toolchain|\.cargo/|rustfmt\.toml$|deny\.toml$)'; then
  all=true
fi

# Files built into the programs (`include_str!`, `include_bytes!`, core/build.rs) count as their code.
core='^(core/|tag-changelog/|install\.sh$|docs/(agents|cli)\.md$|docs/images/logo\.svg$)'
# A test reads the README, as the crate's README must list the same commands.
cli='^(cli/|README\.md$)'
gui='^(gui/|docs/keybindings\.md$|docs/images/logo\.svg$)'
xtask='^(xtask/|about\.toml$)'
scripts='\.(sh|ps1)$'

# Writes whether job $1 runs: when `$2...` succeeds, unless the tree was already checked.
need() {
  local job=$1 runs=false
  shift
  if [ "$checked" = false ] && "$@"; then runs=true; fi
  echo "$job=$runs" >>"$out"
  echo "$job: $runs"
}
case "$workflow" in
  ci)
    need lint touches "$core|$cli|$gui|$xtask"
    need deny touches "$xtask"
    need test touches "$core|$cli|$gui"
    need msrv touches "$core|$cli"
    need installers touches "$core|$cli|$scripts"
    need upgrade touches "$core|$cli|^e2e/upgrade\.sh$"
    need app touches "$core|$gui|^e2e/smoke-app\."
    ;;
  mutants)
    # Only the Rust lines changed are mutated: a release branch or a new dependency alone has none.
    need mutants test "$rust_changed" = true
    ;;
  *)
    echo "unknown workflow $workflow" >&2
    exit 1
    ;;
esac
