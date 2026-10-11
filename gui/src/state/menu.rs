//! The menu a right click opens on a card or an entry: what can be done to it, each item one of the
//! keys that does it, so the menu can't do anything the keyboard wouldn't.

use super::{Board, Key, Mode, Status};
use crate::text::event::{KeyCode, KeyEvent, KeyModifiers};
use kanbrain_core::model::{DecisionState, Kind};

/// One row of the menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuItem {
    /// What it says.
    pub label: &'static str,
    /// The key it presses on the entry, also shown beside it.
    pub code: KeyCode,
    /// Whether it destroys something, to draw it as a warning.
    pub danger: bool,
}

/// The menu shown, on which entry, and the highlighted row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuPicker {
    /// The entry it acts on; it's also selected while the menu is open.
    pub key: Key,
    /// Index into `Board::menu_items`.
    pub sel: usize,
}

/// A row that presses `code`.
const fn item(label: &'static str, code: KeyCode) -> MenuItem {
    MenuItem { label, code, danger: false }
}

impl Board {
    /// What can be done to the entry `key` names: its details, the status moves a task doesn't have
    /// yet, editing it, and the rest of the details' buttons, in the order they show there.
    pub fn menu_items(&self, key: Key) -> Vec<MenuItem> {
        let Some(entry) = self.task(key) else { return Vec::new() };
        let mut items = vec![item(if self.drawer { "Hide details" } else { "Show details" }, KeyCode::Enter)];
        if entry.is_task() {
            let moves = [
                (Status::Doing, "Start", KeyCode::Char('s')),
                (Status::Parked, "Park", KeyCode::Char('p')),
                (Status::Done, "Done", KeyCode::Char('d')),
                (Status::Todo, "Back to to do", KeyCode::Char('t')),
            ];
            items.extend(moves.into_iter().filter(|&(status, ..)| status != entry.status).map(|(_, l, c)| item(l, c)));
        }
        items.extend([
            item("Edit title and tags", KeyCode::Char('e')),
            item("Edit description", KeyCode::Char('m')),
            item("Add comment", KeyCode::Char('c')),
            item("Link…", KeyCode::Char('r')),
        ]);
        if entry.kind == Kind::Decision {
            if entry.decision != DecisionState::Accepted {
                items.push(item("Accept", KeyCode::Char('y')));
            }
            if entry.decision != DecisionState::Rejected {
                items.push(item("Reject…", KeyCode::Char('j')));
            }
        }
        if self.sources.len() > 1 {
            items.push(item("Move to project…", KeyCode::Char('M')));
        }
        items.push(item("Open file", KeyCode::Char('o')));
        items.push(MenuItem { danger: true, ..item("Delete…", KeyCode::Char('x')) });
        items
    }

    /// Selects the entry and opens its menu, from a right click on it.
    pub fn open_menu(&mut self, key: Key) {
        if self.task(key).is_none() {
            return;
        }
        self.select(key);
        self.mode = Mode::Menu(MenuPicker { key, sel: 0 });
    }

    /// Closes the menu and presses the key of its row at `index` on its entry.
    pub fn choose_menu(&mut self, index: usize) {
        let Mode::Menu(menu) = self.mode else { return };
        self.mode = Mode::Normal;
        let Some(chosen) = self.menu_items(menu.key).get(index).copied() else { return };
        self.selected = Some(menu.key);
        self.on_key(KeyEvent::new(chosen.code, KeyModifiers::NONE));
    }

    /// Keys while the menu is open: arrows move, `enter` chooses, an item's own key
    /// chooses it, anything else closes the menu.
    pub(super) fn key_menu(&mut self, mut menu: MenuPicker, key: KeyEvent) {
        let items = self.menu_items(menu.key);
        let last = items.len().saturating_sub(1);
        match key.code {
            KeyCode::Up => menu.sel = menu.sel.saturating_sub(1),
            KeyCode::Down => menu.sel = (menu.sel + 1).min(last),
            KeyCode::Enter => {
                self.mode = Mode::Menu(menu);
                return self.choose_menu(menu.sel);
            }
            code => {
                if let Some(index) = items.iter().position(|i| i.code == code) {
                    self.mode = Mode::Menu(menu);
                    self.choose_menu(index);
                }
                return;
            }
        }
        self.mode = Mode::Menu(menu);
    }
}
