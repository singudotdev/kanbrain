use super::*;

fn parse(args: &[&str]) -> Result<EntryArgs> {
    let (name, rest) = args.split_first().unwrap();
    Ok(EntryArgs::parse(name, rest.iter().map(|s| (*s).to_string()))?.expect("an entry command"))
}

#[test]
fn add_takes_options_anywhere_and_the_rest_as_title() {
    let args = parse(&["add", "--decision", "Use", "--tag", "#Arch", "markdown", "--link", "#3", "--json"]).unwrap();
    assert!(args.json);
    let EntryCommand::Add(add) = args.command else { panic!("{:?}", args.command) };
    assert_eq!(add.kind, Kind::Decision);
    assert_eq!(
        (add.title.as_str(), add.tags.as_slice(), add.links.as_slice()),
        ("Use markdown", &["arch".to_string()][..], &[3][..])
    );
    assert!(parse(&["add", "--start", "--note", "x"]).is_err(), "notes can't be started");
    assert!(parse(&["add", "--tag", "x"]).is_err(), "a title is needed");
}

#[test]
fn ids_text_and_stdin() {
    assert_eq!(parse(&["done", "#4"]).unwrap().command, EntryCommand::SetStatus(4, Status::Done));
    assert_eq!(
        parse(&["comment", "4", "found", "it"]).unwrap().command,
        EntryCommand::Comment(4, Text::Given("found it".into()))
    );
    assert_eq!(parse(&["describe", "4", "-"]).unwrap().command, EntryCommand::Describe(4, Text::Stdin));
    assert_eq!(
        parse(&["link", "4", "5,", "#6", "api#2"]).unwrap().command,
        EntryCommand::Link(4, vec![5, 6], vec![RepoLink { repo: "api".into(), id: 2 }])
    );
    assert!(parse(&["show", "four"]).is_err());
    assert!(parse(&["show", "4", "5"]).is_err());
    assert!(parse(&["comment", "4"]).is_err());
    assert!(parse(&["search", " "]).is_err(), "everything would match");
    assert_eq!(parse(&["search", "--", "--", "x"]).unwrap().command, EntryCommand::Search("-- x".into()), "the first");
    // A word that isn't a link is said, not left out.
    let error = |args: &[&str]| parse(args).unwrap_err().to_string();
    assert!(error(&["link", "2", "abc", "3"]).contains("`abc` isn't an entry id"));
    assert!(error(&["add", "--link", "foo", "title"]).contains("`foo` isn't an entry id"));
}

#[test]
fn a_missing_text_or_title_is_named_once_for_its_command() {
    let error = |args: &[&str]| parse(args).unwrap_err().to_string();
    assert_eq!(error(&["reject", "6"]), "reject needs why: TEXT, or - to read it from stdin");
    assert_eq!(error(&["ask", "6"]), "ask needs the question: TEXT, or - to read it from stdin");
    assert_eq!(error(&["propose", "1"]), "propose needs the decision's title");
    assert_eq!(error(&["add", "--tag", "x"]), "add needs a title");
    assert!(EntryArgs::parse("nope", std::iter::empty()).unwrap().is_none());
}

#[test]
fn project_shows_or_replaces_a_part() {
    assert_eq!(parse(&["project"]).unwrap().command, EntryCommand::Project(None));
    assert_eq!(
        parse(&["project", "describe", "An", "online", "shop."]).unwrap().command,
        EntryCommand::Project(Some(ProjectChange::Text(
            ProjectPart::Description,
            Text::Given("An online shop.".into())
        )))
    );
    assert_eq!(
        parse(&["project", "notes", "-"]).unwrap().command,
        EntryCommand::Project(Some(ProjectChange::Text(ProjectPart::Notes, Text::Stdin)))
    );
    assert!(parse(&["project", "describe"]).is_err(), "text is needed");
    let change = |args: &[&str]| parse(args).unwrap().command;
    assert_eq!(
        change(&["project", "rename", "Web", "shop"]),
        EntryCommand::Project(Some(ProjectChange::Rename("Web shop".into(), false)))
    );
    assert_eq!(change(&["project", "repo", "api"]), EntryCommand::Project(Some(ProjectChange::Repo("api".into()))));
    assert_eq!(change(&["project", "move", "blog"]), EntryCommand::Project(Some(ProjectChange::Move("blog".into()))));
    assert_eq!(change(&["project", "remove", "--purge"]), EntryCommand::Project(Some(ProjectChange::Remove(true))));
    assert!(parse(&["project", "rename"]).is_err(), "a name is needed");
    assert!(parse(&["project", "remove", "now"]).is_err());
    assert!(parse(&["project", "frobnicate"]).is_err());
}

