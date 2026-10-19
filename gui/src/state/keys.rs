//! Every key the window takes outside its dialogs, in one table: the key, the tabs it works on, when it's worth
//! offering, what it's called and what it runs. The keys, the command palette and the right-click menu all read it,
//! so none can offer what another doesn't, or call it something else.

use super::nav::Step;
use super::{Board, Confirm, Key, Mode, Picker, Request, Tab, Target};
use crate::text::event::{KeyCode, KeyEvent, KeyModifiers};
use kanbrain_core::brain::Group;
use kanbrain_core::model::{DecisionState, Status, Task};
use kanbrain_core::period::{Period, Unit};

/// One thing a key does.
#[derive(Debug)]
pub struct Action {
    /// The key that runs it.
    pub key: KeyEvent,
    /// The key as the palette writes it; several keys in one row say so (`enter / space`).
    pub shown: &'static str,
    /// The tabs its key works on.
    pub tabs: Tabs,
    /// When the palette and the menu offer it. Its key works regardless, and says why it can't when it can't.
    pub when: When,
    /// What the palette says it does, which its search matches; empty for a key the palette doesn't list on its
    /// own, such as the arrows, which one row tells about.
    pub does: &'static str,
    /// What the right-click menu calls it; empty for what isn't in the menu.
    pub label: &'static str,
    /// Whether it destroys something, to draw it as a warning.
    pub danger: bool,
    /// Whether choosing it runs it: false for a row that only says how to do something, like moving around.
    pub runs: bool,
    /// What it does.
    pub run: fn(&mut Board),
}

/// The tabs a key works on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tabs {
    /// Every tab.
    Every,
    /// The board, the notes and the decisions: the tabs that list entries.
    Entries,
    /// The board only.
    Board,
    /// The notes and the decisions: the tabs that list entries without columns.
    Lists,
    /// The report only.
    Report,
    /// The calendar only.
    Calendar,
}

impl Tabs {
    /// Whether keys of these tabs work on `tab`.
    fn has(self, tab: Tab) -> bool {
        match self {
            Self::Every => true,
            Self::Entries => matches!(tab, Tab::Board | Tab::Notes | Tab::Decisions),
            Self::Board => tab == Tab::Board,
            Self::Lists => matches!(tab, Tab::Notes | Tab::Decisions),
            Self::Report => tab == Tab::Report,
            Self::Calendar => tab == Tab::Calendar,
        }
    }
}

/// When an action is worth offering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum When {
    /// Always.
    Always,
    /// With an entry selected.
    Entry,
    /// With an entry selected and its details shown (`true`) or hidden (`false`).
    Details(bool),
    /// With a task selected that isn't in this status.
    MoveTo(Status),
    /// With a card selected on the board tab, which can move between columns.
    Card,
    /// With a decision selected that isn't in this state, or a task that waits on one.
    DecisionNot(DecisionState),
    /// With an entry selected and another brain to move it to.
    OtherBrain,
    /// With an entry selected that doesn't wait for your answer.
    NotWaiting,
    /// With an entry selected that has comments (`true`) or none.
    Commented(bool),
    /// With a task selected that waits for your answer to an agent's question.
    Asks,
    /// With a task selected that waits on a decision an agent proposed.
    Proposed,
    /// With a task waiting for your answer somewhere shown.
    Waiting,
    /// With a project shown, or an entry of one selected.
    Project,
    /// With a project's repository shown, or an entry of one selected.
    Repo,
    /// With a newer release found.
    Update,
    /// With the details shown.
    Drawer,
    /// With the details hidden and something searched.
    Searched,
}

