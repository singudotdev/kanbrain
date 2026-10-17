//! The `kb` binary as agents and scripts run it: arguments, stdin, stdout, stderr and the exit code, over
//! temporary folders. The commands' logic is tested inside the core crate; these check the process around it.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

/// `kb`, to run in `dir` with the central brain in `central`, and the platform data folder `kb` falls back to without
/// one inside `central` too ([`fallback_inside`]).
fn kb_command(dir: &Path, central: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_kb"));
    command.current_dir(dir).env("KANBRAIN_DIR", central);
    fallback_inside(&mut command, central);
    command
}

/// Points the platform data folder that `command`'s `kb` falls back to without `KANBRAIN_DIR` (`XDG_DATA_HOME` on Linux,
/// under `HOME` on macOS) inside `folder`: a change that loses the variable, as a mutant of how it's read does, would
/// otherwise write into the real central brain of whoever runs the tests. Windows takes that folder from the system,
/// not the environment; there the tests run on throwaway CI machines.
fn fallback_inside<'a>(command: &'a mut Command, folder: &Path) -> &'a mut Command {
    let fallback = folder.join(".fallback");
    command.env("XDG_DATA_HOME", &fallback).env("HOME", &fallback)
}

/// Runs `kb ARGS` in `dir`, with the central brain in `central` and `stdin` as its input.
fn kb(dir: &Path, central: &Path, args: &[&str], stdin: &str) -> Output {
    let mut child = kb_command(dir, central)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(stdin.as_bytes()).unwrap();
    child.wait_with_output().unwrap()
}

/// A temporary folder to run `kb` in. Without a `.brain` of its own, `kb` takes the nearest one above it rather than
/// the central brain the test gives: one above the temporary folders (`TMPDIR` inside a repository using kanbrain)
/// would take the test's writes, so the test stops before any. Looked for from the folder's real path, as `kb` finds it
/// from the current directory: a `TMPDIR` that's a link into such a folder would hide it.
fn tempdir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let real = dir.path().canonicalize().unwrap();
    if let Some(above) = real.ancestors().skip(1).map(|d| d.join(".brain")).find(|brain| brain.is_dir()) {
        panic!("{} is above the temporary folders and would take kb's writes: set TMPDIR elsewhere", above.display());
    }
    dir
}

