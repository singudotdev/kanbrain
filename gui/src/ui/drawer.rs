//! The details drawer: everything about the selected entry, beside the board or list, with a button
//! for each thing that can be done to it.

use super::theme::{self, BORDER, DIM_STYLE, Line, Span};
use super::widgets::{
    self, Kind, card, dim, dim_small, entry_pill, section, segment, segmented, small, state_pill, tag_chips,
};
use super::{BrainView, action, markdown, when};
use crate::state::{Board, COLUMNS, Confirm, Key, Mode, Target};
use crate::text::event::KeyCode;
use gpui::{ClickEvent, Context, Div, ElementId, FontWeight, ScrollHandle, div, prelude::*, px, rgb};
use kanbrain_core::model::{
    Asked, Author, Comment, DecisionState, Kind as EntryKind, Status, StatusChange, Task, fmt_age, fmt_clock, fmt_dur,
    now,
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
        .children(task.is_task().then(|| status_block(key, task, cx)))
        .children(task.is_task().then(|| time_block(task)))
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_2()
                .child(action("edit", "Edit", Kind::Secondary, KeyCode::Char('e'), cx))
                .child(action("relate", "Link…", Kind::Secondary, KeyCode::Char('r'), cx))
                .when(task.kind == EntryKind::Decision && task.decision != DecisionState::Accepted, |d| {
                    d.child(action("accept", "Accept", Kind::Primary, KeyCode::Char('y'), cx))
                })
                .when(task.kind == EntryKind::Decision && task.decision != DecisionState::Rejected, |d| {
                    d.child(action("reject", "Reject…", Kind::Secondary, KeyCode::Char('j'), cx))
                })
                .child(action("open-file", "Open file", Kind::Secondary, KeyCode::Char('o'), cx))
                .when(board.sources.len() > 1, |d| {
                    d.child(action("move", "Move…", Kind::Secondary, KeyCode::Char('M'), cx))
                })
                .child(action("delete", "Delete", Kind::Danger, KeyCode::Char('x'), cx)),
        )
        .child(divider())
        .child(description_block(task, cx))
        .child(related_block(board, key, cx))
        .child(comments_block(key, task, cx))
        .children(task.is_task().then(|| history_block(task)));
    card().child(content)
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
        .child(div().text_size(px(18.)).font_weight(FontWeight::SEMIBOLD).child(task.title.clone()))
        .child(dim_small(format!("created {}", when(task.created))))
        .when(!task.tags.is_empty(), |d| d.child(tag_chips(board.brain(key.src), &task.tags).pt_1()))
}

/// The question an agent asked and the task waits on, with a button to answer it and one to open each
/// decision it proposed for it; or that it's answered, and the agent picks it up from there.
fn asked_block(board: &Board, key: Key, task: &Task, cx: &mut Context<BrainView>) -> Option<Div> {
    let callout = div().flex().flex_col().gap_2().p_3().rounded_md().border_1();
    match task.asked()? {
        Asked::Waiting(question) => {
            let answer = small(action("answer", "Answer", Kind::Primary, KeyCode::Char('c'), cx));
            let proposed = board.related_keys(key).into_iter().filter(|&k| {
                board.task(k).is_some_and(|d| d.kind == EntryKind::Decision && d.decision == DecisionState::Proposed)
            });
            let open = proposed.map(|k| {
                small(widgets::button(
                    ElementId::Name(format!("open-proposed {}/{}", k.src, k.id).into()),
                    format!("Open decision #{}", k.id),
                    Kind::Secondary,
                ))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.act(cx, |b| b.open_entry(k))))
            });
            let buttons = div().flex().gap_2().child(answer).children(open);
            Some(
                callout
                    .border_color(rgb(theme::ACCENT))
                    .bg(rgb(theme::SELECTED))
                    .child(dim_small(format!("{} asks, {}", who(question), when(question.created))))
                    .child(markdown::render("question", &question.body))
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

/// Who wrote a comment, for people: "An agent" or "You".
fn who(comment: &Comment) -> &'static str {
    match comment.author {
        Author::Agent => "An agent",
        Author::Human => "You",
    }
}

/// The status buttons, the current one selected, and how long the task has had it.
fn status_block(key: Key, task: &Task, cx: &mut Context<BrainView>) -> Div {
    let segments = COLUMNS.map(|status| {
        segment(("status", status as usize), theme::status_label(status), task.status == status)
            .debug_selector(move || format!("drawer status {}", status.as_str()))
            .tooltip(widgets::tooltip(format!("Shortcut: {}", status_key(status))))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.act(cx, |b| b.set_status(key, status))))
    });
    let since = task.status_since();
    div().flex().flex_col().gap_1p5().child(segmented(segments).self_start()).child(
        dim(format!("{} for {}, since {}", theme::status_label(task.status), fmt_age(now() - since), when(since)))
            .text_size(px(12.)),
    )
}

