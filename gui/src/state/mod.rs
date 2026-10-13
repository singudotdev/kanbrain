//! The window's state: the brains, which tab, scope and entry are shown, what's being typed, and
//! every action the keys and buttons run. Plain Rust with no GPUI, so it's tested without a window; `ui` only draws
//! it and turns clicks and keystrokes into these calls.
//!
//! - [`mode`]: the dialogs and fields that take the keys.
//! - [`actions`]: changes to entries, through the brain, with a message for the status bar.
//! - [`input`]: what each key does in each dialog, and typing into fields.
//! - [`keys`]: the keys outside dialogs, which the palette and the right-click menu offer too.
//! - [`manage`]: renaming, moving and removing projects and repositories.
//! - [`nav`]: moving the selection around the board.
//! - [`palette`]: the command palette, every action with its key.
//! - [`sources`]: the central brain and every project repository, and the scope that filters them.

mod actions;
mod input;
pub(crate) mod keys;
mod manage;
mod menu;
pub mod mode;
mod nav;
mod palette;
mod sources;
#[cfg(test)]
#[path = "../../tests/state.rs"]
mod tests;

pub use keys::{Action, When};
pub use menu::MenuPicker;
pub use mode::{
    AddProject, Confirm, Edit, Field, Form, FormTarget, LinkPicker, Manage, Mode, Picker, PickerKey, ProjectOption,
    QuickAdd, Target, WhereFor, WherePicker,
};
pub use sources::{Key, Scope, Source};

use crate::text::editor::LineInput;
use anyhow::Result;
use kanbrain_core::brain::{CalendarView, ReportView};
use kanbrain_core::model::{Kind, Status, Task};
use kanbrain_core::period::{Period, Unit};
use kanbrain_core::projects;
use kanbrain_core::store::Change;
use std::path::{Path, PathBuf};
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

/// What the board asks of the window, which can run programs and open the system's dialogs.
#[derive(Debug, PartialEq, Eq)]
pub enum Request {
    /// Open the file in `$EDITOR`, and re-read the files when it exits.
    Edit(PathBuf),
    /// Open the system's folder dialog, for a project to add, and hand the folder to [`Board::name_project`],
    /// with the project it joins when it's added from that project's info.
    Folder(Option<String>),
    /// Install the newer release and start it in place of this app; see `crate::update`.
    Update,
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
    /// Status bar message and when it was set; it expires after `MESSAGE_TTL`.
    message: Option<(String, Instant)>,
    /// The files the last change wrote, as they were before and after, for `u` to put back.
    last_change: Vec<Change>,
}

impl Board {
    /// The central brain at `central` and every registered project, with the first card selected.
    /// Opened in a project's `.brain/` (`opened_in`), it shows that project, or that repository of a
    /// project with several.
    pub fn open(central: &Path, opened_in: Option<PathBuf>) -> Result<Self> {
        let sources = sources::load(central, opened_in.as_deref())?;
        let mut board = Self {
            sources,
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
            last_change: Vec::new(),
        };
        if let Some(src) = board.opened_in.as_ref().and_then(|dir| board.src_at(dir)) {
            let project = board.sources[src].project.clone().unwrap_or_default();
            let repos = board.repos_of(&project).count();
            board.scope = if repos > 1 { Scope::Repo(src) } else { Scope::Project(project) };
        }
        board.after_load();
        Ok(board)
    }

    /// The index of the brain whose folder is `dir`.
    fn src_at(&self, dir: &Path) -> Option<usize> {
        self.sources.iter().position(|s| projects::same_dir(s.brain.store.dir(), dir))
    }

    /// Shows `msg` in the status bar for a few seconds.
    pub fn notify(&mut self, msg: impl Into<String>) {
        self.message = Some((msg.into(), Instant::now()));
    }

    /// The status bar message, while it hasn't expired.
    pub fn message(&self) -> Option<&str> {
        self.message.as_ref().filter(|(_, at)| at.elapsed() < MESSAGE_TTL).map(|(msg, _)| msg.as_str())
    }

    /// Re-reads every brain, and the list of projects, which may have gained or lost one. The
    /// selection, a one-repository scope and an open dialog stay on the same folders, wherever they now are in
    /// the list; a dialog on a folder no longer there closes.
    pub fn reload(&mut self) -> Result<()> {
        let before: Vec<PathBuf> = self.sources.iter().map(|s| s.brain.store.dir().to_path_buf()).collect();
        self.sources = sources::load(&self.central, self.opened_in.as_deref())?;
        let now_at: Vec<Option<usize>> = before.iter().map(|dir| self.src_at(dir)).collect();
        let to = |src: usize| now_at.get(src).copied().flatten();
        self.selected = self.selected.and_then(|k| Some(Key::new(to(k.src)?, k.id)));
        if let Scope::Repo(src) = self.scope {
            self.scope = to(src).map_or(Scope::All, Scope::Repo);
        }
        let open = !matches!(self.mode, Mode::Normal);
        self.mode = std::mem::replace(&mut self.mode, Mode::Normal).remap(to).unwrap_or(Mode::Normal);
        if open && matches!(self.mode, Mode::Normal) {
            self.notify("closed: its project is gone");
        }
        self.after_load();
        Ok(())
    }

    /// Re-reads the brains whose folders are among `dirs`, those another program changed, and the other
    /// repositories of their projects, rather than all of them: an agent's every command changes one, and
    /// the window may show many. Everything is
    /// re-read when the list of projects or their shared cards may have changed, or a folder is gone.
    pub fn reload_changed(&mut self, dirs: &[PathBuf]) -> Result<()> {
        let cards = self.central.join("projects");
        let everything =
            dirs.iter().any(|d| *d == cards || !d.is_dir()) || (dirs.contains(&self.central) && !self.same_repos()?);
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

    /// Reports skipped files and puts the selection somewhere visible.
    fn after_load(&mut self) {
        let warnings: Vec<&String> = self.sources.iter().flat_map(|s| &s.brain.warnings).collect();
        if !warnings.is_empty() {
            let list = warnings.iter().map(|w| w.as_str()).collect::<Vec<_>>().join("; ");
            self.notify(format!("skipped: {list}"));
        }
        self.keep_selection();
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
        self.all_in_column(Status::Done).len() - self.column(Status::Done).len()
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
        let related = self.related_keys(key);
        let mut candidates: Vec<(Key, &Task)> =
            self.linkable(key.src).filter(|&(k, t)| k != key && t.matches(picker.search.text())).collect();
        candidates.sort_by_key(|&(k, t)| (!related.contains(&k), k.src != key.src, std::cmp::Reverse(t.id)));
        candidates
    }
}
