use super::*;
use crate::projects;

/// What the hook says for `input`, run in a folder outside any project.
fn hook(event: HookEvent, input: &str, central: &Path) -> Option<String> {
    let outside = tempfile::tempdir().unwrap();
    respond(event, input, central, outside.path())
}

/// A central brain and a repository folder registered as project `shop`, with its card as `init` makes
/// it; the folders live as long as the returned ones.
fn project() -> (tempfile::TempDir, tempfile::TempDir, PathBuf) {
    let (central, code) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let dir = code.path().join("api/.brain");
    std::fs::create_dir_all(&dir).unwrap();
    projects::register(central.path(), "shop", &dir).unwrap();
    projects::write_repo_card(&dir, &projects::RepoCard { project: "shop".into(), repo: None, notes: String::new() })
        .unwrap();
    projects::ensure_description(central.path(), "shop").unwrap();
    let root = code.path().join("api");
    (central, code, root)
}

/// The hook's input as Claude Code sends it, from `cwd`.
fn input(cwd: &Path, stop_hook_active: bool) -> String {
    json!({ "hook_event_name": "Stop", "cwd": cwd, "stop_hook_active": stop_hook_active }).to_string()
}

#[test]
fn an_undescribed_project_is_asked_for_with_every_prompt_and_holds_the_turn_once() {
    let (central, _code, root) = project();
    let prompt = hook(HookEvent::Prompt, &input(&root, false), central.path()).expect("it asks");
    assert!(
        prompt.contains("Before anything else in this turn") && prompt.contains("`kb project describe -`"),
        "{prompt}"
    );
    assert!(!prompt.contains("role"), "one repository has no role to describe: {prompt}");

    let stop: Value =
        serde_json::from_str(&hook(HookEvent::Stop, &input(&root, false), central.path()).unwrap()).unwrap();
    let reminder = stop["hookSpecificOutput"]["additionalContext"].as_str().expect("the turn goes on with it");
    assert_eq!(stop["hookSpecificOutput"]["hookEventName"], "Stop");
    assert!(reminder.contains("Before ending this turn") && reminder.contains("`kb project describe -`"), "{reminder}");
    assert_eq!(hook(HookEvent::Stop, &input(&root, true), central.path()), None, "but only once: no loop");
}

#[test]
fn a_described_project_costs_nothing_and_outside_one_too() {
    let (central, _code, root) = project();
    projects::write_description(central.path(), "shop", "An online shop.").unwrap();
    for event in [HookEvent::Prompt, HookEvent::Stop] {
        assert_eq!(hook(event, &input(&root, false), central.path()), None);
    }
    let elsewhere = tempfile::tempdir().unwrap();
    assert_eq!(hook(HookEvent::Stop, &input(elsewhere.path(), false), central.path()), None, "not a project");
    let bad = respond(HookEvent::Stop, "not json", central.path(), elsewhere.path());
    assert_eq!(bad, None, "bad input falls back to the folder it runs in, and never breaks a session");
}

#[test]
fn with_several_repositories_this_ones_role_counts_too() {
    let (central, code, root) = project();
    let web = code.path().join("web/.brain");
    std::fs::create_dir_all(&web).unwrap();
    projects::register(central.path(), "shop", &web).unwrap();
    projects::write_description(central.path(), "shop", "An online shop.").unwrap();
    let prompt = hook(HookEvent::Prompt, &input(&root, false), central.path()).expect("the role is missing");
    assert!(prompt.contains("`kb project notes -`") && !prompt.contains("describe -"), "{prompt}");
    projects::write_repo_card(
        &root.join(".brain"),
        &projects::RepoCard { project: "shop".into(), repo: None, notes: "The API.".into() },
    )
    .unwrap();
    assert_eq!(hook(HookEvent::Prompt, &input(&root, false), central.path()), None);
}

#[test]
fn a_task_an_agent_started_and_never_commented_on_is_reminded_once() {
    let (central, _code, root) = project();
    projects::write_description(central.path(), "shop", "An online shop.").unwrap();
    let mut brain = crate::brain::Brain::open(Store::at(root.join(".brain")).unwrap()).unwrap();
    let new = |title: &str| crate::brain::NewEntry { title: title.into(), start: true, ..Default::default() };
    let users = brain.add(new("The user's")).unwrap();
    assert_eq!(hook(HookEvent::Stop, &input(&root, false), central.path()), None, "not the agent's to report");

    brain.author = Author::Agent;
    let task = brain.add(new("Cache sessions")).unwrap();
    assert_ne!(users, task);
    let other = brain.add(new("Expire sessions")).unwrap();
    let stop = hook(HookEvent::Stop, &input(&root, false), central.path()).expect("a reminder");
    assert!(stop.contains(&format!("`kb comment {task}`")), "{stop}");
    assert!(stop.contains(&format!("`kb comment {other}`")), "each one a subagent runs: {stop}");
    // Another session's, as far as this one knows: never told to finish what it didn't start.
    assert!(stop.contains("If you started it") && stop.contains("if you didn't, leave it alone"), "{stop}");
    assert!(!stop.contains("since you started it"), "{stop}");
    brain.add_comment(other, "expiry set to a day").unwrap();
    assert_eq!(hook(HookEvent::Stop, &input(&root, true), central.path()), None, "once a turn");
    brain.add_comment(task, "sessions cached; next: expiry").unwrap();
    assert_eq!(hook(HookEvent::Stop, &input(&root, false), central.path()), None);
}

