//! Behaviour tests: change a brain over a temporary data folder and check what lands on disk.

use super::*;
use crate::api::ApiError;
use crate::model::{Session, now};
use tempfile::TempDir;

/// A brain over a fresh folder holding `tasks`; the folder lives as long as the fixture.
struct Fixture {
    brain: Brain,
    dir: TempDir,
}

impl Fixture {
    fn new(tasks: Vec<Task>) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::at(dir.path()).unwrap();
        for mut task in tasks {
            store.save(&mut task).unwrap();
        }
        let brain = Brain::open(Store::at(dir.path()).unwrap()).unwrap();
        Self { brain, dir }
    }

    fn with_titles(titles: &[&str]) -> Self {
        let tasks = titles.iter().zip(1..).map(|(title, id)| Task::new(id, (*title).to_string(), now())).collect();
        Self::new(tasks)
    }

    /// What's on disk, independent of the brain's in-memory state.
    fn reloaded(&self) -> Vec<Task> {
        let (mut tasks, warnings) = Store::at(self.dir.path()).unwrap().load().unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");
        tasks.sort_by_key(|t| t.id);
        tasks
    }
}

fn error(result: Result<impl std::fmt::Debug>) -> ApiError {
    result.unwrap_err().downcast().unwrap()
}

#[test]
fn status_changes_are_recorded_once() {
    let mut f = Fixture::with_titles(&["feature"]);
    assert!(f.brain.set_status(1, Status::Doing).unwrap());
    assert!(f.brain.set_status(1, Status::Done).unwrap());
    assert!(!f.brain.set_status(1, Status::Done).unwrap(), "no change, nothing recorded");
    let on_disk = &f.reloaded()[0];
    assert_eq!(on_disk.status, Status::Done);
    let steps: Vec<(Status, Status)> = on_disk.history.iter().map(|c| (c.from, c.to)).collect();
    assert_eq!(steps, [(Status::Todo, Status::Doing), (Status::Doing, Status::Done)]);
}

#[test]
fn only_tasks_have_a_status() {
    let mut f = Fixture::with_titles(&[]);
    let note = f.brain.add(NewEntry { kind: Kind::Note, title: "how deploys work".into(), ..NewEntry::default() });
    let note = note.unwrap();
    assert_eq!(error(f.brain.set_status(note, Status::Doing)), ApiError::NotATask(note, Kind::Note));
    assert_eq!(error(f.brain.set_status(99, Status::Doing)), ApiError::NotFound(99));
}

#[test]
fn columns_show_the_latest_move_first() {
    let mut f = Fixture::with_titles(&["feature", "bug", "docs"]);
    f.brain.set_status(1, Status::Parked).unwrap();
    f.brain.set_status(3, Status::Parked).unwrap();
    let ids = |status| f.brain.column(status).iter().map(|t| t.id).collect::<Vec<_>>();
    assert_eq!(ids(Status::Parked), [3, 1]);
    assert_eq!(ids(Status::Todo), [2]);
    assert_eq!(ids(Status::Done), [0; 0]);
}

#[test]
fn a_person_runs_one_task_and_another_start_is_refused() {
    let mut f = Fixture::with_titles(&["feature", "bug"]);
    f.brain.set_status(1, Status::Doing).unwrap();
    assert_eq!(error(f.brain.set_status(2, Status::Doing)), ApiError::DoingFull(1));
    let new = NewEntry { title: "new".into(), start: true, ..NewEntry::default() };
    assert_eq!(error(f.brain.create(new)), ApiError::DoingFull(1));
    let tasks = f.reloaded();
    assert_eq!(tasks.len(), 2, "a task that can't start isn't created");
    assert_eq!((tasks[0].status, tasks[1].status), (Status::Doing, Status::Todo), "nothing is parked to make room");
    f.brain.set_status(1, Status::Parked).unwrap();
    f.brain.set_status(2, Status::Doing).unwrap();
    assert_eq!(f.brain.person_doing(), Some(2));
}

