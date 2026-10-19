# Changelog

All notable changes to this project are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- `kb completions powershell`: tab completion in PowerShell (5.1 and 7, on Windows, Linux and macOS), as bash, zsh
  and fish have it, each id's title as its tooltip. Load it from `$PROFILE` with
  `kb completions powershell | Out-String | Invoke-Expression`. A project with a space, or a tag or project holding a
  character PowerShell reads otherwise (`$`, quotes, the typographic ones `‘ ’ ‚ ‛ “ ” „` too, brackets, wildcards
  `* ?`, a leading dash `– — ―` it reads as `-`…), comes in single quotes, each single quote doubled. Windows
  PowerShell 5.1 completes no word starting with a single `-` (a tag to take off), which it takes for a parameter.
- CI's lints (`cargo clippy` and `cargo doc`, warnings failing) run on macOS and Windows too, where code behind
  `cfg(windows)` or `cfg(not(unix))` is compiled.
- `kanbrain-core`, for code built on it: `model::Comment::first_line` and `model::Task::doing_by(Author)`;
  `store::Store::load_entries`, with each file skipped as a `store::Skipped` and why (`store::SkipReason`);
  `cli::Step` and `cli::StepKind`, what `cli::init_steps`, `cli::rename_project_steps`, `cli::rename_repo_steps`,
  `cli::move_repo_steps`, `cli::remove_repo_steps` and `cli::remove_project_steps` return; `cli::update::update_program`
  and `cli::update::cargo_update_command`.
- The kanban app selects the text it shows with the mouse, to copy with `ctrl+c` (`cmd+c` on macOS) and, on Linux,
  the primary selection: the details (title, tags, the question, description, comments, the thread's changes, history,
  times), Manage project's descriptions and folders, a delete's confirmation, a merge's lines and the status bar's
  message. Drag, double-click a word, triple-click a paragraph, `shift`+click to extend; a click elsewhere drops it. A
  drag runs across one text's paragraphs, list items and code lines, not into the next comment, and copies them as
  shown, without the markdown, on lines of their own. Links still open with a click, a field's selection is copied
  first, and cards and list rows still drag rather than select.
- The kanban app's board shows scrollbars where it scrolls: one beside a column whose cards don't all show, one under
  the columns when the window is too narrow for them. Drag the thumb to scroll; a press beside it jumps there.

### Changed

- The kanban app's new / edit form shows the tags as chips in their colors, each with × to take it off, instead of
  text: a comma or space after a tag typed, or picking one from the list, adds it as a chip.
- A Claude Code hook is kanbrain's, for `kb init`, `kb context` and `kb deinit` to update or take out, only as `init`
  writes it (`kb hook tool`) or with `kb` given by a plain path: letters (`é` too), digits and `_ . / : -`, a leading
  `~/` in bash, and in quotes also spaces and `( )` (`'/Users/John Doe/bin/kb' hook tool`). A Windows path's `\`s
  only in `'…'` for bash (`'C:\Users\John Doe\AppData\Local\Programs\kanbrain\kb.exe' hook tool`), or after `& `
  under `"shell": "powershell"`; or the exec form (`"command"` the path, `"args"` the arguments). Anything else is
  the user's. One that looks like kanbrain's (`kb` followed by the hook's words anywhere in it: variables set before
  it, another command, a `$`, other quotes) gets none of kanbrain's beside it: `init` says so, to replace it or add
  kanbrain's by hand, and `deinit` leaves it and says to remove it. Variables set before `kb`
  (`KANBRAIN_DIR=… kb context`) made it kanbrain's before, and `deinit` took it out.

### Deprecated

- `kanbrain-core`, for code built on it: `Brain::change_status`, `Brain::comment` and `Brain::describe`, which nothing
  in kanbrain calls any more: `Brain::set_status`, `Brain::add_comment` and `Brain::set_description` do the same.
  `cli::update::update`, which always said `kb`: `cli::update::update_program` names the program running.
  `store::Store::load`, whose warnings are text: `Store::load_entries` gives each file skipped as a `Skipped`, with
  why (`SkipReason`), and says it the same way.
  `cli::init`, `cli::rename_project`, `cli::rename_repo`, `cli::move_repo`, `cli::remove_repo` and
  `cli::remove_project`, which return text: their `_steps` versions give each line as a `Step`, with what it was
  (`StepKind`: a move, links dropped, the tasks deleted…), and say it the same way.

