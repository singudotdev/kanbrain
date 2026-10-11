#!/bin/sh
# kanbrain installer for Linux and macOS.
#
#   curl -fsSL <url>/install.sh | sh
#   wget -qO-  <url>/install.sh | sh
#
# Options (after `sh -s --` when piping):
#   --app cli|gui|both which app: the command line `kb` (default), the kanban app `kanbrain` (which also takes
#                      kb's commands), or both. Both use the same tasks. With --uninstall/--purge the default is both.
#   --version vX.Y.Z   install a specific release (default: latest)
#   --dir PATH         install directory inside your home (default: ~/.local/bin)
#   --uninstall        remove the app(s) (your tasks are kept)
#   --purge            remove the app(s) AND your central brain's data: entries, tags.md, the project registry
#                      and cards, exported reports (asks for confirmation; add --yes to skip it). Projects'
#                      own .brain/ folders are kept.
#
# kanbrain is installed and used per user: this script refuses to run as root/sudo and only
# installs inside $HOME. (If root is your only user, e.g. in a container: KANBRAIN_ALLOW_ROOT=1.)
#
# Environment overrides: KANBRAIN_APP, KANBRAIN_VERSION, KANBRAIN_INSTALL_DIR, KANBRAIN_REPO,
# KANBRAIN_HOST (github | gitlab), KANBRAIN_GITLAB_URL, KANBRAIN_BASE_URL, KANBRAIN_ALLOW_ROOT.

set -eu

# The whole script is one block, so `sh` reads all of it before running any: a download cut short runs nothing, and
# an early exit (--uninstall, --help) doesn't close the pipe on a curl still writing, which made it fail with "(23)".
{

# ---- where releases are published: edit these two lines for your repository ----
REPO="${KANBRAIN_REPO:-singudotdev/kanbrain}"
HOST="${KANBRAIN_HOST:-github}"

APP="${KANBRAIN_APP:-}"
VERSION="${KANBRAIN_VERSION:-latest}"
INSTALL_DIR="${KANBRAIN_INSTALL_DIR:-$HOME/.local/bin}"
UNINSTALL=0
PURGE=0
YES=0

say() { printf '%s\n' "kanbrain: $*"; }
die() { printf '%s\n' "kanbrain: error: $*" >&2; exit 1; }

while [ $# -gt 0 ]; do
    case "$1" in
        --app) [ $# -ge 2 ] || die "--app needs a value"; APP="$2"; shift 2 ;;
        --version) [ $# -ge 2 ] || die "--version needs a value"; VERSION="$2"; shift 2 ;;
        --dir) [ $# -ge 2 ] || die "--dir needs a value"; INSTALL_DIR="$2"; shift 2 ;;
        --uninstall) UNINSTALL=1; shift ;;
        --purge) UNINSTALL=1; PURGE=1; shift ;;
        -y | --yes) YES=1; shift ;;
        -h | --help)
            printf '%s\n' "usage: install.sh [--app cli|gui|both] [--version vX.Y.Z] [--dir PATH] [--uninstall | --purge [--yes]]"
            exit 0
            ;;
        *) die "unknown option: $1" ;;
    esac
done

# ---- per-user only ----
if [ "$(id -u)" = 0 ] && [ "${KANBRAIN_ALLOW_ROOT:-}" != 1 ]; then
    die "kanbrain is installed per user: run this as your normal user, not as root or with sudo.
       (Only if root is your only user, e.g. a container, set KANBRAIN_ALLOW_ROOT=1.)"
