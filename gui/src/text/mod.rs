//! Typing text: the editors that hold it and the key presses they take, without any drawing.
//!
//! - [`editor`]: one-line fields and the multi-line editor, with a cursor and a selection.
//! - [`event`]: key presses as the editors and the board see them.

pub mod editor;
pub mod event;
