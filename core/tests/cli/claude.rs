use super::*;

/// The commands each event's hooks run, in `path`'s settings.
fn commands(path: &Path, event: &str) -> Vec<String> {
    let settings: Value = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    settings["hooks"][event]
        .as_array()
        .map(|groups| groups.iter().map(|g| g["hooks"][0]["command"].as_str().unwrap().to_string()).collect())
        .unwrap_or_default()
}

#[test]
fn the_hooks_are_added_once_beside_other_settings_and_removed_cleanly() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join(SETTINGS);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mine = r#"{"permissions":{"allow":["Bash(ls)"]},"hooks":{"SessionStart":[{"hooks":[{"type":"command","command":"date"}]}]}}"#;
    fs::write(&path, mine).unwrap();
    assert_eq!(add_hooks(root.path()).unwrap(), (Change::Written, vec![]));
    assert_eq!(add_hooks(root.path()).unwrap(), (Change::Unchanged, vec![]), "only once");
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.find("permissions") < text.find("hooks"), "keys keep their order: {text}");
    assert_eq!(commands(&path, "SessionStart"), ["date", "kb context"], "next to the user's hook");
    assert_eq!(commands(&path, "UserPromptSubmit"), ["kb hook prompt"]);
    assert_eq!(commands(&path, "Stop"), ["kb hook stop"]);
    assert_eq!(commands(&path, "PreToolUse"), ["kb hook edit", "kb hook tool"]);
    let settings: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(settings["hooks"]["SessionStart"][1]["matcher"], "startup|resume|clear|compact");
    assert!(settings["hooks"]["Stop"][0].get("matcher").is_none(), "Stop takes no matcher");
    assert_eq!(settings["hooks"]["PreToolUse"][0]["matcher"], "Edit|Write|MultiEdit", "the tools that write files");
    assert!(settings["hooks"]["PreToolUse"][1].get("matcher").is_none(), "every tool");

    assert_eq!(remove_hooks(root.path()).unwrap(), (Change::Written, vec![]));
    let back: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(back, serde_json::from_str::<Value>(mine).unwrap(), "the user's settings are as they were");
    assert_eq!(remove_hooks(root.path()).unwrap(), (Change::Unchanged, vec![]));
}

#[test]
fn a_file_holding_only_the_hooks_goes_with_them() {
    let root = tempfile::tempdir().unwrap();
    assert_eq!(add_hooks(root.path()).unwrap(), (Change::Written, vec![]));
    assert_eq!(remove_hooks(root.path()).unwrap(), (Change::Removed, vec![]));
    assert!(!root.path().join(".claude").exists(), "nor the folder it made");
}

#[test]
fn settings_that_arent_json_are_left_alone() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join(SETTINGS);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, "{ not json").unwrap();
    assert!(add_hooks(root.path()).is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), "{ not json");
}

#[test]
fn hooks_are_brought_up_to_date_only_where_kanbrains_are() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join(SETTINGS);
    assert_eq!(update_hooks(root.path()).unwrap(), Change::Unchanged, "no settings: none written anew");
    assert!(!path.exists());
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let theirs = r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"date"}]}]}}"#;
    fs::write(&path, theirs).unwrap();
    assert_eq!(update_hooks(root.path()).unwrap(), Change::Unchanged, "only the user's: left as they are");
    assert_eq!(fs::read_to_string(&path).unwrap(), theirs);

    // An older release's: the ones it lacked are added, the rest kept.
    let older = r#"{"hooks":{"SessionStart":[{"matcher":"startup|resume|clear|compact","hooks":[{"type":"command","command":"kb context"}]}],"PreToolUse":[{"matcher":"Edit|Write|MultiEdit","hooks":[{"type":"command","command":"kb hook edit"}]}]}}"#;
    fs::write(&path, older).unwrap();
    assert_eq!(update_hooks(root.path()).unwrap(), Change::Written);
    assert_eq!(commands(&path, "PreToolUse"), ["kb hook edit", "kb hook tool"]);
    assert_eq!(commands(&path, "Stop"), ["kb hook stop"]);
    assert_eq!(update_hooks(root.path()).unwrap(), Change::Unchanged);

    // `kb` written by its path, as a user may: the same hook, not one to add again, and taken out with the rest.
    let pathed = fs::read_to_string(&path).unwrap().replace("\"kb ", "\"'/Users/John Doe/bin/kb' ");
    fs::write(&path, &pathed).unwrap();
    assert_eq!(update_hooks(root.path()).unwrap(), Change::Unchanged, "{pathed}");

    assert_eq!(remove_hooks(root.path()).unwrap(), (Change::Removed, vec![]));
}

