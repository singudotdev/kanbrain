//! The window's state: the brains, which tab, scope and entry are shown, what's being typed, and
//! every action the keys and buttons run. Plain Rust with no GPUI, so it's tested without a window; `ui` only draws
//! it and turns clicks and keystrokes into these calls.
//!
//! - [`mode`]: the dialogs and fields that take the keys.
//! - [`actions`]: changes to entries, through the brain, with a message for the status bar.
//! - [`form`]: the new / edit form and the quick-add field.
//! - [`decisions`]: the questions and decisions waiting on you, and answering them.
//! - [`input`]: what each key does in each dialog, and typing into fields.
//! - [`keys`]: the keys outside dialogs, which the palette and the right-click menu offer too.
//! - [`manage`]: renaming, moving and removing projects and repositories.
//! - [`menu`]: the right-click menu on an entry.
//! - [`nav`]: moving the selection around the board.
//! - [`palette`]: the command palette, every action with its key.
//! - [`shown`]: text shown rather than typed into, selected with the mouse to copy.
//! - [`sources`]: the central brain and every project repository, and the scope that filters them.

mod actions;
mod decisions;
mod form;
mod input;
pub(crate) mod keys;
mod manage;
mod menu;
pub mod mode;
mod nav;
mod palette;
mod shown;
mod sources;
#[cfg(test)]
#[path = "../../tests/state/mod.rs"]
mod tests;

pub use keys::{Action, When};
pub use menu::MenuPicker;
pub use mode::{
    AddProject, CommentPicker, Confirm, Edit, Field, Form, FormTarget, LinkPicker, Manage, Mode, Pending, Picker,
    PickerKey, ProjectOption, QuickAdd, Target, Tell, Telling, WhereFor, WherePicker,
};
pub use shown::Shown;
pub use sources::{Key, Scope, Source};

use crate::text::editor::LineInput;
use crate::text::merge::{self, Merged};
use crate::watch;
use anyhow::Result;
use kanbrain_core::brain::{CalendarView, ReportView};
use kanbrain_core::model::{Kind, Status, Task};
use kanbrain_core::period::{Period, Unit};
use kanbrain_core::projects;
use kanbrain_core::store::Change;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

/// How long a status bar message shows.
const MESSAGE_TTL: Duration = Duration::from_secs(5);

/// The board's columns, left to right, in the order a task usually moves through them.
pub const COLUMNS: [Status; 4] = [Status::Todo, Status::Doing, Status::Parked, Status::Done];

/// Done tasks shown before "Show all": the most recently finished.
pub const DONE_LIMIT: usize = 20;

/// How the board tab lays its tasks out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Layout {
    /// A column per status, with cards dragged between them.
    #[default]
    Kanban,
    /// One list with a section per status: denser, and long titles fit on one line.
    List,
}

/// The window's tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    /// Tasks by status: kanban columns, or a list (see [`Layout`]).
    #[default]
    Board,
    /// Notes: what was learned.
    Notes,
    /// Decisions: what was chosen, and why.
    Decisions,
    /// Tracked time by task or tag.
    Report,
    /// Each day's interactions: time worked, moves, comments, new entries.
    Calendar,
}

impl Tab {
    /// Every tab, in the order they're shown and numbered (`1`…`5`).
    pub const ALL: [Self; 5] = [Self::Board, Self::Notes, Self::Decisions, Self::Report, Self::Calendar];

    /// The tab's name.
    pub fn label(self) -> &'static str {
        match self {
            Self::Board => "Board",
            Self::Notes => "Notes",
            Self::Decisions => "Decisions",
            Self::Report => "Report",
            Self::Calendar => "Calendar",
        }
    }

    /// The kind of entry the tab lists; the report and the calendar list none.
    pub fn kind(self) -> Option<Kind> {
        match self {
            Self::Board => Some(Kind::Task),
            Self::Notes => Some(Kind::Note),
            Self::Decisions => Some(Kind::Decision),
            Self::Report | Self::Calendar => None,
        }
    }

    /// The tab listing entries of `kind`.
    fn of(kind: Kind) -> Self {
        match kind {
            Kind::Task => Self::Board,
            Kind::Note => Self::Notes,
            Kind::Decision => Self::Decisions,
        }
    }
}

/// How a message saying what a read skipped starts.
const SKIPPED: &str = "skipped: ";

