//! The details drawer: everything about the selected entry, beside the board or list, with a button
//! for each thing that can be done to it. Its text is selected with the mouse to copy, each part on its own: the
//! title, the tags, the question, the description, each comment and change, the history.

use super::shown::{self, Selectable};
use super::theme::{self, BORDER, DIM_STYLE, Line, Span};
use super::widgets::{
    self, Kind, card, dim, dim_small, entry_pill, labeled_chip, section, segment, segmented, small, state_pill,
};
use super::{BrainView, action, entry_action, markdown, when};
use crate::state::{Board, COLUMNS, Key, Target, When};
use crate::text::event::KeyCode;
use gpui::{ClickEvent, Context, Div, ElementId, FontWeight, ScrollHandle, Stateful, div, prelude::*, px, rgb};
use kanbrain_core::model::{
    Activity, Asked, Author, Comment, DecisionState, Kind as EntryKind, StatusChange, Task, ThreadItem, counted,
    fmt_age, fmt_clock, fmt_dur, now,
};
use kanbrain_core::period::{Period, Unit};

/// The drawer for `task`, which `key` names.
pub fn draw(board: &Board, key: Key, task: &Task, scroll: &ScrollHandle, cx: &mut Context<BrainView>) -> Div {
    let content = div()
        .id("drawer")
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .track_scroll(scroll)
        .flex()
        .flex_col()
        .gap_4()
        .p_4()
        .child(heading(board, key, task, cx))
        .children(asked_block(board, key, task, cx))
        .children(task.is_task().then(|| status_block(board, key, task, cx)))
        .children(task.is_task().then(|| time_block(board, key, task, cx)))
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_2()
                .child(named(board, "edit", 'e', Kind::Secondary, cx))
                .child(named(board, "relate", 'r', Kind::Secondary, cx))
                .when(task.kind == EntryKind::Decision && task.decision != DecisionState::Accepted, |d| {
                    d.child(named(board, "accept", 'y', Kind::Primary, cx)).child(named(
                        board,
                        "accept-comment",
                        'Y',
                        Kind::Secondary,
                        cx,
                    ))
                })
                .when(task.kind == EntryKind::Decision && task.decision != DecisionState::Rejected, |d| {
                    d.child(named(board, "reject", 'j', Kind::Secondary, cx))
                })
                .child(named(board, "open-file", 'o', Kind::Secondary, cx))
                .when(board.sources.len() > 1, |d| d.child(named(board, "move", 'M', Kind::Secondary, cx)))
                .child(named(board, "delete", 'x', Kind::Danger, cx)),
        )
        .child(divider())
        .child(description_block(board, key, task, cx))
        .child(related_block(board, key, cx))
        .child(comments_block(board, key, task, cx))
        .children(task.is_task().then(|| history_block(board, key, task, cx)));
    card().child(content)
}

/// A button for what `key` does to the selected entry, named as the right-click menu names it, from the same table.
fn named(board: &Board, id: &'static str, key: char, kind: Kind, cx: &mut Context<BrainView>) -> Stateful<Div> {
    entry_action(id, board.label_of(KeyCode::Char(key)), kind, KeyCode::Char(key), cx)
}

/// Kind or status, the project it's in, id, a close button; the title, when it was created, and the tags.
fn heading(board: &Board, key: Key, task: &Task, cx: &mut Context<BrainView>) -> Div {
    let place = board.sources[key.src].label.clone();
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(entry_pill(task))
                .children(state_pill(task))
                .children(place.map(|p| div().text_size(px(12.)).text_color(rgb(theme::ACCENT)).child(p)))
                .child(dim_small(format!("#{}", task.id)))
                .child(div().flex_1())
                .child(
                    small(action("close-drawer", "✕", Kind::Ghost, KeyCode::Esc, cx))
                        .debug_selector(|| "close drawer".into()),
                ),
        )
        .child(
            shown::text(board, region(key, "title"), Line::raw(&task.title), cx)
                .text_size(px(18.))
                .font_weight(FontWeight::SEMIBOLD),
        )
        .child(small_dim_text(board, region(key, "created"), format!("created {}", when(task.created)), cx))
        .when(!task.tags.is_empty(), |d| d.child(tags(board, key, &task.tags, cx).pt_1()))
}

