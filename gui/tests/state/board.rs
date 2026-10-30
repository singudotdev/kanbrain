//! The board, its tabs and its keys: moving cards and the selection, quick add, search, the menu and the palette,
//! undo, the report and the calendar.

use super::*;

#[test]
fn the_menu_offers_what_the_keys_do_and_presses_them() {
    let mut f = Fixture::new(&["feature", "bug"]);
    f.board.open_menu(k(1));
    let labels: Vec<&str> = f.board.menu_items(k(1)).iter().map(|i| i.label).collect();
    assert_eq!(
        labels,
        [
            "Show details",
            "Start",
            "Park",
            "Done",
            "Edit title and tags",
            "Edit description",
            "Add comment",
            "Link",
            "Open file",
            "Delete"
        ],
        "no move to its own status, and no Move to project with one brain"
    );
    f.press(KeyCode::Down);
    f.press(KeyCode::Enter);
    assert_eq!(f.status(1), Status::Doing, "row 1 is Start");
    assert!(matches!(f.board.mode, Mode::Normal), "choosing closes it");
    f.board.open_menu(k(2));
    f.press(KeyCode::Char('d'));
    assert_eq!(f.status(2), Status::Done, "an item's own key chooses it");
    f.board.open_menu(k(2));
    assert!(!f.board.menu_items(k(2)).iter().any(|i| i.label == "Done"), "done tasks don't offer Done");
    f.press(KeyCode::Esc);
    assert!(matches!(f.board.mode, Mode::Normal), "esc closes it");
}

#[test]
fn opens_with_the_first_card_selected() {
    let f = Fixture::new(&["feature", "bug"]);
    assert_eq!(f.board.tab, Tab::Board);
    assert_eq!(f.board.position(), Some((0, 0)));
    assert_eq!(f.board.column(Status::Todo).len(), 2);
}

#[test]
fn the_list_layout_walks_every_section_with_up_and_down() {
    let mut f = Fixture::new(&["a", "b", "c"]);
    f.board.select(k(1));
    f.press(KeyCode::Char('s'));
    f.press(KeyCode::Char('v'));
    assert_eq!(f.board.layout, Layout::List);
    assert_eq!(f.board.selected, Some(k(1)), "switching keeps the selection");
    let order = f.board.visible_ids();
    assert_eq!(order.last(), Some(&k(1)), "To do's rows, then Doing's");
    f.board.select(order[0]);
    for &next in &order[1..] {
        f.press(KeyCode::Down);
        assert_eq!(f.board.selected, Some(next), "down goes row by row, across sections");
    }
    f.press(KeyCode::Left);
    assert_eq!(f.board.selected, Some(k(1)), "no columns to step sideways to");
    f.shift(KeyCode::Right);
    assert_eq!(f.status(1), Status::Parked, "moving a task to the next status still works");
    f.press(KeyCode::Esc);
    f.press(KeyCode::Char('v'));
    assert_eq!(f.board.layout, Layout::Kanban);
}

#[test]
fn new_tasks_from_the_keyboard_go_to_to_do() {
    let mut f = Fixture::new(&["a", "b"]);
    f.board.set_status(k(1), Status::Done);
    f.board.select(k(1));
    f.press(KeyCode::Char('n'));
    f.type_text("form");
    f.press(KeyCode::Enter);
    assert_eq!(f.status(3), Status::Todo, "n: To do, whatever card is selected");
    f.board.select(k(1));
    f.press(KeyCode::Char('v'));
    f.press(KeyCode::Char('a'));
    f.type_text("quick");
    f.press(KeyCode::Enter);
    f.press(KeyCode::Esc);
    assert_eq!(f.status(4), Status::Todo, "a: To do too, in the list as on the columns");
    f.board.switch_tab(Tab::Report);
    f.press(KeyCode::Char('n'));
    f.type_text("elsewhere");
    f.press(KeyCode::Enter);
    assert_eq!(f.status(5), Status::Todo, "n works on the report tab, whose button presses it");
}

