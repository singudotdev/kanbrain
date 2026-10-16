use super::*;
use crate::api::text::line;
use crate::model::{Asked, Author, Changed, DecisionState};
use crate::store::Store;

/// A brain over a fresh folder; the folder lives as long as the returned guard.
fn brain() -> (Brain, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let brain = Brain::open(Store::at(dir.path()).unwrap()).unwrap();
    (brain, dir)
}

fn new(kind: Kind, title: &str) -> NewEntry {
    NewEntry { kind, title: title.into(), ..NewEntry::default() }
}

#[test]
fn agents_add_link_comment_and_finish_tasks() {
    let (mut brain, _dir) = brain();
    let task = brain.add(new(Kind::Task, "Fix login 500")).unwrap();
    let note =
        brain.add(NewEntry { links: vec![task], description: Some("nginx timeout".into()), ..new(Kind::Note, "SSO") });
    let note = note.unwrap();
    brain.add_comment(task, "root cause found").unwrap();
    brain.set_status(task, Status::Done).unwrap();

    let reloaded = Brain::open(Store::at(brain.store.dir()).unwrap()).unwrap();
    let task = reloaded.entry(task).unwrap();
    assert_eq!((task.status, task.comments.len()), (Status::Done, 1));
    assert_eq!(reloaded.backlinks(task.id).iter().map(|t| t.id).collect::<Vec<_>>(), [note]);
}

/// The API error `r` failed with.
fn error<T: std::fmt::Debug>(r: anyhow::Result<T>) -> ApiError {
    r.unwrap_err().downcast::<ApiError>().unwrap()
}

#[test]
fn errors_say_what_is_wrong() {
    let (mut brain, _dir) = brain();
    let note = brain.add(new(Kind::Note, "a note")).unwrap();
    assert_eq!(error(brain.set_status(note, Status::Doing)), ApiError::NotATask(note, Kind::Note));
    assert_eq!(error(brain.add_comment(99, "x")), ApiError::NotFound(99));
    assert_eq!(error(brain.add_comment(note, "  ")), ApiError::Empty("comment"));
    assert_eq!(error(brain.link(note, &[note], &[])), ApiError::SelfLink(note));
    assert_eq!(error(brain.add(new(Kind::Task, " "))), ApiError::Empty("title"));
}

#[test]
fn search_reads_descriptions_and_comments() {
    let (mut brain, _dir) = brain();
    let note = brain.add(NewEntry { description: Some("Tokens expire after 5 min".into()), ..new(Kind::Note, "SSO") });
    let note = note.unwrap();
    let task = brain.add(NewEntry { tags: vec!["prod".into()], ..new(Kind::Task, "Fix login") }).unwrap();
    brain.add_comment(task, "token refresh was the culprit").unwrap();
    let ids = |query: &str| brain.search(query).iter().map(|t| t.id).collect::<Vec<_>>();
    assert_eq!(ids("token"), [task, note], "newest first");
    assert_eq!(ids("#pro"), [task]);
    assert_eq!(ids("expire 5"), [note]);
}

#[test]
fn context_lists_active_tasks_decisions_and_notes() {
    let (mut brain, _dir) = brain();
    let task = brain.add(NewEntry { start: true, ..new(Kind::Task, "Fix login") }).unwrap();
    brain.add(new(Kind::Decision, "Use markdown files")).unwrap();
    let done = brain.add(new(Kind::Task, "Old thing")).unwrap();
    brain.set_status(done, Status::Done).unwrap();
    let context = brain.context_markdown();
    assert!(context.contains(&format!("## Tasks: doing\n\n- #{task} Fix login")));
    assert!(context.contains("## Decisions") && context.contains("Use markdown files"));
    assert!(context.contains(&format!("## Tasks: done lately\n\n- #{done} Old thing")));
}

#[test]
fn context_shows_where_work_stopped_and_stays_short() {
    let (mut brain, _dir) = brain();
    let task = brain.add(NewEntry { start: true, ..new(Kind::Task, "Fix login") }).unwrap();
    brain.add_comment(task, &format!("{}\nsecond line", "x".repeat(200))).unwrap();
    for i in 0..30 {
        brain.add(new(Kind::Note, &format!("note {i}"))).unwrap();
    }
    for i in 0..8 {
        let id = brain.add(new(Kind::Task, &format!("finished {i}"))).unwrap();
        brain.set_status(id, Status::Done).unwrap();
    }
    let context = brain.context_markdown();
    // `0m`, or `<1m` once a second has passed since.
    let excerpt = format!(" ago): {}…\n", "x".repeat(120));
    assert!(context.contains("  last comment (") && context.contains(&excerpt), "{context}");
    assert!(context.contains("- … 10 more: `kb search` finds them"), "notes are capped");
    let done: Vec<_> = context.lines().filter(|l| l.contains("finished")).collect();
    assert_eq!(done.len(), 5, "only the latest done");
    assert!(!context.contains("Commands:"), "the CLI adds its usage itself");
}

