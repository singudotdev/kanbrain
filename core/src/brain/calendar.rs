//! What happened on each day of a period, for the calendar: time worked, status changes, comments
//! and new entries. Filtered like the report, by period and tag, and also by kind of interaction.
//! Each entry shows once a day, summed up by its last status change (or else what else happened to
//! it), with the time worked on it and its comments that day: a day of interruptions would
//! otherwise list every park and resume.

use super::Brain;
use crate::enums::named_enum;
use crate::model::{Kind, Status, Task, covered, now};
use crate::period::{Period, Unit};
use chrono::{DateTime, Duration, Local, NaiveDate, NaiveTime};

named_enum! {
    /// Which interactions the calendar shows. `next()` cycles through them.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub enum Show {
        /// Every kind.
        #[default]
        All => "all",
        /// Time worked: the sessions.
        Time => "time",
        /// Status changes.
        Moves => "moves",
        /// Comments written.
        Comments => "comments",
        /// Entries created.
        New => "new",
    }
}

/// What the calendar shows: which period, and which tag and kind of interaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarView {
    /// The day, week or month shown. An hour shows as its whole day.
    pub period: Period,
    /// Only entries carrying this tag.
    pub tag: Option<String>,
    /// Only entries matching this search (see [`Task::matches`]); empty matches all.
    pub query: String,
    /// Only this kind of interaction.
    pub show: Show,
}

impl CalendarView {
    /// Every interaction in `period`.
    pub fn new(period: Period) -> Self {
        Self { period, tag: None, query: String::new(), show: Show::All }
    }
}

/// What happened in an interaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum What {
    /// A session, clipped to its day. `end` is `None` while it's still running.
    Worked {
        /// When it stopped, or the end of the day if it went on past midnight.
        end: Option<DateTime<Local>>,
    },
    /// The task's status changed.
    Moved {
        /// The status before.
        from: Status,
        /// The status after.
        to: Status,
    },
    /// A comment was written.
    Commented {
        /// The comment's first non-empty line.
        excerpt: String,
    },
    /// The entry was created.
    Created,
}

impl What {
    /// Whether `show` includes this kind of interaction.
    fn shown_by(&self, show: Show) -> bool {
        matches!(
            (show, self),
            (Show::All, _)
                | (Show::Time, Self::Worked { .. })
                | (Show::Moves, Self::Moved { .. })
                | (Show::Comments, Self::Commented { .. })
                | (Show::New, Self::Created)
        )
    }
}

/// One interaction with an entry.
#[derive(Debug, Clone)]
pub struct Event {
    /// When it happened; for a session, when it started that day.
    pub at: DateTime<Local>,
    /// The entry's id.
    pub id: u32,
    /// The entry's title.
    pub title: String,
    /// Task, note or decision.
    pub kind: Kind,
    /// The entry's tags.
    pub tags: Vec<String>,
    /// What happened.
    pub what: What,
    /// Which brain it's from, as the index of its calendar in [`Calendar::merge`]; 0 otherwise.
    pub source: usize,
    /// Time worked on the entry that day, up to now while it runs.
    pub worked: Duration,
    /// Comments written on the entry that day.
    pub comments: usize,
}

/// One day of the calendar.
#[derive(Debug)]
pub struct Day {
    /// The date.
    pub date: NaiveDate,
    /// Whether it's in the period. A month's grid starts on a Monday and ends on a Sunday, so its
    /// first and last weeks have days of the months around it, which stay empty.
    pub in_period: bool,
    /// Time worked that day on the entries the tag filter keeps, by the clock: tasks that ran at once
    /// count that time once.
    pub tracked: Duration,
    /// One summary per entry with interactions shown, in time order.
    pub events: Vec<Event>,
    /// The day's sessions, clipped to it, from which `tracked` is worked out again when calendars merge.
    worked: Vec<(DateTime<Local>, DateTime<Local>)>,
}

/// A computed calendar: whole weeks for a month, otherwise the period's days.
#[derive(Debug)]
pub struct Calendar {
    /// Every day of the grid, in order.
    pub days: Vec<Day>,
    /// Time worked in the period, counted like each day's.
    pub tracked: Duration,
}

impl Calendar {
    /// How many interactions are shown.
    pub fn events(&self) -> usize {
        self.days.iter().map(|d| d.events.len()).sum()
    }

    /// Several brains' calendars for the same view as one: each event says which it came from, as
    /// its calendar's index in `parts`.
    pub fn merge(parts: Vec<Calendar>) -> Calendar {
        let mut parts = parts.into_iter().enumerate();
        let Some((_, mut merged)) = parts.next() else {
            return Calendar { days: Vec::new(), tracked: Duration::zero() };
        };
        for (source, calendar) in parts {
            for (day, other) in merged.days.iter_mut().zip(calendar.days) {
                day.worked.extend(other.worked);
                day.tracked = covered(day.worked.iter().copied());
                day.events.extend(other.events.into_iter().map(|e| Event { source, ..e }));
                day.events.sort_by_key(|e| (e.at, e.source, e.id));
            }
        }
        merged.tracked = merged.days.iter().map(|d| d.tracked).sum();
        merged
    }

    /// The ids of the entries shown, each once, in time order.
    pub fn ids(&self) -> Vec<u32> {
        self.entries().into_iter().map(|(_, id)| id).collect()
    }

