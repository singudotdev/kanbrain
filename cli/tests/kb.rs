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