#[test]
fn context_starts_with_what_the_project_is_about() {
    use crate::projects;
    let (central, code) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (api, web) = (code.path().join("api/.brain"), code.path().join("web/.brain"));
    for dir in [&api, &web] {
        std::fs::create_dir_all(dir).unwrap();
        projects::register(central.path(), "shop", dir).unwrap();
        projects::write_repo_card(
            dir,
            &projects::RepoCard { project: "shop".into(), repo: None, notes: String::new() },
        )
        .unwrap();
    }
    projects::write_description(central.path(), "shop", "An online shop.").unwrap();
    projects::write_repo_card(
        &web,
        &projects::RepoCard { project: "shop".into(), repo: None, notes: "The React front end.".into() },
    )
    .unwrap();
    let brain = Brain::open_in(Store::at(&web).unwrap(), central.path()).unwrap();
    let context = brain.context_markdown();
    let expected = format!(
        "## Project shop\n\nAn online shop.\n\nThis is its `web` repository.\n\nThe React front end.\n\n\
         Its other repositories. Work one of them needs, or a problem found in it, is a task there, not a note \
         here: `kb --project shop/NAME add TITLE`, then `kb link ID NAME#ID` relates it to the task here. \
         `kb --project shop/NAME search WORDS` finds its decisions and notes.\n\n\
         - api: {}\n",
        code.path().join("api").display()
    );
    assert!(context.contains(&expected), "{context}");

    let mut brain = brain;
    brain.set_project_description("").unwrap();
    brain.set_repo_notes("").unwrap();
    let context = brain.context_markdown();
    assert!(context.contains("Not described yet. Before other work, describe the application"), "{context}");
    assert!(context.contains("`kb project describe -`"), "{context}");
    assert!(context.contains("Its role isn't described yet: `kb project notes -`"), "{context}");
}

