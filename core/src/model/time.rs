//! Timestamps as stored in task files, and ages and durations as shown on screen.

use jiff::civil::DateTime;
use jiff::tz::TimeZone;
use jiff::{SignedDuration, Timestamp, ZonedDateTime};

/// RFC 3339 with the local UTC offset, e.g. `2026-09-24T10:12:00+02:00`.
const TIMESTAMP_FORMAT: &str = "%Y-%m-%dT%H:%M:%S%:z";

/// Local-time formats accepted when a timestamp is typed by hand.
const HAND_TYPED_FORMATS: [&str; 4] = ["%Y-%m-%d %H:%M:%S", "%Y-%m-%d %H:%M", "%Y-%m-%dT%H:%M:%S", "%Y-%m-%dT%H:%M"];

/// Now, truncated to whole seconds, so files round-trip exactly.
pub fn now() -> Timestamp {
    let now = Timestamp::now();
    Timestamp::from_second(now.as_second()).unwrap_or(now)
}

/// `at` in local time, the system's time zone (`TZ`, else the system's setting).
pub fn local(at: Timestamp) -> ZonedDateTime {
    at.to_zoned_datetime(TimeZone::system())
}

/// The moment local time reads `civil`: in the hour a DST change repeats, the first of the two; in the hour it
/// skips, as many minutes past the change as `civil` is past its start, so the change's own minute is the moment it
/// happened. Never none for a time typed in a file by hand, which then reads.
pub fn local_moment(civil: DateTime) -> Option<Timestamp> {
    moment_in(&TimeZone::system(), civil)
}

/// [`local_moment`] in `tz` (RFC 5545's choice,
/// [`AmbiguousTimestamp::compatible`](https://docs.rs/jiff/1.0.1/jiff/tz/struct.AmbiguousTimestamp.html#method.compatible)):
/// none only out of jiff's range of years.
fn moment_in(tz: &TimeZone, civil: DateTime) -> Option<Timestamp> {
    tz.to_ambiguous_timestamp(civil).compatible().ok()
}

/// A timestamp in the stored format.
pub fn fmt_ts(at: Timestamp) -> String {
    fmt_in(at, TimeZone::system())
}

/// [`fmt_ts`] in `tz`.
fn fmt_in(at: Timestamp, tz: TimeZone) -> String {
    at.to_zoned_datetime(tz).strftime(TIMESTAMP_FORMAT).to_string()
}

/// Accepts RFC 3339 plus the hand-typing friendly local formats above.
pub fn parse_ts(s: &str) -> Option<Timestamp> {
    let s = s.trim();
    if let Ok(at) = s.parse::<Timestamp>() {
        return Some(at);
    }
    HAND_TYPED_FORMATS.iter().find_map(|format| DateTime::strptime(format, s).ok()).and_then(local_moment)
}

/// How long ago, rounded down: `<1m`, `5m`, `3h`, `2d`, `6w`.
pub fn fmt_age(d: SignedDuration) -> String {
    let minutes = d.as_mins().max(0);
    match minutes {
        // An age is never none, though times are kept to the second and one read within it is exactly zero.
        0 => "<1m".to_string(),
        ..60 => format!("{minutes}m"),
        60..1440 => format!("{}h", minutes / 60),
        1440..20160 => format!("{}d", minutes / 1440),
        _ => format!("{}w", minutes / 10080),
    }
}

/// `45m`, `1h 05m`; rounded down to whole minutes, but `<1m` for less than one: a report row or a total that took
/// seconds isn't one that took none.
pub fn fmt_dur(d: SignedDuration) -> String {
    let minutes = d.as_mins().max(0);
    match minutes {
        0 if d > SignedDuration::ZERO => "<1m".to_string(),
        60.. => format!("{}h {:02}m", minutes / 60, minutes % 60),
        _ => format!("{minutes}m"),
    }
}

/// `01:02:03` for a live clock.
pub fn fmt_clock(d: SignedDuration) -> String {
    let s = d.as_secs().max(0);
    format!("{:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
}

/// The time `spans` (start, end) cover, counting once what several cover at the same moment: two tasks
/// worked on in parallel for an hour took an hour of the day, not two. Empty or reversed spans count nothing.
pub fn covered(spans: impl IntoIterator<Item = (Timestamp, Timestamp)>) -> SignedDuration {
    let mut spans: Vec<_> = spans.into_iter().filter(|(start, end)| end > start).collect();
    spans.sort_unstable();
    let mut total = SignedDuration::ZERO;
    let mut open: Option<(Timestamp, Timestamp)> = None;
    for (start, end) in spans {
        match &mut open {
            Some((_, until)) if start <= *until => *until = (*until).max(end),
            _ => {
                if let Some((from, until)) = open.replace((start, end)) {
                    total += until - from;
                }
            }
        }
    }
    total + open.map_or(SignedDuration::ZERO, |(from, until)| until - from)
}

#[cfg(test)]
#[path = "../../tests/model/time.rs"]
mod tests;
