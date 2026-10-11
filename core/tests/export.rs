use super::*;
use crate::brain::ReportRow;
use crate::period::Period;
use chrono::NaiveDateTime;

fn view(unit: Unit, group: Group, tag: Option<&str>) -> ReportView {
    let at = NaiveDateTime::parse_from_str("2026-09-24 10:00", "%Y-%m-%d %H:%M").unwrap();
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
        total: Duration::hours(1),
        rows: vec![ReportRow {
            label: "#1 a | b".into(),
            tags: vec!["prod".into()],
            total: Duration::hours(1),
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