#[test]
fn entries_link_to_other_repositories_of_the_project_and_show_both_ways() {
    use crate::projects;
    let (central, code) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (api, web) = (code.path().join("api/.brain"), code.path().join("web/.brain"));
    for dir in [&api, &web] {
        std::fs::create_dir_all(dir).unwrap();
        projects::register(central.path(), "shop", dir).unwrap();
        projects::write_repo_card(
            dir,
            &projects::RepoCard { project: "shop".into(), repo: None, notes: String::new() },
        )
        .unwrap();
    }
    let open = |dir: &std::path::Path| Brain::open_in(Store::at(dir).unwrap(), central.path()).unwrap();
    let total = open(&api).add(new(Kind::Task, "expose the cart total")).unwrap();
    let mut web_brain = open(&web);
    let cart = web_brain.add(new(Kind::Task, "show the cart total")).unwrap();
    let to_api = |id| vec![RepoLink { repo: "api".into(), id }];
    web_brain.link(cart, &[], &to_api(total)).unwrap();
    assert!(web_brain.entry_text(cart).unwrap().contains("\n- api#1 expose the cart total (todo)\n"));
    // Each logs what its own `- links:` line gained and lost: the other repository's isn't written for it.
    let logged = |brain: &Brain, id| -> Vec<Changed> {
        let task = Brain::open_in(Store::at(brain.store.dir()).unwrap(), central.path()).unwrap();
        task.entry(id).unwrap().activity.iter().map(|a| a.change.clone()).collect()
    };
    let related = |added: &[&str], removed: &[&str]| Changed::Related {
        added: added.iter().map(ToString::to_string).collect(),
        removed: removed.iter().map(ToString::to_string).collect(),
    };
    assert_eq!(logged(&web_brain, cart), [related(&["api#1"], &[])]);
    assert!(open(&api).entry_text(total).unwrap().contains("\n- web#1 show the cart total (todo)\n"), "both ways");

    let error = |result: anyhow::Result<()>| result.unwrap_err().downcast::<ApiError>().unwrap();
    assert_eq!(error(web_brain.link(cart, &[], &to_api(9))), ApiError::NotFoundIn(to_api(9).remove(0)));
    let blog = vec![RepoLink { repo: "blog".into(), id: 1 }];
    assert_eq!(error(web_brain.link(cart, &[], &blog)), ApiError::NoRepo("blog".into()), "not this project's");
    let note = NewEntry { repo_links: to_api(total), ..new(Kind::Note, "totals are in cents") };
    let note = web_brain.add(note).unwrap();
    assert_eq!(web_brain.entry(note).unwrap().repo_links, to_api(total), "add --link takes them too");

    let mut api_brain = open(&api);
    assert!(!api_brain.toggle_repo_link(total, &mut web_brain, cart).unwrap(), "toggling from api unlinks");
    assert!(web_brain.entry(cart).unwrap().repo_links.is_empty(), "from the side that stored it");
    assert!(api_brain.toggle_repo_link(total, &mut web_brain, cart).unwrap(), "and again links, stored in api");
    assert_eq!(api_brain.entry(total).unwrap().repo_links, [RepoLink { repo: "web".into(), id: cart }]);
    assert_eq!(logged(&api_brain, total), [related(&["web#1"], &[])], "only the link it keeps");

    // Deleted by hand in api, it's unlinked once web loads; with api's folder gone for now, links stay.
    web_brain.link(cart, &[], &to_api(total)).unwrap();
    let path = |brain: &Brain, id| brain.entry(id).unwrap().path.clone().unwrap();
    std::fs::remove_file(path(&api_brain, total)).unwrap();
    assert!(open(&web).entry(cart).unwrap().repo_links.is_empty(), "the deleted entry's link is gone");
    assert_eq!(open(&web).entry(note).unwrap().repo_links, [], "the note's too");
    let other = open(&api).add(new(Kind::Task, "another")).unwrap();
    web_brain = open(&web);
    web_brain.link(cart, &[], &to_api(other)).unwrap();
    // An older brain with no `last-id`: below its highest file's id (#2's), a missing entry (#1) was deleted.
    std::fs::remove_file(api.join("last-id")).unwrap();
    let mut gone = web_brain.entry(note).unwrap().clone();
    gone.repo_links = to_api(total);
    Store::at(&web).unwrap().save(&mut gone).unwrap();
    assert_eq!(open(&web).entry(note).unwrap().repo_links, [], "dropped without a last-id too");
    web_brain = open(&web);
    // Neither an entry whose file doesn't parse nor one newer than api's last id, as on a branch from before it,
    // was deleted: links to them stay.
    let mut ahead = web_brain.entry(note).unwrap().clone();
    ahead.repo_links = to_api(99);
    Store::at(&web).unwrap().save(&mut ahead).unwrap();
    std::fs::write(path(&open(&api), other), "not an entry\n").unwrap();
    assert_eq!(open(&web).entry(cart).unwrap().repo_links, to_api(other), "its file is still there");
    assert_eq!(open(&web).entry(note).unwrap().repo_links, to_api(99), "not yet there");
    std::fs::rename(&api, code.path().join("api-away")).unwrap();
    assert_eq!(open(&web).entry(cart).unwrap().repo_links, to_api(other), "kept while api can't be read");
}

#[test]
fn a_person_starting_a_second_task_is_told_which_one_runs() {
    let (mut brain, _dir) = brain();
    let first = brain.add(NewEntry { start: true, ..new(Kind::Task, "first") }).unwrap();
    let second = brain.add(new(Kind::Task, "second")).unwrap();
    let refused = brain.set_status(second, Status::Doing).unwrap_err().to_string();
    assert_eq!(
        refused,
        format!("#{first} is in doing, and a person works on one task at a time: park or finish it first")
    );
    brain.author = Author::Agent;
    brain.set_status(second, Status::Doing).unwrap();
}

#[test]
fn show_is_compact_text() {
    let (mut brain, _dir) = brain();
    let task = brain.add(NewEntry { start: true, ..new(Kind::Task, "Fix login") }).unwrap();
    for _ in 0..4 {
        brain.set_status(task, Status::Parked).unwrap();
        brain.set_status(task, Status::Doing).unwrap();
    }
    brain.add(NewEntry { links: vec![task], ..new(Kind::Decision, "Rotate tokens") }).unwrap();
    let text = brain.entry_text(task).unwrap();
    assert!(text.starts_with(&format!("# #{task} Fix login (doing)\n")), "{text}");
    let tracked =
        ["0m", "<1m"].iter().flat_map(|t| ["0m", "<1m"].map(|r| format!("tracked {t} in 5 sessions, running for {r}")));
    assert!(tracked.into_iter().any(|line| text.contains(&line)), "{text}");
    assert!(text.contains("## History (latest 5 of 9)"), "{text}");
    assert!(text.contains("## Related\n\n- #2 Rotate tokens (decision)"), "{text}");
    assert!(!text.contains("## Description") && !text.contains("## Comments"), "empty sections are left out");
    assert!(!text.contains("## Changes"), "a new entry's links aren't a change: {text}");
    assert!(!text.contains("+0") && !text.contains(":00+"), "no offsets or seconds: {text}");
}