/// What a command that must succeed printed.
fn ok(output: &Output) -> String {
    assert!(output.status.success(), "failed: {}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8(output.stdout.clone()).unwrap()
}

#[test]
fn an_agent_session_runs_through_the_binary() {
    let (central, repo) = (tempdir(), tempdir());
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
fn context_brings_a_projects_hooks_up_to_date() {
    let (central, repo) = (tempdir(), tempdir());
    let run = |args: &[&str]| kb(repo.path(), central.path(), args, "");
    ok(&run(&["init", "demo"]));
    let settings = repo.path().join(".claude/settings.local.json");
    // As an older release wrote them: without the hook a newer one added.
    let full = std::fs::read_to_string(&settings).unwrap();
    let older = full.replace("kb hook tool", "date");
    std::fs::write(&settings, &older).unwrap();
    ok(&run(&["context"]));
    let now = std::fs::read_to_string(&settings).unwrap();
    assert!(now.contains("\"kb hook tool\"") && now.contains("\"date\""), "added, the rest kept: {now}");
    // Taken out with `deinit`: not written anew, the user's own left as it is.
    ok(&run(&["deinit"]));
    ok(&run(&["context"]));
    let left = std::fs::read_to_string(&settings).unwrap();
    assert!(!left.contains("kb ") && left.contains("\"date\""), "{left}");
}

#[test]
fn show_json_names_the_links_from_other_repositories() {
    let (central, code) = (tempdir(), tempdir());
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
    let (central, dir) = (tempdir(), tempdir());
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
    let (central, dir) = (tempdir(), tempdir());
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
    let (central, repo) = (tempdir(), tempdir());
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
fn tag_adds_and_removes_and_says_when_nothing_changed() {
    let (central, dir) = (tempdir(), tempdir());
    let run = |args: &[&str]| kb(dir.path(), central.path(), args, "");
    ok(&run(&["add", "--tag", "auth", "Fix", "login"]));
    let changed = run(&["tag", "1", "+bug", "-auth"]);
    assert_eq!(ok(&changed), "#1 Fix login (todo) #bug\n");
    assert!(changed.stderr.is_empty(), "a change says nothing more");
    let same = run(&["tag", "1", "+bug"]);
    assert_eq!(String::from_utf8_lossy(&same.stderr), "#1's tags were already so: nothing changed\n");
    assert_eq!(ok(&same), "#1 Fix login (todo) #bug\n", "stdout keeps the entry's line");
    let json = ok(&run(&["tag", "1", "--json", "-bug"]));
    assert!(json.starts_with("{\"id\":1,") && json.contains(r#""tags":[]"#), "{json}");
    let misused = String::from_utf8(run(&["tag", "1", "bug", "x"]).stderr).unwrap();
    assert!(misused.contains("usage:\n  kb tag ID +TAG|-TAG…\n"), "{misused}");
    // Two words could be a tag renamed: an entry's id is likelier a change without its sign.
    let unsigned = String::from_utf8(run(&["tag", "1", "bug"]).stderr).unwrap();
    assert_eq!(unsigned, "Error: #1 is an entry: to tag it, `kb tag 1 +bug`; to rename tag #1, `kb tag '#1' bug`\n");
}

#[test]
fn tag_renames_a_tag_in_every_entry_that_has_it() {
    let (central, dir) = (tempdir(), tempdir());
    let run = |args: &[&str]| kb(dir.path(), central.path(), args, "");
    ok(&run(&["add", "--tag", "ops", "Deploy"]));
    ok(&run(&["add", "--tag", "ui", "Login"]));
    ok(&run(&["add", "--tag", "ops,ui", "Alerts"]));
    assert_eq!(ok(&run(&["tag", "#ops", "infra"])), "#1 Deploy (todo) #infra\n#3 Alerts (todo) #infra #ui\n");
    let json = ok(&run(&["tag", "--json", "infra", "platform"]));
    assert!(json.starts_with("[{\"id\":1,") && json.contains(r#""tags":["platform","ui"]"#), "{json}");
    // `#ops` or `#infra`, as the two fall in one second or two: the first rename then takes in the second.
    assert!(ok(&run(&["show", "3"])).contains("added the tag #platform and removed the tag #"));
    let none = String::from_utf8(run(&["tag", "ops", "infra"]).stderr).unwrap();
    assert_eq!(none, "Error: no entry is tagged #ops\n");
}

#[test]
fn rename_changes_the_title_and_says_when_nothing_changed() {
    let (central, dir) = (tempdir(), tempdir());
    let run = |args: &[&str]| kb(dir.path(), central.path(), args, "");
    ok(&run(&["add", "--tag", "auth", "Fix", "login"]));
    let changed = run(&["rename", "1", "Fix", "the", "SSO", "login"]);
    assert_eq!(ok(&changed), "#1 Fix the SSO login (todo) #auth\n");
    assert!(changed.stderr.is_empty(), "a change says nothing more");
    let same = run(&["rename", "#1", "Fix", "the", "SSO", "login"]);
    assert_eq!(String::from_utf8_lossy(&same.stderr), "#1 already had that title: nothing changed\n");
    assert_eq!(ok(&same), "#1 Fix the SSO login (todo) #auth\n", "stdout keeps the entry's line");
    let json = ok(&run(&["rename", "1", "--json", "Fix", "SSO"]));
    assert!(json.starts_with("{\"id\":1,") && json.contains(r#""title":"Fix SSO""#), "{json}");
    let misused = String::from_utf8(run(&["rename", "1"]).stderr).unwrap();
    assert!(misused.contains("usage:\n  kb rename ID TITLE…\n"), "{misused}");
}

/// What bash's completion of `kb` offers for `line` (its last word being completed), in `dir`.
#[cfg(unix)]
fn bash_completes(dir: &Path, central: &Path, line: &[&str]) -> String {
    let bin = Path::new(env!("CARGO_BIN_EXE_kb")).parent().unwrap();
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default());
    // `eval`, not `source <(…)`, which macOS's bash 3.2 reads as empty.
    let script =
        r#"eval "$(kb completions bash)"; COMP_WORDS=("$@"); COMP_CWORD=$(($# - 1)); _kb; echo "${COMPREPLY[*]}""#;
    let output = Command::new("bash")
        .args(["-c", script, "bash"])
        .args(line)
        .current_dir(dir)
        .env("KANBRAIN_DIR", central)
        .env("PATH", path)
        // Linux's fallback only, as `fallback_inside` sets it: the shell keeps its own `HOME`.
        .env("XDG_DATA_HOME", central.join(".fallback"))
        .output()
        .unwrap();
    ok(&output).trim_end().to_string()
}

#[cfg(unix)]
#[test]
fn bash_completes_tag_with_the_id_then_signed_tags() {
    let (central, dir) = (tempdir(), tempdir());
    ok(&kb(dir.path(), central.path(), &["add", "--tag", "ui", "--tag", "bug", "Fix", "login"], ""));
    let completes = |line: &[&str]| bash_completes(dir.path(), central.path(), line);
    assert_eq!(completes(&["kb", "tag", ""]), "1", "the id first");
    assert_eq!(completes(&["kb", "tag", "--project", "central", ""]), "1", "options aren't the id");
    assert_eq!(completes(&["kb", "tag", "1", ""]), "+bug -bug +ui -ui");
    assert_eq!(completes(&["kb", "tag", "1", "+ui", "-b"]), "-bug");
    assert_eq!(completes(&["kb", "show", ""]), "1");
    assert_eq!(completes(&["kb", "rename", ""]), "1");
}

/// What PowerShell's completion of `kb` offers for `line`, completed where its `|` is (its end without one), in `dir`:
/// each match's text, then its tooltip after a tab, one per line. `shell` is `pwsh` (7) or Windows' `powershell`
/// (5.1). The console's output encoding is set to Latin-1 meanwhile, as Windows' OEM code pages read UTF-8 wrong.
/// `None` without the shell, which GitHub's runners have on every OS: CI never skips.
fn powershell_completes(shell: &str, dir: &Path, central: &Path, line: &str) -> Option<String> {
    let bin = Path::new(env!("CARGO_BIN_EXE_kb")).parent().unwrap();
    let path = std::env::var_os("PATH").unwrap_or_default();
    let paths = std::iter::once(bin.to_path_buf()).chain(std::env::split_paths(&path));
    let at = line.find('|').unwrap_or(line.len());
    let script = r#"$ErrorActionPreference = 'Stop'
kb completions powershell | Out-String | Invoke-Expression
[Console]::OutputEncoding = [System.Text.Encoding]::GetEncoding('iso-8859-1')
$found = (TabExpansion2 -inputScript $env:KB_LINE -cursorColumn ([int]$env:KB_AT)).CompletionMatches
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
$found | ForEach-Object { "$($_.CompletionText)`t$($_.ToolTip)" }"#;
    let output = match Command::new(shell)
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .current_dir(dir)
        .env("KANBRAIN_DIR", central)
        .env("PATH", std::env::join_paths(paths).unwrap())
        // Linux's fallback only, as `fallback_inside` sets it: PowerShell keeps its own `HOME`.
        .env("XDG_DATA_HOME", central.join(".fallback"))
        .env("KB_LINE", line.replacen('|', "", 1))
        .env("KB_AT", at.to_string())
        .output()
    {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound && std::env::var_os("CI").is_none() => return None,
        output => output.unwrap(),
    };
    let stdout = ok(&output);
    assert!(output.stderr.is_empty(), "nothing over the line: {}", String::from_utf8_lossy(&output.stderr));
    Some(stdout.replace("\r\n", "\n").trim_end().to_string())
}

#[test]
fn powershell_completes_commands_ids_and_tag_with_the_id_then_signed_tags() {
    let (central, dir, other) = (tempdir(), tempdir(), tempdir());
    ok(&kb(other.path(), central.path(), &["init", "my app"], ""));
    let tags = ["--tag", "ui", "--tag", "bug", "--tag", "café", "--tag", "o'k"];
    ok(&kb(dir.path(), central.path(), &[&["add"][..], &tags, &["Fix", "café", "login"]].concat(), ""));
    // Windows PowerShell, 5.1, is on every Windows; pwsh, 7, is where it's installed.
    let shells: &[&str] = if cfg!(windows) { &["pwsh", "powershell"] } else { &["pwsh"] };
    for &shell in shells {
        let completes = |line: &str| powershell_completes(shell, dir.path(), central.path(), line);
        let Some(shown) = completes("kb sh") else {
            eprintln!("no {shell} here: its completion isn't checked");
            continue;
        };
        assert_eq!(shown, "show\tshow", "{shell}");
        let texts = |line: &str| {
            let lines = completes(line).unwrap();
            lines.lines().map(|l| l.split('\t').next().unwrap().to_string()).collect::<Vec<_>>().join(" ")
        };
        // Windows PowerShell 5.1 doesn't ask a program's completer for a word starting with a single `-`, which it
        // takes for a parameter: no tag to take off. `--` options it does, as PowerShell 7 does both.
        let dashed = |expected: &'static str| if shell == "powershell" { "" } else { expected };
        let cases = [
            ("kb show ", "1", "the id"),
            ("kb tag ", "1", "the id first"),
            ("kb tag --project central ", "1", "options aren't the id"),
            ("kb tag 1 ", "+bug -bug +café -café '+o''k' '-o''k' +ui -ui", "quoted as needed"),
            ("kb tag 1 +ui -b", dashed("-bug"), "a tag to take off"),
            ("kb tag 1 +|u", "+ui", "in the middle of a word, by all of it"),
            ("kb --project ce|ntral show", "central", "in the middle of a word, after an option"),
            ("kb rename ", "1", "the id"),
            ("kb add --tag ", "bug café 'o''k' ui", "the tags in use"),
            ("kb add --t", "--tag", "options"),
            ("kb --project ", "central 'my app'", "the projects"),
            ("kb --project 'my", "'my app'", "a quote begun"),
            ("kb report --group ", "task tag", "the groups"),
            ("kb completions ", "bash zsh fish powershell", "the shells"),
            ("kb.exe sh", "show", "as Windows names it too"),
            ("kb --project nowhere show ", "", "kb's error isn't shown, nothing is offered"),
        ];
        // Every case, before failing: a shell CI alone runs shows all it does differently at once.
        let wrong: Vec<_> = cases
            .iter()
            .map(|&(line, expected, why)| (line, texts(line), expected, why))
            .filter(|(_, got, expected, _)| got != expected)
            .collect();
        assert!(wrong.is_empty(), "{shell}: (line, offered, expected, why) {wrong:#?}");
        assert_eq!(completes("kb show ").unwrap(), "1\tFix café login", "{shell}: the id, its title beside it");
    }
}

#[test]
fn words_that_look_like_options_are_text_after_a_double_dash_or_inside_it() {
    let (central, repo) = (tempdir(), tempdir());
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
    let (central, repo) = (tempdir(), tempdir());
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
    let (central, repo) = (tempdir(), tempdir());
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
    let (central, repo, elsewhere) = (tempdir(), tempdir(), tempdir());
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
    let (central, repo) = (tempdir(), tempdir());
    ok(&kb(repo.path(), central.path(), &["init", "demo"], ""));
    let input = format!("{{\"cwd\": {:?}}}", repo.path().display().to_string());
    let asked = ok(&kb(repo.path(), central.path(), &["hook", "prompt"], &input));
    assert!(asked.contains("isn't described yet"), "{asked}");
    ok(&kb(repo.path(), central.path(), &["project", "describe", "-"], "A demo app."));
    assert_eq!(ok(&kb(repo.path(), central.path(), &["hook", "prompt"], &input)), "", "silent once described");
}

#[test]
fn the_edit_hook_sends_an_agent_writing_the_brain_to_kb() {
    let (central, repo) = (tempdir(), tempdir());
    ok(&kb(repo.path(), central.path(), &["init", "demo"], ""));
    let cwd = format!("{:?}", repo.path().display().to_string());
    let input = |file: &str| {
        format!("{{\"cwd\": {cwd}, \"tool_name\": \"Write\", \"tool_input\": {{\"file_path\": {file:?}}}}}")
    };
    let refused = ok(&kb(repo.path(), central.path(), &["hook", "edit"], &input(".brain/0001-plan.md")));
    assert!(refused.contains("\"permissionDecision\":\"deny\""), "{refused}");
    assert_eq!(ok(&kb(repo.path(), central.path(), &["hook", "edit"], &input("src/lib.rs"))), "", "the code is fine");
}

#[cfg(unix)]
#[test]
fn the_edit_hook_reads_a_tilde_as_the_home_folder() {
    let home = tempdir();
    let central = home.path().join("notes");
    std::fs::create_dir(&central).unwrap();
    let input = r#"{"cwd": "/", "tool_input": {"file_path": "~/notes/tags.md"}}"#;
    let mut child = Command::new(env!("CARGO_BIN_EXE_kb"))
        .args(["hook", "edit"])
        .env("KANBRAIN_DIR", &central)
        .env("HOME", home.path())
        .env("XDG_DATA_HOME", home.path().join(".fallback"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
    let refused = String::from_utf8(child.wait_with_output().unwrap().stdout).unwrap();
    assert!(refused.contains("\"permissionDecision\":\"deny\""), "{refused}");
}

// Unix only, as `fallback_inside` says.
#[cfg(unix)]
#[test]
fn a_kb_that_loses_its_central_brain_falls_back_inside_the_tests_folders() {
    // What a mutant of how `KANBRAIN_DIR` is read does: wrote a test's entries into the real central brain once.
    let (central, here) = (tempdir(), tempdir());
    let run = kb_command(here.path(), central.path()).env_remove("KANBRAIN_DIR").args(["path"]).output().unwrap();
    let fallback = std::path::PathBuf::from(ok(&run).trim_end());
    assert!(fallback.starts_with(central.path().join(".fallback")), "{}", fallback.display());
}

// Unix only: on Windows the platform data folder is the real user's, whatever the environment says.
#[cfg(unix)]
#[test]
fn an_empty_kanbrain_dir_is_unset_rather_than_the_current_folder() {
    let (here, home) = (tempdir(), tempdir());
    let run = Command::new(env!("CARGO_BIN_EXE_kb"))
        .args(["path"])
        .current_dir(here.path())
        .env("KANBRAIN_DIR", "")
        .env("HOME", home.path())
        .env("XDG_DATA_HOME", home.path().join("data"))
        .output()
        .unwrap();
    let central = PathBuf::from(ok(&run).trim_end());
    assert!(central.starts_with(home.path()) && central.ends_with("kanbrain"), "{}", central.display());
    assert!(central.join("format").is_file(), "the platform data folder became the central brain");
    assert_eq!(std::fs::read_dir(here.path()).unwrap().count(), 0, "nothing written in the current folder");
}

#[test]
fn a_relative_kanbrain_dir_is_refused() {
    let here = tempdir();
    let run = fallback_inside(&mut Command::new(env!("CARGO_BIN_EXE_kb")), here.path())
        .args(["add", "Lost"])
        .current_dir(here.path())
        .env("KANBRAIN_DIR", "todo")
        .output()
        .unwrap();
    assert!(!run.status.success());
    let said = String::from_utf8_lossy(&run.stderr);
    assert!(said.contains("KANBRAIN_DIR must be an absolute path, not `todo`"), "{said}");
    assert_eq!(std::fs::read_dir(here.path()).unwrap().count(), 0, "nothing written in the current folder");
}

#[test]
fn a_command_waiting_for_its_stdin_holds_up_no_other() {
    let (central, repo) = (tempdir(), tempdir());
    ok(&kb(repo.path(), central.path(), &["init", "demo"], ""));
    ok(&kb(repo.path(), central.path(), &["add", "Shared"], ""));
    let mut typing = kb_command(repo.path(), central.path())
        .args(["comment", "1", "-"])
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
    let (central, code) = (tempdir(), tempdir());
    let children: Vec<_> = (0..8)
        .map(|n| {
            let repo = code.path().join(format!("repo{n}"));
            std::fs::create_dir(&repo).unwrap();
            kb_command(&repo, central.path())
                .args(["init", &format!("project{n}")])
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
    let (central, repo) = (tempdir(), tempdir());
    ok(&kb(repo.path(), central.path(), &["init", "demo"], ""));
    // As the app does: the central brain first, then the repositories.
    let central_lock = Store::at(central.path()).unwrap().lock().unwrap();
    let rename = kb_command(repo.path(), central.path())
        .args(["project", "rename", "renamed"])
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
    let (central, repo) = (tempdir(), tempdir());
    ok(&kb(repo.path(), central.path(), &["init", "demo"], ""));
    ok(&kb(repo.path(), central.path(), &["add", "Shared"], ""));
    // As an agent's subagents would: all at once, each adding a task and commenting on the same one.
    let spawn = |args: Vec<String>| {
        kb_command(repo.path(), central.path())
            .args(args)
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
    let (central, repo) = (tempdir(), tempdir());
    let init = |path: &std::ffi::OsStr| {
        let mut init = kb_command(repo.path(), central.path());
        init.args(["init", "demo"]).env("PATH", path);
        ok(&init.output().unwrap())
    };
    let said = "`kb` isn't on your PATH";
    assert!(init("".as_ref()).contains(said), "agents and the hooks run it");
    let bin = Path::new(env!("CARGO_BIN_EXE_kb")).parent().unwrap();
    assert!(!init(bin.as_os_str()).contains(said));
}
