use super::*;
use chrono::Duration;

#[test]
fn status_changes_are_recorded_once() {
    let t0 = parse_ts("2026-09-24 09:00").unwrap();
    let mut task = Task::new(1, "x".into(), t0);
    assert_eq!(task.status_since(), t0);
    assert!(task.set_status(Status::Doing, t0 + Duration::hours(1), Author::Human));
    assert!(!task.set_status(Status::Doing, t0 + Duration::hours(2), Author::Human), "already doing");
    assert!(task.set_status(Status::Done, t0 + Duration::hours(3), Author::Human));
    assert_eq!(
        task.history,
        [
            StatusChange { at: t0 + Duration::hours(1), from: Status::Todo, to: Status::Doing, by: Author::Human },
            StatusChange { at: t0 + Duration::hours(3), from: Status::Doing, to: Status::Done, by: Author::Human },
        ]
    );
    assert_eq!(task.status_since(), t0 + Duration::hours(3));
}

#[test]
fn leaving_doing_closes_the_session() {
    let t0 = parse_ts("2026-09-24 09:00").unwrap();
    let mut task = Task::new(1, "x".into(), t0);
    task.set_status(Status::Doing, t0, Author::Human);
    task.start_session(t0);
    assert_eq!(task.running_for(t0 + Duration::minutes(5)), Some(Duration::minutes(5)));
    task.set_status(Status::Parked, t0 + Duration::minutes(30), Author::Human);
    assert_eq!(task.sessions[0].end, Some(t0 + Duration::minutes(30)));
    assert_eq!(task.total(t0 + Duration::hours(2)), Duration::minutes(30));
}

#[test]
fn time_between_clips_sessions_to_the_range() {
    let session = Session { start: parse_ts("2026-09-23 23:00").unwrap(), end: parse_ts("2026-09-24 01:30") };
    let (from, to) = (parse_ts("2026-09-24 00:00").unwrap(), parse_ts("2026-09-25 00:00").unwrap());
    assert_eq!(session.overlap(from, to, now()), Duration::minutes(90));
}

#[test]
fn search() {
    let mut task = Task::new(1, "Fix Login page".into(), now());
    task.tags = vec!["prod".into(), "support".into()];
    assert!(task.matches(""));
    assert!(task.matches("login"));
    assert!(task.matches("LOGIN #pro"));
    assert!(task.matches("supp"));
    assert!(!task.matches("#login"));
    assert!(!task.matches("login #backend"));
    assert!(task.matches("##PROD,"), "a tag written as tags are typed, as search reads it");
    task.description = "Flaky on mobile".into();
    assert!(!task.matches("mobile"), "the board filter doesn't look in the description; kb search does");
}

#[test]
fn a_question_waits_for_a_person_until_the_task_is_picked_up() {
    let t0 = now();
    let at = |minutes| t0 + Duration::minutes(minutes);
    let mut task = Task::new(1, "Pick a cache".into(), t0);
    task.set_status(Status::Doing, at(1), Author::Agent);
    task.comments.push(Comment { question: true, ..Comment::new(at(2), "Redis?".into(), Author::Agent) });
    task.set_status(Status::Parked, at(2), Author::Agent);
    assert!(matches!(task.asked(), Some(Asked::Waiting(q)) if q.body == "Redis?"));

    task.comments.push(Comment::new(at(3), "still thinking".into(), Author::Agent));
    assert!(matches!(task.asked(), Some(Asked::Waiting(_))), "an agent's comment isn't an answer");
    task.comments.push(Comment::new(at(4), "Redis".into(), Author::Human));
    assert!(matches!(task.asked(), Some(Asked::Answered(a)) if a.body == "Redis"));

    task.set_status(Status::Doing, at(5), Author::Agent);
    assert_eq!(task.asked(), None, "only parked tasks wait");
    task.set_status(Status::Parked, at(6), Author::Agent);
    assert_eq!(task.asked(), None, "picked up: parked again for another reason");
}

#[test]
fn a_count_takes_the_singular_for_one_only() {
    assert_eq!(counted(1, "session", "sessions"), "1 session");
    assert_eq!(counted(0, "session", "sessions"), "0 sessions");
    assert_eq!(counted(2, "entry", "entries"), "2 entries");
}