### Fixed

- `kanbrain update`, the app's command line, names itself: `kanbrain X.Y.Z is the latest release`, and installed with
  cargo, `cargo install --locked --git https://github.com/singudotdev/kanbrain --tag vX.Y.Z kanbrain-app && kanbrain
  init --all`, the release it found. It said `kb`, and to run `cargo install kanbrain`, which installs the command
  line, not the app. The app's **Update to X.Y.Z** gives the same command, where it said to "update it the same way".
- `kb report --tag a,b` is refused with the error `kb list --tag a,b` gives: it counted only the first tag's
  tasks, without a word.
- `kb init --all`, so `kb update`, writes nothing into a repository whose `.brain/` a newer kanbrain wrote, which it
  says it leaves alone: it still rewrote that repository's `.brain/project.md` (a project or repository renamed, a
  card on an old project name), its `CLAUDE.md` / `AGENTS.md` section and its Claude Code hooks. Renaming its project
  (there or with `kb project rename`) renames it in the registry and says its card keeps the old name until
  `kb update`. `kb init` in such a repository is refused before anything is written, saying to update. A repository
  whose name `init --all` would fix keeps it, said, while another of its project is such a one: renaming it rewrote
  its card and some siblings' links, then failed on that one, and `init --all` stopped there, so `kb update` failed.
  `kb project repo`, `kb project move`, `kb project remove`, `kb deinit`, `kb init NAME` (a move) and the app's
  Manage project refuse the same way, before anything is written, when the repository's or a sibling's brain is one.
- The kanban app started from the Start Menu, Finder or a desktop's launcher, without a console (on Windows it never
  has one), says an error before the board opens in a small window titled "kanbrain couldn't open", with Copy (the
  message, for a bug report) and Close (or Enter, Esc), and exits with a failing code: running as root, a relative
  `KANBRAIN_DIR`, or a brain it can't read or a newer kanbrain wrote only went to stderr, so the app just didn't open.
  The error still goes to stderr too, and alone on Linux without a display (over ssh, or under `sudo`); `kanbrain`
  with arguments, the command line, is unchanged. The release smoke checks fail on that window or that error.
- The kanban app on Linux without a display (over ssh, under `sudo`, no `WAYLAND_DISPLAY` or `DISPLAY`) exits with
  a failing code and says on stderr to run it from the desktop session: GPUI ran headless, the board's window unseen,
  and the app never ended. A display named but dead fails the same way, saying so: a `WAYLAND_DISPLAY` nothing
  answers at (or with no `XDG_RUNTIME_DIR` for it) says to unset it to use X11, a `DISPLAY` with no X server there
  says `no X server at DISPLAY=…`; GPUI panicked (exit 101), on X11 only after a TCP timeout of minutes. A display
  that refuses the app later (X authorization) exits with a failing code and GPUI's reason instead of a panic. A
  board's window that can't open says why in the "kanbrain couldn't open" window and exits with a failing code: it
  said it on stderr only and exited with 0. The Linux smoke check runs the app with none, a dead `WAYLAND_DISPLAY`
  and a dead `DISPLAY`.
- `kb init` and `kb context` no longer add a second copy of a hook that runs `kb` at a quoted path with spaces
  (`'/Users/John Doe/bin/kb' context`) beside it, and `kb deinit` takes it out with the rest. `kb deinit` no longer
  takes out a hook of the user's that only looks like kanbrain's (`true&&/usr/bin/kb hook tool`, `kb.sh hook tool`).
- In a window smaller than the kanban app's board, the mouse wheel scrolled the column under the pointer and the
  board sideways at once. The vertical wheel now scrolls only the column, a horizontal wheel or `shift`+wheel only
  the board; a touchpad scrolls each way its fingers go, a slanting swipe both.
- `KANBRAIN_DIR` set but empty counts as unset, the platform data folder: `kb` or the kanban app wrote `format` into
  whatever folder it ran in, and the command then failed (`reading …: No such file or directory`). A relative one is
  refused with an error naming it, as it named another central brain from each folder `kb` ran in, hooks included; on
  Windows `\todo` and `D:todo` are relative too. One starting with `~`, which only a shell expands (not inside quotes,
  nor in a systemd environment.d file), says so. `install.sh --purge` and `install.ps1`'s purge refuse a relative one
  the same way.