/// What `kb hook edit` says when an agent's Edit, run from `cwd`, is about to write `file`.
fn edit(cwd: &Path, file: &Path, central: &Path) -> Option<String> {
    let input = json!({
        "hook_event_name": "PreToolUse",
        "cwd": cwd,
        "tool_name": "Edit",
        "tool_input": { "file_path": file },
    });
    hook(HookEvent::Edit, &input.to_string(), central)
}

#[test]
fn an_agent_writing_a_brains_files_is_sent_to_kb() {
    let (central, _code, root) = project();
    let refused = edit(&root, &root.join(".brain/0001-fix-login.md"), central.path()).expect("refused");
    let out: Value = serde_json::from_str(&refused).unwrap();
    assert_eq!(out["hookSpecificOutput"]["hookEventName"], "PreToolUse");
    assert_eq!(out["hookSpecificOutput"]["permissionDecision"], "deny");
    let why = out["hookSpecificOutput"]["permissionDecisionReason"].as_str().unwrap();
    assert!(why.contains("`kb --help`") && why.contains("tell the user"), "{why}");
    assert!(
        edit(&root, ".brain/0002-new.md".as_ref(), central.path()).is_some(),
        "a new one, from the session's folder"
    );
    for owned in ["projects/shop.md", "0001-plan.md", "tags.md", "projects.md", "last-id", "format", "reports/x.md"] {
        assert!(edit(&root, &central.path().join(owned), central.path()).is_some(), "the central brain's {owned}");
    }

    assert_eq!(edit(&root, &root.join("src/main.rs"), central.path()), None, "the code is the agent's to edit");
    assert_eq!(edit(&root, &root.join("CLAUDE.md"), central.path()), None, "and the section's file");
    assert_eq!(edit(&root, &root.join("docs/.brainstorm.md"), central.path()), None, "only a folder named .brain");
    // `..` is resolved as Claude Code resolves it, before anything is looked up.
    assert_eq!(edit(&root, &root.join(".brain/../src/main.rs"), central.path()), None, "out of .brain again");
    let back_in = central.path().join("nosuch/../0009-new.md");
    assert!(edit(&root, &back_in, central.path()).is_some(), "into the central brain through a folder not there");
    assert_eq!(hook(HookEvent::Edit, "not json", central.path()), None, "nothing to refuse without a path");
}

#[test]
fn the_central_brain_kept_in_a_notes_vault_leaves_the_vaults_own_files_to_the_agent() {
    let (central, _code, root) = project();
    let vault = central.path();
    assert_eq!(edit(&root, &vault.join("daily/2026-10-15.md"), vault), None, "a note of the vault's");
    std::fs::write(vault.join("Inbox.md"), "# Inbox\n\n- buy milk\n").unwrap();
    assert_eq!(edit(&root, &vault.join("Inbox.md"), vault), None, "at its top too");
    assert_eq!(edit(&root, &vault.join("2026-10-15.md"), vault), None, "and a new one, dated");
    std::fs::write(vault.join("plan.md"), "# Plan\n\n- id: 3\n- type: task\n").unwrap();
    assert!(edit(&root, &vault.join("plan.md"), vault).is_some(), "but an entry renamed by hand is kanbrain's");
    let code = vault.join("code/app");
    std::fs::create_dir_all(code.join(".brain")).unwrap();
    assert_eq!(edit(&code, &code.join("src/main.rs"), vault), None, "a repository kept in it");
    assert!(edit(&code, &code.join(".brain/0001-plan.md"), vault).is_some(), "but not that repository's brain");
    assert_eq!(edit(&root, vault, vault), None, "nor the folder itself");
}

#[cfg(unix)]
#[test]
fn a_brain_reached_through_a_link_is_a_brain_still() {
    let (central, code, root) = project();
    let link = code.path().join("notes");
    std::os::unix::fs::symlink(root.join(".brain"), &link).unwrap();
    assert!(edit(&root, &link.join("0001-fix-login.md"), central.path()).is_some());
    let central_link = code.path().join("central");
    std::os::unix::fs::symlink(central.path(), &central_link).unwrap();
    assert!(edit(&root, &central_link.join("0001-plan.md"), central.path()).is_some());
    // And the central brain set through a link (`KANBRAIN_DIR`), written at its real path.
    let real = central.path().canonicalize().unwrap().join("0001-plan.md");
    assert!(edit(&root, &real, &central_link).is_some());
}
