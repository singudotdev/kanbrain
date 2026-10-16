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
    assert_eq!(add_hooks(root.path()).unwrap(), Change::Written);
    assert_eq!(add_hooks(root.path()).unwrap(), Change::Unchanged, "only once");
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

    assert_eq!(remove_hooks(root.path()).unwrap(), Change::Written);
    let back: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(back, serde_json::from_str::<Value>(mine).unwrap(), "the user's settings are as they were");
    assert_eq!(remove_hooks(root.path()).unwrap(), Change::Unchanged);
}

#[test]
fn a_file_holding_only_the_hooks_goes_with_them() {
    let root = tempfile::tempdir().unwrap();
    assert_eq!(add_hooks(root.path()).unwrap(), Change::Written);
    assert_eq!(remove_hooks(root.path()).unwrap(), Change::Removed);
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

    // `kb` written by its path, as a user may: the same hook, not one to add again.
    let pathed = fs::read_to_string(&path).unwrap().replace("\"kb ", "\"/opt/bin/kb ");
    fs::write(&path, &pathed).unwrap();
    assert_eq!(update_hooks(root.path()).unwrap(), Change::Unchanged, "{pathed}");
    assert_eq!(remove_hooks(root.path()).unwrap(), Change::Removed, "and taken out with the rest");

    // With variables set before it, the same; a line of the user's that runs `kb` after another command isn't.
    add_hooks(root.path()).unwrap();
    let set = fs::read_to_string(&path).unwrap().replace("\"kb context", "\"KANBRAIN_DIR=/x A=1 kb context");
    let set = set.replace("\"kb hook stop", "\"cd /x && /usr/bin/kb hook stop");
    fs::write(&path, &set).unwrap();
    assert_eq!(update_hooks(root.path()).unwrap(), Change::Written, "{set}");
    assert_eq!(commands(&path, "SessionStart"), ["KANBRAIN_DIR=/x A=1 kb context"]);
    assert_eq!(commands(&path, "Stop"), ["cd /x && /usr/bin/kb hook stop", "kb hook stop"]);
    remove_hooks(root.path()).unwrap();
    assert_eq!(commands(&path, "Stop"), ["cd /x && /usr/bin/kb hook stop"], "the user's, kept");
}

#[test]
fn a_word_sets_a_variable_as_the_shell_takes_it() {
    for word in ["KANBRAIN_DIR=/x", "_X=1", "a1=", "A=b=c"] {
        assert!(assigns(word), "{word}");
    }
    for word in ["1X=2", "a-b=1", "=x", "--dir=/x", "kb", "x"] {
        assert!(!assigns(word), "{word}");
    }
}
