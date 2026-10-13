# kanbrain documentation

kanbrain is project memory for you and your coding agents: tasks, notes and decisions as markdown files, kept
current by agents that stop to ask you when a choice is yours. It's two programs over the same files: a kanban
board (`kanbrain`) and a command line (`kb`).
Tasks move across **To do**, **Doing**, **Parked** and **Done**, carry tags, a description and comments,
remember when they moved, and track the time spent on them.

## Contents

1. [Getting started](getting-started.md): install, first run, where data lives
2. [Tasks, tags & search](tasks-and-tags.md): the board, statuses and history, tags, descriptions and comments, notes and decisions, search
3. [Time tracking](time-tracking.md): sessions, one task of yours at a time, and reports
4. [For coding agents](agents.md): `.brain/` per project, what goes where, the agent commands, JSON
5. [Keybindings](keybindings.md): every key and mouse action in the kanban app
6. [Command line](cli.md): `list`, `status`, `report`, `init`, `deinit`, `hook`, `path`, and the agent commands
7. [Data format](data-format.md): task files, `tags.md`, editing by hand
8. [Integrations](integrations.md): status bars, syncing between machines, notes apps
9. [FAQ & troubleshooting](faq.md)
10. [Development](development.md): architecture, building, tests, contributing

## Philosophy

- **The files are the truth.** Every change is written at once to a small markdown file. There's no database, nothing
  to save and nothing to sync but files; agents, scripts, your editor and the board all work on the same folder, and
  the board follows their changes by itself.
- **The board for people, the CLI for agents.** You see and move cards; agents run commands that print text or JSON.
  Neither has to know the other is there.
- **One thing at a time, for you.** Doing holds one task of yours per brain (each project repository, and the
  central one), and its clock runs while it's there; to switch, you park it, so the board always shows what you
  were doing before. Agents can run several, one per subagent.
- **Mouse or keyboard.** Every action has a button, and every button has a key, named in its tooltip; `?` opens a command palette that lists what can be done now, searchable, and runs one.
- **Zero config.** The only settings are optional environment variables.

## Concepts in 30 seconds

| concept | meaning |
| --- | --- |
| **task** | something to do. It has a title, optional tags, a status, a description, comments and a history |
| **note** | something to remember: a finding, how something works. Like a task without a status |
| **decision** | something decided, with its context and consequences (an ADR). Starts from a template |
| **link** | relates two entries (`r`, or the form's Related field); both show the other under "Related", and clicking it goes there |
| **status** | the task's column: `todo` (not started), `doing` (being worked on, with its clock running: one of yours at a time per brain, several of an agent's), `parked` (started, set aside for now), `done` |
| **history** | when the task's status changed, e.g. `todo → doing` on Mon 28 Sep 10:12 |
| **tag** | a label such as `prod` or `support`, shown as a colored chip |
| **brain** | a folder of entries: the central brain, for everything outside a project, or a project repository's `.brain/` |
| **project** | one or more repositories registered with `kb init NAME`, sharing a card with the application's context; each repository adds its role |
| **description** | free text about the task, like an issue's body, in markdown |
| **comment** | a timestamped note added over time (like a GitHub/GitLab issue comment); can be edited and deleted |
| **session** | one stretch of work on a task, from moving it to Doing to moving it out, e.g. 10:12–10:57 |

> **Using this as a GitHub/GitLab wiki:** the pages only link to each other with relative links, so you can copy
> `docs/*.md` into the project wiki as is. Rename `README.md` to `Home.md` for the wiki's start page.
