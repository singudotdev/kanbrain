# kanbrain: the `kb` command line

**Project memory for you and your coding agents.** `kb` keeps tasks, notes and decisions as plain markdown files next
to your code, tracks the time spent on each task by itself, and gives coding agents (Claude Code, Codex, ...) a
small, cheap command line to read and update them. Agents keep the board current and stop to ask you when a choice
is yours; you answer on the board. There's no database: you can read the files, grep them and commit them with git.

This crate installs `kb`. The kanban board that shows the same files live, `kanbrain`, isn't on crates.io (it's built
on GPUI from Zed's repository): get it from the
[releases](https://github.com/singudotdev/kanbrain/releases/latest) or with the
[installers](https://github.com/singudotdev/kanbrain#1-install).

## Install

```sh
cargo install kanbrain
```

It needs Rust 1.88 or newer. Update it the same way, then refresh your projects' agent instructions with
`kb init --all`.

## Connect a project to its agents

```sh
cd my-project
kb init
```

This creates `.brain/`, the project's data folder (commit it to share the memory, or ignore it to keep it local),
registers the project, and writes a short section into `CLAUDE.md` / `AGENTS.md` telling the agents how to use `kb`.
For Claude Code it also adds hooks that hand the agent `kb context` at the start of each session. `kb deinit` undoes
it and keeps the data.

## How an agent uses it

```sh
kb context                         # start of session: project card, active tasks, decisions, notes
kb search login                    # find the task (or `kb add "Fix login 500"`)
kb start 4                         # clock starts; the previously running task is parked
kb comment 4 "token refresh race; fix next"
echo "Context…/Decision…/Consequences…" | kb add --decision --link 4 --body - "Retry refresh once"
kb done 4
```

The next session's `context` picks up where this one stopped. Output is kept small so it doesn't waste the agent's
context: write commands reply with one line, like `#4 Fix login 500 (doing) #auth`.

## Commands

| command | what it does |
| --- | --- |
| `kb context` | summary for agents: project card, active tasks, recent done, decisions, notes |
| `kb add [--note\|--decision] [--tag T] [--link ID] [--body TEXT\|-] [--start] TITLE` | create a task (or note/decision) |
| `kb start\|park\|done\|todo ID` | move a task; `start` runs its clock and parks the running one |
| `kb comment ID TEXT` | add a timestamped comment (`-` reads from stdin) |
| `kb describe ID TEXT` | replace the description |
| `kb link ID OTHER…` | link entries to each other (`REPO#ID`: in another repository of the project) |
| `kb project [describe\|notes TEXT\|-]` | show or rewrite the project's description and this repository's role |
| `kb show ID` | one entry in full: status, time, description, comments, links, history |
| `kb search WORDS…` | find entries by words; `#tag` matches tags only |
| `kb list [doing\|todo\|parked\|done\|all] [notes] [decisions] [--tag T]` | print tasks as markdown |
| `kb status` | the running task and its clock, for status bars |
| `kb report [hour\|day\|week\|month] [--group task\|tag] [--save]` | tracked time as markdown |
| `kb completions bash\|zsh\|fish` | tab completion of commands, ids, projects and tags |

`add`, `show`, `search`, `list`, the moves and `comment`/`describe`/`link` accept `--json`. `--project NAME` runs a
command on another registered project from any folder. Outside any project, entries go to the central folder
(`~/.local/share/kanbrain` on Linux, or `$KANBRAIN_DIR`).

## Documentation

[For coding agents](https://github.com/singudotdev/kanbrain/blob/main/docs/agents.md) ·
[Command line](https://github.com/singudotdev/kanbrain/blob/main/docs/cli.md) ·
[Tasks, tags & search](https://github.com/singudotdev/kanbrain/blob/main/docs/tasks-and-tags.md) ·
[Time tracking](https://github.com/singudotdev/kanbrain/blob/main/docs/time-tracking.md) ·
[Data format](https://github.com/singudotdev/kanbrain/blob/main/docs/data-format.md) ·
[FAQ](https://github.com/singudotdev/kanbrain/blob/main/docs/faq.md)

## License

[AGPL-3.0-only](https://github.com/singudotdev/kanbrain/blob/main/LICENSE), with the additional terms in
[NOTICE](https://github.com/singudotdev/kanbrain/blob/main/NOTICE): anything built on kanbrain that you distribute,
or that others use over a network, must be shared under the same license with its source, keep the attribution
"Based on kanbrain by singudotdev", and use another name and logo.
