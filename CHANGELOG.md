# Changelog

All notable changes to this project are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.4.1] - 2026-10-14

### Added

- The kanban app's form picks tags from a list: in the tags field, the tags in use by entries of its kind drop down
  with how many use each (`Board::form_tags`), those it holds left out, filtered by the tag being typed
  (`Form::typed_tag`) as the dropdowns filter, the first starting with it highlighted. A click, or `↓` `↑` and
  `enter`, puts one in its place (`Form::put_tag`); with none highlighted, `enter` saves; a line says when none
  matches: the tag will be a new one. It replaces the chips of the tags in use under the field.

### Changed

- The kanban app's buttons and menu entries drop the trailing ellipsis: **Rename**, **Move**, **Remove**, **Remove
  project**, **Link**, **Answer**, **Accept with a comment**, **Reject**, **Move to project**, **Delete**, **Edit or
  delete a comment**.
- The kanban app's dropdowns, pickers and right-click menu draw a line between rows, as the list view does
  (`dropdown::list`, `menu::draw`), and a dialog's list sits in a bordered box (`dialogs::list_box`): the link
  picker, comments, which project and the command palette now too.


### Fixed

- `kb init --all` (run by the installer and `kb update`) renames a registry's only project named `Central`, or one
  with `:` or `/`, instead of panicking (`rename_unreachable_projects`).
- An entry file that doesn't parse (a sync conflict's, a hand edit's) is no longer lost: saving another entry over
  its name is refused (`Store::save`), and a new entry never takes its id (`Brain::take_id` counts the files there).
- The kanban app: a ctrl+letter does nothing outside the editor, where `ctrl+d` discarded the edit and a second one
  moved the selected task to Done (`Board::key`); a dialog on an entry another program deleted closes, saying so,
  instead of taking the keys unseen (`Mode::entry`, `after_load`); a dragged card is dropped on its own brain, found
  by folder, though a reload reordered the brains meanwhile (`Board::drop_dragged`).
- `kb init`'s message and `docs/agents.md` say the `Stop` hook reminds an agent to describe the project, as it does,
  rather than holding the turn.
## [0.4.0] - 2026-10-14

### Added

- A key for every button of the kanban app: `N` edits the repository's role, `C` lists the entry's comments to edit
  or delete one, `ctrl+d` discards the editor's changes, `shift+tab` goes back a tab or a form field, and Manage
  project's buttons have keys, on the repository `↑` `↓` highlight.
- `kb show --json` lists the other repositories' entries linking to it, in `repo_linked_from`.
- `--` ends a text's options: every word after it is the text (`kb comment 3 -- use --json`).

### Changed

- The kanban app embeds its fonts, IBM Plex Sans and Lilex: bold and italic now show on Linux, and the editor and
  code are monospace on every platform.
- A tag shows in one color all over the app, though several brains color it.
- Time under a minute shows as `<1m`, not `0m`, which now means none.
- `kb list` marks a task waiting for an answer and a decision proposed or rejected, as `kb context` does.
- A command-line error is followed by that command's usage only, not every command's; `kb COMMAND --help` and
  `kb help COMMAND` print it.
- `--project` takes a name in any case when only one project or repository has it, and `central` in any case; a
  project of several repositories alone is told to name one, not that it doesn't exist.
- `kanbrain-core`, for code built on it: `model::Comment` has an `answer` field and `cli::ProjectChange::Rename` a
  second one, `true` for a name given after `--`. Build a `Comment` with `Comment::new`, then `..` for the rest.
- `kb` refuses what 0.3.5 took, saying what to type instead: `kb project rename OLD NEW` (now
  `kb project rename -- OLD NEW`), a project or repository name starting with `-`, a project called `Central` in any
  case, several tags after one `kb list --tag`, a word `kb link` can't read as a link, `kb search` without words,
  `kb reject` without why, an empty stdin for `-`, and `kb init` where the path has a line break or in the central
  brain. `kb init --all`, which `kb update` runs, renames the projects and repositories already registered under such
  names; until then, `kb init` in one of their repositories says to run it.

### Fixed

- Several `kb init`s at once all keep their repository in the list of projects; before, one could drop another's.
- `kb project` no longer can freeze with the kanban app open, each waiting for the other's write lock.
- The kanban app no longer misses a change an agent makes while it reloads, including in a repository just added.
- A command reading its text from stdin (`kb comment 3 -`) no longer holds up other `kb` commands and the app while
  it waits for the input.