#[test]
fn project_writes_the_shared_card_and_the_repository_notes() {
    let (central, code) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let dir = code.path().join("api/.brain");
    std::fs::create_dir_all(&dir).unwrap();
    crate::projects::register(central.path(), "shop", &dir).unwrap();
    crate::projects::write_repo_card(
        &dir,
        &crate::projects::RepoCard { project: "shop".into(), repo: None, notes: String::new() },
    )
    .unwrap();
    let open = || Brain::open_in(crate::store::Store::at(&dir).unwrap(), central.path()).unwrap();
    let mut brain = open();
    run_project(&mut brain, Some(ProjectChange::Text(ProjectPart::Description, Text::Given("An online shop.".into()))))
        .unwrap();
    run_project(&mut brain, Some(ProjectChange::Text(ProjectPart::Notes, Text::Given("The API.".into())))).unwrap();
    let project = open().project.unwrap();
    assert_eq!((project.description.as_str(), project.notes.as_str()), ("An online shop.", "The API."));
    let mut outside = Brain::open(crate::store::Store::at(central.path()).unwrap()).unwrap();
    assert!(run_project(&mut outside, None).is_err(), "the central brain isn't a project");
}

#[test]
fn project_lists_the_repositorys_role_only_where_there_are_several() {
    let mut project = ProjectInfo {
        name: "notes".into(),
        description: "A notes CLI.".into(),
        repo: "notes".into(),
        notes: String::new(),
        siblings: Vec::new(),
    };
    assert_eq!(project_text(&project), "## Project notes\n\nA notes CLI.\n", "one repository: no role to write");
    project.siblings.push(("web".into(), "/code/web/.brain".into()));
    assert!(project_text(&project).ends_with("## Repository notes\n\n(empty)\n"), "several: its role is missing");
}

#[test]
fn tag_takes_signed_tags_in_order() {
    let args = parse(&["tag", "#4", "+UI,#bug", "--json", "-wip", "+c++"]).unwrap();
    assert!(args.json);
    let (add, remove) = (|t: &str| TagChange::Add(t.into()), |t: &str| TagChange::Remove(t.into()));
    assert_eq!(args.command, EntryCommand::Tag(4, vec![add("ui"), add("bug"), remove("wip"), add("c++")]));
    let error = |args: &[&str]| parse(args).unwrap_err().to_string();
    assert_eq!(error(&["tag", "4"]), "tag needs what to change: +TAG adds a tag, -TAG removes one");
    assert_eq!(error(&["tag", "4", "ui"]), "`ui`: +TAG adds a tag, -TAG removes one");
    assert_eq!(error(&["tag", "4", "+#"]), "`+#` names no tag");
    assert_eq!(error(&["tag", "4", "--ui"]), "unknown tag option `--ui`: +TAG adds a tag, -TAG removes one");
    assert_eq!(error(&["tag", "4", "+ui", "--"]), "unexpected argument `--`", "tag takes no text");
    assert_eq!(error(&["tag", "+ui"]), "`+ui` isn't an entry id");
}

#[test]
fn rename_takes_the_words_as_the_title() {
    let rename = |args: &[&str]| parse(args).unwrap().command;
    assert_eq!(rename(&["rename", "#4", "Fix", "the", "login"]), EntryCommand::Rename(4, "Fix the login".into()));
    assert_eq!(
        rename(&["rename", "4", "--", "--verbose", "breaks"]),
        EntryCommand::Rename(4, "--verbose breaks".into())
    );
    assert_eq!(rename(&["rename", "4", "-", "x"]), EntryCommand::Rename(4, "- x".into()), "a dash is a word");
    assert!(parse(&["rename", "4", "x", "--json"]).unwrap().json);
    let error = |args: &[&str]| parse(args).unwrap_err().to_string();
    assert_eq!(error(&["rename", "4"]), "rename needs the new title");
    assert_eq!(error(&["rename", "4", " "]), "rename needs the new title");
    assert_eq!(error(&["rename", "4", "--start", "x"]), "unknown rename option `--start`");
    assert_eq!(error(&["rename", "x", "y"]), "`x` isn't an entry id");
}