/// The key that moves the selected task to `status`.
fn status_key(status: Status) -> char {
    match status {
        Status::Todo => 't',
        Status::Doing => 's',
        Status::Parked => 'p',
        Status::Done => 'd',
    }
}

/// Time tracked on the task: the running clock, today, this week and in total.
fn time_block(task: &Task) -> Div {
    let now = now();
    let within = |unit| {
        let (from, to) = Period::current(unit).bounds();
        fmt_dur(task.time_between(from, to, now))
    };
    let mut text = format!(
        "Tracked today {} · this week {} · total {} in {} session(s)",
        within(Unit::Day),
        within(Unit::Week),
        fmt_dur(task.total(now)),
        task.sessions.len()
    );
    if let Some(running) = task.running_for(now) {
        text = format!("Running {} · {text}", fmt_clock(running));
    }
    dim_small(text)
}

/// The whole description, and a button to edit it.
fn description_block(task: &Task, cx: &mut Context<BrainView>) -> Div {
    let edit = small(action("edit-description", "Edit", Kind::Secondary, KeyCode::Char('m'), cx));
    let body = if task.description.is_empty() {
        dim("No description yet.")
    } else {
        markdown::render("description", &task.description)
    };
    div().flex().flex_col().gap_2().child(section("Description", [edit])).child(body)
}

/// The entries linked to this one, whichever side stores the link, each opening when clicked: those of
/// its brain, then those of the project's other repositories, named by their repository.
fn related_block(board: &Board, key: Key, cx: &mut Context<BrainView>) -> Div {
    let related: Vec<_> = board.related_keys(key).into_iter().filter_map(|k| Some((k, board.task(k)?))).collect();
    let link = small(action("link", "Link…", Kind::Secondary, KeyCode::Char('r'), cx));
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

/// Every comment, oldest first, each with Edit and Delete; and a button to add one.
fn comments_block(key: Key, task: &Task, cx: &mut Context<BrainView>) -> Div {
    let add = small(action("add-comment", "Add", Kind::Secondary, KeyCode::Char('c'), cx));
    let comments: Vec<_> = task
        .comments
        .iter()
        .enumerate()
        .map(|(index, comment)| {
            let edited = comment.edited.map(|at| format!(" · edited {}", when(at))).unwrap_or_default();
            let what = if comment.question { "question" } else { "comment" };
            let by = if comment.author == Author::Agent { format!("Agent {what} · ") } else { String::new() };
            let edit = small(widgets::button(("edit-comment", index), "Edit", Kind::Ghost)).on_click(cx.listener(
                move |this, _: &ClickEvent, _, cx| this.act(cx, |b| b.open_editor(Target::Comment(key, index))),
            ));
            let delete = small(widgets::button(("delete-comment", index), "Delete", Kind::Ghost)).on_click(
                cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.act(cx, |b| b.mode = Mode::Confirm(Confirm::DeleteComment(key, index)));
                }),
            );
            div()
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
                        .child(dim_small(format!("{by}{}{edited}", when(comment.created))).flex_1())
                        .child(edit)
                        .child(delete),
                )
                .child(markdown::render(format!("comment {index}"), &comment.body))
        })
        .collect();
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(section(format!("Comments ({})", task.comments.len()), [add]))
        .when(task.comments.is_empty(), |d| d.child(dim("No comments yet.")))
        .children(comments)
}

/// Every status change, newest first.
fn history_block(task: &Task) -> Div {
    let changes = task.history.iter().rev().map(|change| history_line(change).render());
    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_size(px(12.))
        .child(section("History", []))
        .when(task.history.is_empty(), |d| d.child(dim("No status changes yet.")))
        .children(changes)
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