#[test]
fn agents_start_as_many_tasks_as_they_work_on() {
    let mut f = Fixture::with_titles(&["mine", "api", "web", "docs"]);
    f.brain.set_status(1, Status::Doing).unwrap();
    f.brain.author = Author::Agent;
    for id in [2, 3, 4] {
        f.brain.set_status(id, Status::Doing).unwrap();
    }
    assert_eq!(f.brain.running().count(), 4, "one per subagent, beside the person's");
    assert_eq!(f.brain.person_doing(), Some(1), "only the person's counts");
    f.brain.author = Author::Human;
    f.brain.set_status(4, Status::Parked).unwrap();
    assert_eq!(error(f.brain.set_status(4, Status::Doing)), ApiError::DoingFull(1), "resumed by a person");
    let reopened = Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
    assert_eq!(reopened.running().count(), 3, "loading parks none of the agents'");
}

#[test]
fn parking_for_the_day_stops_peoples_clocks_and_leaves_agents_running() {
    let mut f = Fixture::with_titles(&["feature", "bug"]);
    f.brain.set_status(1, Status::Doing).unwrap();
    f.brain.author = Author::Agent;
    f.brain.set_status(2, Status::Doing).unwrap();
    f.brain.park_people_running().unwrap();
    let tasks = f.reloaded();
    assert_eq!((tasks[0].status, tasks[0].is_running()), (Status::Parked, false));
    assert_eq!((tasks[1].status, tasks[1].is_running()), (Status::Doing, true), "the agent may still be at work");
}

#[test]
fn loading_keeps_one_task_doing() {
    let t0 = now() - chrono::Duration::hours(2);
    let doing = |id: u32, minutes: i64| {
        let mut task = Task::new(id, format!("task {id}"), t0);
        task.set_status(Status::Doing, t0 + chrono::Duration::minutes(minutes), Author::Human);
        task
    };
    let dir = tempfile::tempdir().unwrap();
    let store = Store::at(dir.path()).unwrap();
    for mut task in [doing(1, 30), doing(2, 10)] {
        store.save(&mut task).unwrap();
    }
    let brain = Brain::open(store).unwrap();
    let status = |id| brain.task(id).unwrap().status;
    assert_eq!((status(1), status(2)), (Status::Doing, Status::Parked));
}

#[test]
fn an_entry_sharing_its_id_with_another_gets_a_new_one() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::at(dir.path()).unwrap();
    let mut original = Task::new(5, "p14".into(), now());
    store.save(&mut original).unwrap();
    // A sync tool's conflict copy of it, with the same title, edited on the other machine.
    let copy = dir.path().join("0005-p14 (conflicted copy).md");
    let mut edited = original.clone();
    edited.description = "edited elsewhere".into();
    std::fs::write(&copy, edited.to_markdown()).unwrap();

    let mut brain = Brain::open(Store::at(dir.path()).unwrap()).unwrap();
    let kept = brain.task(5).unwrap();
    assert_eq!((kept.description.as_str(), kept.path.clone()), ("", original.path.clone()), "under its own name");
    let renumbered = brain.tasks.iter().find(|t| t.description == "edited elsewhere").unwrap().id;
    assert_eq!(renumbered, 6);
    assert!(!copy.exists() && dir.path().join("0006-p14.md").exists());
    brain.set_status(5, Status::Done).unwrap();
    assert_eq!(brain.task(renumbered).unwrap().status, Status::Todo, "a command reaches only its own");
    assert_eq!(brain.add(NewEntry { title: "next".into(), ..NewEntry::default() }).unwrap(), 7);
}

#[test]
fn comments_are_added_edited_and_deleted() {
    let mut f = Fixture::with_titles(&["feature"]);
    f.brain.add_comment(1, "first").unwrap();
    f.brain.add_comment(1, "second").unwrap();
    f.brain.update_comment(1, 0, "first, edited").unwrap();
    f.brain.delete_comment(1, 1).unwrap();
    let comments = &f.reloaded()[0].comments;
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].body, "first, edited");
    assert!(comments[0].edited.is_some());
    assert_eq!(error(f.brain.add_comment(1, "  ")), ApiError::Empty("comment"));
    assert_eq!(error(f.brain.delete_comment(1, 5)), ApiError::NoComment(1, 5));
}

