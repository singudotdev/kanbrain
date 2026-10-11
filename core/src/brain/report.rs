//! Time per task or tag for a period, with per-day/per-week breakdown and a session timeline.

use super::Brain;
use crate::enums::named_enum;
use crate::model::{Task, now};
use crate::period::Period;
use chrono::{DateTime, Duration, Local};
use std::collections::BTreeMap;

named_enum! {
    /// How report rows are grouped. The name is shown in titles and exports and typed as `--group`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub enum Group {
        /// Task rows, each showing its tags.
        #[default]
        Task => "task",
        /// One row per tag; a task with several tags counts under each.
        Tag => "tag",
    }
}

/// What a report shows: which period, and how it's grouped and filtered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportView {
    /// The hour, day, week or month being reported.
    pub period: Period,
    /// Rows per task or per tag.
    pub group: Group,
    /// Only count tasks carrying this tag.
    pub tag: Option<String>,
    /// Only count tasks matching this search (see [`Task::matches`]); empty matches all.
    pub query: String,
}

impl ReportView {
    /// The report for `period`, grouped by task, unfiltered.
    pub fn new(period: Period) -> Self {
        Self { period, group: Group::default(), tag: None, query: String::new() }
    }
}

/// Label for tasks without tags when grouping by tag.
const UNTAGGED: &str = "(untagged)";

/// Pauses between sessions at least this long are shown as untracked time.
const MIN_GAP_MINUTES: i64 = 5;

/// One session (clipped to the report's period) in the timeline.
#[derive(Debug)]
pub struct Slot {
    /// When the session started (or the period started, if later).
    pub start: DateTime<Local>,
    /// `None` while still running.
    pub end: Option<DateTime<Local>>,
    /// Id of the task the session belongs to.
    pub id: u32,
    /// Title of the task.
    pub title: String,
    /// Tags of the task.
    pub tags: Vec<String>,
}

/// One row of a report: a task or a tag, with its time per column and in total.
#[derive(Debug)]
pub struct ReportRow {
    /// `#id title` or `#tag`.
    pub label: String,
    /// The task's tags; empty for tag rows.
    pub tags: Vec<String>,
    /// Time in the whole period.
    pub total: Duration,
    /// Time per breakdown column, parallel to `Report::columns`.
    pub cells: Vec<Duration>,
}

/// A computed report, rendered both in the desktop app and as markdown (`export`).
#[derive(Debug)]
pub struct Report {
    /// Real tracked time; when grouping by tag, rows can add up to more (multi-tag tasks).
    pub total: Duration,
    /// Tasks (or tags) with tracked time, busiest first.
    pub rows: Vec<ReportRow>,
    /// Breakdown column labels (days of a week, weeks of a month); empty for hour/day.
    pub columns: Vec<String>,
    /// Sum of each breakdown column over all tasks.
    pub column_totals: Vec<Duration>,
    /// Every session in the period, in start order.
    pub timeline: Vec<Slot>,
}

impl Report {
    /// Each timeline slot with the untracked gap (if long enough) right before it.
    pub fn timeline_with_gaps(&self, now: DateTime<Local>) -> Vec<(Option<Duration>, &Slot)> {
        let mut prev_end: Option<DateTime<Local>> = None;
        self.timeline
            .iter()
            .map(|slot| {
                let gap = prev_end.map(|p| slot.start - p).filter(|g| *g >= Duration::minutes(MIN_GAP_MINUTES));
                let end = slot.end.unwrap_or(now);
                prev_end = Some(prev_end.map_or(end, |p| p.max(end)));
                (gap, slot)
            })
            .collect()
    }
}

impl Report {
    /// Several brains' reports for the same view as one. Task rows and timeline entries are named
    /// after their project (`api · #3 Fix login`), since each brain numbers its entries from 1; tag
    /// rows of the same tag are added up.
    pub fn merge(parts: Vec<(Option<String>, Report)>, group: Group) -> Report {
        let columns = parts.first().map(|(_, r)| r.columns.clone()).unwrap_or_default();
        let mut total = Duration::zero();
        let mut column_totals = vec![Duration::zero(); columns.len()];
        let mut rows: BTreeMap<String, ReportRow> = BTreeMap::new();
        let mut timeline = Vec::new();
        for (project, report) in parts {
            let named = |label: String| project.as_ref().map_or_else(|| label.clone(), |p| format!("{p} · {label}"));
            total += report.total;
            add_cells(&mut column_totals, &report.column_totals);
            for row in report.rows {
                let label = if group == Group::Task { named(row.label) } else { row.label };
                match rows.get_mut(&label) {
                    Some(sum) => {
                        sum.total += row.total;
                        add_cells(&mut sum.cells, &row.cells);
                    }
                    None => _ = rows.insert(label.clone(), ReportRow { label, ..row }),
                }
            }
            timeline.extend(report.timeline.into_iter().map(|slot| Slot { title: named(slot.title.clone()), ..slot }));
        }
        let mut rows: Vec<ReportRow> = rows.into_values().collect();
        rows.sort_by_key(|r| std::cmp::Reverse(r.total));
        timeline.sort_by_key(|s| s.start);
        Report { total, rows, columns, column_totals, timeline }
    }
}

