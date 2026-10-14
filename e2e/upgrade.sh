#!/usr/bin/env bash
# Upgrade check, on Linux x86_64: brains written by every released `kb` open with this build and lose nothing.
#
# For each release tag (`v*`), its `kb` is downloaded from the GitHub release (checksum checked) and writes a
# varied set of brains in a throwaway home: the central one, a project of two repositories and one of one; tasks
# in every status with sessions, notes and decisions (accepted, rejected, proposed), questions answered and not,
# agents' comments, links within a brain and across repositories, unicode and markdown. Every entry's
# `show --json` is recorded. Then this build, on a copy:
#
# - reads every brain (`list`, `show`, `context`, `status`, `report`) without changing a file;
# - shows every entry as the old version did, field by field (new fields may be added);
# - runs `init --all`, which renames the names v0.4.0 stopped taking, and writes (comments, status changes)
#   without an error.
#
# CI runs it on every push (the `upgrade` job). It needs cargo, curl, git, python3 and the repository's tags, and
# touches nothing outside a temporary folder.
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
cargo build -q --manifest-path "$repo/Cargo.toml" --bin kb
head_kb="$repo/target/debug/kb"
releases="https://github.com/singudotdev/kanbrain/releases/download"
asset="kb-x86_64-unknown-linux-musl.tar.gz"

work=$(mktemp -d "${TMPDIR:-/tmp}/kb-upgrade.XXXXXX")
trap 'rm -rf -- "$work"' EXIT

fail() {
    echo "FAIL: $*" >&2
    exit 1
}

# `kb` from $1 in the throwaway home $2, as a person; `agent` runs it as an agent. No agent variable leaks in.
person() {
    local bin=$1 home=$2
    shift 2
    env -u CLAUDECODE -u AI_AGENT -u AGENT -u CODEX_SANDBOX -u CODEX_CI -u CODEX_THREAD_ID -u GEMINI_CLI \
        -u KANBRAIN_AUTHOR HOME="$home" KANBRAIN_DIR="$home/central" XDG_CACHE_HOME="$home/.cache" \
        XDG_DATA_HOME="$home/.local/share" "$bin" "$@"
}
agent() {
    local bin=$1 home=$2
    shift 2
    KANBRAIN_AUTHOR=agent person "$bin" "$home" "$@"
}
id_of() { grep -o '#[0-9]*' | head -n 1 | tr -d '#'; }

# The brains written by `kb` $1 under $2.
build() {
    local kb=$1 w=$2
    p() { person "$kb" "$w/home" "$@"; }
    a() { agent "$kb" "$w/home" "$@"; }
    mkdir -p "$w/home/central" "$w/src/shop/api" "$w/src/shop/web" "$w/src/solo"
    for d in shop/api shop/web solo; do git -C "$w/src/$d" init -q; done

    cd "$w/home"
    c1=$(p add --tag home "Buy café ☕ beans: naïve résumé 日本語" | id_of)
    p add --note --tag home --body $'# Heading\n\n- **bold** item\n\n```rust\nfn main() {}\n```' "A note" >/dev/null
    c3=$(p add "$(printf 'A long title %.0s' $(seq 12))end" | id_of)
    p link "$c1" "$c3" >/dev/null
    p comment "$c1" "a person's comment with **markdown** 🎉" >/dev/null
    p "done" "$c3" >/dev/null

    cd "$w/src/shop/api" && p init shop >/dev/null
    cd "$w/src/shop/web" && p init shop >/dev/null
    cd "$w/src/shop/api"
    p project describe $'Shop: an **online** store.\nSecond line.' >/dev/null
    p project notes "API: REST and webhooks" >/dev/null
    local a1 a2 a3 a4 d1 d2 w1 w2
    a1=$(p add --tag backend --body $'Implement `/orders`.\n\n1. validate\n2. persist' "Orders endpoint" | id_of)
    a2=$(p add --tag backend "Payments" | id_of)
    a3=$(p add "Fix a flaky test" | id_of)
    a4=$(p add --tag docs "Write the docs" | id_of)
    p add --decision --body "Use JSON:API?" "Response format" >/dev/null
    p start "$a3" >/dev/null && sleep 1 && p park "$a3" >/dev/null && p start "$a3" >/dev/null && p "done" "$a3" >/dev/null
    p comment "$a2" "Waiting on the provider's keys" >/dev/null
    a comment "$a2" "Agent: the SDK needs v3." >/dev/null
    d1=$(a propose "$a1" --body "Use **REST**." "Use REST" | id_of)
    d2=$(a propose "$a1" --body "gRPC inside." "Use gRPC" | id_of)
    a propose "$a4" --body "mdBook." "Docs tool" >/dev/null
    p accept "$d1" "Agreed." >/dev/null
    p reject "$d2" "Too heavy for now." >/dev/null
    a ask "$a2" "Stripe or Adyen?" >/dev/null
    p comment "$a2" "Stripe." >/dev/null
    cd "$w/src/shop/web"
    p project notes "Web: the front end" >/dev/null
    w1=$(p add --tag frontend "Checkout page" | id_of)
    w2=$(p add --tag frontend "Cart drawer ✨" | id_of)
    p link "$w1" "api#$a1" >/dev/null
    p link "$w2" "$w1" >/dev/null
    a comment "$w1" "agent progress: page scaffolded" >/dev/null
    p start "$w1" >/dev/null
    a start "$w2" >/dev/null 2>&1 || true
    # Names v0.4.0 stopped taking, which `kb init --all` renames: a project starting with `-`, one named as the
    # central brain in other case, and a repository starting with `-`. A release that refuses them writes none.
    mkdir -p "$w/src/dash" "$w/src/upper" "$w/src/docs"
    for d in dash upper docs; do git -C "$w/src/$d" init -q; done
    cd "$w/src/dash" && p init -dash >/dev/null 2>&1 && p add "Dash task" >/dev/null
    cd "$w/src/upper" && p init Central >/dev/null 2>&1 && p add "Upper task" >/dev/null
    cd "$w/src/docs" && p init docs >/dev/null && p add "Docs task" >/dev/null
    p project repo -docs >/dev/null 2>&1 || true
    cd "$w/src/solo" && p init solo >/dev/null
    p project describe "A side project" >/dev/null
    p add --tag misc "Solo todo" >/dev/null
    p "done" "$(p add "Solo done" | id_of)" >/dev/null
}