#[test]
fn show_lists_changes_among_the_comments_by_time() {
    let (mut brain, _dir) = brain();
    let id = brain.add(new(Kind::Task, "Fix login")).unwrap();
    let t0 = crate::model::parse_ts("2026-10-16 10:00").unwrap();
    let at = |minutes| t0 + chrono::Duration::minutes(minutes);
    let task = &mut brain.tasks[0];
    task.set_title("Fix the login".into(), at(1), Author::Agent);
    task.set_tags(vec!["ux".into()], at(1), Author::Human);
    task.comments.push(crate::model::Comment::new(at(2), "why".into(), Author::Human));
    task.relate(at(3), Author::Agent, &["shop-api#3".into()], &[]);
    let text = brain.entry_text(id).unwrap();
    let thread = "\n## Comments and changes\n\n\
                  - 2026-10-16 10:01 changed the title from \"Fix login\" to \"Fix the login\" (agent)\n\
                  - 2026-10-16 10:01 added the tag #ux\n\n\
                  ### 2026-10-16 10:02\n\nwhy\n\n\
                  - 2026-10-16 10:03 related it to shop-api#3 (agent)\n";
    assert!(text.contains(thread), "{text}");
    brain.tasks[0].comments.clear();
    assert!(brain.entry_text(id).unwrap().contains("\n## Changes\n\n- 2026-10-16 10:01 changed the title"));

    let json = serde_json::to_value(json::Entry::new(&brain, brain.entry(id).unwrap())).unwrap();
    let fmt_ts = crate::model::fmt_ts;
    let expected = serde_json::json!([
        {"at": fmt_ts(at(1)), "author": "agent", "changed": "title", "from": "Fix login", "to": "Fix the login"},
        {"at": fmt_ts(at(1)), "changed": "tags", "added": ["ux"]},
        {"at": fmt_ts(at(3)), "author": "agent", "changed": "related", "added": ["shop-api#3"]},
    ]);
    assert_eq!(json["activity"], expected);
    // `show` lists only the latest changes, as History's (the JSON, every one): an entry's every tag and link toggled
    // would cost an agent each time.
    for minute in 10..20 {
        brain.tasks[0].set_description(minute.to_string(), at(minute), Author::Human);
    }
    let text = brain.entry_text(id).unwrap();
    assert!(text.contains("\n## Changes (latest 10 of 13 changes)\n\n- 2026-10-16 10:10 changed the description\n"));
    assert!(
        !text.contains("changed the title") && !text.contains("added the tag") && !text.contains("related it"),
        "{text}"
    );
}

#[test]
fn json_has_backlinks_and_no_status_for_notes() {
    let (mut brain, _dir) = brain();
    let task = brain.add(new(Kind::Task, "Fix login")).unwrap();
    let note = brain.add(NewEntry { links: vec![task], ..new(Kind::Note, "SSO") }).unwrap();
    let task_json = serde_json::to_value(json::Entry::new(&brain, brain.entry(task).unwrap())).unwrap();
    assert_eq!(task_json["linked_from"], serde_json::json!([note]));
    assert_eq!(task_json["status"], "todo");
    let note_json = serde_json::to_value(json::Entry::new(&brain, brain.entry(note).unwrap())).unwrap();
    assert!(note_json.get("status").is_none());
    for empty in ["linked_from", "comments", "activity", "description", "running"] {
        assert!(note_json.get(empty).is_none(), "{empty} is left out when empty");
    }
}