- `kb update` on Windows prints the install folder in single quotes in the PowerShell command it gives: in double
  quotes, a `$` or a backtick in the folder's name was expanded, so the update went elsewhere. A typographic single
  quote (`‘ ’ ‚ ‛`) in the folder's name, which PowerShell also reads as one, no longer breaks the app's update on
  Windows either.
- `kb init` keeps the line endings of a CRLF `CLAUDE.md` / `AGENTS.md` (git's `core.autocrlf` on Windows): it wrote
  the section with LF and left the CRLF after its end marker, so once git checked the file out again as CRLF (a
  clone, a pull touching it), `init` and `init --all` said it updated the file and rewrote it with mixed line endings
  and one more blank line. The section, and the blank line before it, now go in with the line break of the text
  around it, so a second `init` leaves the file unchanged; a section an older kanbrain wrote that way is mended to
  CRLF, its stray blank line taken out. `kb deinit` takes the section out of a CRLF file without leaving a `\r` or a
  blank line, and removes a file left holding only a byte order mark, which `init` no longer follows with two blank
  lines.
- On Windows, a `kb` command or the app no longer fails when an antivirus, the search indexer or an editor has a
  brain's file open just as it's replaced or deleted (`Access is denied`, `being used by another process`): the
  rename or delete is tried again for up to about 1.3 seconds.
- The kanban app no longer loses what `kb` changed in an open form or editor ("#1's status changed on disk"), nor a
  file it found skipped, when the save or action it read for takes over a second, on a slow disk or a busy machine:
  it's said beside that action's message for the whole key or click, not for a second.
- On Windows, `kb init` registers a repository in `projects.md` as `C:\code\api\.brain` (a share's as
  `\\server\share\…`), not the `\\?\C:\…` canonicalizing gives, which `projects.md`, `kb`'s output and the
  kanban app showed. The prefix stays where the plain path would name something else (a name ending in a dot or
  space, a device name such as `con`, `con.tar.gz`, `COM¹` or `CONIN$`, more than `MAX_PATH`). A registry an earlier
  kanbrain wrote with the prefix reads plain, and is written without it the next time `kb` writes `projects.md`
  (`init`, `deinit`, a move or rename). `kb init` refuses a folder whose path isn't valid Unicode, which
  `projects.md`, a UTF-8 file, can't hold, also when reached through a link: it used to register a garbled path,
  naming another folder.
- `kb deinit` (and `--purge`) takes only kanbrain's hooks out of a matcher group in `.claude/settings.local.json`,
  keeping a hook of the user's in the same `hooks` list, and the group unless kanbrain's were all it held. It took
  the whole group out, the user's hooks with it, and never told of a look-alike there, which it now says to remove.

## [0.6.2] - 2026-10-16

### Added

- The kanban app's editor merges what `kb` changed in a description, comment, project description or role while it
  was open, line by line: lines only one side changed take that change, as do changes that only touch (a line edited
  and one added after it), and the save goes on (`description saved, merged with the change made meanwhile`). Lines
  both changed stop the save and show as conflicts in the editor's place, yours beside theirs (a description's under
  who changed it and when): `m`, `t` and `b` (or a click) take yours, theirs or both, **Yours for the rest** (`esc`)
  takes yours where nothing's picked, and **Use the result** (`enter`) puts the merged text in the editor to look over
  and save. `kb` changing the text again while they're picked merges them again. It stopped at any change, for saving
  again to replace it all.
- A save that stops after **Tell the agent?** was answered asks it again with the note typed.
- The edit form shows under the title the one `kb` gave while you changed it too, who gave it and when; **Use it**
  (`ctrl+t`) takes it.

## [0.6.1] - 2026-10-16

### Fixed

- Pasting a text with line breaks into a one-line field (a title, tags, a search) joins its lines with one space, as
  `kb add` does: a blank line or a line's spaces at a break left two or more. A break at either end of the paste still
  parts it from the text beside it.
- Deleting a comment in the kanban app deletes the one it asked about, wherever it is by then: one deleted before it
  meanwhile (another window, a pull) made it delete the next. If that comment was edited or deleted meanwhile,
  nothing is deleted, and the status bar says so.
