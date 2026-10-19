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
For Claude Code it also adds hooks that hand the agent `kb context` at the start of each session, remind it to
describe an undescribed project and to comment on the task it started, tell it of your comments on a task it's
working on with its next step, and send it to `kb` when it tries to write the brain's files itself. `kb deinit`
undoes it and keeps the data.

## How an agent uses it

```sh
kb context                         # start of session: project card, active tasks, decisions, notes
kb search login                    # find the task (or `kb add "Fix login 500"`)
kb start 4                         # clock starts (an agent can run several at once)
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
| `kb start\|park\|done\|todo ID` | move a task; `start` runs its clock (a person runs one task at a time; agents, several) |
| `kb comment ID TEXT` | add a timestamped comment (`-` reads from stdin) |
| `kb ask ID TEXT` | ask the user a question on a task, which waits for the answer in Parked |
| `kb wait ID` | block until a person answers the task's question (or decides on a decision), then print the answer; at most 8 minutes by default |
| `kb propose ID [--body TEXT\|-] TITLE` | propose a decision for a task, for the user to accept or reject |
| `kb accept ID [TEXT]` / `kb reject ID TEXT` | accept or reject a decision, which answers the tasks waiting on it |
| `kb describe ID TEXT` | replace the description |
| `kb rename ID TITLE` | replace the title |
| `kb tag ID +TAG\|-TAG…` | add tags to an entry, or remove them |
| `kb tag OLD NEW` | rename a tag in every entry that has it |
| `kb link ID OTHER…` | link entries to each other (`REPO#ID`: in another repository of the project) |
| `kb project [describe\|notes TEXT\|-]` | show or rewrite the project's description and this repository's role |
| `kb project rename\|repo\|move\|remove` | rename the project or this repository, move it to another project, or remove the project |
| `kb show ID` | one entry in full: status, time, description, comments, links, history |
| `kb search WORDS…` | find entries by words; `#tag` matches tags only |
| `kb list [doing\|todo\|parked\|done\|all] [notes] [decisions] [--tag T] [--json]` | print tasks as markdown |
| `kb status` | the running tasks and their clocks, for status bars |
| `kb report [hour\|day\|week\|month] [--group task\|tag] [--tag T] [--save]` | tracked time as markdown |
| `kb update` | install the latest release over this one, then bring every project up to it (`kb init --all`) |
| `kb completions bash\|zsh\|fish\|powershell` | tab completion of commands, ids, projects and tags |

The entry commands and `list` accept `--json` (all but `wait`, `context` and `project`). `--project NAME` runs a
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