/// What the board asks of the window, which can run programs and open the system's dialogs.
#[derive(Debug, PartialEq, Eq)]
pub enum Request {
    /// Open the file in `$EDITOR`, and re-read the files when it exits.
    Edit(PathBuf),
    /// Open the system's folder dialog, for a project to add, and hand the folder to [`Board::name_project`],
    /// with the repository (its `.brain/`) whose Manage project it's added from, whose project it joins: by folder,
    /// as the board may be read again while the dialog is open, which can shift every brain's place.
    Folder(Option<PathBuf>),
    /// Install the newer release and start it in place of this app; see `crate::update`.
    Update,
}

/// What `u` does next: undo the last change, or redo it, having just undone it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NextUndo {
    /// Put the files back as they were before it.
    Undo,
    /// Write them again as it left them.
    Redo,
}

/// Where the board reads the time its messages are said and read at: the window's, whose tests move it by hand, as a
/// loaded machine can stall a thread for seconds between a key and what checks its message.
#[derive(Clone)]
pub struct Clock(Rc<dyn Fn() -> Instant>);

impl Clock {
    /// A clock reading `now`, as the window's executor gives it.
    pub fn new(now: impl Fn() -> Instant + 'static) -> Self {
        Self(Rc::new(now))
    }

    /// The time it reads now.
    fn now(&self) -> Instant {
        (self.0)()
    }
}

impl Default for Clock {
    fn default() -> Self {
        Self::new(Instant::now)
    }
}

impl std::fmt::Debug for Clock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Clock")
    }
}

/// Who said the status bar's message, for what's said next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Said {
    /// An action: what's said next replaces it.
    Action,
    /// A read of the brains, in the input still running: said only once, so that input's action goes beside it,
    /// however long it takes to write.
    Found,
    /// A read whose input is over, or that no input made (opening, the window's timer): the next action's message goes
    /// beside it until it was shown for `watch::INTERVAL`, as a save right after the timer read would close the form
    /// marked with it before it could be read; replaced after.
    FoundBefore,
}

/// A status bar message: what each said, the newest first, and them together as the bar shows them.
#[derive(Debug)]
struct Message {
    /// What each said: an action's, and what a read found that it goes beside.
    parts: Vec<String>,
    /// The parts as shown, one after the other.
    shown: String,
    /// When it was said: it expires after `MESSAGE_TTL`.
    at: Instant,
    /// Who said it.
    by: Said,
}

impl Message {
    /// `parts`, said at `at` by `by`.
    fn new(parts: Vec<String>, at: Instant, by: Said) -> Self {
        Self { shown: parts.join(" · "), parts, at, by }
    }

    /// Whether it's what a read found that what's said at `now` goes beside: see [`Said`].
    fn found_at(&self, now: Instant) -> bool {
        match self.by {
            Said::Found => true,
            Said::FoundBefore => now.saturating_duration_since(self.at) < watch::INTERVAL,
            Said::Action => false,
        }
    }
}

/// Everything the window shows and remembers.
#[derive(Debug)]
pub struct Board {
    /// The central brain first, then every project repository; see [`sources`].
    pub sources: Vec<Source>,
    /// The central brain's folder, which lists the projects.
    pub central: PathBuf,
    /// The `.brain/` the window was opened in, shown even if it isn't registered.
    opened_in: Option<PathBuf>,
    /// Registered repositories left out because they couldn't be read (see [`sources::load`]).
    skipped: Vec<sources::Skipped>,
    /// What was skipped when last said, so each read says only what's newly skipped.
    warned: Vec<String>,
    /// Which brains are shown.
    pub scope: Scope,
    /// The tab shown.
    pub tab: Tab,
    /// How the board tab lays its tasks out.
    pub layout: Layout,
    /// The selected entry, by brain and id so it stays selected when entries move or files reload.
    pub selected: Option<Key>,
    /// Whether the details drawer shows the selected entry.
    pub drawer: bool,
    /// Filters the tab shown: words and `#tags`, typed or picked from the tag dropdown; see
    /// [`Task::matches`]. Each tab has its own.
    pub search: LineInput,
    /// The other tabs' searches, kept while they're not shown, by `Tab::ALL` index.
    searches: [LineInput; Tab::ALL.len()],
    /// The dialog or field taking the keys.
    pub mode: Mode,
    /// Whether the Done column shows every done task, not only the latest `DONE_LIMIT`.
    pub all_done: bool,
    /// The report tab's period, grouping and tag filter.
    pub report: ReportView,
    /// The calendar tab's period and filters.
    pub calendar: CalendarView,
    /// Set to close the window.
    pub quit: bool,
    /// Set when the window should do something only it can; it takes it and does it.
    pub request: Option<Request>,
    /// A newer release's version, once the window found one out.
    pub available: Option<String>,
    /// Status bar message.
    message: Option<Message>,
    /// The time messages are said and read at.
    pub clock: Clock,
    /// The files the last change wrote, as they were before and after, for `u` to put back.
    last_change: Vec<Change>,
    /// What `u` does next to `last_change`.
    next_undo: NextUndo,
    /// The text shown, not typed into, that the mouse selected in, if any.
    shown: Option<Shown>,
}

