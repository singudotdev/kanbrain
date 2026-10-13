# Changelog

All notable changes to this project are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.3.5] - 2026-10-13

### Fixed

- Deleting a repository's brain with `--purge` (`kb deinit --purge`, `kb project remove --purge`, the app's Remove…)
  also deletes what the cache folder kept about it: its write lock file and its migration backups.
- A brain in the system's temporary folder keeps its write lock and backups there (`<tmp>/kanbrain-cache/`), not in
  the user's cache folder, so tests and throwaway brains leave nothing behind.
- Uninstalling with `--purge` (`KANBRAIN_PURGE` on Windows) also clears kanbrain's cache folder: the write locks and
  the backups taken before migrations, which hold copies of the data. Only those two go.

## [0.3.4] - 2026-10-13

### Added

- Deciding on an agent's proposal from the task that waits on it: **Accept** (`y`), **Accept with a comment…** (`Y`,
  new, also on the decision) or **Reject…** (`j`, which always asks why), in its details, the palette and the
  right-click menu; such a task no longer offers a plain **Answer**.
- Keys in the kanban app for what only the mouse did: `E` edits the description of the project shown (the bar's
  **Edit**), `D` opens the decision an agent proposed that the selected task waits on. On a task waiting for your
  answer, the palette and the right-click menu offer **Answer** (`c`) instead of a comment.

### Changed

- The kanban app's **Project info** button is now **Manage project**, drawn as a button: renaming, moving and
  removing projects and repositories is there, with the project's description and time (`I`, as before).

### Fixed

- On the calendar, the buttons of an entry's details (**Edit**, **Link…**, **Add**, **Accept**, **Delete…**…) act on
  the entry: they pressed keys the calendar takes for itself, so the description's **Edit** showed a month instead.
- Closing the kanban app's answer editor without writing anything no longer looks like answering: until something's
  typed its button is **Close**, then **Answer** (**Add comment**, **Reject**, **Save** elsewhere), and the status bar
  says "nothing answered: the question still waits". A blank comment, answer or reason for rejecting isn't saved.
- The command palette (`?`) offers only what works now, in sections: the selected entry's actions (no Start for a
  started task, Accept and Reject only on a decision, Move to project only with another one), the tab's, then
  every tab's (Manage project only with a project shown, Update only with a newer release); `esc` is there when it
  closes the details or clears the search, and the notes and decisions tabs say their arrows move up and down.
- A task's details name their buttons as the right-click menu does (**Edit title and tags**, **Move to
  project…**, **Delete…**, **Answer…**), from the same table, so the two can't drift apart.
- Closing a task's details in the kanban app with ✕ no longer opens a new task in Done: closing them widens the
  board and brought the last column's **+** under the pointer. Each column's **+** now sits beside its count.

## [0.3.3] - 2026-10-13

### Removed

- The kanban app's "What's next?" dialog: stopping your running task (Park, Done, To do, deleting it) asks nothing;
  the board shows what's parked and to do.

### Changed

- The agent section `kb init` writes in `CLAUDE.md` / `AGENTS.md` has its two long lines wrapped like the rest; the
  words are the same. Updating rewrites it in every project.

## [0.3.2] - 2026-10-13

### Added

- Data format versions and migrations: each brain records its format in a `format` file. A version that changes it
  migrates older brains when they're opened to be changed and on `kb update`, after copying them to the cache
  folder; a brain written by a newer version is refused, read or written, rather than misread.
- Updating from the kanban app: it checks for a newer release when it opens and daily (`KANBRAIN_NO_UPDATE_CHECK=1`
  turns that off) and offers it in the status bar; **Update** (`U`) installs it like `kb update` and opens it.

### Changed

- `kb update`, and the app's, wait until no agent has a task in doing, so their instructions and data don't change
  mid-session.

## [0.3.1] - 2026-10-13

### Fixed

