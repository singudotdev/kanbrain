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
