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
fn show_json_names_the_links_from_other_repositories() {
    let (central, code) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (api, web) = (code.path().join("api"), code.path().join("web"));
    for repo in [&api, &web] {
        std::fs::create_dir(repo).unwrap();
        ok(&kb(repo, central.path(), &["init", "shop"], ""));
    }
    ok(&kb(&api, central.path(), &["add", "Expose", "the", "total"], ""));
    ok(&kb(&web, central.path(), &["add", "--link", "api#1", "Show", "the", "total"], ""));
    let json = ok(&kb(&api, central.path(), &["show", "1", "--json"], ""));
    assert!(json.contains(r#""repo_linked_from":["web#1"]"#) && json.lines().count() == 1, "{json}");
}

#[test]
fn list_gives_a_sync_conflict_copy_an_id_of_its_own() {
    let (central, dir) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    ok(&kb(dir.path(), central.path(), &["add", "First"], ""));
    let first = std::fs::read_dir(central.path())
        .unwrap()
        .flatten()
        .find(|e| e.file_name().to_string_lossy().starts_with("0001-"));
    std::fs::copy(first.unwrap().path(), central.path().join("0001-first (conflicted copy).md")).unwrap();
    assert_eq!(ok(&kb(dir.path(), central.path(), &["list"], "")), "## todo\n\n- #2 First\n- #1 First\n");
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
    assert!(
        !unknown.status.success() && stderr.contains("unknown command `frobnicate`") && stderr.contains("kb --help")
    );
    let misused = String::from_utf8(run(&["wait", "6", "--timeout", "x"]).stderr).unwrap();
    assert!(misused.contains("usage:\n  kb wait ID [--timeout MINUTES]\n"), "{misused}");
    assert!(!misused.contains("kb add") && misused.lines().count() < 12, "only the command's usage: {misused}");
    ok(&run(&["add", "--decision", "Use", "markdown"]));
    let again = run(&["accept", "1"]);
    assert_eq!(String::from_utf8_lossy(&again.stderr), "#1 was already accepted: nothing changed\n");
    assert!(ok(&again).starts_with("#1 Use markdown"), "stdout keeps the entry's line");
    let help = ok(&run(&["list", "--help"]));
    assert!(help.starts_with("usage:\n  kb list ") && !help.contains("kb add"), "{help}");
    assert_eq!(ok(&run(&["help", "list"])), help);
    assert!(ok(&run(&["help"])).contains("kb add"), "the whole usage");
    let show = String::from_utf8(run(&["show", "abc"]).stderr).unwrap();
    assert!(show.starts_with("Error: `abc` isn't an entry id\n") && !show.contains("kb search"), "{show}");
    assert!(ok(&run(&["--version"])).starts_with("kb "));
    assert_eq!(ok(&run(&["list"])), "nothing to list\n", "said, not left blank");
    ok(&run(&["add", "Ship"]));
    let started = run(&["start", "2"]);
    assert!(ok(&started).starts_with("#2 Ship") && started.stderr.is_empty(), "a change says nothing more");
    assert_eq!(String::from_utf8_lossy(&run(&["start", "2"]).stderr), "#2 was already doing: nothing changed\n");
    let json = run(&["wait", "2", "--json"]);
    assert!(String::from_utf8_lossy(&json.stderr).contains("wait prints the answer as text only"), "{json:?}");
    assert!(
        ok(&run(&["wait", "2", "--timeout", "1"])).contains("not waiting for an answer"),
        "nothing asked: it returns at once"
    );
    let proposed = ok(&run(&["propose", "2", "Use", "Redis"]));
    assert!(proposed.starts_with("#3 Use Redis") && proposed.contains(" · #2 Ship"), "both lines: {proposed}");
    let json = ok(&run(&["propose", "2", "Use", "Kafka", "--json"]));
    assert!(json.starts_with('{') && json.contains("Use Kafka") && !json.contains(" · "), "the decision: {json}");
    for (args, said) in
        [(["context", "--json"], "context is markdown only"), (["project", "--json"], "project is markdown only")]
    {
        let json = run(&args);
        assert!(String::from_utf8_lossy(&json.stderr).contains(said), "{json:?}");
    }
}

#[test]
fn an_empty_stdin_for_a_dash_is_refused_and_overwrites_nothing() {
    let (central, repo) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let run = |args: &[&str], stdin: &str| kb(repo.path(), central.path(), args, stdin);
    ok(&run(&["init", "demo"], ""));
    ok(&run(&["add", "--body", "keep me", "Fix login"], ""));
    ok(&run(&["project", "describe", "An online shop."], ""));
    // As an agent's shell tool runs it with the heredoc left out.
    for args in [&["describe", "1", "-"][..], &["project", "describe", "-"], &["project", "notes", "-"]] {
        let out = run(args, " \n");
        assert!(!out.status.success(), "{args:?} succeeded");
        assert!(String::from_utf8_lossy(&out.stderr).contains("nothing came on stdin"), "{args:?}");
    }
    assert!(ok(&run(&["show", "1"], "")).contains("keep me"));
    assert!(ok(&run(&["project"], "")).contains("An online shop."));
}

#[test]
fn words_that_look_like_options_are_text_after_a_double_dash_or_inside_it() {
    let (central, repo) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let run = |args: &[&str], stdin: &str| kb(repo.path(), central.path(), args, stdin);
    ok(&run(&["init", "demo"], ""));
    assert_eq!(
        ok(&run(&["add", "--", "--verbose", "flag", "breaks", "it"], "")),
        "#1 --verbose flag breaks it (todo)\n"
    );
    ok(&run(&["comment", "1", "--", "use", "--json", "and", "--project", "x"], ""));
    // `-h` inside the words is text too; only right after the command is it asking for help.
    ok(&run(&["comment", "1", "pass", "-h", "to", "the", "tool"], ""));
    let shown = ok(&run(&["show", "1"], ""));
    assert!(shown.contains("use --json and --project x"), "{shown}");
    assert!(shown.contains("pass -h to the tool"), "{shown}");
    assert!(ok(&run(&["search", "--", "--verbose"], "")).contains("#1"));
    assert!(ok(&run(&["add", "--help"], "")).starts_with("usage:"));
    // A `project` change asked for help shows it, rather than take `--help` for a name or a text.
    ok(&run(&["project", "describe", "An online shop."], ""));
    for sub in ["describe", "notes", "rename", "repo", "move"] {
        assert!(ok(&run(&["project", sub, "--help"], "")).starts_with("usage:"), "{sub}");
    }
    assert!(ok(&run(&["project"], "")).contains("An online shop."), "unchanged");
}

#[test]
fn files_that_dont_read_as_entries_are_named_on_stderr() {
    let (central, repo) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let run = |args: &[&str]| kb(repo.path(), central.path(), args, "");
    ok(&run(&["init", "demo"]));
    std::fs::write(repo.path().join(".brain/0007-broken.md"), "# Broken\n\nno id line\n").unwrap();
    // A note of the user's own, as a brain kept in a vault holds: not kanbrain's to mention.
    std::fs::write(repo.path().join(".brain/2024-01-15.md"), "# Daily\n").unwrap();
    for args in [&["list"][..], &["show", "7"]] {
        let stderr = String::from_utf8(run(args).stderr).unwrap();
        assert!(stderr.contains("kb: skipped 0007-broken.md"), "{args:?}: {stderr}");
        assert!(!stderr.contains("2024-01-15"), "{args:?}: {stderr}");
    }
}

#[test]
fn renaming_a_project_takes_the_new_name_only_and_central_is_no_project() {
    let (central, repo) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let run = |args: &[&str]| kb(repo.path(), central.path(), args, "");
    ok(&run(&["init", "shop"]));
    let out = run(&["project", "rename", "shop", "store"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("`kb project rename store`"));
    ok(&run(&["project", "rename", "--", "shop", "v2"]));
    assert!(ok(&run(&["project"])).contains("shop v2"), "a name starting with the old one, asked for with --");
    ok(&run(&["project", "rename", "store"]));
    let out = run(&["project", "rename", "Central"]);
    assert!(String::from_utf8_lossy(&out.stderr).contains("is the name of the central brain"), "{out:?}");
}

#[test]
fn project_reaches_a_registered_brain_from_any_folder() {
    let (central, repo, elsewhere) =
        (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    ok(&kb(repo.path(), central.path(), &["init", "demo"], ""));
    ok(&kb(repo.path(), central.path(), &["add", "In", "the", "repo"], ""));
    let from_elsewhere = ok(&kb(elsewhere.path(), central.path(), &["list", "--project", "demo"], ""));
    assert!(from_elsewhere.contains("- #1 In the repo"), "{from_elsewhere}");
    assert_eq!(
        ok(&kb(elsewhere.path(), central.path(), &["list"], "")),
        "nothing to list\n",
        "outside it, the central brain"
    );
    let path = ok(&kb(elsewhere.path(), central.path(), &["--project", "CENTRAL", "path"], ""));
    assert_eq!(path.trim(), central.path().display().to_string(), "`central` in any case");
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

#[test]
fn init_says_so_when_kb_isnt_on_the_path() {
    let (central, repo) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let init = |path: &std::ffi::OsStr| {
        let mut init = Command::new(env!("CARGO_BIN_EXE_kb"));
        init.args(["init", "demo"]).current_dir(repo.path()).env("KANBRAIN_DIR", central.path()).env("PATH", path);
        ok(&init.output().unwrap())
    };
    let said = "`kb` isn't on your PATH";
    assert!(init("".as_ref()).contains(said), "agents and the hooks run it");
    let bin = Path::new(env!("CARGO_BIN_EXE_kb")).parent().unwrap();
    assert!(!init(bin.as_os_str()).contains(said));
}
