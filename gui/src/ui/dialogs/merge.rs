//! The lines both the editor and `kb` changed, shown in place of the editor's text when a save stops over them: each
//! conflict with yours beside theirs, to pick one or both. Every side's and settled run's lines are a text of their
//! own the mouse selects.

use super::dialog;
use super::editor::{edited_name, title};
use crate::state::{Board, Edit, Target};
use crate::text::event::KeyCode;
use crate::text::merge::{Conflict, Merge, Pick, Region};
use crate::ui::drawer::change_label;
use crate::ui::shown;
use crate::ui::theme::{self, ACCENT, BORDER, Line};
use crate::ui::widgets::{Kind, button, dim_small, segment, segmented, tooltip};
use crate::ui::{BrainView, action};
use gpui::{ClickEvent, Context, Div, ScrollHandle, Stateful, div, prelude::*, px, relative, rgb};
use kanbrain_core::model::{Changed, counted};

/// The conflicts of `merge`, which `edit` stopped saving over, in the text's order between the lines that merged.
pub(super) fn draw(
    board: &Board,
    edit: &Edit,
    merge: &Merge,
    (width, scroll): (f32, &ScrollHandle),
    follow: bool,
    cx: &mut Context<BrainView>,
) -> Div {
    let count = merge.conflicts().count();
    let mut index = 0;
    let mut rows = Vec::with_capacity(merge.regions.len());
    for (at, region) in merge.regions.iter().enumerate() {
        rows.push(match region {
            Region::Settled(text) => settled(board, at, text, cx),
            Region::Conflict(conflict) => {
                let current = index == merge.at;
                if follow && current {
                    scroll.scroll_to_item(at);
                }
                let row = conflict_box(board, conflict, index, count, current, cx);
                index += 1;
                row
            }
        });
    }
    let text = div()
        .id("merge")
        .debug_selector(|| "merge".into())
        .flex_1()
        .min_h(px(160.))
        .overflow_y_scroll()
        .track_scroll(scroll)
        .p_3()
        .flex()
        .flex_col()
        .gap_2()
        .rounded_md()
        .bg(rgb(theme::INPUT))
        .border_1()
        .border_color(rgb(BORDER))
        .font_family(theme::EDITOR_FONT)
        .text_size(px(13.))
        .children(rows);
    let content = div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .gap_2()
        .child(dim_small(edited_name(board, edit.target)))
        .child(dim_small(changed_by(board, edit.target)))
        .child(text)
        .child(dim_small(
            "Tab or ↑ ↓ (Shift+Tab back) move between conflicts; M takes yours, T theirs, B both. Enter puts the \
             result in the editor to look over and save; Esc takes yours where nothing's picked; Ctrl+D discards \
             your changes.",
        ));
    let buttons = vec![
        button("discard", "Discard changes", Kind::Secondary)
            .tooltip(tooltip("Shortcut: Ctrl+D"))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.act(cx, Board::discard_edit))),
        action("merge-mine", "Yours for the rest", Kind::Secondary, KeyCode::Esc, cx),
        action("merge-use", "Use the result", Kind::Primary, KeyCode::Enter, cx),
    ];
    dialog(title(heading(merge), true, edit.changed_on_disk), content, buttons).w(px(width)).h(relative(0.8))
}

/// What the dialog's title says: how many conflicts are left to pick, until none is.
pub(super) fn heading(merge: &Merge) -> String {
    match merge.conflicts().filter(|c| c.pick.is_none()).count() {
        0 => "All picked: use the result".to_string(),
        left => format!("{} to pick", counted(left, "conflict", "conflicts")),
    }
}

/// What a conflict's side says in place of lines that show nothing: none, or only empty ones.
pub(super) fn side_note(lines: &str) -> Option<&'static str> {
    match lines {
        "" => Some("(removed)"),
        "\n" => Some("(an empty line)"),
        _ if lines.trim().is_empty() => Some("(empty lines)"),
        _ => None,
    }
}

/// Lines as a region holds them, each ending in a break, as a text to draw: without the last line's break, which
/// would draw an empty line after it, but with the empty lines it holds.
pub(super) fn as_drawn(lines: &str) -> String {
    lines.strip_suffix('\n').unwrap_or(lines).to_string()
}

