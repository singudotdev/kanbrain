# Getting started

## Install with the installer (recommended)

kanbrain comes as two programs that share your tasks. Install either one, or both:

| program | command | what it is |
| --- | --- | --- |
| **command line** (default) | `kb` | the CLI that agents and scripts use; works over SSH. One self-contained file, about 1 MB, no dependencies |
| **kanban app** | `kanbrain` | the board: columns you drag cards across, details beside them, notes, decisions and the time report. Given arguments, it takes `kb`'s commands too (`kanbrain context`) |

The installer picks the right file for your system, verifies its SHA-256 checksum (it refuses to install without
one) and installs it for **your user only**.
The kanban app also gets its launcher, with its icon: an entry in your application menu (Linux), the app in
`~/Applications` (macOS, so Launchpad, Spotlight and the Dock show it), or a Start Menu entry (Windows).

> **kanbrain is a per-user app.** Each user installs their own copy in their home folder and has their own tasks.
> - The installers **refuse to run as root / with `sudo`** (Linux, macOS) or **from an elevated "Run as administrator" PowerShell** (Windows).
> - The install folder must be **inside your home folder** (Linux/macOS) or **user profile** (Windows). System folders such as `/usr/local/bin` or `Program Files` are rejected.
> - The app itself **refuses to run as root**, so `sudo kanbrain` can't create root-owned files in your data folder.
>
> If root is genuinely your only user, e.g. in a container, opt in with `KANBRAIN_ALLOW_ROOT=1`
> (for both the installer and the app). The Windows equivalent for admin-only accounts is `$env:KANBRAIN_ALLOW_ADMIN = "1"`.

### Linux and macOS

```sh
curl -fsSL https://github.com/singudotdev/kanbrain/releases/latest/download/install.sh | sh
```

No curl? Use `wget -qO- … | sh`. Only a POSIX `sh` is required (bash, dash, busybox and zsh all work).

Options go after `sh -s --`. The kanban app instead of the command line:

```sh
curl -fsSL https://github.com/singudotdev/kanbrain/releases/latest/download/install.sh | sh -s -- --app gui
```

Both:

```sh
curl -fsSL https://github.com/singudotdev/kanbrain/releases/latest/download/install.sh | sh -s -- --app both
```

A specific release:

```sh
curl -fsSL https://github.com/singudotdev/kanbrain/releases/latest/download/install.sh | sh -s -- --version v0.5.2
```

Another folder in your home (the default is `~/.local/bin`):

```sh
curl -fsSL https://github.com/singudotdev/kanbrain/releases/latest/download/install.sh | sh -s -- --dir ~/bin
```