impl Board {
    /// The central brain at `central` and every registered project, with the first card selected.
    /// Opened in a project's `.brain/` (`opened_in`), it shows that project, or that repository of a
    /// project with several.
    pub fn open(central: &Path, opened_in: Option<PathBuf>) -> Result<Self> {
        let (sources, skipped) = sources::load(central, opened_in.as_deref())?;
        let mut board = Self {
            sources,
            skipped,
            warned: Vec::new(),
            central: central.to_path_buf(),
            opened_in,
            scope: Scope::All,
            tab: Tab::default(),
            layout: Layout::default(),
            selected: None,
            drawer: false,
            search: LineInput::default(),
            searches: Default::default(),
            mode: Mode::Normal,
            all_done: false,
            report: ReportView::new(Period::current(Unit::Week)),
            calendar: CalendarView::new(Period::current(Unit::Week)),
            quit: false,
            request: None,
            available: None,
            message: None,
            clock: Clock::default(),
            last_change: Vec::new(),
            next_undo: NextUndo::Undo,
            shown: None,
        };
        if let Some(src) = board.opened_in.as_ref().and_then(|dir| board.src_at(dir)) {
            let project = board.project_name(src).to_string();
            let repos = board.repos_of(&project).count();
            board.scope = if repos > 1 { Scope::Repo(src) } else { Scope::Project(project) };
        }
        board.after_load();
        // Opening isn't an input: what it found shows, and the first key's message replaces it.
        board.input_done();
        Ok(board)
    }

    /// The index of the brain whose folder is `dir`.
    fn src_at(&self, dir: &Path) -> Option<usize> {
        self.sources.iter().position(|s| projects::same_dir(s.brain.store.dir(), dir))
    }

    /// Shows `msg` in the status bar for a few seconds.
    pub fn notify(&mut self, msg: impl Into<String>) {
        self.say(&msg.into(), Said::Action);
    }

    /// Shows what a read of the brains found (a file skipped, `kb` changing what a dialog edits), which the action
    /// that read for, said next, doesn't replace: it's said only once.
    fn notify_found(&mut self, msg: impl Into<String>) {
        self.say(&msg.into(), Said::Found);
    }

    /// Shows `msg`, after what a read found just before, if that's what's shown.
    fn say(&mut self, msg: &str, by: Said) {
        // Plain text: the command line's messages, which these often are, quote names in markdown's backticks.
        let msg = msg.replace('`', "");
        let now = self.clock.now();
        let found = self.message.take().filter(|said| said.found_at(now) && said.shown != msg).map(|said| said.parts);
        let parts = std::iter::once(msg).chain(found.into_iter().flatten()).collect();
        self.message = Some(Message::new(parts, now, by));
    }

    /// Ends the input the window ran, from the read before it to its action: what that read found, its action had
    /// its turn to go beside, and what's said next replaces it.
    pub fn input_done(&mut self) {
        if let Some(said) = &mut self.message
            && said.by == Said::Found
        {
            said.by = Said::FoundBefore;
        }
    }

    /// Puts `rest` in the status bar in place of `found`, what a read just said there: a save that stops over part
    /// of it is about to say that part, with what to do, beside the rest. One that what follows doesn't go beside is
    /// replaced by it anyway.
    fn unsay_found(&mut self, found: Option<String>, rest: Option<String>) {
        let (Some(found), Some(said)) = (found, &mut self.message) else { return };
        if said.by == Said::Action {
            return;
        }
        // As `say` keeps it.
        let (found, rest) = (found.replace('`', ""), rest.map(|rest| rest.replace('`', "")));
        let parts: Vec<String> = std::mem::take(&mut said.parts)
            .into_iter()
            .filter_map(|part| if part == found { rest.clone() } else { Some(part) })
            .collect();
        self.message = (!parts.is_empty()).then(|| Message::new(parts, said.at, said.by));
    }

