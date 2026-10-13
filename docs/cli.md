# Command line

```
kb list [doing|todo|parked|done|all] [notes] [decisions] [--tag TAG] [--json]
                 print tasks as markdown, one section per status
kb init [NAME] [--repo REPO]
                 make this folder a project: its own .brain/, registered as NAME (default: the
                 folder's name) with a card for the application, and a section telling its agents
                 about it in CLAUDE.md / AGENTS.md (run again to update them)
kb init --all    update the agents' section and hooks in every registered project to this version's
kb deinit [--purge]
                 stop this folder (or --project NAME) being a project: unregister it, remove its
                 section from CLAUDE.md / AGENTS.md and, with its last repository, the project's card;
                 its .brain/ is kept unless --purge
kb hook prompt|stop
                 what the Claude Code hooks `init` adds run (see For coding agents)
kb status    print the running tasks, in whichever project, and how many wait for an answer
                 (for status bars)
kb report [hour|day|week|month] [--group task|tag] [--tag TAG] [--save]
                 print the current period's tracked time as markdown
kb add | show | search | start | park | done | todo | comment | ask | wait | propose | accept | reject
   | describe | link | project | context
                 entries by id, for agents and scripts (see For coding agents)
kb path      print the data directory in use (a project's .brain/ inside it)
kb update    install the latest release over this one
kb completions bash|zsh|fish
                 print a tab completion script for the shell
kb --version print the version (also -V)
kb --help    show usage (also with no arguments)

--project NAME         anywhere on the line: run the command on a registered project's brain,
                       or `central`, from any folder
```

