//! The calendar tab: each day's interactions with entries (time worked, moves, comments, new
//! entries) for a day, a week or a month, filtered like the report by tag and by kind of interaction.

use super::report::{capitalized, char_key};
use super::theme::{self, BORDER, DIM, TEXT};
use super::widgets::{Kind, card, dim, dim_small, segment, segmented, tag_chips};
use super::{BrainView, action};
use crate::state::{Board, Key};
use crate::text::event::KeyCode;
use chrono::{DateTime, Datelike, Local, NaiveDate};
use gpui::{ClickEvent, Context, Div, FontWeight, ScrollHandle, Stateful, Styled, div, prelude::*, px, rgb};
use kanbrain_core::brain::{Day, Event, Show, What};
use kanbrain_core::model::{counted, fmt_dur, now};
use kanbrain_core::period::Unit;

/// The period buttons, with the key that picks each.
const UNIT_KEYS: [(Unit, char); 3] = [(Unit::Day, 'd'), (Unit::Week, 'w'), (Unit::Month, 'm')];
/// Interactions listed in a month's day before "+N more"; the day itself shows them all.
const MONTH_EVENTS: usize = 3;
/// Height of a month's day, so weeks line up whatever they hold.
const MONTH_DAY_HEIGHT: f32 = 112.;

/// A toolbar to change the period and filters, then the days.
pub fn draw(board: &Board, scroll: &ScrollHandle, cx: &mut Context<BrainView>) -> Div {
    let view = &board.calendar;
    let (mut calendar, srcs) = board.calendar();
    // From the merged calendars' order to the brains' indexes, so an event names its entry's `Key`.
    for event in calendar.days.iter_mut().flat_map(|d| &mut d.events) {
        event.source = srcs[event.source];
    }
    let units = UNIT_KEYS.map(|(unit, key)| {
        segment(("unit", key as usize), capitalized(unit.as_str()), view.period.unit == unit)
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.press(char_key(key), cx)))
    });
    let show = match view.show {
        Show::All => "All interactions".to_string(),
        Show::New => "New entries".to_string(),
        other => capitalized(other.as_str()),
    };
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
        .child(action("show", show, Kind::Secondary, KeyCode::Char('i'), cx));
    let summary = div()
        .flex()
        .items_baseline()
        .gap_3()
        .child(div().text_size(px(18.)).font_weight(FontWeight::SEMIBOLD).child(view.period.label()))
        .child(div().text_color(rgb(theme::GREEN)).font_weight(FontWeight::SEMIBOLD).child(fmt_dur(calendar.tracked)))
        .child(dim(format!("tracked · {}", counted(calendar.events(), "interaction", "interactions"))));
    let now = now();
    let days = match view.period.unit {
        Unit::Month => month(board, &calendar.days, now, cx),
        Unit::Week => week(board, &calendar.days, now, cx),
        Unit::Hour | Unit::Day => day(board, &calendar.days, cx),
    };
    let content = div().flex().flex_col().gap_4().p_4().child(summary).child(days);
    let body = card()
        .flex_1()
        .child(div().id("calendar").flex_1().min_h_0().overflow_y_scroll().track_scroll(scroll).child(content));
    div().flex_1().min_h_0().flex().flex_col().child(bar).child(body)
}

