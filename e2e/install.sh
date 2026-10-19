#!/usr/bin/env bash
# End-to-end check of install.sh as a user runs it, on Linux and macOS: this build of `kb` is packaged as a release
# asset, served over HTTP, installed into a throwaway home, and then:
#
# - the download is checked against its checksum, and refused when that doesn't match;
# - the installed `kb` runs, and `init --all` has brought a registered project's agent section up to date;
# - `--uninstall` takes it out again; `--purge` deletes the central brain's files and kanbrain's locks and backups in
#   the cache, keeping files it didn't create though named like its own, through a linked data folder, and refusing
#   the home folder however it's written and a relative KANBRAIN_DIR;
# - the kanban app gets its launcher with the logo: an application menu entry and icon on Linux, an app bundle on
#   macOS. `kb` stands in for the app, which takes the same commands, so GPUI needn't be built.
#
# CI runs it when a change touches it (the `installers` job). It needs cargo and python3, and touches nothing outside a
# temporary folder.
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
# shellcheck source=e2e/lib.sh
. "$(dirname "$0")/lib.sh"
cargo build -q --release --manifest-path "$repo/Cargo.toml" --bin kb
built="$repo/target/release/kb"

work=$(mktemp -d "${TMPDIR:-/tmp}/kb-install.XXXXXX")
server=""
cleanup() {
    # Waiting for the stopped server keeps bash from reporting it as "Terminated".
    if [ -n "$server" ]; then kill "$server" 2>/dev/null && { wait "$server" 2>/dev/null || true; }; fi
    rm -rf -- "$work"
}
trap cleanup EXIT

# The release assets install.sh downloads on this system, as the Release workflow names them.
case "$(uname -s)" in
    Linux)
        arch=$(uname -m)
        [ "$arch" = arm64 ] && arch=aarch64
        asset="kb-$arch-unknown-linux-musl.tar.gz"
        app_asset="kanbrain-$arch-unknown-linux-gnu.tar.gz"
        ;;
    Darwin)
        asset="kb-universal-apple-darwin.tar.gz"
        app_asset="kanbrain-universal-apple-darwin.tar.gz"
        ;;
    *) fail "unsupported system: $(uname -s)" ;;
esac
mkdir -p "$work/pkg" "$work/app" "$work/dist"
cp "$built" "$repo/LICENSE" "$work/pkg/"
tar -czf "$work/dist/$asset" -C "$work/pkg" kb LICENSE
# The app's archive, as the Release workflow makes it: on macOS with the icon for its bundle.
cp "$built" "$work/app/kanbrain"
cp "$repo/LICENSE" "$repo/gui/icons/kanbrain.icns" "$work/app/"
app_files="kanbrain LICENSE"
[ "$(uname -s)" = Darwin ] && app_files="$app_files kanbrain.icns"
# shellcheck disable=SC2086 # the file names are split on purpose
tar -czf "$work/dist/$app_asset" -C "$work/app" $app_files
for file in "$asset" "$app_asset"; do
    (cd "$work/dist" && if command -v sha256sum >/dev/null; then sha256sum "$file"; else shasum -a 256 "$file"; fi) \
        >"$work/dist/$file.sha256"
done

# Served over HTTP on a free port. Waited for by fetching from it: Python can take a while to start the first time
# on a fresh machine, as on CI's macOS.
port=$(python3 -c 'import socket; s = socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1])')
python3 -m http.server "$port" --bind 127.0.0.1 --directory "$work/dist" >"$work/server.log" 2>&1 &
server=$!
for _ in $(seq 120); do
    curl -fsS -o /dev/null "http://127.0.0.1:$port/$asset.sha256" 2>/dev/null && break
    sleep 0.5
done
curl -fsS -o /dev/null "http://127.0.0.1:$port/$asset.sha256" || fail "the test server didn't start: $(cat "$work/server.log")"

export HOME="$work/home" KANBRAIN_DIR="$work/home/central" SHELL=/bin/bash
# The menu entry and icon then go under $HOME/.local/share, where the checks below look.
unset XDG_DATA_HOME
export KANBRAIN_BASE_URL="http://127.0.0.1:$port"
mkdir -p "$HOME/code/demo"

# A project registered by an older version: its agent section is out of date.
(cd "$HOME/code/demo" && "$built" init demo >/dev/null)
claude="$HOME/code/demo/CLAUDE.md"
printf '<!-- kanbrain:start -->\nold advice\n<!-- kanbrain:end -->\n' >"$claude"

