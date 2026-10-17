# Development

## Building and testing

The repository is a Cargo workspace with four crates:

| crate | folder | what |
| --- | --- | --- |
| `kanbrain-core` | `core/` | everything shared: entries, files, tags, the `Brain` and its actions, reports, the command line |
| `kanbrain` | `cli/` | the command line, `kb`: a `main` that runs the core's `cli::Program` |
| `kanbrain-app` | `gui/` | the kanban app, `kanbrain` (GPUI); given arguments, it runs `cli::Program` too |
| `xtask` | `xtask/` | the workspace's own tasks, `cargo xtask`: the release archives' third-party notices |

Plain `cargo` commands cover the core and the command line (the workspace's default members), which build fast
and without GPUI. Add `-p kanbrain-app` or `--workspace` for the kanban app.

Run a command of the command line:

```sh
cargo run -- list all
```

Run the kanban app:

```sh
cargo run -p kanbrain-app
```

The core and command line tests:

```sh
cargo test
```

All the tests, the kanban app's too:

```sh
cargo test --workspace
```

Keep `TMPDIR` (and a mutation run's worktree) out of any folder with a `.brain` above it, as this repository has
once kanbrain's own tasks are kept in it: `kb` takes the nearest `.brain` above where it runs, so the command line's
tests, run in temporary folders, would write into it. They stop first, saying so, a `TMPDIR` that links there too.

Lints, which must be warning-free (pedantic is on, see below):

```sh
cargo clippy --workspace --all-targets
```

Code for another system (`cfg(windows)`, `cfg(not(unix))`) is only linted for it. Windows' can be checked from Linux
(`rustup target add x86_64-pc-windows-msvc`, then `--target x86_64-pc-windows-msvc`), the app included; macOS's only
for the core and the command line (`--target aarch64-apple-darwin -p kanbrain-core -p kanbrain`), as GPUI's Metal
shaders need Xcode. CI lints on all three.

Formatting (`rustfmt.toml`):

```sh
cargo fmt --all
```

Optimized, stripped binaries of both programs, in `target/release/`:

```sh
cargo build --release --workspace
```

The kanban app needs its graphics libraries' development packages on Linux, see
[Build from source](getting-started.md#requirements).

kanbrain refuses to run as root. In containers where you are root, add `-e KANBRAIN_ALLOW_ROOT=1` (tests
don't need it).

Use a throwaway data folder while developing so you don't touch your real tasks. With the board open, commands run
in another terminal move its cards. The board, on Linux or macOS:

```sh
KANBRAIN_DIR=/tmp/kanbrain-dev cargo run -p kanbrain-app
```

A command, in another terminal:

```sh
KANBRAIN_DIR=/tmp/kanbrain-dev cargo run -- add --start "Try the board"
```

On Windows (PowerShell), the board:

```powershell
$env:KANBRAIN_DIR = "$env:TEMP\kanbrain-dev"; cargo run -p kanbrain-app
```

A command, in another terminal:

```powershell
$env:KANBRAIN_DIR = "$env:TEMP\kanbrain-dev"; cargo run -- add --start "Try the board"
```

No local Rust toolchain? Build in a container instead:

```sh
podman run --rm -v "$PWD":/src:Z -w /src docker.io/library/rust:1 cargo test
```

## Code style

- **Formatting:** `cargo fmt`, configured in `rustfmt.toml` (120 columns; short constructs stay on one line).
- **Lints:** `Cargo.toml` enables clippy's `pedantic` group and `missing_docs_in_private_items` for the whole
  workspace, and forbids `unsafe` code. The few allowed exceptions are listed there with their reason.
- **Small modules with one job**, each starting with a `//!` comment that says what it's for. Functions stay short;
  when a `match` arm grows, it becomes a named helper.
- **Comments:** every item (type, field, variant, function, constant) has a `///` doc comment saying what it's for,
  and every file starts with a `//!` comment describing the module. Inside functions, comments explain *why* (or the
  non-obvious *what*), not what each line already says. Clippy reports undocumented items; check broken doc
  references with:

  ```sh
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --document-private-items   # also browsable: target/doc/kanbrain_core/, target/doc/kb/, target/doc/kanbrain/
  ```
- **Fixed lists of values are declared once.** Statuses, kinds, report units and groups are declared with the macros
  in `enums.rs`: one line per variant, `Variant => "name"`. The list of all variants (`ALL`), their order
  (`position`, `next`), and the conversions to and from text (`as_str`, `parse`) are generated from that line, so they
  can't get out of sync. The `--help` text is built from the same names.
- **Named constants** instead of magic numbers (`DONE_LIMIT`, `MESSAGE_TTL`, `watch::INTERVAL`, …).
- **Errors:** `anyhow::Result` with context (`writing <path>`) at the edges; refusals an agent can act on are typed
  (`api::ApiError`: no such entry, not a task, empty text…). Errors in the kanban app become a status bar message,
  never a crash. `expect` is only used for things that cannot fail (writing to a `String`, calendar arithmetic),
  and the message says why.

## Dependencies

The crates use the **Rust 2024 edition**. Shared versions and lints are in the workspace `Cargo.toml`.

| crate | version | used for |
| --- | --- | --- |
| [`gpui`](https://www.gpui.rs) | Zed 1.23.2 (git) | `gui`: the kanban app's window, layout, input and drag and drop. From Zed's repository, pinned to a tag: the crates.io release (0.2.2) can't draw on NVIDIA with Wayland, and newer GPUI isn't published. The workspace `Cargo.toml` carries the `[patch]` entries Zed builds it with |
| [`chrono`](https://docs.rs/chrono) | 0.4 | dates, times, time zones |
| [`dirs`](https://docs.rs/dirs) | 7 | platform data folder |
| [`anyhow`](https://docs.rs/anyhow) | 1 | error handling |
| [`serde`](https://serde.rs), [`serde_json`](https://docs.rs/serde_json) | 1 | `--json` output; merging Claude Code's settings (`preserve_order` keeps their keys' order) |
| [`rustix`](https://docs.rs/rustix) | 1 | Unix only: the effective user id for the root check, without `unsafe` |
| [`pulldown-cmark`](https://docs.rs/pulldown-cmark) | 0.13 | `gui`: parsing descriptions and comments to draw them as markdown; the version Zed uses |
| [`fs4`](https://docs.rs/fs4) | 1.1 | each brain's write lock, an advisory lock on a file in the cache (`Store::lock`) |
| `gpui_platform` | Zed 1.23.2 (git) | `gui`: GPUI's window system backends, Wayland and X11 on Linux |
| [`image`](https://docs.rs/image) | 0.25 | `gui`: decoding the window icon (PNG) |
| [`winresource`](https://docs.rs/winresource) | 0.1 | `gui`, Windows only, at build time: the program's icon and version in its `.exe` |
| [`similar`](https://docs.rs/similar) | 3.2 | `gui`: the line diff and three-way merge of what the editor and `kb` changed meanwhile (`text::merge`) |
| [`tempfile`](https://docs.rs/tempfile) | 3 | tests only: temporary data folders |

The minimum supported Rust version of the core and the command line is **1.88** (`rust-version` in `core/Cargo.toml` and
`cli/Cargo.toml`): the code uses let chains (`if a && let Some(b) = c`), which edition 2024 allows from Rust 1.88. Cargo
picks dependency versions that still support it. CI checks it with Rust 1.88, and it's fine to develop on any newer
Rust.
The kanban app follows GPUI, which needs a recent stable Rust.
Keep `Cargo.lock` committed: kanbrain is an application, and the lockfile makes builds reproducible.

To update dependencies: `cargo update` for compatible releases; for new major versions, change the version in
`Cargo.toml`, then run `cargo build` and the full check (`cargo fmt --all --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`) and try both programs.
To move GPUI to a newer Zed release, change the tag in `gui/Cargo.toml` and copy that release's `[patch.crates-io]`
entries from Zed's workspace `Cargo.toml`.

## Architecture

```
core/build.rs        stops a version without its release notes in tag-changelog/ from building
core/src/            kanbrain-core: shared by both programs
├── lib.rs
├── brain/           the domain service: every entry in the data folder and the operations on them
│   ├── mod.rs       Brain: loading (and fixing several running sessions or doing tasks), queries, columns, backlinks, related, tags
│   ├── actions.rs   status changes with tracking (one task in doing), create, update, delete, links, descriptions, comments, report export
│   ├── report.rs    time per task or tag for a period, breakdown columns, timeline (ReportView, Group); totals by the clock
│   └── calendar.rs  each day's interactions over a period: sessions, moves, comments, new entries (CalendarView, Show)
├── cli/             the command line
│   ├── mod.rs       Program (the binary's name), Command parsing, `--project`, `list`, `status`, `report`, `path`, usage
│   ├── claude.rs    the Claude Code hooks `init` merges into .claude/settings.local.json (and `deinit` takes out)
│   ├── hook.rs      `kb hook prompt|stop|edit|tool`: the hooks' checks that hold an agent to describing the project first, and to `kb` for the brain's files, and tell it of the user's comments
│   ├── entries.rs   agent commands by id: `add`, `show`, `search`, status changes, `comment`, `ask`, `propose`, `accept`/`reject`, `describe`, `rename`, `tag`, `link`, `context`
│   ├── completions.rs `completions bash|zsh|fish|powershell`: the scripts, and `complete`, the ids, projects and tags they offer
│   ├── update.rs    `update`, and the app's: the latest release asked for, the installer run again for the apps beside this one, into the same folder (a macOS bundle's says which), once no agent has a task in doing
│   ├── init.rs      `init`: the repository's .brain/ and card, its registration, the "Project memory" section in CLAUDE.md / AGENTS.md; `init --all` refreshes every registered repository's; `deinit` undoes it
│   └── manage.rs    renaming a project or a repository, moving a repository to another project, removing them (`deinit --purge`, `project remove`), rewriting or dropping the links between repositories
├── api/             operations by id for the CLI, with typed refusals (ApiError)
│   ├── mod.rs       add, change_status, comment, describe, link, full-text search
│   ├── json.rs      entries as JSON, one line without empty fields (Entry, Summary)
│   ├── text.rs      entries as compact text for agents: `show`, one-line answers, capped `search`
│   └── context.rs   the markdown summary agents read first: the project's card, capped sections, latest comments
├── model/           the domain
│   ├── mod.rs       Task (any entry), Kind, Status, StatusChange, Session, Comment, Activity (changes, logged)
│   ├── markdown.rs  the task file format: writing and parsing
│   ├── text.rs      tags, slugs, heading rules for user text
│   └── time.rs      timestamps, ages (`5m`, `3h`, `2d`), durations (`1h 05m`, `00:42:17`), time covered by overlapping spans
├── period.rs        report periods: hour, day, ISO week, month
├── export.rs        reports as markdown tables
├── enums.rs         `named_enum!` / `ordered_enum!`: enums whose variants are listed in one place
├── platform.rs      OS specifics: the per-user (not root) check
├── projects.rs      projects of one or more repositories: the central brain's projects.md, the shared cards (projects/NAME.md), each repository's project.md
├── migrate.rs       data format versions (FORMAT, the `format` file) and the migrations between them, with a backup first
├── store.rs         the data folder: entry files, tags.md, last-id, reports/; its write lock (in the user's cache folder); finding a project's .brain/; recording the files a change writes, for undo
└── tags.rs          tag colors: palette, no-repeat assignment, tags.md format

cli/README.md        the `kanbrain` crate's page on crates.io: `kb` only (core/README.md: kanbrain-core's)
cli/src/main.rs      kb (the crate `kanbrain` on crates.io): runs `cli::Program`
cli/tests/kb.rs      the binary run from outside: arguments, stdin, output, exit codes
e2e/install.sh       install.sh end to end, from a local copy of a release (CI; install.ps1 for Windows)
e2e/upgrade.sh       brains written by every released kb, opened by this build (CI)
e2e/update.sh        every released kb update installing this build (CI)
e2e/smoke-app.sh     the release app starts, stays up and idles; a screenshot (CI; smoke-app.ps1 for Windows)
e2e/agent.sh         a real agent working with kanbrain, scenario by scenario (run by hand)
docs/decisions/      why things are as they are, one decision a file (NNNN-title.md)
xtask/               `cargo xtask notices kb|kanbrain` (the alias is in .cargo/config.toml): THIRD-PARTY-LICENSES.txt,
                     from cargo-about (about.toml) and licenses/ for what's compiled in but isn't a crate

gui/                 the crate `kanbrain-app`, the `kanbrain` program; not on crates.io (GPUI from git)
gui/build.rs         embeds the icon into kanbrain.exe when building for Windows
gui/icons/           the app's icons, made from docs/images/logo.svg by make.sh: .ico (Windows), .icns (macOS), 128 px PNG (X11)
gui/fonts/           IBM Plex Sans and Lilex (from Zed's repository, with their licenses), embedded: the window's text and code
gui/src/             kanbrain: the kanban app, and `cli::Program` when given arguments
├── main.rs          entry point: --help / --version, the root check, opening the window
├── update.rs        the app's update: a newer release asked for daily, installed in place of the app (on Windows by PowerShell once it closes), which then starts again
├── watch.rs         polls the data folders (names, sizes, times) every second, naming those other programs changed
├── state/           what's on screen and every action, without GPUI (tested without a window)
│   ├── mod.rs       Board: the brains, the scope, the tab and the board's layout (Layout: kanban or list), the selected entry (by Key), the details, search, report view, status message, what the window must do (Request: $EDITOR, the folder dialog, the update), a newer release found; columns and lists
│   ├── mode.rs      the dialogs and fields that take the keys (Mode): search, form, quick add, editor, confirm, links, project and tag dropdowns, which project a quick-added task goes in or an entry moves to, Manage project, command palette, adding a project (AddProject, ProjectOption), renaming, moving or removing a project or repository (Manage)
│   ├── actions.rs   status changes and drops, asking which brain a new entry goes in when the scope shows several, moving an entry to another brain, the editor's save, deletes, links, undo (recording what each change writes), reload, park & quit, export
│   ├── decisions.rs the questions and decisions waiting on you: which, the next one, answering, accepting or rejecting
│   ├── form.rs      the new / edit form (its tags to pick, related entries, column and project) and quick add
│   ├── manage.rs    renaming, moving and removing projects and repositories from Manage project
│   ├── input.rs     keys in each dialog; typing, select all, cut, paste, the mouse in text
│   ├── keys.rs      every key outside dialogs in one table (ACTIONS: key, tabs, when it's offered, palette text, menu label, what it runs), which the keys, the palette and the menu read
│   ├── menu.rs      the right-click menu: the table's actions for that entry, each one of its keys (MenuPicker)
│   ├── palette.rs   the command palette (`?`): the table's actions that work now, in sections (the selected entry, the tab, everywhere), searchable; choosing one presses its key
│   ├── nav.rs       moving the selection between cards, columns and list rows
│   └── sources.rs   the central brain and every project repository (Source), entries by Key { src, id }, the Scope filter, merged reports and calendars, adding a project from a picked folder (`cli::init`)
├── text/            text editing with a selection
│   ├── mod.rs       the editors and the key presses they take, without any drawing
│   ├── editor.rs    multi-line TextEditor (descriptions, comments), one-line LineInput (fields)
│   ├── merge.rs     the three-way line merge of what the editor and `kb` changed meanwhile (Merge, its conflicts and picks)
│   └── event.rs     KeyCode / KeyEvent / KeyModifiers: what GPUI keystrokes become
└── ui/              the window: draws the Board with GPUI and turns clicks, drags and keys into its calls
    ├── mod.rs       BrainView (the root view), the window and its app id, ticking, watching, the status bar, `action` buttons, clipboard, the system's folder dialog, $EDITOR
    ├── keys.rs      GPUI keystrokes → the board's keys, and keys as people write them (`Ctrl+C`)
    ├── text.rs      text cut to a width, dates, a field's text with its selection highlighted
    ├── topbar.rs    the logo, tabs, the project filter and + Add project, the shown project's description, search and the tag filter, New / Reload / Commands; the running tasks with Park and Park & quit
    ├── board.rs     the columns, cards (dragged as DraggedCard), quick add, "Show N older"
    ├── list.rs      the board's list layout: a section per status, a row per task, dragged like cards
    ├── drawer.rs    the details beside the board or list
    ├── entries.rs   the notes and decisions lists
    ├── report.rs    the report tab
    ├── calendar.rs  the calendar tab: day, week and month
    ├── dialogs/     dialogs over the window; a delete's confirmation in mod.rs
    │   ├── form.rs     the new / edit form: project, title, tags, related entries, column
    │   ├── editor.rs   the text editor for descriptions, comments and project text
│   ├── merge.rs    the conflicts of a merge, shown in the editor's place: yours beside theirs, to pick
│   ├── tell.rs     Tell the agent?, asked on saving a task an agent is working on
    │   ├── pickers.rs  the link picker, which project, the command palette
    │   ├── project.rs  Manage project: description, repositories, the week's time, and the buttons that manage them
    │   ├── manage.rs   renaming a project or repository, moving a repository, removing either
    │   └── add_project.rs  adding a project: the folder picked, and the project it joins or starts, from a searchable list
    ├── markdown.rs  descriptions, comments and project text drawn as markdown: parsed into Blocks, then drawn
    ├── menu.rs      the right-click menu, drawn at the click (`on_right_click` on cards and list rows)
    ├── dropdown.rs  dropdowns with a search box (the project filter and the tag filters), and the list and row every dropdown and picker is built from
    ├── widgets.rs   buttons, segmented controls, text fields, tag chips, cards, tooltips
    ├── pointer.rs   the mouse in text fields and the editor: place the cursor, drag to select, double/triple-click, middle-click paste
    ├── caret.rs     the text cursor: a thin line drawn over the text, blinking (`Blink`, restarted on every input); a one-line field scrolled to keep it in view (`keep_in_view`)
    └── theme.rs     colors, fonts, status colors, styled text

core/tests/, gui/tests/, xtask/tests/   each crate's tests (see Tests)
```

### Data flow

```
key press ───────────▶ Board::on_key (keys.rs, input.rs) ──▶ Board action (actions.rs) ──▶ Brain action (core)
button ──▶ BrainView::press ──┘                                                        │
card dropped on a column ──▶ Board::drop_card ─────────────────────────────────────────┤
CLI command ──▶ api ──▶ Brain action ──────────────────────────────────────────────────┤
                                                                                       ├─▶ Task::set_status (history line, sessions)
                                                                                       └─▶ Store::save ──▶ NNNN-slug.md (atomic write)

after each input, every 500 ms while a task runs or a message shows, else every minute ──▶ ui draws &Board
                                                             (ages and clocks are computed from `now`)
every second / before each input ──▶ Watcher::changed ──▶ Board::reload_changed    (changes made by other programs)
```

- **The files are the source of truth.** Every change is written immediately; there's no in-memory state that could be lost.
  `Brain::reload` rebuilds a brain from disk. When the watcher sees a change, every second and before each input (so
  the board never saves a whole entry over one an agent just changed), `Board::reload_changed` re-reads only the
  brains whose folders changed, and everything when the list of projects or their cards may have. After `$EDITOR`
  returns, and on `R`, everything is re-read. After its own input it marks the folders as seen (`Watcher::sync`), so
  its own saves don't trigger a reload. Each input holds every brain's write lock (`Store::lock`, the one `kb`
  takes) from that re-read to its saves, so an agent's `kb` can't write in between, nor the board over it.
- **Selection by key.** The board selects entries by `Key { src, id }` (which brain, and the id there, since every
  brain numbers from 1), never by position, so reloads and moves never jump to another card.
- **Status changes** go through `Brain::set_status`, which refuses a person's start while one of theirs runs (agents' aren't limited), opens and closes
  sessions, and appends a `StatusChange` to the history only when the status really changes. The `- status:` line in
  the file is the truth; the history is the record of how the task got there.
- **Modes** (`state::Mode`): `Normal`, `Search`, `Form`, `QuickAdd`, `Edit` (text editor with a `Target`: description,
  new or existing comment, a decision's reason, a project's description or a repository's notes), `Confirm` (a
  delete, or the update), `Links`, `Scopes` and `Tags` (the dropdowns), `Where` (which brain a quick-added task goes
  in, or an entry moves to), `Menu` (the right-click menu), `ManageProject`, `Palette` (the command
  palette), `AddProject` (naming a picked folder's project) and `Manage` (renaming, moving or removing from Manage
  project). A reload points an open one at its brains' new places (`Mode::remap`). `on_key` takes the current mode out, handles the
  key, and each handler puts back the mode to continue in.
- **A button is its key.** `ui::action(id, label, kind, code, cx)` makes a button that presses its key through
  `BrainView::press`, with the key in its tooltip. So a button can never do something its key wouldn't, and the state
  tests, which drive the board with keys, cover the buttons too. Clicks that aren't keys (select, open, drop, focus a
  field) call a `Board` method through `BrainView::act`.
- **Drag and drop:** a card is `on_drag(DraggedCard { key, title })` with a small preview view; each column has
  `drag_over` (highlight) and `on_drop`, which calls `Board::drop_card`.
- **Clipboard:** `BrainView::clipboard` catches `ctrl+a/c/x/v` (`cmd` on macOS) before `Board::on_key` and calls
  `Board::select_all`, `selected_text`, `cut` and `paste` with the system clipboard. `shift` with a movement key extends
  the selection, and typing or pasting replaces it.
- **Mouse in text:** `ui::pointer` keeps each drawn line's GPUI `TextLayout` (`Hit`) to map the mouse to a
  character, and calls `Board::point` with an `editor::Pointer` (place, extend, word, line). Drags use window-wide
  listeners, so they keep selecting outside the field; `BrainView::drag` names the field. On Linux,
  `BrainView::share_selection` copies the selection to the primary selection after each input.

### Adding an action

1. Put the change to entries in `core/src/brain/actions.rs` if the command line or agents need it too, and call it
   from a `Board` method in `gui/src/state/actions.rs`.
2. Give it a row in `ACTIONS` in `gui/src/state/keys.rs`: its key, the tabs it works on, when it's worth offering,
   what the palette says, and a menu label if the right-click menu offers it. The key, the palette and the menu
   then all have it; a test refuses a key that runs two things on one tab.
3. Add a button where it belongs with `ui::action`.
4. Document it in [`keybindings.md`](keybindings.md) and the relevant page.

### Changing the file format

`model/markdown.rs` documents the format at the top and holds both the writer and the parser; change them together and
extend the `round_trip` test. Unknown `- key:` lines, `- links:` words and `##` sections are kept in `Task::unknown` and written back, so a version
that doesn't know a new field doesn't drop it when it saves the entry.
kanbrain has been released, so the format stays backward compatible: data folders written by any released
version must keep loading. A change an older version would misread (a field whose absence means something else, a
file moved or renamed, a meaning changed) is a new data format:

1. add the step to `MIGRATIONS` in `core/src/migrate.rs`, a function that brings a brain from the previous format,
   and bump `FORMAT` (a test checks every format after the first has its step);
2. add a test with a brain in the old format, migrated to what the new one expects;
3. say it in the version's release notes.

Brains migrate when opened to be changed and on `kb update`; one in a newer format than the running version is
refused, so an older `kb` never rewrites what it doesn't know.

Steps chain: a brain several formats behind, from a user skipping versions, runs every step it lacks in order, so:

- a step is never removed, however old: someone may update from the first version;
- a step works on the files' text, not through today's types (`Task`, `Store::load`): those change with later
  formats, and a step written through them would misread or drop what its format had.

## Tests

Eight layers: unit tests of `core` and `gui` (below), the kanban app in a real test window (`ui`), the `kb` binary run
from outside ([below](#the-command-line-from-outside)), the installers end to end
([below](#installers-end-to-end)), brains of every release opened by this build (`e2e/upgrade.sh`), the release app
started on each system (`e2e/smoke-app.*`), mutation testing of every change to Rust (`mutants.yml`), and a real
agent end to end ([below](#agents-end-to-end), by hand).

Unit tests live in each crate's `tests/` folder, one file per module, laid out like `src/` in `core` and side by side in
`gui`: `gui/src/text/editor.rs` is tested by `gui/tests/editor.rs`, `core/src/model/mod.rs` by `core/tests/model.rs`. They have access to private items:
the module includes its test file with a hook at its end, and the file starts with `use super::*`:

```rust
#[cfg(test)]
#[path = "../../tests/editor.rs"]
mod tests;
```

The path is relative to the module's folder. `autotests = false` in each crate stops Cargo from also building these
files as integration tests. Keep them inside the crate: rust-analyzer doesn't follow `#[path]` out of it.

| module | covers |
| --- | --- |
| `brain` | status history, columns, one person's task in doing and agents' unlimited (a second start refused, loading parks the extra ones), sessions, comments, links, renames and deletes, ids never given twice, moving to another brain, reports, the calendar's days and filters, the decision template |
| `api`, `cli` | agent operations and their refusals, links to other repositories, compact text and JSON, `context`'s caps, command parsing, `--project`, `list` output, usage, completion scripts and candidates, which apps `update` installs and where (a macOS bundle's folder), whether a release is newer, an update waiting for agents' tasks in doing and not a person's |
| `cli::init` | the agent section written, updated in place and left alone when unclosed; a CRLF file keeping its line endings, a second `init` leaving it as it is, a section an older kanbrain wrote into it with LF mended, and `deinit` taking the section out cleanly; a file holding only a byte order mark taken for empty; `init --all` updating every registered repository that has it, and no other; registering repositories and joining them into a project; `deinit` taking it all back but `.brain/`; the Claude Code hooks added once beside other settings, and removed leaving them as they were; `kb hook` asking with every prompt and reminding once a turn while the project or this repository's role isn't described, and of a task an agent started and left uncommented, and silent once they are; `kb hook tool` telling an agent of the user's comment on its task until it comments, and not of what came before it started; `kb hook stop` counting only the agent's comments; `kb context` adding the hooks a release added only where kanbrain's are; `kb hook edit` refusing a write to a brain's file, through a link or a `..` too, and leaving a notes vault's own files and a repository kept in the central brain to the agent |
| `projects` | the registry and its labels, cards, moving a repository between projects, refusing two repositories with one name |
| `migrate` | every format after the first having its step; an old brain backed up, then migrated step by step and only once, resuming from where it is; a brain in a newer format refused, read or written |
| `cli::manage` | two folders named alike getting distinct repository names (`--repo` refused when taken); renaming a repository rewriting its siblings' links; renaming a project with its card and repositories; moving a repository dropping its links to the old project; removing keeping or deleting the tasks, and a whole project |
| `model` | file round trip, what a newer version wrote kept, hand-typed history, missing id, status changes recorded once, search matching, tags, slugs, heading rules, timestamps, ages, overlapping time counted once |
| `store` | renaming on title change, unreadable files become warnings, `tags.md` isn't a task, whole-file writes |
| `tags` | no color repeats until the palette is used up, `tags.md` round trip, custom colors |
| `period`, `export` | period bounds and stepping, report tables |
| `enums` | generated `ALL` / `position` / `next`, `parse` as the inverse of `as_str` |
| `state` (gui) | the board driven by keys over a temporary folder: the palette offering only what works now, moving between cards and columns, moving cards, drops, quick add, the form, the tags it offers and picks, held as chips (added by a separator typed or pasted, taken off with ×), and its related entries, the editor, deletes, search and tag filters per tab, tabs, links, several brains and the project filter, a new entry going where the filter points and its form picking another brain, moving an entry to another one, the right-click menu's items and keys, the command palette, nothing asked once your task stops, the report, the calendar, selection kept across reloads, only changed brains read again, undo and redo, adding a project from a picked folder (its name, and joining one), renaming, moving and removing projects and repositories from Manage project, the update offered once a newer release is found and refused while an agent works, the details' buttons acting on the entry from the calendar too, the form's tags highlight after a paste, a cut or a reload, an edit saving only what was changed in the form over what `kb` changed meanwhile (also when the list of brains changes while it's open), a save merging what `kb` changed meanwhile in a description, comment, project description or role and stopping over the lines both changed, to pick yours, theirs or both in each (in either case, yours for the rest, merged again once `kb` changes the text again), or over a title, which `ctrl+t` takes as `kb` left it, a save stopping after the agent's question asking it again with the note, a comment moved meanwhile not saved over another, a verdict `kb` gave meanwhile not overturned unseen, an open form or editor marked, and said once, when `kb` changed what it edits or the task's status, said once too when a save stops over it, a comment's delete taking the comment asked about wherever it is now, or none if it changed, opening one on a task an agent has in doing saying so, a brain found while the board acts locked and read again under its lock, saving a task an agent is working on asking whether to tell it (with a note, or just saving, or back), undone in one, and the question kept across a reload and closed with its entry |
| `dialogs` (gui) | the delete question naming the entry, what removing one repository or several says, Manage project scrolling to the repository it's opened from, the editor naming what its text belongs to, the form's and editor's titles marked `changed on disk`, the editor's unsaved dot kept last, a merge's title counting the conflicts left, its lines keeping their empty ones, and a side showing nothing saying what it is |
| `keys` (gui) | no key running two things on one tab; what the menu offers named and described; every key in `docs/keybindings.md` on its tabs |
| `editor` (gui) | line editing (split/join), unicode, wrapping; one-line cursor editing, line breaks pasted into one line (a lone `\r` too) as one space, as `kb add` takes a title; selecting, copying and replacing; mouse selection by character, word and line |
| `merge` (gui) | a three-way merge of the editor's text and `kb`'s: lines only one side changed merged (the last line too, with or without a line break after it), the same change on both no conflict, changes that only touch merged (a line edited and one added after it) but lines added at one place or within the other's not, lines both changed a conflict between settled lines, picks making the text and moving to the next conflict, the rest taking one side without undoing the picks made, and moving between conflicts stopping at the ends |
| `markdown` (gui) | inline styles and link ranges, kept line breaks, heading levels, numbered and task lists, nesting, code, quotes, tables, rules |
| `update` (gui) | a program replaced on disk since it started restarting from the new file |
| `watch` (gui) | new, changed, renamed and deleted files are noticed once, naming the folder; temporary files aren't; own writes are taken as seen |
| `xtask` | the notices list each license with the crates under it and leave this workspace's out; each program carries Rust's standard library and what else it embeds (musl, the app's fonts in `gui/fonts/`) |
| `ui` (gui) | in a real (test) window: GPUI keystrokes to keys, shortcut names, clicks and double-clicks on cards (in every brain), a right click's menu acting on its card, dragging a card onto a column, closing the details (no + under ✕ once they close; a click or a double-click on ✕ opening nothing), reloading after another program's change, acting right after one without undoing it, opening an entry from the calendar, buttons pressed while typing, dropdowns that scroll to the highlighted row and how many rows each shows, the form's tag list and a click picking one, the list following its highlight, clipboard shortcuts, + Add project through the folder dialog, a one-line field scrolling sideways to keep its cursor in view, one shown again starting at its start, conflicts drawn with both sides and picked with a click, and the form's line with the title `kb` gave, taken with its button |

### The command line, from outside

`cli/tests/kb.rs` runs the built `kb` the way agents and scripts do, over temporary folders: an agent's session from
`init` to `context`, text read from stdin with `-`, one-line JSON, errors on stderr with a failing exit code and the
usage, `--project` from another folder, completion candidates, the hooks asking until the project is described, the
edit hook refusing a write to `.brain/` (and a `~` path read as the home folder), `tag` and `rename` and what they
say when nothing changed, bash running `kb completions bash` itself to complete `tag`'s id and then its signed tags,
PowerShell completing through `kb completions powershell` (`pwsh`, and on Windows also 5.1; skipped only off CI
without `pwsh`), quoting what it inserts and reading `kb`'s UTF-8 under any code page, an empty `KANBRAIN_DIR` read
as unset rather than the current folder and a relative one refused, and sixteen commands run at once losing no
change and no id (the brain's write lock). They're Cargo integration tests (`CARGO_BIN_EXE_kb`), so they see only
what the binary prints.

### Installers, end to end

`e2e/install.sh` (Linux, macOS) and `e2e/install.ps1` (Windows) package this build of `kb` as a release asset, serve
it over HTTP on `127.0.0.1` and install it with `install.sh` / `install.ps1` into a throwaway home, as a user would
(`KANBRAIN_BASE_URL` points the installer at it). They check that the download's checksum is verified and a wrong
one refused, that the installed `kb` is this build, that `init --all` brought a registered project's agent section
up to date, that uninstalling takes it out, and that `--purge` deletes the central brain's files and kanbrain's
locks and backups in the cache, leftover temporary files included, keeps a file kanbrain didn't create, and refuses
a relative `KANBRAIN_DIR`. Both also install the kanban app (`kb` standing in for it, so GPUI isn't built) and check
its launcher: the menu entry and icon on Linux, the app bundle on macOS, the Start Menu shortcut on Windows, where
both apps install at once. CI runs them on all three systems, after `shellcheck` on the shell scripts and
PSScriptAnalyzer on the PowerShell ones; locally they need cargo and Python.

### Upgrades, end to end

`e2e/upgrade.sh` (Linux) downloads the `kb` of every release tag, checksum checked, and has it write a varied set of
brains in a throwaway home: the central one, a project of two repositories and one of one, with tasks in every status
and their sessions, notes, decisions accepted, rejected and proposed, questions answered and not, agents' comments,
links within a brain and across repositories, unicode and markdown. This build then reads a copy without changing a
file, shows every entry as the old version did, field by field (it may add fields), and runs `init --all` and writes.
Releases that took the names v0.4.0 refuses also write a project named `-dash`, one named `Central` and a repository
named `-docs`, which `init --all` must rename to `dash`, `Central-project` and `docs` with their tasks; at least one
release must have written each. A tag whose release isn't published yet is skipped. CI runs it when a change touches
it; locally it needs curl, git, python3 and the tags. The old `kb` then writes an entry this build logged changes to,
and must keep its `## Activity`.

`e2e/update.sh` (Linux) runs every release's `kb update` before this build is released: a `curl` put first on `PATH`
answers for GitHub, with a latest release after every one there is, this tree's `install.sh` (which releases before
v0.4.0 fetch from `main`, later ones from the latest release) and an archive of this build's `kb`. Each release's
`kb` writes a central brain and a project, updates, and then its folder must hold this build's `kb`, which reads the
brains, and running `init --all` again must change no agent instructions, as the installer ran it. CI runs it in the
same job, also when `install.sh` changes.

### The app, started

`e2e/smoke-app.sh` (Linux, macOS) and `e2e/smoke-app.ps1` (Windows) start the release build of the kanban app on a
few demo tasks, check that it stays up and idles (at most 3% of a core: with nothing running it redraws once a
minute), and save a screenshot for a person to look at. CI runs them on all three systems, Linux under Xvfb with
Mesa's software Vulkan, and keeps the screenshots as the run's artifacts.

### Mutation testing

`cargo mutants --in-diff FILE` alters every changed line (`==` for `!=`, a body for `Ok(())`…) and runs the tests
again: a change no test notices is behavior nothing guards. CI runs it on each push's and pull request's diff and
fails on a mutant no test catches. `.cargo/mutants.toml` lists what no test can tell apart (starting the installer,
a hover style), each with why; anything else missed is a test to write. Locally, run it from a clean checkout
(`git worktree add`) outside the repository (see `TMPDIR` above): it copies the working tree as it is when it starts.

### Agents, end to end

`e2e/agent.sh` checks that a real coding agent works with kanbrain the way `init` tells it to. It runs Claude
Code headless on small throwaway projects, one per scenario, and judges what landed in `.brain/` and in the code:

| scenario | the agent… |
| --- | --- |
| `implement` | in an undescribed project, describes it first; then the change gets a task, started, commented on and finished |
| `plan` | asked for a plan, adds a task per item in To do, and starts none |
| `question` | asked a question, answers it and adds nothing |
| `decision` | choosing between approaches, records a decision with Context, Decision and Consequences |
| `ask` | told the choice is the user's, who is away, asks it on the task (parked, waiting) instead of guessing |
| `propose` | facing a storage choice nobody made, proposes a decision and leaves its task waiting on it |
| `wait` | with the user at the board, asks, waits with `kb wait` in the background, and finishes with the answer in the same session |
| `answered` | in a new session, picks up the task whose question was answered, and applies the answer |
| `full` | with the user's own task running, starts its own beside it, does the work, and leaves the user's alone |
| `parallel` | asked for three independent changes by subagents, gives each its own task, running at once, each commented on |
| `told` | commented on by the user while it works, answers on the task and does what was asked, in the same session |
| `links` | told of a pull request by its URL, names it on the board with its link each time |
| `repos` | with two repositories, leaves the other one's work there as a task, linked to the task here |
| `resume` | in a new session, picks up the task that was running, from its last comment |
| `edit` | without the section, told to edit a task's file in `.brain/`, is refused by the hook and marks it done with `kb` |

It calls the model, so it costs money (`E2E_BUDGET` dollars per scenario at most, 1 by default) and takes minutes,
and an agent's choices vary between runs: run it by hand before a release that changes what agents are told (the
section, `context`, the hooks), not in CI. `e2e/agent.sh plan decision` runs only those; `E2E_MODEL` picks the model.

The kanban app's look is checked by running it (`KANBRAIN_DIR=/tmp/t cargo run -p kanbrain-app`) and looking at it.

## Continuous integration

`.github/workflows/ci.yml` runs on every push to `main` and `develop` and every pull request, each job as the change
needs it (see below):

- `cargo clippy --workspace --all-targets -- -D warnings` and the docs with
  `cargo doc --workspace --no-deps --document-private-items` failing on warnings, on Linux, macOS and Windows (code
  behind `cfg(windows)` or `cfg(not(unix))` is only compiled there); on Linux also `cargo fmt --all --check`, and
  the published crates packaged as they'd be published (`cargo publish --dry-run -p kanbrain-core -p kanbrain`)
- `cargo deny check`: dependency licenses, sources and security advisories, as `deny.toml` allows them; and the
  third-party notices still generate (`cargo xtask notices`, through cargo-about), with every license accepted
- the installers end to end on Linux, macOS and Windows (`e2e/install.sh`, `e2e/install.ps1`, the latter in
  PowerShell 7 and Windows PowerShell 5.1, from a folder with an accented name), after `shellcheck` and
  PSScriptAnalyzer on every script
- `cargo test --workspace` on Linux, macOS and Windows
- `cargo test` (core and command line) with the minimum supported Rust version (1.88)
- mutation testing of the change (`cargo mutants --in-diff`), failing on a mutant no test catches: its own
  workflow, `mutants.yml`, in eight shards, testing the lines changed since the last commit whose run passed, so
  every push's lines are tested though a newer push replaces a run still waiting
- brains written by every released `kb` opened by this build (`e2e/upgrade.sh`), and every released `kb update`
  installing it (`e2e/update.sh`)
- the release app started on Linux, macOS and Windows, idle under 3% of a core, its screenshot kept
  (`e2e/smoke-app.sh`, `e2e/smoke-app.ps1`)

Each job runs only when the change touches what it checks: a first `plan` job (`.github/plan.sh`) compares the
change, a rename's both paths, with the last commit on the branch (pushed to, or a pull request's base) whose push
run passed; with none, a push is measured from the push before it, and a pull request runs every job.

| Changed | Jobs |
| --- | --- |
| `core/`, or a file built into it or its tests (`tag-changelog/`, `docs/agents.md`, `docs/cli.md`) | all but the licenses |
| `cli/`, or the README (a test reads its commands) | lints, tests, minimum Rust version, installers, upgrade |
| `install.sh`, which every release's `kb update` runs | the installers, upgrade |
| `gui/`, or a file built into it or its tests (`docs/keybindings.md`, the logo, `install.sh`) | lints, tests, the app started; its fonts the licenses too |
| `xtask/` | lints, tests, licenses |
| a script (`*.sh`, `*.ps1`), or the app's icons (`gui/icons/`, which the installers carry) | the installers (with `shellcheck` and PSScriptAnalyzer); `e2e/upgrade.sh`, `e2e/update.sh` and `e2e/smoke-app.*` their own job too |
| the workflows or `.github/plan.sh`, `Cargo.toml`, `Cargo.lock`, the toolchain or tools' configuration (`.cargo/`, `rustfmt.toml`, `deny.toml`) | everything |
| a crate's `Cargo.toml` | that crate's jobs, and the licenses |
| only documentation | nothing |

Mutation testing runs when Rust lines changed, whatever the folder.

A `release/*` branch runs everything, unless its tree was checked already. A tree a passing run of this repository
(not a fork's pull request, which runs its own workflow) already checked runs nothing again: the squash of a pull
request into `develop`, of a release into `main`, or a commit only re-signed, holds the tree its pull request passed
with. Each run keeps an artifact named after its tree (`ci-tree-…`, `mutants-tree-…`, 90 days) for a later run to find.

`.github/workflows/release.yml` refuses a tag that doesn't match the version in `Cargo.toml`, or whose commit has no
passing `ci.yml` and `mutants.yml` run (one that found its tree already checked counts).

## Branches

`main` is what's released: the last version tagged, working, and nothing else. Work happens on a branch per change
(`feat/…`, `fix/…`, `docs/…`, `test/…`, `ci/…`) from `develop`, and comes back to it through a pull request once CI and
mutation testing pass on it, squashed into one commit whose message is the pull request's title (one line,
imperative, conventional), so `develop`'s history is a change per line. A release is cut from `develop` on
`release/vX.Y.Z`, which adds only its notes and the version, is checked there in full (CI, the installers, the upgrade
from every release, the app by hand), and is squashed into `main` through a pull request: one commit per release,
which is tagged. `main` is then merged back into `develop`, so the next release's pull request holds only what's new.
A fix for a released version starts from `main` on `hotfix/…` and goes back to both.

## Releases and installers

### Publishing a release

1. Every breaking change in the range (a commit marked `!`: `feat!:`, `fix!:`, `refactor!:`, …) comes with its
   migration, so existing users' data and setup move forward on their own: a data format change has its step in
   `MIGRATIONS` (see [Changing the file format](#changing-the-file-format)); a change of behavior or of the agents'
   instructions is fixed on load or by `kb init --all`, which `kb update` runs. Only a change to `kanbrain-core`'s Rust
   API alone, which isn't meant for other crates, needs none; it's breaking all the same when
   `cargo semver-checks -p kanbrain-core --baseline-rev vLAST` says so (see
   [decision 1](decisions/0001-kanbrain-core-follows-cargo-semver.md)).
2. Write the version's release notes in `tag-changelog/vX.Y.Z.md` (terse bullets anchored in the code, naming each
   breaking change's migration) and its section in `CHANGELOG.md`, with its compare link at the end of the file, and
   commit them. `core/build.rs` stops a version without them from compiling in the repository.
3. Update `version` under `[workspace.package]` in `Cargo.toml`, and `kanbrain-core`'s `version` under
   `[workspace.dependencies]` to the same, and the version in the examples of `docs/getting-started.md` and
   `install.ps1`; run `cargo build` so `Cargo.lock` follows, and commit it on its own. Every crate of the workspace
   shares it.
4. Steps 2 and 3 go on `release/vX.Y.Z`, cut from `develop` (see [Branches](#branches)). Open its pull request into
   `main` and wait for CI on Linux, macOS and Windows, and for mutation testing, to pass on it (the latter can take
   hours after many commits). Squash it into `main` as `chore: release vX.Y.Z`, wait for both to pass again on that
   commit, which is the one tagged, and merge `main` back into `develop`. A tag is a release, and the release workflow
   refuses a commit without a passing run of each.
5. Create a signed, annotated tag and push it:

   ```sh
   git tag -s vX.Y.Z -m "vX.Y.Z: <summary>"
   git push origin vX.Y.Z
   ```

`.github/workflows/release.yml` then checks that the tag matches `Cargo.toml` and that CI passed on its commit, builds both programs for every platform
(Linux x86_64 and ARM64, macOS universal, Windows x64), and creates the GitHub release with
`tag-changelog/vX.Y.Z.md` as its notes and the assets below, their `.sha256` files, and `install.sh` / `install.ps1`. From then on the one-line installers work.
Before that it records a signed build provenance attestation for every archive, which
`gh attestation verify <file> -R singudotdev/kanbrain` checks.
Once the release is out, publish the command line to crates.io, the core first since it depends on it:
`cargo publish -p kanbrain-core && cargo publish -p kanbrain`. The kanban app isn't published there: GPUI comes from
Zed's repository, and crates.io takes no git dependencies.

Each program and platform is a separate job (`kb · linux x86_64`, `kanbrain · macos universal`, …), so one failing build
doesn't stop the others, and their files can still be downloaded from the run. The release is only published when all
of them succeed, so the installers never find a file missing.

The installers (`install.sh`, `install.ps1`) download files from the project's **releases**. A release must contain
these assets, each with a matching `<file>.sha256` (the output of `sha256sum <file>`):

| asset | contents | build target |
| --- | --- | --- |
| `kb-x86_64-unknown-linux-musl.tar.gz` | `kb` | `x86_64-unknown-linux-musl` |
| `kb-aarch64-unknown-linux-musl.tar.gz` | `kb` | `aarch64-unknown-linux-musl` |
| `kb-universal-apple-darwin.tar.gz` | `kb` (universal: `lipo` of x86_64 + aarch64) | `x86_64-apple-darwin`, `aarch64-apple-darwin` |
| `kb-x86_64-pc-windows-msvc.zip` | `kb.exe` | `x86_64-pc-windows-msvc` |
| `kanbrain-x86_64-unknown-linux-gnu.tar.gz` | `kanbrain` | `x86_64-unknown-linux-gnu`, built on Ubuntu 22.04 (glibc 2.35) |
| `kanbrain-aarch64-unknown-linux-gnu.tar.gz` | `kanbrain` | `aarch64-unknown-linux-gnu`, built on Ubuntu 22.04 |
| `kanbrain-universal-apple-darwin.tar.gz` | `kanbrain` (universal) | `x86_64-apple-darwin`, `aarch64-apple-darwin` |
| `kanbrain-x86_64-pc-windows-msvc.zip` | `kanbrain.exe` | `x86_64-pc-windows-msvc` |

The command line is static on Linux (musl), so it runs anywhere. The kanban app can't be: it loads the system's graphics
libraries (Vulkan, Wayland/X11, xkbcommon, fontconfig), so it's built against glibc on the oldest supported Ubuntu.

Archives contain the binary, `LICENSE`, `NOTICE` and `THIRD-PARTY-LICENSES.txt` (the licenses of everything
compiled in, which the permissive ones require with every copy: `cargo xtask notices`) at the root, not in a subfolder, plus `kanbrain.icns` in the macOS kanban
app's, for the app bundle `install.sh` makes. macOS builds need a macOS machine or runner (Apple SDK).
Linux and Windows can be cross-compiled from Linux, e.g. for a quick local check. Linux (static, needs `musl-tools`):

```sh
rustup target add x86_64-unknown-linux-musl
cargo build --release -p kanbrain --target x86_64-unknown-linux-musl
```

Windows (needs `mingw-w64`):

```sh
rustup target add x86_64-pc-windows-gnu
cargo build --release -p kanbrain --target x86_64-pc-windows-gnu
```

### Pointing the installers at your repository

They are set to this repository (`singudotdev/kanbrain` on GitHub). For a fork, edit the two lines near the top of **both** scripts:

```sh
REPO="${KANBRAIN_REPO:-singudotdev/kanbrain}"     # install.sh
HOST="${KANBRAIN_HOST:-github}"           # github | gitlab
```

```powershell
$repo = ... 'singudotdev/kanbrain'              # install.ps1
$hostKind = ... 'github'
```

| host | "latest" download URL used |
| --- | --- |
| GitHub | `https://github.com/<repo>/releases/latest/download/<asset>` |
| GitLab | `https://gitlab.com/<repo>/-/releases/permalink/latest/downloads/<asset>` (asset links must use `filepath: /<asset>`) |

Self-hosted GitLab: set `KANBRAIN_GITLAB_URL`. Any other server: set `KANBRAIN_BASE_URL` to the folder containing the assets.
The same variables are handy for testing against a local folder:

```sh
cd dist && python3 -m http.server 8000 &
KANBRAIN_BASE_URL=http://127.0.0.1:8000 sh install.sh --app both --dir ~/kb-test
```

Lint the shell installer with `shellcheck install.sh` (it's kept clean).

The purge logic in both scripts must stay in sync with what the app writes in the central brain: entry files named
`NNNN-slug.md` (`store::Store::save`), `tags.md`, `projects.md` and `projects/*.md` (`projects`), `reports/*.md`
(`Brain::write_report`), `last-id` and `format`, and the temporary files `store::write_file` leaves after a crash
(`.NAME.PID-N.tmp`); in the cache folder, `locks/` and `backups/` (`store::cache_dir_of`, mirrored by
`cache_dir` in `install.sh` and `$cache` in `install.ps1`). Both find the central brain
with `kb --project central path`, never plain `path`, which inside a project names its `.brain/`. If you add
a new kind of file to the data folder, add it to `is_owned` in `install.sh` and `Test-Owned` in `install.ps1`.

## Contributing

See [CONTRIBUTING.md](../CONTRIBUTING.md). A new action in the kanban app needs a key and a button; see
[Adding an action](#adding-an-action).
