//! Time per task or tag for a period, with per-day/per-week breakdown and a session timeline.

use super::Brain;
use crate::enums::named_enum;
use crate::model::{Task, covered, now};
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
    /// The brain the task is in, when a report merges several, which number their entries alike.
    pub project: Option<String>,
}

impl Slot {
    /// `#id title`, after its brain when the report merges several: `api · #3 Fix login`.
    pub fn label(&self) -> String {
        match &self.project {
            Some(project) => format!("{project} · #{} {}", self.id, self.title),
            None => format!("#{} {}", self.id, self.title),
        }
    }

    /// Where the slot starts and ends within `[from, to)`, a running one ending `now`.
    fn span(
        &self,
        from: DateTime<Local>,
        to: DateTime<Local>,
        now: DateTime<Local>,
    ) -> (DateTime<Local>, DateTime<Local>) {
        (self.start.max(from), self.end.unwrap_or(now).min(to))
    }
}

/// One row of a report: a task or a tag, with its time per column and in total.
#[derive(Debug)]
pub struct ReportRow {
    /// `#id title` or `#tag`.
    pub label: String,
    /// The task's tags; empty for tag rows.
    pub tags: Vec<String>,
    /// Time in the whole period; for a tag, time two of its tasks ran at once counts once.
    pub total: Duration,
    /// Time per breakdown column, parallel to `Report::columns`.
    pub cells: Vec<Duration>,
}

/// A computed report, rendered both in the desktop app and as markdown (`export`).
#[derive(Debug)]
pub struct Report {
    /// Time worked, by the clock: when tasks ran at once (an agent's subagents, say) that time counts once,
    /// so task rows can add up to more, and tag rows too (a task with several tags counts under each).
    pub total: Duration,
    /// Tasks (or tags) with tracked time, busiest first.
    pub rows: Vec<ReportRow>,
    /// Breakdown column labels (days of a week, weeks of a month); empty for hour/day.
    pub columns: Vec<String>,
    /// Time worked in each breakdown column, counted like `total`.
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
    /// Several brains' reports for the same view as one, worked out again from their sessions so time
    /// tasks of different brains ran at once counts once too. Task rows and timeline entries are named
    /// after their brain (`api · #3 Fix login`), since each brain numbers its entries from 1.
    pub fn merge(parts: Vec<(Option<String>, Report)>, view: &ReportView) -> Report {
        let timeline = parts
            .into_iter()
            .flat_map(|(project, report)| {
                report.timeline.into_iter().map(move |slot| Slot { project: project.clone(), ..slot })
            })
            .collect();
        Report::from_timeline(timeline, view, now())
    }

    /// The report of `timeline`, every session in `view`'s period: rows, breakdown columns and totals.
    fn from_timeline(mut timeline: Vec<Slot>, view: &ReportView, now: DateTime<Local>) -> Report {
        let (from, to) = view.period.bounds();
        // Breakdown columns, clipped to the period (a month's first and last weeks).
        let (columns, ranges): (Vec<String>, Vec<_>) = view
            .period
            .columns()
            .into_iter()
            .map(|(label, p)| {
                let (a, b) = p.bounds();
                (label, (a.max(from), b.min(to)))
            })
            .unzip();
        timeline.sort_by_key(|s| s.start);
        let time = |slots: &[&Slot], (a, b): (DateTime<Local>, DateTime<Local>)| {
            covered(slots.iter().map(|s| s.span(a, b, now)))
        };
        let all: Vec<&Slot> = timeline.iter().collect();
        // Keyed by label; BTreeMap keeps rows with equal time in a stable order.
        let mut groups: BTreeMap<String, (Vec<&Slot>, Vec<String>)> = BTreeMap::new();
        for slot in &timeline {
            for label in row_labels(slot, view.group) {
                let group = groups.entry(label).or_default();
                group.0.push(slot);
                if view.group == Group::Task {
                    group.1.clone_from(&slot.tags);
                }
            }
        }
        let mut rows: Vec<ReportRow> = groups
            .into_iter()
            .map(|(label, (slots, tags))| ReportRow {
                label,
                tags,
                total: time(&slots, (from, to)),
                cells: ranges.iter().map(|&range| time(&slots, range)).collect(),
            })
            .collect();
        rows.sort_by_key(|r| std::cmp::Reverse(r.total));
        Report {
            total: time(&all, (from, to)),
            column_totals: ranges.iter().map(|&range| time(&all, range)).collect(),
            rows,
            columns,
            timeline,
        }
    }
}

impl Brain {
    /// Computes the report for `view`: rows, breakdown columns, totals and the timeline.
    pub fn report(&self, view: &ReportView) -> Report {
        let now = now();
        let (from, to) = view.period.bounds();
        let timeline = self
            .tasks
            .iter()
            .filter(|t| view.tag.as_ref().is_none_or(|tag| t.tags.contains(tag)) && t.matches(&view.query))
            .flat_map(|task| slots(task, from, to, now))
            .collect();
        Report::from_timeline(timeline, view, now)
    }
}

/// The rows a session counts toward: its task, or each of its task's tags.
fn row_labels(slot: &Slot, group: Group) -> Vec<String> {
    match group {
        Group::Task => vec![slot.label()],
        Group::Tag if slot.tags.is_empty() => vec![UNTAGGED.to_string()],
        Group::Tag => slot.tags.iter().map(|tag| format!("#{tag}")).collect(),
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
        project: None,
    })
}