- Moving a repository to another project (Project info's **Move…**, `kb project move`, `kb init NAME`) no longer
  leaves it listed in its old project too on macOS and Windows.

## [0.3.0] - 2026-10-13

### Added

- Managing projects: rename a project or one of its repositories, move a repository to another project, and remove a
  repository or a whole project, keeping its tasks or deleting them. In the kanban app from **Project info** (`I`),
  which also adds a repository to the project; from the command line with `kb project rename|repo|move|remove` and
  `kb deinit --purge`.

### Changed

- `kb deinit` also drops the links the project's other repositories have to the repository it takes out.

### Fixed

- Adding two folders with the same name to one project (from the app or `kb init`) no longer fails: the second gets
  a name of its own (`backend-app`), kept in its `.brain/project.md`; `kb init --repo REPO` names it yourself.

## [0.2.0] - 2026-10-13

### Changed

- **Breaking:** a person runs one task at a time per brain, and starting another while one runs is refused instead
  of parking it. Agents aren't limited: one splitting its work between subagents runs a task per subagent, and the
  agents' section says how. Running tasks show in cyan when a person started them, in green when an agent did.
- Report totals, tag rows, the calendar's days and the app's Today count by the clock: time several tasks ran at
  once counts once. Task rows keep each task's own time.
- **Park & quit** parks the tasks you started; agents' keep running. "What's next?" asks once yours stops, whatever
  agents run there.
- The `Stop` hook reminds an agent of each task it started without commenting since, not only the first.

### Fixed

- `kb` commands run at the same time, as an agent's parallel calls do, no longer overwrite each other's changes or
  give two entries the same id: each takes the brain's write lock. The kanban app takes it too while it acts on
  input, so a change made on the board and one an agent makes at that moment don't overwrite each other.

## [0.1.0] - 2026-10-11

### Added

- `kb`, the command line for agents and scripts: `init` connects a repository to its agents (`.brain/`, the project
  registry, the agent instructions and Claude Code hooks), and commands add, move, comment on, link, search, list and
  report on tasks, notes and decisions, with `--json` output.
- `kanbrain`, the kanban app: board or list, notes, decisions, report and calendar tabs, project and tag filters,
  undo, a command palette, and adding a project from a folder. It shows other programs' changes within a second.
- Agents ask instead of guessing: `kb ask` parks a task with a question and `kb propose` proposes a decision; the
  board shows them as waiting for you until you answer or accept or reject. `kb wait`, run in the background,
  wakes the agent with your answer; otherwise its next `context` lists the answered ones first. Comments and moves
  record whether a person or an agent made them.
- Projects of several repositories sharing one card, with entries linked across them.
- Time tracking with one running task per brain, and reports by task or tag.
- Installers for Linux, macOS and Windows, with checksums, the kanban app's launcher and icon, and `kb update`.
- Release archives with SHA-256 checksums and signed build provenance, and the licenses of everything compiled into
  each program (`THIRD-PARTY-LICENSES.txt`).
- The command line on crates.io: `cargo install kanbrain` installs `kb`.
- Licensed AGPL-3.0-only with additional terms in `NOTICE`: attribution, no presenting kanbrain as one's own work,
  modified versions marked as such, and no rights to the kanbrain name or logo.

[Unreleased]: https://github.com/singudotdev/kanbrain/compare/v0.3.5...HEAD
[0.3.5]: https://github.com/singudotdev/kanbrain/compare/v0.3.4...v0.3.5
[0.3.4]: https://github.com/singudotdev/kanbrain/compare/v0.3.3...v0.3.4
[0.3.3]: https://github.com/singudotdev/kanbrain/compare/v0.3.2...v0.3.3
[0.3.2]: https://github.com/singudotdev/kanbrain/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/singudotdev/kanbrain/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/singudotdev/kanbrain/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/singudotdev/kanbrain/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/singudotdev/kanbrain/releases/tag/v0.1.0