impl When {
    /// Whether it holds on `board`, for the entry `key` (the selected one, or the one the menu is on).
    fn holds(self, board: &Board, key: Option<Key>) -> bool {
        let entry = key.and_then(|k| board.task(k));
        match self {
            Self::Always => true,
            Self::Entry => entry.is_some(),
            Self::Details(shown) => entry.is_some() && board.drawer == shown,
            Self::MoveTo(status) => entry.is_some_and(|t| t.is_task() && t.status != status),
            Self::Card => board.tab == Tab::Board && entry.is_some(),
            Self::DecisionNot(state) => {
                key.and_then(|k| board.decision_for(k)).and_then(|d| board.task(d)).is_some_and(|d| d.decision != state)
            }
            Self::OtherBrain => entry.is_some() && board.sources.len() > 1,
            Self::NotWaiting => entry.is_some_and(|t| !t.waits_for_answer()),
            Self::Commented(some) => entry.is_some_and(|t| t.comments.is_empty() != some),
            // A task waiting on a proposed decision is answered by deciding (`y`, `Y`, `j`).
            Self::Asks => {
                entry.is_some_and(Task::waits_for_answer) && key.is_some_and(|k| board.proposed_for(k).is_empty())
            }
            Self::Proposed => key.is_some_and(|k| !board.proposed_for(k).is_empty()),
            Self::Waiting => !board.waiting().is_empty(),
            Self::Project => board.project_here().is_some(),
            Self::Repo => board.repo_here().is_some(),
            Self::Update => board.available.is_some(),
            Self::Drawer => board.drawer,
            Self::Searched => !board.drawer && !board.search.is_empty(),
        }
    }

    /// Whether it's about the selected entry, which the palette lists first.
    fn about_entry(self) -> bool {
        matches!(
            self,
            Self::Entry
                | Self::Details(_)
                | Self::MoveTo(_)
                | Self::Card
                | Self::DecisionNot(_)
                | Self::OtherBrain
                | Self::NotWaiting
                | Self::Commented(_)
                | Self::Asks
                | Self::Proposed
        )
    }
}

/// An action that `does`, run by the key `shown` writes (one ASCII character).
const fn letter(shown: &'static str, tabs: Tabs, when: When, does: &'static str, run: fn(&mut Board)) -> Action {
    named(KeyCode::Char(shown.as_bytes()[0] as char), shown, tabs, when, does, run)
}

/// An action run by a key without a character, or several keys written as one (`shown`).
const fn named(
    code: KeyCode,
    shown: &'static str,
    tabs: Tabs,
    when: When,
    does: &'static str,
    run: fn(&mut Board),
) -> Action {
    Action {
        key: KeyEvent::new(code, KeyModifiers::NONE),
        shown,
        tabs,
        when,
        does,
        label: "",
        danger: false,
        runs: true,
        run,
    }
}

/// A key the palette doesn't list on its own.
const fn unlisted(code: KeyCode, tabs: Tabs, run: fn(&mut Board)) -> Action {
    named(code, "", tabs, When::Always, "", run)
}

/// A palette row that only says how to do something.
const fn tell(shown: &'static str, tabs: Tabs, when: When, does: &'static str) -> Action {
    Action { runs: false, ..named(KeyCode::Esc, shown, tabs, when, does, |_| {}) }
}

impl Action {
    /// The same action, held with shift.
    const fn shifted(self) -> Self {
        Self { key: KeyEvent::new(self.key.code, KeyModifiers::SHIFT), ..self }
    }

    /// The same action, also in the right-click menu as `label`.
    const fn menu(self, label: &'static str) -> Self {
        Self { label, ..self }
    }

    /// The same action, drawn as a warning.
    const fn danger(self) -> Self {
        Self { danger: true, ..self }
    }

    /// Whether `key` runs it. Shift tells moving the selection (`←` `→`) from moving the card (`shift+←`
    /// `shift+→`); other keys don't look at it.
    fn matches(&self, key: KeyEvent) -> bool {
        let shift = |k: KeyEvent| k.modifiers.contains(KeyModifiers::SHIFT);
        self.runs
            && self.key.code == key.code
            && (!matches!(key.code, KeyCode::Left | KeyCode::Right) || shift(self.key) == shift(key))
    }
}

/// Runs `then` on the selected entry, if there's one.
fn selected(board: &mut Board, then: impl FnOnce(&mut Board, Key)) {
    if let Some(key) = board.selected {
        then(board, key);
    }
}