out=$(sh "$repo/install.sh" --app cli 2>&1) || fail "install failed: $out"
echo "$out"
installed="$HOME/.local/bin/kb"
case "$out" in *"checksum ok"*) ;; *) fail "the checksum wasn't checked" ;; esac
[ "$("$installed" --version)" = "$("$built" --version)" ] || fail "the installed kb isn't this build"
case "$out" in *"demo: updated CLAUDE.md"*) ;; *) fail "init --all didn't run" ;; esac
grep -q "## Project memory: kanbrain" "$claude" || fail "the project's section wasn't updated"

# Both apps are fetched before either is put in place: the app's download failing leaves the installed kb as it
# was (the same file, not replaced), so the two never end up at different versions.
touch -t 200001010000 "$installed"
: >"$work/stamp"
mv "$work/dist/$app_asset" "$work/app-asset.away"
if sh "$repo/install.sh" --app both >/dev/null 2>&1; then fail "an install missing the app's download succeeded"; fi
[ "$installed" -ot "$work/stamp" ] || fail "a failed install replaced kb before fetching the app"
mv "$work/app-asset.away" "$work/dist/$app_asset"
# A `kb.new` left by an install cut short doesn't swallow the next one.
mkdir "$installed.new"
sh "$repo/install.sh" --app cli >/dev/null || fail "installing over a leftover kb.new failed"
[ -f "$installed" ] || fail "kb isn't a file after installing over a leftover kb.new"

# A download that doesn't match its checksum is refused, and the installed kb is left as it was.
cp "$work/dist/$asset.sha256" "$work/good.sha256"
printf '%064d  %s\n' 0 "$asset" >"$work/dist/$asset.sha256"
if bad=$(sh "$repo/install.sh" --app cli 2>&1); then fail "a wrong checksum was installed: $bad"; fi
case "$bad" in *"checksum mismatch"*) ;; *) fail "a wrong checksum failed for another reason: $bad" ;; esac
"$installed" --version >/dev/null || fail "a refused download broke the installed kb"
cp "$work/good.sha256" "$work/dist/$asset.sha256"

sh "$repo/install.sh" --uninstall --app cli >/dev/null || fail "uninstall failed"
[ ! -e "$installed" ] || fail "uninstall left $installed"
grep -q "## Project memory: kanbrain" "$claude" || fail "uninstall touched the project"

# The kanban app, and its launcher with the logo.
sh "$repo/install.sh" --app gui >/dev/null || fail "installing the app failed"
case "$(uname -s)" in
    Linux)
        desktop="$HOME/.local/share/applications/kanbrain.desktop"
        icon="$HOME/.local/share/icons/hicolor/scalable/apps/kanbrain.svg"
        grep -q "^Icon=kanbrain$" "$desktop" || fail "the menu entry names no icon"
        # A folder whose name the menu entry's Exec has to escape.
        odd="$HOME/odd dir\$x%y"
        sh "$repo/install.sh" --app gui --dir "$odd" >/dev/null || fail "installing the app into $odd failed"
        grep -qxF "Exec=\"$HOME/odd dir\\\\\$x%%y/kanbrain\"" "$desktop" || fail "Exec isn't escaped: $(grep Exec "$desktop")"
        if command -v desktop-file-validate >/dev/null; then
            desktop-file-validate "$desktop" || fail "the menu entry isn't valid"
        fi
        sh "$repo/install.sh" --app gui >/dev/null || fail "installing the app back failed"
        cmp -s "$icon" "$repo/docs/images/logo.svg" || fail "the installed icon isn't the logo"
        ;;
    Darwin)
        bundle="$HOME/Applications/kanbrain.app/Contents"
        cmp -s "$bundle/Resources/kanbrain.icns" "$repo/gui/icons/kanbrain.icns" || fail "the bundle has no icon"
        grep -q "<key>CFBundleIconFile</key><string>kanbrain</string>" "$bundle/Info.plist" ||
            fail "Info.plist names no icon"
        plutil -lint "$bundle/Info.plist" >/dev/null || fail "Info.plist isn't valid"
        "$bundle/MacOS/kanbrain" --version >/dev/null || fail "the bundle's executable doesn't run"
        [ "$(cat "$bundle/Resources/install-dir")" = "$HOME/.local/bin" ] ||
            fail "the bundle doesn't say where the app's update goes"
        ;;