#[test]
fn links_toggle_from_either_side() {
    let mut f = Fixture::with_titles(&["feature", "bug"]);
    assert!(f.brain.toggle_link(1, 2).unwrap());
    assert_eq!(f.brain.related(2).iter().map(|t| t.id).collect::<Vec<_>>(), [1]);
    assert!(!f.brain.toggle_link(2, 1).unwrap(), "unlinked from the other side");
    assert!(f.reloaded().iter().all(|t| t.links.is_empty()));
    assert_eq!(error(f.brain.toggle_link(1, 1)), ApiError::SelfLink(1));
}

#[test]
fn each_brain_runs_its_own_task() {
    // The other brain is the central one; this one is a registered project.
    let central = Fixture::with_titles(&["elsewhere"]);
    let mut central_brain = Brain::open(Store::at(central.dir.path()).unwrap()).unwrap();
    central_brain.set_status(1, Status::Doing).unwrap();
    let mut f = Fixture::with_titles(&["here"]);
    crate::projects::register(central.dir.path(), "here", f.dir.path()).unwrap();
    f.brain.central = Some(central.dir.path().to_path_buf());
    f.brain.set_status(1, Status::Doing).unwrap();
    assert!(central.reloaded()[0].is_running(), "starting here leaves the central brain's task running");
}

#[test]
fn deleting_an_entry_removes_the_links_to_it() {
    let mut f = Fixture::with_titles(&["feature", "fix", "note"]);
    f.brain.link(1, &[3], &[]).unwrap();
    f.brain.link(2, &[3, 1], &[]).unwrap();
    f.brain.delete(3).unwrap();
    let links: Vec<_> = f.reloaded().iter().map(|t| (t.id, t.links.clone())).collect();
    assert_eq!(links, [(1, vec![]), (2, vec![1])]);
}

#[test]
fn ids_of_deleted_entries_are_never_given_again() {
    let mut f = Fixture::with_titles(&["feature", "fix", "note"]);
    let add = |brain: &mut Brain| brain.add(NewEntry { title: "new".into(), ..NewEntry::default() }).unwrap();
    f.brain.delete(3).unwrap();
    assert_eq!(add(&mut f.brain), 4, "not the deleted 3");
    std::fs::remove_file(f.brain.task(4).unwrap().path.clone().unwrap()).unwrap();
    let mut reopened = Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
    assert_eq!(add(&mut reopened), 5, "nor 4, deleted by hand");
}

#[test]
fn moving_an_entry_renumbers_it_drops_its_links_and_keeps_the_rest() {
    let mut from = Fixture::with_titles(&["feature", "fix"]);
    let mut to = Fixture::with_titles(&["there", "running there"]);
    from.brain.link(2, &[1], &[]).unwrap();
    from.brain.add_comment(1, "halfway").unwrap();
    from.brain.set_status(1, Status::Doing).unwrap();
    to.brain.set_status(2, Status::Doing).unwrap();
    assert_eq!(error(from.brain.move_to(1, &mut to.brain)), ApiError::DoingFull(2), "no room there");
    to.brain.set_status(2, Status::Parked).unwrap();

    assert_eq!(from.brain.move_to(1, &mut to.brain).unwrap(), 3, "the next free id there");
    let moved = &to.reloaded()[2];
    assert_eq!((moved.title.as_str(), moved.status, moved.comments.len()), ("feature", Status::Doing, 1));
    assert!(moved.is_running() && moved.links.is_empty(), "keeps its clock, not its links");
    let left: Vec<_> = from.reloaded().iter().map(|t| (t.id, t.links.clone())).collect();
    assert_eq!(left, [(2, vec![])], "gone from here, and nothing links to it");
    assert!(matches!(error(from.brain.move_to(1, &mut to.brain)), ApiError::NotFound(1)));
}