fi
case "$INSTALL_DIR" in /*) ;; *) INSTALL_DIR="$PWD/$INSTALL_DIR" ;; esac
INSTALL_DIR=${INSTALL_DIR%/}
case "$INSTALL_DIR" in
    *..*) die "install folder must be a plain path without '..': $INSTALL_DIR" ;;
esac
case "$INSTALL_DIR/" in
    "${HOME%/}"/*) ;;
    *) die "install folder must be inside your home folder (${HOME%/}), got $INSTALL_DIR" ;;
esac

# ---- which apps ----
if [ -z "$APP" ]; then
    if [ "$UNINSTALL" = 1 ]; then APP=both; else APP=cli; fi
fi
case "$APP" in
    cli) BINS="kb" ;;
    gui) BINS="kanbrain" ;;
    both) BINS="kb kanbrain" ;;
    *) die "--app must be cli, gui or both, got $APP" ;;
esac

# The desktop app's entry in the Linux application menu, and the one versions before 0.3 made for kanbrain-gui.
DESKTOP_FILE="${XDG_DATA_HOME:-$HOME/.local/share}/applications/kanbrain.desktop"
OLD_DESKTOP_FILE="${XDG_DATA_HOME:-$HOME/.local/share}/applications/kanbrain-gui.desktop"
# Its icon, by the name the entry and the app's window (its Wayland app id) give.
ICON_FILE="${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor/scalable/apps/kanbrain.svg"
# On macOS the app's icon belongs to an app bundle: Launchpad, Spotlight and the Dock show the app from it.
APP_BUNDLE="$HOME/Applications/kanbrain.app"

# Whether $INSTALL_DIR/kanbrain is the command line of a version before 0.3, when the command line was
# `kanbrain` and the app `kanbrain-gui`: any `kanbrain` that old is the command line.
old_cli() {
    v=$("$INSTALL_DIR/kanbrain" --version 2>/dev/null) || return 1
    case "$v" in "kanbrain 0."[0-2].*) return 0 ;; esac
    return 1
}

# Removes what versions before 0.3 installed in place of `$1`: the app `kanbrain-gui` and its menu entry
# for the app, the old `kanbrain` command line for `kb`.
remove_old() {
    case "$1" in
        kanbrain)
            for old in "$INSTALL_DIR/kanbrain-gui" "$OLD_DESKTOP_FILE"; do
                if [ -e "$old" ]; then rm -f "$old" && say "removed $old (from before 0.3)"; fi
            done
            ;;
        kb)
            if old_cli; then rm -f "$INSTALL_DIR/kanbrain" && say "removed the old command line $INSTALL_DIR/kanbrain"; fi
            ;;
    esac
}

# Where the central brain is: ask the installed command line, else mirror its defaults. `--project central`,
# because run inside a project `path` names that project's .brain/, which purge must not touch.
data_dir() {
    for bin in "$INSTALL_DIR/kb" "$INSTALL_DIR/kanbrain"; do
        if [ -x "$bin" ] && dir=$("$bin" --project central path 2>/dev/null) && [ -n "$dir" ]; then
            printf '%s\n' "$dir"
            return
        fi
    done
    if [ -n "${KANBRAIN_DIR:-}" ]; then
        printf '%s\n' "$KANBRAIN_DIR"
    elif [ "$(uname -s)" = Darwin ]; then
        printf '%s\n' "$HOME/Library/Application Support/kanbrain"
    else
        printf '%s\n' "${XDG_DATA_HOME:-$HOME/.local/share}/kanbrain"
    fi
}

# Files kanbrain creates in the central brain: NNNN-slug.md entries, tags.md, last-id, the project registry
# (projects.md) and cards (projects/*.md), exported reports, older versions' config.md, leftover temp files.
owned_files() {
    find "$1" -maxdepth 1 -type f \( -name '[0-9][0-9][0-9][0-9]*-*.md' -o -name 'tags.md' -o -name 'projects.md' \
        -o -name 'last-id' -o -name 'config.md' -o -name '.*.md.tmp' -o -name '.last-id.tmp' \)
    for sub in projects reports; do
        if [ -d "$1/$sub" ]; then find "$1/$sub" -maxdepth 1 -type f -name '*.md'; fi
    done
}

confirm() {
    [ "$YES" = 1 ] && return 0
    # stdin is the script itself when piped from curl/wget, so ask on the terminal.
    if { true </dev/tty; } 2>/dev/null; then
        printf 'kanbrain: %s [y/N] ' "$1" >/dev/tty
        read -r answer </dev/tty || answer=""
        case "$answer" in y | Y | yes | YES) return 0 ;; esac
        return 1
    fi
    die "no terminal to confirm on; re-run with --yes to delete your data"
}

if [ "$UNINSTALL" = 1 ]; then
    if [ "$PURGE" = 1 ]; then
        data=$(data_dir)
        case "$data" in
            "" | / | "$HOME" | "$HOME/") die "refusing to delete data in '$data'" ;;
        esac
        if [ -d "$data" ]; then
            entries=$(find "$data" -maxdepth 1 -type f -name '[0-9][0-9][0-9][0-9]*-*.md' | wc -l | tr -d ' ')
            say "data folder: $data"
            say "this permanently deletes $entries entry file(s), tags.md, the project registry and cards, and exported reports"
            say "projects' own .brain/ folders are kept"
            confirm "delete your kanbrain data?" || die "cancelled, nothing was removed"
        else
            say "no data folder at $data"
            data=""
        fi
    fi

    for bin in $BINS; do
        # Before `kb`, the command line itself was `kanbrain`: take that out first, while it can still be told
        # apart from the app.
        remove_old "$bin"
        if [ -e "$INSTALL_DIR/$bin" ]; then
            rm -f "$INSTALL_DIR/$bin"
            say "removed $INSTALL_DIR/$bin"
        else
            say "no binary at $INSTALL_DIR/$bin"
        fi
        if [ "$bin" = kanbrain ] && [ -e "$DESKTOP_FILE" ]; then
            rm -f "$DESKTOP_FILE" "$ICON_FILE"
            say "removed $DESKTOP_FILE"
        fi
        if [ "$bin" = kanbrain ] && [ -d "$APP_BUNDLE" ]; then
            rm -rf -- "$APP_BUNDLE"
            say "removed $APP_BUNDLE"
        fi
    done

    if [ "$PURGE" = 1 ] && [ -n "$data" ]; then
        owned_files "$data" | while IFS= read -r f; do rm -f "$f"; done
        rmdir "$data/projects" "$data/reports" 2>/dev/null || true
        if rmdir "$data" 2>/dev/null; then
            say "removed $data"
        else
            say "deleted kanbrain's files; kept $data because it contains other files"
        fi
    elif [ "$PURGE" = 0 ]; then
        say "your tasks were kept in $(data_dir) (use --purge to delete them too)"
    fi
    exit 0
fi

# ---- detect platform ----
os=$(uname -s)
arch=$(uname -m)
case "$arch" in
    x86_64 | amd64) arch=x86_64 ;;
    aarch64 | arm64) arch=aarch64 ;;
    *) die "unsupported CPU architecture: $arch" ;;
esac
case "$os" in
    Linux | Darwin) ;;
    MINGW* | MSYS* | CYGWIN*) die "on Windows, use the PowerShell installer: irm <url>/install.ps1 | iex" ;;
    *) die "unsupported operating system: $os" ;;
esac

# The release file for one app. On Linux the command line is static (musl); the desktop app links the
# system's graphics libraries (glibc).
asset_for() {
    case "$os:$1" in
        Linux:kb) printf '%s\n' "kb-$arch-unknown-linux-musl.tar.gz" ;;
        Linux:kanbrain) printf '%s\n' "kanbrain-$arch-unknown-linux-gnu.tar.gz" ;;
        Darwin:*) printf '%s\n' "$1-universal-apple-darwin.tar.gz" ;;
    esac
}

# ---- download location ----
if [ -n "${KANBRAIN_BASE_URL:-}" ]; then
    base="$KANBRAIN_BASE_URL"
else
    case "$HOST" in
        github)
            if [ "$VERSION" = latest ]; then
                base="https://github.com/$REPO/releases/latest/download"
            else
                base="https://github.com/$REPO/releases/download/$VERSION"
            fi
            ;;
        gitlab)
            gl="${KANBRAIN_GITLAB_URL:-https://gitlab.com}"
            if [ "$VERSION" = latest ]; then
                base="$gl/$REPO/-/releases/permalink/latest/downloads"
            else
                base="$gl/$REPO/-/releases/$VERSION/downloads"
            fi
            ;;
        *) die "KANBRAIN_HOST must be github or gitlab" ;;
    esac
fi

if command -v curl >/dev/null 2>&1; then
    fetch() { curl -fsSL --retry 3 -o "$2" "$1"; }
elif command -v wget >/dev/null 2>&1; then
    fetch() { wget -q -O "$2" "$1"; }
else
    die "curl or wget is required"
fi

tmp=$(mktemp -d 2>/dev/null || mktemp -d -t kanbrain)
trap 'rm -rf "$tmp"' EXIT INT TERM

mkdir -p "$INSTALL_DIR" || die "cannot create $INSTALL_DIR (try --dir)"
[ -w "$INSTALL_DIR" ] || die "$INSTALL_DIR is not writable (choose another folder in your home with --dir)"

# Downloads, verifies and installs one app.
install_app() {
    bin=$1
    asset=$(asset_for "$bin")
    say "downloading $asset ($VERSION)"
    fetch "$base/$asset" "$tmp/$asset" || die "download failed: $base/$asset"

    # ---- verify checksum ----
    # Every release publishes one; without it, nothing says the download is the release's.
    fetch "$base/$asset.sha256" "$tmp/$asset.sha256" 2>/dev/null || die "no checksum found for $asset: $base/$asset.sha256"
    expected=$(cut -d' ' -f1 <"$tmp/$asset.sha256")
    if command -v sha256sum >/dev/null 2>&1; then
        actual=$(sha256sum "$tmp/$asset" | cut -d' ' -f1)
    elif command -v shasum >/dev/null 2>&1; then
        actual=$(shasum -a 256 "$tmp/$asset" | cut -d' ' -f1)
    else
        die "sha256sum or shasum is needed to check $asset"
    fi
    [ "$actual" = "$expected" ] || die "checksum mismatch for $asset (expected $expected, got $actual)"
    say "checksum ok"

    # ---- install ----
    remove_old "$bin"
    tar -xzf "$tmp/$asset" -C "$tmp" || die "could not extract $asset"
    [ -f "$tmp/$bin" ] || die "archive does not contain a '$bin' binary"
    # Replace atomically so a running copy isn't disturbed.
    cp "$tmp/$bin" "$INSTALL_DIR/$bin.new"
    chmod 755 "$INSTALL_DIR/$bin.new"
    mv -f "$INSTALL_DIR/$bin.new" "$INSTALL_DIR/$bin"
    if [ "$os" = Darwin ]; then
        xattr -d com.apple.quarantine "$INSTALL_DIR/$bin" 2>/dev/null || true
    fi
    say "installed $("$INSTALL_DIR/$bin" --version 2>/dev/null || echo "$bin") to $INSTALL_DIR/$bin"

    # On macOS, an app bundle with the app's icon (in its release archive), launching the installed binary. Its
    # executable is a hard link to it, not a copy or a script: the Dock shows the bundle's icon only for the
    # bundle's own executable, and the link costs no space. A copy where they can't link.
    if [ "$bin" = kanbrain ] && [ "$os" = Darwin ] && [ -f "$tmp/kanbrain.icns" ]; then
        mkdir -p "$APP_BUNDLE/Contents/MacOS" "$APP_BUNDLE/Contents/Resources"
        cp "$tmp/kanbrain.icns" "$APP_BUNDLE/Contents/Resources/kanbrain.icns"
        rm -f "$APP_BUNDLE/Contents/MacOS/kanbrain"
        ln "$INSTALL_DIR/kanbrain" "$APP_BUNDLE/Contents/MacOS/kanbrain" 2>/dev/null ||
            cp "$INSTALL_DIR/kanbrain" "$APP_BUNDLE/Contents/MacOS/kanbrain"
        version=$("$INSTALL_DIR/kanbrain" --version 2>/dev/null | sed 's/^kanbrain //')
        cat >"$APP_BUNDLE/Contents/Info.plist" <<EOF_PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>kanbrain</string>
    <key>CFBundleDisplayName</key><string>kanbrain</string>
    <key>CFBundleIdentifier</key><string>dev.singu.kanbrain</string>
    <key>CFBundleExecutable</key><string>kanbrain</string>
    <key>CFBundleIconFile</key><string>kanbrain</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>${version:-0}</string>
    <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
EOF_PLIST
        # So Finder and Launchpad pick up the new icon and version.
        touch "$APP_BUNDLE"
        say "added kanbrain to your Applications ($APP_BUNDLE)"
    fi

    # The desktop app goes in the Linux application menu.
    if [ "$bin" = kanbrain ] && [ "$os" = Linux ]; then
        mkdir -p "$(dirname "$DESKTOP_FILE")" "$(dirname "$ICON_FILE")"
        # docs/images/logo.svg, which a test keeps it equal to.
        cat >"$ICON_FILE" <<'EOF_ICON'
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512" width="512" height="512">
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="#1B263B"/>
      <stop offset="1" stop-color="#0D1B2A"/>
    </linearGradient>
  </defs>
  <rect width="512" height="512" rx="112" fill="url(#bg)"/>
  <g transform="translate(256 256) scale(1.12) translate(-250 -256)">
    <g fill="none" stroke="#64FFDA" stroke-width="24" stroke-linecap="round" stroke-linejoin="round">
      <path d="M248 128 C 218 104 168 106 152 138 C 112 140 90 176 100 210 C 74 230 74 276 100 294 C 90 330 114 366 152 370 C 170 400 220 408 248 384 Z"/>
      <path d="M196 152 C 192 180 212 196 236 192"/>
      <path d="M140 262 C 170 252 200 270 202 300"/>
    </g>
    <g fill="#64FFDA">
      <rect x="274" y="138" width="110" height="60" rx="18"/>
      <rect x="274" y="224" width="138" height="64" rx="18" opacity="0.62"/>
      <rect x="274" y="314" width="104" height="60" rx="18" opacity="0.38"/>
    </g>
  </g>
</svg>
EOF_ICON
        cat >"$DESKTOP_FILE" <<EOF_DESKTOP
[Desktop Entry]
Type=Application
Name=kanbrain
GenericName=Kanban board
Comment=Tasks in To do, Doing, Parked and Done columns, with time tracking, stored as markdown files
Exec="$INSTALL_DIR/kanbrain"
Icon=kanbrain
Terminal=false
Categories=Office;ProjectManagement;
EOF_DESKTOP
        say "added kanbrain to your application menu ($DESKTOP_FILE)"
    fi
}

for bin in $BINS; do
    install_app "$bin"
done

# Projects' agents follow the new version: their section and hooks are refreshed in every registered repository.
# Only by a version that has `init --all`: an older one would take `--all` for a project's name.
first=${BINS%% *}
if "$INSTALL_DIR/$first" --help 2>/dev/null | grep -q "init --all"; then
    if out=$("$INSTALL_DIR/$first" init --all 2>&1); then
        printf '%s\n' "$out" | while IFS= read -r line; do [ -z "$line" ] || say "$line"; done
    else
        say "warning: couldn't update your projects' agent instructions ($out); run: $first init --all"
    fi
fi

# ---- PATH hint ----
case ":$PATH:" in
    *":$INSTALL_DIR:"*) say "run: $(printf '%s\n' "$BINS" | sed 's/ / or /')" ;;
    *)
        say "$INSTALL_DIR is not in your PATH. Add it with:"
        # shellcheck disable=SC2016 # $PATH is meant literally in the printed hint
        case "${SHELL:-sh}" in
            */fish | fish) printf '    fish_add_path %s\n' "$INSTALL_DIR" ;;
            */zsh | zsh) printf '    echo '\''export PATH="%s:$PATH"'\'' >> ~/.zshrc && exec zsh\n' "$INSTALL_DIR" ;;
            *) printf '    echo '\''export PATH="%s:$PATH"'\'' >> ~/.bashrc && exec bash\n' "$INSTALL_DIR" ;;
        esac
        ;;
esac

}
