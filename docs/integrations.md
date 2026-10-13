# Integrations

## Status bars

`kb status` prints the running tasks and their clocks, or `idle`, then how many tasks wait for your answer, if any
(`· 2 waiting`). It reads the files directly, so it's fast and
works whether or not the board is open:

```sh
$ kb status
#2 Fix login 500 00:42:17
```

It looks in every brain: with tasks running in several projects, it lists each with its label,
`shop/shop-web #3 Checkout page 00:42:17 · blog #1 First post 00:05:10`.

### waybar

```jsonc
// ~/.config/waybar/config
"modules-right": ["custom/kanbrain", "clock"],
"custom/kanbrain": {
    "exec": "kb status",
    "interval": 5,
    "format": "▶ {}",
    "on-click": "kanbrain"
}
```

### tmux

```tmux
# ~/.tmux.conf
set -g status-right "#(kb status) | %H:%M"
```

Use it the same way in polybar, i3blocks, starship or SwiftBar. For a count instead, e.g. of tasks waiting in Parked:

```sh
kb list parked | grep -c '^- '
```

## Syncing between machines

Your data is a folder of small text files, so you can sync it any way you sync files. Point `KANBRAIN_DIR` at the synced folder on each machine.

### git

Once, on Linux or macOS (set `KANBRAIN_DIR` for good as in
[Where your data lives](getting-started.md#where-your-data-lives)):

```sh
export KANBRAIN_DIR=~/todo
mkdir -p ~/todo && cd ~/todo && git init
```

On Windows (PowerShell):

```powershell
[Environment]::SetEnvironmentVariable("KANBRAIN_DIR", "$HOME\todo", "User")
mkdir $HOME\todo; cd $HOME\todo; git init
```

Then, whenever you like, on Linux or macOS:

```sh
cd ~/todo && git add -A && git commit -m "todo $(date +%F)" && git push
```

On Windows:

```powershell
cd $HOME\todo; git add -A; git commit -m "todo $(Get-Date -Format yyyy-MM-dd)"; git push
```

Each task is its own file and writes are small, so merges rarely conflict.

The central brain's `projects.md` lists each project repository by its path on this machine, so keep it out of the
sync (e.g. add it to `.gitignore`) and run `kb init NAME` in each repository on each machine. The projects'
cards in `projects/` are fine to sync. A registered path that doesn't exist on a machine is skipped there.

### Syncthing, Dropbox, iCloud, OneDrive, Nextcloud

These work as is. If the sync tool creates conflict copies (`… (conflicted copy).md`), kanbrain loads them as tasks too.
Delete the one you don't want.

### Obsidian and other notes apps

Set `KANBRAIN_DIR` to a folder inside your vault, e.g. `~/vault/todo`. Task files render as regular notes,
and you can link to them. Their `## Description`, `## Comments` and `## History` sections read like normal notes.