    /// Shows what went wrong in the status bar, with its causes.
    pub fn notify_error(&mut self, e: &anyhow::Error) {
        self.notify(format!("{e:#}"));
    }

    /// The status bar message, while it hasn't expired.
    pub fn message(&self) -> Option<&str> {
        self.message_at(self.clock.now())
    }

    /// The status bar message at `now`, while it hasn't expired by then.
    pub fn message_at(&self, now: Instant) -> Option<&str> {
        self.message
            .as_ref()
            .filter(|said| now.saturating_duration_since(said.at) < MESSAGE_TTL)
            .map(|said| said.shown.as_str())
    }

    /// Re-reads every brain, and the list of projects, which may have gained or lost one. The
    /// selection, a one-repository scope and an open dialog stay on the same folders, wherever they now are in
    /// the list; a dialog on a folder no longer there closes.
    pub fn reload(&mut self) -> Result<()> {
        let before: Vec<PathBuf> = self.sources.iter().map(|s| s.brain.store.dir().to_path_buf()).collect();
        let shown: Vec<usize> = match &self.scope {
            Scope::Project(name) => self.repos_of(name).collect(),
            _ => Vec::new(),
        };
        (self.sources, self.skipped) = sources::load(&self.central, self.opened_in.as_deref())?;
        let now_at: Vec<Option<usize>> = before.iter().map(|dir| self.src_at(dir)).collect();
        let to = |src: usize| now_at.get(src).copied().flatten();
        self.selected = self.selected.and_then(|k| Some(Key::new(to(k.src)?, k.id)));
        if let Scope::Repo(src) = self.scope {
            self.scope = to(src).map_or(Scope::All, Scope::Repo);
        }
        // A project shown that's gone from the command line: under its new name when its repositories are now all of
        // another's, as after a rename or a move of its only repository to a new project; else all of them.
        if let Scope::Project(name) = &self.scope
            && self.repos_of(name).next().is_none()
        {
            let mut now: Vec<usize> = shown.iter().filter_map(|&src| to(src)).collect();
            now.sort_unstable();
            let renamed = now
                .first()
                .and_then(|&src| self.sources[src].project.clone())
                .filter(|project| now.len() == shown.len() && self.repos_of(project).eq(now.iter().copied()));
            self.scope = renamed.map_or(Scope::All, Scope::Project);
        }
        let open = !matches!(self.mode, Mode::Normal);
        self.mode = std::mem::replace(&mut self.mode, Mode::Normal).remap(to).unwrap_or(Mode::Normal);
        if open && matches!(self.mode, Mode::Normal) {
            self.notify_found("closed: its project is gone");
        }
        self.after_load();
        Ok(())
    }

    /// Re-reads the brains whose folders are among `dirs`, those another program changed, and the other
    /// repositories of their projects, rather than all of them: an agent's every command changes one, and
    /// the window may show many. Everything is re-read when the list of projects or their shared cards may
    /// have changed, a repository was renamed, or a folder is gone.
    pub fn reload_changed(&mut self, dirs: &[PathBuf]) -> Result<()> {
        let cards = projects::cards_dir(&self.central);
        let left_out = |d: &PathBuf| self.skipped.iter().any(|s| s.dir == *d);
        let everything = dirs.iter().any(|d| *d == cards || !d.is_dir() || left_out(d)) || !self.same_repos()?;
        if everything {
            return self.reload();
        }
        // A project's other repositories too: an entry deleted in one drops the links the others have to it.
        let changed = |s: &Source| dirs.iter().any(|d| d == s.brain.store.dir());
        let projects: Vec<String> =
            self.sources.iter().filter(|s| changed(s)).filter_map(|s| s.project.clone()).collect();
        for source in &mut self.sources {
            if changed(source) || source.project.as_ref().is_some_and(|p| projects.contains(p)) {
                source.brain.reload()?;
            }
        }
        self.after_load();
        Ok(())
    }