#[test]
fn t_takes_a_task_back_to_to_do_and_hash_alone_filters_the_reports_tags() {
    let mut f = Fixture::new(&["a"]);
    f.press(KeyCode::Char('d'));
    f.press(KeyCode::Char('t'));
    assert_eq!(f.status(1), Status::Todo, "t, like s p d, names the status");
    f.board.switch_tab(Tab::Report);
    f.press(KeyCode::Char('f'));
    assert!(
        matches!(f.board.mode, Mode::Normal) && !f.board.all_done,
        "f is the board's, for done tasks; not a second tag filter"
    );
    for tab in [Tab::Notes, Tab::Decisions] {
        f.board.switch_tab(tab);
        f.press(KeyCode::Char('f'));
        assert!(!f.board.all_done, "nor on {tab:?}, where it would change the board unseen");
    }
    f.press(KeyCode::Char('#'));
    assert!(matches!(f.board.mode, Mode::Tags(_)));
}

#[test]
fn shift_arrows_move_the_card_across_columns_and_record_it() {
    let mut f = Fixture::new(&["feature"]);
    f.shift(KeyCode::Right);
    assert_eq!(f.status(1), Status::Doing);
    assert!(f.board.brain(0).task(1).unwrap().is_running(), "doing runs the clock");
    f.shift(KeyCode::Right);
    f.shift(KeyCode::Right);
    assert_eq!(f.status(1), Status::Done);
    f.shift(KeyCode::Right);
    assert_eq!(f.status(1), Status::Done, "done is the last column");
    let steps: Vec<_> = f.on_disk(1).history.iter().map(|c| (c.from, c.to)).collect();
    assert_eq!(steps, [(Status::Todo, Status::Doing), (Status::Doing, Status::Parked), (Status::Parked, Status::Done)]);
    assert_eq!(f.board.selected, Some(k(1)), "the card stays selected as it moves");
}

#[test]
fn stopping_your_task_asks_nothing_and_selects_it() {
    let mut f = Fixture::new(&["feature", "bug"]);
    f.board.set_status(k(1), Status::Doing);
    f.board.set_status(k(1), Status::Parked);
    assert!(matches!(f.board.mode, Mode::Normal), "parking it: the board shows what's there to start");
    f.board.set_status(k(2), Status::Doing);
    f.board.open_entry(k(2));
    f.board.set_status(k(2), Status::Done);
    assert!(matches!(f.board.mode, Mode::Normal), "finishing it");
    assert_eq!(f.board.selected, Some(k(2)));
    f.board.set_status(k(1), Status::Doing);
    f.board.open_entry(k(1));
    f.press(KeyCode::Char('x'));
    f.press(KeyCode::Char('y'));
    assert!(matches!(f.board.mode, Mode::Normal), "deleting it");
    assert_eq!(f.board.brain(0).running().count(), 0);
}

#[test]
fn starting_a_second_task_of_yours_is_refused_but_an_agents_isnt() {
    let mut f = Fixture::new(&["feature", "bug", "agent's"]);
    f.board.set_status(k(1), Status::Doing);
    f.board.drop_card(k(2), Status::Doing);
    assert_eq!((f.status(1), f.status(2)), (Status::Doing, Status::Todo), "nobody's task is parked for it");
    assert!(f.board.message().unwrap().contains("#1 is in doing"), "{:?}", f.board.message());
    f.board.brain_mut(0).author = kanbrain_core::model::Author::Agent;
    f.board.brain_mut(0).set_status(3, Status::Doing).unwrap();
    f.board.brain_mut(0).author = kanbrain_core::model::Author::Human;
    assert_eq!(f.board.running().count(), 2, "yours and the agent's both run");
}

#[test]
fn dropping_a_card_moves_it_unless_it_is_its_own_column() {
    let mut f = Fixture::new(&["feature", "bug"]);
    f.board.drop_card(k(2), Status::Done);
    assert_eq!((f.status(2), f.board.selected), (Status::Done, Some(k(2))));
    f.board.drop_card(k(1), Status::Todo);
    assert_eq!((f.status(1), f.board.selected), (Status::Todo, Some(k(1))));
    assert!(f.on_disk(1).history.is_empty(), "dropping it back writes nothing");
    assert!(!f.board.message().is_some_and(|m| m.contains("already")), "and says nothing");
}

#[test]
fn a_dragged_card_is_found_by_its_brains_folder() {
    let mut f = Fixture::new(&["feature", "bug"]);
    let gone = f.dir.path().join("gone");
    f.board.drop_dragged(&gone, 2, Status::Done);
    assert_eq!(f.status(2), Status::Todo, "its brain gone from the list meanwhile: nothing moves");
    let dir = f.dir.path().to_path_buf();
    f.board.drop_dragged(&dir, 2, Status::Done);
    assert_eq!((f.status(2), f.board.selected), (Status::Done, Some(k(2))));
}

