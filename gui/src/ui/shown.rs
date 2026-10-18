//! Text shown rather than typed into, selected with the mouse to copy: an entry's details, the dialogs' text, the
//! status bar's message. Cards and list rows aren't: dragging them moves them.
//!
//! Each such text is a [`Selectable`], built while it's drawn: every piece laid out as one (a paragraph, a code
//! block's line, a tag) goes through [`Selectable::line`], which highlights what's selected of it and remembers where
//! it was drawn; [`Selectable::finish`] then makes the element holding them take the mouse. Pressing, dragging,
//! double- and triple-clicking and shift+click select as in a field ([`super::pointer`]), across the text's pieces,
//! and the selection is the board's ([`Board::point_shown`]).

use super::BrainView;
use super::pointer::{Hit, TextBox, at, follow_drag, pointer_of};
use super::text::highlighted;
use super::theme::Line;
use crate::state::{Board, Shown};
use crate::text::editor::Pointer;
use gpui::{
    Context, Div, Global, InteractiveElement, MouseButton, MouseDownEvent, ParentElement, SharedString, Styled,
    StyledText, div,
};
use std::rc::Rc;

/// Whether the frame being drawn drew the text holding the selection as it was selected; the root view drops the
/// selection after a frame that didn't (another entry's details, a dialog closed, a message gone, `kb` changing it):
/// a copy would otherwise take text no longer shown.
#[derive(Default)]
pub struct Drawn(pub bool);

impl Global for Drawn {}

/// A text being drawn that the mouse selects.
pub struct Selectable<'a> {
    /// Which text, among those the window shows: an entry's say, `0/3 description`.
    region: SharedString,
    /// The selection, if this text holds it.
    shown: Option<&'a Shown>,
    /// Its pieces so far, as drawn.
    pieces: Vec<String>,
    /// Where each was drawn.
    hits: Vec<Hit>,
}

impl<'a> Selectable<'a> {
    /// The text `region`. Its name must differ from every other text's the window shows at once, and from the same
    /// place's for another entry or repository, so a selection never lands in another.
    pub fn new(board: &'a Board, region: impl Into<SharedString>) -> Self {
        let region = region.into();
        Self { shown: board.shown_in(&region), region, pieces: Vec::new(), hits: Vec::new() }
    }

    /// `line` drawn as the text's next piece, with what's selected of it highlighted.
    pub fn line(&mut self, line: Line) -> StyledText {
        let piece = line.text();
        let selected = self.shown.and_then(|shown| shown.range(self.pieces.len(), &piece));
        let text = highlighted(line, selected).render();
        self.hits.push(Hit::new(&text, piece.chars().count()));
        self.pieces.push(piece);
        text
    }

    /// Makes `element`, which holds every piece, take the mouse to select them. A click outside it drops the
    /// selection it holds.
    pub fn finish<E: InteractiveElement + ParentElement + Styled>(self, element: E, cx: &mut Context<BrainView>) -> E {
        let Self { region, shown, pieces, hits } = self;
        if hits.is_empty() {
            return element;
        }
        if shown.is_some_and(|shown| shown.is(&pieces)) {
            cx.set_global(Drawn(true));
        }
        let (pieces, hits): (Rc<[String]>, Rc<[Hit]>) = (pieces.into(), hits.into());
        let named = region.clone();
        let (pressed, pressed_pieces, pressed_hits) = (region.clone(), Rc::clone(&pieces), Rc::clone(&hits));
        // Not stopped: a link under the press opens on its release, and a conflict's side picks on its click.
        let press = cx.listener(move |this, event: &MouseDownEvent, _, cx| {
            this.board.point_shown(&pressed, &pressed_pieces, at(&pressed_hits, event.position), pointer_of(event));
            this.drag = Some(TextBox::Shown);
            this.after_input(cx);
        });
        let element = element
            // `text REGION` for the window tests, which find a text by its name; nothing outside them.
            .debug_selector(move || format!("text {named}"))
            .cursor_text()
            .on_mouse_down(MouseButton::Left, press);
        // Only the text holding the selection follows a drag and drops it on a click elsewhere: listeners on every
        // text shown, for each move of the mouse, would add up with a long thread. A press makes its text the one
        // holding it, which the frame drawn after it takes up.
        if shown.is_none() {
            return element;
        }
        let (dragged, left) = (region.clone(), region);
        let drag = follow_drag(
            TextBox::Shown,
            hits,
            move |this, place, cx| {
                if this.board.shown_in(&dragged).is_some() {
                    this.board.point_shown(&dragged, &pieces, place, Pointer::Extend);
                    this.share_selection(cx);
                }
            },
            cx,
        );
        let elsewhere = cx.listener(move |this, _: &MouseDownEvent, _, cx| {
            if this.board.shown_in(&left).is_some() {
                this.board.clear_shown();
                cx.notify();
            }
        });
        element.on_mouse_down_out(elsewhere).child(drag)
    }
}

/// `line` as a text of its own, `region`, selected with the mouse.
pub fn text(board: &Board, region: impl Into<SharedString>, line: Line, cx: &mut Context<BrainView>) -> Div {
    let mut text = Selectable::new(board, region);
    let drawn = text.line(line);
    text.finish(div().child(drawn), cx)
}
