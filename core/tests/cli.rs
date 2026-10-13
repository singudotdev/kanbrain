use super::*;
use crate::model::now;

fn parse(args: &[&str]) -> Result<Command> {
    Command::parse(args.iter().map(|s| (*s).to_string()))
}

#[test]
fn commands() {
    assert_eq!(parse(&[]).unwrap(), Command::Help);
    assert!(parse(&["--track"]).is_err());
    assert_eq!(parse(&["path"]).unwrap(), Command::Path);
    assert_eq!(parse(&["status"]).unwrap(), Command::Status);
    assert_eq!(parse(&["init"]).unwrap(), Command::Init(None, None));
    assert_eq!(parse(&["init", "api"]).unwrap(), Command::Init(Some("api".into()), None));
    assert_eq!(parse(&["init", "--all"]).unwrap(), Command::InitAll);
    assert_eq!(parse(&["deinit"]).unwrap(), Command::Deinit(false));
    assert_eq!(parse(&["deinit", "--purge"]).unwrap(), Command::Deinit(true));
    assert_eq!(
        parse(&["init", "--repo", "web", "shop"]).unwrap(),
        Command::Init(Some("shop".into()), Some("web".into()))
    );
    assert_eq!(parse(&["hook", "stop"]).unwrap(), Command::Hook(hook::HookEvent::Stop));
    assert!(parse(&["hook", "nope"]).is_err());
    let (project, rest) = take_project(["list", "--project", "api", "done"].map(String::from).to_vec()).unwrap();
    assert_eq!((project.as_deref(), rest), (Some("api"), vec!["list".to_string(), "done".to_string()]));
    assert!(take_project(vec!["list".into(), "--project".into()]).is_err());
    assert_eq!(parse(&["--version"]).unwrap(), Command::Version);
    assert!(parse(&["nope"]).is_err());
    assert!(parse(&["path", "extra"]).is_err());
}

#[test]
fn usage_names_the_program_only_on_command_lines() {
    let text = usage(Program { name: "kanbrain" });
    let named: Vec<_> = text.lines().filter_map(|l| l.strip_prefix("  kanbrain ")).collect();
    assert!(
        named.contains(&"show ID   print an entry compactly: state, time, description, comments, related, history")
    );
    assert!(!named.iter().any(|l| l.starts_with("create") || l.starts_with("(all")), "{text}");
    // Every description starts in the same column, the agent commands' included.
    let at: Vec<_> = ["print the running tasks", "create a task", "print an entry"]
        .iter()
        .filter_map(|d| text.lines().find_map(|l| l.find(d)))
        .collect();
    assert!(at.len() == 3 && at.iter().all(|&c| c == at[0]), "descriptions line up: {at:?}\n{text}");
}

#[test]
fn list_options() {
    let list = |args: &[&str]| match parse(args).unwrap() {
        Command::List(args) => args,
        other => panic!("expected a list command, got {other:?}"),
    };
    assert_eq!(list(&["list"]).statuses, [Status::Doing, Status::Todo, Status::Parked]);
    assert_eq!(
        list(&["list", "done", "doing", "--tag", "#Prod", "done"]),
        ListArgs { statuses: vec![Status::Doing, Status::Done], kinds: vec![], tag: Some("prod".into()), json: false }
    );
    let docs = list(&["list", "decisions", "notes", "note", "--json"]);
    assert_eq!((docs.statuses, docs.kinds, docs.json), (vec![], vec![Kind::Note, Kind::Decision], true));
    assert_eq!(list(&["list", "doing", "notes"]).statuses, [Status::Doing], "statuses and kinds combine");
    assert_eq!(list(&["list", "all"]).statuses, Status::ALL);
    assert!(parse(&["list", "paused"]).is_err());
    assert!(parse(&["list", "--tag"]).is_err());
}

