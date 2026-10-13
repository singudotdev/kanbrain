//! The command palette (`?`): every action of the tab with its key, filtered by a search. Choosing one
//! presses its key, so the palette can't do anything the keyboard wouldn't, and it doubles as the list
//! of shortcuts.

use super::{Board, Mode, Picker, PickerKey, Tab};
use crate::text::event::{KeyCode, KeyEvent, KeyModifiers};

/// One row of the palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Command {
    /// The key as it's written on the keyboard, shown beside it.
    pub key: &'static str,
    /// What it does, which the search matches along with the key.
    pub does: &'static str,
    /// The key it presses; `None` for a row that only says how to do something, like moving around.
    pub press: Option<KeyEvent>,
    /// What it needs to do anything; the palette leaves it out otherwise.
    pub needs: Needs,
}

/// What a command needs to do anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Needs {
    /// Nothing: it works anywhere.
    Nothing,
    /// A selected entry.
    Entry,
    /// The board tab.
    Board,
    /// A selected card on the board tab, as moving it between columns does.
    Card,
}

/// A row that presses `code`.
const fn run(key: &'static str, does: &'static str, code: KeyCode, needs: Needs) -> Command {
    Command { key, does, press: Some(KeyEvent::new(code, KeyModifiers::NONE)), needs }
}

/// A row that presses a letter or symbol, written as itself.
const fn letter(key: &'static str, c: char, does: &'static str, needs: Needs) -> Command {
    run(key, does, KeyCode::Char(c), needs)
}

/// A row that only says how to do something.
const fn tell(key: &'static str, does: &'static str, needs: Needs) -> Command {
    Command { key, does, press: None, needs }
}

/// The board's, the notes' and the decisions' own commands.
const BOARD: &[Command] = &[
    tell("← → ↑ ↓", "Move between cards and columns", Needs::Nothing),
    Command {
        key: "shift+→",
        does: "Move the card to the next column",
        press: Some(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT)),
        needs: Needs::Card,
    },
    Command {
        key: "shift+←",
        does: "Move the card to the previous column",
        press: Some(KeyEvent::new(KeyCode::Left, KeyModifiers::SHIFT)),
        needs: Needs::Card,
    },
    letter("s", 's', "Start: move to Doing and run its clock", Needs::Card),
    letter("p", 'p', "Park: move to Parked", Needs::Card),
    letter("d", 'd', "Done: move to Done", Needs::Card),
    letter("t", 't', "Back to To do", Needs::Card),
    run("enter / space", "Show or hide the details", KeyCode::Enter, Needs::Entry),
    letter("e", 'e', "Edit title and tags", Needs::Entry),
    letter("m", 'm', "Edit the description", Needs::Entry),
    letter("c", 'c', "Add a comment", Needs::Entry),
    letter("r", 'r', "Relate to other entries", Needs::Entry),
    letter("o", 'o', "Open the file in $EDITOR", Needs::Entry),
    letter("M", 'M', "Move to another project (renumbered there, links dropped)", Needs::Entry),
    letter("x", 'x', "Delete (asks first)", Needs::Entry),
    letter("y", 'y', "Accept the decision, answering the tasks waiting on it", Needs::Entry),
    letter("j", 'j', "Reject the decision, saying why", Needs::Entry),
    letter("w", 'w', "Go to the next task waiting for your answer", Needs::Nothing),
    letter("a", 'a', "Add tasks at the top of To do, one after another", Needs::Board),
    letter("v", 'v', "Board as kanban columns or as a list", Needs::Board),
    letter("f", 'f', "Show every done task, or only the latest", Needs::Board),
];

/// The report tab's own commands.
const REPORT: &[Command] = &[
    run("←", "Previous period", KeyCode::Left, Needs::Nothing),
    run("→", "Next period", KeyCode::Right, Needs::Nothing),
    letter("h", 'h', "An hour", Needs::Nothing),
    letter("d", 'd', "A day", Needs::Nothing),
    letter("w", 'w', "A week", Needs::Nothing),
    letter("m", 'm', "A month", Needs::Nothing),
    letter("t", 't', "Back to the current period", Needs::Nothing),
    letter("g", 'g', "Group by task or by tag", Needs::Nothing),
    letter("e", 'e', "Export to reports/ as markdown", Needs::Nothing),
];