#[test]
fn rename_saves_only_a_change_and_renames_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let open = || Brain::open(crate::store::Store::at(dir.path()).unwrap()).unwrap();
    let mut brain = open();
    let new =
        NewEntry { kind: Kind::Decision, title: "Use SQL".into(), tags: vec!["db".into()], ..NewEntry::default() };
    let id = brain.add(new).unwrap();
    assert!(retitle(&mut brain, id, "Use markdown\nfiles ").unwrap(), "a decision's title too");
    let saved = open();
    let entry = saved.entry(id).unwrap();
    assert_eq!((entry.title.as_str(), entry.tags.as_slice()), ("Use markdown files", &["db".to_string()][..]));
    let names: Vec<_> = std::fs::read_dir(dir.path()).unwrap().flatten().map(|e| e.file_name()).collect();
    assert!(names.iter().any(|n| n.to_string_lossy().ends_with("-use-markdown-files.md")), "{names:?}");
    assert!(!names.iter().any(|n| n.to_string_lossy().contains("use-sql")), "{names:?}");
    assert!(!retitle(&mut brain, id, " Use markdown files").unwrap(), "the same once on one line");
    assert!(retitle(&mut brain, 9, "x").unwrap_err().to_string().contains("no entry #9"));
}

#[test]
fn tag_saves_only_a_change_and_colors_a_new_tag() {
    let dir = tempfile::tempdir().unwrap();
    let open = || Brain::open(crate::store::Store::at(dir.path()).unwrap()).unwrap();
    let mut brain = open();
    let new = NewEntry {
        kind: Kind::Note,
        title: "Login flow".into(),
        tags: vec!["a".into(), "b".into()],
        ..NewEntry::default()
    };
    let id = brain.add(new).unwrap();
    let (add, remove) = (|t: &str| TagChange::Add(t.into()), |t: &str| TagChange::Remove(t.into()));
    assert!(retag(&mut brain, id, &[add("c"), remove("a"), add("b")]).unwrap(), "a note's tags too");
    let saved = open();
    assert_eq!(saved.entry(id).unwrap().tags, ["b", "c"], "added at the end, once");
    assert!(saved.tag_colors.get("c").is_some(), "as the app's form saves them");
    assert!(!retag(&mut brain, id, &[add("b"), remove("a")]).unwrap(), "nothing to change");
    assert!(!retag(&mut brain, id, &[add("a"), remove("a")]).unwrap(), "in order: added, then removed");
    assert!(retag(&mut brain, 9, &[add("a")]).unwrap_err().to_string().contains("no entry #9"));
}

#[test]
fn questions_proposals_and_verdicts() {
    assert_eq!(parse(&["ask", "4", "-"]).unwrap().command, EntryCommand::Ask(4, Text::Stdin));
    let EntryCommand::Propose(4, decision) = parse(&["propose", "4", "--body", "-", "Use", "Redis"]).unwrap().command
    else {
        panic!("a proposal for #4")
    };
    assert_eq!((decision.title.as_str(), decision.body), ("Use Redis", Some(Text::Stdin)));
    assert!(parse(&["propose", "4", "--note", "x"]).is_err(), "it's a decision");
    assert_eq!(parse(&["accept", "9"]).unwrap().command, EntryCommand::Resolve(9, DecisionState::Accepted, None));
    assert_eq!(
        parse(&["reject", "9", "too", "costly"]).unwrap().command,
        EntryCommand::Resolve(9, DecisionState::Rejected, Some(Text::Given("too costly".into())))
    );
    assert!(parse(&["reject", "9"]).is_err(), "rejecting needs why");
}

