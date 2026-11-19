//! Calendar periods used by reports: an hour, a day, an ISO week (Monday–Sunday) or a month.

use crate::enums::named_enum;
use crate::model::local_moment;
use jiff::civil::DateTime;
use jiff::{Timestamp, ToSpan, ZonedDateTime};

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
    pub start: DateTime,
}

impl Period {
    /// The period of length `unit` that contains `at`.
    pub fn containing(unit: Unit, at: DateTime) -> Self {
        let d = at.date();
        let day = match unit {
            Unit::Hour | Unit::Day => d,
            Unit::Week => d - d.weekday().to_monday_zero_offset().days(),
            Unit::Month => d.first_of_month(),
        };
        let hour = if unit == Unit::Hour { at.hour() } else { 0 };
        Self { unit, start: day.at(hour, 0, 0, 0) }
    }

    /// The period of length `unit` that contains now.
    pub fn current(unit: Unit) -> Self {
        Self::containing(unit, ZonedDateTime::now().datetime())
    }

    /// The first moment after the period (the start of the next one).
    pub fn end(self) -> DateTime {
        match self.unit {
            Unit::Hour => self.start + 1.hour(),
            Unit::Day => self.start + 1.day(),
            Unit::Week => self.start + 7.days(),
            // The first day of the following month (December rolls over to January).
            Unit::Month => self.start.date().last_of_month().tomorrow().expect("a month after").at(0, 0, 0, 0),
        }
    }

    /// The following period of the same length.
    pub fn next(self) -> Self {
        Self { unit: self.unit, start: self.end() }
    }

    /// The preceding period of the same length.
    pub fn prev(self) -> Self {
        Self::containing(self.unit, self.start - 1.second())
    }

    /// Whether `at` falls inside the period.
    pub fn contains(self, at: DateTime) -> bool {
        self.start <= at && at < self.end()
    }

    /// Switches unit, staying around "now" if this period contains it.
    pub fn with_unit(self, unit: Unit) -> Self {
        let now = ZonedDateTime::now().datetime();
        Self::containing(unit, if self.contains(now) { now } else { self.start })
    }

    /// The period as a `[start, end)` range of local times.
    pub fn bounds(self) -> (Timestamp, Timestamp) {
        (to_local(self.start), to_local(self.end()))
    }

    /// Human-readable name for titles, e.g. `week 39 · Sep 21 – Sep 27 2026`.
    pub fn label(self) -> String {
        match self.unit {
            Unit::Hour => format!(
                "{} {}–{}",
                self.start.strftime("%a %Y-%m-%d"),
                self.start.strftime("%H:%M"),
                self.end().strftime("%H:%M")
            ),
            Unit::Day => self.start.strftime("%A %Y-%m-%d").to_string(),
            Unit::Week => format!(
                "week {} · {} – {}",
                self.start.date().iso_week_date().week(),
                self.start.strftime("%b %d"),
                (self.end() - 1.day()).strftime("%b %d %Y")
            ),
            Unit::Month => self.start.strftime("%B %Y").to_string(),
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
                Unit::Day => p.start.strftime("%a %d").to_string(),
                _ => format!("W{}", p.start.date().iso_week_date().week()),
            };
            cols.push((label, p));
            p = p.next();
        }
        cols
    }
}

/// The moment local time reads `civil`, a period's edge (an hour, a midnight): one a DST change skips is the moment of
/// the change ([`local_moment`]).
fn to_local(civil: DateTime) -> Timestamp {
    local_moment(civil).unwrap_or(Timestamp::UNIX_EPOCH)
}

#[cfg(test)]
#[path = "../tests/period.rs"]
mod tests;
