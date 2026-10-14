//! Building blocks shared by the tabs and dialogs: buttons, text fields, status pills, tag chips, cards and tooltips.

use super::caret::caret;
use super::pointer::{Hit, TextBox, pointable};
use super::theme::{self, ACCENT, BORDER, DIM, INPUT, Line, TEXT};
use super::{BrainView, byte_at, edited};
use crate::state::Board;
use crate::text::editor::LineInput;
use gpui::{
    AnyView, App as GpuiApp, Context, Div, ElementId, FontWeight, SharedString, Stateful, Window, div, prelude::*, px,
    rgb, rgba, transparent_black,
};
use kanbrain_core::model::{Asked, DecisionState, Status, Task};

/// How a button looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// The main action of a screen or dialog.
    Primary,
    /// Any other action.
    Secondary,
    /// Deletes something.
    Danger,
    /// A quiet action: only outlined until hovered, so it still reads as a button rather than as text.
    Ghost,
}

/// A clickable button; the caller adds what it does with `on_click`.
pub fn button(id: impl Into<ElementId>, label: impl Into<SharedString>, kind: Kind) -> Stateful<Div> {
    let (bg, hover, fg, border) = match kind {
        Kind::Primary => (rgb(ACCENT).into(), rgb(theme::ACCENT_HOVER), theme::ON_ACCENT, ACCENT),
        Kind::Secondary => (rgb(theme::HOVER).into(), rgb(BORDER), TEXT, BORDER),
        Kind::Danger => (rgb(theme::RED).into(), rgb(theme::RED_HOVER), theme::ON_ACCENT, theme::RED),
        Kind::Ghost => (transparent_black(), rgb(theme::HOVER), TEXT, BORDER),
    };
    let id = id.into();
    // `button ID` for the window tests, which click buttons by it; nothing outside them.
    let selector = format!("button {id}");
    div()
        .id(id)
        .debug_selector(|| selector)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .h(px(30.))
        .px_3()
        .rounded_md()
        .border_1()
        .border_color(rgb(border))
        .bg(bg)
        .text_color(rgb(fg))
        .text_size(px(13.))
        .font_weight(FontWeight::MEDIUM)
        .whitespace_nowrap()
        .cursor_pointer()
        .hover(move |s| s.bg(hover))
        .active(|s| s.opacity(0.85))
        .child(label.into())
}

/// A smaller button, for actions inside a section or a row.
pub fn small(button: Stateful<Div>) -> Stateful<Div> {
    button.h(px(24.)).px_2().text_size(px(12.))
}

/// One option of a segmented control; the selected one stands out.
pub fn segment(id: impl Into<ElementId>, label: impl Into<SharedString>, selected: bool) -> Stateful<Div> {
    let id = id.into();
    let selector = format!("segment {id}");
    div()
        .id(id)
        .debug_selector(|| selector)
        .flex()
        .items_center()
        .h(px(26.))
        .px_3()
        .rounded_sm()
        .text_size(px(13.))
        .whitespace_nowrap()
        .cursor_pointer()
        .when(selected, |d| d.bg(rgb(ACCENT)).text_color(rgb(theme::ON_ACCENT)).font_weight(FontWeight::MEDIUM))
        .when(!selected, |d| d.text_color(rgb(DIM)).hover(|s| s.bg(rgb(theme::HOVER)).text_color(rgb(TEXT))))
        .child(label.into())
}

/// Segments side by side, in one rounded box.
pub fn segmented(segments: impl IntoIterator<Item = Stateful<Div>>) -> Div {
    div()
        .flex()
        .flex_none()
        .gap_0p5()
        .p_0p5()
        .rounded_md()
        .bg(rgb(INPUT))
        .border_1()
        .border_color(rgb(BORDER))
        .children(segments)
}

/// A one-line text field: the text with its cursor while `active`, else the text or `placeholder`.
/// The mouse places the cursor and selects, and first makes it the field being typed into.
pub fn text_field(
    which: TextBox,
    input: &LineInput,
    active: bool,
    placeholder: &str,
    cx: &mut Context<BrainView>,
) -> Stateful<Div> {
    // The placeholder shows while the field is empty even if it has the keys: a dialog's search box
    // has them from the start, and would otherwise be a blank box that doesn't say what it's for.
    let content = if input.is_empty() {
        Line::styled(placeholder, theme::DIM_STYLE)
    } else if active {
        edited(input.text(), input.selection())
    } else {
        Line::raw(input.text())
    };
    let text = content.render();
    let hit = Hit::new(&text, input.text().chars().count());
    let cursor = active.then(|| caret(&text, byte_at(input.text(), input.cursor())));
    let field = div()
        .id(which.id())
        .flex()
        .items_center()
        .h(px(32.))
        .px_2()
        .rounded_md()
        .bg(rgb(INPUT))
        .border_1()
        .border_color(rgb(if active { ACCENT } else { BORDER }))
        .whitespace_nowrap()
        .overflow_hidden()
        .debug_selector(|| which.id().to_string())
        .child(text)
        .children(cursor);
    pointable(field, which, vec![hit], cx)
}

