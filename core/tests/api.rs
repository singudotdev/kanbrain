use super::*;
use crate::api::text::line;
use crate::model::{Asked, Author, DecisionState};
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
    brain.comment(task, "root cause found").unwrap();
    brain.change_status(task, Status::Done).unwrap();

    let reloaded = Brain::open(Store::at(brain.store.dir()).unwrap()).unwrap();
    let task = reloaded.entry(task).unwrap();
    assert_eq!((task.status, task.comments.len()), (Status::Done, 1));
    assert_eq!(reloaded.backlinks(task.id).iter().map(|t| t.id).collect::<Vec<_>>(), [note]);
}

#[test]
fn errors_say_what_is_wrong() {
    let (mut brain, _dir) = brain();
    let note = brain.add(new(Kind::Note, "a note")).unwrap();
    let error = |r: anyhow::Result<()>| r.unwrap_err().downcast::<ApiError>().unwrap();
    assert_eq!(error(brain.change_status(note, Status::Doing).map(drop)), ApiError::NotATask(note, Kind::Note));
    assert_eq!(error(brain.comment(99, "x")), ApiError::NotFound(99));
    assert_eq!(error(brain.comment(note, "  ")), ApiError::Empty("comment"));
    assert_eq!(error(brain.link(note, &[note], &[])), ApiError::SelfLink(note));
    assert_eq!(error(brain.add(new(Kind::Task, " ")).map(drop)), ApiError::Empty("title"));
}

#[test]
fn search_reads_descriptions_and_comments() {
    let (mut brain, _dir) = brain();
    let note = brain.add(NewEntry { description: Some("Tokens expire after 5 min".into()), ..new(Kind::Note, "SSO") });
    let note = note.unwrap();
    let task = brain.add(NewEntry { tags: vec!["prod".into()], ..new(Kind::Task, "Fix login") }).unwrap();
    brain.comment(task, "token refresh was the culprit").unwrap();
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
    brain.change_status(done, Status::Done).unwrap();
    let context = brain.context_markdown();
    assert!(context.contains(&format!("## Tasks: doing\n\n- #{task} Fix login")));
    assert!(context.contains("## Decisions") && context.contains("Use markdown files"));
    assert!(context.contains(&format!("## Tasks: done lately\n\n- #{done} Old thing")));
}

#[test]
fn context_shows_where_work_stopped_and_stays_short() {
    let (mut brain, _dir) = brain();
    let task = brain.add(NewEntry { start: true, ..new(Kind::Task, "Fix login") }).unwrap();
    brain.comment(task, &format!("{}\nsecond line", "x".repeat(200))).unwrap();
    for i in 0..30 {
        brain.add(new(Kind::Note, &format!("note {i}"))).unwrap();
    }
    for i in 0..8 {
        let id = brain.add(new(Kind::Task, &format!("finished {i}"))).unwrap();
        brain.change_status(id, Status::Done).unwrap();
    }
    let context = brain.context_markdown();
    let excerpt = format!("  last comment (0m ago): {}…\n", "x".repeat(120));
    assert!(context.contains(&excerpt), "{context}");
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
        projects::write_repo_card(dir, "shop", "").unwrap();
    }
    projects::write_description(central.path(), "shop", "An online shop.").unwrap();
    projects::write_repo_card(&web, "shop", "The React front end.").unwrap();
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
        projects::write_repo_card(dir, "shop", "").unwrap();
    }
    let open = |dir: &std::path::Path| Brain::open_in(Store::at(dir).unwrap(), central.path()).unwrap();
    let total = open(&api).add(new(Kind::Task, "expose the cart total")).unwrap();
    let mut web_brain = open(&web);
    let cart = web_brain.add(new(Kind::Task, "show the cart total")).unwrap();
    let to_api = |id| vec![RepoLink { repo: "api".into(), id }];
    web_brain.link(cart, &[], &to_api(total)).unwrap();
    assert!(web_brain.entry_text(cart).unwrap().contains("\n- api#1 expose the cart total (todo)\n"));
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

    // Deleted by hand in api, it's unlinked once web loads; with api's folder gone for now, links stay.
    web_brain.link(cart, &[], &to_api(total)).unwrap();
    let path = |brain: &Brain, id| brain.entry(id).unwrap().path.clone().unwrap();
    std::fs::remove_file(path(&api_brain, total)).unwrap();
    assert!(open(&web).entry(cart).unwrap().repo_links.is_empty(), "the deleted entry's link is gone");
    assert_eq!(open(&web).entry(note).unwrap().repo_links, [], "the note's too");
    let other = open(&api).add(new(Kind::Task, "another")).unwrap();
    web_brain = open(&web);
    web_brain.link(cart, &[], &to_api(other)).unwrap();
    std::fs::rename(&api, code.path().join("api-away")).unwrap();
    assert_eq!(open(&web).entry(cart).unwrap().repo_links, to_api(other), "kept while api can't be read");
}

#[test]
fn starting_a_task_says_which_one_it_parked() {
    let (mut brain, _dir) = brain();
    let first = brain.add(NewEntry { start: true, ..new(Kind::Task, "first") }).unwrap();
    let second = brain.add(new(Kind::Task, "second")).unwrap();
    assert_eq!(brain.change_status(second, Status::Doing).unwrap(), Some(first));
    assert_eq!(brain.change_status(second, Status::Done).unwrap(), None);
}

#[test]
fn show_is_compact_text() {
    let (mut brain, _dir) = brain();
    let task = brain.add(NewEntry { start: true, ..new(Kind::Task, "Fix login") }).unwrap();
    for _ in 0..4 {
        brain.change_status(task, Status::Parked).unwrap();
        brain.change_status(task, Status::Doing).unwrap();
    }
    brain.add(NewEntry { links: vec![task], ..new(Kind::Decision, "Rotate tokens") }).unwrap();
    let text = brain.entry_text(task).unwrap();
    assert!(text.starts_with(&format!("# #{task} Fix login (doing)\n")), "{text}");
    assert!(text.contains("tracked 0m in 5 session(s), running for 0m"), "{text}");
    assert!(text.contains("## History (latest 5 of 9)"), "{text}");
    assert!(text.contains("## Related\n\n- #2 Rotate tokens (decision)"), "{text}");
    assert!(!text.contains("## Description") && !text.contains("## Comments"), "empty sections are left out");
    assert!(!text.contains("+0") && !text.contains(":00+"), "no offsets or seconds: {text}");
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
    for empty in ["linked_from", "comments", "description", "running"] {
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
