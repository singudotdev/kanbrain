//! The `kb` binary as agents and scripts run it: arguments, stdin, stdout, stderr and the exit code, over
//! temporary folders. The commands' logic is tested inside the core crate; these check the process around it.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

/// Runs `kb ARGS` in `dir`, with the central brain in `central` and `stdin` as its input.
fn kb(dir: &Path, central: &Path, args: &[&str], stdin: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_kb"))
        .args(args)
        .current_dir(dir)
        .env("KANBRAIN_DIR", central)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(stdin.as_bytes()).unwrap();
    child.wait_with_output().unwrap()
}

/// What a command that must succeed printed.
fn ok(output: &Output) -> String {
    assert!(output.status.success(), "failed: {}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8(output.stdout.clone()).unwrap()
}

#[test]
fn an_agent_session_runs_through_the_binary() {
    let (central, repo) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let run = |args: &[&str], stdin: &str| kb(repo.path(), central.path(), args, stdin);
    ok(&run(&["init", "demo"], ""));
    assert!(std::fs::read_to_string(repo.path().join("CLAUDE.md")).unwrap().contains("## Project memory: kanbrain"));
    assert!(repo.path().join(".claude/settings.local.json").is_file(), "the hooks");

    assert_eq!(ok(&run(&["add", "--start", "--tag", "auth", "Fix", "login"], "")), "#1 Fix login (doing) #auth\n");
    ok(&run(&["comment", "1", "-"], "token refresh race\nfix next\n"));
    assert_eq!(ok(&run(&["done", "#1"], "")), "#1 Fix login (done) #auth\n");
    let shown = ok(&run(&["show", "1"], ""));
    assert!(shown.contains("token refresh race\nfix next"), "the comment read from stdin: {shown}");
    let context = ok(&run(&["context"], ""));
    assert!(context.contains("## Tasks: done lately\n\n- #1 Fix login"), "{context}");
    let json = ok(&run(&["show", "1", "--json"], ""));
    assert!(json.starts_with("{\"id\":1,") && json.ends_with("}\n") && json.lines().count() == 1, "{json}");
}

#[test]
fn errors_go_to_stderr_with_a_failing_exit_code() {
    let (central, dir) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let run = |args: &[&str]| kb(dir.path(), central.path(), args, "");
    let missing = run(&["show", "9"]);
    assert!(!missing.status.success() && missing.stdout.is_empty());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("no entry #9"));
    let unknown = run(&["frobnicate"]);
    let stderr = String::from_utf8_lossy(&unknown.stderr);
    assert!(!unknown.status.success() && stderr.contains("unknown command `frobnicate`") && stderr.contains("usage:"));
    assert!(ok(&run(&["--version"])).starts_with("kb "));
}