#[test]
fn wait_ends_on_a_persons_answer_or_verdict() {
    let eight = Some(Duration::from_secs(8 * 60));
    assert_eq!(parse(&["wait", "#6"]).unwrap().command, EntryCommand::Wait(6, eight));
    let two = Some(Duration::from_secs(120));
    assert_eq!(parse(&["wait", "6", "--timeout", "2"]).unwrap().command, EntryCommand::Wait(6, two));
    assert_eq!(parse(&["wait", "6", "--timeout", "0"]).unwrap().command, EntryCommand::Wait(6, None), "no limit");
    assert!(parse(&["wait", "6", "now"]).is_err());
    assert!(parse(&["wait", "6", "--timeout"]).is_err());
    let dir = tempfile::tempdir().unwrap();
    let open = Brain::open(crate::store::Store::at(dir.path()).unwrap()).unwrap();
    let mut brain = Brain { author: crate::model::Author::Agent, ..open };
    let new = |title: &str| NewEntry { title: title.into(), ..NewEntry::default() };
    let task = brain.add(new("Rate-limit login")).unwrap();
    let settled_on = |brain: &Brain, id| settled(brain.entry(id).unwrap(), None);
    let head = format!("#{task} Rate-limit login");
    assert_eq!(settled_on(&brain, task).unwrap(), format!("{head}: not waiting for an answer (todo)"));
    wait(&brain, task, None).expect("nothing to wait for: it returns at once");
    let note = brain.add(NewEntry { kind: Kind::Note, ..new("Webhooks repeat") }).unwrap();
    assert_eq!(
        settled_on(&brain, note).unwrap(),
        format!("#{note} Webhooks repeat: a note, not waiting for an answer")
    );
    // At once, not when the time is up.
    let missing = wait(&brain, 99, Some(Duration::from_secs(5))).unwrap_err();
    assert_eq!(missing.downcast::<ApiError>().unwrap(), ApiError::NotFound(99));

    brain.ask(task, "Per IP or per account?").unwrap();
    assert_eq!(settled_on(&brain, task), None);
    // On a thread, so a limit that never passes fails here instead of hanging the run.
    let (sent, got) = std::sync::mpsc::channel();
    let store = dir.path().to_path_buf();
    std::thread::spawn(move || {
        let brain = Brain::open(crate::store::Store::at(&store).unwrap()).unwrap();
        sent.send(wait(&brain, task, Some(Duration::ZERO)).unwrap_err().to_string()).unwrap();
    });
    let timed_out = got.recv_timeout(Duration::from_secs(10)).expect("stopped at the time limit");
    assert!(timed_out.starts_with(&format!("no answer on #{task} yet")), "{timed_out}");
    brain.add_comment(task, "Checking the logs meanwhile.").unwrap();
    assert_eq!(settled_on(&brain, task), None, "an agent's comment isn't an answer");
    brain.author = crate::model::Author::Human;
    brain.add_comment(task, "Per account").unwrap();
    assert_eq!(settled_on(&brain, task).unwrap(), format!("{head}: answered\n\nPer account"));
    // Answered and moved on before `wait` looked again: the answer to the question it found is still said.
    let asked_at = brain.entry(task).unwrap().comments.iter().rev().find(|c| c.question).unwrap().created;
    brain.change_status(task, Status::Todo).unwrap();
    assert_eq!(
        settled(brain.entry(task).unwrap(), Some(asked_at)).unwrap(),
        format!("{head}: answered, now todo\n\nPer account")
    );
    brain.change_status(task, Status::Parked).unwrap();

    brain.author = crate::model::Author::Agent;
    let decision = brain.propose(task, new("Use Redis")).unwrap();
    assert_eq!(settled_on(&brain, decision), None, "proposed");
    assert_eq!(settled_on(&brain, task), None, "the task asks about it");
    brain.author = crate::model::Author::Human;
    brain.resolve(decision, DecisionState::Rejected, "too heavy").unwrap();
    assert_eq!(
        settled_on(&brain, decision).unwrap(),
        format!("#{decision} Use Redis: rejected\n\nRejected: too heavy")
    );
    assert!(
        settled_on(&brain, task).unwrap().ends_with(&format!("Rejected decision #{decision}, Use Redis: too heavy"))
    );
}

#[test]
fn a_wait_longer_than_any_is_refused_rather_than_cut_short() {
    let error = minutes("999999999999999999").unwrap_err().to_string();
    assert!(error.contains("longer than any wait"), "{error}");
    assert_eq!(minutes("2").unwrap(), Some(Duration::from_secs(120)));
}

#[test]
fn waiting_on_an_entry_deleted_meanwhile_says_its_gone_at_once() {
    let dir = tempfile::tempdir().unwrap();
    let mut brain = Brain::open(crate::store::Store::at(dir.path()).unwrap()).unwrap();
    let task = brain.add(NewEntry { title: "Rate-limit login".into(), ..NewEntry::default() }).unwrap();
    brain.ask(task, "Per IP or per account?").unwrap();
    let file = brain.entry(task).unwrap().path.clone().unwrap();
    let deleter = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(1500));
        std::fs::remove_file(file).unwrap();
    });
    let started = std::time::Instant::now();
    let error = wait(&brain, task, Some(Duration::from_secs(60))).unwrap_err();
    deleter.join().unwrap();
    assert_eq!(error.downcast::<ApiError>().unwrap(), ApiError::NotFound(task));
    assert!(started.elapsed() < Duration::from_secs(10), "not after the time limit");
}

