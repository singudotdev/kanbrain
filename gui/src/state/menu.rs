//! The menu a right click opens on a card or an entry: what can be done to it, from [`super::keys::ACTIONS`], each
//! item one of the keys that does it, so the menu can't do anything the keyboard wouldn't.

use super::{Board, Key, Mode};
use crate::text::event::{KeyCode, KeyEvent};

/// The menu shown, on which entry, and the highlighted row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuPicker {
    /// The entry it acts on; it's also selected while the menu is open.
    pub key: Key,
    /// Index into `Board::menu_items`.
    pub sel: usize,
}

impl Board {
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
        self.on_key(chosen.key);
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
                if let Some(index) = items.iter().position(|i| i.key.code == code) {
                    self.mode = Mode::Menu(menu);
                    self.choose_menu(index);
                }
                return;
            }
        }
        self.mode = Mode::Menu(menu);
    }
}