#[test]
fn loading_drops_links_to_entries_deleted_by_hand() {
    let mut feature = Task::new(1, "feature".into(), now());
    feature.links = vec![2, 7];
    let f = Fixture::new(vec![feature, Task::new(2, "fix".into(), now())]);
    assert_eq!(f.brain.task(1).unwrap().links, [2]);
    assert_eq!(f.reloaded()[0].links, [2], "and the file is fixed");
}

#[test]
fn reading_only_leaves_the_files_as_they_are() {
    let dir = tempfile::tempdir().unwrap();
    let mut feature = Task::new(1, "feature".into(), now());
    feature.links = vec![7];
    feature.tags = vec!["new".into()];
    Store::at(dir.path()).unwrap().save(&mut feature).unwrap();
    let files = || {
        let mut files: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().path())
            .map(|p| (p.clone(), std::fs::read_to_string(p).unwrap()))
            .collect();
        files.sort();
        files
    };
    let before = files();
    let brain = Brain::read_only(Store::at(dir.path()).unwrap()).unwrap();
    assert_eq!(brain.task(1).unwrap().links, [7], "as written, not repaired");
    assert_eq!(files(), before, "no last-id, tags.md or fixed link written");
}

#[test]
fn updating_renames_the_file_and_deleting_removes_it() {
    let mut f = Fixture::with_titles(&["feature"]);
    f.brain.update(1, " Export CSV ", vec!["backend".into()]).unwrap();
    assert!(f.dir.path().join("0001-export-csv.md").is_file());
    assert_eq!(f.reloaded()[0].tags, ["backend"]);
    assert_eq!(error(f.brain.update(1, " ", Vec::new())), ApiError::Empty("title"));
    f.brain.delete(1).unwrap();
    assert!(f.reloaded().is_empty());
}

#[test]
fn report_sums_sessions_per_task_and_tag() {
    use crate::period::{Period, Unit};
    use chrono::Duration;
    let t0 = crate::model::parse_ts("2026-09-24 09:00").unwrap();
    let session = |start: i64, minutes: i64| Session {
        start: t0 + Duration::minutes(start),
        end: Some(t0 + Duration::minutes(start + minutes)),
    };
    let mut feature = Task::new(1, "feature".into(), t0);
    feature.tags = vec!["backend".into()];
    feature.sessions = vec![session(0, 60), session(90, 30)];
    let mut fix = Task::new(2, "fix".into(), t0);
    fix.tags = vec!["backend".into(), "prod".into()];
    fix.sessions = vec![session(60, 30)];
    let f = Fixture::new(vec![feature, fix]);

    let mut view = ReportView::new(Period::containing(Unit::Day, t0.naive_local()));
    let report = f.brain.report(&view);
    assert_eq!(report.total, Duration::minutes(120));
    let rows: Vec<_> = report.rows.iter().map(|r| (r.label.as_str(), r.total.num_minutes())).collect();
    assert_eq!(rows, [("#1 feature", 90), ("#2 fix", 30)]);
    assert_eq!(report.timeline.len(), 3);

    view.group = Group::Tag;
    let rows: Vec<_> = f.brain.report(&view).rows.iter().map(|r| (r.label.clone(), r.total.num_minutes())).collect();
    assert_eq!(rows, [("#backend".to_string(), 120), ("#prod".to_string(), 30)]);

    view.query = "fix".into();
    assert_eq!(f.brain.report(&view).total, Duration::minutes(30), "a search keeps only the tasks matching it");
}