/// An entry as a small colored label: a task's status, or "Note" / "Decision".
pub fn entry_pill(task: &Task) -> Div {
    if task.is_task() {
        status_pill(task.status)
    } else {
        pill(theme::kind_label(task.kind, false), theme::kind_color(task.kind))
    }
}

/// What an entry's status or kind doesn't say, as a small colored label: a parked task waiting for the
/// user's answer, or answered; a decision proposed or rejected.
pub fn state_pill(task: &Task) -> Option<Div> {
    let (label, color) = match (task.asked(), task.decision) {
        (Some(Asked::Waiting(_)), _) => ("Waiting for you", ACCENT),
        (Some(Asked::Answered(_)), _) => ("Answered", theme::GREEN),
        (None, DecisionState::Proposed) => ("Proposed", theme::YELLOW),
        (None, DecisionState::Rejected) => ("Rejected", theme::RED),
        (None, DecisionState::Accepted) => return None,
    };
    Some(pill(label, color))
}

/// A task's status as a small colored label.
pub fn status_pill(status: Status) -> Div {
    pill(theme::status_label(status), theme::status_color(status))
}

/// A small label in `color` on a tint of it.
fn pill(label: &'static str, color: u32) -> Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .h(px(20.))
        .px_2()
        .rounded_full()
        .bg(rgba((color << 8) | 0x26))
        .text_color(rgb(color))
        .text_size(px(12.))
        .font_weight(FontWeight::MEDIUM)
        .child(label)
}

/// A tag as a colored chip.
pub fn tag_chip(board: &Board, tag: &str) -> Div {
    labeled_chip(board, tag, tag.to_string())
}

/// `tag`'s chip, in its color (see [`Board::tag_colors`]), reading `label` instead of the bare name, e.g. `#devops`.
pub fn labeled_chip(board: &Board, tag: &str, label: String) -> Div {
    let (bg, fg) = board.tag_colors(tag).unwrap_or((theme::HOVER, TEXT));
    div()
        .flex_none()
        .px_1p5()
        .rounded_sm()
        .bg(rgb(bg))
        .text_color(rgb(fg))
        .text_size(px(12.))
        .whitespace_nowrap()
        .child(label)
}

/// Tags as chips in a row that wraps.
pub fn tag_chips(board: &Board, tags: &[String]) -> Div {
    div().flex().flex_wrap().gap_1().children(tags.iter().map(|tag| tag_chip(board, tag)))
}

/// A bordered panel.
pub fn card() -> Div {
    div().flex().flex_col().min_h_0().rounded_lg().border_1().border_color(rgb(BORDER)).bg(rgb(theme::SURFACE))
}

/// A section heading with actions on the right: `Comments (2)          [Add]`.
pub fn section(title: impl Into<SharedString>, actions: impl IntoIterator<Item = Stateful<Div>>) -> Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_2()
        .child(div().font_weight(FontWeight::SEMIBOLD).child(title.into()))
        .child(div().flex().gap_1().children(actions))
}

/// Secondary text.
pub fn dim(text: impl Into<SharedString>) -> Div {
    div().text_color(rgb(DIM)).child(text.into())
}

/// Secondary text in the small size: hints under fields, counts, dates.
pub fn dim_small(text: impl Into<SharedString>) -> Div {
    dim(text).text_size(px(12.))
}

/// A tooltip builder for `.tooltip(…)` showing `text`.
pub fn tooltip(text: impl Into<SharedString>) -> impl Fn(&mut Window, &mut GpuiApp) -> AnyView + 'static {
    let text = text.into();
    move |_window, cx| cx.new(|_| Tooltip(text.clone())).into()
}

/// A tooltip's content.
struct Tooltip(SharedString);

impl Render for Tooltip {
    fn render(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .px_2()
            .py_1()
            .rounded_md()
            .bg(rgb(theme::POPUP))
            .border_1()
            .border_color(rgb(BORDER))
            .text_color(rgb(TEXT))
            .text_size(px(12.))
            .font_family(theme::UI_FONT)
            .child(self.0.clone())
    }
}
