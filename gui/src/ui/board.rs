//! The board: one column per status, To do · Doing · Parked · Done, each a scrolling stack of cards
//! with a `+` for a new task in it. Cards are dragged between columns to change their status.

use super::offscreen::Rows;
use super::pointer::TextBox;
use super::theme::{self, BORDER, DIM, TEXT};
use super::widgets::{self, Kind, card, dim, dim_small, small, state_pill, tag_chips};
use super::{BrainView, menu, scrollbar, truncate_words};
use crate::state::{Board, COLUMNS, Key, Mode};
use gpui::{
    Axis, ClickEvent, Context, Div, ElementId, FontWeight, Pixels, ScrollHandle, SharedString, Stateful, Window, div,
    prelude::*, px, rgb,
};
use kanbrain_core::model::{Asked, Status, Task, counted, fmt_age, fmt_clock, fmt_dur, now};
use std::path::PathBuf;

/// Characters of a card's title, and of the question it waits on, shown before they're cut at a word's end: about
/// three lines of a column at its narrowest. Cut by characters rather than by GPUI's line clamp, which cuts mid-word.
const TITLE_CHARS: usize = 90;
/// See [`TITLE_CHARS`].
const QUESTION_CHARS: usize = 110;

/// Narrowest a column gets before the board scrolls sideways.
const COLUMN_MIN_WIDTH: f32 = 220.;

/// How thick a scrollbar's strip is: a column's padding (`p_2`, half a rem), which it's drawn in, at GPUI's default
/// 16px rem, which the app keeps.
const BAR: Pixels = px(8.);
/// Space between the columns and the board's scrollbar under them.
const BAR_GAP: Pixels = px(2.);

/// A card being dragged: which task, and the title its preview shows under the pointer.
pub struct DraggedCard {
    /// The task's brain, by its folder: a reload while it's dragged may move the brain in the list, and an index
    /// would then name another.
    dir: PathBuf,
    /// The task's id there.
    id: u32,
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
            .text_ellipsis()
            .child(self.title.clone())
    }
}

/// The four columns side by side.
pub fn draw(
    board: &Board,
    scrolls: &[Rows; COLUMNS.len()],
    sideways: &ScrollHandle,
    follow: bool,
    cx: &mut Context<BrainView>,
) -> Div {
    // The selected card's column comes into view, as its card does in the column.
    if follow && let Some(at) = board.selected_task().and_then(|t| COLUMNS.iter().position(|&s| s == t.status)) {
        sideways.scroll_to_item(at);
    }
    let columns = COLUMNS.iter().zip(scrolls).map(|(&status, rows)| column(board, status, rows, follow, cx));
    let columns: Vec<_> = columns.collect();
    let row = each_axis(div().id("board")).size_full().track_scroll(sideways).flex().gap_3().children(columns);
    // The columns and the bar under them.
    let over = scrollbar::Over::default();
    let region = div().absolute().top_0().left_0().right_0().bottom(-BAR_GAP - BAR).child(over.mark());
    // Under the columns, in the padding between them and the status bar.
    let bar = scrollbar::bar(sideways, Axis::Horizontal, "scrollbar board", &over)
        .debug_selector(|| "scrollbar board".to_string())
        .absolute()
        .left_0()
        .right_0()
        .bottom(-BAR_GAP - BAR)
        .h(BAR);
    div().relative().flex_1().min_h_0().min_w_0().flex().child(region).child(row).child(bar)
}