- A save that stops over what `kb` changed, right after the app said what changed on disk, says it once: what else
  that read found stays (`#1's description changed meanwhile: … · #1's status changed on disk`).
- A one-line field opened again starts at its start, not scrolled where it was left the last time.
- fish offers `--tag` only after `add`, `list` and `report`; it came after `kb tag ID -` too.
- `kb hook edit` refuses an agent's write to a central `.md` file `kb` can't read, which it reports as a broken entry,
  and reads no file of the central brain but a top-level `.md` it's asked about.

## [0.6.0] - 2026-10-16

### Added

- Entries log who changed their title, tags, related entries or description, and when, in a new `## Activity`
  section of their file (`Task::activity`, `model::Activity`, `Changed`): a title with what it was and became, tags
  and related entries as what was added and removed (`+ux -bug`, `+5 -shop-api#3`), a description as changed. A
  relation is logged on both entries; one with another repository's entry, on the entry whose file keeps the link.
  Deleting or moving an entry logs nothing.
- The kanban app's details list each change among the comments, by time: `An agent changed the title from "Fix
  login" to "Fix login 500"`, `You added the tag #prod`.
- `kb show` lists the latest 10 changes with the comments (`Comments and changes`, `(latest 10 of N changes)`);
  `--json` gives them all in `activity` (`changed`, `from`, `to`, `added`, `removed`, `author`).
- `kb tag OLD NEW` renames a tag in every entry that has it, giving the new tag the old one's color, and lists them
  (`--json`: their summaries). A tag named as an entry's id is written `#5`: `kb tag 5 ui` is refused, saying to
  write `kb tag 5 +ui` to tag entry 5 or `kb tag '#5' ui` to rename the tag.
- `e2e/update.sh`, run by CI's upgrade job: every released `kb update` installs this build, served as the latest
  release by a stand-in `curl`, and this build reads the brains it wrote.

### Changed

- **Breaking:** for `kanbrain-core`'s Rust API only, `model::Task` has an `activity` field, and `Task`, `Comment`,
  `StatusChange`, `Session`, `Unknown`, `Activity`, `Changed`, `api::ApiError` and the JSON shapes in `api::json`
  are `#[non_exhaustive]`, so a field or variant added later isn't breaking. Outside the crate, build them with
  `Task::new`, `Comment::new` and `Activity::new`, change entries with `Task::set_title`, `set_tags`,
  `set_description`, `relate` and `log` (which log), and give a `match` on `ApiError` a `_` arm. `kb`, the app and
  the data need nothing migrated: the data format stays 1, a file without `## Activity` reads as one with nothing
  logged, and every released version keeps the section, writing it back, on the same brain
  ([ADR 3](docs/decisions/0003-entries-log-their-changes-in-an-activity-section.md)).

## [0.5.2] - 2026-10-16

### Added

- Claude Code hook `kb hook tool` (`PreToolUse`, every tool; `claude::HOOKS`, `hook::tell_of_comments`): an agent
  working on a task you commented on since its last comment there is told, with each step, to `kb show` it and
  `kb comment` what it'll do about it, until it does. Context, never a block; it only reads the brain (about a
  millisecond; 6 ms with 500 entries).
- The kanban app asks **Tell the agent?** when saving the form, the description or a comment of a task an agent has
  in doing (`Mode::Tell`, `Telling`, `Board::tell_agent`): `enter` saves and adds your comment saying what changed,
  with an optional note (`Changed the title and tags: …`); `ctrl+enter` just saves; `esc` goes back. One undo takes
  both back.
- `kb context` brings a project's Claude Code hooks up to date where kanbrain's are already there
  (`claude::update_hooks`), so a release's new hook reaches every project with its next session.

### Changed

- `kb init`, `kb deinit` and `kb context` take a hook that runs `kb` by its path or with variables set before it
  (`KANBRAIN_DIR=… kb context`) as kanbrain's, instead of adding it again; one that runs `kb` after another command
  (`cd x && kb …`) is left as the user's.
- `kb hook stop` reminds an agent of a task it started and left uncommented even after the user commented on it:
  only the agent's own comments say where it stands (`hook::uncommented`).

## [0.5.1] - 2026-10-16

### Added