- `kb wait` and `kb report` take the brain's write lock while opening it, since opening may fix files.
- Every write is flushed to disk under a temporary name of its own before taking the file's place; one a crash
  leaves behind is deleted later, and uninstalling with `--purge` deletes them too.
- A sync tool's conflict copy of an entry gets the next free id, instead of sharing the original's: commands meant
  for one reached the other, and saving the copy could overwrite the original.
- `kb update` and `--project` no longer recreate the `.brain/` folder of a repository deleted by hand:
  `--project` says to take it out with `kb --project NAME deinit`.
- `kb status` still shows the other brains when one can't be read, and a `format` file saying `0` is an error
  rather than a crash.
- Brains in the temporary folder, as tests make, leave nothing in its cache once they're gone.
- The kanban app:
  - the board scrolls sideways to the selected card's column instead of hiding the Done column under the details,
    also when the details open on it;
  - a card's question wraps within the card instead of being cut at its edge, and a long title or question is cut at
    a word's end;
  - Manage project's editors and adding a repository from it ("Add a repository to PROJECT") go back to it, on the
    repository it was on, and it keeps the highlighted repository, its top and its keys in view;
  - the status bar shows messages without backticks; an editor whose text is back to what it was no longer reads
    as changed, nor says it was saved; answering an agent says the answer was sent;
  - a note or decision of headings alone previews its first heading, and the delete dialogs say what will be removed;
  - a calendar day of several sessions spans from the first, beside their total;
  - a dialog left open follows its project when another is added or removed;
  - a button's tooltip names its key only where the key does the same;
  - the `#tag` filter matches as `kb search` does;
  - an empty `$VISUAL` falls back to `$EDITOR`;
  - running tasks keep their place in the top bar, which wraps alike on every tab;
  - the notes and decisions lists line up across brains, preview without markdown and say `1 note`;
  - an empty column says when the search hides its tasks, and saving a form without a title says so under it;
  - a move's message names the repository when several are shown, and answering an agent says the answer was sent;
  - Manage project stays open after taking one of several repositories out, and its remove dialog speaks of what it
    removes;
  - the board's list view lines its titles up across brains;
  - a shown project taken out from the command line falls back to All, not a blank scope;
  - a comment's Edit and Delete stay inside the details, the New button keeps one width so the top bar wraps alike,
    the project's line in the bar is plain text, and the link picker's highlight follows the entry it links;
  - moves, deletes and verdicts name the entry's repository when several are shown, `u` says when it redoes, and
    Unlink and delete tasks says what it deleted;
  - a renamed project stays shown, a new task that can't start says why, saving the title and tags says so, `C`
    without comments says how to add one, long lists say what's hidden above, and dialogs name the repository;
  - the calendar's month names each entry's project when several are shown;
  - quiet buttons have an outline, so they no longer look like plain text;
  - a repository renamed with `kb project repo` and a shown project renamed from the command line show under their
    new names; saving the form unchanged no longer rewrites the entry nor says it was saved;
  - long titles in the calendar's week end with an ellipsis instead of being cut mid-letter, and its moves stay in
    their column; a running task's row in the list view lines up with the others; the move dialog, the edit
    form and the comments list name the repository;
  - it redraws only when something on it changes with time, not twice a second while idle.
- `kb update` and the app's update run the latest release's installer, not the one on `main`.
- New tags get the same color whatever build of kanbrain picks it.
- `kb list --json` prints on one line, like every other command's JSON; `kb list` with nothing to show says so, gives
  a conflict copy its own id before listing it, and `kb search` asks for words rather than listing everything.
- Shell completions offer every `kb project` subcommand, `deinit --purge`, `init --repo` and `wait --timeout`, only
  the project names `--project` takes, and open tasks' ids first.
