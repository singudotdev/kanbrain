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
        EntryCommand::Project(Some(ProjectChange::Rename("Web shop".into())))
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
    let settled_on = |brain: &Brain, id| settled(brain.entry(id).unwrap());
    let head = format!("#{task} Rate-limit login");
    assert_eq!(settled_on(&brain, task).unwrap(), format!("{head}: not waiting for an answer (todo)"));
    wait(&brain, task, None).expect("nothing to wait for: it returns at once");
    let note = brain.add(NewEntry { kind: Kind::Note, ..new("Webhooks repeat") }).unwrap();
    assert_eq!(
        settled_on(&brain, note).unwrap(),
        format!("#{note} Webhooks repeat: a note, not waiting for an answer")
    );
    assert!(wait(&brain, 99, None).is_err(), "no such entry");

    brain.ask(task, "Per IP or per account?").unwrap();
    assert_eq!(settled_on(&brain, task), None);
    let timed_out = wait(&brain, task, Some(Duration::ZERO)).unwrap_err().to_string();
    assert!(timed_out.starts_with(&format!("no answer on #{task} yet")), "{timed_out}");
    brain.add_comment(task, "Checking the logs meanwhile.").unwrap();
    assert_eq!(settled_on(&brain, task), None, "an agent's comment isn't an answer");
    brain.author = crate::model::Author::Human;
    brain.add_comment(task, "Per account").unwrap();
    assert_eq!(settled_on(&brain, task).unwrap(), format!("{head}: answered\n\nPer account"));

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