All commands read and write the same folder as the kanban app (see
[where your data lives](getting-started.md#where-your-data-lives)), so you can run them while it's open: the board
follows within a second.

## Which brain

A command works on one brain, the first of:

1. the project given with `--project NAME` (`central` for the central brain). A project with several repositories
   names one as `PROJECT/REPO`, e.g. `shop/shop-web`;
2. the nearest `.brain/` in the current folder or above it, like git finds `.git`;
3. the central brain: `$KANBRAIN_DIR`, else the platform data folder (see
   [where your data lives](getting-started.md#where-your-data-lives)).

```sh
kb --project shop/shop-api list doing   # the API's tasks in doing, from anywhere
kb add --project central "Renew passport"
```

## `kb init`

Makes the current folder a repository of project `NAME` (default: the folder's name):

- creates `.brain/`, the repository's own data folder, and `.brain/project.md`, which names its project and, with
  several repositories, holds this one's role among them (`project notes`);
- registers it in the central brain's `projects.md`. Another repository already in project `NAME` makes it a
  multi-repository project: each keeps its own tasks, labelled `NAME/REPO` (`shop/shop-web`);
- creates the project's card in the central brain, `projects/NAME.md`, if it's new: the application's shared context
  (what it is, its services and how they connect, conventions they share), written with `project describe`.
  `context` starts with it, then this repository's role and where the other repositories are, so an agent knows the
  project without exploring it;
- writes the "Project memory" section into `CLAUDE.md` / `AGENTS.md` and, with `CLAUDE.md`, Claude Code hooks into
  `.claude/settings.local.json`: the context at the start of each session, and `kb hook prompt|stop`, which hold an
  agent to describing an undescribed project first and to commenting on the tasks it started (see
  [For coding agents](agents.md)).

Running it again is safe: it updates the agent section and the hooks, and `init NAME` moves the repository to project
`NAME`.

Within a project, each repository has a name of its own: its folder's, unless that's taken by another of the project's
repositories (two folders named `app` in different places), when the folder above goes before it (`backend-app`), or
a number. `--repo REPO` names it yourself. A name other than the folder's is kept in its `.brain/project.md`, so links
to it (`REPO#3`) read the same on every machine. `kb project repo NAME` renames it later.

### Managing a project

Run inside one of the project's repositories (or with `--project`):

| command | does |
| --- | --- |
| `kb project rename NAME…` | renames the project: the registry, its card and each repository's `.brain/project.md` |
| `kb project repo NAME` | renames this repository in its project, and rewrites the other repositories' links to it |
| `kb project move PROJECT…` | moves this repository to another project, created if new; its links to the old project's repositories are dropped, on both sides |
| `kb project remove [--purge]` | takes every repository of the project out, as `deinit` does each; `--purge` deletes their `.brain/` too |

The kanban app does the same from **Project info** (`I`).

### `kb init --all`

Brings every registered repository's agent instructions up to this version's: the section in each agent file that
already has it, and the Claude Code hooks where `CLAUDE.md` has it. A repository whose section you took out by hand is
left alone, and so is one whose folder is gone. It prints what it changed, one line per repository and file.

The installer runs it after installing, and so does `kb update`, so upgrading updates every project's agents. Run it
yourself after installing another way, such as building from source.

## `kb deinit`

Undoes `init` for the repository of the current folder, or for the one `--project NAME` names from anywhere
(`--project shop/shop-web deinit`):

- takes it out of the central brain's `projects.md`, so the kanban app no longer shows it and `--project` no longer
  reaches it;
- deletes the project's card, `projects/NAME.md`, if this was the project's last repository. Other repositories of
  the project keep it;
- removes the "Project memory" section from `CLAUDE.md` / `AGENTS.md`, and deletes a file left empty, which is
  the one `init` created; the same for the hooks in `.claude/settings.local.json`.

It also drops the links the project's other repositories have to it, since it's no longer one of them.

**`.brain/` is kept**, with its tasks, notes and decisions: commands run in the folder still use it. `--purge` deletes
it too; otherwise run `init` again to register it again. A repository whose folder is already gone can still be taken
out with `--project`.

## `kb list`

Prints tasks as a markdown list, one `## status` section per status, most recently changed first. Sections without
tasks are left out.

```
$ kb list
## doing

- #2 Fix login 500 #prod #support

## todo

- #4 Review Ana's PR #support
- #1 Feature: export CSV #feature #backend
```

| option | meaning | default |
| --- | --- | --- |
| `doing` / `todo` / `parked` / `done` | only these statuses; give several to combine them | all but `done` |
| `all` | every status, done included | |
| `notes` / `decisions` | also list notes and/or decisions after the tasks, newest first; given alone, only those | |
| `--json` | a JSON array of `{ id, kind, title, status, tags }` in the same order | markdown |
| `--tag TAG` | only tasks with this tag (`prod` or `#prod`) | all |

Examples:

```sh
kb list                     # everything not done yet
kb list done --tag client-a # what's finished for one client
kb list all | wl-copy       # copy to the clipboard (Wayland); xclip, pbcopy or clip elsewhere
kb list doing | grep -c '^- '   # how many tasks are in progress
```

## `kb status`

Prints one line:

```
$ kb status
#2 Fix login 500 00:42:17
$ kb status          # nothing running
idle
```

It looks in every brain, wherever it's run from. A project's task starts with its label, and when several run
(agents can run several per brain) they're joined on the line: `shop/shop-web #3 Checkout page 00:42:17 · blog #1 First post 00:05:10`.
Tasks an agent asked you about, in any brain, are counted at the end: `idle · 2 waiting`.

The time is the length of the **current session**, not the task's total. See [Integrations](integrations.md) for status bar setups.

## `kb report`

Prints a markdown report for the **current** period to stdout. This is the same content as **Export .md** on the report tab.

| option | meaning | default |
| --- | --- | --- |
| `hour` / `day` / `week` / `month` | period, the one containing now | `week` |
| `--group task` | one row per task, with its tags | ✓ |
| `--group tag` | one row per tag | |
| `--tag TAG` | only tasks with this tag (`prod` or `#prod`) | all |
| `--save` | also write the file to `<data folder>/reports/` and print its path to stderr | off |

Examples:

```sh
kb report                              # this week, by task
kb report day                          # today, with timeline
kb report month --group tag            # this month per tag
kb report week --tag client-a --save   # timesheet for one client, saved
```

To copy it to the clipboard, on Linux with Wayland:

```sh
kb report | wl-copy
```

On Linux with X11:

```sh
kb report | xclip -selection clipboard
```

On macOS:

```sh
kb report | pbcopy
```

On Windows:

```sh
kb report | clip
```

Save a report every Friday evening, on Linux or macOS with cron (`crontab -e`):

```cron
0 18 * * 5  kb report week --save >/dev/null 2>&1
```

On Windows, with the Task Scheduler (PowerShell):

```powershell
Register-ScheduledTask -TaskName "kanbrain report" -Trigger (New-ScheduledTaskTrigger -Weekly -DaysOfWeek Friday -At 18:00) -Action (New-ScheduledTaskAction -Execute "kb" -Argument "report week --save")
```

## `kb path`

Prints the folder of the brain in use: the project's `.brain/` inside a project, else the central brain, e.g.
`/home/you/.local/share/kanbrain`. `kb --project central path` always names the central brain.
Handy for scripts: `cd "$(kb path)"` (in PowerShell, `cd (kb path)`).

## `kb update`

Runs the installer for the latest release, into the folder `kb` is in. It checks the download's checksum and
replaces `kb`, and `kanbrain` too when it's installed beside it, then updates every project (`kb init --all`):
its agent instructions, and its data, migrated to the new version's format when it changed, after a backup (see
[Format versions and migrations](data-format.md#format-versions-and-migrations)). On Windows, where a
running program can't be replaced, it prints the PowerShell command to run once the app is closed. Installed with
`cargo install`, it doesn't replace anything behind cargo's back: it prints `cargo install kanbrain && kb init --all`.

## `kb completions`

Prints a script that completes `kb` in your shell: the commands and their options, entries' ids (open tasks
first; fish also shows each title), project names after `--project` and tags after `--tag`. Ids and tags come from
the brain the command line names, so `kb --project blog done <tab>` offers blog's. Load it from your shell's
startup:

| shell | add |
| --- | --- |
| bash, in `~/.bashrc` | `source <(kb completions bash)` |
| zsh, in `~/.zshrc` | `source <(kb completions zsh)` |
| fish | `kb completions fish > ~/.config/fish/completions/kb.fish` |

`kanbrain completions SHELL` prints the same for `kanbrain`, which takes the same commands.

## Environment variables

| variable | purpose |
| --- | --- |
| `KANBRAIN_DIR` | the central brain's folder, instead of the platform default; a project's `.brain/` still takes precedence inside it |
| `KANBRAIN_WAIT_TIMEOUT` | minutes `kb wait` waits for an answer without `--timeout` (default 8; `0`: no limit) |
| `KANBRAIN_AUTHOR` | `human` or `agent`: who the comments and status changes `kb` makes are recorded as. Without it, `kb` writes `agent` when a coding agent's own variable is set (`AGENT`, `AI_AGENT`, `CLAUDECODE`, `CODEX_SANDBOX`, `CODEX_CI`, `CODEX_THREAD_ID`, `GEMINI_CLI`), else `human` |
| `KANBRAIN_ALLOW_ROOT` | set to `1` to allow running as root. kanbrain is per-user and refuses root by default; only use this where root is the only user, e.g. containers |
| `VISUAL`, `EDITOR` | editor for **Open file** (`o`) in the kanban app, checked in that order. It may include arguments, e.g. `code -w`. The fallback is `vi`, or `notepad` on Windows |

## Exit codes

`0` on success. Non-zero if an error occurred (for example an unreadable data folder or an unknown option), with a
message on stderr.