/// Makes `area` scroll each way by the wheel or touchpad's own movement that way, and only that way: the board
/// sideways, its columns up and down.
///
/// An area GPUI scrolls one way only also takes movement the other way as its own, so a vertical wheel over a column
/// scrolled the board sideways as well, and a horizontal one the column. Scrolling both ways stops that: the way an
/// area doesn't overflow can't move. GPUI's `restrict_scroll_to_axis` stops it too, but locks a touchpad's swipe to
/// the way it mostly goes, where a slanting one moves both the board and the column. Shift and the wheel need nothing
/// here: each platform hands them over already turned sideways (GPUI on Linux and Windows, the system on macOS).
fn each_axis(area: Stateful<Div>) -> Stateful<Div> {
    let mut area = area.overflow_scroll();
    area.style().allow_concurrent_scroll = Some(true);
    area
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
fn column(board: &Board, status: Status, rows: &Rows, follow: bool, cx: &mut Context<BrainView>) -> Div {
    let scroll = &rows.scroll;
    let cards = board.column(status);
    let adding = quick_add(board, status, cx);
    // The field is the first child, above the cards: in view while typing, with each new task landing
    // right under it, since the newest move comes first.
    let row = if adding.is_some() { Some(0) } else { cards.iter().position(|(key, _)| Some(*key) == board.selected) };
    let header = status_header(board, status, cards.len(), cx);
    let now = now();
    let empty = (cards.is_empty() && adding.is_none()).then(|| dim_small(empty_hint(board, status)).p_2());
    let more = (status == Status::Done).then(|| show_all_done(board, cx)).flatten();
    let other = |shown: bool| std::iter::repeat_n(None, usize::from(shown));
    let cards_keys = cards.iter().map(|&(key, _)| Some(key));
    let order = other(adding.is_some()).chain(cards_keys).chain(other(empty.is_some())).chain(other(more.is_some()));
    let mut frame = rows.frame(order.collect(), row.filter(|_| follow));
    let mut children: Vec<_> =
        adding.into_iter().filter_map(|field| frame.child(|| field.into_any_element())).collect();
    children.extend(cards.iter().filter_map(|&(key, task)| {
        frame.child(|| menu::on_right_click(draw_card(board, key, task, now, cx), key, cx).into_any_element())
    }));
    children.extend(empty.and_then(|hint| frame.child(|| hint.into_any_element())));
    children.extend(more.and_then(|more| frame.child(|| more.into_any_element())));
    let body = frame
        .finish(div())
        .id(("column", status as usize))
        .debug_selector(move || format!("column {}", status.as_str()))
        .flex_1()
        .min_h_0()
        .track_scroll(scroll)
        .flex()
        .flex_col()
        .gap_2()
        .p_2()
        .children(children);
    // The whole column, its header too.
    let over = scrollbar::Over::default();
    // In the cards' right padding, beside them.
    let bar = scrollbar::bar(scroll, Axis::Vertical, ("scrollbar column", status as usize), &over)
        .debug_selector(move || format!("scrollbar column {}", status.as_str()))
        .absolute()
        .top_0()
        .bottom_0()
        .right_0()
        .w(BAR);
    let body = div().relative().flex_1().min_h_0().flex().flex_col().child(each_axis(body)).child(bar);
    drop_target(card().relative().flex_1().min_w(px(COLUMN_MIN_WIDTH)), status, cx)
        .child(over.mark())
        .child(header)
        .child(body)
}

/// Makes `target` take dropped cards, moving them to `status`, and light up while one is over it.
pub(super) fn drop_target(target: Div, status: Status, cx: &mut Context<BrainView>) -> Div {
    target
        .drag_over::<DraggedCard>(|style, _, _, _| style.border_color(rgb(theme::ACCENT)).bg(rgb(theme::HOVER)))
        .on_drop(cx.listener(move |this, card: &DraggedCard, _, cx| {
            this.act(cx, |b| b.drop_dragged(&card.dir, card.id, status));
        }))
}

/// Makes `element`, which shows `task`, draggable onto a column or a list's section.
pub(super) fn draggable(element: Stateful<Div>, board: &Board, key: Key, task: &Task) -> Stateful<Div> {
    let dir = board.brain(key.src).store.dir().to_path_buf();
    element.on_drag(DraggedCard { dir, id: key.id, title: task.title.clone().into() }, |card, _, _, cx| {
        cx.new(|_| DragPreview { title: card.title.clone() })
    })
}

/// What an empty column is for; or, when the search hides every task it has, that it does. An empty Doing with one
/// project or brain shown says where to look, since a task running in another shows nowhere else.
pub(super) fn empty_hint(board: &Board, status: Status) -> String {
    let hidden = board.column_total(status);
    if hidden > 0 {
        return format!("{} here, none matching the search.", counted(hidden, "task", "tasks"));
    }
    match status {
        Status::Todo => "Nothing to do yet. Press + to add a task.",
        Status::Doing if board.scope.is_empty() => "Nothing running. Drop a card here to start its clock.",
        Status::Doing => "Nothing running here: All shows what runs elsewhere. Drop a card here to start its clock.",
        Status::Parked => "Started tasks set aside land here.",
        Status::Done => "Finished tasks land here.",
    }
    .to_string()
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
        .when(task.waits_for_answer(), |d| d.border_l_4().border_color(rgb(theme::ACCENT)))
        .cursor_pointer()
        .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
            this.act(cx, |b| b.click_entry(key, event.click_count()));
        }));
    draggable(card, board, key, task)
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
                .text_color(rgb(title_color(task.status)))
                .child(truncate_words(&task.title, TITLE_CHARS)),
        )
        .when(!task.tags.is_empty(), |d| {
            d.child(tag_chips(board, &task.tags).debug_selector(move || format!("tags {}/{}", key.src, key.id)))
        })
        .children(asked(task).map(|d| d.debug_selector(move || format!("asked {}/{}", key.src, key.id))))
        .child(footer)
}

/// A task's title color, on its card or its row: dimmed once done, as what's no longer to do.
pub(super) fn title_color(status: Status) -> u32 {
    if status == Status::Done { DIM } else { TEXT }
}

/// The question a parked task waits on, or its answer, for its card: what the person watching the board
/// has to act on, without opening it.
fn asked(task: &Task) -> Option<Div> {
    let comment = match task.asked()? {
        Asked::Waiting(comment) | Asked::Answered(comment) => comment,
    };
    let first = comment.first_line();
    Some(
        div()
            .flex()
            .flex_col()
            .items_start()
            .gap_1()
            .children(state_pill(task))
            // As wide as the card: at the text's own width it ran past the card's edge.
            .child(div().w_full().text_size(px(12.)).text_color(rgb(DIM)).child(truncate_words(first, QUESTION_CHARS))),
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
    Some(
        small(widgets::button("all-done", label, Kind::Ghost))
            .debug_selector(|| "all done".to_string())
            .self_center()
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.act(cx, |b| {
                    b.all_done = !b.all_done;
                    b.keep_selection();
                });
            })),
    )
}
