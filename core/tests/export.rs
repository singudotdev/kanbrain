use super::*;
use crate::brain::ReportRow;
use crate::period::Period;
use jiff::civil::DateTime;

fn view(unit: Unit, group: Group, tag: Option<&str>) -> ReportView {
    let at = DateTime::strptime("%Y-%m-%d %H:%M", "2026-09-24 10:00").unwrap();
    ReportView { group, tag: tag.map(String::from), ..ReportView::new(Period::containing(unit, at)) }
}

#[test]
fn file_names() {
    assert_eq!(file_name(&view(Unit::Week, Group::Task, None)), "report-2026-W39.md");
    assert_eq!(file_name(&view(Unit::Day, Group::Tag, None)), "report-2026-09-24-by-tag.md");
    assert_eq!(file_name(&view(Unit::Month, Group::Task, Some("prod"))), "report-2026-09-prod.md");
}

#[test]
fn table_escapes_pipes_and_adds_a_tags_column() {
    let report = Report {
        total: SignedDuration::from_hours(1),
        rows: vec![ReportRow {
            label: "#1 a | b".into(),
            tags: vec!["prod".into()],
            total: SignedDuration::from_hours(1),
            cells: Vec::new(),
        }],
        columns: Vec::new(),
        column_totals: Vec::new(),
        timeline: Vec::new(),
    };
    let now = crate::model::now();
    let markdown = to_markdown(&report, &view(Unit::Day, Group::Task, None), now);
    assert!(markdown.contains("| task | tags | total |"));
    assert!(markdown.contains("| #1 a \\| b | prod | 1h 00m |"));
}

#[test]
fn a_days_timeline_lists_each_session_in_local_time_with_the_gaps_between() {
    use crate::brain::Slot;
    let at = |s: &str| crate::model::local_moment(DateTime::strptime("%Y-%m-%d %H:%M", s).unwrap()).unwrap();
    let slot = |start: &str, end: Option<&str>| Slot {
        start: at(start),
        end: end.map(at),
        id: 1,
        title: "a | b".into(),
        tags: vec!["prod".into(), "api".into()],
        project: None,
    };
    let report = Report {
        total: SignedDuration::from_mins(90),
        rows: vec![ReportRow {
            label: "#1 a | b".into(),
            tags: vec!["prod".into()],
            total: SignedDuration::from_mins(90),
            cells: Vec::new(),
        }],
        columns: Vec::new(),
        column_totals: Vec::new(),
        timeline: vec![slot("2026-09-24 09:00", Some("2026-09-24 10:00")), slot("2026-09-24 10:30", None)],
    };
    let markdown = to_markdown(&report, &view(Unit::Day, Group::Task, None), at("2026-09-24 11:00"));
    let timeline = markdown.split("\n## Timeline\n").nth(1).expect("a day's report has a timeline");
    let rows: Vec<_> = timeline.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(
        rows,
        [
            "| start | end | duration | task | tags |",
            "| --- | --- | ---: | --- | --- |",
            "| 09:00 | 10:00 | 1h 00m | #1 a \\| b | prod, api |",
            "|  |  | 30m | _untracked_ |  |",
            "| 10:30 | running | 30m | #1 a \\| b | prod, api |",
        ]
    );
}
