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

#[test]
fn edits_are_logged_with_who_made_them_and_only_when_they_change_something() {
    let t0 = parse_ts("2026-10-16 10:00").unwrap();
    let mut task = Task::new(1, "Old".into(), t0);
    task.tags = vec!["bug".into(), "api".into()];
    assert!(!task.set_title("Old".into(), t0, Author::Agent));
    assert!(!task.set_tags(vec!["bug".into(), "api".into()], t0, Author::Agent));
    assert!(!task.set_description(String::new(), t0, Author::Agent));
    assert!(task.activity.is_empty(), "nothing changed");

    let t1 = t0 + Duration::minutes(1);
    assert!(task.set_title("New".into(), t1, Author::Agent));
    assert!(task.set_tags(vec!["api".into(), "ux".into()], t1, Author::Human));
    assert!(task.set_description("text".into(), t1, Author::Agent));
    task.relate(t1, Author::Agent, &["5".into()], &[]);
    assert_eq!((task.title.as_str(), task.description.as_str()), ("New", "text"));
    assert_eq!(task.tags, ["api", "ux"]);
    let words = |w: &[&str]| w.iter().map(ToString::to_string).collect::<Vec<_>>();
    assert_eq!(
        task.activity,
        [
            Activity::new(t1, Author::Agent, Changed::Title { from: "Old".into(), to: "New".into() }),
            Activity::new(t1, Author::Human, Changed::Tags { added: words(&["ux"]), removed: words(&["bug"]) }),
            Activity::new(t1, Author::Agent, Changed::Description),
            Activity::new(t1, Author::Agent, Changed::Related { added: words(&["5"]), removed: vec![] }),
        ]
    );
}

#[test]
fn changes_one_author_makes_in_the_same_second_are_one() {
    let t0 = parse_ts("2026-10-16 10:00").unwrap();
    let words = |w: &[&str]| w.iter().map(ToString::to_string).collect::<Vec<_>>();
    let mut task = Task::new(1, "A".into(), t0);
    // The app saves a form's related entries one at a time.
    task.relate(t0, Author::Human, &words(&["5"]), &[]);
    task.relate(t0, Author::Human, &words(&["6"]), &words(&["9"]));
    task.set_title("B".into(), t0, Author::Human);
    task.set_title("C".into(), t0, Author::Human);
    assert_eq!(
        task.activity,
        [
            Activity::new(t0, Author::Human, Changed::Related { added: words(&["5", "6"]), removed: words(&["9"]) }),
            Activity::new(t0, Author::Human, Changed::Title { from: "A".into(), to: "C".into() }),
        ],
        "the latest change of a kind takes in the next"
    );
    task.relate(t0, Author::Human, &[], &words(&["5", "6"]));
    task.relate(t0, Author::Human, &words(&["9"]), &[]);
    task.set_title("A".into(), t0, Author::Human);
    assert!(task.activity.is_empty(), "undone in the same second: {:?}", task.activity);

    task.set_description("x".into(), t0, Author::Human);
    task.set_description("y".into(), t0, Author::Human);
    task.set_description("z".into(), t0, Author::Agent);
    task.set_description("w".into(), t0 + Duration::seconds(1), Author::Agent);
    assert_eq!(task.activity.len(), 3, "another author, or another second, is another change: {:?}", task.activity);
}

#[test]
fn a_change_says_what_it_did() {
    let words = |w: &[&str]| w.iter().map(ToString::to_string).collect::<Vec<_>>();
    let said = [
        (Changed::Title { from: "A".into(), to: "B".into() }, r#"changed the title from "A" to "B""#),
        (Changed::Description, "changed the description"),
        (Changed::Tags { added: words(&["ux"]), removed: vec![] }, "added the tag #ux"),
        (
            Changed::Tags { added: words(&["ux", "a"]), removed: words(&["bug"]) },
            "added the tags #ux, #a and removed the tag #bug",
        ),
        (Changed::Tags { added: vec![], removed: words(&["a", "b"]) }, "removed the tags #a, #b"),
        (
            Changed::Related { added: words(&["5", "shop-api#3"]), removed: words(&["9"]) },
            "related it to #5, shop-api#3 and unrelated it from #9",
        ),
        (Changed::Related { added: vec![], removed: words(&["9"]) }, "unrelated it from #9"),
    ];
    for (change, text) in said {
        assert_eq!(change.said(), text);
    }
}

#[test]
fn the_thread_has_comments_and_changes_by_time_a_change_first_in_its_second() {
    let t0 = parse_ts("2026-10-16 10:00").unwrap();
    let mut task = Task::new(1, "A".into(), t0);
    let at = |minutes| t0 + Duration::minutes(minutes);
    task.comments =
        vec![Comment::new(at(1), "one".into(), Author::Human), Comment::new(at(3), "two".into(), Author::Agent)];
    task.set_title("B".into(), at(0), Author::Human);
    task.set_description("x".into(), at(3), Author::Human);
    task.set_tags(vec!["ux".into()], at(5), Author::Agent);
    let thread: Vec<String> = task
        .thread()
        .into_iter()
        .map(|item| match item {
            ThreadItem::Comment(index, comment) => format!("{index}:{}", comment.body),
            ThreadItem::Change(change) => change.change.said(),
        })
        .collect();
    assert_eq!(
        thread,
        [r#"changed the title from "A" to "B""#, "0:one", "changed the description", "1:two", "added the tag #ux"]
    );
}
