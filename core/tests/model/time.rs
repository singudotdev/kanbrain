use super::*;

#[test]
fn hand_typed_and_stored_formats_agree() {
    let typed = parse_ts("2026-09-24 10:12").unwrap();
    assert_eq!(parse_ts(&fmt_ts(typed)), Some(typed));
    assert_eq!(parse_ts("yesterday"), None);
}

#[test]
fn ages() {
    assert_eq!(fmt_age(Duration::seconds(59)), "<1m");
    assert_eq!(fmt_age(Duration::minutes(-5)), "0m");
    assert_eq!(fmt_age(Duration::minutes(65)), "1h");
    assert_eq!(fmt_age(Duration::days(3)), "3d");
    assert_eq!(fmt_age(Duration::days(15)), "2w");
}

#[test]
fn durations() {
    assert_eq!(fmt_dur(Duration::seconds(59)), "<1m");
    assert_eq!(fmt_dur(Duration::zero()), "0m");
    assert_eq!(fmt_dur(Duration::minutes(65)), "1h 05m");
    assert_eq!(fmt_dur(Duration::minutes(-5)), "0m");
    assert_eq!(fmt_clock(Duration::seconds(3723)), "01:02:03");
}

#[test]
fn covered_time_counts_overlaps_once() {
    let at = |minute: i64| parse_ts("2026-09-24 09:00").unwrap() + Duration::minutes(minute);
    // 1h and 2h that share 30 minutes, a span inside another, one apart, and an empty one.
    let spans = [(at(0), at(60)), (at(30), at(150)), (at(40), at(50)), (at(200), at(210)), (at(300), at(300))];
    assert_eq!(covered(spans), Duration::minutes(160));
    assert_eq!(covered([]), Duration::zero());
}