#[test]
fn waiting_on_an_entry_missing_from_one_look_carries_on() {
    let dir = tempfile::tempdir().unwrap();
    let mut brain = Brain::open(crate::store::Store::at(dir.path()).unwrap()).unwrap();
    let task = brain.add(NewEntry { title: "Rate-limit login".into(), ..NewEntry::default() }).unwrap();
    brain.ask(task, "Per IP or per account?").unwrap();
    let asked = || Ok(brain.store.load().unwrap().0);
    // Gone from one look, as a file renamed for a new title is gone between listing and reading it.
    let mut watch = Watch::default();
    assert!(watch.look(task, asked()).is_none());
    assert!(watch.look(task, Ok(Vec::new())).is_none(), "looked at again");
    assert!(watch.look(task, asked()).is_none());
    assert!(watch.look(task, Ok(Vec::new())).is_none(), "a miss after it was back is a first miss again");
    let mut gone = Watch::default();
    assert!(gone.look(task, asked()).is_none());
    assert!(gone.look(task, Ok(Vec::new())).is_none());
    let error = gone.look(task, Ok(Vec::new())).unwrap().unwrap_err();
    assert_eq!(error.downcast::<ApiError>().unwrap(), ApiError::NotFound(task), "not there twice in a row");

    brain.author = crate::model::Author::Human;
    brain.add_comment(task, "Per account").unwrap();
    let answered = watch.look(task, Ok(brain.store.load().unwrap().0)).unwrap().unwrap();
    assert_eq!(answered, format!("#{task} Rate-limit login: answered\n\nPer account"));
}

#[test]
fn a_task_moved_on_unanswered_says_so_rather_than_quote_an_older_comment_or_its_question() {
    let dir = tempfile::tempdir().unwrap();
    let mut brain = Brain::open(crate::store::Store::at(dir.path()).unwrap()).unwrap();
    let task = brain.add(NewEntry { title: "Rate-limit login".into(), ..NewEntry::default() }).unwrap();
    brain.add_comment(task, "Login is slow on mobile.").unwrap();
    // Times are whole seconds: the person's comment is from before the question, not the same second.
    std::thread::sleep(Duration::from_millis(1100));
    brain.author = crate::model::Author::Agent;
    brain.ask(task, "Per IP or per account?").unwrap();
    let asked_at = brain.entry(task).unwrap().comments.iter().rev().find(|c| c.question).unwrap().created;
    brain.change_status(task, Status::Todo).unwrap();
    assert_eq!(
        settled(brain.entry(task).unwrap(), Some(asked_at)).unwrap(),
        format!("#{task} Rate-limit login: not waiting for an answer (todo)")
    );
}

#[test]
fn a_wait_fails_at_once_on_a_folder_it_cant_read_but_looks_again_after_its_first_read() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("brain");
    let mut brain = Brain::open(crate::store::Store::at(&path).unwrap()).unwrap();
    let task = brain.add(NewEntry { title: "Rate-limit login".into(), ..NewEntry::default() }).unwrap();
    brain.author = crate::model::Author::Agent;
    brain.ask(task, "Per IP or per account?").unwrap();

    // The folder caught mid-change after the first look: looked at again, until the answer is there.
    let away = dir.path().join("away");
    let (from, to) = (path.clone(), away.clone());
    // On Windows a folder `wait` is reading that moment can't be renamed: tried again.
    let rename = |from: &std::path::Path, to: &std::path::Path| {
        let mut tries = 0;
        while let Err(e) = std::fs::rename(from, to) {
            tries += 1;
            assert!(tries < 500, "{e}");
            std::thread::sleep(Duration::from_millis(20));
        }
    };
    let mover = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(1500));
        rename(&from, &to);
        std::thread::sleep(Duration::from_millis(2500));
        rename(&to, &from);
        let mut brain = Brain::open(crate::store::Store::at(&from).unwrap()).unwrap();
        brain.add_comment(task, "Per account").unwrap();
    });
    wait(&brain, task, Some(Duration::from_secs(60))).expect("the answer, once the folder is back");
    mover.join().unwrap();

    std::fs::rename(&path, &away).unwrap();
    let started = std::time::Instant::now();
    let error = wait(&brain, task, Some(Duration::from_secs(60))).unwrap_err().to_string();
    assert!(!error.starts_with("no answer"), "{error}");
    assert!(started.elapsed() < Duration::from_secs(10), "not after the time limit");
}