/// Whether a hook running `handler` is kanbrain's `kb hook tool`.
fn is_hook_tool(handler: &Value) -> bool {
    runs(&json!({ "hooks": [handler] }), "kb hook tool")
}

const INSTALLED: &str = r"C:\Users\John Doe\AppData\Local\Programs\kanbrain\kb.exe";

#[test]
fn kanbrains_hook_is_kb_bare_or_by_a_plain_path() {
    let installed = format!("'{INSTALLED}' hook tool");
    let double = format!(r#""{INSTALLED}" hook tool"#);
    // Each line, and whether it's kanbrain's: in bash (or without `shell`), in PowerShell, after `& ` in PowerShell.
    // Bash takes a `\` out unless it's in '…'; PowerShell runs a quoted path only after `&`; only bash expands `~/`.
    let lines: [(&str, bool, bool, bool); 22] = [
        ("kb hook tool", true, true, true),
        ("kb hook tool  ", true, true, true),
        ("kb hook tool\t", true, true, true),
        ("/opt/bin/kb hook tool", true, true, true),
        ("./kb hook tool", true, true, true),
        ("C:/Users/me/KB.EXE hook tool", true, true, true),
        ("/home/me/.local/bin/kb hook tool", true, true, true),
        ("/home/josé/.local/bin/kb hook tool", true, true, true),
        ("~/.local/bin/kb hook tool", true, false, false),
        (r"C:\Users\me\kb.exe hook tool", false, true, true),
        (r"\\srv\share\kb.exe hook tool", false, true, true),
        (&installed, true, false, true),
        (&double, false, false, true),
        (r"'C:\Users\José\AppData\Local\Programs\kanbrain\kb.exe' hook tool", true, false, true),
        (r#""C:\Program Files (x86)\kanbrain\kb.exe" hook tool"#, false, false, true),
        (r"'C:\Program Files (x86)\kanbrain\kb.exe' hook tool", true, false, true),
        (r"'\\srv\share\kb.exe' hook tool", true, false, true),
        (r#""\\srv\share\kb.exe" hook tool"#, false, false, true),
        ("'/Users/John Doe/bin/kb' hook tool", true, false, true),
        (r#""/Users/John Doe/bin/kb" hook tool"#, true, false, true),
        (r#""/home/josé/bin/kb" hook tool"#, true, false, true),
        ("'/home/josé/bin/kb' hook tool", true, false, true),
    ];
    for (line, bash, powershell, call) in lines {
        assert_eq!(is_hook_tool(&json!({ "type": "command", "command": line })), bash, "{line}");
        assert_eq!(is_hook_tool(&json!({ "type": "command", "command": line, "shell": "bash" })), bash, "bash: {line}");
        let pwsh = json!({ "type": "command", "command": line, "shell": "powershell" });
        assert_eq!(is_hook_tool(&pwsh), powershell, "pwsh: {line}");
        let called = format!("& {line}");
        assert_eq!(
            is_hook_tool(&json!({ "type": "command", "command": called, "shell": "powershell" })),
            call,
            "{called}"
        );
        assert!(!is_hook_tool(&json!({ "type": "command", "command": called })), "`&` is PowerShell's: {called}");
    }
    for program in [
        "kb",
        "/opt/bin/kb",
        INSTALLED,
        r"C:\Program Files (x86)\kanbrain\KB.exe",
        "/Users/John Doe/bin/kb",
        "/home/josé/kb",
        r"\\srv\share\kb.exe",
    ] {
        let exec = json!({ "type": "command", "command": program, "args": ["hook", "tool"] });
        assert!(is_hook_tool(&exec), "{program}");
    }
    // Only a command hook runs anything.
    for kind in [json!("prompt"), json!("agent"), json!(null)] {
        let mut handler = json!({ "command": "kb hook tool" });
        if !kind.is_null() {
            handler["type"] = kind.clone();
        }
        assert!(!is_hook_tool(&handler), "{kind}");
    }
}

#[test]
fn a_path_no_shell_reads_as_itself_is_the_users() {
    for line in [
        // A `\` outside a Windows path, in any of its folders: bash takes it out (`/usr/binx/kb`).
        r"/usr/bin\x/kb hook tool",
        r"C:\x/kb hook tool",
        r"'C:\x/kb' hook tool",
        // Dashes, spaces and quotes PowerShell reads as its own, though not ASCII.
        "/opt/a\u{2013}b/kb hook tool",
        "/opt/a\u{2014}b/kb hook tool",
        "/opt/a\u{a0}b/kb hook tool",
        "'/opt/a\u{2019}b/kb' hook tool",
        "\"/opt/a\u{201d}b/kb\" hook tool",
        // `~` only starting a bare path, as `~/`.
        "'~/bin/kb' hook tool",
        "\"~/bin/kb\" hook tool",
        "/opt/~x/kb hook tool",
        "~user/kb hook tool",
        "~kb hook tool",
    ] {
        for shell in [json!(null), json!("bash"), json!("powershell")] {
            let mut handler = json!({ "type": "command", "command": line });
            if !shell.is_null() {
                handler["shell"] = shell.clone();
            }
            assert!(!is_hook_tool(&handler), "{line} ({shell})");
            handler["command"] = json!(format!("& {line}"));
            assert!(!is_hook_tool(&handler), "& {line} ({shell})");
        }
    }
    for program in [r"C:\x/kb", r"/usr/bin\x/kb", "~/bin/kb", "/opt/a\u{2013}b/kb"] {
        let exec = json!({ "type": "command", "command": program, "args": ["hook", "tool"] });
        assert!(!is_hook_tool(&exec), "{program}");
    }
}

#[test]
fn hooks_kanbrain_took_as_its_own_by_their_path_still_are() {
    // As v0.6.2 took them: `kb` by a path without spaces, `~` and non-ASCII letters included.
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join(SETTINGS);
    add_hooks(root.path()).unwrap();
    let pathed = fs::read_to_string(&path)
        .unwrap()
        .replace("\"kb context", "\"~/.local/bin/kb context")
        .replace("\"kb hook", "\"/home/josé/.local/bin/kb hook");
    fs::write(&path, &pathed).unwrap();
    assert_eq!(add_hooks(root.path()).unwrap(), (Change::Unchanged, vec![]), "{pathed}");
    assert_eq!(remove_hooks(root.path()).unwrap(), (Change::Removed, vec![]));
}

#[test]
fn a_prompt_hook_with_kbs_words_is_no_look_alike() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join(SETTINGS);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let prompt = json!({ "type": "prompt", "command": "kb hook tool" });
    fs::write(&path, json!({ "hooks": { "PreToolUse": [{ "hooks": [prompt] }] } }).to_string()).unwrap();
    assert_eq!(add_hooks(root.path()).unwrap(), (Change::Written, vec![]));
    assert_eq!(commands(&path, "PreToolUse"), ["kb hook tool", "kb hook edit", "kb hook tool"]);
    assert_eq!(remove_hooks(root.path()).unwrap(), (Change::Written, vec![]));
    assert_eq!(commands(&path, "PreToolUse"), ["kb hook tool"], "the prompt hook stays");
}

#[test]
fn anything_else_is_the_users() {
    for command in LOOK_ALIKES.iter().chain(&OTHERS) {
        for shell in [json!(null), json!("bash"), json!("powershell")] {
            let mut handler = json!({ "type": "command", "command": command });
            if !shell.is_null() {
                handler["shell"] = shell.clone();
            }
            assert!(!is_hook_tool(&handler), "{command} ({shell})");
        }
    }
    for (program, args) in [
        ("kb", json!(["hook"])),
        ("kb", json!(["hook", "tool", "x"])),
        ("kb", json!(["hook tool"])),
        ("kb", json!(["hook", 1])),
        ("kb", json!("hook tool")),
        ("kb hook tool", json!([])),
        ("/opt/kb.sh", json!(["hook", "tool"])),
        ("/opt/kbx", json!(["hook", "tool"])),
        ("/opt/$x/kb", json!(["hook", "tool"])),
        ("C:\\x\\\"kb.exe", json!(["hook", "tool"])),
        // `\` separates only in a Windows path: elsewhere it's part of a name (a file `x\kb`), or bash eats it.
        ("/opt/x\\kb", json!(["hook", "tool"])),
        ("", json!(["hook", "tool"])),
    ] {
        let exec = json!({ "type": "command", "command": program, "args": args });
        assert!(!is_hook_tool(&exec), "{program} {args}");
    }
    for shell in [json!("zsh"), json!("cmd"), json!(1)] {
        assert!(!is_hook_tool(&json!({ "type": "command", "command": "kb hook tool", "shell": shell })), "{shell}");
    }
    assert!(!is_hook_tool(&json!({ "type": "command" })), "no command");
    assert!(!runs(&json!({}), "kb hook tool"), "no hooks");
}

#[test]
fn every_earlier_counterexample_is_the_users() {
    // The negatives of both earlier rounds' shell readings, and their positives this whitelist refuses: none is
    // kanbrain's, in any shell.
    let bash = [
        "/Users/John Doe/bin/kb hook tool",
        r"C:\Program Files\kanbrain\kb.exe hook tool",
        "cd /x && /usr/bin/kb hook tool",
        "true&&/usr/bin/kb hook tool",
        "date\n/usr/bin/kb hook tool",
        "\"/a b/x\";/opt/kb hook tool",
        r"x;C:\kb.exe hook tool",
        "\"$(rm -rf /tmp/x)\"/kb hook tool",
        "$(echo /opt)/kb hook tool",
        "`date`/kb hook tool",
        "\"`date`\"/kb hook tool",
        "echo '/a b/kb' hook tool",
        "\"/a b/kb\" hook tool; date",
        "kb hook tool > /tmp/log",
        "kb hook tool < /dev/null",
        "kb hook tool | cat",
        "kb hook tool &",
        "(kb hook tool)",
        "kb hook 'tool x'",
        "kb hook edit",
        "kb hook tool more",
        "kbx hook tool",
        "/opt/kb.sh hook tool",
        "/opt/kb.py hook tool",
        "/opt/kb.bak hook tool",
        r#""/a b/kbx.exe" hook tool"#,
        "\"A=b\" kb hook tool",
        "A\"=b\" kb hook tool",
        r"\A=b kb hook tool",
        "\"/a b/kb hook tool",
        "'/a b/kb hook tool",
        "kb hook tool\\",
        "",
        "kb  hook tool",
        r"/Users/John\ Doe/bin/kb hook tool",
        r#"/Users/"John Doe"/bin/kb hook tool"#,
        r#""/Users/a \"b\" \\c/kb" hook tool"#,
        r#""/a \$(b)/kb" hook tool"#,
        r#"KANBRAIN_DIR="/a b" "/Users/John Doe/bin/kb" hook tool"#,
        "KANBRAIN_DIR=$HOME/x kb hook tool",
        "\"$HOME/John Doe/kb\" hook tool",
        "kb 'hook' \"tool\"",
    ];
    let powershell = [
        format!("& '{INSTALLED}' hook tool; date"),
        format!("& '{INSTALLED}' hook tool | Out-Null"),
        format!("& '{INSTALLED}' hook tool > x"),
        format!("& '{INSTALLED}' hook tool\ndate"),
        "kb hook tool && date".into(),
        "& & kb hook tool".into(),
        r#"& "$(Remove-Item x)\kb.exe" hook tool"#.into(),
        "$(Remove-Item x)\\kb.exe hook tool".into(),
        "& \"C:\\a`b\\kb.exe\" hook tool".into(),
        "kb hook `tool".into(),
        "kb hook,tool".into(),
        "(kb) hook tool".into(),
        "& { kb hook tool }".into(),
        r"& 'C:\a b\kb.ps1' hook tool".into(),
        r"C:\a b\kb.exe hook tool".into(),
        r"& 'C:\a b'\kb.exe hook tool".into(),
        r"& C:\a'b'\kb.exe hook tool".into(),
        "& hook'kb' tool".into(),
        r"& 'C:\a b\kb.exe hook tool".into(),
        "kb hook edit".into(),
        "&".into(),
        format!("&'{INSTALLED}' hook tool"),
        r"& 'C:\Users\John''s\kb.exe' hook tool".into(),
        "& 'C:\\a`b\\kb.exe' hook tool".into(),
        r#"& "$env:LOCALAPPDATA\Programs\kanbrain\kb.exe" hook tool"#.into(),
        r"& 'C:\$(x)\kb.exe' hook tool".into(),
        r#"& "C:\Users\a ""b""\kb.exe" hook tool"#.into(),
        "& kb 'hook' \"tool\"".into(),
    ];
    for command in bash.iter().copied().chain(powershell.iter().map(String::as_str)) {
        for shell in [json!(null), json!("bash"), json!("powershell")] {
            let mut handler = json!({ "type": "command", "command": command });
            if !shell.is_null() {
                handler["shell"] = shell.clone();
            }
            assert!(!is_hook_tool(&handler), "{command} ({shell})");
        }
    }
    // Under PowerShell a quoted program runs only after `&`: without it, it's a string.
    let quoted = format!("'{INSTALLED}' hook tool");
    assert!(!is_hook_tool(&json!({ "type": "command", "command": quoted, "shell": "powershell" })), "{quoted}");
}

#[test]
fn a_hook_that_only_names_kb_and_a_hooks_words_gets_kanbrains_beside_it() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join(SETTINGS);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let theirs = json!({ "hooks": { "SessionStart": [{ "hooks": [{ "type": "command", "command": "cat ~/notes/kb/context" }] }] } });
    fs::write(&path, theirs.to_string()).unwrap();
    assert_eq!(add_hooks(root.path()).unwrap(), (Change::Written, vec![]));
    assert_eq!(commands(&path, "SessionStart"), ["cat ~/notes/kb/context", "kb context"]);
}

#[test]
fn an_exec_look_alike_is_told_of_with_its_arguments_as_written() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join(SETTINGS);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let exec = json!({ "type": "command", "command": "/opt/x\\kb", "args": ["hook", "tool", 1] });
    let theirs = json!({ "hooks": { "PreToolUse": [{ "hooks": [exec] }] } });
    fs::write(&path, theirs.to_string()).unwrap();
    let edited = Edited { event: "PreToolUse", command: "/opt/x\\kb hook tool 1".into(), hook: "kb hook tool" };
    assert_eq!(add_hooks(root.path()).unwrap(), (Change::Written, vec![edited]));
}

/// Hooks of the user's that run `kb hook tool`, or seem to, in a way kanbrain doesn't vouch for: each is left alone
/// and told of.
const LOOK_ALIKES: [&str; 50] = [
    "KANBRAIN_DIR=/x A=1 kb hook tool",
    "/Users/John Doe/bin/kb hook tool",
    r"C:\Program Files\kanbrain\kb.exe hook tool",
    "&'/opt/kb' hook tool",
    "cd /x && /usr/bin/kb hook tool",
    "true&&/usr/bin/kb hook tool",
    "date\n/usr/bin/kb hook tool",
    "/usr/bin/kb hook tool\ndate",
    "\"/a b/x\";/opt/kb hook tool",
    r"x;C:\kb.exe hook tool",
    "\"$(rm -rf /tmp/x)\"/kb hook tool",
    "$HOME/bin/kb hook tool",
    "`date`/kb hook tool",
    r#""C:\x\";calc;\"\kb.exe" hook tool"#,
    "& \u{2018}C:\\a b\\kb.exe\u{2019} hook tool",
    "\u{201c}C:\\a b\\kb.exe\u{201d} hook tool",
    "\"/a 'b'/kb\" hook tool",
    "'/a \"b\"/kb' hook tool",
    "\"/a b,c/kb\" hook tool",
    "\"/a b*/kb\" hook tool",
    "/opt/k?/kb hook tool",
    "\"A=b\" kb hook tool",
    "kb hook tool > /tmp/log",
    "kb hook tool | cat",
    "kb  hook tool",
    "kb 'hook' \"tool\"",
    "kb hook tool; date",
    "kb hook tool &",
    "{ kb hook tool; }",
    "/opt/KB hook tool",
    // `\` outside a Windows path: bash runs `/usr/bin/xkb`, or `xkb`; or a file named `x\kb`.
    r"/usr/bin/x\kb hook tool",
    r"x\kb hook tool",
    r"'/opt/x\kb' hook tool",
    r#""/opt/x\\kb" hook tool"#,
    // `kb` run some other way, or named in prose: told of either way, never doubled.
    r#"KANBRAIN_DIR="/a b" kb hook tool"#,
    "KANBRAIN_DIR='/a b' kb hook tool",
    "if x; then kb hook tool; fi",
    "while x; do kb hook tool; done",
    "! kb hook tool",
    "exec kb hook tool",
    "env KANBRAIN_DIR=/x kb hook tool",
    "/usr/bin/env kb hook tool",
    "command kb hook tool",
    "time kb hook tool",
    "nohup kb hook tool",
    "bash -c 'kb hook tool'",
    r#""$HOME/My Tools/kb" hook tool"#,
    "echo kb hook tool >> log",
    "git log --grep kb hook tool",
    "/bin/echo kb hook tool",
];

/// Hooks of the user's that aren't kanbrain's and don't look like it: left alone, unsaid.
const OTHERS: [&str; 16] = [
    "date",
    "echo hook tool",
    "kbx hook tool",
    "/opt/kb.sh hook tool",
    "kb hook toolbox",
    "kb hook tools",
    "kb tool hook",
    "kb hook",
    "kb",
    "tool hook kb",
    "",
    // A path that names no `kb`, its `\` before the closing quote; `kb.exe` followed by more than the hook.
    r#""C:\a b\kb.exe\" hook tool"#,
    "'C:\\a b\\kb.exe\u{2019}; calc; \u{2018}' hook tool",
    // `kb` and the hook's words, but not `kb` running it.
    "cat ~/notes/kb/hook/tool",
    "echo kb > x; other hook tool",
    "grep -r kb . && hook tool",
];

#[test]
fn a_hook_edited_by_hand_is_told_of_rather_than_doubled_or_taken_out() {
    for command in LOOK_ALIKES {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join(SETTINGS);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let theirs = json!({ "hooks": { "PreToolUse": [{ "hooks": [{ "type": "command", "command": command }] }] } });
        fs::write(&path, theirs.to_string()).unwrap();
        let edited = vec![Edited { event: "PreToolUse", command: command.to_string(), hook: "kb hook tool" }];
        assert_eq!(add_hooks(root.path()).unwrap(), (Change::Written, edited.clone()), "{command}");
        assert_eq!(commands(&path, "PreToolUse"), [command, "kb hook edit"], "no copy beside it: {command}");
        assert_eq!(add_hooks(root.path()).unwrap(), (Change::Unchanged, edited.clone()), "told again: {command}");
        assert_eq!(update_hooks(root.path()).unwrap(), Change::Unchanged, "{command}");
        assert_eq!(remove_hooks(root.path()).unwrap(), (Change::Written, edited), "{command}");
        let back: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(back, theirs, "left as it was: {command}");
    }
}

#[test]
fn a_hook_of_the_users_own_is_left_alone_and_unsaid() {
    for command in OTHERS {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join(SETTINGS);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let theirs = json!({ "hooks": { "PreToolUse": [{ "hooks": [{ "type": "command", "command": command }] }] } });
        fs::write(&path, theirs.to_string()).unwrap();
        assert_eq!(add_hooks(root.path()).unwrap(), (Change::Written, vec![]), "{command}");
        assert_eq!(commands(&path, "PreToolUse"), [command, "kb hook edit", "kb hook tool"], "{command}");
        assert_eq!(remove_hooks(root.path()).unwrap(), (Change::Written, vec![]), "{command}");
        let back: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(back, theirs, "left as it was: {command}");
    }
}

#[test]
fn a_hook_edited_by_hand_beside_kanbrains_is_unsaid_until_kanbrains_goes() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join(SETTINGS);
    add_hooks(root.path()).unwrap();
    let mut settings: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    let stop = json!({ "hooks": [{ "type": "command", "command": "cd /x && kb hook stop" }] });
    settings["hooks"]["Stop"].as_array_mut().unwrap().push(stop);
    // An exec form with an argument more.
    let exec =
        json!({ "hooks": [{ "type": "command", "command": "/Users/John Doe/bin/kb", "args": ["context", "x"] }] });
    settings["hooks"]["SessionStart"] = json!([exec]);
    fs::write(&path, settings.to_string()).unwrap();
    let context =
        Edited { event: "SessionStart", command: "/Users/John Doe/bin/kb context x".into(), hook: "kb context" };
    assert_eq!(add_hooks(root.path()).unwrap(), (Change::Unchanged, vec![context.clone()]), "kanbrain's Stop is there");
    let stop = Edited { event: "Stop", command: "cd /x && kb hook stop".into(), hook: "kb hook stop" };
    assert_eq!(remove_hooks(root.path()).unwrap(), (Change::Written, vec![context, stop]));
    assert_eq!(commands(&path, "Stop"), ["cd /x && kb hook stop"]);
    assert_eq!(commands(&path, "SessionStart"), ["/Users/John Doe/bin/kb"]);
    assert_eq!(commands(&path, "PreToolUse"), Vec::<String>::new());
}
