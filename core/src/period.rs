//! Calendar periods used by reports: an hour, a day, an ISO week (Monday–Sunday) or a month.

use crate::enums::named_enum;
use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Timelike};

named_enum! {
    /// The length of a report period. `next()` cycles through them in this order (`tab`).
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Unit {
        /// One clock hour, e.g. 14:00–15:00.
        Hour => "hour",
        /// One calendar day.
        Day => "day",
        /// One ISO week, Monday to Sunday.
        Week => "week",
        /// One calendar month.
        Month => "month",
    }
}

/// A calendar-aligned span of local time: one hour, day, ISO week (Mon–Sun) or month.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Period {
    /// How long the period is.
    pub unit: Unit,
    /// Its first moment, in local time (e.g. Monday 00:00 for a week).
    pub start: NaiveDateTime,
}

impl Period {
    /// The period of length `unit` that contains `at`.
    pub fn containing(unit: Unit, at: NaiveDateTime) -> Self {
        let d = at.date();
        let day = match unit {
            Unit::Hour | Unit::Day => d,
            Unit::Week => d - Duration::days(i64::from(d.weekday().num_days_from_monday())),
            Unit::Month => d.with_day(1).expect("day 1 exists"),
        };
        let hour = if unit == Unit::Hour { at.hour() } else { 0 };
        Self { unit, start: day.and_hms_opt(hour, 0, 0).expect("valid hour") }
    }

    /// The period of length `unit` that contains now.
    pub fn current(unit: Unit) -> Self {
        Self::containing(unit, Local::now().naive_local())
    }

    /// The first moment after the period (the start of the next one).
    pub fn end(self) -> NaiveDateTime {
        match self.unit {
            Unit::Hour => self.start + Duration::hours(1),
            Unit::Day => self.start + Duration::days(1),
            Unit::Week => self.start + Duration::days(7),
            // The first day of the following month (December rolls over to January).
            Unit::Month => {
                let d = self.start.date();
                let (y, m) = if d.month() == 12 { (d.year() + 1, 1) } else { (d.year(), d.month() + 1) };
                NaiveDate::from_ymd_opt(y, m, 1).expect("valid month").and_time(NaiveTime::MIN)
            }
        }
    }

    /// The following period of the same length.
    pub fn next(self) -> Self {
        Self { unit: self.unit, start: self.end() }
    }

    /// The preceding period of the same length.
    pub fn prev(self) -> Self {
        Self::containing(self.unit, self.start - Duration::seconds(1))
    }

    /// Whether `at` falls inside the period.
    pub fn contains(self, at: NaiveDateTime) -> bool {
        self.start <= at && at < self.end()
    }

    /// Switches unit, staying around "now" if this period contains it.
    pub fn with_unit(self, unit: Unit) -> Self {
        let now = Local::now().naive_local();
        Self::containing(unit, if self.contains(now) { now } else { self.start })
    }

    /// The period as a `[start, end)` range of local times.
    pub fn bounds(self) -> (DateTime<Local>, DateTime<Local>) {
        (to_local(self.start), to_local(self.end()))
    }

    /// Human-readable name for titles, e.g. `week 39 · Sep 21 – Sep 27 2026`.
    pub fn label(self) -> String {
        match self.unit {
            Unit::Hour => format!(
                "{} {}–{}",
                self.start.format("%a %Y-%m-%d"),
                self.start.format("%H:%M"),
                self.end().format("%H:%M")
            ),
            Unit::Day => self.start.format("%A %Y-%m-%d").to_string(),
            Unit::Week => format!(
                "week {} · {} – {}",
                self.start.iso_week().week(),
                self.start.format("%b %d"),
                (self.end() - Duration::days(1)).format("%b %d %Y")
            ),
            Unit::Month => self.start.format("%B %Y").to_string(),
        }
    }

    /// Sub-periods used as breakdown columns: days of a week, weeks of a month.
    /// Weeks that cross the month edge are included; callers clip them to the month.
    pub fn columns(self) -> Vec<(String, Period)> {
        let sub = match self.unit {
            Unit::Week => Unit::Day,
            Unit::Month => Unit::Week,
            Unit::Hour | Unit::Day => return Vec::new(),
        };
        let mut cols = Vec::new();
        let mut p = Period::containing(sub, self.start);
        while p.start < self.end() {
            let label = match sub {
                Unit::Day => p.start.format("%a %d").to_string(),
                _ => format!("W{}", p.start.iso_week().week()),
            };
            cols.push((label, p));
            p = p.next();
        }
        cols
    }
}

/// Local time for `naive`. A time skipped by a DST change doesn't exist; then use the first
/// valid quarter hour after it (DST gaps are at most an hour).
fn to_local(naive: NaiveDateTime) -> DateTime<Local> {
    (0..=4)
        .find_map(|quarter| Local.from_local_datetime(&(naive + Duration::minutes(15 * quarter))).earliest())
        .unwrap_or_else(|| Local.from_utc_datetime(&naive))
}

#[cfg(test)]
#[path = "../tests/period.rs"]
mod tests;