#[test]
fn arrows_move_between_columns_and_cards() {
    let mut f = Fixture::new(&["a", "b", "c"]);
    f.board.set_status(k(3), Status::Parked);
    f.board.select(k(2));
    f.press(KeyCode::Down);
    let below = f.board.selected;
    assert_ne!(below, Some(k(2)));
    f.press(KeyCode::Right);
    assert_eq!(f.board.selected, Some(k(3)), "doing is empty, so it skips to parked");
    f.press(KeyCode::Left);
    assert_eq!(f.board.position().map(|p| p.0), Some(0));
    f.press(KeyCode::Char('l'));
    assert_eq!(f.board.position().map(|p| p.0), Some(0), "only the arrows move, not vi's letters");
}

#[test]
fn quick_add_creates_tasks_in_the_column_and_keeps_typing() {
    let mut f = Fixture::new(&[]);
    f.board.quick_add(Status::Parked);
    f.type_text("later");
    f.press(KeyCode::Enter);
    f.type_text("much later");
    f.press(KeyCode::Enter);
    f.press(KeyCode::Esc);
    assert!(matches!(f.board.mode, Mode::Normal));
    let titles: Vec<_> = f.board.column(Status::Parked).iter().map(|(_, t)| t.title.clone()).collect();
    assert_eq!(titles, ["much later", "later"]);
}

#[test]
fn the_link_pickers_highlight_follows_the_entry_it_links() {
    let mut f = Fixture::new(&["a", "b", "c"]);
    f.board.select(k(1));
    f.press(KeyCode::Char('r'));
    f.press(KeyCode::Down);
    f.press(KeyCode::Enter);
    let Mode::Links(picker) = &f.board.mode else { panic!("the picker stays open") };
    let highlighted = f.board.link_candidates(picker)[picker.rows.sel].0;
    assert_eq!(highlighted, k(2), "#2, now listed first as linked, stays highlighted");
}

#[test]
fn deleting_asks_first() {
    let mut f = Fixture::new(&["feature", "bug"]);
    f.board.select(k(1));
    f.press(KeyCode::Char('x'));
    f.press(KeyCode::Char('n'));
    assert!(f.board.brain(0).task(1).is_some());
    f.press(KeyCode::Char('x'));
    f.press(KeyCode::Char('y'));
    assert!(f.board.brain(0).task(1).is_none());
    assert_eq!(f.board.selected, Some(k(2)), "the selection moves to a remaining card");
}

#[test]
fn search_filters_cards_and_keeps_a_visible_selection() {
    let mut f = Fixture::new(&["feature", "bug"]);
    f.board.select(k(1));
    f.press(KeyCode::Char('/'));
    f.type_text("bug");
    assert_eq!(f.board.visible_ids(), [k(2)]);
    assert_eq!(f.board.selected, Some(k(2)));
    f.press(KeyCode::Enter);
    f.press(KeyCode::Esc);
    assert_eq!(f.board.visible_ids().len(), 2, "esc clears the search");
}

#[test]
fn u_undoes_the_last_change_and_again_redoes_it() {
    let mut f = Fixture::new(&["feature", "bug"]);
    f.board.brain_mut(0).link(1, &[2], &[]).unwrap();
    f.board.select(k(2));
    f.type_text("xy");
    assert!(f.board.task(k(2)).is_none(), "deleted");
    f.press(KeyCode::Down);
    f.type_text("u");
    assert_eq!(f.on_disk(2).title, "bug", "moving the selection wrote nothing, so the delete is undone");
    assert_eq!(f.on_disk(1).links, [2], "with the link to it");
    assert_eq!(f.board.message(), Some("undone; u again redoes"));
    f.type_text("u");
    assert!(f.board.task(k(2)).is_none(), "and redone");
    assert_eq!(f.board.message(), Some("redone; u again undoes"));

    f.board.select(k(1));
    f.type_text("s");
    let mut cli = kanbrain_core::brain::Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
    cli.add_comment(1, "from an agent").unwrap();
    f.type_text("u");
    assert_eq!(f.on_disk(1).status, Status::Doing, "not undone over the agent's comment");
    assert_eq!(f.board.message(), Some("can't undo: 0001-feature.md changed since"));
}

