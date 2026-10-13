# FAQ & troubleshooting

### Can several tasks be in "doing" at the same time?

Yours, one per brain: the central brain and each project repository have their own Doing, so work in different
projects can run side by side, each with its clock. Within one, starting another task while yours runs is refused:
park or finish it first. Agents aren't limited, so one working with subagents can run several tasks next to yours;
they show in green, yours in cyan. If files edited by hand or synced from another machine leave several of your
tasks in one brain's doing, the most recently started stays and the others are parked when the files are read.

### Can I turn time tracking off?

No: it's always on, and costs nothing if you don't look at it. Moving cards is all it takes; the report is there
when you want to know where the day went.

### Where are my tasks?

Run `kb path`. The default is `~/.local/share/kanbrain` on Linux, `~/Library/Application Support/kanbrain` on macOS,
and `%APPDATA%\kanbrain` on Windows. `KANBRAIN_DIR` overrides it. Inside a project, its `.brain/` is used
instead; `kb path` prints the folder in use where you run it.

### The status bar says "skipped: …"

Some `.md` files in the data folder couldn't be read as tasks. The message names each file and the reason (usually a missing `- id:` line).
kanbrain doesn't modify those files. Fix them or move them out of the folder; the board reloads by itself.

It also names a repository written by a newer kanbrain (`… was written by a newer kanbrain`), which the board leaves
out: update with `kb update` (see [Format versions and migrations](data-format.md#format-versions-and-migrations)).

### An agent changed a task, but the board doesn't show it

The board looks at every brain's folder every second. If it still doesn't change, the agent is probably using a
folder the board doesn't show: run `kb path` where the agent works. A `.brain/` that `kb init`
never registered is only shown when the app is started inside it; run `init` there to register it. `R` reloads by
hand.

### My agent doesn't use kanbrain, or doesn't keep the board up to date

Run `kb init` in the project again. It updates the "Project memory" section in `CLAUDE.md` / `AGENTS.md`, which
says that kanbrain is the only place for the project's tasks, that "todos" or a plan mean kanbrain tasks
rather than the agent's built-in to-do list, and that the board is watched live. For Claude Code it also refreshes
the hooks that hand the agent the context and hold it to describing an undescribed project (see
[For coding agents](agents.md)). Sections written by older versions said less. Check that the agent works in the
repository itself (or a folder below it), where it reads that file.

### How do I remove a project?

`kb deinit` in the repository, or `kb --project NAME deinit` from anywhere: it's no longer
registered or shown on the board, and its agents' section is removed. Its `.brain/` folder is kept; delete it to drop
its tasks. See [Command line](cli.md#kb-deinit).

### **Open file** (`o`) doesn't open my editor

kanbrain uses `$VISUAL`, then `$EDITOR`, then `vi` (`notepad` on Windows). GUI editors must **wait** until the file is closed. VS Code:

```sh
export EDITOR="code -w"
```

Sublime Text:

```sh
export EDITOR="subl -w"
```

Zed:

```sh
export EDITOR="zed --wait"
```

Put the line in your shell's profile (`~/.bashrc`, `~/.zshrc`, or `set -Ux EDITOR …` in fish) to keep it. On
Windows, set it for your user with PowerShell, then restart the app:

```powershell
[Environment]::SetEnvironmentVariable("EDITOR", "code -w", "User")
```

### Can I fix the date of a status change?

Yes. Open the task with `o` and change the time in its `## History` line. See [Data format](data-format.md#fixing-mistakes).

### How do I rename a tag everywhere?

There's no button for it yet. Search for `#ops` to see every entry with the tag, and change each with `e` (**Edit**).
Renaming the tag's line in `tags.md` too keeps its color.

### Does it work on Windows?

Yes. Data goes to `%APPDATA%\kanbrain` and **Open file** opens Notepad unless `EDITOR` is set. The kanban app
opens no console window, so use `kb` from a terminal for the command line.

### Why does a card say `2d` when it's been almost 3 days?

Ages round down: minutes, then hours, days and weeks. The exact time is in the details' history.

### "kanbrain is a per-user app and doesn't run as root"

kanbrain is installed and used per user, so run it as your normal account, without `sudo`. Running it as root would either
use root's data folder instead of yours, or leave root-owned files in yours.
If root really is your only user (e.g. a Docker/Podman container), set `KANBRAIN_ALLOW_ROOT=1`.
The same applies to the installer. On Windows, run the installer from a normal (non-administrator) PowerShell.

### Can I install it for all users on a machine?

No, on purpose. Each user runs the installer themselves and gets their own copy and their own tasks.
It's about 1 MB, so the duplication doesn't matter.

### How do I uninstall, or remove all my data?

`--uninstall` removes only the program. `--purge` also deletes the central brain's data, after asking; projects'
`.brain/` folders are kept. It never deletes files that kanbrain didn't create.
See [Uninstalling](getting-started.md#uninstalling).

### How do I back up my data?

Copy the data folder, or [keep it in git](integrations.md#git). It's all plain text.
