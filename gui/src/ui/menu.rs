//! The menu a right click opens on a card or an entry, drawn where the click was, over everything:
//! each row names an action and its key, and choosing it presses that key on the entry.

use super::theme::{self, BORDER, DIM, RED, TEXT};
use super::{BrainView, key_name};
use crate::state::{Board, MenuPicker, Mode};
use gpui::{
    ClickEvent, Context, MouseButton, MouseDownEvent, Pixels, Point, Stateful, deferred, div, prelude::*, px, rgb,
};

/// How wide the menu is.
const WIDTH: f32 = 220.;
/// Height of one row.
const ROW_HEIGHT: f32 = 26.;

/// The menu, if one is open, at `at` (where the right click was).
pub fn draw(board: &Board, at: Option<Point<Pixels>>, cx: &mut Context<BrainView>) -> Option<impl IntoElement> {
    let (Mode::Menu(menu), Some(at)) = (&board.mode, at) else { return None };
    let MenuPicker { key, sel } = *menu;
    let rows = board.menu_items(key).into_iter().enumerate().map(|(index, item)| {
        div()
            .id(("menu", index))
            .debug_selector(move || format!("menu {index}"))
            .flex()
            .items_center()
            .justify_between()
            .gap_4()
            .h(px(ROW_HEIGHT))
            .px_2()
            .rounded_sm()
            .cursor_pointer()
            .text_color(rgb(if item.danger { RED } else { TEXT }))
            .when(index == sel, |d| d.bg(rgb(theme::SELECTED)))
            .when(index != sel, |d| d.hover(|s| s.bg(rgb(theme::HOVER))))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.act(cx, |b| b.choose_menu(index))))
            .child(item.label)
            .child(div().text_size(px(11.)).text_color(rgb(DIM)).child(key_name(item.key)))
    });
    let panel = div()
        .occlude()
        .w(px(WIDTH))
        .p_1()
        .flex()
        .flex_col()
        .rounded_md()
        .border_1()
        .border_color(rgb(BORDER))
        .bg(rgb(theme::POPUP))
        .shadow_lg()
        .text_size(px(13.))
        .on_mouse_down_out(cx.listener(|this, _, _, cx| {
            if matches!(this.board.mode, Mode::Menu(_)) {
                this.act(cx, |b| b.mode = Mode::Normal);
            }
        }))
        .children(rows);
    // Drawn after everything else, so it covers what's under it; moved in if it would leave the window.
    Some(deferred(gpui::anchored().position(at).snap_to_window_with_margin(px(8.)).child(panel)).with_priority(2))
}

/// Opens the menu of `key`'s entry on a right click, where the click was.
pub fn on_right_click(
    element: Stateful<gpui::Div>,
    key: crate::state::Key,
    cx: &mut Context<BrainView>,
) -> Stateful<gpui::Div> {
    element.on_mouse_down(
        MouseButton::Right,
        cx.listener(move |this, event: &MouseDownEvent, _, cx| {
            this.menu_at = Some(event.position);
            this.act(cx, |b| b.open_menu(key));
        }),
    )
}