#[test]
fn selection_survives_a_reload_by_id() {
    let mut f = Fixture::new(&["feature", "bug"]);
    f.board.select(k(2));
    let mut task = f.on_disk(1);
    task.set_status(Status::Done, now(), Author::Human);
    Store::at(f.dir.path()).unwrap().save(&mut task).unwrap();
    f.board.reload().unwrap();
    assert_eq!(f.board.selected, Some(k(2)));
    assert_eq!(f.status(1), Status::Done);
}

#[test]
fn a_dialog_on_an_entry_deleted_meanwhile_closes() {
    for open in ['x', 'r', 'e', 'm'] {
        let mut f = Fixture::new(&["feature", "bug"]);
        f.board.select(k(2));
        f.press(KeyCode::Char(open));
        assert!(!matches!(f.board.mode, Mode::Normal), "{open} opens a dialog on #2");
        let store = Store::at(f.dir.path()).unwrap();
        store.delete(&f.on_disk(2)).unwrap();
        f.board.reload_changed(&all_of(&f.board, &[f.dir.path().to_path_buf()])).unwrap();
        assert!(matches!(f.board.mode, Mode::Normal), "{open}: closed, not left taking the keys unseen");
        assert_eq!(f.board.message(), Some("closed: #2 is gone, deleted or moved meanwhile"));
    }
    // Named with its repository when several are shown: "#1" alone could be any of them.
    let (central, code) = projects();
    let mut board = Board::open(central.path(), None).unwrap();
    board.select(Key::new(1, 1));
    press(&mut board, KeyCode::Char('e'));
    let api = Store::at(code.path().join("api/.brain")).unwrap();
    api.delete(&api.load_entries().unwrap().0[0]).unwrap();
    board.reload().unwrap();
    assert_eq!(board.message(), Some("closed: shop/api #1 is gone, deleted or moved meanwhile"));
}

#[test]
fn notes_have_their_own_tab_and_open_from_links() {
    let mut f = Fixture::new(&["feature"]);
    f.press(KeyCode::Char('2'));
    assert_eq!(f.board.tab, Tab::Notes);
    f.press(KeyCode::Char('n'));
    f.type_text("how deploys work");
    f.press(KeyCode::Enter);
    let note = f.board.selected.unwrap();
    f.press(KeyCode::Char('s'));
    assert!(f.board.message().unwrap().contains("only tasks have a status"));
    f.press(KeyCode::Char('r'));
    f.type_text("feature");
    f.press(KeyCode::Enter);
    f.press(KeyCode::Esc);
    assert!(f.board.brain(0).are_related(note.id, 1));
    f.board.open_entry(k(1));
    assert_eq!((f.board.tab, f.board.drawer), (Tab::Board, true));
}

#[test]
fn the_report_tab_moves_through_periods() {
    let mut f = Fixture::new(&["feature"]);
    f.press(KeyCode::Char('4'));
    let week = f.board.report.period;
    f.press(KeyCode::Left);
    assert_eq!(f.board.report.period, week.prev());
    f.press(KeyCode::Char('d'));
    assert_eq!(f.board.report.period.unit, kanbrain_core::period::Unit::Day);
    f.press(KeyCode::Char('g'));
    assert_eq!(f.board.report.group, kanbrain_core::brain::Group::Tag);
}

#[test]
fn the_calendar_filters_like_the_report_and_selects_what_it_shows() {
    use kanbrain_core::brain::Show;
    use kanbrain_core::period::Unit;
    let mut f = Fixture::new(&["feature", "bug"]);
    f.board.set_status(k(2), Status::Doing);
    f.press(KeyCode::Char('5'));
    assert_eq!(f.board.tab, Tab::Calendar);
    // Both were created today and #2 also moved and ran: in time order, #1 first.
    assert_eq!(f.board.visible_ids(), [k(1), k(2)]);
    assert_eq!(f.board.selected, Some(k(2)), "the selection stays when the calendar shows it");
    f.press(KeyCode::Up);
    f.press(KeyCode::Enter);
    assert_eq!((f.board.selected, f.board.drawer), (Some(k(1)), true));
    f.press(KeyCode::Char('i'));
    f.press(KeyCode::Char('i'));
    assert_eq!(f.board.calendar.show, Show::Moves);
    assert_eq!(f.board.visible_ids(), [k(2)], "only #2 moved");
    f.press(KeyCode::Char('m'));
    assert_eq!(f.board.calendar.period.unit, Unit::Month);
    f.press(KeyCode::Left);
    assert!(f.board.visible_ids().is_empty(), "nothing happened last month");
    assert_eq!((f.board.selected, f.board.drawer), (None, false));
    f.press(KeyCode::Char('t'));
    let today = chrono::Local::now().date_naive();
    f.board.calendar_day(today);
    assert_eq!(f.board.calendar.period.unit, Unit::Day);
    assert_eq!(f.board.visible_ids(), [k(2)]);
}

