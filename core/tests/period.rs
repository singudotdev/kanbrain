use super::*;

fn at(s: &str) -> NaiveDateTime {
    NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap()
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
