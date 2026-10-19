#!/usr/bin/env bash
# Smoke check of the kanban app as built for release, on Linux and macOS: it starts on a few demo tasks, stays up,
# keeps idle (nothing running, no message showing: it redraws once a minute), and a screenshot of the screen is saved
# for a person to look at; on Linux, without a display it fails saying why. Windows has the same check in
# e2e/smoke-app.ps1.
#
# Usage: e2e/smoke-app.sh [APP] [OUT]: APP is the app's binary (target/release/kanbrain), OUT the folder the
# screenshot goes to (target/smoke). Linux needs a display: CI runs it under `xvfb-run`, with Mesa's software
# Vulkan, and ImageMagick's `import` for the screenshot; macOS uses `screencapture`.
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
# shellcheck source=e2e/lib.sh
. "$(dirname "$0")/lib.sh"
app=${1:-$repo/target/release/kanbrain}
out=${2:-$repo/target/smoke}
# Percent of one core the idle app may use, averaged over the measure.
idle_limit=3
work=$(mktemp -d "${TMPDIR:-/tmp}/kb-smoke.XXXXXX")
pid=""
cleanup() {
    if [ -n "$pid" ]; then kill "$pid" 2>/dev/null || true; fi
    rm -rf -- "$work"
}
trap cleanup EXIT

export HOME="$work/home" KANBRAIN_DIR="$work/home/central"
unset "${agent_variables[@]}"
mkdir -p "$KANBRAIN_DIR" "$out"
# The app takes `kb`'s commands with arguments: the demo tasks, none running.
"$app" add --tag auth "Rate-limit the login endpoint" >/dev/null
"$app" add --note --body "Retries are idempotent." "Webhooks arrive more than once" >/dev/null
"$app" "done" "$("$app" add "Paginate GET /orders" | grep -o '#[0-9]*' | tr -d '#')" >/dev/null

# Started with ENV (none: no display at all), the app fails within the timeout saying WHY, rather than running
# headless, unseen, forever, or panicking.
no_display() {
    local why=$1 status=0
    shift
    env -u DISPLAY -u WAYLAND_DISPLAY -u WAYLAND_SOCKET "$@" timeout 30 "$app" >"$work/no-display.log" 2>&1 ||
        status=$?
    [ "$status" -eq 1 ] || fail "with '$*' the app exited with $status, not 1: $(cat "$work/no-display.log")"
    grep -qF -- "$why" "$work/no-display.log" || fail "with '$*' the app didn't say why: $(cat "$work/no-display.log")"
}
if [ "$(uname -s)" = Linux ]; then
    no_display 'run kanbrain from the desktop session'
    no_display "no Wayland compositor at WAYLAND_DISPLAY=$work/no-wayland" WAYLAND_DISPLAY="$work/no-wayland"
    # Not :99, the first display xvfb-run -a tries: CI's own X server may be there.
    no_display 'no X server at DISPLAY=:9876' DISPLAY=:9876
fi

"$app" >"$work/app.log" 2>&1 &
pid=$!
# Its first frames, fonts loaded.
sleep 15
kill -0 "$pid" 2>/dev/null || fail "the app exited: $(cat "$work/app.log")"
# Failing to open the board, it says why on stderr and in a window that stays up until closed.
if grep -q '^Error:' "$work/app.log"; then fail "the app couldn't open the board: $(cat "$work/app.log")"; fi

# CPU time the app's process used, in hundredths of a second.
cpu() {
    case "$(uname -s)" in
        Linux)
            # utime and stime, fields 14 and 15 of /proc/PID/stat, in clock ticks; after the `(name)`, which may
            # hold spaces, they're the 12th and 13th.
            local ticks
            ticks=$(getconf CLK_TCK)
            sed 's/.*) //' "/proc/$pid/stat" | awk -v t="$ticks" '{ print int(($12 + $13) * 100 / t) }'
            ;;
        Darwin)
            # `ps -o time` is M:SS.ss (or H:MM:SS).
            ps -o time= -p "$pid" | awk -F: '{ s = 0; for (i = 1; i <= NF; i++) s = s * 60 + $i; print int(s * 100) }'
            ;;
    esac
}
before=$(cpu)
sleep 20
after=$(cpu)
used=$(((after - before) / 20))
echo "idle: ${used}% of a core"
[ "$used" -le "$idle_limit" ] || fail "the idle app used ${used}% of a core, over ${idle_limit}%"

case "$(uname -s)" in
    Linux)
        if command -v import >/dev/null; then
            import -window root "$out/app-linux.png"
        else
            echo "no ImageMagick import: no screenshot"
        fi
        ;;
    # A runner may not let it capture the screen: the app's check stands without it.
    Darwin) screencapture -x "$out/app-macos.png" || echo "screencapture failed: no screenshot" ;;
esac
kill -0 "$pid" 2>/dev/null || fail "the app exited while idle: $(cat "$work/app.log")"
echo "PASS smoke-app.sh"