/// Moves the selected task to `status`.
fn move_to(board: &mut Board, status: Status) {
    selected(board, |b, key| b.set_status(key, status));
}

/// Comments on the selected entry, which answers the question it waits on, if any.
fn comment(board: &mut Board) {
    selected(board, |b, key| b.open_editor(Target::NewComment(key)));
}

/// Shows or hides the selected entry's details.
fn toggle_details(board: &mut Board) {
    selected(board, |b, _| b.drawer = !b.drawer);
}

/// Closes the details; with them closed, clears the search.
fn escape(board: &mut Board) {
    if board.drawer {
        board.drawer = false;
    } else {
        board.search.clear();
        board.keep_selection();
    }
}

/// Every action. Two on a tab share a key only when they run the same, shown differently by when they hold (`esc`
/// closes the details or clears the search), so a key runs whichever it matches first. The menu lists its actions
/// in this order, and the palette within each of its sections.
pub const ACTIONS: &[Action] = &[
    // The selected entry.
    named(KeyCode::Enter, "enter / space", Tabs::Entries, When::Details(false), "Show the details", toggle_details)
        .menu("Show details"),
    named(KeyCode::Enter, "enter / space", Tabs::Entries, When::Details(true), "Hide the details", toggle_details)
        .menu("Hide details"),
    unlisted(KeyCode::Char(' '), Tabs::Entries, toggle_details),
    letter("s", Tabs::Entries, When::MoveTo(Status::Doing), "Start: move to Doing and run its clock", |b| {
        move_to(b, Status::Doing);
    })
    .menu("Start"),
    letter("p", Tabs::Entries, When::MoveTo(Status::Parked), "Park: move to Parked", |b| move_to(b, Status::Parked))
        .menu("Park"),
    letter("d", Tabs::Entries, When::MoveTo(Status::Done), "Done: move to Done", |b| move_to(b, Status::Done))
        .menu("Done"),
    letter("t", Tabs::Entries, When::MoveTo(Status::Todo), "Back to To do", |b| move_to(b, Status::Todo))
        .menu("Back to to do"),
    named(KeyCode::Right, "shift+→", Tabs::Entries, When::Card, "Move the card to the next column", |b| {
        b.move_card(Step::Right);
    })
    .shifted(),
    named(KeyCode::Left, "shift+←", Tabs::Entries, When::Card, "Move the card to the previous column", |b| {
        b.move_card(Step::Left);
    })
    .shifted(),
    letter("e", Tabs::Entries, When::Entry, "Edit title and tags", |b| selected(b, |b, _| b.edit_selected()))
        .menu("Edit title and tags"),
    letter("m", Tabs::Entries, When::Entry, "Edit the description", |b| {
        selected(b, |b, key| b.open_editor(Target::Description(key)));
    })
    .menu("Edit description"),
    letter("c", Tabs::Entries, When::Asks, "Answer the agent's question", comment).menu("Answer"),
    letter("c", Tabs::Entries, When::NotWaiting, "Add a comment", comment).menu("Add comment"),
    letter("C", Tabs::Entries, When::Commented(true), "Edit or delete one of its comments", Board::open_comments)
        .menu("Edit or delete a comment"),
    // Not on the menu: there, an entry without comments has nothing to offer.
    letter("C", Tabs::Entries, When::Commented(false), "Edit or delete one of its comments", Board::open_comments),
    letter("D", Tabs::Entries, When::Proposed, "Open the decision it waits on", Board::open_proposed)
        .menu("Open proposed decision"),
    letter("r", Tabs::Entries, When::Entry, "Relate to other entries", |b| selected(b, |b, _| b.open_links()))
        .menu("Link"),
    letter(
        "y",
        Tabs::Entries,
        When::DecisionNot(DecisionState::Accepted),
        "Accept the decision (or the one the task waits on), answering the tasks waiting on it",
        |b| {
            selected(b, |b, _| b.accept_selected());
        },
    )
    .menu("Accept"),
    letter("Y", Tabs::Entries, When::DecisionNot(DecisionState::Accepted), "Accept the decision with a comment", |b| {
        selected(b, |b, _| b.accept_with_comment());
    })
    .menu("Accept with a comment"),
    letter(
        "j",
        Tabs::Entries,
        When::DecisionNot(DecisionState::Rejected),
        "Reject the decision (or the one the task waits on), saying why",
        |b| {
            selected(b, |b, _| b.reject_selected());
        },
    )
    .menu("Reject"),
    letter("M", Tabs::Entries, When::OtherBrain, "Move to another project (renumbered there, links dropped)", |b| {
        selected(b, |b, _| b.ask_move());
    })
    .menu("Move to project"),
    letter("o", Tabs::Entries, When::Entry, "Open the file in $EDITOR", |b| selected(b, |b, _| b.open_file()))
        .menu("Open file"),
    letter("x", Tabs::Entries, When::Entry, "Delete (asks first)", |b| {
        selected(b, |b, key| b.mode = Mode::Confirm(Confirm::Delete(key)));
    })
    .menu("Delete")
    .danger(),
    // Every tab.
    letter("n", Tabs::Every, When::Always, "New task (a note or decision on their tabs)", |b| {
        b.new_entry(Status::Todo);
    }),
    letter("/", Tabs::Every, When::Always, "Search titles and tags; #tag matches tags only", |b| b.mode = Mode::Search),
    letter("#", Tabs::Every, When::Always, "Filter by tags", Board::open_tags),
    named(KeyCode::Esc, "esc", Tabs::Every, When::Drawer, "Close the details", escape),
    named(KeyCode::Esc, "esc", Tabs::Every, When::Searched, "Clear the search", escape),
    letter("P", Tabs::Every, When::Always, "Choose the project shown", Board::open_scopes),
    letter(
        "I",
        Tabs::Every,
        When::Project,
        "Manage project: rename, move or remove it and its repositories; its description and time",
        Board::open_manage_project,
    ),
    letter("E", Tabs::Every, When::Project, "Edit the project's description", Board::edit_project_description),
    letter("N", Tabs::Every, When::Repo, "Edit the repository's role in its project", Board::edit_repo_notes),
    letter("A", Tabs::Every, When::Always, "Add a project: pick a code folder, as kb init there", |b| {
        b.request = Some(Request::Folder(None));
    }),
    letter("1", Tabs::Every, When::Always, "Go to the board", |b| b.switch_tab(Tab::Board)),
    letter("2", Tabs::Every, When::Always, "Go to the notes", |b| b.switch_tab(Tab::Notes)),
    letter("3", Tabs::Every, When::Always, "Go to the decisions", |b| b.switch_tab(Tab::Decisions)),
    letter("4", Tabs::Every, When::Always, "Go to the report", |b| b.switch_tab(Tab::Report)),
    letter("5", Tabs::Every, When::Always, "Go to the calendar", |b| b.switch_tab(Tab::Calendar)),
    named(KeyCode::Tab, "tab", Tabs::Every, When::Always, "Go to the next tab", |b| {
        let next = Tab::ALL.iter().position(|&t| t == b.tab).map_or(0, |i| (i + 1) % Tab::ALL.len());
        b.switch_tab(Tab::ALL[next]);
    }),
    named(KeyCode::BackTab, "shift+tab", Tabs::Every, When::Always, "Go to the previous tab", |b| {
        let at = Tab::ALL.iter().position(|&t| t == b.tab).unwrap_or(0);
        b.switch_tab(Tab::ALL[(at + Tab::ALL.len() - 1) % Tab::ALL.len()]);
    }),
    letter("u", Tabs::Every, When::Always, "Undo the last change; again, redo it", Board::undo),
    letter("R", Tabs::Every, When::Always, "Reload the files", Board::reload_from_disk),
    letter(
        "U",
        Tabs::Every,
        When::Update,
        "Update kanbrain to the newer release, once no agent is working",
        Board::ask_update,
    ),
    letter("?", Tabs::Every, When::Always, "", Board::open_palette),
    letter(
        "Q",
        Tabs::Every,
        When::Always,
        "Park your running tasks and quit (agents' keep running)",
        Board::park_and_quit,
    ),
    letter("q", Tabs::Every, When::Always, "Quit; running tasks keep their clocks running", |b| b.quit = true),
    // The report.
    named(KeyCode::Left, "←", Tabs::Report, When::Always, "Previous period", |b| {
        b.report.period = b.report.period.prev();
    }),
    named(KeyCode::Right, "→", Tabs::Report, When::Always, "Next period", |b| {
        b.report.period = b.report.period.next();
    }),
    letter("h", Tabs::Report, When::Always, "An hour", |b| b.report.period = b.report.period.with_unit(Unit::Hour)),
    letter("d", Tabs::Report, When::Always, "A day", |b| b.report.period = b.report.period.with_unit(Unit::Day)),
    letter("w", Tabs::Report, When::Always, "A week", |b| b.report.period = b.report.period.with_unit(Unit::Week)),
    letter("m", Tabs::Report, When::Always, "A month", |b| b.report.period = b.report.period.with_unit(Unit::Month)),
    letter("t", Tabs::Report, When::Always, "Back to the current period", |b| {
        b.report.period = Period::current(b.report.period.unit);
    }),
    letter("g", Tabs::Report, When::Always, "Group by task or by tag", |b| {
        b.report.group = if b.report.group == Group::Task { Group::Tag } else { Group::Task };
    }),
    letter("e", Tabs::Report, When::Always, "Export to reports/ as markdown", Board::export_report),
    // The calendar.
    named(KeyCode::Left, "←", Tabs::Calendar, When::Always, "Previous period", |b| {
        b.calendar.period = b.calendar.period.prev();
    }),
    named(KeyCode::Right, "→", Tabs::Calendar, When::Always, "Next period", |b| {
        b.calendar.period = b.calendar.period.next();
    }),
    letter("d", Tabs::Calendar, When::Always, "A day", |b| b.calendar.period = b.calendar.period.with_unit(Unit::Day)),
    letter("w", Tabs::Calendar, When::Always, "A week", |b| {
        b.calendar.period = b.calendar.period.with_unit(Unit::Week);
    }),
    letter("m", Tabs::Calendar, When::Always, "A month", |b| {
        b.calendar.period = b.calendar.period.with_unit(Unit::Month);
    }),
    letter("t", Tabs::Calendar, When::Always, "Back to the current period", |b| {
        b.calendar.period = Period::current(b.calendar.period.unit);
    }),
    letter("i", Tabs::Calendar, When::Always, "Which interactions: all, time, moves, comments, new entries", |b| {
        b.calendar.show = b.calendar.show.next();
    }),
    tell("↑ ↓", Tabs::Calendar, When::Always, "Select the entries shown, in time order"),
    unlisted(KeyCode::Up, Tabs::Calendar, |b| b.step(Step::Up)),
    unlisted(KeyCode::Down, Tabs::Calendar, |b| b.step(Step::Down)),
    named(KeyCode::Enter, "enter / space", Tabs::Calendar, When::Details(false), "Show the details", toggle_details),
    named(KeyCode::Enter, "enter / space", Tabs::Calendar, When::Details(true), "Hide the details", toggle_details),
    unlisted(KeyCode::Char(' '), Tabs::Calendar, toggle_details),
    // The board, the notes and the decisions.
    tell("← → ↑ ↓", Tabs::Board, When::Always, "Move between cards and columns"),
    tell("↑ ↓", Tabs::Lists, When::Always, "Move up and down the list"),
    unlisted(KeyCode::Left, Tabs::Entries, |b| b.step(Step::Left)),
    unlisted(KeyCode::Right, Tabs::Entries, |b| b.step(Step::Right)),
    unlisted(KeyCode::Up, Tabs::Entries, |b| b.step(Step::Up)),
    unlisted(KeyCode::Down, Tabs::Entries, |b| b.step(Step::Down)),
    letter("a", Tabs::Board, When::Always, "Add tasks at the top of To do, one after another", |b| {
        b.quick_add(Status::Todo);
    }),
    letter("v", Tabs::Board, When::Always, "Board as kanban columns or as a list", Board::toggle_layout),
    letter("f", Tabs::Board, When::Always, "Show every done task, or only the latest", |b| {
        b.all_done = !b.all_done;
        b.keep_selection();
    }),
    letter("w", Tabs::Entries, When::Waiting, "Go to the next task waiting for your answer", Board::next_waiting),
];