/// Accumulates a row's total and per-column cells.
struct RowSums {
    /// Time in the whole period.
    total: Duration,
    /// Time per breakdown column.
    cells: Vec<Duration>,
    /// The task's tags (task rows only).
    tags: Vec<String>,
}

impl RowSums {
    /// An empty row for a report with `columns` breakdown columns.
    fn new(columns: usize) -> Self {
        Self { total: Duration::zero(), cells: vec![Duration::zero(); columns], tags: Vec::new() }
    }

    /// Adds one task's time to the row.
    fn add(&mut self, total: Duration, cells: &[Duration]) {
        self.total += total;
        add_cells(&mut self.cells, cells);
    }
}

/// Adds `cells` to `sums`, column by column.
fn add_cells(sums: &mut [Duration], cells: &[Duration]) {
    for (sum, cell) in sums.iter_mut().zip(cells) {
        *sum += *cell;
    }
}

impl Brain {
    /// Computes the report for `view`: rows, breakdown columns, totals and the timeline.
    pub fn report(&self, view: &ReportView) -> Report {
        let now = now();
        let (from, to) = view.period.bounds();
        // Breakdown columns, clipped to the period (a month's first and last weeks).
        let (labels, ranges): (Vec<String>, Vec<_>) = view
            .period
            .columns()
            .into_iter()
            .map(|(label, p)| {
                let (a, b) = p.bounds();
                (label, (a.max(from), b.min(to)))
            })
            .unzip();

        let mut total = Duration::zero();
        let mut column_totals = vec![Duration::zero(); labels.len()];
        // Keyed by label; BTreeMap keeps rows with equal time in a stable order.
        let mut rows: BTreeMap<String, RowSums> = BTreeMap::new();
        let mut timeline = Vec::new();

        let tasks = self
            .tasks
            .iter()
            .filter(|t| view.tag.as_ref().is_none_or(|tag| t.tags.contains(tag)) && t.matches(&view.query));
        for task in tasks {
            let task_total = task.time_between(from, to, now);
            if task_total.is_zero() {
                continue;
            }
            let cells: Vec<Duration> = ranges.iter().map(|&(a, b)| task.time_between(a, b, now)).collect();
            total += task_total;
            add_cells(&mut column_totals, &cells);

            for label in row_labels(task, view.group) {
                let row = rows.entry(label).or_insert_with(|| RowSums::new(ranges.len()));
                if view.group == Group::Task {
                    row.tags.clone_from(&task.tags);
                }
                row.add(task_total, &cells);
            }
            timeline.extend(slots(task, from, to, now));
        }

        let mut rows: Vec<ReportRow> = rows
            .into_iter()
            .map(|(label, sums)| ReportRow { label, tags: sums.tags, total: sums.total, cells: sums.cells })
            .collect();
        rows.sort_by_key(|r| std::cmp::Reverse(r.total));
        timeline.sort_by_key(|s| s.start);
        Report { total, rows, columns: labels, column_totals, timeline }
    }
}

/// The rows a task counts toward: itself, or each of its tags.
fn row_labels(task: &Task, group: Group) -> Vec<String> {
    match group {
        Group::Task => vec![format!("#{} {}", task.id, task.title)],
        Group::Tag if task.tags.is_empty() => vec![UNTAGGED.to_string()],
        Group::Tag => task.tags.iter().map(|tag| format!("#{tag}")).collect(),
    }
}

/// The task's sessions that fall in `[from, to)`, clipped to it.
fn slots(
    task: &Task,
    from: DateTime<Local>,
    to: DateTime<Local>,
    now: DateTime<Local>,
) -> impl Iterator<Item = Slot> + '_ {
    task.sessions.iter().filter(move |s| !s.overlap(from, to, now).is_zero()).map(move |s| Slot {
        start: s.start.max(from),
        end: s.end.map(|e| e.min(to)),
        id: task.id,
        title: task.title.clone(),
        tags: task.tags.clone(),
    })
}