#[test]
fn moving_a_note_sideways_does_nothing() {
    let mut f = Fixture::new(&[]);
    f.board.switch_tab(Tab::Notes);
    f.board.step(Step::Right);
    assert_eq!(f.board.selected, None);
}

#[test]
fn each_tab_has_its_own_tag_and_text_filters() {
    use kanbrain_core::brain::NewEntry;
    use kanbrain_core::model::Kind as EntryKind;
    let mut f = Fixture::new(&["login bug", "export"]);
    let brain = f.board.brain_mut(0);
    brain.update(1, "login bug", vec!["auth".into()]).unwrap();
    brain.update(2, "export", vec!["csv".into()]).unwrap();
    for (title, tags) in [("sso notes", vec!["auth".into(), "sso".into()]), ("deploy notes", vec![])] {
        brain.add(NewEntry { kind: EntryKind::Note, title: title.into(), tags, ..NewEntry::default() }).unwrap();
    }
    f.board.set_status(k(1), Status::Doing);
    let titles = |board: &Board| {
        board.visible_ids().iter().map(|&key| board.task(key).unwrap().title.clone()).collect::<Vec<_>>()
    };
    let offered = |board: &Board| board.tag_candidates(&Picker::default()).into_iter().flatten().collect::<Vec<_>>();
    assert_eq!(offered(&f.board), ["auth", "csv"], "the board offers its tasks' tags");
    f.board.search = crate::text::editor::LineInput::new("#auth");
    assert_eq!(titles(&f.board), ["login bug"]);

    f.board.switch_tab(Tab::Notes);
    assert_eq!(f.board.search.text(), "", "notes have their own search");
    assert_eq!(offered(&f.board), ["auth", "sso"], "and offer their own tags: no #csv");
    assert_eq!(titles(&f.board), ["deploy notes", "sso notes"]);
    f.board.search = crate::text::editor::LineInput::new("sso");
    assert_eq!(titles(&f.board), ["sso notes"]);

    f.board.switch_tab(Tab::Board);
    assert_eq!(f.board.search.text(), "#auth", "the board's is back");
    f.board.switch_tab(Tab::Notes);
    assert_eq!(f.board.search.text(), "sso", "and so are the notes'");

    f.board.switch_tab(Tab::Calendar);
    assert_eq!(offered(&f.board), ["auth", "csv", "sso"], "the calendar shows everything");
    f.board.search = crate::text::editor::LineInput::new("export");
    assert_eq!(titles(&f.board), ["export"]);
    f.board.switch_tab(Tab::Report);
    assert_eq!(f.board.report_view().query, "", "the report's search is its own");
}

#[test]
fn picking_tags_adds_them_to_the_search_and_takes_them_out() {
    let mut f = Fixture::new(&["login bug", "export"]);
    let brain = f.board.brain_mut(0);
    brain.update(1, "login bug", vec!["auth".into(), "prod".into()]).unwrap();
    brain.update(2, "export", vec!["csv".into()]).unwrap();
    f.press(KeyCode::Char('/'));
    f.type_text("login");
    // The Tags button, clicked while typing in the search box: the dropdown stays open for several.
    f.board.press(KeyEvent::new(KeyCode::Char('#'), KeyModifiers::NONE));
    let pick = |f: &mut Fixture, tag: Option<&str>| {
        let Mode::Tags(picker) = &f.board.mode else { panic!("the dropdown stays open") };
        let index = f.board.tag_candidates(picker).iter().position(|t| t.as_deref() == tag).unwrap();
        f.board.choose_tag(index);
    };
    pick(&mut f, Some("auth"));
    assert_eq!(f.board.search.text(), "login #auth", "typed words and picked tags filter together");
    pick(&mut f, Some("prod"));
    assert_eq!(f.board.search_tags(), ["auth", "prod"], "several tags at once");
    assert_eq!(f.board.search.text(), "login #auth #prod");
    assert_eq!(f.board.column(Status::Todo).len(), 1);
    pick(&mut f, Some("auth"));
    assert_eq!(f.board.search.text(), "login #prod", "picking it again takes it out");
    pick(&mut f, None);
    assert_eq!(f.board.search.text(), "login", "all tags keeps the words");
    f.press(KeyCode::Esc);
    assert!(matches!(f.board.mode, Mode::Normal), "esc closes it");
    assert_eq!(f.board.search.text(), "login");
}

