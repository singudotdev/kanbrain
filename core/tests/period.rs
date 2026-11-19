use super::*;

fn at(s: &str) -> DateTime {
    DateTime::strptime("%Y-%m-%d %H:%M", s).unwrap()
}

#[test]
fn alignment() {
    let t = at("2026-09-24 14:37"); // Thursday
    assert_eq!(Period::containing(Unit::Hour, t).start, at("2026-09-24 14:00"));
    assert_eq!(Period::containing(Unit::Day, t).start, at("2026-09-24 00:00"));
    assert_eq!(Period::containing(Unit::Week, t).start, at("2026-09-21 00:00"));
    assert_eq!(Period::containing(Unit::Month, t).start, at("2026-09-01 00:00"));
}

#[test]
fn month_navigation_crosses_years() {
    let dec = Period::containing(Unit::Month, at("2026-12-10 00:00"));
    assert_eq!(dec.next().start, at("2027-01-01 00:00"));
    assert_eq!(dec.next().prev(), dec);
    assert_eq!(dec.prev().start, at("2026-11-01 00:00"));
}

#[test]
fn columns() {
    let week = Period::containing(Unit::Week, at("2026-09-24 00:00"));
    assert_eq!(week.columns().len(), 7);
    assert_eq!(week.columns()[0].0, "Mon 21");
    // Sept 2026 starts on a Tuesday and ends on a Wednesday: W36..W40.
    let month = Period::containing(Unit::Month, at("2026-09-24 00:00"));
    let labels: Vec<String> = month.columns().into_iter().map(|c| c.0).collect();
    assert_eq!(labels, ["W36", "W37", "W38", "W39", "W40"]);
    assert_eq!(Period::containing(Unit::Day, at("2026-09-24 00:00")).columns().len(), 0);
}

#[test]
fn each_period_ends_where_the_next_starts_and_names_itself() {
    let t = at("2026-09-24 14:37"); // Thursday of ISO week 39
    let cases = [
        (Unit::Hour, "2026-09-24 15:00", "Thu 2026-09-24 14:00–15:00"),
        (Unit::Day, "2026-09-25 00:00", "Thursday 2026-09-24"),
        (Unit::Week, "2026-09-28 00:00", "week 39 · Sep 21 – Sep 27 2026"),
        (Unit::Month, "2026-10-01 00:00", "September 2026"),
    ];
    for (unit, end, label) in cases {
        let period = Period::containing(unit, t);
        assert_eq!((period.end(), period.label()), (at(end), label.to_string()), "{unit:?}");
    }
}