/// The name of the text `what` of `key`'s entry, among those the window shows.
fn region(key: Key, what: &str) -> String {
    format!("{}/{} {what}", key.src, key.id)
}

/// Secondary text in the small size, as [`dim_small`], the text `region` the mouse selects.
fn small_dim_text(board: &Board, region: String, text: String, cx: &mut Context<BrainView>) -> Div {
    dim_small(shown::text(board, region, Line::raw(text), cx))
}

/// The entry's tags as chips in a row that wraps, as [`widgets::tag_chips`], selected with the mouse as text: a tag
/// on each line.
fn tags(board: &Board, key: Key, tags: &[String], cx: &mut Context<BrainView>) -> Div {
    let mut text = Selectable::new(board, region(key, "tags"));
    let chips: Vec<_> = tags.iter().map(|tag| labeled_chip(board, tag, text.line(Line::raw(tag)))).collect();
    text.finish(div().flex().flex_wrap().gap_1().children(chips), cx)
}

/// The question an agent asked and the task waits on, with a button to answer it and one to open each
/// decision it proposed for it; or that it's answered, and the agent picks it up from there.
fn asked_block(board: &Board, key: Key, task: &Task, cx: &mut Context<BrainView>) -> Option<Div> {
    let callout = div().flex().flex_col().gap_2().p_3().rounded_md().border_1();
    match task.asked()? {
        Asked::Waiting(question) => {
            let proposed = board.proposed_for(key);
            // Waiting on a proposed decision, it's answered by deciding on it, from here.
            let decide: Vec<_> = if proposed.is_empty() {
                vec![named(board, "answer", 'c', Kind::Primary, cx)]
            } else {
                vec![
                    named(board, "accept", 'y', Kind::Primary, cx),
                    named(board, "accept-comment", 'Y', Kind::Secondary, cx),
                    named(board, "reject", 'j', Kind::Secondary, cx),
                ]
            };
            let open = proposed.into_iter().map(|k| {
                small(widgets::button(
                    ElementId::Name(format!("open-proposed {}/{}", k.src, k.id).into()),
                    format!("Open decision #{}", k.id),
                    Kind::Secondary,
                ))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.act(cx, |b| b.open_entry(k))))
            });
            let buttons = div().flex().flex_wrap().gap_2().children(decide.into_iter().map(small)).children(open);
            let mut text = Selectable::new(board, region(key, "question"));
            let asks = text.line(Line::raw(format!("{} asks, {}", who(question.author), when(question.created))));
            let body = markdown::render("question", &question.body, &mut text);
            let asked = div().flex().flex_col().gap_2().child(dim_small(asks));
            Some(
                callout
                    .border_color(rgb(theme::ACCENT))
                    .bg(rgb(theme::SELECTED))
                    .child(text.finish(asked.child(body), cx))
                    .child(buttons),
            )
        }
        Asked::Answered(answer) => Some(
            callout
                .border_color(rgb(BORDER))
                .child(dim_small(format!("Answered {}: the agent picks it up from here.", when(answer.created)))),
        ),
    }
}

/// Who wrote a comment or made a change, for people: "An agent" or "You".
pub(super) fn who(author: Author) -> &'static str {
    match author {
        Author::Agent => "An agent",
        Author::Human => "You",
    }
}

