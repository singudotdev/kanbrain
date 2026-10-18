//! Reports as markdown: what the desktop app's report shows, as tables.

use crate::brain::{Group, Report, ReportView};
use crate::model::{fmt_dur, infallible, slugify};
use crate::period::Unit;
use chrono::{DateTime, Datelike, Duration, Local};
use std::fmt::{self, Write};

/// Markdown version of a report.
pub fn to_markdown(report: &Report, view: &ReportView, now: DateTime<Local>) -> String {
    let mut out = String::new();
    infallible(write_report(&mut out, report, view, now));
    out
}

/// e.g. `report-2026-W39.md`, `report-2026-09-24-by-tag.md`, `report-2026-09-prod.md`.
pub fn file_name(view: &ReportView) -> String {
    let start = view.period.start;
    let mut name = match view.period.unit {
        Unit::Hour => start.format("report-%Y-%m-%d-%Hh").to_string(),
        Unit::Day => start.format("report-%Y-%m-%d").to_string(),
        Unit::Week => {
            let week = start.iso_week();
            format!("report-{}-W{:02}", week.year(), week.week())
        }
        Unit::Month => start.format("report-%Y-%m").to_string(),
    };
    if view.group == Group::Tag {
        name.push_str("-by-tag");
    }
    if let Some(tag) = &view.tag {
        name.push('-');
        name.push_str(&slugify(tag));
    }
    name.push_str(".md");
    name
}

/// Writes the whole report: summary, table and (for hours and days) the timeline.
fn write_report(out: &mut String, report: &Report, view: &ReportView, now: DateTime<Local>) -> fmt::Result {
    let period = view.period;
    writeln!(out, "# Time report · {}\n", period.label())?;
    writeln!(out, "- period: {}", period.unit.as_str())?;
    writeln!(out, "- grouped by: {}", view.group.as_str())?;
    if let Some(tag) = &view.tag {
        writeln!(out, "- only tag: #{tag}")?;
    }
    if !view.query.trim().is_empty() {
        writeln!(out, "- only tasks matching: {}", view.query.trim())?;
    }
    writeln!(out, "- tracked: **{}**", fmt_dur(report.total))?;
    writeln!(out, "- generated: {}\n", now.format("%Y-%m-%d %H:%M"))?;

    if report.rows.is_empty() {
        return writeln!(out, "Nothing tracked in this period.");
    }
    write_table(out, report, view.group)?;
    if view.group == Group::Tag && report.rows.len() > 1 {
        writeln!(out, "\n_Tasks with several tags count under each, so tag rows can add up to more than the total._")?;
    }
    if matches!(period.unit, Unit::Hour | Unit::Day) {
        write_timeline(out, report, now)?;
    }
    Ok(())
}

/// One row per task (with a tags column) or per tag, one column per day/week, and a totals row.
fn write_table(out: &mut String, report: &Report, group: Group) -> fmt::Result {
    let with_tags = group == Group::Task;
    let tags_column = |value: String| if with_tags { vec![value] } else { Vec::new() };

    let mut header = vec![group.as_str().to_string()];
    header.extend(tags_column("tags".into()));
    header.extend(report.columns.iter().cloned());
    header.push("total".into());
    table_row(out, &header)?;

    let mut align = vec!["---".to_string()];
    align.extend(tags_column("---".into()));
    align.extend(std::iter::repeat_n("---:".to_string(), report.columns.len() + 1));
    table_row(out, &align)?;

    for row in &report.rows {
        let mut cells = vec![escape(&row.label)];
        cells.extend(tags_column(row.tags.join(", ")));
        cells.extend(row.cells.iter().map(|&d| cell(d)));
        cells.push(fmt_dur(row.total));
        table_row(out, &cells)?;
    }

    let mut totals = vec!["**total**".to_string()];
    totals.extend(tags_column(String::new()));
    totals.extend(report.column_totals.iter().map(|&d| cell(d)));
    totals.push(format!("**{}**", fmt_dur(report.total)));
    table_row(out, &totals)
}

/// Every session in order, with untracked gaps between them.
fn write_timeline(out: &mut String, report: &Report, now: DateTime<Local>) -> fmt::Result {
    writeln!(out, "\n## Timeline\n")?;
    table_row(out, &["start", "end", "duration", "task", "tags"].map(String::from))?;
    table_row(out, &["---", "---", "---:", "---", "---"].map(String::from))?;
    for (gap, slot) in report.timeline_with_gaps(now) {
        if let Some(gap) = gap {
            table_row(out, &[String::new(), String::new(), fmt_dur(gap), "_untracked_".into(), String::new()])?;
        }
        let end = slot.end.unwrap_or(now);
        table_row(
            out,
            &[
                slot.start.format("%H:%M").to_string(),
                slot.end.map_or_else(|| "running".to_string(), |e| e.format("%H:%M").to_string()),
                fmt_dur(end - slot.start),
                escape(&slot.label()),
                slot.tags.join(", "),
            ],
        )?;
    }
    Ok(())
}

/// Writes one markdown table row: `| a | b | c |`.
fn table_row(out: &mut String, cells: &[String]) -> fmt::Result {
    writeln!(out, "| {} |", cells.join(" | "))
}

/// Empty cells stay blank so the table is easy to scan.
fn cell(d: Duration) -> String {
    if d.is_zero() { String::new() } else { fmt_dur(d) }
}

/// Keeps task titles from breaking the table.
fn escape(s: &str) -> String {
    s.replace('|', "\\|")
}

#[cfg(test)]
#[path = "../tests/export.rs"]
mod tests;
