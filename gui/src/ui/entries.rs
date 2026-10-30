//! The notes and decisions tabs: a list, newest first; the drawer shows the selected one.

use super::offscreen::Rows;
use super::theme::{self, BORDER, DIM};
use super::widgets::{Kind, card, dim, dim_small, state_pill, tag_chips};
use super::{BrainView, action, markdown, menu};
use crate::state::{Board, Key};
use crate::text::event::KeyCode;
use gpui::{ClickEvent, Context, Div, FontWeight, Stateful, div, prelude::*, px, rgb};
use kanbrain_core::model::{Task, counted};

/// Width of the column of a row's brain, when several are shown, here and in the board's list.
pub(super) const LABEL_WIDTH: f32 = 110.;
/// Width of the column of a row's id.
const ID_WIDTH: f32 = 40.;

/// The list of the current tab's entries, or what to do when there are none.
pub fn draw(board: &Board, rows: &Rows, follow: bool, cx: &mut Context<BrainView>) -> Div {
    let entries = board.entries();
    let kind = board.tab.label().to_lowercase();
    let header = div()
        .flex()
        .items_center()
        .justify_between()
        .px_3()
        .py_2()
        .border_b_1()
        .border_color(rgb(BORDER))
        .child(div().font_weight(FontWeight::SEMIBOLD).child(board.tab.label()))
        .child(dim_small(counted(entries.len(), kind.trim_end_matches('s'), &kind)));
    if entries.is_empty() {
        let (title, hint) = if board.search.is_empty() {
            (format!("No {kind} yet"), "Agents and you add them as you learn and decide things.")
        } else {
            (format!("No matching {kind}"), "Try another search.")
        };
        let empty = div()
            .flex_1()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_2()
            .child(div().text_size(px(18.)).font_weight(FontWeight::SEMIBOLD).child(title))
            .child(dim(hint))
            .child(div().pt_2().child(action("empty-new", "+ New", Kind::Primary, KeyCode::Char('n'), cx)));
        return card().flex_1().child(header).child(empty);
    }
    let follow = entries.iter().position(|(key, _)| Some(*key) == board.selected).filter(|_| follow);
    let mut frame = rows.frame(entries.iter().map(|&(key, _)| Some(key)).collect(), follow);
    let items: Vec<_> = entries
        .iter()
        .enumerate()
        .filter_map(|(row, &(key, entry))| frame.child(|| entry_row(board, row, key, entry, cx).into_any_element()))
        .collect();
    let list = frame
        .finish(div(), items)
        .id("entries")
        .debug_selector(|| "entries".to_string())
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .track_scroll(&rows.scroll);
    card().flex_1().overflow_hidden().child(header).child(list)
}

/// The row of `entry`, the `row`th: its id, title, state and tags, and its text's first line under them.
fn entry_row(board: &Board, row: usize, key: Key, entry: &Task, cx: &mut Context<BrainView>) -> Stateful<Div> {
    let id = entry.id;
    let selected = board.selected == Some(key);
    let first_line = first_line(&entry.description);
    // With several brains, each row names its own in a column of one width, so the titles line up.
    let label = board.source_label(key.src).map(|l| {
        div()
            .w(px(LABEL_WIDTH))
            .flex_none()
            .truncate()
            .text_size(px(12.))
            .text_color(rgb(theme::ACCENT))
            .child(l.to_string())
    });
    let indent = ID_WIDTH + 8. + if label.is_some() { LABEL_WIDTH + 8. } else { 0. };
    let row_element = div()
        .id(("entry", row))
        .debug_selector(move || format!("entry {}/{id}", key.src))
        .flex()
        .flex_col()
        .gap_1()
        .px_3()
        .py_2()
        .border_b_1()
        .border_color(rgb(theme::BG))
        .cursor_pointer()
        .when(selected, |d| d.bg(rgb(theme::SELECTED)))
        .when(!selected, |d| d.hover(|s| s.bg(rgb(theme::HOVER))))
        .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
            this.act(cx, |b| b.click_entry(key, event.click_count()));
        }))
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .children(label)
                .child(dim_small(format!("#{id}")).w(px(ID_WIDTH)).flex_none())
                .child(
                    div()
                        .debug_selector(move || format!("entry {}/{id} title", key.src))
                        .min_w_0()
                        .truncate()
                        .font_weight(FontWeight::MEDIUM)
                        .child(entry.title.clone()),
                )
                .children(state_pill(entry))
                .child(tag_chips(board, &entry.tags).flex_nowrap().flex_none()),
        )
        // Under the title, past the id and the brain's label.
        .children(first_line.map(|line| {
            div()
                .debug_selector(move || format!("entry {}/{id} line", key.src))
                .ml(px(indent))
                .text_size(px(12.))
                .text_color(rgb(DIM))
                .truncate()
                .child(markdown::plain(line))
        }));
    menu::on_right_click(row_element, key, cx)
}

/// The line of `description` a row shows under its title: the first with text, else the first heading, so a body of
/// headings alone still shows something.
pub(super) fn first_line(description: &str) -> Option<&str> {
    let lines = || description.lines().filter(|l| !l.trim().is_empty());
    lines().find(|l| !l.starts_with('#')).or_else(|| lines().next())
}