- Errors name their command once (`reject needs why`, `propose needs the decision's title`) and a bad id without the
  parser's words, `kb wait` on a note gives no status, counts say `1 session` and `1 entry`, ages say `<1m`, `kb
  update` names agents' tasks as `kb status` does, `kb init --all` says "more" only after others it listed, and moving
  or removing a repository says what happens to its notes and tasks.
- `kb --help` names every command that takes `--json`.
- A status move or a verdict that changes nothing says so on stderr.
- Moving a repository says so first, by the name it had, keeps a name of its own (or says why it can't), and when it
  was its project's last, takes the project's description to the new one if that has none (or says it's dropped),
  and removes the old card.
- `kb project remove` no longer says the card stays before removing it; a merged report names tasks `api #3`.
- `install.sh` takes `KANBRAIN_UNINSTALL`, `KANBRAIN_PURGE` and `KANBRAIN_YES` as `install.ps1` does, and cancelling
  a purge is no longer an error; `install.ps1` installs the second app at the version asked for.
- Data that could be lost no longer is:
  - on macOS and Windows, saving an entry whose file was renamed only in case deleted it;
  - a file an editor saved with a byte-order mark lost its title;
  - links to an entry whose file didn't parse, or to one a repository's older branch doesn't have, were dropped;
  - a title with line breaks made its entry unreadable; such files read again;
  - projects whose names slug alike (`my_app` and `my-app`, `Shop` and `shop`, any two in another script) shared
    one card, each overwriting the other's description;
  - `--purge` deleted files of your own named like kanbrain's (`2024-01-15.md` in a vault), followed a linked data
    folder only part way, and took `$HOME//` for a data folder.
- A `tags.md` color of other characters no longer crashes every command; the largest id no longer wraps to 0.
- An agent's `accept` or `reject` answers the task that asked about the decision; one asking something else keeps
  waiting. `kb wait` returns the answer even when the task was moved right after it, and no longer takes a task
  retitled while it waits for a deleted one.
- `describe -` and `project describe|notes -` with nothing on stdin are refused instead of wiping the text; a word
  `link` can't read is refused instead of dropped; files that don't read as entries are named on stderr.
- `-h` in a comment's words is text; `KANBRAIN_AUTHOR` is read in any case; the Stop hook no longer tells an agent to
  finish another session's task.
- `kb init NAME` moves a repository as `kb project move` does; a folder named `My App` gets a name links can hold, as
  does one named after the folder above it; a repository away during a rename follows its project; `kb init --all`
  goes on past a repository it can't update.
- `kb init` no longer writes over a `CLAUDE.md` or `AGENTS.md` it can't read (one not in UTF-8): it leaves it as it
  is and says so.
- A new repository no longer takes the name of one whose folder is gone (an unmounted disk): that one keeps it, and
  stays registered.
- A repository moved to another project keeps its name there unless that's refused, and says why.
- Crash leftovers are cleaned up only among kanbrain's own files, not other programs' hidden `.tmp` files in a
  notes vault used as the central brain.
- `kb update` says when it's already on the latest release, and ignores uninstall, purge and version variables left
  in the shell; the installers fetch every app before replacing any, and say when the app can't start. On Windows
  PowerShell 5.1, uninstalling with `--purge` under a profile with an accent (`C:\Users\José`) found no data folder
  and deleted nothing; a temporary folder with `[` `]` in its name no longer breaks installing.
- The kanban app opens with a repository that can't be read left out, says so once rather than on every agent's
  write, and shows it once it's repaired; it takes each brain's lock while opening, reads again a write an agent made
  while it was starting, and names the repository in the link picker's title. A folder added from Manage project
  joins that project though the board changed while the folder dialog was open; the form's project list keeps its
  highlighted row; a decision already rejected and an entry the form created are named with their repository.

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

[Unreleased]: https://github.com/singudotdev/kanbrain/compare/v0.4.1...HEAD
[0.4.1]: https://github.com/singudotdev/kanbrain/compare/v0.4.0...v0.4.1
[0.4.0]: https://github.com/singudotdev/kanbrain/compare/v0.3.5...v0.4.0
[0.3.5]: https://github.com/singudotdev/kanbrain/compare/v0.3.4...v0.3.5
[0.3.4]: https://github.com/singudotdev/kanbrain/compare/v0.3.3...v0.3.4
[0.3.3]: https://github.com/singudotdev/kanbrain/compare/v0.3.2...v0.3.3
[0.3.2]: https://github.com/singudotdev/kanbrain/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/singudotdev/kanbrain/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/singudotdev/kanbrain/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/singudotdev/kanbrain/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/singudotdev/kanbrain/releases/tag/v0.1.0