#[test]
fn a_ctrl_letter_does_nothing_but_in_the_editor() {
    let mut f = Fixture::new(&["a"]);
    let ctrl = |c| KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL);
    for c in ['d', 's', 'p', 'n', 'x', 'q'] {
        f.board.on_key(ctrl(c));
    }
    assert_eq!(f.status(1), Status::Todo, "ctrl+d, ctrl+s and ctrl+p aren't d, s and p");
    assert!(matches!(f.board.mode, Mode::Normal), "nor ctrl+n n, nor ctrl+x x");
    f.press(KeyCode::Char('m'));
    f.type_text("text");
    f.board.on_key(ctrl('d'));
    assert_eq!(f.board.message(), Some("changes discarded"), "in the editor, ctrl+d discards");
    f.board.on_key(ctrl('d'));
    assert_eq!(f.status(1), Status::Todo, "and a second one, on the board, is nothing");
}

#[test]
fn the_palette_lists_what_can_be_done_with_its_key_and_runs_it() {
    let mut f = Fixture::new(&["a"]);
    f.board.selected = None;
    f.press(KeyCode::Char('?'));
    let Mode::Palette(picker) = &f.board.mode else { panic!("? opens the palette: {:?}", f.board.mode) };
    let keys: Vec<&str> = f.board.commands(picker).iter().map(|c| c.shown).collect();
    assert!(keys.contains(&"n") && !keys.contains(&"s"), "with no card selected, a card's commands are left out");
    f.press(KeyCode::Esc);
    f.board.select(k(1));
    f.press(KeyCode::Char('?'));
    f.type_text("start");
    let Mode::Palette(picker) = &f.board.mode else { panic!("typing filters") };
    let keys: Vec<&str> = f.board.commands(picker).iter().map(|c| c.shown).collect();
    assert_eq!(keys, ["s"], "searching by what it does");
    f.press(KeyCode::Enter);
    assert_eq!(f.status(1), Status::Doing, "enter presses its key");
    assert!(matches!(f.board.mode, Mode::Normal), "and closes the palette");
    f.press(KeyCode::Char('?'));
    f.type_text("← → ↑ ↓");
    f.press(KeyCode::Enter);
    assert!(matches!(f.board.mode, Mode::Palette(_)), "a row that only tells how stays open");
    f.press(KeyCode::Esc);
    f.board.switch_tab(Tab::Report);
    f.press(KeyCode::Char('?'));
    f.type_text("export");
    let Mode::Palette(picker) = &f.board.mode else { panic!("the report has one too") };
    assert_eq!(f.board.commands(picker).first().map(|c| c.shown), Some("e"), "with the report's own commands");
}

#[test]
fn the_palette_offers_only_what_works_now() {
    let mut f = Fixture::new(&["a task"]);
    let shown = |board: &Board| -> Vec<&str> { board.commands(&Picker::default()).iter().map(|c| c.does).collect() };
    f.board.select(k(1));
    let rows = shown(&f.board);
    assert!(rows.contains(&"Start: move to Doing and run its clock"), "{rows:?}");
    for absent in ["Accept the decision", "Reject the decision", "Move to another project", "Manage project", "Update"]
    {
        assert!(!rows.iter().any(|r| r.starts_with(absent)), "{absent}: a task, one brain, no project, no release");
    }
    assert!(!rows.contains(&"Close the details") && !rows.contains(&"Clear the search"), "nothing for esc to do");
    assert_eq!(f.board.commands(&Picker::default())[0].section(), keys::Section::Entry, "the selection's first");

    f.board.available = Some("9.0.0".into());
    f.board.drawer = true;
    let rows = shown(&f.board);
    assert!(rows.iter().any(|r| r.starts_with("Update")) && rows.contains(&"Close the details"), "{rows:?}");
    assert!(rows.contains(&"Hide the details") && !rows.contains(&"Show the details"));
    f.board.drawer = false;
    f.board.search = LineInput::new("task");
    assert!(shown(&f.board).contains(&"Clear the search"));

    f.board.set_status(k(1), Status::Doing);
    assert!(!shown(&f.board).contains(&"Start: move to Doing and run its clock"), "it's started already");
    f.board.switch_tab(Tab::Notes);
    let rows = shown(&f.board);
    assert!(rows.contains(&"Move up and down the list") && !rows.contains(&"Move between cards and columns"));
}