Uninstalling uses the same script, see [Uninstalling](#uninstalling).

If the install folder isn't on your `PATH`, the installer prints the exact line to add for your shell.

### Windows

In PowerShell (Windows PowerShell 5.1 or PowerShell 7):

```powershell
irm https://github.com/singudotdev/kanbrain/releases/latest/download/install.ps1 | iex
```

It installs to `%LOCALAPPDATA%\Programs\kanbrain\kb.exe` (and `kanbrain.exe` for the kanban app) and adds that
folder to your **user** `PATH`, so open a new terminal afterwards.
Options are environment variables. The kanban app instead of the command line:

```powershell
$env:KANBRAIN_APP = "gui"; irm https://github.com/singudotdev/kanbrain/releases/latest/download/install.ps1 | iex
```

Both:

```powershell
$env:KANBRAIN_APP = "both"; irm https://github.com/singudotdev/kanbrain/releases/latest/download/install.ps1 | iex
```

A specific release:

```powershell
$env:KANBRAIN_VERSION = "v0.5.2"; irm https://github.com/singudotdev/kanbrain/releases/latest/download/install.ps1 | iex
```

Another folder in your profile:

```powershell
$env:KANBRAIN_INSTALL_DIR = "$HOME\tools"; irm https://github.com/singudotdev/kanbrain/releases/latest/download/install.ps1 | iex
```

Windows on ARM uses the x64 build through Windows' built-in emulation.

### Supported platforms

| OS | CPU | command line | kanban app |
| --- | --- | --- | --- |
| Linux | x86_64, ARM64 | `kb-<cpu>-unknown-linux-musl.tar.gz` (any distro, statically linked) | `kanbrain-<cpu>-unknown-linux-gnu.tar.gz` (glibc 2.35+, Wayland or X11) |
| macOS 11+ | Intel and Apple Silicon (universal) | `kb-universal-apple-darwin.tar.gz` | `kanbrain-universal-apple-darwin.tar.gz` |
| Windows 10/11 | x64 (ARM64 via emulation) | `kb-x86_64-pc-windows-msvc.zip` | `kanbrain-x86_64-pc-windows-msvc.zip` |

`<cpu>` is `x86_64` or `aarch64`. You can also download these files by hand from the releases page, unpack them,
and put `kb` / `kanbrain` anywhere on your `PATH`.
To check that a file you downloaded was built by this repository's release workflow:
`gh attestation verify <file> -R singudotdev/kanbrain`.

The kanban app on Linux uses your desktop's graphics libraries: Vulkan (`libvulkan1` / `vulkan-icd-loader`), `libxkbcommon`,
and Wayland or X11; **+ Add project**'s folder dialog is the desktop's `xdg-desktop-portal`. Desktop installs normally have
them already. On Windows it opens no console window, so use
`kb` there for the command line (`list`, `path`).

For tab completion of commands, ids and projects, see [`kb completions`](cli.md#kb-completions).

### Updating

Run `kb update` (or `kanbrain update`), which runs the installer again for the apps you have, in the same
folder; or run the installer yourself, with the same `--app`. It replaces the binaries; your data isn't touched.
Then it brings the agent instructions of every project you ran `kb init` in up to the new version's
([`kb init --all`](cli.md#kb-init---all)), so you don't have to run `init` in each one.

The kanban app checks for a newer release when it opens and once a day, and offers it in the status bar:
**Update to X.Y.Z** (`U`) does the same as `kb update`, then opens the new version. On Windows it closes first,
since a running program can't be replaced there, and the installer opens it again. `KANBRAIN_NO_UPDATE_CHECK`
turns the check off ([environment variables](cli.md#environment-variables)).

Both use `curl`, which macOS and Windows have; on Linux, install it if `kb update` says it can't run it.

Neither updates while an agent has a task in doing, in any project, since the agents' instructions, and maybe their
data, would change mid-session: they say which tasks to wait for. A task an agent left in doing, you can park.

Installed with `cargo install kanbrain`, update the same way, then refresh the projects yourself:
`cargo install kanbrain && kb init --all`.

## Uninstalling

There are two levels:

| | removes the program and its launcher | removes the central brain's data |
| --- | :---: | :---: |
| **uninstall** | ✓ | |
| **purge** (full uninstall) | ✓ | ✓ |

**Linux / macOS**

Both levels remove both programs; add `--app cli` or `--app gui` (`$env:KANBRAIN_APP` on Windows) to remove just one.

Uninstall, the program only:

```sh
curl -fsSL https://github.com/singudotdev/kanbrain/releases/latest/download/install.sh | sh -s -- --uninstall
```

Purge, the program and the data (asks first):

```sh
curl -fsSL https://github.com/singudotdev/kanbrain/releases/latest/download/install.sh | sh -s -- --purge
```

Purge without the question, for scripts:

```sh
curl -fsSL https://github.com/singudotdev/kanbrain/releases/latest/download/install.sh | sh -s -- --purge --yes
```

**Windows**

Uninstall, the program only (it also takes it out of `PATH`):

```powershell
$env:KANBRAIN_UNINSTALL = "1"; irm https://github.com/singudotdev/kanbrain/releases/latest/download/install.ps1 | iex
```

Purge, the program and the data (asks first):

```powershell
$env:KANBRAIN_PURGE = "1"; irm https://github.com/singudotdev/kanbrain/releases/latest/download/install.ps1 | iex
```

Purge without the question, for scripts:

```powershell
$env:KANBRAIN_PURGE = "1"; $env:KANBRAIN_YES = "1"; irm https://github.com/singudotdev/kanbrain/releases/latest/download/install.ps1 | iex
```

These variables stay set in that PowerShell window, where installing again would uninstall or purge again: close
it, or clear them with `Remove-Item Env:KANBRAIN_UNINSTALL, Env:KANBRAIN_PURGE, Env:KANBRAIN_YES -ErrorAction
SilentlyContinue`. `kb update` and the app's update clear them, and `KANBRAIN_VERSION`, before installing.

What purge does:

- **It finds the central brain** by asking the installed command line (`kb --project central path`), so
  `KANBRAIN_DIR` is respected, wherever you run it from.
- **It shows what will be deleted and asks `[y/N]`.** Anything but `y` cancels, and nothing is removed.
  Without a terminal to ask on (e.g. in CI), it refuses unless you pass `--yes` / `KANBRAIN_YES`.
- **It deletes only what kanbrain created:** entry files (`NNNN-*.md`), `tags.md`, `last-id`, `format`, the project registry
  (`projects.md`) and cards (`projects/*.md`), exported reports (`reports/*.md`) and leftover temp files. If the
  folder has anything else in it (for example your `KANBRAIN_DIR` is inside a notes vault or a git repository), those
  files and the folder are **kept**. **Projects' own `.brain/` folders are kept**: they belong to their repositories, so delete them there if you want.
- **It also clears kanbrain's cache folder** (`~/.cache/kanbrain` on Linux, `~/Library/Caches/kanbrain` on macOS,
  `%LOCALAPPDATA%\kanbrain` on Windows): each brain's write lock, and the backups taken before migrating one, which
  are copies of its data. Only those two go; anything else there is kept.
- **It refuses** to work on your home folder or a drive root.

Purge can't be undone. To keep a copy first, on Linux or macOS:

```sh
tar -czf kanbrain-backup.tar.gz -C "$(kb --project central path)" .
```

On Windows (PowerShell):

```powershell
Compress-Archive "$(kb --project central path)\*" kanbrain-backup.zip
```

If you installed with `cargo install`, remove the programs with `cargo uninstall kanbrain` (the `kb` command line) / `cargo uninstall kanbrain-app`.
Your data then stays in the [data folder](#where-your-data-lives); delete it by hand if you want.

## Build from source

### Requirements

- **Rust 1.88 or newer** for the command line; the kanban app needs a recent stable Rust. Install it with [rustup](https://rustup.rs):
  - Linux / macOS: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
  - Arch Linux: `sudo pacman -S rustup && rustup default stable`
  - Windows: download `rustup-init.exe` from [rustup.rs](https://rustup.rs)
- For the kanban app on Linux, the development packages of its graphics libraries, e.g. on Debian/Ubuntu
  `sudo apt install libfontconfig-dev libwayland-dev libx11-xcb-dev libxkbcommon-x11-dev libvulkan1`
  (other distributions: see the [Zed Linux build docs](https://zed.dev/docs/development/linux), which use the same libraries).
  On macOS it needs Xcode (for the Metal shader compiler).

### Build and install

The command line (`kb`) from crates.io:

```sh
cargo install kanbrain
```

From a clone of the repository, the command line:

```sh
cargo install --path cli
```

The kanban app (`kanbrain`):

```sh
cargo install --path gui
```

This builds an optimized binary and puts it in `~/.cargo/bin/` (`%USERPROFILE%\.cargo\bin\` on Windows).
On Windows, rustup already put it on your `PATH`. On Linux and macOS, make sure it's there. In bash or zsh:

```sh
export PATH="$HOME/.cargo/bin:$PATH"
```

In fish:

```fish
fish_add_path ~/.cargo/bin
```

To build without installing, the command line (into `target/release/kb`):

```sh
cargo build --release
```

The kanban app (into `target/release/kanbrain`):

```sh
cargo build --release -p kanbrain-app
```

## First run

```sh
kanbrain
```

1. Press **+ New task** (or `n`), type a title and, in the second field, tags if you like (`feature, backend`), then `enter`.
   The card lands in **To do**; the form's **Column** picks another, and a column's `+` opens it with that column
   picked. To add several to To do in a row, press `a`: each `enter` adds one and keeps the field open.
2. When you start working on it, drag the card to **Doing**, or press `s` (`shift+→` moves it one column right).
   Its clock starts, and the top bar shows it. To switch to another, park this one first (`p`).
3. Double-click the card (or `enter`) to open its details: write a description (`m`), add comments as you go (`c`).
4. When it's finished, drag it to **Done** or press `d`.
5. At the end of the day, **Park & quit** (`Q`) stops your clock and closes the window. Everything is already saved.

Press `?` at any time for the command palette: what can be done now, with its key. Type to find one, `enter` runs it. Every
button's tooltip names its key, too.

From a terminal, or for an agent, the same steps are:

```sh
kb add --tag feature "Export CSV"    # → #1 Export CSV (todo)
kb start 1
kb comment 1 "the header row is done"
kb done 1
```

The board, if it's open, follows each command within a second. `kb --help` lists every command.

## Where your data lives

| platform | default folder |
| --- | --- |
| Linux | `~/.local/share/kanbrain` (or `$XDG_DATA_HOME/kanbrain`) |
| macOS | `~/Library/Application Support/kanbrain` |
| Windows | `%APPDATA%\kanbrain` (e.g. `C:\Users\you\AppData\Roaming\kanbrain`) |

`kb path` prints the folder in use.

That's the **central brain**, for everything outside a project. To keep it somewhere else (a notes vault, a git
repository, a synced folder), set `KANBRAIN_DIR`. In bash or zsh:

```sh
export KANBRAIN_DIR=~/notes/todo
```

In fish:

```fish
set -Ux KANBRAIN_DIR ~/notes/todo
```

On Windows, for your user, so the kanban app started from the Start Menu sees it too (open a new terminal
afterwards):

```powershell
[Environment]::SetEnvironmentVariable("KANBRAIN_DIR", "D:\notes\todo", "User")
```

Only in the current PowerShell session:

```powershell
$env:KANBRAIN_DIR = "D:\notes\todo"
```

The folder is created if it doesn't exist. See [Data format](data-format.md) for what goes inside.

### Projects

Run `kb init` in a repository to make it a project, or use **+ Add project** in the kanban app and pick its folder:
it gets its own data folder, `.brain/`, is registered in the central brain, and its coding agents are told about it
(see [For coding agents](agents.md)). From then on,
every command run in it (or a folder below it) uses its `.brain/`, like git finds `.git`; elsewhere they use the
central brain.

A project can span **several repositories**, e.g. a shop's `shop-api` and `shop-web`: run `kb init shop`
in each. Each keeps its own tasks in its own `.brain/`, and they share the project's card, the application's shared context.

Commit `.brain/` to keep a repository's tasks, notes and decisions next to its code (and available to coding agents
working in it), or add it to `.gitignore` to keep them local.

The kanban app shows the central brain and every project together, with a filter to show just one.

## Trying it without touching your real data

On Linux or macOS:

```sh
KANBRAIN_DIR=/tmp/kanbrain-test kanbrain
```

On Windows (PowerShell):

```powershell
$env:KANBRAIN_DIR = "$env:TEMP\kanbrain-test"; kanbrain
```

## Next steps

- [Tasks, tags & search](tasks-and-tags.md): the board, history, tags and comments
- [For coding agents](agents.md): give a project's agents the same memory
- [Keybindings](keybindings.md)