#[test]
fn agents_ask_propose_and_people_answer() {
    let (mut brain, _dir) = brain();
    brain.author = Author::Agent;
    let task = brain.add(NewEntry { start: true, ..new(Kind::Task, "Cache sessions") }).unwrap();
    let other = brain.add(new(Kind::Task, "Other")).unwrap();
    brain.ask(other, "Which region?").unwrap();
    let decision = brain.propose(task, NewEntry { description: Some("Redis".into()), ..new(Kind::Task, "Use Redis") });
    let decision = decision.unwrap();
    assert_eq!(brain.entry(decision).unwrap().decision, DecisionState::Proposed);
    assert_eq!(brain.entry(decision).unwrap().links, [task]);
    assert_eq!(brain.entry(task).unwrap().status, Status::Parked);
    assert_eq!(line(brain.entry(task).unwrap()), format!("#{task} Cache sessions (parked, waiting for an answer)"));
    assert_eq!(line(brain.entry(decision).unwrap()), format!("#{decision} Use Redis (decision, proposed)"));

    brain.author = Author::Human;
    let error = |r: Result<bool>| r.unwrap_err().downcast::<ApiError>().unwrap();
    assert_eq!(error(brain.resolve(decision, DecisionState::Rejected, " ")), ApiError::Empty("reason"));
    assert_eq!(error(brain.resolve(task, DecisionState::Accepted, "")), ApiError::NotADecision(task, Kind::Task));
    assert!(brain.resolve(decision, DecisionState::Accepted, "").unwrap());
    assert!(!brain.resolve(decision, DecisionState::Accepted, "").unwrap(), "already accepted");
    let answer = &brain.entry(task).unwrap().comments.last().unwrap().body;
    assert_eq!(answer, &format!("Accepted decision #{decision}, Use Redis."));
    assert!(matches!(brain.entry(task).unwrap().asked(), Some(Asked::Answered(_))));
    assert!(matches!(brain.entry(other).unwrap().asked(), Some(Asked::Waiting(_))), "an unrelated task still waits");

    let context = brain.context_markdown();
    let answered = context.find("## Answered: pick these up first").expect(&context);
    assert!(answered < context.find("## Tasks: parked").unwrap(), "{context}");
    assert!(context.contains(&format!("- #{other} Other · waiting for an answer\n  question (")), "{context}");
    assert!(context.contains("  answer ("), "{context}");
}

#[test]
fn an_agent_giving_the_users_verdict_answers_the_task_that_asked() {
    let (mut brain, _dir) = brain();
    brain.author = Author::Agent;
    let task = brain.add(NewEntry { start: true, ..new(Kind::Task, "Pick a database") }).unwrap();
    let decision = brain.propose(task, new(Kind::Decision, "Use SQLite")).unwrap();
    // Another task linking the decision, but waiting on a question of its own.
    let other = brain.add(NewEntry { links: vec![decision], ..new(Kind::Task, "Pick a region") }).unwrap();
    brain.ask(other, "Which region?").unwrap();
    // As `kb accept` run by an agent when the user said so in the chat.
    assert!(brain.resolve(decision, DecisionState::Accepted, "user said ok").unwrap());
    assert!(matches!(brain.entry(task).unwrap().asked(), Some(Asked::Answered(_))), "not waiting forever");
    assert!(matches!(brain.entry(other).unwrap().asked(), Some(Asked::Waiting(_))), "its own question still waits");
    // A question that names another repository's entry of that number isn't about this decision.
    let cache = brain.add(new(Kind::Decision, "Use Redis")).unwrap();
    let elsewhere = brain.add(NewEntry { links: vec![cache], ..new(Kind::Task, "Check the web") }).unwrap();
    brain.ask(elsewhere, &format!("Is web#{cache} merged?")).unwrap();
    assert!(brain.resolve(cache, DecisionState::Rejected, "not now").unwrap());
    assert!(matches!(brain.entry(elsewhere).unwrap().asked(), Some(Asked::Waiting(_))), "web#N isn't #N");
    let reread = Store::at(brain.store.dir()).unwrap().load().unwrap().0;
    let task = reread.iter().find(|t| t.id == task).unwrap();
    assert!(matches!(task.asked(), Some(Asked::Answered(_))), "and so from the file");
}

#[test]
fn a_question_a_person_asks_waits_for_an_answer_like_an_agents() {
    let (mut brain, _dir) = brain();
    let task = brain.add(new(Kind::Task, "Pick a region")).unwrap();
    brain.ask(task, "Which region, team?").unwrap();
    assert!(matches!(brain.entry(task).unwrap().asked(), Some(Asked::Waiting(_))), "not its own answer");
    brain.add_comment(task, "eu-west").unwrap();
    assert!(matches!(brain.entry(task).unwrap().asked(), Some(Asked::Answered(_))));
}
