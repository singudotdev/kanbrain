//! The board: one column per status, To do · Doing · Parked · Done, each a scrolling stack of cards
//! with a `+` for a new task in it. Cards are dragged between columns to change their status.

use super::pointer::TextBox;
use super::theme::{self, BORDER, DIM, TEXT};
use super::widgets::{self, Kind, card, dim, dim_small, small, state_pill, tag_chips};
use super::{BrainView, menu};
use crate::state::{Board, COLUMNS, Key, Mode};
use gpui::{
    ClickEvent, Context, Div, ElementId, FontWeight, ScrollHandle, SharedString, Stateful, Window, div, prelude::*, px,
    rgb,
};
use kanbrain_core::model::{Asked, Status, Task, fmt_age, fmt_clock, fmt_dur, now};

/// Narrowest a column gets before the board scrolls sideways.
const COLUMN_MIN_WIDTH: f32 = 220.;

/// A card being dragged: which task, and the title its preview shows under the pointer.
pub struct DraggedCard {
    /// The task.
    key: Key,
    /// The task's title.
    title: SharedString,
}

/// What follows the pointer while a card is dragged.
struct DragPreview {
    /// The dragged task's title.
    title: SharedString,
}

impl Render for DragPreview {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w(px(COLUMN_MIN_WIDTH))
            .p_2p5()
            .rounded_md()
            .border_1()
            .border_color(rgb(theme::ACCENT))
            .bg(rgb(theme::SELECTED))
            .text_size(px(13.))
            .text_color(rgb(TEXT))
            .opacity(0.9)
            .line_clamp(2)
            .child(self.title.clone())
    }
}

/// The four columns side by side.
pub fn draw(board: &Board, scrolls: &[ScrollHandle; COLUMNS.len()], follow: bool, cx: &mut Context<BrainView>) -> Div {
    let columns = COLUMNS.iter().zip(scrolls).map(|(&status, scroll)| column(board, status, scroll, follow, cx));
    let columns: Vec<_> = columns.collect();
    div().flex_1().min_h_0().flex().gap_3().children(columns)
}

/// A column's or a list section's header: the status, how many of its tasks are shown, and `+` to add one there.
pub(super) fn status_header(board: &Board, status: Status, shown: usize, cx: &mut Context<BrainView>) -> Div {
    let total = board.column_total(status);
    let count = if shown == total { total.to_string() } else { format!("{shown} of {total}") };
    div()
        .flex()
        .items_center()
        .gap_2()
        .px_3()
        .py_2()
        .border_b_1()
        .border_color(rgb(BORDER))
        .child(div().size(px(8.)).rounded_full().bg(rgb(theme::status_color(status))))
        .child(div().font_weight(FontWeight::SEMIBOLD).child(theme::status_label(status)))
        .child(dim_small(count))
        // Beside the count, not at the header's end: closing the details widens the board and brought the last
        // column's + under the pointer, which clicked it as the details closed.
        .child(
            small(widgets::button(("add", status as usize), "+", Kind::Ghost))
                .debug_selector(move || format!("add {}", status.as_str()))
                .tooltip(widgets::tooltip(if status == Status::Todo {
                    "New task here · Shortcut: n"
                } else {
                    "New task here"
                }))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.act(cx, |b| b.new_entry(status)))),
        )
}

