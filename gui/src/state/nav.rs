//! Moving the selection: between columns and up and down a column on the kanban board, up and down the
//! list on the board's list layout and the notes and decisions tabs.

use super::{Board, COLUMNS, Key, Layout, Tab};
use kanbrain_core::model::Status;

/// A direction to move the selection in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// The previous column.
    Left,
    /// The next column.
    Right,
    /// The card or entry above.
    Up,
    /// The card or entry below.
    Down,
}

impl Board {
    /// The selected card's column (index into `COLUMNS`) and row.
    pub fn position(&self) -> Option<(usize, usize)> {
        let id = self.selected?;
        COLUMNS
            .iter()
            .enumerate()
            .find_map(|(col, &status)| self.column(status).iter().position(|(k, _)| *k == id).map(|row| (col, row)))
    }

    /// Moves the selection one step. Sideways on the board it goes to the nearest column with cards,
    /// at the same height or the last card if the column is shorter.
    pub fn step(&mut self, step: Step) {
        if self.tab != Tab::Board || self.layout == Layout::List {
            self.step_list(step);
            return;
        }
        let Some((col, row)) = self.position() else {
            self.keep_selection();
            return;
        };
        let target = match step {
            Step::Up => self.card_at(col, row.saturating_sub(1)),
            Step::Down => self.card_at(col, row + 1).or(self.selected),
            Step::Left => (0..col).rev().find_map(|c| self.card_at(c, row)),
            Step::Right => (col + 1..COLUMNS.len()).find_map(|c| self.card_at(c, row)),
        };
        if let Some(id) = target {
            self.selected = Some(id);
        }
    }

    /// The card at `row` of column `col`, or its last card if it's shorter; `None` for an empty column.
    fn card_at(&self, col: usize, row: usize) -> Option<Key> {
        let cards = self.column(COLUMNS[col]);
        cards.get(row).or(cards.last()).map(|(key, _)| *key)
    }

    /// Up and down a list: the board's in its list layout, or the notes or decisions.
    fn step_list(&mut self, step: Step) {
        let ids = self.visible_ids();
        let Some(index) = self.selected.and_then(|id| ids.iter().position(|&i| i == id)) else {
            self.keep_selection();
            return;
        };
        let index = match step {
            Step::Up => index.saturating_sub(1),
            Step::Down => (index + 1).min(ids.len() - 1),
            Step::Left | Step::Right => index,
        };
        self.selected = Some(ids[index]);
    }

    /// The column next to the selected card's, in the direction of `step`, for moving the card there.
    pub fn neighbour_status(&self, step: Step) -> Option<Status> {
        let task = self.selected_task().filter(|t| t.is_task())?;
        let col = COLUMNS.iter().position(|&s| s == task.status)?;
        match step {
            Step::Left => col.checked_sub(1).map(|c| COLUMNS[c]),
            Step::Right => COLUMNS.get(col + 1).copied(),
            Step::Up | Step::Down => None,
        }
    }
}