/// Who changed the text meanwhile, and when, as the entry's activity says for a description; the other texts keep
/// no such record.
pub(super) fn changed_by(board: &Board, target: Target) -> String {
    let change = match target {
        Target::Description(key) => board
            .task(key)
            .and_then(|task| task.activity.iter().rev().find(|a| matches!(a.change, Changed::Description))),
        _ => None,
    };
    let said = change.map_or_else(|| "It was changed meanwhile".to_string(), change_label);
    format!("{said}. The lines only one of you changed are merged; pick where you both changed the same lines.")
}

/// Lines the merge settled, the `at`th region, as they'll be; a single empty one keeps its height.
fn settled(board: &Board, at: usize, text: &str, cx: &mut Context<BrainView>) -> Div {
    shown::text(board, format!("merge settled {at}"), Line::raw(as_drawn(text)), cx).text_color(rgb(theme::DIM))
}

/// Conflict `index` of `count`: yours beside theirs, each picked with a click, and the picks as segments. The one
/// being picked (`current`) stands out.
fn conflict_box(
    board: &Board,
    conflict: &Conflict,
    index: usize,
    count: usize,
    current: bool,
    cx: &mut Context<BrainView>,
) -> Div {
    let picks = [("Yours (M)", Pick::Mine), ("Theirs (T)", Pick::Theirs), ("Both (B)", Pick::Both)];
    let picks = picks.into_iter().enumerate().map(|(at, (label, pick))| {
        segment(("merge-pick", index * picks.len() + at), label, conflict.pick == Some(pick))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.act(cx, |b| b.pick_conflict(index, pick))))
    });
    let (mine, theirs) = conflict.taken();
    div()
        .flex()
        .flex_col()
        .gap_2()
        .p_2()
        .rounded_md()
        .border_1()
        .border_color(rgb(if current { ACCENT } else { BORDER }))
        .bg(rgb(theme::SURFACE))
        .debug_selector(move || format!("conflict {index}"))
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .font_family(theme::UI_FONT)
                // For the window tests, which find the one being picked; nothing outside them.
                .debug_selector(
                    move || if current { "current conflict".into() } else { format!("conflict head {index}") },
                )
                .child(dim_small(format!("Conflict {} of {count}", index + 1)).flex_1())
                .child(segmented(picks)),
        )
        .child(
            div()
                .flex()
                .gap_2()
                .child(side(board, ("merge-mine", index), "Yours", &conflict.mine, mine, index, Pick::Mine, cx))
                .child(side(
                    board,
                    ("merge-theirs", index),
                    "Theirs",
                    &conflict.theirs,
                    theirs,
                    index,
                    Pick::Theirs,
                    cx,
                )),
        )
}

/// One side of conflict `index`: its lines, or what they are if they show nothing, outlined once `taken`. A click
/// picks it, but not one ending a drag that selected some of its lines.
#[expect(clippy::too_many_arguments, reason = "the board and the window, besides what the side shows")]
fn side(
    board: &Board,
    id: (&'static str, usize),
    label: &str,
    text: &str,
    taken: bool,
    index: usize,
    pick: Pick,
    cx: &mut Context<BrainView>,
) -> Stateful<Div> {
    let region = format!("{} {index}", id.0);
    let lines = match side_note(text) {
        Some(note) => div().text_color(rgb(theme::DIM)).italic().child(note),
        None => shown::text(board, region.clone(), Line::raw(as_drawn(text)), cx),
    };
    div()
        .id(id)
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap_1()
        .p_2()
        .rounded_sm()
        .cursor_pointer()
        .border_1()
        .border_color(rgb(if taken { ACCENT } else { BORDER }))
        .debug_selector(move || format!("{}-{index}", id.0))
        .when(taken, |d| d.bg(rgb(theme::SELECTED)))
        .hover(|s| s.bg(rgb(theme::HOVER)))
        .child(dim_small(label.to_string()).font_family(theme::UI_FONT))
        .child(lines)
        // A click picks it, so a double-click picks it too, then selects the word; a drag that selected doesn't.
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
            if !this.board.selects_shown(&region) {
                this.act(cx, |b| b.pick_conflict(index, pick));
            }
        }))
}
