//! The board's list layout: the same tasks as the kanban columns, in one list with a section per status,
//! To do · Doing · Parked · Done. A row is one line, so long titles fit and more tasks show at once.
//! Rows are dragged onto a section to change their status, as cards are onto a column.

use super::board::{clock, draggable, drop_target, empty_hint, meta, quick_add, show_all_done, status_header};
use super::entries::LABEL_WIDTH;
use super::theme::{self, DIM, TEXT};
use super::widgets::{self, card, dim, dim_small, tag_chips};
use super::{BrainView, menu};
use crate::state::{Board, COLUMNS, Key};
use gpui::{AnyElement, ClickEvent, Context, Div, ElementId, ScrollHandle, Stateful, div, prelude::*, px, rgb};
use kanbrain_core::model::{Status, Task, fmt_age, now};

/// Every section, one under the other, scrolling together.
pub fn draw(board: &Board, scroll: &ScrollHandle, follow: bool, cx: &mut Context<BrainView>) -> Div {
    let now = now();
    // The scroll handle finds a row by its index among the list's children, so they're kept flat:
    // each section's header, its quick-add field, rows and hints are all children of the list.
    let mut items: Vec<AnyElement> = Vec::new();
    let mut follow_row = None;
    for status in COLUMNS {
        let rows = board.column(status);
        items.push(header(board, status, rows.len(), cx).into_any_element());
        let adding = quick_add(board, status, cx);
        let empty = rows.is_empty() && adding.is_none();
        if let Some(field) = adding {
            follow_row = Some(items.len());
            items.push(field.px_3().py_2().into_any_element());
        }
        for &(key, task) in &rows {
            if follow_row.is_none() && board.selected == Some(key) {
                follow_row = Some(items.len());
            }
            items.push(menu::on_right_click(row(board, key, task, now, cx), key, cx).into_any_element());
        }
        if empty {
            items.push(dim_small(empty_hint(board, status)).px_3().py_2().into_any_element());
        }
        if status == Status::Done
            && let Some(more) = show_all_done(board, cx)
        {
            items.push(div().flex().py_2().child(more).into_any_element());
        }
    }
    if follow && let Some(index) = follow_row {
        scroll.scroll_to_item(index);
    }
    card()
        .flex_1()
        .overflow_hidden()
        .child(div().id("task-list").flex_1().min_h_0().overflow_y_scroll().track_scroll(scroll).children(items))
}

/// A section's header: the status, its count, and `+` to add a task there. Dropping a row on it moves
/// the task to that status.
fn header(board: &Board, status: Status, shown: usize, cx: &mut Context<BrainView>) -> Div {
    let header = status_header(board, status, shown, cx)
        .debug_selector(move || format!("section {}", status.as_str()))
        .bg(rgb(theme::SURFACE));
    drop_target(header, status, cx)
}

/// A task on one line: brain and id, title, tags, what it carries, its clock while running, and how
/// long it's been in its status.
fn row(
    board: &Board,
    key: Key,
    task: &Task,
    now: chrono::DateTime<chrono::Local>,
    cx: &mut Context<BrainView>,
) -> Stateful<Div> {
    let id = task.id;
    let selected = board.selected == Some(key);
    let running = task.running_for(now);
    let row = div()
        // Ids repeat across brains, so the element's id names the brain too, as on the cards.
        .id(ElementId::Name(format!("row {}/{id}", key.src).into()))
        .debug_selector(move || format!("row {}/{id}", key.src))
        .flex()
        .items_center()
        .gap_2()
        .px_3()
        .py_1p5()
        .border_b_1()
        .border_color(rgb(theme::BG))
        .text_size(px(13.))
        .cursor_pointer()
        .when(selected, |d| d.bg(rgb(theme::SELECTED)))
        .when(!selected, |d| d.hover(|s| s.bg(rgb(theme::HOVER))))
        // Over the row rather than a border of it, which would push this row's columns right of the others'.
        .relative()
        .children(
            running
                .is_some()
                .then(|| div().absolute().left_0().top_0().bottom_0().w(px(4.)).bg(rgb(theme::running_color(task)))),
        )
        .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
            this.act(cx, |b| b.click_entry(key, event.click_count()));
        }))
        // In a column of one width, so the titles line up whichever brain each row is in.
        .children(board.source_label(key.src).map(|l| {
            div()
                .w(px(LABEL_WIDTH))
                .flex_none()
                .truncate()
                .text_size(px(12.))
                .text_color(rgb(theme::ACCENT))
                .child(l.to_string())
        }))
        .child(dim_small(format!("#{id}")).w(px(40.)).flex_none())
        .child(
            div()
                .min_w_0()
                .truncate()
                .text_color(rgb(if task.status == Status::Done { DIM } else { TEXT }))
                .child(task.title.clone()),
        )
        .children(widgets::state_pill(task))
        .child(tag_chips(board, &task.tags).flex_nowrap().flex_none())
        .child(div().flex_1())
        .child(dim(meta(board, key, task, now)).text_size(px(11.)).flex_none())
        .children(running.map(|r| clock(r, theme::running_color(task)).text_size(px(11.)).flex_none()))
        .child(dim(fmt_age(now - task.status_since())).text_size(px(11.)).w(px(32.)).flex_none().text_right());
    draggable(row, key, task)
}