# After `init --all`, the copy at $2 has renamed the names the old version's data at $1 had, and reaches their tasks.
# Each rename checked counts in `dashes`, `uppers` and `repos`, so that a setup writing none fails below.
renamed() {
    local old=$1 w=$2 reg=$2/home/central/projects.md
    h() { person "$head_kb" "$w/home" "$@"; }
    if grep -q '^- -dash: ' "$old/home/central/projects.md"; then
        grep -q "^- dash: $w/src/dash/.brain$" "$reg" || fail "$tag: project \`-dash\` not renamed to \`dash\`"
        (cd "$w/home" && h --project dash list | grep 'Dash task' >/dev/null) || fail "$tag: \`dash\` lost its task"
        dashes=$((dashes + 1))
    fi
    if grep -q '^- Central: ' "$old/home/central/projects.md"; then
        grep -q "^- Central-project: $w/src/upper/.brain$" "$reg" || fail "$tag: project \`Central\` not renamed"
        (cd "$w/src/upper" && h list | grep 'Upper task' >/dev/null) || fail "$tag: \`Central-project\` lost its task"
        uppers=$((uppers + 1))
    fi
    if grep -qs '^- repo: -docs$' "$old/src/docs/.brain/project.md"; then
        # The folder's name, `docs`, is a repository's name without a `- repo:` line.
        if grep '^- repo: ' "$w/src/docs/.brain/project.md" | grep -v '^- repo: docs$' >/dev/null; then
            fail "$tag: repository \`-docs\` not renamed to \`docs\`"
        fi
        (cd "$w/src/docs" && h list | grep 'Docs task' >/dev/null) || fail "$tag: repository \`docs\` lost its task"
        repos=$((repos + 1))
    fi
}