/// The status buttons, the current one selected, and how long the task has had it.
fn status_block(board: &Board, key: Key, task: &Task, cx: &mut Context<BrainView>) -> Div {
    let segments = COLUMNS.map(|status| {
        segment(("status", status as usize), theme::status_label(status), task.status == status)
            .debug_selector(move || format!("drawer status {}", status.as_str()))
            .when_some(board.shortcut(When::MoveTo(status)), |segment, key| {
                segment.tooltip(widgets::tooltip(format!("Shortcut: {key}")))
            })
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.act(cx, |b| b.set_status(key, status))))
    });
    let since = task.status_since();
    let since = format!("{} for {}, since {}", theme::status_label(task.status), fmt_age(now() - since), when(since));
    div().flex().flex_col().gap_1p5().child(segmented(segments).self_start()).child(small_dim_text(
        board,
        region(key, "status"),
        since,
        cx,
    ))
}

/// Time tracked on the task: the running clock, today, this week and in total.
fn time_block(board: &Board, key: Key, task: &Task, cx: &mut Context<BrainView>) -> Div {
    let now = now();
    let within = |unit| {
        let (from, to) = Period::current(unit).bounds();
        fmt_dur(task.time_between(from, to, now))
    };
    let mut text = format!(
        "Tracked today {} · this week {} · total {} in {}",
        within(Unit::Day),
        within(Unit::Week),
        fmt_dur(task.total(now)),
        counted(task.sessions.len(), "session", "sessions")
    );
    if let Some(running) = task.running_for(now) {
        text = format!("Running {} · {text}", fmt_clock(running));
    }
    small_dim_text(board, region(key, "time"), text, cx)
}

/// The whole description, and a button to edit it.
fn description_block(board: &Board, key: Key, task: &Task, cx: &mut Context<BrainView>) -> Div {
    let edit = small(entry_action("edit-description", "Edit", Kind::Secondary, KeyCode::Char('m'), cx));
    let body = if task.description.is_empty() {
        dim("No description yet.")
    } else {
        let mut text = Selectable::new(board, region(key, "description"));
        let body = markdown::render("description", &task.description, &mut text);
        text.finish(body, cx)
    };
    div().flex().flex_col().gap_2().child(section("Description", [edit])).child(body)
}

/// The entries linked to this one, whichever side stores the link, each opening when clicked: those of
/// its brain, then those of the project's other repositories, named by their repository.
fn related_block(board: &Board, key: Key, cx: &mut Context<BrainView>) -> Div {
    let related: Vec<_> = board.related_keys(key).into_iter().filter_map(|k| Some((k, board.task(k)?))).collect();
    let link = small(entry_action("link", "Link", Kind::Secondary, KeyCode::Char('r'), cx));
    let rows: Vec<_> = related
        .iter()
        .map(|&(other_key, other)| {
            let id = other.id;
            div()
                .id(ElementId::Name(format!("related {}/{id}", other_key.src).into()))
                .debug_selector(move || format!("related {}/{id}", other_key.src))
                .flex()
                .items_center()
                .gap_2()
                .px_1()
                .py_0p5()
                .rounded_sm()
                .cursor_pointer()
                .hover(|s| s.bg(rgb(theme::HOVER)))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.act(cx, |b| b.open_entry(other_key))))
                .child(entry_pill(other))
                .child(div().min_w_0().truncate().child(format!(
                    "{} {}",
                    board.link_name(key.src, other_key),
                    other.title
                )))
        })
        .collect();
    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_size(px(13.))
        .child(section(format!("Related ({})", related.len()), [link]))
        .when(related.is_empty(), |d| d.child(dim("Nothing linked yet.")))
        .children(rows)
}

/// The thread: every comment, each with Edit and Delete, and every change to the title, tags, related entries and
/// description, one line each, oldest first; and a button to add a comment.
fn comments_block(board: &Board, key: Key, task: &Task, cx: &mut Context<BrainView>) -> Div {
    let add = small(entry_action("add-comment", "Add", Kind::Secondary, KeyCode::Char('c'), cx));
    let thread: Vec<_> = task
        .thread()
        .into_iter()
        .enumerate()
        .map(|(at, item)| match item {
            ThreadItem::Comment(index, comment) => comment_card(board, key, index, comment, cx),
            ThreadItem::Change(change) => change_line(board, key, at, change, cx),
        })
        .collect();
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(section(format!("Comments ({})", task.comments.len()), [add]))
        .when(task.comments.is_empty(), |d| d.child(dim("No comments yet.")))
        .children(thread)
}

