use super::*;

#[test]
fn hand_typed_and_stored_formats_agree() {
    let typed = parse_ts("2026-09-24 10:12").unwrap();
    assert_eq!(parse_ts(&fmt_ts(typed)), Some(typed));
    assert_eq!(parse_ts("yesterday"), None);
}

#[test]
fn stored_timestamps_are_written_as_they_always_were() {
    let madrid = jiff::tz::db().get("Europe/Madrid").unwrap();
    let at: Timestamp = "2026-09-24T08:12:00Z".parse().unwrap();
    assert_eq!(fmt_in(at, madrid), "2026-09-24T10:12:00+02:00");
    assert_eq!(fmt_in(at, TimeZone::UTC), "2026-09-24T08:12:00+00:00");
    // Written in another time zone, by an older kanbrain or by hand: the same moment.
    let written = [
        "2026-09-24T10:12:00+02:00",
        "2026-09-24T08:12:00+00:00",
        "2026-09-24T08:12:00Z",
        "2026-09-24t10:12:00+02:00",
        "2026-09-24 10:12:00+02:00",
        "2026-09-24T10:12:00.000+02:00",
    ];
    for written in written {
        assert_eq!(parse_ts(written), Some(at), "{written}");
    }
}

#[test]
fn only_what_older_versions_read_as_a_time_reads_as_one() {
    // chrono's RFC 3339 took a minus sign (U+2212) for the offset's, and the 60th second (a leap second, which it
    // kept as the 59th's last: in whole seconds, the 59th, as jiff reads it).
    assert_eq!(parse_ts("2026-09-24T06:12:00\u{2212}02:00"), parse_ts("2026-09-24T08:12:00Z"));
    assert_eq!(parse_ts("2026-09-24T08:12:60Z"), parse_ts("2026-09-24T08:12:59Z"));
    // ISO 8601 forms jiff reads but chrono didn't, so a `### ` line or a history item written so stays text.
    let refused = [
        "2026-09-24T10:12Z",
        "2026-09-24T10:12+02:00",
        "2026-09-24 10Z",
        "2026-09-24T10+02",
        "20260924T1012Z",
        "20260924T101200Z",
        "2026-09-24T10:12:00+0200",
        "2026-09-24T10:12:00+02",
        "2026-09-24T10:12:00,5Z",
        "2026-09-24T10:12:00+02:00[Europe/Madrid]",
        "2026-09-24T10:12:00+24:00",
        "2026-09-24T10:12:00+02:00:00",
        "+002026-09-24T10:12:00Z",
        "2026\u{2212}09-24T10:12:00Z",
        "+ 2026-09-24 10:12",
        "- 2026-09-24 10:12",
    ];
    for refused in refused {
        assert_eq!(parse_ts(refused), None, "{refused}");
    }
}

#[test]
fn a_time_typed_in_an_hour_a_dst_change_skips_or_repeats_reads() {
    let madrid = jiff::tz::db().get("Europe/Madrid").unwrap();
    let at = |typed| moment_in(&madrid, DateTime::strptime("%Y-%m-%d %H:%M", typed).unwrap()).map(|at| at.to_string());
    // Clocks go from 02:00 to 03:00: the change's minute is the moment it happened (as chrono read it), and a time
    // within the hour skipped as many minutes past it (which chrono, and so a file holding it, didn't read).
    assert_eq!(at("2026-03-29 02:00").as_deref(), Some("2026-03-29T01:00:00Z"));
    assert_eq!(at("2026-03-29 02:30").as_deref(), Some("2026-03-29T01:30:00Z"));
    assert_eq!(at("2026-03-29 03:00").as_deref(), Some("2026-03-29T01:00:00Z"));
    // Clocks go from 03:00 back to 02:00: the first 02:30, still summer time, on every platform (chrono took the
    // second on Linux and macOS, the first on Windows).
    assert_eq!(at("2026-10-25 02:30").as_deref(), Some("2026-10-25T00:30:00Z"));
    assert_eq!(at("2026-10-25 12:00").as_deref(), Some("2026-10-25T11:00:00Z"));
}

#[test]
fn ages() {
    assert_eq!(fmt_age(SignedDuration::from_secs(59)), "<1m");
    assert_eq!(fmt_age(SignedDuration::ZERO), "<1m", "read within the second it started");
    assert_eq!(fmt_age(SignedDuration::from_mins(-5)), "<1m");
    assert_eq!(fmt_age(SignedDuration::from_mins(65)), "1h");
    assert_eq!(fmt_age(SignedDuration::from_hours(72)), "3d");
    assert_eq!(fmt_age(SignedDuration::from_hours(360)), "2w");
}

#[test]
fn durations() {
    assert_eq!(fmt_dur(SignedDuration::from_secs(59)), "<1m");
    assert_eq!(fmt_dur(SignedDuration::ZERO), "0m");
    assert_eq!(fmt_dur(SignedDuration::from_mins(65)), "1h 05m");
    assert_eq!(fmt_dur(SignedDuration::from_mins(-5)), "0m");
    assert_eq!(fmt_clock(SignedDuration::from_secs(3723)), "01:02:03");
}

#[test]
fn covered_time_counts_overlaps_once() {
    let at = |minute: i64| parse_ts("2026-09-24 09:00").unwrap() + SignedDuration::from_mins(minute);
    // 1h and 2h that share 30 minutes, a span inside another, one apart, and an empty one.
    let spans = [(at(0), at(60)), (at(30), at(150)), (at(40), at(50)), (at(200), at(210)), (at(300), at(300))];
    assert_eq!(covered(spans), SignedDuration::from_mins(160));
    assert_eq!(covered([]), SignedDuration::ZERO);
}