/// The calendar tab's own commands.
const CALENDAR: &[Command] = &[
    run("←", "Previous period", KeyCode::Left, Needs::Nothing),
    run("→", "Next period", KeyCode::Right, Needs::Nothing),
    letter("d", 'd', "A day", Needs::Nothing),
    letter("w", 'w', "A week", Needs::Nothing),
    letter("m", 'm', "A month", Needs::Nothing),
    letter("t", 't', "Back to the current period", Needs::Nothing),
    letter("i", 'i', "Which interactions: all, time, moves, comments, new entries", Needs::Nothing),
    tell("↑ ↓", "Select the entries shown, in time order", Needs::Nothing),
    run("enter / space", "Show or hide the details", KeyCode::Enter, Needs::Entry),
];

/// Commands of every tab, after the tab's own.
const EVERYWHERE: &[Command] = &[
    letter("n", 'n', "New task (a note or decision on their tabs)", Needs::Nothing),
    letter("/", '/', "Search titles and tags; #tag matches tags only", Needs::Nothing),
    letter("#", '#', "Filter by tags", Needs::Nothing),
    letter("P", 'P', "Choose the project shown", Needs::Nothing),
    letter("I", 'I', "Project info: description, repositories, time", Needs::Nothing),
    letter("A", 'A', "Add a project: pick a code folder, as `kb init` there", Needs::Nothing),
    letter("1", '1', "Go to the board", Needs::Nothing),
    letter("2", '2', "Go to the notes", Needs::Nothing),
    letter("3", '3', "Go to the decisions", Needs::Nothing),
    letter("4", '4', "Go to the report", Needs::Nothing),
    letter("5", '5', "Go to the calendar", Needs::Nothing),
    run("tab", "Go to the next tab", KeyCode::Tab, Needs::Nothing),
    letter("u", 'u', "Undo the last change; again, redo it", Needs::Nothing),
    letter("R", 'R', "Reload the files", Needs::Nothing),
    letter("Q", 'Q', "Park your running tasks and quit (agents' keep running)", Needs::Nothing),
    letter("q", 'q', "Quit; running tasks keep their clocks running", Needs::Nothing),
];

impl Board {
    /// The palette's rows: the tab's commands, then those of every tab, leaving out what can't do
    /// anything now (an entry's with none selected), and keeping those whose description or key
    /// contains every word of its search.
    pub fn commands(&self, picker: &Picker) -> Vec<Command> {
        let own = match self.tab {
            Tab::Report => REPORT,
            Tab::Calendar => CALENDAR,
            Tab::Board | Tab::Notes | Tab::Decisions => BOARD,
        };
        own.iter()
            .chain(EVERYWHERE)
            .filter(|c| self.can(c.needs))
            .filter(|c| picker.matches(&format!("{} {}", c.does, c.key)))
            .copied()
            .collect()
    }

    /// Whether a command that needs `needs` would do anything now.
    fn can(&self, needs: Needs) -> bool {
        let entry = self.selected.is_some();
        let board = self.tab == Tab::Board;
        match needs {
            Needs::Nothing => true,
            Needs::Entry => entry,
            Needs::Board => board,
            Needs::Card => board && entry,
        }
    }

    /// Opens the palette.
    pub fn open_palette(&mut self) {
        self.mode = Mode::Palette(Picker::default());
    }

    /// Closes the palette and presses the key of its row at `index`; a row that only says how to do
    /// something keeps it open.
    pub fn choose_command(&mut self, index: usize) {
        let Mode::Palette(picker) = &self.mode else { return };
        let Some(command) = self.commands(picker).get(index).copied() else { return };
        let Some(key) = command.press else { return };
        self.mode = Mode::Normal;
        self.on_key(key);
    }

    /// Keys while the palette is open: arrows move, typing filters, `enter` runs the highlighted
    /// command, `esc` closes it.
    pub(super) fn key_palette(&mut self, mut picker: Picker, key: KeyEvent) {
        let count = self.commands(&picker).len();
        let outcome = picker.key(key, count);
        let sel = picker.sel;
        if outcome != PickerKey::Close {
            self.mode = Mode::Palette(picker);
        }
        if outcome == PickerKey::Choose {
            self.choose_command(sel);
        }
    }
}