    /// Reports what is newly skipped, files and repositories left out, and puts the selection somewhere visible.
    /// Only what wasn't said already: an agent's every write reads a brain again, and the window reads everything
    /// once more a second after it opens.
    fn after_load(&mut self) {
        // A dialog on an entry another program deleted: drawn as nothing, it would still take every key.
        if let Some(key) = self.mode.entry()
            && self.task(key).is_none()
        {
            self.mode = Mode::Normal;
            // A `kb move` takes it out of this brain as a delete does.
            self.notify_found(format!("closed: {} is gone, deleted or moved meanwhile", self.entry_name(key)));
        }
        self.mark_changed_on_disk();
        let left_out = self.skipped.iter().map(|s| s.said.clone());
        // Named with their brain: two repositories may skip files of one name.
        let files = self.sources.iter().flat_map(|s| {
            s.brain.warnings.iter().map(move |w| s.label.as_ref().map_or_else(|| w.clone(), |l| format!("{l}: {w}")))
        });
        let now: Vec<String> = left_out.chain(files).collect();
        let new: Vec<&str> = now.iter().filter(|w| !self.warned.contains(w)).map(String::as_str).collect();
        if !new.is_empty() {
            self.notify_found(format!("{SKIPPED}{}", new.join("; ")));
        }
        self.warned = now;
        self.keep_selection();
    }

    /// Marks the open edit form or editor once `kb` changed what it edits, and says what, once: the save that follows
    /// is the user's call, made knowing it. One waiting for [`Tell`]'s answer too: the agent may finish the task then.
    fn mark_changed_on_disk(&mut self) {
        self.refresh_theirs_title();
        self.merge_again();
        let said = match &self.mode {
            Mode::Edit(edit) => self.edit_mark(edit),
            Mode::Form(form) => self.form_mark(form),
            Mode::Tell(tell) => match &tell.pending {
                Pending::Edit(edit) => self.edit_mark(edit),
                Pending::Form(form) => self.form_mark(form),
            },
            _ => None,
        };
        let Some(said) = said else { return };
        match &mut self.mode {
            Mode::Edit(edit) => edit.changed_on_disk = true,
            Mode::Form(form) => form.changed_on_disk = true,
            Mode::Tell(tell) => match &mut tell.pending {
                Pending::Edit(edit) => edit.changed_on_disk = true,
                Pending::Form(form) => form.changed_on_disk = true,
            },
            // Only an edit or a form says what changed.
            _ => return,
        }
        self.notify_found(said);
    }

    /// Shows in the form the title `kb` gives now, in place of the one a save stopped over, as the one saving again
    /// keeps yours over.
    fn refresh_theirs_title(&mut self) {
        let Mode::Form(form) = &self.mode else { return };
        let Some(task) = form.theirs_title.as_ref().and(form.target.key()).and_then(|key| self.task(key)) else {
            return;
        };
        let title = task.title.clone();
        if let Mode::Form(form) = &mut self.mode {
            if let Some(opened) = &mut form.opened {
                opened.title.clone_from(&title);
            }
            form.theirs_title = Some(title);
        }
    }

    /// Merges again the open conflicts of a text `kb` changed again while they were picked, which show its old text
    /// otherwise. Picks made are lost, as the conflicts may not be the same.
    fn merge_again(&mut self) {
        let Mode::Edit(edit) = &self.mode else { return };
        // Another comment at its index isn't merged with, which would put that comment in the editor: the text typed
        // comes back, for the save to stop over (`same_comment`).
        if edit.merge.is_some()
            && let Some(stop) = self.comment_moved(edit)
        {
            if let Mode::Edit(edit) = &mut self.mode {
                edit.merge = None;
            }
            return self.notify_found(stop);
        }
        let (Some(merge), Some(now)) = (&edit.merge, self.text_of(edit.target)) else { return };
        if now == merge.theirs {
            return;
        }
        let (merged, now) = (merge::merge(&edit.base, &edit.editor.text(), now), now.to_string());
        let name = self.text_name(edit.target);
        let Mode::Edit(edit) = &mut self.mode else { return };
        let said = match merged {
            Merged::Clean(text) => {
                // All of theirs is the text as it is: nothing to save.
                let theirs = text == now;
                edit.settle(now, &text);
                if theirs {
                    format!("{name} changed again: it's theirs as it is now, esc closes")
                } else {
                    format!("{name} changed again: it all merges now, esc saves it")
                }
            }
            Merged::Conflicted(conflicts) => {
                edit.merge = Some(conflicts);
                format!("{name} changed again: pick again where you both changed it")
            }
        };
        self.notify_found(said);
    }

