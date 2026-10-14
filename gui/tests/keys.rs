use super::*;

#[test]
fn a_key_runs_one_thing_on_each_tab() {
    for tab in Tab::ALL {
        let runs: Vec<&Action> = ACTIONS.iter().filter(|a| a.runs && a.tabs.has(tab)).collect();
        for (i, a) in runs.iter().enumerate() {
            for b in &runs[i + 1..] {
                if a.matches(b.key) {
                    // As addresses: the same function, not two that happen to do alike.
                    assert_eq!(a.run as usize, b.run as usize, "{:?} on {tab:?}: {} and {}", a.key, a.does, b.does);
                }
            }
        }
    }
}

#[test]
fn what_the_menu_offers_it_names_and_the_palette_describes() {
    for action in ACTIONS.iter().filter(|a| !a.label.is_empty()) {
        assert!(!action.does.is_empty(), "{} is in the menu but not the palette", action.label);
        assert!(action.runs, "{} only tells", action.label);
    }
}

#[test]
fn the_palette_puts_only_the_entrys_own_commands_first() {
    let section = |shown: &str| ACTIONS.iter().find(|a| a.shown == shown).unwrap().section();
    assert_eq!(section("e"), Section::Entry);
    assert_ne!(section("A"), Section::Entry, "adding a project isn't about the entry");
}

#[test]
fn the_palettes_rows_are_plain_text() {
    // Drawn as written, unlike the status bar's messages, which drop markdown's backticks.
    for action in ACTIONS {
        assert!(!action.does.contains('`'), "{}", action.does);
    }
}

#[test]
fn every_key_is_documented_on_its_tabs() {
    // On Windows, git may check the docs out with CRLF line endings.
    let docs = include_str!("../../docs/keybindings.md").replace("\r\n", "\n");
    let section = |title: &str| {
        docs.split("\n## ").find(|s| s.starts_with(title)).unwrap_or_else(|| panic!("no section {title}"))
    };
    for action in ACTIONS.iter().filter(|a| !a.does.is_empty()) {
        let tabs: &[&str] = match action.tabs {
            Tabs::Report => &["Report tab"],
            Tabs::Calendar => &["Calendar tab"],
            Tabs::Every => &["Board, notes and decisions", "Report tab", "Calendar tab"],
            Tabs::Entries | Tabs::Board | Tabs::Lists => &["Board, notes and decisions"],
        };
        for tab in tabs {
            for key in action.shown.split([' ', '/']).filter(|k| !k.is_empty()) {
                assert!(
                    section(tab).contains(&format!("`{key}`")),
                    "docs/keybindings.md, {tab}: `{key}` ({})",
                    action.does
                );
            }
        }
    }
}
