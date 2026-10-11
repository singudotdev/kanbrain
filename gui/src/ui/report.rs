//! The report tab: tracked time per task or tag for an hour, day, week or month,
//! with a per-day or per-week breakdown and, for hours and days, the timeline with untracked gaps.

use super::theme::{self, BORDER, DIM};
use super::widgets::{Kind, card, dim, section, segment, segmented, tag_chip};
use super::{BrainView, action};
use crate::state::Board;
use crate::text::event::{KeyCode, KeyEvent, KeyModifiers};
use chrono::Duration;
use gpui::{ClickEvent, Context, Div, FontWeight, ScrollHandle, div, prelude::*, px, rgb};
use kanbrain_core::brain::{Group, Report, ReportRow, ReportView};
use kanbrain_core::model::{fmt_dur, now};
use kanbrain_core::period::Unit;

/// Width of each breakdown column and of the total column.
const CELL_WIDTH: f32 = 72.;
/// The period buttons, with the key that picks each.
const UNIT_KEYS: [(Unit, char); 4] = [(Unit::Hour, 'h'), (Unit::Day, 'd'), (Unit::Week, 'w'), (Unit::Month, 'm')];

/// A toolbar to change the period, grouping and filter and to export, then the report.
pub fn draw(board: &Board, scroll: &ScrollHandle, cx: &mut Context<BrainView>) -> Div {
    let view = &board.report;
    let report = board.report();
    let units = UNIT_KEYS.map(|(unit, key)| {
        segment(("unit", key as usize), capitalized(unit.as_str()), view.period.unit == unit)
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.press(char_key(key), cx)))
    });
    let group = if view.group == Group::Tag { "By tag" } else { "By task" };
    let bar = div()
        .flex()
        .flex_none()
        .flex_wrap()
        .items_center()
        .gap_2()
        .pb_3()
        .child(segmented(units))
        .child(action("prev", "‹", Kind::Secondary, KeyCode::Left, cx))
        .child(action("now", "Now", Kind::Secondary, KeyCode::Char('t'), cx))
        .child(action("next", "›", Kind::Secondary, KeyCode::Right, cx))
        .child(action("group", group, Kind::Secondary, KeyCode::Char('g'), cx))
        .child(div().flex_1())
        .child(action("export", "Export .md", Kind::Primary, KeyCode::Char('e'), cx));

    let summary = div()
        .flex()
        .items_baseline()
        .gap_3()
        .child(div().text_size(px(18.)).font_weight(FontWeight::SEMIBOLD).child(view.period.label()))
        .child(div().text_color(rgb(theme::GREEN)).font_weight(FontWeight::SEMIBOLD).child(fmt_dur(report.total)))
        .child(dim(format!(
            "tracked · {} {}",
            report.rows.len(),
            if view.group == Group::Tag { "tag(s)" } else { "task(s)" }
        )));
    let mut content = div().flex().flex_col().gap_4().p_4().child(summary);
    content = if report.rows.is_empty() {
        content.child(dim("Nothing tracked in this period."))
    } else {
        content.child(table(board, view, &report))
    };
    if matches!(view.period.unit, Unit::Hour | Unit::Day) && !report.timeline.is_empty() {
        content = content.child(timeline(board, &report));
    }
    if view.group == Group::Tag && report.rows.len() > 1 {
        content = content.child(
            dim("Tasks with several tags count under each, so tag rows can add up to more than the total.")
                .text_size(px(12.)),
        );
    }
    let body = card()
        .flex_1()
        .child(div().id("report").flex_1().min_h_0().overflow_y_scroll().track_scroll(scroll).child(content));
    div().flex_1().min_h_0().flex().flex_col().child(bar).child(body)
}

/// One row per task or tag, a column per day (week) or week (month), and a totals row.
fn table(board: &Board, view: &ReportView, report: &Report) -> Div {
    let header = row()
        .text_size(px(12.))
        .text_color(rgb(DIM))
        .child(div().flex_1().child(capitalized(view.group.as_str())))
        .children(report.columns.iter().map(|c| cell(c.clone())))
        .child(cell("Total"));
    let rows = report.rows.iter().map(|r| {
        row()
            .border_t_1()
            .border_color(rgb(theme::BG))
            .child(row_name(board, view, r))
            .children(r.cells.iter().map(|&d| duration_cell(d)))
            .child(cell(fmt_dur(r.total)).font_weight(FontWeight::SEMIBOLD))
    });
    let totals = row()
        .border_t_1()
        .border_color(rgb(BORDER))
        .font_weight(FontWeight::SEMIBOLD)
        .child(div().flex_1().child("Total"))
        .children(report.column_totals.iter().map(|&d| duration_cell(d)))
        .child(cell(fmt_dur(report.total)).text_color(rgb(theme::GREEN)));
    div().flex().flex_col().child(header).children(rows).child(totals)
}

/// Every session in order, with untracked gaps between them.
fn timeline(board: &Board, report: &Report) -> Div {
    let now = now();
    let mut lines = div().flex().flex_col().gap_1().text_size(px(13.)).child(section("Timeline", []));
    for (gap, slot) in report.timeline_with_gaps(now) {
        if let Some(gap) = gap {
            lines = lines.child(dim(format!("untracked {}", fmt_dur(gap))).italic().pl_4());
        }
        let end = slot.end.map_or_else(|| "now".to_string(), |e| e.format("%H:%M").to_string());
        lines = lines.child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .w(px(100.))
                        .flex_none()
                        .text_color(rgb(DIM))
                        .child(format!("{}–{end}", slot.start.format("%H:%M"))),
                )
                .child(div().w(px(64.)).flex_none().text_right().child(fmt_dur(slot.end.unwrap_or(now) - slot.start)))
                .child(div().min_w_0().truncate().child(format!("#{} {}", slot.id, slot.title)))
                .child(chips(board, &slot.tags)),
        );
    }
    lines
}

/// A task's label with its tag chips, or a tag as a chip.
fn row_name(board: &Board, view: &ReportView, r: &ReportRow) -> Div {
    let name = div().flex_1().min_w_0().flex().items_center().gap_2();
    match r.label.strip_prefix('#').filter(|_| view.group == Group::Tag) {
        Some(tag) => name.child(tag_chip(board.tag_brain(tag), tag)),
        None => name.child(div().min_w_0().truncate().child(r.label.clone())).child(chips(board, &r.tags)),
    }
}

/// Tags in a row that doesn't wrap, each in its color: rows can mix brains, so each tag's comes
/// from a brain that uses it.
fn chips(board: &Board, tags: &[String]) -> Div {
    div().flex().flex_none().gap_1().children(tags.iter().map(|tag| tag_chip(board.tag_brain(tag), tag)))
}

/// A table row.
fn row() -> Div {
    div().flex().items_center().gap_2().min_h(px(32.)).px_2()
}

/// A right-aligned fixed-width cell.
fn cell(text: impl Into<gpui::SharedString>) -> Div {
    div().w(px(CELL_WIDTH)).flex_none().text_right().child(text.into())
}

/// A duration cell; empty ones show a dim dot so the table is easy to scan.
fn duration_cell(d: Duration) -> Div {
    if d.is_zero() { cell("·").text_color(rgb(DIM)) } else { cell(fmt_dur(d)) }
}

/// `week` → `Week`.
pub(super) fn capitalized(word: &str) -> String {
    let mut chars = word.chars();
    chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

/// The key event of a plain character key.
pub(super) fn char_key(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
}