/// A column: its name and count, the quick-add field while adding there, and its cards.
fn column(board: &Board, status: Status, scroll: &ScrollHandle, follow: bool, cx: &mut Context<BrainView>) -> Div {
    let cards = board.column(status);
    let adding = quick_add(board, status, cx);
    // The field is the first child, above the cards: in view while typing, with each new task landing
    // right under it, since the newest move comes first.
    let row = if adding.is_some() { Some(0) } else { cards.iter().position(|(key, _)| Some(*key) == board.selected) };
    if follow && let Some(row) = row {
        scroll.scroll_to_item(row);
    }
    let header = status_header(board, status, cards.len(), cx);
    let now = now();
    let rows: Vec<_> = cards
        .iter()
        .map(|&(key, task)| menu::on_right_click(draw_card(board, key, task, now, cx), key, cx).into_any_element())
        .collect();
    let empty = (cards.is_empty() && adding.is_none()).then(|| dim_small(empty_hint(status)).p_2());
    let body = div()
        .id(("column", status as usize))
        .debug_selector(move || format!("column {}", status.as_str()))
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .track_scroll(scroll)
        .flex()
        .flex_col()
        .gap_2()
        .p_2()
        .children(adding)
        .children(rows)
        .children(empty)
        .children((status == Status::Done).then(|| show_all_done(board, cx)).flatten());
    drop_target(card().flex_1().min_w(px(COLUMN_MIN_WIDTH)), status, cx).child(header).child(body)
}

/// Makes `target` take dropped cards, moving them to `status`, and light up while one is over it.
pub(super) fn drop_target(target: Div, status: Status, cx: &mut Context<BrainView>) -> Div {
    target
        .drag_over::<DraggedCard>(|style, _, _, _| style.border_color(rgb(theme::ACCENT)).bg(rgb(theme::HOVER)))
        .on_drop(cx.listener(move |this, card: &DraggedCard, _, cx| this.act(cx, |b| b.drop_card(card.key, status))))
}

/// Makes `element`, which shows `task`, draggable onto a column or a list's section.
pub(super) fn draggable(element: Stateful<Div>, key: Key, task: &Task) -> Stateful<Div> {
    element.on_drag(DraggedCard { key, title: task.title.clone().into() }, |card, _, _, cx| {
        cx.new(|_| DragPreview { title: card.title.clone() })
    })
}

/// What an empty column is for.
pub(super) fn empty_hint(status: Status) -> &'static str {
    match status {
        Status::Todo => "Nothing to do yet. Press + to add a task.",
        Status::Doing => "Nothing running. Drop a card here to start its clock.",
        Status::Parked => "Started tasks set aside land here.",
        Status::Done => "Finished tasks land here.",
    }
}

/// A card: id (and brain, when several are shown), age in its column, title, tags, and what it
/// carries (comments, links, time).
fn draw_card(
    board: &Board,
    key: Key,
    task: &Task,
    now: chrono::DateTime<chrono::Local>,
    cx: &mut Context<BrainView>,
) -> Stateful<Div> {
    let id = task.id;
    let brain = board.brain(key.src);
    let selected = board.selected == Some(key);
    let running = task.running_for(now);
    let footer = div()
        .flex()
        .items_center()
        .gap_2()
        .text_size(px(11.))
        .text_color(rgb(DIM))
        .child(meta(board, key, task, now))
        .children(running.map(|r| clock(r, theme::running_color(task))));
    let card = div()
        // Ids repeat across brains, so the element's id names the brain too: two cards with one id
        // would share GPUI's hit-testing and click state, and only the first would take clicks.
        .id(ElementId::Name(format!("card {}/{id}", key.src).into()))
        .debug_selector(move || format!("card {}/{id}", key.src))
        .flex()
        .flex_none()
        .flex_col()
        .gap_1p5()
        .p_2p5()
        .rounded_md()
        .border_1()
        .border_color(rgb(if selected { theme::ACCENT } else { BORDER }))
        .bg(rgb(if selected { theme::SELECTED } else { theme::BG }))
        .when(!selected, |d| d.hover(|s| s.bg(rgb(theme::HOVER))))
        .when(running.is_some(), |d| d.border_l_4().border_color(rgb(theme::running_color(task))))
        .when(matches!(task.asked(), Some(Asked::Waiting(_))), |d| d.border_l_4().border_color(rgb(theme::ACCENT)))
        .cursor_pointer()
        .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
            this.act(cx, |b| b.click_entry(key, event.click_count()));
        }));
    draggable(card, key, task)
        .child(
            div()
                .flex()
                .justify_between()
                .text_size(px(11.))
                .text_color(rgb(DIM))
                .child(
                    div()
                        .flex()
                        .gap_1p5()
                        .children(
                            board
                                .source_label(key.src)
                                .map(|l| div().text_color(rgb(theme::ACCENT)).child(l.to_string())),
                        )
                        .child(format!("#{id}")),
                )
                .child(fmt_age(now - task.status_since())),
        )
        .child(
            div()
                .text_size(px(13.))
                .line_clamp(3)
                .text_color(rgb(if task.status == Status::Done { DIM } else { TEXT }))
                .child(task.title.clone()),
        )
        .when(!task.tags.is_empty(), |d| d.child(tag_chips(brain, &task.tags)))
        .children(asked(task))
        .child(footer)
}

