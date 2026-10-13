use super::*;

#[test]
fn every_command_offered_is_in_the_usage_and_the_reference() {
    let usage = crate::cli::usage(crate::cli::Program { name: "kb" });
    let reference = include_str!("../../../docs/cli.md");
    for command in COMMANDS {
        let listed = |text: &str| {
            [format!("kb {command}"), format!("|{command}"), format!("| {command}")]
                .iter()
                .any(|s| text.contains(s.as_str()))
        };
        assert!(listed(&usage), "{command} is in the usage");
        assert!(listed(reference), "{command} is in docs/cli.md");
    }
}

#[test]
fn ids_come_open_tasks_first_and_projects_by_the_names_project_takes() {
    let (central, code) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let mut brain = Brain::open(Store::at(central.path()).unwrap()).unwrap();
    let new = |title: &str| crate::brain::NewEntry { title: title.into(), ..Default::default() };
    brain.add(new("old")).unwrap();
    let done = brain.add(new("finished")).unwrap();
    brain.set_status(done, Status::Done).unwrap();
    brain.add(new("newest")).unwrap();
    brain.add(crate::brain::NewEntry { kind: crate::model::Kind::Note, ..new("a note") }).unwrap();
    let ids = candidates(Candidates::Ids, Store::at(central.path()).unwrap(), central.path()).unwrap();
    assert_eq!(ids, ["3\tnewest", "1\told", "4\ta note", "2\tfinished"]);

    for repo in ["api", "web"] {
        projects::register(central.path(), "shop", &code.path().join(repo).join(".brain")).unwrap();
    }
    projects::register(central.path(), "blog", &code.path().join("blog").join(".brain")).unwrap();
    let names = candidates(Candidates::Projects, Store::at(central.path()).unwrap(), central.path()).unwrap();
    assert_eq!(names, ["central", "shop/api", "shop/web", "blog"], "not `shop`, which --project refuses");
}

#[test]
fn scripts_name_the_program() {
    for shell in [Shell::Bash, Shell::Zsh, Shell::Fish] {
        let text = script(shell, "kanbrain");
        assert!(text.contains("kanbrain complete ids") && !text.contains("kb "), "{text}");
    }
    assert!(script(Shell::Bash, "kanbrain").contains("complete -F _kanbrain kanbrain"));
}

#[test]
fn every_word_project_takes_is_offered() {
    for word in PROJECT.split(' ') {
        // `remove` takes no name, the others one.
        let args = [word, "x"].into_iter().take(if word == "remove" { 1 } else { 2 }).map(String::from);
        let parsed = crate::cli::EntryArgs::parse("project", args).unwrap().unwrap();
        assert!(matches!(parsed.command, crate::cli::EntryCommand::Project(Some(_))), "{word}");
    }
    let script = script(Shell::Bash, "kb");
    assert!(script.contains(PROJECT) && script.contains("deinit) reply=\"--purge\""), "{script}");
}