    /// What `kb` changed of what `edit` edits, unless it was said already.
    fn edit_mark(&self, edit: &Edit) -> Option<String> {
        if edit.changed_on_disk { None } else { self.edit_changed_on_disk(edit) }
    }

    /// What `kb` changed of what `form` edits, unless it was said already.
    fn form_mark(&self, form: &Form) -> Option<String> {
        if form.changed_on_disk { None } else { self.form_changed_on_disk(form) }
    }

    /// Tasks in a column, filtered by the scope and the search, most recently moved there first; the
    /// Done column stops at `DONE_LIMIT` unless `all_done`.
    pub fn column(&self, status: Status) -> Vec<(Key, &Task)> {
        let limit = if status == Status::Done && !self.all_done { DONE_LIMIT } else { usize::MAX };
        let mut cards = self.all_in_column(status);
        cards.truncate(limit);
        cards
    }

    /// Every task in a column matching the scope and the search, most recently moved there first.
    fn all_in_column(&self, status: Status) -> Vec<(Key, &Task)> {
        let mut cards: Vec<(Key, &Task)> = self
            .scoped()
            .flat_map(|(src, brain)| brain.column(status).into_iter().map(move |t| (Key::new(src, t.id), t)))
            .filter(|(_, t)| self.shows(t))
            .collect();
        // As within one brain (`Brain::column`): among moves at the same time, the newest entry first.
        cards.sort_by_key(|(key, t)| (std::cmp::Reverse(t.status_since()), key.src, std::cmp::Reverse(key.id)));
        cards
    }

    /// Every task in a column in scope, whether or not the search shows it.
    pub fn column_total(&self, status: Status) -> usize {
        self.scoped().map(|(_, brain)| brain.column(status).len()).sum()
    }

    /// Whether the tab's search keeps `task`.
    pub fn shows(&self, task: &Task) -> bool {
        task.matches(self.search.text())
    }

    /// The tags the search filters by: its `#tag` words, without the `#`.
    pub fn search_tags(&self) -> Vec<String> {
        self.search
            .text()
            .split_whitespace()
            .filter_map(|w| w.strip_prefix('#'))
            .filter(|t| !t.is_empty())
            .map(str::to_lowercase)
            .collect()
    }

    /// Done tasks matching the search that the Done column leaves out.
    pub fn hidden_done(&self) -> usize {
        let all = self.all_in_column(Status::Done).len();
        if self.all_done { 0 } else { all.saturating_sub(DONE_LIMIT) }
    }

    /// The notes or decisions of the current tab, filtered by the scope and the search, newest first.
    pub fn entries(&self) -> Vec<(Key, &Task)> {
        let Some(kind) = self.tab.kind() else { return Vec::new() };
        let mut entries: Vec<(Key, &Task)> = self
            .scoped()
            .flat_map(|(src, brain)| brain.tasks.iter().map(move |t| (Key::new(src, t.id), t)))
            .filter(|(_, t)| t.kind == kind && self.shows(t))
            .collect();
        entries.sort_by_key(|(key, t)| (std::cmp::Reverse(t.created), key.src, std::cmp::Reverse(key.id)));
        entries
    }

    /// The entries the current tab shows, in reading order: column by column on the board, by first
    /// interaction on the calendar.
    pub fn visible_ids(&self) -> Vec<Key> {
        match self.tab {
            Tab::Board => COLUMNS.iter().flat_map(|&s| self.column(s)).map(|(key, _)| key).collect(),
            Tab::Notes | Tab::Decisions => self.entries().into_iter().map(|(key, _)| key).collect(),
            Tab::Calendar => {
                let (calendar, srcs) = self.calendar();
                calendar.entries().into_iter().map(|(part, id)| Key::new(srcs[part], id)).collect()
            }
            Tab::Report => Vec::new(),
        }
    }

    /// The selected entry.
    pub fn selected_task(&self) -> Option<&Task> {
        self.selected.and_then(|key| self.task(key))
    }

    /// The selected entry, if the drawer shows it.
    pub fn drawer_task(&self) -> Option<&Task> {
        self.selected_task().filter(|_| self.drawer)
    }

    /// Selects the entry `key` names.
    pub fn select(&mut self, key: Key) {
        if self.task(key).is_some() {
            self.selected = Some(key);
        }
    }