/// The question a parked task waits on, or its answer, for its card: what the person watching the board
/// has to act on, without opening it.
fn asked(task: &Task) -> Option<Div> {
    let comment = match task.asked()? {
        Asked::Waiting(comment) | Asked::Answered(comment) => comment,
    };
    let first = comment.body.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or_default();
    Some(
        div()
            .flex()
            .flex_col()
            .items_start()
            .gap_1()
            .children(state_pill(task))
            .child(div().text_size(px(12.)).text_color(rgb(DIM)).line_clamp(2).child(first.to_string())),
    )
}

/// What a task carries, for its card or row: `2 comments · 1 link · 1h 05m`. The time is left out while
/// it runs, since [`clock`] shows it then.
pub(super) fn meta(board: &Board, key: Key, task: &Task, now: chrono::DateTime<chrono::Local>) -> String {
    let links = board.related_keys(key).len();
    // Words rather than emoji: GPUI has no emoji fallback, so without an emoji font installed they draw as boxes.
    let meta = [
        (!task.comments.is_empty()).then(|| count(task.comments.len(), "comment")),
        (links > 0).then(|| count(links, "link")),
        (!task.sessions.is_empty() && task.running_for(now).is_none()).then(|| fmt_dur(task.total(now))),
    ];
    meta.into_iter().flatten().collect::<Vec<_>>().join(" · ")
}

/// A running task's clock, in `color` (see [`theme::running_color`]).
pub(super) fn clock(running: chrono::Duration, color: u32) -> Div {
    div().text_color(rgb(color)).font_family(theme::EDITOR_FONT).child(format!("▶ {}", fmt_clock(running)))
}

/// `n` things, in the singular for one: `1 comment`, `2 comments`.
fn count(n: usize, thing: &str) -> String {
    if n == 1 { format!("1 {thing}") } else { format!("{n} {thing}s") }
}

/// The field a new task is typed into at the top of `status`'s column or section, while adding there.
pub(super) fn quick_add(board: &Board, status: Status, cx: &mut Context<BrainView>) -> Option<Div> {
    let Mode::QuickAdd(add) = &board.mode else { return None };
    if add.status != status {
        return None;
    }
    Some(
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(widgets::text_field(TextBox::QuickAdd, &add.title, true, "Title, then Enter", cx))
            .child(dim("Enter adds it and keeps typing · Esc stops").text_size(px(11.))),
    )
}

/// "Show all N done" when the Done column is cut short, and "Show latest" to cut it again.
pub(super) fn show_all_done(board: &Board, cx: &mut Context<BrainView>) -> Option<Stateful<Div>> {
    let hidden = board.hidden_done();
    let label = match (board.all_done, hidden) {
        (false, 0) => return None,
        (false, n) => format!("Show {n} older"),
        (true, _) if board.column_total(Status::Done) <= crate::state::DONE_LIMIT => return None,
        (true, _) => "Show only the latest".to_string(),
    };
    Some(small(widgets::button("all-done", label, Kind::Ghost)).self_center().on_click(cx.listener(
        |this, _: &ClickEvent, _, cx| {
            this.act(cx, |b| {
                b.all_done = !b.all_done;
                b.keep_selection();
            });
        },
    )))
}
