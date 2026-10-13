use super::*;

/// cargo-about's report for two licenses: one only this workspace's crates use, one shared with a dependency.
fn report() -> About {
    serde_json::from_str(
        r#"{"licenses": [
            {"name": "GNU Affero General Public License v3.0 only", "id": "AGPL-3.0-only", "text": "AGPL text",
             "used_by": [{"crate": {"name": "kanbrain-core", "version": "0.1.0", "repository": null}}]},
            {"name": "MIT License", "id": "MIT", "text": "  Copyright (c) Someone\n\nPermission...  \n",
             "used_by": [{"crate": {"name": "anyhow", "version": "1.0.104",
                                    "repository": "https://github.com/dtolnay/anyhow"}},
                         {"crate": {"name": "kanbrain", "version": "0.1.0", "repository": null}},
                         {"crate": {"name": "dirs", "version": "7.0.0", "repository": null}}]}
        ]}"#,
    )
    .unwrap()
}

#[test]
fn the_notices_list_each_license_with_its_crates_and_leave_this_workspace_out() {
    let text = notices(&PROGRAMS[0], &report());
    assert!(text.starts_with(HEADER));
    assert!(!text.contains("AGPL text"), "only this workspace's crates use it");
    assert!(!text.contains("kanbrain-core") && !text.contains("- kanbrain "), "this workspace's crates aren't listed");
    let mit = "MIT License (MIT), used by:\n  - anyhow 1.0.104 (https://github.com/dtolnay/anyhow)\n  - dirs 7.0.0\n\n\
               Copyright (c) Someone\n\nPermission...\n";
    assert!(text.contains(mit), "{text}");
}

#[test]
fn each_program_carries_the_licenses_of_what_isnt_a_crate() {
    let kb = notices(&PROGRAMS[0], &About { licenses: Vec::new() });
    assert!(kb.contains("The Rust standard library") && kb.contains("Rich Felker"), "Rust's std and musl");
    assert!(!kb.contains("Lilex"));
    let app = notices(&PROGRAMS[1], &About { licenses: Vec::new() });
    assert!(app.contains("IBM Plex Sans") && app.contains("The Lilex Project Authors"), "the fonts GPUI embeds");
    assert!(!app.contains("musl"));
}

#[test]
fn the_notices_accept_the_licenses_cargo_deny_allows() {
    // The quoted names in the list that `key = [` starts in `file`, one per line as both files write them.
    let list = |file: &str, key: &str| -> std::collections::BTreeSet<String> {
        let text = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(file)).unwrap();
        let start = text.find(&format!("\n{key} = [")).unwrap_or_else(|| panic!("{file} has no {key}"));
        let list = &text[start..start + text[start..].find(']').unwrap()];
        list.lines().filter_map(|l| Some(l.trim().strip_prefix('"')?.strip_suffix("\",")?.to_string())).collect()
    };
    let mut allowed = list("../deny.toml", "allow");
    allowed.insert("AGPL-3.0-only".into());
    assert_eq!(
        list("about.toml", "accepted"),
        allowed,
        "about.toml accepts deny.toml's licenses and the program's own"
    );
}