esac
KANBRAIN_UNINSTALL=1 sh "$repo/install.sh" --app gui >/dev/null || fail "uninstalling the app through KANBRAIN_UNINSTALL failed"
case "$(uname -s)" in
    Linux)
        if [ -e "$desktop" ] || [ -e "$icon" ]; then fail "uninstall left the menu entry or its icon"; fi
        ;;
    Darwin) [ ! -e "$HOME/Applications/kanbrain.app" ] || fail "uninstall left the app bundle" ;;
esac

# --purge deletes the central brain's data and kanbrain's locks and backups in the cache, and nothing else of either.
case "$(uname -s)" in
    Darwin) cache="$HOME/Library/Caches/kanbrain" ;;
    *) cache="${XDG_CACHE_HOME:-$HOME/.cache}/kanbrain" ;;
esac
mkdir -p "$cache/locks" "$cache/backups/0123456789abcdef/20261013T120000-format-1-to-2"
: >"$cache/locks/0123456789abcdef.lock"
[ -d "$KANBRAIN_DIR" ] || fail "the central brain wasn't there to purge"
# The data folder kept in a synced folder through a link, and shared with notes of the user's own, as in an
# Obsidian vault: purge takes only what kanbrain made, by what each file holds, not by its name.
mkdir -p "$HOME/Sync"
mv "$KANBRAIN_DIR" "$HOME/Sync/central"
ln -s "$HOME/Sync/central" "$KANBRAIN_DIR"
data="$HOME/Sync/central"
printf '# Mine\n\n- id: 7\n- status: todo\n- tags: \n' >"$data/0007-mine.md"
: >"$data/notes-of-mine.txt"
printf '# Daily\n' >"$data/2024-01-15.md"
printf '# Meeting\n' >"$data/2024-01-15-meeting.md"
printf 'my own format notes\n' >"$data/format"
mkdir -p "$data/reports"
printf '# Website redesign\n' >"$data/projects/website-redesign.md"
printf '# Q3\n' >"$data/reports/q3.md"
printf '# Time report · week 42\n' >"$data/reports/kanbrain-week.md"
: >"$data/.0001-left-by-a-crash.md.4242-0.tmp"
: >"$data/projects/.demo.md.4242-1.tmp"
# A data folder with no entry, as kanbrain makes it before the first: purged and the apps uninstalled all the same.
sh "$repo/install.sh" --app cli >/dev/null || fail "installing again failed"
empty="$HOME/empty-central"
mkdir -p "$empty"
printf '1\n' >"$empty/format"
KANBRAIN_DIR="$empty" sh "$repo/install.sh" --purge --yes --app cli >/dev/null || fail "purging a brain with no entry failed"
[ ! -e "$installed" ] || fail "purging a brain with no entry left kb installed"
[ ! -e "$empty" ] || fail "purging a brain with no entry kept it"
# A data folder that's the home folder, however it's written, is refused before anything is deleted.
if KANBRAIN_DIR="$HOME//" sh "$repo/install.sh" --purge --yes >/dev/null 2>&1; then fail "purge took \$HOME// as data"; fi
# A relative one, which names another folder from each folder kb runs in, is refused as kb refuses it.
if (cd "$HOME" && KANBRAIN_DIR=Sync/central sh "$repo/install.sh" --purge --yes >/dev/null 2>&1); then
    fail "purge took a relative KANBRAIN_DIR"
fi
[ -e "$data/0007-mine.md" ] || fail "a refused purge deleted something"
sh "$repo/install.sh" --purge --yes >/dev/null || fail "purging failed"
[ ! -e "$cache" ] || fail "purge left kanbrain's cache"
for gone in 0007-mine.md projects.md tags.md .0001-left-by-a-crash.md.4242-0.tmp projects/demo.md \
    projects/.demo.md.4242-1.tmp reports/kanbrain-week.md; do
    [ ! -e "$data/$gone" ] || fail "purge kept kanbrain's $gone"
done
for kept in notes-of-mine.txt 2024-01-15.md 2024-01-15-meeting.md format projects/website-redesign.md reports/q3.md; do
    [ -e "$data/$kept" ] || fail "purge deleted $kept, which kanbrain didn't make"
done
grep -q "## Project memory: kanbrain" "$claude" || fail "purge touched the project"

echo "PASS install.sh"