- The kanban app's edit form and text editor say, as soon as it happens, that `kb` changed what they edit while open:
  their title reads `· changed on disk` until closed, and the status bar says what (`#1's title and status changed on
  disk`). The form counts its entry's title, tags, related entries and status; the editor its text (a comment moved
  to its place included), the task's status and, rejecting or accepting, the decision's verdict, not new comments
  (`Board::after_load`, `Edit::changed_on_disk`, `Form::changed_on_disk`). What a read finds (this, or a file
  skipped) is said once, so the key that read for doesn't replace it: its message goes beside it (`#1 saved · #1's
  status changed on disk`; `Board::notify_found`).
- Opening the edit form, the description or a comment of a task an agent has in doing says so in the status bar
  (`#1 is being worked on by an agent`): what's saved may cross what it writes next. It only warns; a new comment, how
  to tell the agent, doesn't (`Board::say_agent_working`).

### Changed

- The section `kb init` writes in `CLAUDE.md`/`AGENTS.md` (`init::SECTION`) tells agents to give a pull request,
  issue, CI run, commit or page named in a comment or body its link (`[PR #39](URL)`, not just `PR #39`), so it opens
  from the board; `kb update` brings it to every project (`init --all`), `kb init` to one.

### Fixed

- A dialog the kanban app closes because its entry is gone names it with its repository when several are shown, and
  says it may have moved: `closed: shop/api #1 is gone, deleted or moved meanwhile` (it said `#1 was deleted
  meanwhile`, also after a `kb move`).
- The kanban app's text editor doesn't replace unseen what `kb` wrote while it was open: a description (an agent's
  `kb describe`), comment, project description or repository role that changed meanwhile stops the save, the editor
  left open with the text typed and the status bar saying so (`… changed meanwhile: save again to replace it with
  yours, ctrl+d to keep it`); saved again with nothing newer there, it replaces it (`Edit::base`). It replaced it,
  unsaid, even with nothing typed but put back. A comment isn't saved over another that a deletion meanwhile put at
  its place, though written in the same second with the same text: with the entry's other comments not as the
  editor found them (but for new ones after them), the save stops for good (`… comments changed around this one
  meanwhile`; `Edit::comments`, `Board::same_comment`), and the editor's title names the comment it opened. The edit
  form does the same for a title both changed (`… title changed meanwhile, to “…”`), which v0.5.0 replaced.
- The kanban app's reject and accept-with-a-comment editor doesn't overturn unseen a verdict `kb` gave while it was
  open: it flipped a decision an agent's `kb accept` accepted meanwhile to rejected, saying only `#N rejected`. The save
  stops and says so (`#N was accepted meanwhile: save again to reject it anyway, ctrl+d to leave it`); given the
  verdict being written, it stops for good (`… copy your reason, then ctrl+d`), where it dropped the reason or
  comment unsaid while saying `#N rejected` (`Edit::decision`).

## [0.5.0] - 2026-10-15

### Added

- `kb tag ID +TAG|-TAG…` adds tags to an entry or removes them, in order, as the kanban app's form saves them
  (`Brain::update`, tag colors included); tags already so write nothing and say `nothing changed` on stderr
  (`cli::entries::retag`). With `--json`, and tab completion of `+TAG` and `-TAG` in bash, zsh and fish. The agents'
  section names it (`init --all`).
- `kb rename ID TITLE…` replaces an entry's title, as the kanban app's form saves it (`Brain::update`: on one line,
  its file renamed after it); the same title writes nothing and says `nothing changed` on stderr
  (`cli::entries::retitle`). With `--json`, `--` before a title that starts with `--`, and tab completion of the id.
  The agents' section names it (`init --all`).

### Changed

- **Breaking:** for `kanbrain-core`'s Rust API only, what the command line parses to is `#[non_exhaustive]`:
  `cli::Command`, `cli::EntryCommand`, `cli::ProjectChange`, `cli::ProjectPart`, `cli::TagChange` and `cli::Text`
  and each of their variants, `cli::EntryArgs`, `cli::AddArgs`, `cli::ListArgs` and `cli::ReportArgs`. A new
  command, or a new option of one, is no longer a breaking change of the crate; outside it, a `match` on them needs
  `_` and `..`, and only the crate builds them. `kb`, the kanban app and the data are unchanged, and nothing needs
  migrating.