#[test]
fn list_output() {
    let mut fix = Task::new(1, "Fix login".into(), now());
    fix.tags = vec!["prod".into(), "support".into()];
    fix.set_status(Status::Doing, now(), Author::Human);
    let docs = Task::new(2, "Write docs".into(), now());
    let mut old = Task::new(3, "Old thing".into(), now());
    old.set_status(Status::Done, now(), Author::Human);
    let tasks = [fix, docs, old];

    let all = ListArgs { statuses: Status::ALL.to_vec(), kinds: vec![], tag: None, json: false };
    assert_eq!(
        list_markdown(&tasks, &all),
        "## doing\n\n- #1 Fix login #prod #support\n\n## todo\n\n- #2 Write docs\n\n## done\n\n- #3 Old thing\n"
    );
    let prod = ListArgs { tag: Some("prod".into()), ..all };
    assert_eq!(list_markdown(&tasks, &prod), "## doing\n\n- #1 Fix login #prod #support\n");
}

#[test]
fn list_notes_and_decisions() {
    let task = Task::new(1, "Fix login".into(), now());
    let mut note = Task::new(2, "Token TTL".into(), now());
    note.kind = Kind::Note;
    note.tags = vec!["auth".into()];
    let mut decision = Task::new(3, "Refresh server-side".into(), now());
    decision.kind = Kind::Decision;
    let tasks = [task, note, decision];
    let docs = ListArgs { statuses: vec![], kinds: vec![Kind::Note, Kind::Decision], tag: None, json: false };
    assert_eq!(
        list_markdown(&tasks, &docs),
        "## notes\n\n- #2 Token TTL #auth\n\n## decisions\n\n- #3 Refresh server-side\n"
    );
    let json: serde_json::Value = serde_json::from_str(&list_json(&tasks, &docs).unwrap()).unwrap();
    assert_eq!(json[0]["kind"], "note");
    assert_eq!(json[1]["id"], 3);
}

#[test]
fn report_options() {
    let Command::Report(args) = parse(&["report", "month", "--group", "tag", "--tag", "#Prod", "--save"]).unwrap()
    else {
        panic!("expected a report command");
    };
    assert_eq!(args, ReportArgs { unit: Unit::Month, group: Group::Tag, tag: Some("prod".into()), save: true });

    let Command::Report(defaults) = parse(&["report"]).unwrap() else { panic!("expected a report command") };
    assert_eq!((defaults.unit, defaults.group, defaults.save), (Unit::Week, Group::Task, false));

    assert!(parse(&["report", "--group", "both"]).is_err());
    assert!(parse(&["report", "--tag"]).is_err());
}

#[test]
fn the_usage_names_init_all_for_the_installers() {
    // install.sh and install.ps1 run `init --all` only when `--help` names it: an older version would take
    // `--all` for a project's name. Rewording this line would silently stop upgrades refreshing projects.
    assert!(usage(Program { name: "kb" }).contains("init --all"));
}

#[test]
fn the_usage_gives_the_attribution_notice_requires() {
    // NOTICE's first additional term has works based on kanbrain keep this in their `--help`.
    let usage = usage(Program { name: "kb" });
    assert!(usage.contains("kanbrain by singudotdev (https://github.com/singudotdev/kanbrain)"));
    assert!(usage.contains("AGPL-3.0-only with the additional terms in\nNOTICE"));
}

#[test]
fn the_status_line_names_running_tasks_and_skips_a_brain_it_cant_read() {
    use crate::brain::NewEntry;
    let (central, code) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let [api, blog] = ["api", "blog"].map(|name| code.path().join(name).join(store::PROJECT_DIR));
    for (dir, project) in [(&api, "shop"), (&blog, "blog")] {
        projects::register(central.path(), project, dir).unwrap();
    }
    assert_eq!(status_line(central.path(), now()).unwrap(), "idle");
    let mut brain = Brain::open(Store::at(&api).unwrap()).unwrap();
    let id = brain.add(NewEntry { title: "Fix login".into(), ..NewEntry::default() }).unwrap();
    brain.set_status(id, Status::Doing).unwrap();
    assert!(status_line(central.path(), now()).unwrap().starts_with("shop #1 Fix login 00:00:"));
    Store::at(&blog).unwrap();
    std::fs::write(blog.join(crate::migrate::FORMAT_FILE), "99\n").unwrap();
    assert!(status_line(central.path(), now()).unwrap().starts_with("shop #1 Fix login"), "blog is newer");
}
