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
    let settings: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(settings["hooks"]["SessionStart"][1]["matcher"], "startup|resume|clear|compact");
    assert!(settings["hooks"]["Stop"][0].get("matcher").is_none(), "Stop takes no matcher");

    assert_eq!(remove_hooks(root.path()).unwrap(), Change::Written);
    let back: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(back, serde_json::from_str::<Value>(mine).unwrap(), "the user's settings are as they were");
    assert_eq!(remove_hooks(root.path()).unwrap(), Change::Unchanged);
}

#[test]
fn a_hook_written_before_sb_is_replaced_and_removed() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join(SETTINGS);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let old = r#"{"hooks":{"SessionStart":[{"matcher":"startup|resume|clear|compact","hooks":[{"type":"command","command":"kanbrain context"}]}]}}"#;
    fs::write(&path, old).unwrap();
    assert_eq!(add_hooks(root.path()).unwrap(), Change::Written);
    assert_eq!(commands(&path, "SessionStart"), ["kb context"], "replaced, not added");
    fs::write(&path, old).unwrap();
    assert_eq!(remove_hooks(root.path()).unwrap(), Change::Removed, "deinit takes the old one out too");
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
