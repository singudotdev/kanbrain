//! Timestamps as stored in task files, and ages and durations as shown on screen.

use chrono::{DateTime, Duration, Local, NaiveDateTime, TimeZone, Timelike};

/// RFC 3339 with the local UTC offset, e.g. `2026-09-24T10:12:00+02:00`.
const TIMESTAMP_FORMAT: &str = "%Y-%m-%dT%H:%M:%S%:z";

/// Local-time formats accepted when a timestamp is typed by hand.
const HAND_TYPED_FORMATS: [&str; 4] = ["%Y-%m-%d %H:%M:%S", "%Y-%m-%d %H:%M", "%Y-%m-%dT%H:%M:%S", "%Y-%m-%dT%H:%M"];

/// Current local time truncated to whole seconds, so files round-trip exactly.
pub fn now() -> DateTime<Local> {
    let now = Local::now();
    now.with_nanosecond(0).unwrap_or(now)
}

/// A timestamp in the stored format.
pub fn fmt_ts(dt: DateTime<Local>) -> String {
    dt.format(TIMESTAMP_FORMAT).to_string()
}

/// Accepts RFC 3339 plus the hand-typing friendly local formats above.
pub fn parse_ts(s: &str) -> Option<DateTime<Local>> {
    let s = s.trim();
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Local));
    }
    HAND_TYPED_FORMATS
        .iter()
        .find_map(|format| NaiveDateTime::parse_from_str(s, format).ok())
        .and_then(|naive| Local.from_local_datetime(&naive).earliest())
}

/// How long ago, rounded down: `<1m`, `5m`, `3h`, `2d`, `6w`.
pub fn fmt_age(d: Duration) -> String {
    let minutes = d.num_minutes().max(0);
    match minutes {
        // As `fmt_dur` says it.
        0 if d > Duration::zero() => "<1m".to_string(),
        ..60 => format!("{minutes}m"),
        60..1440 => format!("{}h", minutes / 60),
        1440..20160 => format!("{}d", minutes / 1440),
        _ => format!("{}w", minutes / 10080),
    }
}

/// `45m`, `1h 05m`; rounded down to whole minutes, but `<1m` for less than one: a report row or a total that took
/// seconds isn't one that took none.
pub fn fmt_dur(d: Duration) -> String {
    let minutes = d.num_minutes().max(0);
    match minutes {
        0 if d > Duration::zero() => "<1m".to_string(),
        60.. => format!("{}h {:02}m", minutes / 60, minutes % 60),
        _ => format!("{minutes}m"),
    }
}

/// `01:02:03` for a live clock.
pub fn fmt_clock(d: Duration) -> String {
    let s = d.num_seconds().max(0);
    format!("{:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
}

/// The time `spans` (start, end) cover, counting once what several cover at the same moment: two tasks
/// worked on in parallel for an hour took an hour of the day, not two. Empty or reversed spans count nothing.
pub fn covered(spans: impl IntoIterator<Item = (DateTime<Local>, DateTime<Local>)>) -> Duration {
    let mut spans: Vec<_> = spans.into_iter().filter(|(start, end)| end > start).collect();
    spans.sort_unstable();
    let mut total = Duration::zero();
    let mut open: Option<(DateTime<Local>, DateTime<Local>)> = None;
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
    total + open.map_or_else(Duration::zero, |(from, until)| until - from)
}

#[cfg(test)]
#[path = "../../tests/model/time.rs"]
mod tests;