#[test]
fn the_calendar_lists_each_days_interactions() {
    use crate::model::{Comment, StatusChange};
    use crate::period::{Period, Unit};
    use chrono::Duration;
    let t0 = crate::model::parse_ts("2026-09-24 09:00").unwrap();
    let at = |minutes: i64| t0 + Duration::minutes(minutes);
    let mut feature = Task::new(1, "feature".into(), t0);
    feature.tags = vec!["backend".into()];
    feature.status = Status::Parked;
    feature.history = vec![
        StatusChange { at: at(10), from: Status::Todo, to: Status::Doing, by: Author::Human },
        StatusChange { at: at(70), from: Status::Doing, to: Status::Parked, by: Author::Human },
    ];
    feature.sessions = vec![Session { start: at(10), end: Some(at(70)) }];
    feature.comments = vec![Comment::new(at(30), "\nhalf way\nmore".into(), Author::Human)];
    // A note from the next day, and a session that runs past midnight.
    let mut note = Task::new(2, "gotcha".into(), at(24 * 60));
    note.kind = Kind::Note;
    let mut late = Task::new(3, "late".into(), at(-60));
    late.status = Status::Done;
    late.sessions = vec![Session { start: at(14 * 60), end: Some(at(16 * 60)) }];
    let f = Fixture::new(vec![feature, note, late]);

    let mut view = CalendarView::new(Period::containing(Unit::Week, t0.naive_local()));
    let calendar = f.brain.calendar(&view);
    assert_eq!(calendar.days.len(), 7);
    let thursday = &calendar.days[3];
    let what: Vec<_> = thursday.events.iter().map(|e| (e.id, e.what.clone(), e.worked, e.comments)).collect();
    // One line per entry and day: #3 was only created that day (and worked late), #1's last move wins
    // over its creation, the move into doing, its session and its comment.
    assert_eq!(
        what,
        [
            (3, What::Created, Duration::minutes(60), 0),
            (1, What::Moved { from: Status::Doing, to: Status::Parked }, Duration::minutes(60), 1),
        ]
    );
    assert_eq!(thursday.tracked, Duration::minutes(60 + 60), "the late session counts until midnight");
    let friday = &calendar.days[4];
    assert_eq!(friday.events.iter().map(|e| e.id).collect::<Vec<_>>(), [3, 2], "midnight, then the note");
    assert_eq!(calendar.tracked, Duration::minutes(180));
    assert_eq!(calendar.ids(), [3, 1, 2]);

    view.show = Show::Comments;
    assert_eq!(f.brain.calendar(&view).events(), 1);
    view.show = Show::All;
    view.tag = Some("backend".into());
    let calendar = f.brain.calendar(&view);
    assert_eq!((calendar.ids(), calendar.tracked), (vec![1], Duration::minutes(60)));
}

#[test]
fn a_calendar_day_of_several_sessions_spans_from_the_first() {
    use crate::period::{Period, Unit};
    use chrono::Duration;
    let t0 = crate::model::parse_ts("2026-09-24 09:00").unwrap();
    let session =
        |start: i64, end: i64| Session { start: t0 + Duration::minutes(start), end: Some(t0 + Duration::minutes(end)) };
    let mut task = Task::new(1, "twice".into(), t0 - Duration::days(1));
    task.sessions = vec![session(0, 10), session(60, 61)];
    let f = Fixture::new(vec![task]);
    let mut view = CalendarView::new(Period::containing(Unit::Day, t0.naive_local()));
    view.show = Show::Time;
    let calendar = f.brain.calendar(&view);
    let event = &calendar.days[0].events[0];
    let end = match event.what {
        What::Worked { end } => end,
        _ => None,
    };
    assert_eq!((event.at, end, event.worked), (t0, Some(t0 + Duration::minutes(61)), Duration::minutes(11)));
}