#[test]
fn project_reaches_a_registered_brain_from_any_folder() {
    let (central, repo, elsewhere) =
        (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    ok(&kb(repo.path(), central.path(), &["init", "demo"], ""));
    ok(&kb(repo.path(), central.path(), &["add", "In", "the", "repo"], ""));
    let from_elsewhere = ok(&kb(elsewhere.path(), central.path(), &["list", "--project", "demo"], ""));
    assert!(from_elsewhere.contains("- #1 In the repo"), "{from_elsewhere}");
    assert_eq!(ok(&kb(elsewhere.path(), central.path(), &["list"], "")), "", "outside it, the central brain");
    let ids = ok(&kb(elsewhere.path(), central.path(), &["complete", "ids", "--project", "demo"], ""));
    assert_eq!(ids, "1\tIn the repo\n", "completion reads it too");

    std::fs::remove_dir_all(repo.path().join(".brain")).unwrap();
    let gone = kb(elsewhere.path(), central.path(), &["list", "--project", "demo"], "");
    assert!(!gone.status.success() && String::from_utf8_lossy(&gone.stderr).contains("deinit"));
    assert!(!repo.path().join(".brain").exists(), "a deleted brain isn't made again");
}

#[test]
fn the_hooks_hold_an_agent_to_describing_the_project() {
    let (central, repo) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    ok(&kb(repo.path(), central.path(), &["init", "demo"], ""));
    let input = format!("{{\"cwd\": {:?}}}", repo.path().display().to_string());
    let asked = ok(&kb(repo.path(), central.path(), &["hook", "prompt"], &input));
    assert!(asked.contains("isn't described yet"), "{asked}");
    ok(&kb(repo.path(), central.path(), &["project", "describe", "-"], "A demo app."));
    assert_eq!(ok(&kb(repo.path(), central.path(), &["hook", "prompt"], &input)), "", "silent once described");
}

#[test]
fn a_command_waiting_for_its_stdin_holds_up_no_other() {
    let (central, repo) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    ok(&kb(repo.path(), central.path(), &["init", "demo"], ""));
    ok(&kb(repo.path(), central.path(), &["add", "Shared"], ""));
    let mut typing = Command::new(env!("CARGO_BIN_EXE_kb"))
        .args(["comment", "1", "-"])
        .current_dir(repo.path())
        .env("KANBRAIN_DIR", central.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(300));
    let (done, finished) = std::sync::mpsc::channel();
    let (repo_dir, central_dir) = (repo.path().to_path_buf(), central.path().to_path_buf());
    std::thread::spawn(move || done.send(kb(&repo_dir, &central_dir, &["add", "Meanwhile"], "")).unwrap());
    let added = finished.recv_timeout(std::time::Duration::from_secs(20)).expect("add waited for the comment's stdin");
    ok(&added);
    typing.stdin.take().unwrap().write_all(b"typed at last").unwrap();
    ok(&typing.wait_with_output().unwrap());
    assert!(ok(&kb(repo.path(), central.path(), &["show", "1"], "")).contains("typed at last"));
}

#[test]
fn repositories_registered_at_once_are_all_kept() {
    let (central, code) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let children: Vec<_> = (0..8)
        .map(|n| {
            let repo = code.path().join(format!("repo{n}"));
            std::fs::create_dir(&repo).unwrap();
            Command::new(env!("CARGO_BIN_EXE_kb"))
                .args(["init", &format!("project{n}")])
                .current_dir(repo)
                .env("KANBRAIN_DIR", central.path())
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap()
        })
        .collect();
    for child in children {
        ok(&child.wait_with_output().unwrap());
    }
    let registry = std::fs::read_to_string(central.path().join("projects.md")).unwrap();
    assert!((0..8).all(|n| registry.contains(&format!("- project{n}: "))), "{registry}");
}

#[test]
fn a_project_change_waits_for_the_central_brain_before_locking_its_own() {
    use kanbrain_core::store::Store;
    let (central, repo) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    ok(&kb(repo.path(), central.path(), &["init", "demo"], ""));
    // As the app does: the central brain first, then the repositories.
    let central_lock = Store::at(central.path()).unwrap().lock().unwrap();
    let rename = Command::new(env!("CARGO_BIN_EXE_kb"))
        .args(["project", "rename", "renamed"])
        .current_dir(repo.path())
        .env("KANBRAIN_DIR", central.path())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(500));
    let (sender, receiver) = std::sync::mpsc::channel();
    let brain = repo.path().join(".brain");
    // Another thread: this one's lock on the central brain would let it take any lock again.
    std::thread::spawn(move || sender.send(Store::at(&brain).unwrap().lock().is_ok()).unwrap());
    let free = receiver.recv_timeout(std::time::Duration::from_secs(5));
    drop(central_lock);
    assert_eq!(free, Ok(true), "kb held its repository's lock while waiting for the central brain's: a deadlock");
    assert!(ok(&rename.wait_with_output().unwrap()).contains("renamed project `demo` to `renamed`"));
}

#[test]
fn commands_run_in_parallel_lose_nothing() {
    let (central, repo) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    ok(&kb(repo.path(), central.path(), &["init", "demo"], ""));
    ok(&kb(repo.path(), central.path(), &["add", "Shared"], ""));
    // As an agent's subagents would: all at once, each adding a task and commenting on the same one.
    let spawn = |args: Vec<String>| {
        Command::new(env!("CARGO_BIN_EXE_kb"))
            .args(args)
            .current_dir(repo.path())
            .env("KANBRAIN_DIR", central.path())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap()
    };
    let children: Vec<_> = (0..8)
        .flat_map(|n| {
            [vec!["add".into(), format!("Part {n}")], vec!["comment".into(), "1".into(), format!("note {n}")]]
        })
        .map(spawn)
        .collect();
    for child in children {
        ok(&child.wait_with_output().unwrap());
    }
    let list = ok(&kb(repo.path(), central.path(), &["list", "all"], ""));
    let ids: std::collections::BTreeSet<u32> =
        list.lines().filter_map(|line| line.strip_prefix("- #")?.split(' ').next()?.parse().ok()).collect();
    assert_eq!(ids, (1..=9).collect(), "every add got its own id: {list}");
    let shown = ok(&kb(repo.path(), central.path(), &["show", "1"], ""));
    assert!((0..8).all(|n| shown.contains(&format!("note {n}"))), "every comment kept: {shown}");
}

#[test]
fn the_crates_readme_lists_the_same_commands_as_the_repositorys() {
    // crates.io shows cli/README.md: its table is the main README's, but for the app's row.
    let read = |path: &str| std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).unwrap();
    let commands = |readme: String| -> Vec<String> {
        readme.lines().filter(|l| l.starts_with("| `kb ")).map(str::to_string).collect()
    };
    assert_eq!(commands(read("README.md")), commands(read("../README.md")));
}