/// Weeks of seven days, each with its time and first interactions (`shop/api #3` when several brains are shown, as
/// a week's and a day's name them); clicking a day shows it whole.
fn month(board: &Board, days: &[Day], now: DateTime<Local>, cx: &mut Context<BrainView>) -> Div {
    let names = days.iter().take(7).map(|d| {
        div().flex_1().min_w_0().px_2().text_size(px(12.)).text_color(rgb(DIM)).child(d.date.format("%a").to_string())
    });
    let weeks = days.chunks(7).map(|week| {
        div().flex().gap_1().children(week.iter().map(|d| {
            let date = d.date;
            let more = d.events.len().saturating_sub(MONTH_EVENTS);
            div()
                .id(("day", date_key(date)))
                .debug_selector(move || format!("day {date}"))
                .flex_1()
                .min_w_0()
                .h(px(MONTH_DAY_HEIGHT))
                .flex()
                .flex_col()
                .gap_0p5()
                .p_1p5()
                .rounded_md()
                .border_1()
                .border_color(rgb(if date == now.date_naive() { theme::ACCENT } else { BORDER }))
                .overflow_hidden()
                .text_size(px(11.))
                .when(!d.in_period, |c| c.opacity(0.35))
                .when(d.in_period, |c| {
                    c.bg(rgb(theme::BG)).cursor_pointer().hover(|s| s.bg(rgb(theme::HOVER))).on_click(
                        cx.listener(move |this, _: &ClickEvent, _, cx| this.act(cx, |b| b.calendar_day(date))),
                    )
                })
                .child(day_heading(d, "%d"))
                .children(d.events.iter().take(MONTH_EVENTS).map(|e| {
                    div().flex().gap_1().child(marker(e, true)).child(
                        div().min_w_0().truncate().text_color(rgb(TEXT)).child(match board.source_label(e.source) {
                            Some(label) => format!("{label} #{}", e.id),
                            None => format!("#{}", e.id),
                        }),
                    )
                }))
                .children((more > 0).then(|| dim(format!("+{more} more"))))
        }))
    });
    div().flex().flex_col().gap_1().child(div().flex().gap_1().children(names)).children(weeks)
}

/// Seven columns, each day's interactions in time order.
fn week(board: &Board, days: &[Day], now: DateTime<Local>, cx: &mut Context<BrainView>) -> Div {
    div().flex().gap_2().children(days.iter().map(|d| {
        let date = d.date;
        div()
            .debug_selector(move || format!("week day {date}"))
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_1()
            .p_1p5()
            .rounded_md()
            .border_1()
            .border_color(rgb(if d.date == now.date_naive() { theme::ACCENT } else { BORDER }))
            .bg(rgb(theme::BG))
            .child(day_heading(d, "%a %d").text_size(px(12.)).pb_1())
            .children(d.events.iter().enumerate().map(|(i, e)| event_row(board, e, (d.date, i), false, cx)))
            .when(d.events.is_empty(), |c| c.child(dim_small("·").px_1()))
    }))
}

/// One day, with room for comment excerpts and tags.
fn day(board: &Board, days: &[Day], cx: &mut Context<BrainView>) -> Div {
    let Some(d) = days.first() else { return div() };
    if d.events.is_empty() {
        return div().child(dim("Nothing happened on this day."));
    }
    div()
        .flex()
        .flex_col()
        .gap_1()
        .children(d.events.iter().enumerate().map(|(i, e)| event_row(board, e, (d.date, i), true, cx)))
}

/// A day's date, and its time worked if any.
fn day_heading(d: &Day, format: &str) -> Div {
    div()
        .flex()
        .justify_between()
        .gap_1()
        .child(div().font_weight(FontWeight::SEMIBOLD).child(d.date.format(format).to_string()))
        .when(!d.tracked.is_zero(), |h| h.child(div().text_color(rgb(theme::GREEN)).child(fmt_dur(d.tracked))))
}