/// Comment `index` of `key`'s entry, with Edit and Delete: its heading and its text one text the mouse selects.
fn comment_card(board: &Board, key: Key, index: usize, comment: &Comment, cx: &mut Context<BrainView>) -> Div {
    let edit = small(widgets::button(("edit-comment", index), "Edit", Kind::Ghost))
        .tooltip(widgets::tooltip("Shortcut: C, then Enter on it"))
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
            this.act(cx, |b| b.open_editor(Target::Comment(key, index)));
        }));
    let delete = small(widgets::button(("delete-comment", index), "Delete", Kind::Ghost))
        .tooltip(widgets::tooltip("Shortcut: C, then x on it"))
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
            this.act(cx, |b| b.ask_delete_comment(key, index));
        }));
    let mut text = Selectable::new(board, region(key, &format!("comment {index}")));
    let label = text.line(Line::raw(comment_label(comment)));
    let body = markdown::render(format!("comment {index}"), &comment.body, &mut text);
    let card = div()
        .flex()
        .flex_col()
        .gap_1()
        .p_2()
        .rounded_md()
        .bg(rgb(theme::BG))
        .child(
            div()
                .flex()
                .items_center()
                .gap_1()
                // It wraps rather than push the buttons past the panel's edge, as an edit's date did.
                .child(dim_small(label).flex_1().min_w_0())
                .child(edit)
                .child(delete),
        )
        .child(body);
    text.finish(card, cx).debug_selector(move || format!("thread comment {index}"))
}

/// What a comment's card says above it: an agent's kind of comment, when, and when it was edited.
pub(super) fn comment_label(comment: &Comment) -> String {
    let edited = comment.edited.map(|at| format!(" · edited {}", when(at))).unwrap_or_default();
    let what = if comment.question { "question" } else { "comment" };
    let by = if comment.author == Author::Agent { format!("Agent {what} · ") } else { String::new() };
    format!("{by}{}{edited}", when(comment.created))
}

/// A change in the thread, the `at`th item, as a line between the comments.
fn change_line(board: &Board, key: Key, at: usize, change: &Activity, cx: &mut Context<BrainView>) -> Div {
    small_dim_text(board, region(key, &format!("change {at}")), change_label(change), cx)
        .px_2()
        .debug_selector(move || format!("thread change {at}"))
}

/// What a change's line says: who did what, and when.
pub(super) fn change_label(change: &Activity) -> String {
    format!("{} {} · {}", who(change.by), change.change.said(), when(change.at))
}

/// Every status change, newest first: one text the mouse selects, a change on each line.
fn history_block(board: &Board, key: Key, task: &Task, cx: &mut Context<BrainView>) -> Div {
    let mut text = Selectable::new(board, region(key, "history"));
    let changes: Vec<_> = task.history.iter().rev().map(|change| text.line(history_line(change))).collect();
    let changes = text.finish(div().flex().flex_col().gap_1().children(changes), cx);
    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_size(px(12.))
        .child(section("History", []))
        .when(task.history.is_empty(), |d| d.child(dim("No status changes yet.")))
        .child(changes)
}

/// `Mon 28 Sep 2026 10:12  To do → Doing`, the new status in its color.
fn history_line(change: &StatusChange) -> Line {
    Line::from(vec![
        Span::styled(format!("{}  ", when(change.at)), DIM_STYLE),
        Span::raw(format!("{} → ", theme::status_label(change.from))),
        Span::styled(theme::status_label(change.to), theme::status(change.to)),
        Span::styled(if change.by == Author::Agent { "  · agent" } else { "" }, DIM_STYLE),
    ])
}

/// A thin horizontal line.
fn divider() -> Div {
    div().h(px(1.)).flex_none().bg(rgb(BORDER))
}