#[test]
fn the_details_buttons_act_on_the_entry_from_the_calendar_too() {
    let mut f = Fixture::new(&["a task"]);
    f.board.switch_tab(Tab::Calendar);
    f.board.select(k(1));
    f.board.drawer = true;
    let period = f.board.calendar.period;
    f.board.run_on_entry(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE));
    assert!(matches!(f.board.mode, Mode::Edit(_)), "the description's Edit, not a month: {:?}", f.board.mode);
    assert_eq!(f.board.calendar.period, period);
    f.press(KeyCode::Esc);
    f.board.run_on_entry(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE));
    assert!(matches!(f.board.mode, Mode::Form(_)), "Edit title and tags");
    f.press(KeyCode::Esc);
    f.board.set_status(k(1), Status::Doing);
    f.board.press(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE));
    assert_eq!(f.status(1), Status::Doing, "the calendar's Now button is still its own, with the details open");
    f.press(KeyCode::Char('m'));
    assert_eq!(f.board.calendar.period.unit, Unit::Month, "and so are its keys");
}

#[test]
fn a_buttons_shortcut_is_named_only_on_the_tabs_its_key_does_the_same() {
    let mut f = Fixture::new(&[]);
    assert_eq!(f.board.shortcut(When::Waiting), Some("w"));
    assert_eq!(f.board.shortcut(When::MoveTo(Status::Done)), Some("d"));
    for tab in [Tab::Report, Tab::Calendar] {
        f.board.switch_tab(tab);
        assert_eq!(f.board.shortcut(When::Waiting), None, "w picks a week there");
        assert_eq!(f.board.shortcut(When::MoveTo(Status::Done)), None, "d picks a day there");
    }
}

#[test]
fn the_done_column_shows_the_latest_until_asked_for_all() {
    let titles: Vec<String> = (1..=DONE_LIMIT + 3).map(|n| format!("task {n}")).collect();
    let mut f = Fixture::new(&titles.iter().map(String::as_str).collect::<Vec<_>>());
    for id in 1..=u32::try_from(DONE_LIMIT + 3).unwrap() {
        f.board.set_status(k(id), Status::Done);
    }
    assert_eq!((f.board.column(Status::Done).len(), f.board.hidden_done()), (DONE_LIMIT, 3));
    assert_eq!(f.board.column(Status::Done)[0].0, k(23), "the latest first");
    f.press(KeyCode::Char('f'));
    assert_eq!((f.board.column(Status::Done).len(), f.board.hidden_done()), (DONE_LIMIT + 3, 0));
    f.press(KeyCode::Char('f'));
    assert_eq!(f.board.hidden_done(), 3, "f again hides them");
}

#[test]
fn opening_waits_for_a_brain_another_program_is_writing() {
    let (central, code) = projects();
    let api = code.path().join("api/.brain");
    let (locked, release) = (std::sync::mpsc::channel(), std::sync::mpsc::channel::<()>());
    // As an agent's `kb` holding api's lock while it writes: on its own thread, as the lock is per thread.
    let writer = std::thread::spawn(move || {
        let _lock = Store::at(&api).unwrap().lock().unwrap();
        locked.0.send(()).unwrap();
        release.1.recv().unwrap();
    });
    locked.1.recv().unwrap();
    let path = central.path().to_path_buf();
    let opening = std::thread::spawn(move || Board::open(&path, None).map(|b| b.sources.len()));
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert!(!opening.is_finished(), "it waits rather than repair files under the writer");
    release.0.send(()).unwrap();
    writer.join().unwrap();
    assert_eq!(opening.join().unwrap().unwrap(), 4);
}

#[test]
fn the_boards_own_writes_never_read_as_changes() {
    let mut f = Fixture::new(&["feature", "bug"]);
    let mut watcher = crate::watch::Watcher::new(f.board.watched_dirs());
    watcher.changed();
    f.board.select(k(1));
    f.press(KeyCode::Char('s'));
    assert_eq!(f.status(1), Status::Doing);
    watcher.sync(f.board.watched_dirs(), |file| f.board.known(file));
    assert!(watcher.changed().is_empty(), "the start it saved, known as written");
}
