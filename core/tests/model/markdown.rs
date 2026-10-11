use super::*;
use chrono::Duration;

#[test]
fn round_trip() {
    let t0 = parse_ts("2026-09-24 09:00").unwrap();
    let mut task = Task::new(7, "Fix prod login".into(), t0);
    task.description = "customer X reported it\n\n- check logs\n### steps".into();
    task.comments = vec![
        Comment::new(t0, "first\n\nwith a blank line".into(), Author::Human),
        Comment {
            edited: Some(t0 + Duration::minutes(9)),
            ..Comment::new(t0 + Duration::minutes(5), "second".into(), Author::Human)
        },
        Comment::new(t0 + Duration::minutes(7), String::new(), Author::Human),
    ];
    task.tags = vec!["prod".into(), "support".into()];
    task.set_status(Status::Doing, t0 + Duration::minutes(10), Author::Human);
    task.start_session(t0 + Duration::minutes(10));
    task.set_status(Status::Done, t0 + Duration::hours(2), Author::Human);

    let back = Task::from_markdown(&task.to_markdown()).unwrap();
    assert_eq!(back.id, 7);
    assert_eq!(back.title, task.title);
    assert_eq!(back.status, Status::Done);
    assert_eq!(back.created, t0);
    assert_eq!(back.description, task.description);
    assert_eq!(back.comments, task.comments);
    assert_eq!(back.tags, task.tags);
    assert_eq!(back.history, task.history);
    assert_eq!(back.sessions, task.sessions);
    assert_eq!(back.total(t0), Duration::minutes(110));
}

#[test]
fn untracked_tasks_have_no_sessions_section() {
    assert!(!Task::new(1, "x".into(), now()).to_markdown().contains("## Sessions"));
}

#[test]
fn open_sessions_that_cannot_be_running_are_closed() {
    let md = "# x\n\n- id: 1\n- status: parked\n\n## History\n\n- 2026-09-24 11:00 doing -> parked\n\n\
              ## Sessions\n\n- 2026-09-24 10:00 -> running\n- 2026-09-24 09:00 -> running\n";
    let task = Task::from_markdown(md).unwrap();
    assert_eq!(task.sessions[0].end, parse_ts("2026-09-24 10:00"), "closed at the next start");
    assert_eq!(task.sessions[1].end, parse_ts("2026-09-24 11:00"), "closed when the task was parked");
    assert!(!task.is_running());
}

#[test]
fn hand_typed_history_and_missing_status() {
    let md = "# x\n\n- id: 1\n\n## History\n\n- 2026-09-24 10:00 todo -> doing\n";
    let task = Task::from_markdown(md).unwrap();
    assert_eq!(task.status, Status::Todo, "the status line decides, not the history");
    assert_eq!(task.history[0].at, parse_ts("2026-09-24 10:00").unwrap());
    assert_eq!(task.history[0].to, Status::Doing);
    assert!(Task::from_markdown("# x\n\n- id: 1\n\n## History\n\n- 2026-09-24 10:00 todo -> nope\n").is_err());
}

#[test]
fn missing_id_is_an_error() {
    assert!(Task::from_markdown("# no id\n").is_err());
}

#[test]
fn notes_have_a_kind_and_links_but_no_status_or_history() {
    let mut note = Task::new(3, "How deploys work".into(), now());
    note.kind = Kind::Note;
    note.links = vec![1, 2];
    let md = note.to_markdown();
    assert!(md.contains("- kind: note") && md.contains("- links: 1, 2"));
    assert!(!md.contains("- status:") && !md.contains("## History"));
    let back = Task::from_markdown(&md).unwrap();
    assert_eq!((back.kind, back.links), (Kind::Note, vec![1, 2]));
}

#[test]
fn links_accept_hashes_and_skip_repeats() {
    assert_eq!(parse_links("#5 9, 5, x").0, [5, 9]);
}

#[test]
fn links_to_other_repositories_keep_their_name_and_round_trip() {
    let (links, repo_links) = parse_links("5, shop-api#3, shop-api#3, web#x");
    assert_eq!(links, [5]);
    assert_eq!(repo_links, [RepoLink { repo: "shop-api".into(), id: 3 }], "repeats and bad ids skipped");
    let mut task = Task::new(1, "cart total".into(), now());
    (task.links, task.repo_links) = (links, repo_links);
    let md = task.to_markdown();
    assert!(md.contains("- links: 5, shop-api#3\n"), "{md}");
    assert_eq!(Task::from_markdown(&md).unwrap().repo_links, task.repo_links);
}

#[test]
fn what_a_newer_version_wrote_is_written_back() {
    let newer = "# Ship it\n\n- id: 4\n- status: todo\n- tags: \n- links: 2, web#1, blog/web#3, ~5\n\
                 - due: 2026-11-01\n- created: 2026-10-01T09:00:00+02:00\n\n## Description\n\n## Comments\n\n\
                 ## Checklist\n\n- [x] tests\n- [ ] notes\n\n## History\n\n";
    let mut task = Task::from_markdown(newer).unwrap();
    assert_eq!(task.links, [2]);
    task.comments.push(Comment::new(now(), "saved by this version".into(), Author::Human));
    let md = task.to_markdown();
    assert!(md.contains("- links: 2, web#1, blog/web#3, ~5\n- due: 2026-11-01\n"), "{md}");
    assert!(md.ends_with("\n\n## Checklist\n\n- [x] tests\n- [ ] notes\n\n"), "{md}");
    assert_eq!(Task::from_markdown(&md).unwrap().to_markdown(), md, "and stays as it is");
}

#[test]
fn authors_questions_and_decision_states_round_trip() {
    let t0 = parse_ts("2026-09-24 09:00").unwrap();
    let mut task = Task::new(3, "Pick a cache".into(), t0);
    task.comments = vec![
        Comment { question: true, ..Comment::new(t0, "Redis or in-process?".into(), Author::Agent) },
        Comment { unknown: vec!["pinned".into()], ..Comment::new(t0, "Redis".into(), Author::Human) },
    ];
    task.set_status(Status::Parked, t0, Author::Agent);
    let file = task.to_markdown();
    assert!(file.contains("(agent, question)\n") && file.contains("(pinned)\n"), "{file}");
    assert!(file.contains("todo -> parked (agent)\n"), "{file}");
    let back = Task::from_markdown(&file).unwrap();
    assert_eq!((back.comments, back.history), (task.comments, task.history));

    let mut decision = Task::new(4, "Use Redis".into(), t0);
    decision.kind = Kind::Decision;
    decision.decision = DecisionState::Proposed;
    let file = decision.to_markdown();
    assert!(file.contains("- decision: proposed\n"), "{file}");
    assert_eq!(Task::from_markdown(&file).unwrap().decision, DecisionState::Proposed);
    decision.decision = DecisionState::Accepted;
    assert!(!decision.to_markdown().contains("- decision:"), "in force needs no line");
}

#[test]
fn hand_written_headers_and_unknown_authors_still_read() {
    let md = "# x\n\n- id: 1\n- status: parked\n\n## Comments\n\n### 2026-09-24 10:00 (edited 2026-09-24 10:05)\n\n\
              old form\n\n### Steps (draft)\n\nnot a header\n\n## History\n\n- 2026-09-24 11:00 todo -> parked (robot)\n";
    let task = Task::from_markdown(md).unwrap();
    assert_eq!(task.comments.len(), 1);
    assert_eq!(task.comments[0].edited, parse_ts("2026-09-24 10:05"));
    assert!(task.comments[0].body.contains("### Steps (draft)"));
    assert_eq!(task.history[0].by, Author::Human, "an unknown name doesn't lose the file");
}
