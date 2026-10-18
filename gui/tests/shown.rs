use super::*;
use crate::state::Mode;
use crate::text::editor::LineInput;
use kanbrain_core::store::Store;
use tempfile::TempDir;

/// A board over an empty folder, which lives as long as it.
fn board() -> (Board, TempDir) {
    let dir = tempfile::tempdir().unwrap();
    Store::at(dir.path()).unwrap();
    (Board::open(dir.path(), None).unwrap(), dir)
}

/// A description of two paragraphs, as drawn.
fn paragraphs() -> Vec<String> {
    vec!["First paragraph here".to_string(), "second one".to_string()]
}

#[test]
fn a_drag_selects_across_a_texts_pieces_and_copies_them_on_lines_of_their_own() {
    let (mut board, _dir) = board();
    let text = paragraphs();
    board.point_shown("d", &text, (0, 6), Pointer::Place);
    assert_eq!(board.shown_text(), None, "a press alone selects nothing");
    board.point_shown("d", &text, (1, 6), Pointer::Extend);
    assert_eq!(board.shown_text().as_deref(), Some("paragraph here\nsecond"));
    assert_eq!(board.copy_text().as_deref(), Some("paragraph here\nsecond"), "what ctrl+c takes");
    // Backwards from where it started too.
    board.point_shown("d", &text, (0, 0), Pointer::Extend);
    assert_eq!(board.shown_text().as_deref(), Some("First "));
}

#[test]
fn double_and_triple_clicks_select_a_word_and_a_piece_and_a_drag_grows_by_them() {
    let (mut board, _dir) = board();
    let text = paragraphs();
    board.point_shown("d", &text, (0, 8), Pointer::Word);
    assert_eq!(board.shown_text().as_deref(), Some("paragraph"));
    board.point_shown("d", &text, (1, 1), Pointer::Extend);
    assert_eq!(board.shown_text().as_deref(), Some("paragraph here\nsecond"), "whole words");
    board.point_shown("d", &text, (1, 3), Pointer::Line);
    assert_eq!(board.shown_text().as_deref(), Some("second one"));
    // A paragraph with a line break in it is one piece: its line break is copied.
    let broken = vec!["one\ntwo".to_string()];
    board.point_shown("c", &broken, (0, 1), Pointer::Line);
    assert_eq!(board.shown_text().as_deref(), Some("one\ntwo"));
}

#[test]
fn shift_click_extends_from_the_press_but_only_in_the_same_text_as_drawn() {
    let (mut board, _dir) = board();
    let text = paragraphs();
    board.point_shown("d", &text, (0, 0), Pointer::Place);
    board.point_shown("d", &text, (0, 5), Pointer::Extend);
    assert_eq!(board.shown_text().as_deref(), Some("First"));
    // In another text it starts over there, a press.
    board.point_shown("comment 0", &text, (0, 3), Pointer::Extend);
    assert_eq!(board.shown_text(), None);
    assert!(board.shown_in("comment 0").is_some() && board.shown_in("d").is_none(), "one text holds it");
    board.point_shown("comment 0", &text, (0, 5), Pointer::Extend);
    assert_eq!(board.shown_text().as_deref(), Some("st"));
    // The same text, changed meanwhile, starts over too.
    let changed = vec!["First paragraph, changed".to_string()];
    board.point_shown("comment 0", &changed, (0, 8), Pointer::Extend);
    assert_eq!(board.shown_text(), None);
}

#[test]
fn the_selection_is_drawn_on_the_pieces_it_covers_while_they_read_the_same() {
    let (mut board, _dir) = board();
    let text = vec!["ab".to_string(), "cdef".to_string(), "gh".to_string()];
    board.point_shown("d", &text, (0, 1), Pointer::Place);
    board.point_shown("d", &text, (2, 1), Pointer::Extend);
    let shown = board.shown_in("d").expect("d holds it");
    assert!(shown.is(&text) && !shown.is(&text[..2]));
    assert_eq!([0, 1, 2].map(|i| shown.range(i, &text[i])), [Some((1, 2)), Some((0, 4)), Some((0, 1))]);
    assert_eq!(shown.range(1, "cdxf"), None, "a piece drawn otherwise");
    assert_eq!(shown.range(3, "ij"), None, "a piece it never had");
    board.point_shown("d", &text, (1, 1), Pointer::Place);
    board.point_shown("d", &text, (1, 3), Pointer::Extend);
    let shown = board.shown_in("d").unwrap();
    assert_eq!([0, 1, 2].map(|i| shown.range(i, &text[i])), [None, Some((1, 3)), None], "only the piece it's in");
}

#[test]
fn a_fields_selection_is_copied_before_the_shown_texts_and_a_click_elsewhere_drops_it() {
    let (mut board, _dir) = board();
    let text = paragraphs();
    board.point_shown("d", &text, (1, 0), Pointer::Line);
    assert!(board.selects_shown("d") && !board.selects_shown("e"));
    board.mode = Mode::Search;
    board.search = LineInput::new("bug");
    assert_eq!(board.copy_text().as_deref(), Some("second one"), "the field has nothing selected");
    board.select_all();
    assert_eq!(board.copy_text().as_deref(), Some("bug"), "the field with the keys first");
    board.clear_shown();
    board.mode = Mode::Normal;
    assert_eq!((board.copy_text(), board.selects_shown("d")), (None, false));
}
