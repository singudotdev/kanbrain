//! The command palette (`?`): what can be done now on the tab, with its key, from [`super::keys::ACTIONS`],
//! filtered by a search. Choosing one presses its key, so the palette can't do anything the keyboard wouldn't, and
//! it doubles as the list of shortcuts.

use super::{Board, Mode, Picker};
use crate::text::event::KeyEvent;

impl Board {
    /// Opens the palette.
    pub fn open_palette(&mut self) {
        self.mode = Mode::Palette(Picker::default());
    }

    /// Closes the palette and presses the key of its row at `index`; a row that only says how to do
    /// something keeps it open.
    pub fn choose_command(&mut self, index: usize) {
        let Mode::Palette(picker) = &self.mode else { return };
        let Some(action) = self.commands(picker).get(index).copied() else { return };
        if !action.runs {
            return;
        }
        self.mode = Mode::Normal;
        self.on_key(action.key);
    }

    /// Keys while the palette is open: arrows move, typing filters, `enter` runs the highlighted
    /// command, `esc` closes it.
    pub(super) fn key_palette(&mut self, picker: Picker, key: KeyEvent) {
        let count = self.commands(&picker).len();
        self.pick(picker, key, count, |p| p, Mode::Palette, Self::choose_command);
    }
}