/// An interaction: when, what, and which entry; a click shows the entry's details. `wide` adds the
/// comment's first line and the tags, for the day view.
fn event_row(
    board: &Board,
    event: &Event,
    key: (NaiveDate, usize),
    wide: bool,
    cx: &mut Context<BrainView>,
) -> Stateful<Div> {
    let id = event.id;
    let entry = Key::new(event.source, id);
    let selected = board.selected == Some(entry) && board.drawer;
    let time = match event.what {
        What::Worked { end } if wide => {
            let end = end.map_or_else(|| "now".to_string(), |e| e.format("%H:%M").to_string());
            format!("{}–{end}", event.at.format("%H:%M"))
        }
        _ => event.at.format("%H:%M").to_string(),
    };
    let title =
        div().min_w_0().when(wide, Styled::truncate).when(!wide, |t| t.line_clamp(3)).text_color(rgb(TEXT)).child(
            match board.source_label(event.source) {
                Some(label) => format!("{label} #{id} {}", event.title),
                None => format!("#{id} {}", event.title),
            },
        );
    let excerpt = match &event.what {
        What::Commented { excerpt } if wide && !excerpt.is_empty() => Some(dim(excerpt.clone()).min_w_0().truncate()),
        _ => None,
    };
    div()
        .id(("event", date_key(key.0) * 1000 + key.1))
        .debug_selector(move || format!("event {}/{id}", entry.src))
        .flex()
        .when(wide, |r| r.items_center().gap_3().text_size(px(13.)))
        .when(!wide, |r| r.flex_col().text_size(px(11.)))
        .px_1p5()
        .py_1()
        .rounded_sm()
        .cursor_pointer()
        .when(selected, |r| r.bg(rgb(theme::SELECTED)))
        .when(!selected, |r| r.hover(|s| s.bg(rgb(theme::HOVER))))
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.act(cx, |b| b.show_entry(entry))))
        .child(
            // In a week's narrow columns the time and what happened wrap onto more lines rather than
            // being cut off; the day view has room for them on one.
            div()
                .flex()
                .flex_wrap()
                .gap_x_1p5()
                .min_w_0()
                .when(wide, |m| m.flex_none().w(px(260.)))
                .child(div().text_color(rgb(DIM)).font_family(theme::EDITOR_FONT).child(time))
                .child(marker(event, false)),
        )
        .child(title)
        .children(excerpt)
        .when(wide, |r| r.child(div().flex_1()).child(tag_chips(board, &event.tags).flex_nowrap().flex_none()))
}

/// What happened to the entry that day, colored by kind: its last move (`doing → parked`), else
/// `new note`, `comment` or `▶ worked`; then the time worked that day and, in the day view, how many
/// comments. `compact` (the month) keeps only where a move went: `→ parked`.
fn marker(event: &Event, compact: bool) -> Div {
    let (text, color) = match &event.what {
        What::Worked { end } => {
            let running = if end.is_none() && !compact { "running" } else { "worked" };
            (format!("▶ {running}"), theme::GREEN)
        }
        What::Moved { to, .. } if compact => (format!("→ {}", to.as_str()), theme::status_color(*to)),
        What::Moved { from, to } => (format!("{} → {}", from.as_str(), to.as_str()), theme::status_color(*to)),
        What::Commented { .. } => ("comment".to_string(), theme::PURPLE),
        What::Created => (format!("new {}", event.kind.as_str()), theme::CYAN),
    };
    let worked = (!event.worked.is_zero()).then(|| {
        div().text_color(rgb(theme::GREEN)).child(if compact {
            fmt_dur(event.worked)
        } else {
            format!("▶ {}", fmt_dur(event.worked))
        })
    });
    let comments = (!compact && event.comments > 0 && !matches!(event.what, What::Commented { .. }))
        .then(|| div().text_color(rgb(theme::PURPLE)).child(counted(event.comments, "comment", "comments")));
    // Each piece stays whole; outside the month, the pieces wrap when the column is narrow.
    div()
        .flex()
        .gap_x_1p5()
        .min_w_0()
        .when(compact, Styled::flex_none)
        .when(!compact, Styled::flex_wrap)
        .child({
            let (src, id) = (event.source, event.id);
            div()
                .debug_selector(move || format!("event what {src}/{id}"))
                .whitespace_nowrap()
                .text_color(rgb(color))
                .child(text)
        })
        .children(worked.map(Styled::whitespace_nowrap))
        .children(comments.map(Styled::whitespace_nowrap))
}

/// A number unique to each date, for element ids.
fn date_key(date: NaiveDate) -> usize {
    usize::try_from(date.num_days_from_ce()).unwrap_or_default()
}