### Fixed

- Fish's tab completion goes by the command, the first word after `kb` that isn't an option, as bash's and zsh's do
  (`__kb_words`, `__kb_using` in `kb completions fish`): it went by any word on the line, so `kb project rename `,
  `kb list todo ` or `kb add Fix the show ` offered ids, `kb project describe ` the project's words again, and
  `kb --project central ` no command. `kb report ` offers the periods, which were offered as `--save`'s value.
- The kanban app's edit form saves only what was changed in it, over the entry as it is then (`Form::opened`,
  `form::merged`): a title, tags or related entries that `kb` changed while it was open (`kb rename`, `kb tag`,
  `kb link`) were written back as the form had found them, even saved untouched. Tags and related entries edited
  in it keep those added elsewhere meanwhile, without those taken out; an edited title replaces the one `kb` gave
  it.
- A lone carriage return (`\r`, as `kb add "$(printf 'a\rb')"` gives) is a line break in a title, as markdown
  reads it: it becomes a space like a line feed does, typed (`model::one_line`), pasted into the kanban app
  (`LineInput::insert`, which dropped it) or already in a file (`Task::from_markdown`). It was kept, so the file's
  `# ` line read as two lines to markdown, and `kb list` printed it raw. A project name with one is refused, as
  with a line feed (`projects::check_name`); `kb init --all`, which `kb update` runs, renames one an older version
  registered, its `\r` made a `-` (`cli::init::rename_unreachable_projects`).

## [0.4.2] - 2026-10-15

### Added

- A fourth Claude Code hook, `kb hook edit` (`PreToolUse` on `Edit|Write|MultiEdit`, `cli::claude::HOOKS`): an agent
  about to write a brain's file, anything in a project's `.brain/` or kanbrain's own in the central brain (its
  entries, `tags.md`, `projects.md`, `last-id`, `format`, `projects/`, `reports/`; not a notes vault's other notes),
  also through a link, `~` or `..`, is refused (`permissionDecision: deny`) and told to make the change with `kb`, or
  tell the user what to change (`cli::hook::refuse_brain_write`). Writing the files itself skipped the brain's lock,
  the numbering of new entries and the format. `kb update` adds it to every project (`init --all`).

### Fixed

- A one-line field of the kanban app (a title, a search box, a name) scrolls sideways to keep the cursor in view: a
  line longer than the field was typed past its edge, unseen (`caret::keep_in_view`, `widgets::FieldScrolls`).
- Pasting into or cutting from the kanban app's form's tags field highlights its list as typing does
  (`Board::retag`, `Board::tag_highlight`): a highlight left from before could be picked by enter instead of what
  was pasted.
- The kanban app's form keeps its tags list's highlight on the same tag when the list reloads (`Form::tag_sel` holds
  the tag, not its row), and a click puts the tag clicked (`Board::choose_form_tag`): a tag another program added
  ahead of it moved the rows, and enter or the click picked another one.
- The kanban app holds the lock of a brain it finds while it acts, as of every other: a project registered
  meanwhile was read, and could be written to, without it, so a `kb` run then could write over it or be written
  over. Its lock is taken after catching up, and every brain read again under the locks
  (`Board::lock_found_brains`).
- `f` acts on the kanban app's board only: on the notes or decisions it showed every done task, or only the latest,
  on the board, unseen (`keys::ACTIONS`, `Tabs::Board`; `When::OnBoard` is gone, as nothing checked it on a key).

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

[Unreleased]: https://github.com/singudotdev/kanbrain/compare/v0.6.2...HEAD
[0.6.2]: https://github.com/singudotdev/kanbrain/compare/v0.6.1...v0.6.2
[0.6.1]: https://github.com/singudotdev/kanbrain/compare/v0.6.0...v0.6.1
[0.6.0]: https://github.com/singudotdev/kanbrain/compare/v0.5.2...v0.6.0
[0.5.2]: https://github.com/singudotdev/kanbrain/compare/v0.5.1...v0.5.2
[0.5.1]: https://github.com/singudotdev/kanbrain/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/singudotdev/kanbrain/compare/v0.4.2...v0.5.0
[0.4.2]: https://github.com/singudotdev/kanbrain/compare/v0.4.1...v0.4.2
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