    /// Selects the entry and shows it in the drawer, switching to its tab, and clearing a search or
    /// widening a scope that hides it.
    pub fn open_entry(&mut self, key: Key) {
        let Some(task) = self.task(key) else { return };
        let (tab, done) = (Tab::of(task.kind), task.is_task() && task.status == Status::Done);
        self.show_tab(tab);
        self.all_done |= done;
        if !self.in_scope(key.src) {
            self.scope = Scope::All;
        }
        if !self.visible_ids().contains(&key) {
            self.search.clear();
        }
        self.selected = Some(key);
        self.drawer = true;
    }

    /// Shows another tab with its own search, keeping the current tab's for when it's back:
    /// filtering notes by `#stripe` leaves the board as it was.
    pub(super) fn show_tab(&mut self, tab: Tab) {
        if tab == self.tab {
            return;
        }
        let index = |tab: Tab| Tab::ALL.iter().position(|&t| t == tab).unwrap_or(0);
        let (from, to) = (index(self.tab), index(tab));
        self.searches[from] = std::mem::take(&mut self.search);
        self.search = std::mem::take(&mut self.searches[to]);
        self.tab = tab;
    }

    /// Shows another tab, selecting its first entry unless the selection is there.
    pub fn switch_tab(&mut self, tab: Tab) {
        self.show_tab(tab);
        self.keep_selection();
    }

    /// Lays the board out the other way: kanban columns or a list. Both show the same tasks, so the
    /// selection stays.
    pub fn toggle_layout(&mut self) {
        self.layout = match self.layout {
            Layout::Kanban => Layout::List,
            Layout::List => Layout::Kanban,
        };
    }

    /// Keeps the selection on a visible entry: the same one if it still is, else the first.
    pub fn keep_selection(&mut self) {
        let visible = self.visible_ids();
        if self.selected.is_none_or(|key| !visible.contains(&key)) {
            self.selected = visible.first().copied();
        }
        if self.selected.is_none() {
            self.drawer = false;
        }
    }

    /// The link picker's rows: every other entry of its brain and of its project's other repositories
    /// matching its search, the related ones first, then its brain's, then newest first.
    pub fn link_candidates(&self, picker: &LinkPicker) -> Vec<(Key, &Task)> {
        let key = picker.key;
        self.link_options(key.src, Some(key), picker.rows.search.text(), &self.related_keys(key))
    }

    /// Every entry the brain at `src` can link to but `except`, matching `search`: `linked` first, then the
    /// brain's own, then the newest.
    pub(super) fn link_options(
        &self,
        src: usize,
        except: Option<Key>,
        search: &str,
        linked: &[Key],
    ) -> Vec<(Key, &Task)> {
        let mut options: Vec<(Key, &Task)> =
            self.linkable(src).filter(|&(key, t)| Some(key) != except && t.matches(search)).collect();
        options.sort_by_key(|&(key, t)| (!linked.contains(&key), key.src != src, std::cmp::Reverse(t.id)));
        options
    }
}

/// What of an entry, a project or a repository `kb` may change while a dialog edits it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Changed {
    /// An entry's title.
    Title,
    /// An entry's tags.
    Tags,
    /// The entries related to an entry.
    Related,
    /// A task's status.
    Status,
    /// An entry's description.
    Description,
    /// What else the editor edits: a comment, a verdict's, a project's description or a repository's role.
    Comment,
    /// A decision's verdict being given.
    Verdict,
}

impl Changed {
    /// How the status bar names it.
    fn word(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::Tags => "tags",
            Self::Related => "related entries",
            Self::Status => "status",
            Self::Description => "description",
            Self::Comment => "comment",
            Self::Verdict => "verdict",
        }
    }
}

/// "#1's title and status changed on disk", naming the fields of `name`'s entry that changed; `None` if none did.
fn changed_on_disk(name: &str, fields: &[(bool, Changed)]) -> Option<String> {
    listed(fields).map(|fields| format!("{name}'s {fields} changed on disk"))
}

/// The fields marked changed, as a sentence lists them ("title, tags and status"); `None` if none is.
fn listed(fields: &[(bool, Changed)]) -> Option<String> {
    let changed: Vec<&str> = fields.iter().filter(|(changed, _)| *changed).map(|(_, field)| field.word()).collect();
    let (last, rest) = changed.split_last()?;
    Some(if rest.is_empty() { (*last).to_string() } else { format!("{} and {last}", rest.join(", ")) })
}
