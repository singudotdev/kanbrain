//! Timestamps as stored in task files, and ages and durations as shown on screen.

use jiff::civil::DateTime;
use jiff::tz::{Offset, TimeZone};
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

/// [`fmt_ts`] in `tz`. An offset with seconds (local mean time, before standard time zones) is written in the offset
/// to the nearest minute, at the same moment: [`rfc3339`] reads none with seconds, as no kanbrain before 0.7.0 did.
fn fmt_in(at: Timestamp, tz: TimeZone) -> String {
    let seconds = tz.to_offset(at).seconds();
    // Only the offset is written, so a fixed one in place of the zone's writes the same.
    let to_the_minute = Offset::from_seconds((seconds + 30 * seconds.signum()) / 60 * 60).map_or(tz, TimeZone::fixed);
    at.to_zoned_datetime(to_the_minute).strftime(TIMESTAMP_FORMAT).to_string()
}

/// Accepts RFC 3339 plus the hand-typing friendly local formats above.
pub fn parse_ts(s: &str) -> Option<Timestamp> {
    let s = s.trim();
    if let Some(at) = rfc3339(s) {
        return Some(at);
    }
    // jiff's `%Y` takes whitespace between a year's sign and its digits (`parse_number` skips it), which chrono's
    // didn't: `+ 2026-…` is no time, as before.
    let sign_apart = s.strip_prefix(['+', '-']).is_some_and(|rest| rest.starts_with(char::is_whitespace));
    if sign_apart {
        return None;
    }
    HAND_TYPED_FORMATS.iter().find_map(|format| DateTime::strptime(format, s).ok()).and_then(local_moment)
}

/// RFC 3339 as chrono 0.4.45's `parse_from_rfc3339`, which every kanbrain before 0.7.0 read files with, took it
/// (`format/parse.rs`, `parse_rfc3339`): `YYYY-MM-DD`, `T`, `t` or a space, `hh:mm:ss`, a fraction of any length (to
/// the nanosecond), then `Z`, `z` or `±hh:mm` up to 23:59, its minus sign `-` or `−` (U+2212). jiff reads more of ISO
/// 8601 (no seconds, `+0200`, a time zone's name, …), which a `### ` line or a history item older versions kept as
/// text would read as a time: what's outside this goes to the hand-typed formats, as it did.
fn rfc3339(s: &str) -> Option<Timestamp> {
    let (date_time, rest) = (s.get(..19)?, s.get(19..)?);
    // By bytes: a range of the `str` could cut a character in two (`−` where a digit or `-` should be) and panic.
    let shape = date_time.bytes().zip(b"dddd-dd-ddTdd:dd:dd").all(|(b, of)| match of {
        b'd' => b.is_ascii_digit(),
        b'T' => matches!(b, b'T' | b't' | b' '),
        _ => b == *of,
    });
    if !shape {
        return None;
    }
    let (fraction, offset) = match rest.strip_prefix('.') {
        Some(rest) => {
            let end = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
            if end == 0 {
                return None;
            }
            (&rest[..end.min(9)], &rest[end..])
        }
        None => ("", rest),
    };
    let offset = match offset {
        "Z" | "z" => "Z".to_string(),
        _ => {
            let (sign, hm) = if let Some(hm) = offset.strip_prefix('+') {
                ('+', hm)
            } else {
                ('-', offset.strip_prefix('-').or_else(|| offset.strip_prefix('\u{2212}'))?)
            };
            let (h, m) = hm.split_once(':')?;
            // What isn't digits jiff refuses.
            let in_range = h.len() == 2 && m.len() == 2 && h <= "23" && m <= "59";
            if !in_range {
                return None;
            }
            format!("{sign}{h}:{m}")
        }
    };
    let fraction = if fraction.is_empty() { String::new() } else { format!(".{fraction}") };
    format!("{date_time}{fraction}{offset}").parse().ok()
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