#[test]
fn the_timeline_shows_untracked_gaps_of_five_minutes_or_more() {
    use crate::period::{Period, Unit};
    use chrono::Duration;
    let t0 = crate::model::parse_ts("2026-09-24 09:00").unwrap();
    let session =
        |start: i64, end: i64| Session { start: t0 + Duration::minutes(start), end: Some(t0 + Duration::minutes(end)) };
    let mut long = Task::new(1, "long".into(), t0);
    // 09:00-10:00, then 10:04-10:10 (a 4-minute pause: not shown) and 10:30-10:40 (20 minutes: shown).
    long.sessions = vec![session(0, 60), session(64, 70), session(90, 100)];
    let mut inside = Task::new(2, "inside".into(), t0);
    // 09:10-09:20 runs within the first: no gap before it, and 10:04 still counts from 10:00.
    inside.sessions = vec![session(10, 20)];
    let report = Fixture::new(vec![long, inside])
        .brain
        .report(&ReportView::new(Period::containing(Unit::Day, t0.naive_local())));
    let gaps: Vec<_> = report
        .timeline_with_gaps(t0 + Duration::minutes(120))
        .iter()
        .map(|(gap, slot)| (slot.id, gap.map(|g| g.num_minutes())))
        .collect();
    assert_eq!(gaps, [(1, None), (2, None), (1, None), (1, Some(20))]);
}

#[test]
fn reports_and_calendars_of_several_brains_merge_counting_overlaps_once() {
    use crate::period::{Period, Unit};
    use chrono::Duration;
    let t0 = crate::model::parse_ts("2026-09-24 09:00").unwrap();
    let worked = |id: u32, title: &str, start: i64| {
        let mut task = Task::new(id, title.into(), t0);
        task.tags = vec!["backend".into()];
        task.sessions =
            vec![Session { start: t0 + Duration::minutes(start), end: Some(t0 + Duration::minutes(start + 30)) }];
        task
    };
    // 09:00–09:30 and 09:15–09:45: 45 minutes of the day, though each task has 30.
    let (api, web) = (Fixture::new(vec![worked(1, "fix", 0)]), Fixture::new(vec![worked(1, "export", 15)]));
    let mut view = ReportView::new(Period::containing(Unit::Day, t0.naive_local()));
    let merged = Report::merge(
        vec![(Some("api".into()), api.brain.report(&view)), (Some("web".into()), web.brain.report(&view))],
        &view,
    );
    let rows: Vec<_> = merged.rows.iter().map(|r| (r.label.as_str(), r.total.num_minutes())).collect();
    assert_eq!(rows, [("api · #1 fix", 30), ("web · #1 export", 30)], "each task keeps its own time");
    assert_eq!(merged.total, Duration::minutes(45), "the overlap counts once");
    assert_eq!(merged.timeline[1].label(), "web · #1 export");
    view.group = Group::Tag;
    let merged = Report::merge(vec![(None, api.brain.report(&view)), (None, web.brain.report(&view))], &view);
    let rows: Vec<_> = merged.rows.iter().map(|r| (r.label.as_str(), r.total.num_minutes())).collect();
    assert_eq!(rows, [("#backend", 45)], "one #backend row, by the clock too");

    let view = CalendarView::new(Period::containing(Unit::Day, t0.naive_local()));
    let calendar = Calendar::merge(vec![api.brain.calendar(&view), web.brain.calendar(&view)]);
    assert_eq!(calendar.entries(), [(0, 1), (1, 1)]);
    assert_eq!(calendar.tracked, Duration::minutes(45));
}

#[test]
fn a_months_calendar_is_whole_weeks() {
    use crate::period::{Period, Unit};
    let september = Period::containing(Unit::Month, crate::model::parse_ts("2026-09-24 09:00").unwrap().naive_local());
    let calendar = Fixture::with_titles(&[]).brain.calendar(&CalendarView::new(september));
    let first = &calendar.days[0];
    assert_eq!((first.date.to_string(), first.in_period), ("2026-08-31".to_string(), false));
    let last = calendar.days.last().unwrap();
    assert_eq!((last.date.to_string(), calendar.days.len()), ("2026-10-04".to_string(), 35));
    assert_eq!(calendar.days.iter().filter(|d| d.in_period).count(), 30);
}

#[test]
fn a_new_decision_starts_from_the_adr_sections() {
    let mut f = Fixture::with_titles(&[]);
    let id = f.brain.add(NewEntry { kind: Kind::Decision, title: "Use markdown".into(), ..NewEntry::default() });
    let decision = f.brain.task(id.unwrap()).unwrap();
    assert!(decision.description.starts_with("### Context"));
}
