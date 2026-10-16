#!/usr/bin/env bash
# Update check, on Linux x86_64: every released `kb update` installs this build and leaves its brains working.
#
# Each release's updater fetches an installer (from `main` before v0.4.0, from the latest release since) and the
# installer the release's archives. Here a `curl` put first on PATH answers for GitHub: the latest release is a
# version after every one there is, the installer is this tree's `install.sh`, and its archive holds this build's
# `kb`. So what users will meet once this build is released runs before it is: each release's `kb`, downloaded
# from its GitHub release (checksum checked), writes a central brain and a project in a throwaway home, runs
# `kb update`, and then:
#
# - the `kb` in its folder is this build's;
# - this build reads the brains (`list`, `show`, `context`);
# - the installer ran this build's `init --all`: running it again changes no project's agent instructions.
#
# Run it before a release, with the repository's tags; it needs cargo, curl, git, python3 and tar, and touches
# nothing outside a temporary folder.
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
cargo build -q --manifest-path "$repo/Cargo.toml" --bin kb
head_kb="$repo/target/debug/kb"
releases="https://github.com/singudotdev/kanbrain/releases/download"
asset="kb-x86_64-unknown-linux-musl.tar.gz"
# After every release, so each updater takes it for a newer one.
next=v999.0.0

work=$(mktemp -d "${TMPDIR:-/tmp}/kb-update.XXXXXX")
trap 'rm -rf -- "$work"' EXIT

fail() {
    echo "FAIL: $*" >&2
    exit 1
}

# This build as a release would ship it, where the stand-in `curl` serves it from.
mkdir -p "$work/dist" "$work/shim"
tar -czf "$work/dist/$asset" -C "$(dirname "$head_kb")" kb
(cd "$work/dist" && sha256sum "$asset" >"$asset.sha256")
cp "$repo/install.sh" "$work/dist/install.sh"

# The stand-in `curl`: GitHub's answers, from $work/dist. Any other address fails, as a download that can't be
# made does, saying which.
cat >"$work/shim/curl" <<EOF
#!/bin/sh
dist="$work/dist"
next="$next"
EOF
cat >>"$work/shim/curl" <<'EOF'
url="" out="" head=0
while [ $# -gt 0 ]; do
    case "$1" in
        -o) out=$2; shift ;;
        -w | --max-time | --retry) shift ;;
        http://* | https://*) url=$1 ;;
        --*) ;;
        -*I*) head=1 ;;
    esac
    shift
done
serve() {
    if [ -n "$out" ]; then cp "$1" "$out"; else cat "$1"; fi
}
case "$url" in
    */releases/latest)
        [ "$head" = 1 ] || { echo "curl stand-in: $url without -I" >&2; exit 22; }
        printf 'HTTP/2 302\r\nlocation: https://github.com/singudotdev/kanbrain/releases/tag/%s\r\n\r\n' "$next" ;;
    */main/install.sh | */releases/latest/download/install.sh | */releases/download/"$next"/install.sh)
        serve "$dist/install.sh" ;;
    */releases/download/"$next"/* | */releases/latest/download/*)
        file="$dist/${url##*/}"
        [ -f "$file" ] || { echo "curl stand-in: no $url" >&2; exit 22; }
        serve "$file" ;;
    *) echo "curl stand-in: $url isn't GitHub's for this release" >&2; exit 22 ;;
esac
EOF
chmod +x "$work/shim/curl"

# `kb` from $1 in the throwaway home $2, as a person, with the stand-in `curl` first on PATH. No agent variable
# leaks in: an agent at work stops the update.
person() {
    local bin=$1 home=$2
    shift 2
    env -u CLAUDECODE -u AI_AGENT -u AGENT -u CODEX_SANDBOX -u CODEX_CI -u CODEX_THREAD_ID -u GEMINI_CLI \
        -u KANBRAIN_AUTHOR -u KANBRAIN_VERSION -u KANBRAIN_BASE_URL -u KANBRAIN_INSTALL_DIR -u KANBRAIN_APP \
        PATH="$work/shim:$PATH" HOME="$home" KANBRAIN_DIR="$home/central" XDG_CACHE_HOME="$home/.cache" \
        XDG_DATA_HOME="$home/.local/share" XDG_CONFIG_HOME="$home/.config" "$bin" "$@"
}

tags=$(git -C "$repo" tag -l 'v*' --sort=version:refname)
[ -n "$tags" ] || fail "no release tags: fetch them (git fetch --tags)"
tested=0
for tag in $tags; do
    status=$(command curl -sSL -o /dev/null -w '%{http_code}' "$releases/$tag/$asset.sha256") || fail "$tag: no answer"
    if [ "$status" = 404 ]; then
        echo "skipped $tag: no release published"
        continue
    fi
    dir="$work/$tag" home="$work/$tag/home"
    bin="$home/.local/bin"
    mkdir -p "$bin" "$home/central" "$home/src/demo"
    command curl -fsSL -o "$dir/$asset" "$releases/$tag/$asset" || fail "$tag: downloading its kb failed"
    command curl -fsSL -o "$dir/$asset.sha256" "$releases/$tag/$asset.sha256" || fail "$tag: no checksum"
    (cd "$dir" && sha256sum -c --quiet "$asset.sha256") || fail "$tag: checksum mismatch"
    tar -xzf "$dir/$asset" -C "$bin"
    cmp -s "$bin/kb" "$head_kb" && fail "$tag: its kb is this build's already"

    old() { person "$bin/kb" "$home" "$@"; }
    git -C "$home/src/demo" init -q
    (cd "$home" && old add --tag home "Before the update" >/dev/null) || fail "$tag: writing its central brain"
    (cd "$home/src/demo" && old init demo >/dev/null && old add "A project task" >/dev/null) ||
        fail "$tag: writing its project"

    (cd "$home" && old update </dev/null >"$dir/update.out" 2>&1) ||
        fail "$tag: kb update failed: $(cat "$dir/update.out")"
    cmp -s "$bin/kb" "$head_kb" || fail "$tag: kb update didn't install this build: $(cat "$dir/update.out")"

    new() { person "$bin/kb" "$home" "$@"; }
    (cd "$home" && new list all | grep -q "Before the update") || fail "$tag: the central brain after the update"
    (cd "$home/src/demo" && new show 1 | grep -q "A project task" && new context >/dev/null) ||
        fail "$tag: the project after the update"
    instructions=$(cat "$home/src/demo/CLAUDE.md")
    (cd "$home" && new init --all >/dev/null) || fail "$tag: init --all after the update"
    [ "$(cat "$home/src/demo/CLAUDE.md")" = "$instructions" ] ||
        fail "$tag: the update didn't bring the project's agent instructions up to date"
    echo "PASS $tag"
    tested=$((tested + 1))
done
[ "$tested" -gt 0 ] || fail "no release to update from"
echo "PASS update.sh"
