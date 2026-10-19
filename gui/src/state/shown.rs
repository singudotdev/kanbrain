//! Text the window shows rather than types into (an entry's title, description and comments, a dialog's lines, a
//! message), selected with the mouse to copy, as on a web page. One text at a time holds the selection, and a drag
//! stays in the text it started in: from one comment's paragraphs to the next comment it goes no further.
//!
//! The window names each text it draws (`0/3 description`) and hands over its pieces as drawn, each a run laid out
//! as one (a paragraph, a code block's line, a tag), with the mouse's place as (piece, character). They're the lines
//! of a [`TextEditor`], which selects as the editor does; copying joins them with line breaks. A text drawn otherwise
//! than when it was selected (`kb` changed it meanwhile) starts a new selection at the next press.

use super::Board;
use crate::text::editor::{Pointer, TextEditor};

/// The text holding the selection, and the selection in it.
#[derive(Debug)]
pub struct Shown {
    /// Which text it is, as the window names it.
    region: String,
    /// Its pieces as drawn, one per line, and the selection over them.
    text: TextEditor,
}

impl Shown {
    /// Whether `pieces` are this text's as it was selected.
    pub fn is(&self, pieces: &[String]) -> bool {
        self.text.lines() == pieces
    }

    /// The characters selected, from start to end, of piece `index`, when it still reads `piece`.
    pub fn range(&self, index: usize, piece: &str) -> Option<(usize, usize)> {
        if self.text.lines().get(index).is_none_or(|drawn| drawn != piece) {
            return None;
        }
        let ((r1, c1), (r2, c2)) = self.text.selection()?;
        (r1..=r2)
            .contains(&index)
            .then(|| (if index == r1 { c1 } else { 0 }, if index == r2 { c2 } else { piece.chars().count() }))
    }
}

impl Board {
    /// A mouse press or drag at character `col` of piece `piece` of the text `region` shows, drawn as `pieces`. A
    /// press anywhere but the text holding the selection, or in it drawn otherwise since, starts over there: a
    /// shift+click places the cursor rather than extending.
    pub fn point_shown(&mut self, region: &str, pieces: &[String], (piece, col): (usize, usize), how: Pointer) {
        let same = self.shown.as_ref().is_some_and(|shown| shown.region == region && shown.is(pieces));
        let how = if !same && how == Pointer::Extend { Pointer::Place } else { how };
        let shown = match &mut self.shown {
            Some(shown) if same => shown,
            shown => shown.insert(Shown { region: region.to_string(), text: TextEditor::of_lines(pieces.to_vec()) }),
        };
        shown.text.point((piece, col), how);
    }

    /// The selection of the text `region`, if that one holds it.
    pub fn shown_in(&self, region: &str) -> Option<&Shown> {
        self.shown.as_ref().filter(|shown| shown.region == region)
    }

    /// Whether some of the text `region` is selected: a click on it that does something else (a conflict's side
    /// picks it) doesn't when it ends a drag that selected.
    pub fn selects_shown(&self, region: &str) -> bool {
        self.shown_in(region).is_some_and(|shown| shown.text.selection().is_some())
    }

    /// Drops the selection of shown text: a click elsewhere, or the text no longer drawn.
    pub fn clear_shown(&mut self) {
        self.shown = None;
    }

    /// The shown text selected, pieces on lines of their own.
    pub fn shown_text(&self) -> Option<String> {
        self.shown.as_ref()?.text.selected_text()
    }

    /// What copying takes: the text selected in the field being typed into, else the shown text selected, as a field
    /// with the keys keeps them.
    pub fn copy_text(&mut self) -> Option<String> {
        self.selected_text().or_else(|| self.shown_text())
    }
}

#[cfg(test)]
#[path = "../../tests/shown.rs"]
mod tests;