impl Board {
    /// A key on the board, the lists, the report or the calendar, outside any dialog.
    pub(super) fn key_normal(&mut self, key: KeyEvent) {
        let tab = self.tab;
        let action = ACTIONS.iter().find(|a| a.tabs.has(tab) && a.matches(key));
        if let Some(action) = action {
            (action.run)(self);
        }
        // The calendar's selection follows its period and filters, whichever of its keys changed them.
        if tab == Tab::Calendar && action.is_none_or(|a| a.tabs != Tabs::Every) {
            self.keep_selection();
        }
    }

    /// Runs what `key` does to the selected entry, for a button of its details, on any tab: the calendar's own keys
    /// take some of those letters (`d`, `m`, `t`), and the details' buttons act on the entry all the same.
    pub fn run_on_entry(&mut self, key: KeyEvent) {
        if matches!(self.mode, Mode::Search | Mode::QuickAdd(_)) {
            self.mode = Mode::Normal;
        }
        if let Some(action) = ACTIONS.iter().find(|a| a.tabs == Tabs::Entries && a.matches(key)) {
            (action.run)(self);
        }
    }

    /// The palette's rows for its search: what can be done now on the tab shown, those about the selected entry
    /// first, then the tab's own, then those of every tab.
    pub fn commands(&self, picker: &Picker) -> Vec<&'static Action> {
        let shown = |a: &&Action| {
            !a.does.is_empty()
                && a.tabs.has(self.tab)
                && a.when.holds(self, self.selected)
                && picker.matches(&format!("{} {}", a.does, a.shown))
        };
        let mut rows: Vec<&Action> = ACTIONS.iter().filter(shown).collect();
        rows.sort_by_key(|a| a.section() as u8);
        rows
    }

    /// What the right-click menu calls what `code` does to the selected entry now, for a button that does it.
    pub fn label_of(&self, code: KeyCode) -> &'static str {
        ACTIONS
            .iter()
            .find(|a| a.key.code == code && !a.label.is_empty() && a.when.holds(self, self.selected))
            .map_or("", |a| a.label)
    }

    /// The key that does what's offered `when`, as the palette writes it, if one does on the tab shown: for the
    /// tooltip of a button that does the same.
    pub fn shortcut(&self, when: When) -> Option<&'static str> {
        ACTIONS.iter().find(|a| a.when == when && a.tabs.has(self.tab)).map(|a| a.shown)
    }

    /// What the right-click menu offers on the entry `key`.
    pub fn menu_items(&self, key: Key) -> Vec<&'static Action> {
        ACTIONS
            .iter()
            .filter(|a| !a.label.is_empty() && a.tabs.has(self.tab) && a.when.holds(self, Some(key)))
            .collect()
    }
}

/// The palette's sections, in the order it shows them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    /// What acts on the selected entry.
    Entry,
    /// The rest of the tab's own.
    Tab,
    /// What works the same on every tab.
    Everywhere,
}

impl Section {
    /// Its heading.
    pub fn title(self) -> &'static str {
        match self {
            Self::Entry => "Selected entry",
            Self::Tab => "This tab",
            Self::Everywhere => "Everywhere",
        }
    }
}

impl Action {
    /// The palette section it shows in.
    pub fn section(&self) -> Section {
        match (self.when.about_entry(), self.tabs) {
            (true, _) => Section::Entry,
            (false, Tabs::Every) => Section::Everywhere,
            (false, _) => Section::Tab,
        }
    }
}

#[cfg(test)]
#[path = "../../tests/keys.rs"]
mod tests;