    /// The entries shown as `(source, id)`, each once, in time order; see [`Calendar::merge`].
    pub fn entries(&self) -> Vec<(usize, u32)> {
        let mut entries = Vec::new();
        for event in self.days.iter().flat_map(|d| &d.events) {
            if !entries.contains(&(event.source, event.id)) {
                entries.push((event.source, event.id));
            }
        }
        entries
    }
}

impl Brain {
    /// Computes the calendar for `view`: each day's interactions and time worked.
    pub fn calendar(&self, view: &CalendarView) -> Calendar {
        let now = now();
        let (first, last) = grid(view.period);
        let tasks: Vec<&Task> = self
            .tasks
            .iter()
            .filter(|t| view.tag.as_ref().is_none_or(|tag| t.tags.contains(tag)) && t.matches(&view.query))
            .collect();
        let days: Vec<Day> = first
            .iter_days()
            .take_while(|&date| date <= last)
            .map(|date| {
                let day = Period::containing(Unit::Day, date.and_time(NaiveTime::MIN));
                let in_period = view.period.contains(day.start) || day.contains(view.period.start);
                if !in_period {
                    return Day { date, in_period, tracked: Duration::zero(), events: Vec::new(), worked: Vec::new() };
                }
                let (from, to) = day.bounds();
                let worked: Vec<_> = tasks.iter().flat_map(|t| &t.sessions).map(|s| s.span(from, to, now)).collect();
                let tracked = covered(worked.iter().copied());
                let mut events: Vec<Event> =
                    tasks.iter().filter_map(|task| summary(task, from, to, now, view.show)).collect();
                events.sort_by_key(|e| (e.at, e.id));
                Day { date, in_period, tracked, events, worked }
            })
            .collect();
        let tracked = days.iter().map(|d| d.tracked).sum();
        Calendar { days, tracked }
    }
}

/// The first and last dates the calendar shows for `period`: Monday to Sunday around a month,
/// otherwise its own days.
fn grid(period: Period) -> (NaiveDate, NaiveDate) {
    let first = period.start.date();
    let last = (period.end() - Duration::seconds(1)).date();
    if period.unit == Unit::Month {
        let monday = Period::containing(Unit::Week, period.start).start.date();
        let sunday = Period::containing(Unit::Week, last.and_time(NaiveTime::MIN)).end().date() - Duration::days(1);
        (monday, sunday)
    } else {
        (first, last)
    }
}

/// `task`'s day in `[from, to)` as one event: its last status change, else its creation, else its
/// last comment, else its last session; with the time worked and the comments of that day. `None`
/// when nothing `show` keeps happened to it.
fn summary(task: &Task, from: DateTime<Local>, to: DateTime<Local>, now: DateTime<Local>, show: Show) -> Option<Event> {
    let mut all: Vec<Event> = interactions(task, from, to, now).into_iter().filter(|e| e.what.shown_by(show)).collect();
    all.sort_by_key(|e| e.at);
    let worked = all
        .iter()
        .map(|e| match e.what {
            What::Worked { end } => end.unwrap_or(now) - e.at,
            _ => Duration::zero(),
        })
        .sum();
    let comments = all.iter().filter(|e| matches!(e.what, What::Commented { .. })).count();
    let last = |pick: fn(&What) -> bool| all.iter().rev().find(|e| pick(&e.what));
    let main = last(|w| matches!(w, What::Moved { .. }))
        .or_else(|| last(|w| matches!(w, What::Created)))
        .or_else(|| last(|w| matches!(w, What::Commented { .. })))
        .or_else(|| all.last())?;
    // A day of several sessions shown as one: from the first's start, so the span holds the time worked beside it.
    let at = match main.what {
        What::Worked { .. } => all.iter().find(|e| matches!(e.what, What::Worked { .. })).map_or(main.at, |e| e.at),
        _ => main.at,
    };
    Some(Event { at, worked, comments, ..main.clone() })
}

/// Everything that happened to `task` in `[from, to)`.
fn interactions(task: &Task, from: DateTime<Local>, to: DateTime<Local>, now: DateTime<Local>) -> Vec<Event> {
    let event = |at, what| Event {
        at,
        id: task.id,
        title: task.title.clone(),
        kind: task.kind,
        tags: task.tags.clone(),
        what,
        source: 0,
        worked: Duration::zero(),
        comments: 0,
    };
    let within = |at: DateTime<Local>| from <= at && at < to;
    let mut events = Vec::new();
    if within(task.created) {
        events.push(event(task.created, What::Created));
    }
    // Moves before sessions, so at the same minute "→ doing" reads before the work it started; the
    // sort by time that follows is stable.
    for change in task.history.iter().filter(|c| within(c.at)) {
        events.push(event(change.at, What::Moved { from: change.from, to: change.to }));
    }
    for session in &task.sessions {
        if session.overlap(from, to, now).is_zero() {
            continue;
        }
        // A session still running past the end of this day ends there; one running now stays open.
        let end = match session.end {
            Some(end) => Some(end.min(to)),
            None if now >= to => Some(to),
            None => None,
        };
        events.push(event(session.start.max(from), What::Worked { end }));
    }
    for comment in task.comments.iter().filter(|c| within(c.created)) {
        let excerpt = comment.body.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or_default().to_string();
        events.push(event(comment.created, What::Commented { excerpt }));
    }
    events
}