# Every brain's entries as `kb` $1 shows them, under $3: one JSON file each.
record() {
    local kb=$1 w=$2 out=$3 brain name ids
    mkdir -p "$out"
    for brain in central shop/api shop/web solo; do
        if [ "$brain" = central ]; then cd "$w/home"; else cd "$w/src/$brain"; fi
        name=${brain//\//_}
        ids=$(person "$kb" "$w/home" list all notes decisions --json |
            python3 -I -c 'import json, sys; print(" ".join(str(e["id"]) for e in json.load(sys.stdin)))')
        for id in $ids; do
            person "$kb" "$w/home" show "$id" --json >"$out/$name.$id.json"
        done
    done
}

# What changed between the entries in $1 (the old version's) and $2 (this build's): nothing may be missing or
# different; a field only this build has is fine.
compare() {
    python3 -I - "$1" "$2" <<'EOF'
import json, os, sys

old_dir, new_dir = sys.argv[1], sys.argv[2]
# Minutes are rounded at the time each version ran; the files are a copy's.
ignored = {"tracked_minutes", "path"}
problems = []

def walk(old, new, at):
    if isinstance(old, dict):
        if not isinstance(new, dict):
            problems.append(f"{at}: {old!r} became {new!r}")
            return
        for key, value in old.items():
            if key in ignored:
                continue
            if key not in new:
                problems.append(f"{at}.{key}: missing ({value!r})")
            else:
                walk(value, new[key], f"{at}.{key}")
    elif isinstance(old, list):
        if not isinstance(new, list) or len(old) != len(new):
            problems.append(f"{at}: {old!r} became {new!r}")
            return
        for i, (o, n) in enumerate(zip(old, new)):
            walk(o, n, f"{at}[{i}]")
    elif old != new:
        problems.append(f"{at}: {old!r} became {new!r}")

for name in sorted(os.listdir(old_dir)):
    new_path = os.path.join(new_dir, name)
    if not os.path.exists(new_path):
        problems.append(f"{name}: not shown any more")
        continue
    walk(json.load(open(os.path.join(old_dir, name))), json.load(open(new_path)), name)
if problems:
    print("\n".join(problems))
sys.exit(1 if problems else 0)
EOF
}

tags=$(git -C "$repo" tag -l 'v*' --sort=version:refname)
[ -n "$tags" ] || fail "no release tags: fetch them (git fetch --tags)"
tested=0 dashes=0 uppers=0 repos=0
for tag in $tags; do
    # A tag whose release isn't out (being built, or its run failed) has nothing to download: not a failure here.
    status=$(curl -sSL -o /dev/null -w '%{http_code}' "$releases/$tag/$asset.sha256") || fail "$tag: no answer"
    if [ "$status" = 404 ]; then
        echo "skipped $tag: no release published"
        continue
    fi
    dir="$work/$tag"
    mkdir -p "$dir/bin"
    curl -fsSL -o "$dir/$asset" "$releases/$tag/$asset" || fail "$tag: downloading its kb failed"
    curl -fsSL -o "$dir/$asset.sha256" "$releases/$tag/$asset.sha256" || fail "$tag: no checksum"
    (cd "$dir" && sha256sum -c --quiet "$asset.sha256") || fail "$tag: checksum mismatch"
    tar -xzf "$dir/$asset" -C "$dir/bin"
    old_kb="$dir/bin/kb"

    # A subshell on its own, not in a `||` list, where bash turns `set -e` off for everything it runs: so the first
    # step that fails stops it, instead of only the last counting.
    set +e
    (set -e; build "$old_kb" "$dir/data") 2>"$dir/build.err"
    built=$?
    set -e
    [ "$built" -eq 0 ] || fail "$tag: writing its data failed: $(cat "$dir/build.err")"
    record "$old_kb" "$dir/data" "$dir/old"

    cp -a "$dir/data" "$dir/head"
    # The registry holds absolute paths: pointed at the copy, or this build would read and write the old one's.
    sed -i "s|$dir/data/|$dir/head/|" "$dir/head/home/central/projects.md"
    grep -q "$dir/head/src/shop/api" "$dir/head/home/central/projects.md" || fail "$tag: the copy's registry"
    record "$head_kb" "$dir/head" "$dir/new"
    (cd "$dir/head/home" && person "$head_kb" "$dir/head/home" status >/dev/null) || fail "$tag: status failed"
    for brain in home src/shop/api src/shop/web src/solo; do
        (cd "$dir/head/$brain" && person "$head_kb" "$dir/head/home" context >/dev/null &&
            person "$head_kb" "$dir/head/home" report week >/dev/null) || fail "$tag: reading $brain failed"
    done
    # The copy's registry aside, which names its own folders.
    if ! diff -r --exclude=.git --exclude=.cache --exclude=projects.md "$dir/data" "$dir/head" >"$dir/read.diff"; then
        cat "$dir/read.diff" >&2
        fail "$tag: reading changed files (above)"
    fi
    compare "$dir/old" "$dir/new" || fail "$tag: entries read differently (above)"

    (cd "$dir/head/home" && person "$head_kb" "$dir/head/home" init --all >/dev/null) || fail "$tag: init --all failed"
    renamed "$dir/data" "$dir/head"
    (cd "$dir/head/src/shop/api" && person "$head_kb" "$dir/head/home" comment 2 "from this build ✓" >/dev/null &&
        agent "$head_kb" "$dir/head/home" comment 1 "an agent, from this build" >/dev/null &&
        person "$head_kb" "$dir/head/home" start 3 >/dev/null && person "$head_kb" "$dir/head/home" park 3 >/dev/null) ||
        fail "$tag: writing with this build failed"
    echo "PASS $tag"
    tested=$((tested + 1))
done
[ "$tested" -gt 0 ] || fail "no release to upgrade from"
# Releases before v0.4.0 took these names and are never deleted, so some always wrote them.
if [ "$dashes" -eq 0 ] || [ "$uppers" -eq 0 ] || [ "$repos" -eq 0 ]; then
    fail "no release wrote the names v0.4.0 refuses (renamed: $dashes \`-dash\`, $uppers \`Central\`, $repos \`-docs\`)"
fi
echo "PASS upgrade.sh"
