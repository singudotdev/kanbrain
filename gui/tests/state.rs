//! Behaviour tests: drive the board with key presses over a temporary data folder.

use super::nav::Step;
use super::*;
use crate::text::event::{KeyCode, KeyEvent, KeyModifiers};
use kanbrain_core::model::{Asked, Author, DecisionState, now};
use kanbrain_core::store::Store;
use tempfile::TempDir;

/// An entry of the central brain, where these tests' entries are.
fn k(id: u32) -> Key {
    Key::new(0, id)
}

/// Presses `code` on the board, with no modifier.
fn press(board: &mut Board, code: KeyCode) {
    board.on_key(KeyEvent::new(code, KeyModifiers::NONE));
}

/// A board over a fresh folder holding tasks with these titles (ids from 1); the folder lives as long
/// as the fixture.
struct Fixture {
    board: Board,
    dir: TempDir,
}

impl Fixture {
    fn new(titles: &[&str]) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::at(dir.path()).unwrap();
        for (title, id) in titles.iter().zip(1..) {
            store.save(&mut Task::new(id, (*title).to_string(), now())).unwrap();
        }
        let board = Board::open(dir.path(), None).unwrap();
        Self { board, dir }
    }

    fn press(&mut self, code: KeyCode) {
        press(&mut self.board, code);
    }

    fn shift(&mut self, code: KeyCode) {
        self.board.on_key(KeyEvent::new(code, KeyModifiers::SHIFT));
    }

    fn type_text(&mut self, text: &str) {
        text.chars().for_each(|c| self.press(KeyCode::Char(c)));
    }

    fn status(&self, id: u32) -> Status {
        self.board.brain(0).task(id).unwrap().status
    }

    /// The task as its file says, independent of the board's memory.
    fn on_disk(&self, id: u32) -> Task {
        let (tasks, _) = Store::at(self.dir.path()).unwrap().load().unwrap();
        tasks.into_iter().find(|t| t.id == id).unwrap()
    }
}

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
fn the_form_picks_a_new_tasks_column() {
    let mut f = Fixture::new(&["a"]);
    f.board.new_entry(Status::Parked);
    let Mode::Form(form) = &f.board.mode else { panic!("a column's + opens the form") };
    assert_eq!(form.status(), Some(Status::Parked), "with that column picked");
    f.type_text("picked");
    for _ in 0..3 {
        f.press(KeyCode::Tab);
    }
    let Mode::Form(form) = &f.board.mode else { panic!("the form is open") };
    assert_eq!(form.field, Field::Status, "title, tags, related, then the column");
    f.press(KeyCode::Right);
    f.press(KeyCode::Right);
    f.press(KeyCode::Right);
    let Mode::Form(form) = &f.board.mode else { panic!("the form is open") };
    assert_eq!(form.status(), Some(Status::Done), "right goes on to Done, and stops there");
    f.press(KeyCode::Left);
    f.press(KeyCode::Enter);
    assert_eq!(f.status(2), Status::Parked);
    // Doing, while one of yours runs: it stays in To do, and the status bar says why.
    f.board.set_status(k(1), Status::Doing);
    f.board.new_entry(Status::Doing);
    f.type_text("second");
    f.press(KeyCode::Enter);
    assert_eq!(f.status(3), Status::Todo);
    let said = f.board.message().unwrap();
    assert!(said.starts_with("created #3, in To do: #1 is in doing"), "{said}");
    f.board.switch_tab(Tab::Notes);
    f.press(KeyCode::Char('n'));
    let Mode::Form(form) = &f.board.mode else { panic!("n opens a note's form") };
    assert_eq!(form.status(), None, "notes have no column");
    assert_eq!(form.previous_field(), Field::Links, "so tab skips it");
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
    assert_eq!(f.board.running().len(), 2, "yours and the agent's both run");
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
fn the_form_creates_a_tagged_task_and_edits_it() {
    let mut f = Fixture::new(&[]);
    f.press(KeyCode::Char('n'));
    f.type_text("Fix login");
    f.press(KeyCode::Tab);
    f.type_text("prod, #Auth");
    f.press(KeyCode::Enter);
    let task = f.board.selected_task().unwrap();
    assert_eq!((task.title.as_str(), task.tags.clone()), ("Fix login", vec!["prod".to_string(), "auth".to_string()]));
    f.press(KeyCode::Char('e'));
    (0..5).for_each(|_| f.press(KeyCode::Backspace));
    f.type_text("SSO");
    f.press(KeyCode::Enter);
    assert_eq!(f.on_disk(1).title, "Fix SSO");
    assert_eq!(f.board.message(), Some("#1 saved"));
    let written = std::fs::read_dir(f.dir.path()).unwrap().map(|e| e.unwrap().metadata().unwrap().modified().unwrap());
    let written: Vec<_> = written.collect();
    f.board.message = None;
    f.press(KeyCode::Char('e'));
    f.press(KeyCode::Enter);
    assert!(matches!(f.board.mode, Mode::Normal));
    assert_eq!(f.board.message(), None, "saved as it was: nothing said");
    let again = std::fs::read_dir(f.dir.path()).unwrap().map(|e| e.unwrap().metadata().unwrap().modified().unwrap());
    assert_eq!(again.collect::<Vec<_>>(), written, "nor written");
    // Tags in another order are another set of tags to show: saved.
    f.press(KeyCode::Char('e'));
    f.press(KeyCode::Tab);
    f.board.untag_in_form("prod");
    f.board.untag_in_form("auth");
    f.type_text("auth, prod");
    f.press(KeyCode::Enter);
    assert_eq!(f.on_disk(1).tags, ["auth", "prod"]);

    f.press(KeyCode::Char('n'));
    f.press(KeyCode::Enter);
    assert!(matches!(&f.board.mode, Mode::Form(form) if form.title_missed), "said under the field, kept open");
    f.press(KeyCode::Esc);

    // Tab into the title selects its text, so typing replaces it; shift+tab goes back.
    f.press(KeyCode::Char('e'));
    f.press(KeyCode::Tab);
    f.board.untag_in_form("auth");
    f.board.untag_in_form("prod");
    f.type_text("ops");
    f.press(KeyCode::BackTab);
    f.type_text("Fix SAML");
    f.press(KeyCode::Enter);
    let task = f.on_disk(1);
    assert_eq!((task.title.as_str(), task.tags.as_slice()), ("Fix SAML", &["ops".to_string()][..]));
}

#[test]
fn comments_and_descriptions_are_written_from_the_editor() {
    let mut f = Fixture::new(&["feature"]);
    f.press(KeyCode::Char('c'));
    f.type_text("found it");
    f.press(KeyCode::Esc);
    f.press(KeyCode::Char('m'));
    f.type_text("notes");
    f.board.discard_edit();
    f.press(KeyCode::Char('m'));
    f.type_text("more notes");
    f.board.on_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL));
    assert!(matches!(f.board.mode, Mode::Normal), "ctrl+d discards too");
    f.press(KeyCode::Char('m'));
    f.type_text("x");
    f.press(KeyCode::Backspace);
    assert!(matches!(&f.board.mode, Mode::Edit(edit) if !f.board.changed(edit)), "no • in its title, Close not Save");
    f.press(KeyCode::Esc);
    assert_eq!(f.board.message(), Some("changes discarded"), "typed and put back: nothing said to be saved");
    let task = f.on_disk(1);
    assert_eq!(task.comments[0].body, "found it");
    assert_eq!(task.description, "", "discarded");
}

#[test]
fn a_save_that_stops_over_what_kb_wrote_says_it_once() {
    use kanbrain_core::brain::{Brain, NewEntry};
    let mut f = Fixture::new(&["feature"]);
    let kb = |f: &Fixture| Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
    // Saved within a second of the read that saw it: the stop says what that read said, and more.
    f.press(KeyCode::Char('m'));
    f.type_text("mine");
    kb(&f).set_description(1, "the agent's plan").unwrap();
    f.board.reload().unwrap();
    assert_eq!(f.board.message(), Some("#1's description changed on disk"));
    f.press(KeyCode::Esc);
    let stop = "#1's description changed meanwhile: pick yours or theirs where you both changed it";
    assert_eq!(f.board.message(), Some(stop));
    f.board.discard_edit();

    // What else the read saw is still said, and only that.
    f.press(KeyCode::Char('m'));
    f.type_text("mine");
    let mut brain = kb(&f);
    brain.set_description(1, "the agent's next plan").unwrap();
    brain.set_status(1, Status::Parked).unwrap();
    f.board.reload().unwrap();
    f.press(KeyCode::Esc);
    assert_eq!(f.board.message(), Some(format!("{stop} · #1's status changed on disk").as_str()));
    f.board.discard_edit();

    // The form's title, the same.
    f.press(KeyCode::Char('e'));
    f.type_text(" now");
    kb(&f).update(1, "Fix SSO", vec![]).unwrap();
    f.board.reload().unwrap();
    assert_eq!(f.board.message(), Some("#1's title changed on disk"));
    f.press(KeyCode::Enter);
    let renamed = "#1's title changed meanwhile, to “Fix SSO”: save again to keep yours, ctrl+t to take it";
    assert_eq!(f.board.message(), Some(renamed));
    f.press(KeyCode::Esc);
    f.press(KeyCode::Char('e'));
    f.type_text(" again");
    kb(&f).update(1, "Fix SSO", vec!["auth".into()]).unwrap();
    kb(&f).update(1, "Fix SSO 500", vec!["auth".into()]).unwrap();
    f.board.reload().unwrap();
    assert_eq!(f.board.message(), Some("#1's title and tags changed on disk"));
    f.press(KeyCode::Enter);
    assert_eq!(
        f.board.message(),
        Some(
            "#1's title changed meanwhile, to “Fix SSO 500”: save again to keep yours, ctrl+t to take it · #1's tags \
             changed on disk"
        )
    );
    f.press(KeyCode::Esc);

    // Another comment put at the one being edited's place.
    kb(&f).add_comment(1, "first").unwrap();
    kb(&f).add_comment(1, "second").unwrap();
    f.board.reload().unwrap();
    f.board.open_editor(Target::Comment(k(1), 1));
    f.type_text(", edited");
    kb(&f).delete_comment(1, 0).unwrap();
    f.board.reload().unwrap();
    assert_eq!(f.board.message(), Some("#1's comment changed on disk"));
    f.press(KeyCode::Esc);
    assert_eq!(f.board.message(), Some("#1's comments changed around this one meanwhile: copy your text, then ctrl+d"));
    f.board.discard_edit();

    // A verdict given meanwhile.
    let decision = kb(&f).propose(1, NewEntry { title: "Use Redis".into(), ..Default::default() }).unwrap();
    f.board.reload().unwrap();
    f.board.select(k(decision));
    f.press(KeyCode::Char('j'));
    f.type_text("too costly");
    kb(&f).resolve(decision, DecisionState::Accepted, "").unwrap();
    f.board.reload().unwrap();
    assert_eq!(f.board.message(), Some(format!("#{decision}'s verdict changed on disk").as_str()));
    f.press(KeyCode::Esc);
    assert_eq!(
        f.board.message(),
        Some(
            format!("#{decision} was accepted meanwhile: save again to reject it anyway, ctrl+d to leave it").as_str()
        )
    );
}

#[test]
fn the_editor_doesnt_replace_unseen_what_kb_wrote_while_it_was_open() {
    use kanbrain_core::brain::Brain;
    let mut f = Fixture::new(&["feature"]);
    let kb = |f: &Fixture| Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
    let saving = |f: &mut Fixture| {
        f.board.message = None;
        f.press(KeyCode::Esc);
    };

    // An agent describes the task while the user does: the user's save stops and says so, the editor still open.
    f.press(KeyCode::Char('m'));
    f.type_text("mine");
    // enter is a new line, not a save (ctrl+enter is).
    f.press(KeyCode::Enter);
    assert!(matches!(f.board.mode, Mode::Edit(_)));
    f.press(KeyCode::Backspace);
    kb(&f).set_description(1, "the agent's plan").unwrap();
    f.board.reload().unwrap();
    saving(&mut f);
    let merging = |f: &Fixture| matches!(&f.board.mode, Mode::Edit(edit) if edit.merge.is_some());
    assert!(merging(&f), "kept open, the lines both changed to pick from");
    assert_eq!(
        f.board.message(),
        Some("#1's description changed meanwhile: pick yours or theirs where you both changed it")
    );
    assert_eq!(f.on_disk(1).description, "the agent's plan", "not replaced unseen");
    // esc takes yours where nothing's picked, back in the editor.
    saving(&mut f);
    assert!(matches!(&f.board.mode, Mode::Edit(edit) if edit.merge.is_none() && edit.editor.text() == "mine"));
    assert_eq!(f.board.message(), Some("merged: esc saves it"));

    // Changed again before the second save: stopped again, still not replaced.
    kb(&f).set_description(1, "the agent's new plan").unwrap();
    f.board.reload().unwrap();
    saving(&mut f);
    assert!(merging(&f));
    assert_eq!(f.on_disk(1).description, "the agent's new plan");
    saving(&mut f);

    // Saved again with nothing new on disk: the user's replaces it.
    saving(&mut f);
    assert!(matches!(f.board.mode, Mode::Normal));
    assert_eq!(f.board.message(), Some("description saved"));
    assert_eq!(f.on_disk(1).description, "mine");

    // ctrl+d after being told keeps the agent's.
    f.press(KeyCode::Char('m'));
    f.type_text(" too");
    kb(&f).set_description(1, "theirs").unwrap();
    f.board.reload().unwrap();
    saving(&mut f);
    f.board.on_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL));
    assert!(matches!(f.board.mode, Mode::Normal));
    assert_eq!(f.on_disk(1).description, "theirs");

    // Typed and put back is nothing of the user's: what the agent wrote meanwhile stays, and the editor says Close.
    f.press(KeyCode::Char('m'));
    f.type_text("x");
    f.press(KeyCode::Backspace);
    kb(&f).set_description(1, "theirs, again").unwrap();
    f.board.reload().unwrap();
    assert!(matches!(&f.board.mode, Mode::Edit(edit) if !f.board.changed(edit)), "Close, not Save");
    saving(&mut f);
    assert!(matches!(f.board.mode, Mode::Normal));
    assert_eq!(f.on_disk(1).description, "theirs, again");
    kb(&f).set_description(1, "theirs").unwrap();
    f.board.reload().unwrap();

    // Unchanged on disk, a save is a save, as before.
    f.press(KeyCode::Char('m'));
    f.type_text(" and more");
    saving(&mut f);
    assert_eq!(f.on_disk(1).description, "theirs and more");

    // A comment the agent edited meanwhile, the same.
    kb(&f).add_comment(1, "first").unwrap();
    f.board.reload().unwrap();
    f.board.open_editor(Target::Comment(k(1), 0));
    f.type_text(", mine");
    kb(&f).update_comment(1, 0, "first, the agent's").unwrap();
    f.board.reload().unwrap();
    saving(&mut f);
    assert_eq!(
        f.board.message(),
        Some("#1's comment changed meanwhile: pick yours or theirs where you both changed it")
    );
    assert_eq!(f.on_disk(1).comments[0].body, "first, the agent's");
    saving(&mut f);
    saving(&mut f);
    assert_eq!(f.on_disk(1).comments[0].body, "first, mine");

    // A new comment adds to what's there: nothing to replace, so nothing to stop for.
    f.press(KeyCode::Char('c'));
    f.type_text("second");
    kb(&f).add_comment(1, "the agent's").unwrap();
    f.board.reload().unwrap();
    saving(&mut f);
    assert!(matches!(f.board.mode, Mode::Normal));
    let bodies: Vec<_> = f.on_disk(1).comments.into_iter().map(|c| c.body).collect();
    assert_eq!(bodies, ["first, mine", "the agent's", "second"]);
}

#[test]
fn the_editor_merges_lines_only_one_side_changed() {
    use kanbrain_core::brain::Brain;
    let mut f = Fixture::new(&["feature"]);
    let kb = |f: &Fixture| Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
    kb(&f).set_description(1, "Plan\n\n- one\n- two\n- three").unwrap();
    f.board.reload().unwrap();
    let typing = |f: &mut Fixture, text: &str| {
        f.board.select_all();
        f.board.paste(text);
    };

    // The user's line and the agent's, both kept, and said so.
    f.press(KeyCode::Char('m'));
    typing(&mut f, "Plan, mine\n\n- one\n- two\n- three");
    kb(&f).set_description(1, "Plan\n\n- one\n- three\n- four").unwrap();
    f.board.reload().unwrap();
    f.press(KeyCode::Esc);
    assert!(matches!(f.board.mode, Mode::Normal));
    assert_eq!(f.board.message(), Some("description saved, merged with the change made meanwhile"));
    assert_eq!(f.on_disk(1).description, "Plan, mine\n\n- one\n- three\n- four");

    // The agent made the same change, and another: nothing to write.
    f.press(KeyCode::Char('m'));
    typing(&mut f, "Plan, mine\n\n- one\n- three\n- five");
    kb(&f).set_description(1, "Plan, done\n\n- one\n- three\n- five").unwrap();
    f.board.reload().unwrap();
    f.press(KeyCode::Esc);
    assert!(matches!(f.board.mode, Mode::Normal));
    assert_eq!(f.board.message(), Some("nothing to save: #1's description has your changes already"));

    // Told to the agent working on it, the save says it merged.
    let mut agent = kb(&f);
    agent.author = Author::Agent;
    agent.set_status(1, Status::Doing).unwrap();
    f.board.reload().unwrap();
    f.press(KeyCode::Char('m'));
    typing(&mut f, "Plan\n\n- one\n- three\n- five");
    agent.set_description(1, "Plan, done\n\n- one\n- three\n- five, done").unwrap();
    f.board.reload().unwrap();
    f.press(KeyCode::Esc);
    assert!(matches!(f.board.mode, Mode::Tell(_)));
    f.press(KeyCode::Enter);
    assert_eq!(f.board.message(), Some("description saved, merged with the change made meanwhile; the agent is told"));
    assert_eq!(f.on_disk(1).description, "Plan\n\n- one\n- three\n- five, done");
}

#[test]
fn lines_both_changed_are_picked_in_place_of_the_text() {
    use crate::text::merge::Pick;
    use kanbrain_core::brain::Brain;
    let mut f = Fixture::new(&["feature"]);
    let kb = |f: &Fixture| Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
    kb(&f).set_description(1, "a\nb\nc\nd\ne").unwrap();
    f.board.reload().unwrap();
    f.press(KeyCode::Char('m'));
    f.board.select_all();
    f.board.paste("A\nb\nc\nd\nE");
    kb(&f).set_description(1, "a2\nb\nc\nd\ne2").unwrap();
    f.board.reload().unwrap();
    f.press(KeyCode::Esc);
    let at = |f: &Fixture| match &f.board.mode {
        Mode::Edit(Edit { merge: Some(merge), .. }) => merge.at,
        _ => panic!("picking"),
    };
    assert_eq!(at(&f), 0);

    // The arrows and tab move between conflicts; other keys type nothing.
    f.press(KeyCode::Down);
    assert_eq!(at(&f), 1);
    f.press(KeyCode::Up);
    assert_eq!(at(&f), 0);
    f.press(KeyCode::Tab);
    assert_eq!(at(&f), 1);
    f.shift(KeyCode::BackTab);
    assert_eq!(at(&f), 0);
    f.press(KeyCode::Char('x'));
    assert_eq!(at(&f), 0);

    // Each pick moves on; enter waits for the last.
    f.press(KeyCode::Char('t'));
    assert_eq!(at(&f), 1);
    f.press(KeyCode::Enter);
    assert_eq!(f.board.message(), Some("1 conflict to pick yet: yours (m), theirs (t) or both (b)"));
    assert_eq!(at(&f), 1);
    f.press(KeyCode::Char('b'));
    // A click on a side picks it.
    f.board.pick_conflict(0, Pick::Mine);
    f.press(KeyCode::Enter);
    assert!(
        matches!(&f.board.mode, Mode::Edit(edit) if edit.merge.is_none() && edit.editor.text() == "A\nb\nc\nd\nE\ne2")
    );
    assert_eq!(f.board.message(), Some("merged: esc saves it"));
    assert!(matches!(&f.board.mode, Mode::Edit(edit) if f.board.changed(edit)), "Save, not Close");
    assert_eq!(f.on_disk(1).description, "a2\nb\nc\nd\ne2", "not saved yet");
    f.press(KeyCode::Esc);
    assert_eq!(f.board.message(), Some("description saved"));
    assert_eq!(f.on_disk(1).description, "A\nb\nc\nd\nE\ne2");

    // Picking mine everywhere, then theirs: the same.
    f.press(KeyCode::Char('m'));
    f.board.select_all();
    f.board.paste("A!\nb");
    kb(&f).set_description(1, "a3\nb\nc\nd\nE\ne3").unwrap();
    f.board.reload().unwrap();
    f.press(KeyCode::Esc);
    f.press(KeyCode::Char('m'));
    f.press(KeyCode::Char('m'));
    f.board.use_merge();
    f.press(KeyCode::Esc);
    assert_eq!(f.on_disk(1).description, "A!\nb");
    // Nothing types into the text the conflicts hide: a paste, a cut, a select-all.
    f.press(KeyCode::Char('m'));
    f.board.select_all();
    f.board.paste("mine\nb");
    kb(&f).set_description(1, "theirs\nb").unwrap();
    f.board.reload().unwrap();
    f.press(KeyCode::Esc);
    f.board.select_all();
    f.board.paste("pasted");
    assert_eq!(f.board.selected_text(), None);
    assert_eq!(f.board.cut(), None);
    assert!(!f.board.is_typing(), "no cursor");
    // All theirs is the text as it is: nothing to save, and said so.
    f.press(KeyCode::Char('t'));
    f.press(KeyCode::Enter);
    assert!(matches!(&f.board.mode, Mode::Edit(edit) if edit.editor.text() == "theirs\nb" && !f.board.changed(edit)));
    assert_eq!(f.board.message(), Some("merged: it's theirs as it is, esc closes"));
    f.press(KeyCode::Esc);
    assert_eq!(f.on_disk(1).description, "theirs\nb");
    // ctrl+d while picking discards it all.
    f.press(KeyCode::Char('m'));
    f.board.select_all();
    f.board.paste("mine");
    kb(&f).set_description(1, "theirs").unwrap();
    f.board.reload().unwrap();
    f.press(KeyCode::Esc);
    f.board.on_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL));
    assert!(matches!(f.board.mode, Mode::Normal));
    assert_eq!(f.on_disk(1).description, "theirs");
}

#[test]
fn yours_for_the_rest_keeps_the_lines_only_kb_changed() {
    use kanbrain_core::brain::Brain;
    let mut f = Fixture::new(&["feature"]);
    let kb = |f: &Fixture| Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
    kb(&f).set_description(1, "a\nb\nc\nd\ne").unwrap();
    f.board.reload().unwrap();
    f.press(KeyCode::Char('m'));
    f.board.select_all();
    f.board.paste("A\nb\nc\nd\nE");
    kb(&f).set_description(1, "a2\nb\nc, theirs\nd\ne2").unwrap();
    f.board.reload().unwrap();
    f.press(KeyCode::Esc);
    // Picked in either case; ctrl+a letter picks nothing.
    f.board.on_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL));
    assert!(
        matches!(&f.board.mode, Mode::Edit(Edit { merge: Some(m), .. }) if m.conflicts().all(|c| c.pick.is_none()))
    );
    f.shift(KeyCode::Char('T'));
    assert!(matches!(&f.board.mode, Mode::Edit(Edit { merge: Some(m), .. }) if m.at == 1));
    f.press(KeyCode::Esc);
    assert!(matches!(&f.board.mode, Mode::Edit(edit) if edit.editor.text() == "a2\nb\nc, theirs\nd\nE"));
    f.press(KeyCode::Esc);
    assert_eq!(f.on_disk(1).description, "a2\nb\nc, theirs\nd\nE", "theirs, and the line only kb changed");
}

#[test]
fn conflicts_kb_changes_again_while_picked_are_merged_again() {
    use kanbrain_core::brain::Brain;
    let mut f = Fixture::new(&["feature"]);
    let kb = |f: &Fixture| Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
    kb(&f).set_description(1, "a\nb\nc\nd\ne").unwrap();
    f.board.reload().unwrap();
    f.press(KeyCode::Char('m'));
    f.board.select_all();
    f.board.paste("A\nb\nc\nd\nE");
    kb(&f).set_description(1, "a2\nb\nc\nd\ne2").unwrap();
    f.board.reload().unwrap();
    f.press(KeyCode::Esc);
    f.press(KeyCode::Char('t'));
    // Changed again: the conflicts are of the new text, picked anew.
    kb(&f).set_description(1, "a3\nb\nc\nd\ne2").unwrap();
    f.board.message = None;
    f.board.reload().unwrap();
    assert_eq!(f.board.message(), Some("#1's description changed again: pick again where you both changed it"));
    let Mode::Edit(Edit { merge: Some(merge), .. }) = &f.board.mode else { panic!("still picking") };
    assert_eq!(merge.theirs, "a3\nb\nc\nd\ne2");
    assert!(merge.conflicts().all(|c| c.pick.is_none()));
    // Read again with nothing new: as it was.
    f.press(KeyCode::Char('t'));
    f.board.message = None;
    f.board.reload().unwrap();
    assert_eq!(f.board.message(), None);
    // Changed again to lines the user didn't: it all merges, back in the editor.
    kb(&f).set_description(1, "a\nb\nc, theirs\nd\ne").unwrap();
    f.board.reload().unwrap();
    assert_eq!(f.board.message(), Some("#1's description changed again: it all merges now, esc saves it"));
    assert!(
        matches!(&f.board.mode, Mode::Edit(edit) if edit.merge.is_none() && edit.editor.text() == "A\nb\nc, theirs\nd\nE")
    );
    // What a save merges with is that text now: taken back, it's taken back in the editor's too.
    kb(&f).set_description(1, "a\nb\nc\nd\ne").unwrap();
    f.board.reload().unwrap();
    f.press(KeyCode::Esc);
    assert_eq!(f.on_disk(1).description, "A\nb\nc\nd\nE");
}

#[test]
fn conflicts_kb_changes_again_to_the_text_typed_leave_nothing_to_save() {
    use kanbrain_core::brain::Brain;
    let mut f = Fixture::new(&["feature"]);
    let kb = |f: &Fixture| Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
    kb(&f).set_description(1, "a\nb").unwrap();
    f.board.reload().unwrap();
    f.press(KeyCode::Char('m'));
    f.board.select_all();
    f.board.paste("A\nb");
    kb(&f).set_description(1, "a2\nb").unwrap();
    f.board.reload().unwrap();
    f.press(KeyCode::Esc);
    kb(&f).set_description(1, "A\nb").unwrap();
    f.board.reload().unwrap();
    assert_eq!(f.board.message(), Some("#1's description changed again: it's theirs as it is now, esc closes"));
    f.press(KeyCode::Esc);
    assert!(matches!(f.board.mode, Mode::Normal), "closed: nothing to save");
}

#[test]
fn conflicts_of_a_comment_kb_moved_while_picked_give_back_the_text_typed() {
    use kanbrain_core::brain::Brain;
    let mut f = Fixture::new(&["feature"]);
    let kb = |f: &Fixture| Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
    for body in ["zero", "x\nq", "other"] {
        kb(&f).add_comment(1, body).unwrap();
    }
    f.board.reload().unwrap();
    f.board.open_editor(Target::Comment(k(1), 1));
    f.board.select_all();
    f.board.paste("x, mine\nq\nmy addition");
    kb(&f).update_comment(1, 1, "x, theirs\nq").unwrap();
    f.board.reload().unwrap();
    f.press(KeyCode::Esc);
    assert!(matches!(&f.board.mode, Mode::Edit(edit) if edit.merge.is_some()));
    // The comment before it deleted: its index holds another, which isn't merged with.
    kb(&f).delete_comment(1, 0).unwrap();
    f.board.reload().unwrap();
    let said = f.board.message().unwrap_or_default();
    assert!(said.ends_with("#1's comments changed around this one meanwhile: copy your text, then ctrl+d"), "{said}");
    let typed = "x, mine\nq\nmy addition";
    assert!(matches!(&f.board.mode, Mode::Edit(edit) if edit.merge.is_none() && edit.editor.text() == typed));
    f.press(KeyCode::Char('t'));
    f.press(KeyCode::Esc);
    assert!(matches!(&f.board.mode, Mode::Edit(_)), "the save stops for good");
    let comments: Vec<_> = f.on_disk(1).comments.into_iter().map(|c| c.body).collect();
    assert_eq!(comments, ["x, theirs\nq", "other"]);
}

#[test]
fn a_title_kb_changes_again_after_a_stopped_save_is_the_one_saving_keeps_yours_over() {
    use kanbrain_core::brain::Brain;
    let mut f = Fixture::new(&["feature"]);
    let kb = |f: &Fixture| Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
    f.board.select(k(1));
    f.press(KeyCode::Char('e'));
    f.type_text(" mine");
    kb(&f).update(1, "T1", vec![]).unwrap();
    f.board.reload().unwrap();
    f.press(KeyCode::Enter);
    kb(&f).update(1, "T2", vec![]).unwrap();
    f.board.reload().unwrap();
    assert!(matches!(&f.board.mode, Mode::Form(form) if form.theirs_title.as_deref() == Some("T2")));
    f.press(KeyCode::Enter);
    assert!(matches!(f.board.mode, Mode::Normal), "saved, as the form says");
    assert_eq!(f.on_disk(1).title, "feature mine");
}

#[test]
fn a_save_that_stops_after_telling_the_agent_asks_again_with_the_note() {
    use kanbrain_core::brain::Brain;
    let mut f = Fixture::new(&["feature"]);
    let mut agent = Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
    agent.author = Author::Agent;
    agent.set_status(1, Status::Doing).unwrap();
    agent.set_description(1, "plan").unwrap();
    f.board.reload().unwrap();
    f.press(KeyCode::Char('m'));
    f.board.select_all();
    f.board.paste("my plan");
    f.press(KeyCode::Esc);
    f.type_text("use mine");
    // The agent writes before the answer: the save stops over it.
    agent.set_description(1, "the agent's plan").unwrap();
    f.board.reload().unwrap();
    f.press(KeyCode::Enter);
    assert!(matches!(&f.board.mode, Mode::Edit(edit) if edit.merge.is_some()));
    f.press(KeyCode::Char('m'));
    f.press(KeyCode::Enter);
    f.press(KeyCode::Esc);
    assert!(matches!(&f.board.mode, Mode::Tell(tell) if tell.note.text() == "use mine"), "asked again, with the note");
    // The form's title, the same.
    f.press(KeyCode::Enter);
    assert_eq!(f.on_disk(1).description, "my plan");
    f.press(KeyCode::Char('e'));
    f.press(KeyCode::End);
    f.type_text("!");
    f.press(KeyCode::Enter);
    f.type_text("renamed");
    agent.update(1, "feature, the agent's", vec![]).unwrap();
    f.board.reload().unwrap();
    f.press(KeyCode::Enter);
    assert!(matches!(&f.board.mode, Mode::Form(form) if form.theirs_title.is_some()));
    f.press(KeyCode::Enter);
    assert!(matches!(&f.board.mode, Mode::Tell(tell) if tell.note.text() == "renamed"));
}

#[test]
fn an_open_dialog_says_until_closed_that_kb_changed_what_it_edits() {
    use kanbrain_core::brain::Brain;
    use kanbrain_core::model::Comment;
    let mut f = Fixture::new(&["Fix login", "Other"]);
    let kb = |f: &Fixture| Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
    let reload = |f: &mut Fixture| {
        f.board.message = None;
        f.board.reload().unwrap();
    };
    let marked = |f: &Fixture| match &f.board.mode {
        Mode::Edit(edit) => edit.changed_on_disk,
        Mode::Form(form) => form.changed_on_disk,
        _ => panic!("the dialog closed"),
    };

    // The description editor: a comment, or another entry changed, isn't what it edits.
    f.board.select(k(1));
    f.press(KeyCode::Char('m'));
    kb(&f).add_comment(1, "on it").unwrap();
    kb(&f).set_description(2, "not this one").unwrap();
    reload(&mut f);
    assert!(!marked(&f));
    assert_eq!(f.board.message(), None);
    kb(&f).set_description(1, "the agent's plan").unwrap();
    reload(&mut f);
    assert!(marked(&f));
    assert_eq!(f.board.message(), Some("#1's description changed on disk"));
    // Said once, though it changes again; marked until closed, even once it's back as it was.
    kb(&f).set_description(1, "the agent's newer plan").unwrap();
    reload(&mut f);
    assert_eq!(f.board.message(), None);
    kb(&f).set_description(1, "").unwrap();
    reload(&mut f);
    assert!(marked(&f));
    assert_eq!(f.board.message(), None);
    f.press(KeyCode::Esc);
    f.press(KeyCode::Char('m'));
    assert!(!marked(&f), "a dialog opened again starts unmarked");

    // The task's status counts in any of its editors: the agent may have finished it.
    f.press(KeyCode::Esc);
    f.board.open_editor(Target::NewComment(k(1)));
    kb(&f).set_status(1, Status::Doing).unwrap();
    reload(&mut f);
    assert!(marked(&f));
    assert_eq!(f.board.message(), Some("#1's status changed on disk"));
    f.press(KeyCode::Esc);

    // A comment's editor: that comment's text, not another's.
    kb(&f).add_comment(1, "second").unwrap();
    f.board.reload().unwrap();
    f.board.open_editor(Target::Comment(k(1), 0));
    kb(&f).update_comment(1, 1, "second, edited").unwrap();
    reload(&mut f);
    assert!(!marked(&f));
    kb(&f).update_comment(1, 0, "on it, edited").unwrap();
    reload(&mut f);
    assert!(marked(&f));
    assert_eq!(f.board.message(), Some("#1's comment changed on disk"));
    f.press(KeyCode::Esc);
    // Another put at its index, its text the same: the one before it deleted.
    let mut task = f.on_disk(2);
    let at = now() - chrono::Duration::minutes(5);
    task.comments = ["first", "same", "same"]
        .into_iter()
        .zip(0..)
        .map(|(body, s)| Comment::new(at + chrono::Duration::seconds(s), body.into(), Author::Agent))
        .collect();
    Store::at(f.dir.path()).unwrap().save(&mut task).unwrap();
    f.board.reload().unwrap();
    f.board.open_editor(Target::Comment(k(2), 1));
    kb(&f).delete_comment(2, 0).unwrap();
    reload(&mut f);
    assert!(marked(&f));
    f.press(KeyCode::Esc);

    // The form: its title, tags, related entries and the task's status, together when they came together.
    f.press(KeyCode::Char('e'));
    kb(&f).add_comment(1, "third").unwrap();
    reload(&mut f);
    assert!(!marked(&f));
    let mut brain = kb(&f);
    brain.update(1, "Fix the SSO login", vec!["auth".into()]).unwrap();
    brain.set_status(1, Status::Done).unwrap();
    reload(&mut f);
    assert!(marked(&f));
    assert_eq!(f.board.message(), Some("#1's title, tags and status changed on disk"));
    f.press(KeyCode::Esc);
    f.press(KeyCode::Char('e'));
    kb(&f).toggle_link(1, 2).unwrap();
    reload(&mut f);
    assert_eq!(f.board.message(), Some("#1's related entries changed on disk"));
    // Said once here too; kept through a save that stops.
    f.press(KeyCode::Char('!'));
    kb(&f).update(1, "Fix it", vec!["auth".into()]).unwrap();
    reload(&mut f);
    assert_eq!(f.board.message(), None);
    f.press(KeyCode::Enter);
    assert!(marked(&f));
    f.press(KeyCode::Esc);
    // A related entry taken out, none added.
    f.press(KeyCode::Char('e'));
    kb(&f).toggle_link(1, 2).unwrap();
    reload(&mut f);
    assert!(marked(&f));
    assert_eq!(f.board.message(), Some("#1's related entries changed on disk"));
}

#[test]
fn what_kb_changed_in_an_open_dialog_isnt_replaced_by_what_else_is_said() {
    use kanbrain_core::brain::Brain;
    let mut f = Fixture::new(&["Fix login"]);
    let kb = |f: &Fixture| Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
    let reload = |f: &mut Fixture| {
        f.board.message = None;
        f.board.reload().unwrap();
    };
    // Said beside what the next key does: `catch_up` reads the brains right before it, which is when it's found.
    f.board.select(k(1));
    f.press(KeyCode::Char('e'));
    kb(&f).set_status(1, Status::Doing).unwrap();
    reload(&mut f);
    f.press(KeyCode::Char('!'));
    f.press(KeyCode::Enter);
    assert_eq!(f.board.message(), Some("#1 saved · #1's status changed on disk"));
    // Nor is it lost to a file found skipped by the same read.
    f.press(KeyCode::Char('m'));
    kb(&f).set_description(1, "theirs").unwrap();
    std::fs::write(f.dir.path().join("0009-broken.md"), "# Broken\n").unwrap();
    reload(&mut f);
    let said = f.board.message().unwrap();
    assert!(said.contains("#1's description changed on disk") && said.contains("0009-broken.md"), "{said}");
}

#[test]
fn what_kb_changed_goes_beside_a_slow_save_but_not_beside_a_later_inputs_message() {
    use kanbrain_core::brain::Brain;
    let mut f = Fixture::new(&["Fix login"]);
    let kb = |f: &Fixture| Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
    f.board.select(k(1));
    f.press(KeyCode::Char('e'));
    kb(&f).set_status(1, Status::Doing).unwrap();
    f.board.reload().unwrap();
    // The save the read was for takes seconds, on a slow disk or a busy machine: it's still said beside it.
    if let Some((_, at, _)) = &mut f.board.message {
        *at -= Duration::from_secs(2);
    }
    f.press(KeyCode::Char('!'));
    f.press(KeyCode::Enter);
    assert_eq!(f.board.message(), Some("#1 saved · #1's status changed on disk"));
    // Found by a read no input made, as the window's timer does: shown, then replaced as any message.
    f.press(KeyCode::Char('e'));
    kb(&f).set_status(1, Status::Todo).unwrap();
    f.board.reload().unwrap();
    f.board.input_done();
    assert_eq!(f.board.message(), Some("#1's status changed on disk"));
    f.press(KeyCode::Char('?'));
    f.press(KeyCode::Enter);
    assert_eq!(f.board.message(), Some("#1 saved"));
}

#[test]
fn an_open_verdict_says_kb_decided_meanwhile() {
    use kanbrain_core::brain::{Brain, NewEntry};
    let mut f = Fixture::new(&["cache"]);
    let kb = |f: &Fixture| Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
    let redis = kb(&f).propose(1, NewEntry { title: "Use Redis".into(), ..Default::default() }).unwrap();
    f.board.reload().unwrap();
    f.board.select(k(redis));
    f.press(KeyCode::Char('j'));
    kb(&f).resolve(redis, DecisionState::Accepted, "").unwrap();
    f.board.message = None;
    f.board.reload().unwrap();
    assert!(matches!(&f.board.mode, Mode::Edit(edit) if edit.changed_on_disk));
    assert_eq!(f.board.message(), Some(format!("#{redis}'s verdict changed on disk").as_str()));
}

#[test]
fn an_open_form_says_kb_swapped_a_related_entry() {
    use kanbrain_core::brain::Brain;
    let mut f = Fixture::new(&["Fix login", "Other", "Third"]);
    let kb = |f: &Fixture| Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
    kb(&f).toggle_link(1, 2).unwrap();
    f.board.reload().unwrap();
    f.board.select(k(1));
    f.press(KeyCode::Char('e'));
    // As many related entries as before, but another one.
    let mut brain = kb(&f);
    brain.toggle_link(1, 2).unwrap();
    brain.toggle_link(1, 3).unwrap();
    f.board.message = None;
    f.board.reload().unwrap();
    assert!(matches!(&f.board.mode, Mode::Form(form) if form.changed_on_disk));
    assert_eq!(f.board.message(), Some("#1's related entries changed on disk"));
}

#[test]
fn opening_what_an_agent_is_working_on_says_so() {
    use kanbrain_core::brain::Brain;
    let mut f = Fixture::new(&["Fix login", "Mine"]);
    let mut agent = Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
    agent.author = Author::Agent;
    agent.set_status(1, Status::Doing).unwrap();
    agent.add_comment(1, "on it").unwrap();
    f.board.reload().unwrap();
    let opening = |f: &mut Fixture, open: &dyn Fn(&mut Board)| {
        f.board.message = None;
        open(&mut f.board);
        let said = f.board.message().map(str::to_string);
        assert!(!matches!(f.board.mode, Mode::Normal), "opened");
        f.board.mode = Mode::Normal;
        said
    };
    let working = Some("#1 is being worked on by an agent".to_string());

    // The form, its description and its comments: what's saved may cross what the agent writes next.
    f.board.select(k(1));
    assert_eq!(opening(&mut f, &|b| press(b, KeyCode::Char('e'))), working, "the form");
    assert_eq!(opening(&mut f, &|b| press(b, KeyCode::Char('m'))), working, "the description");
    assert_eq!(opening(&mut f, &|b| b.open_editor(Target::Comment(k(1), 0))), working, "a comment");
    // A new comment crosses nothing: it's how to tell the agent.
    assert_eq!(opening(&mut f, &|b| press(b, KeyCode::Char('c'))), None, "a new comment");

    // A task a person started, or one the agent parked, isn't being worked on by an agent.
    f.board.select(k(2));
    f.board.set_status(k(2), Status::Doing);
    assert_eq!(opening(&mut f, &|b| press(b, KeyCode::Char('e'))), None, "the user's");
    let mut agent = Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
    agent.author = Author::Agent;
    agent.set_status(1, Status::Parked).unwrap();
    f.board.reload().unwrap();
    f.board.select(k(1));
    assert_eq!(opening(&mut f, &|b| press(b, KeyCode::Char('e'))), None, "parked");
}

#[test]
fn saving_what_an_agent_is_working_on_asks_whether_to_tell_it() {
    use kanbrain_core::brain::Brain;
    let mut f = Fixture::new(&["Fix login", "Mine"]);
    let kb = |f: &Fixture| Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
    let mut agent = kb(&f);
    agent.author = Author::Agent;
    agent.set_status(1, Status::Doing).unwrap();
    agent.add_comment(1, "on it").unwrap();
    f.board.reload().unwrap();
    let ctrl_enter = |f: &mut Fixture| f.board.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL));
    let last = |f: &Fixture| f.on_disk(1).comments.last().map(|c| (c.body.clone(), c.author)).unwrap();
    let asked = |f: &Fixture| match &f.board.mode {
        Mode::Tell(tell) => Some(tell.pending.what()),
        _ => None,
    };

    // The form: asked before anything is written; `esc` goes back to it as it was.
    f.board.select(k(1));
    f.press(KeyCode::Char('e'));
    f.press(KeyCode::End);
    f.type_text("!");
    f.press(KeyCode::Enter);
    assert_eq!(asked(&f), Some("its title, tags or related entries"));
    assert_eq!(f.on_disk(1).title, "Fix login", "nothing written yet");
    f.press(KeyCode::Esc);
    assert!(matches!(&f.board.mode, Mode::Form(form) if form.title.text() == "Fix login!"));
    // `enter` tells it, with the note: the user's comment says what changed.
    f.press(KeyCode::Enter);
    f.type_text("keep it short");
    f.press(KeyCode::Enter);
    assert!(matches!(f.board.mode, Mode::Normal));
    assert_eq!(f.on_disk(1).title, "Fix login!");
    assert_eq!(last(&f), ("Changed the title: keep it short".to_string(), Author::Human));
    assert_eq!(f.board.message(), Some("#1 saved; the agent is told"));
    // Saved and told in one go: one undo takes both back.
    f.press(KeyCode::Char('u'));
    assert_eq!(f.on_disk(1).title, "Fix login");
    assert_eq!(last(&f).0, "on it");

    // `ctrl+enter` just saves.
    f.press(KeyCode::Char('e'));
    f.press(KeyCode::Tab);
    f.type_text("auth");
    f.press(KeyCode::Enter);
    assert_eq!(asked(&f), Some("its title, tags or related entries"));
    ctrl_enter(&mut f);
    assert_eq!(f.on_disk(1).tags, ["auth"]);
    assert_eq!(last(&f).0, "on it", "not told");
    assert_eq!(f.board.message(), Some("#1 saved"));

    // The description and a comment, without a note.
    f.press(KeyCode::Char('m'));
    f.type_text("the plan");
    f.press(KeyCode::Esc);
    assert_eq!(asked(&f), Some("its description"));
    f.press(KeyCode::Enter);
    assert_eq!(f.on_disk(1).description, "the plan");
    assert_eq!(last(&f).0, "Changed the description.");
    assert_eq!(f.board.message(), Some("description saved; the agent is told"));
    f.board.open_editor(Target::Comment(k(1), 0));
    f.type_text(", noted");
    f.press(KeyCode::Esc);
    assert_eq!(asked(&f), Some("a comment"));
    f.press(KeyCode::Enter);
    assert_eq!(f.on_disk(1).comments[0].body, "on it, noted");
    assert_eq!(last(&f).0, "Changed a comment.");

    // A save that stops over what `kb` changed meanwhile tells nothing: the form is back, to save again.
    f.press(KeyCode::Char('e'));
    f.press(KeyCode::End);
    f.type_text("?");
    f.press(KeyCode::Enter);
    kb(&f).update(1, "Fix the login", vec!["auth".into()]).unwrap();
    f.board.reload().unwrap();
    let said = f.on_disk(1).comments.len();
    f.press(KeyCode::Enter);
    assert!(matches!(f.board.mode, Mode::Form(_)), "stopped");
    assert_eq!(f.on_disk(1).comments.len(), said);
    f.press(KeyCode::Esc);

    // Nothing asked: a new comment (it's how to tell the agent), a save that changes nothing, a task a person runs.
    f.press(KeyCode::Char('c'));
    f.type_text("one more thing");
    f.press(KeyCode::Esc);
    assert!(matches!(f.board.mode, Mode::Normal));
    assert_eq!(last(&f).0, "one more thing");
    f.press(KeyCode::Char('e'));
    f.press(KeyCode::Enter);
    assert!(matches!(f.board.mode, Mode::Normal));
    f.board.set_status(k(2), Status::Doing);
    f.board.select(k(2));
    f.press(KeyCode::Char('e'));
    f.type_text("!");
    f.press(KeyCode::Enter);
    assert!(matches!(f.board.mode, Mode::Normal));
}

#[test]
fn the_question_hears_kb_meanwhile_and_its_button_just_saves() {
    use kanbrain_core::brain::Brain;
    let mut f = Fixture::new(&["Fix login"]);
    let kb = |f: &Fixture| {
        let mut agent = Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
        agent.author = Author::Agent;
        agent
    };
    kb(&f).set_status(1, Status::Doing).unwrap();
    f.board.reload().unwrap();
    f.board.select(k(1));
    let comments = |f: &Fixture| f.on_disk(1).comments.len();

    // Just save, from its button.
    f.press(KeyCode::Char('m'));
    f.type_text("the plan");
    f.press(KeyCode::Esc);
    f.board.just_save();
    assert!(matches!(f.board.mode, Mode::Normal));
    assert_eq!(f.on_disk(1).description, "the plan");
    assert_eq!(comments(&f), 0);

    // The agent finishes it while the question is open: marked and said as in the form, and not told after all.
    f.press(KeyCode::Char('e'));
    f.press(KeyCode::End);
    f.type_text("!");
    f.press(KeyCode::Enter);
    kb(&f).set_status(1, Status::Done).unwrap();
    f.board.message = None;
    f.board.reload().unwrap();
    assert_eq!(f.board.message(), Some("#1's status changed on disk"));
    assert!(
        matches!(&f.board.mode, Mode::Tell(tell) if matches!(&tell.pending, Pending::Form(form) if form.changed_on_disk))
    );
    f.press(KeyCode::Enter);
    assert_eq!(f.on_disk(1).title, "Fix login!");
    assert_eq!(comments(&f), 0);
    assert_eq!(
        f.board.message(),
        Some("#1 saved; not told: no agent is working on it now · #1's status changed on disk")
    );
}

#[test]
fn the_question_closes_with_its_entry_gone() {
    use kanbrain_core::brain::Brain;
    let mut f = Fixture::new(&["Fix login"]);
    let mut agent = Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
    agent.author = Author::Agent;
    agent.set_status(1, Status::Doing).unwrap();
    f.board.reload().unwrap();
    f.board.select(k(1));
    f.press(KeyCode::Char('m'));
    f.type_text("the plan");
    f.press(KeyCode::Esc);
    assert!(matches!(f.board.mode, Mode::Tell(_)));
    f.board.reload().unwrap();
    assert!(matches!(f.board.mode, Mode::Tell(_)), "kept across a reload");
    Store::at(f.dir.path()).unwrap().delete(&f.on_disk(1)).unwrap();
    f.board.reload().unwrap();
    assert!(matches!(f.board.mode, Mode::Normal));
    assert_eq!(f.board.message(), Some("closed: #1 is gone, deleted or moved meanwhile"));
}

#[test]
fn a_new_entrys_form_has_nothing_on_disk_to_mark() {
    use kanbrain_core::brain::Brain;
    let mut f = Fixture::new(&["Fix login"]);
    f.press(KeyCode::Char('n'));
    Brain::open(Store::at(f.dir.path()).unwrap()).unwrap().update(1, "Renamed", Vec::new()).unwrap();
    f.board.reload().unwrap();
    assert!(matches!(&f.board.mode, Mode::Form(form) if !form.changed_on_disk));
}

#[test]
fn the_editor_doesnt_save_a_comment_over_another_moved_to_its_place_meanwhile() {
    use kanbrain_core::brain::Brain;
    use kanbrain_core::model::Comment;
    type Meanwhile = fn(&mut Brain);
    let mut f = Fixture::new(&["feature"]);
    // Written in one second, as an agent's quick comments are, with the same text: neither tells them apart.
    let at = now() - chrono::Duration::minutes(5);
    let mut task = f.on_disk(1);
    for _ in 0..3 {
        task.comments.push(Comment::new(at, "ok".into(), Author::Agent));
    }
    let bodies = |f: &Fixture| f.on_disk(1).comments.into_iter().map(|c| c.body).collect::<Vec<_>>();
    let cases: [(usize, Meanwhile, &[&str]); 5] = [
        // Another window of the app, or a sync, takes it out: the next is at its index now.
        (0, |kb| kb.delete_comment(1, 0).unwrap(), &["ok", "ok"]),
        // One before it taken out.
        (1, |kb| kb.delete_comment(1, 0).unwrap(), &["ok", "ok"]),
        // The next one edited, then this one taken out: the edited one is at its index.
        (
            0,
            |kb| {
                kb.update_comment(1, 1, "edited").unwrap();
                kb.delete_comment(1, 0).unwrap();
            },
            &["edited", "ok"],
        ),
        // The last taken out and another written after: the new one is at its index.
        (
            2,
            |kb| {
                kb.delete_comment(1, 2).unwrap();
                kb.add_comment(1, "new").unwrap();
            },
            &["ok", "ok", "new"],
        ),
        // Another one edited: the comments aren't as the editor found them, so it can't tell which is which.
        (
            1,
            |kb| {
                kb.update_comment(1, 0, "edited").unwrap();
            },
            &["edited", "ok", "ok"],
        ),
    ];
    for (index, meanwhile, left) in cases {
        Store::at(f.dir.path()).unwrap().save(&mut task.clone()).unwrap();
        f.board.reload().unwrap();
        f.board.open_editor(Target::Comment(k(1), index));
        f.type_text(", edited");
        meanwhile(&mut Brain::open(Store::at(f.dir.path()).unwrap()).unwrap());
        f.board.reload().unwrap();
        for _ in 0..2 {
            f.board.message = None;
            f.press(KeyCode::Esc);
            assert!(matches!(f.board.mode, Mode::Edit(_)), "kept open, to copy the text from");
            assert_eq!(
                f.board.message(),
                Some("#1's comments changed around this one meanwhile: copy your text, then ctrl+d"),
                "editing {index}, {left:?}"
            );
        }
        assert_eq!(bodies(&f), left, "no comment saved over");
        f.board.discard_edit();
    }

    // The last one replaced in its file by one written when it was, by someone else; or by the same author, later.
    let replacements = [
        Comment::new(at, "new".into(), Author::Human),
        Comment::new(at + chrono::Duration::seconds(1), "new".into(), Author::Agent),
    ];
    for replacement in replacements {
        let store = Store::at(f.dir.path()).unwrap();
        store.save(&mut task.clone()).unwrap();
        f.board.reload().unwrap();
        f.board.open_editor(Target::Comment(k(1), 2));
        f.type_text(", edited");
        let mut replaced = f.on_disk(1);
        replaced.comments[2] = replacement;
        store.save(&mut replaced).unwrap();
        f.board.reload().unwrap();
        for _ in 0..2 {
            f.board.message = None;
            f.press(KeyCode::Esc);
            assert!(
                f.board.message().is_some_and(|m| m.contains("changed around this one")),
                "{:?}",
                f.board.message()
            );
        }
        assert_eq!(bodies(&f), ["ok", "ok", "new"], "no comment saved over");
        f.board.discard_edit();
    }

    // A comment written after it, as an agent goes on, changes nothing around it: it's saved.
    Store::at(f.dir.path()).unwrap().save(&mut task.clone()).unwrap();
    f.board.reload().unwrap();
    f.board.open_editor(Target::Comment(k(1), 0));
    f.type_text(", mine");
    Brain::open(Store::at(f.dir.path()).unwrap()).unwrap().add_comment(1, "the agent's").unwrap();
    f.board.reload().unwrap();
    f.press(KeyCode::Esc);
    assert!(matches!(f.board.mode, Mode::Normal));
    assert_eq!(bodies(&f), ["ok, mine", "ok", "ok", "the agent's"]);
}

#[test]
fn capital_c_picks_a_comment_to_edit_or_delete() {
    let mut f = Fixture::new(&["feature"]);
    f.press(KeyCode::Char('C'));
    assert!(matches!(f.board.mode, Mode::Normal), "no comments: nothing to pick");
    assert_eq!(f.board.message(), Some("no comments yet: c adds one"), "and said");
    for text in ["first", "second"] {
        f.press(KeyCode::Char('c'));
        f.type_text(text);
        f.press(KeyCode::Esc);
    }
    f.press(KeyCode::Char('C'));
    assert!(matches!(f.board.mode, Mode::Comments(CommentPicker { sel: 1, .. })), "the latest highlighted");
    f.press(KeyCode::Up);
    f.press(KeyCode::Enter);
    assert!(matches!(&f.board.mode, Mode::Edit(edit) if edit.target == Target::Comment(k(1), 0)));
    f.press(KeyCode::Esc);
    f.press(KeyCode::Char('C'));
    f.press(KeyCode::Char('x'));
    f.press(KeyCode::Char('y'));
    let comments: Vec<String> = f.on_disk(1).comments.into_iter().map(|c| c.body).collect();
    assert_eq!(comments, ["first"]);
}

#[test]
fn running_tasks_keep_their_order_the_longest_running_first() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::at(dir.path()).unwrap();
    let t0 = now() - chrono::Duration::hours(1);
    // Saved newest first, so the files' order isn't the one asked for.
    for (id, minutes) in [(1, 30), (2, 10), (3, 20)] {
        let mut task = Task::new(id, format!("task {id}"), t0);
        let at = t0 + chrono::Duration::minutes(minutes);
        task.set_status(Status::Doing, at, Author::Agent);
        task.start_session(at);
        store.save(&mut task).unwrap();
    }
    let board = Board::open(dir.path(), None).unwrap();
    let ids: Vec<u32> = board.running().iter().map(|(key, _)| key.id).collect();
    assert_eq!(ids, [2, 3, 1]);
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
        f.board.reload_changed(&[f.dir.path().to_path_buf()]).unwrap();
        assert!(matches!(f.board.mode, Mode::Normal), "{open}: closed, not left taking the keys unseen");
        assert_eq!(f.board.message(), Some("closed: #2 is gone, deleted or moved meanwhile"));
    }
    // Named with its repository when several are shown: "#1" alone could be any of them.
    let (central, code) = projects();
    let mut board = Board::open(central.path(), None).unwrap();
    board.select(Key::new(1, 1));
    press(&mut board, KeyCode::Char('e'));
    let api = Store::at(code.path().join("api/.brain")).unwrap();
    api.delete(&api.load().unwrap().0[0]).unwrap();
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

/// A central brain with one task, project shop's `api` and `web` repositories with one task each,
/// and project blog with one; all live as long as the returned folders.
fn projects() -> (TempDir, TempDir) {
    use kanbrain_core::projects;
    let (central, code) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let task = |dir: &std::path::Path, title: &str| {
        Store::at(dir).unwrap().save(&mut Task::new(1, title.to_string(), now())).unwrap();
    };
    task(central.path(), "renew passport");
    for (project, repo, title) in [("shop", "api", "rate limit"), ("shop", "web", "checkout"), ("blog", "blog", "post")]
    {
        let dir = code.path().join(repo).join(".brain");
        task(&dir, title);
        projects::register(central.path(), project, &dir).unwrap();
        projects::write_repo_card(
            &dir,
            &projects::RepoCard { project: project.into(), repo: None, notes: String::new() },
        )
        .unwrap();
    }
    projects::write_description(central.path(), "shop", "An online shop.").unwrap();
    (central, code)
}

#[test]
fn only_the_brains_that_changed_are_read_again() {
    use kanbrain_core::{brain::Brain, projects};
    let (central, code) = projects();
    let mut board = Board::open(central.path(), None).unwrap();
    let dirs: Vec<_> = board.sources.iter().map(|s| s.brain.store.dir().to_path_buf()).collect();
    let finish =
        |dir: &std::path::Path| Brain::open(Store::at(dir).unwrap()).unwrap().set_status(1, Status::Done).unwrap();
    for dir in &dirs[1..=3] {
        finish(dir);
    }
    board.reload_changed(&dirs[1..2]).unwrap();
    let status = |board: &Board, src| board.brain(src).task(1).unwrap().status;
    let statuses = [1, 2, 3].map(|src| status(&board, src));
    assert_eq!(statuses, [Status::Done, Status::Done, Status::Todo], "api as named, web of its project, not blog");

    let another = code.path().join("shop").join(".brain");
    std::fs::create_dir_all(&another).unwrap();
    projects::register(central.path(), "shop", &another).unwrap();
    board.reload_changed(&dirs[..1]).unwrap();
    assert_eq!(board.sources.len(), 5, "a repository registered in the central brain reads everything");
    assert_eq!(status(&board, 3), Status::Done);
}

#[test]
fn a_repository_renamed_by_another_program_reads_with_its_new_name() {
    use kanbrain_core::projects;
    let (central, code) = projects();
    let mut board = Board::open(central.path(), None).unwrap();
    let api = code.path().join("api").join(".brain");
    let src = board.sources.iter().position(|s| s.brain.store.dir() == api).unwrap();
    assert_eq!(board.sources[src].label.as_deref(), Some("shop/api"));
    // As `kb project repo backend` there: only its own `project.md` changes.
    projects::write_repo_card(
        &api,
        &projects::RepoCard { project: "shop".into(), repo: Some("backend".into()), notes: String::new() },
    )
    .unwrap();
    board.reload_changed(&[api]).unwrap();
    assert_eq!(board.sources[src].label.as_deref(), Some("shop/backend"));
}

#[test]
fn manage_project_acts_by_key_on_the_highlighted_repository_or_the_project() {
    let (central, _code) = projects();
    let mut board = Board::open(central.path(), None).unwrap();
    board.set_scope(Scope::Project("shop".into()));
    press(&mut board, KeyCode::Char('I'));
    let Mode::ManageProject(first) = board.mode else { panic!("I opens it") };
    press(&mut board, KeyCode::Down);
    let Mode::ManageProject(second) = board.mode else { panic!("arrows keep it open") };
    assert_ne!(first, second, "the next repository");
    press(&mut board, KeyCode::Char('r'));
    assert!(matches!(&board.mode, Mode::Manage(Manage::RenameRepo { src, .. }) if *src == second));
    press(&mut board, KeyCode::Esc);
    assert!(matches!(board.mode, Mode::ManageProject(_)), "back to it");
    press(&mut board, KeyCode::Char('N'));
    assert!(matches!(&board.mode, Mode::Edit(edit) if edit.target == Target::RepoNotes(second)));
    press(&mut board, KeyCode::Esc);
    assert!(matches!(board.mode, Mode::ManageProject(src) if src == second), "the editor goes back to it too");
    press(&mut board, KeyCode::Char('X'));
    assert!(matches!(board.mode, Mode::Manage(Manage::Remove { whole: true, .. })), "the whole project");
}

#[test]
fn n_edits_the_role_of_the_selected_cards_repository() {
    let (central, _code) = projects();
    let mut board = Board::open(central.path(), None).unwrap();
    board.set_scope(Scope::Central);
    press(&mut board, KeyCode::Char('N'));
    assert!(matches!(board.mode, Mode::Normal), "the central brain has no role");
    board.set_scope(Scope::Repo(2));
    press(&mut board, KeyCode::Char('N'));
    assert!(matches!(&board.mode, Mode::Edit(edit) if edit.target == Target::RepoNotes(2)), "web's");
}

#[test]
fn a_moves_message_names_the_repository_when_several_are_shown() {
    let (central, _code) = projects();
    let mut board = Board::open(central.path(), None).unwrap();
    board.set_status(Key::new(1, 1), Status::Doing);
    assert_eq!(board.message(), Some("shop/api #1 rate limit → doing"));
    board.brain_mut(1).add(kanbrain_core::brain::NewEntry { title: "other".into(), ..Default::default() }).unwrap();
    board.set_status(Key::new(1, 2), Status::Doing);
    let refused = board.message().unwrap();
    assert!(refused.starts_with("shop/api #1 is in doing"), "whose #1: {refused}");
}

#[test]
fn a_tag_has_one_color_whichever_brain_or_scope_shows_it() {
    let (central, code) = projects();
    let colors = |dir: &std::path::Path, text: &str| std::fs::write(dir.join("tags.md"), text).unwrap();
    colors(&code.path().join("api/.brain"), "# Tag colors\n\n- ui: red\n");
    colors(&code.path().join("web/.brain"), "# Tag colors\n\n- ui: blue\n");
    let mut board = Board::open(central.path(), None).unwrap();
    let red = board.tag_colors("ui");
    assert_eq!(red, board.brain(1).tag_colors.chip("ui"), "the first brain that has it: api's");
    board.set_scope(Scope::Repo(2));
    assert_eq!(board.tag_colors("ui"), red, "on web's own cards too");
    assert_eq!(board.tag_colors("nowhere"), None);
}

#[test]
fn a_shown_project_taken_out_from_the_command_line_shows_all() {
    let (central, code) = projects();
    let mut board = Board::open(central.path(), None).unwrap();
    board.set_scope(Scope::Project("blog".into()));
    kanbrain_core::cli::remove_repo(central.path(), &code.path().join("blog/.brain"), false).unwrap();
    board.reload().unwrap();
    assert_eq!(board.scope, Scope::All, "not a blank scope");
}

#[test]
fn a_shown_project_renamed_from_the_command_line_stays_shown() {
    let (central, code) = projects();
    let mut board = Board::open(central.path(), None).unwrap();
    board.set_scope(Scope::Project("shop".into()));
    kanbrain_core::cli::rename_project(central.path(), "shop", "store").unwrap();
    board.reload().unwrap();
    assert_eq!(board.scope, Scope::Project("store".into()), "under its new name");

    // Its last repository joining a project that has others is no rename: all of them.
    board.set_scope(Scope::Project("blog".into()));
    kanbrain_core::cli::move_repo(central.path(), &code.path().join("blog/.brain"), "store").unwrap();
    board.reload().unwrap();
    assert_eq!(board.scope, Scope::All);
}

#[test]
fn every_project_shows_and_the_scope_filters_them() {
    let (central, _code) = projects();
    let mut board = Board::open(central.path(), None).unwrap();
    let titles = |board: &Board| {
        let mut titles: Vec<String> = board.column(Status::Todo).iter().map(|(_, t)| t.title.clone()).collect();
        titles.sort();
        titles
    };
    let scopes: Vec<String> = board.scopes().into_iter().map(|(_, name)| name).collect();
    assert_eq!(scopes, ["All", "Central", "shop", "shop/api", "shop/web", "blog"]);
    assert_eq!(titles(&board), ["checkout", "post", "rate limit", "renew passport"]);
    assert_eq!(board.source_label(1), Some("shop/api"), "cards say where they're from");

    board.set_scope(Scope::Project("shop".into()));
    assert_eq!(titles(&board), ["checkout", "rate limit"]);
    // The dropdown: P opens it on the scope shown, typing filters, Enter shows the highlighted one.
    press(&mut board, KeyCode::Char('P'));
    let Mode::Scopes(picker) = &board.mode else { panic!("P opens the project dropdown") };
    assert_eq!(board.scope_candidates(picker)[picker.sel].1, "shop");
    "api".chars().for_each(|c| press(&mut board, KeyCode::Char(c)));
    let Mode::Scopes(picker) = &board.mode else { panic!("typing keeps it open") };
    let names: Vec<String> = board.scope_candidates(picker).into_iter().map(|(_, n)| n).collect();
    assert_eq!(names, ["shop/api"]);
    press(&mut board, KeyCode::Enter);
    assert!(matches!(board.mode, Mode::Normal), "Enter closes it");
    assert_eq!((board.scope.clone(), titles(&board)), (Scope::Repo(1), vec!["rate limit".to_string()]));
    assert_eq!(board.source_label(1), None, "one brain shown: no labels");
    board.set_scope(Scope::Central);
    assert_eq!(titles(&board), ["renew passport"]);
}

#[test]
fn new_entries_go_to_the_repository_shown_and_ids_are_per_brain() {
    let (central, code) = projects();
    // Written differently from how it was registered, as `/var` and `/private/var` are on macOS: still the same
    // repository, not another brain.
    let web = code.path().join("web/../web/.brain");
    let mut board = Board::open(central.path(), Some(web)).unwrap();
    assert_eq!(board.scope, Scope::Repo(2), "opened in a repository of a project with several");
    press(&mut board, KeyCode::Char('n'));
    "cart".chars().for_each(|c| board.on_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)));
    press(&mut board, KeyCode::Enter);
    assert_eq!(board.selected, Some(Key::new(2, 2)), "#2 in web, though api has a #1 too");
    assert_eq!(board.brain(2).task(2).unwrap().title, "cart");
}

#[test]
fn a_new_entry_goes_where_the_filter_points_and_its_form_picks_another_project() {
    let (central, _code) = projects();
    let mut board = Board::open(central.path(), None).unwrap();
    let form = |board: &Board| {
        let Mode::Form(form) = &board.mode else { panic!("the form is open: {:?}", board.mode) };
        (form.src(), form.field, board.form_projects(form).into_iter().map(|(_, n)| n).collect::<Vec<_>>())
    };
    board.selected = Some(Key::new(3, 1));
    press(&mut board, KeyCode::Char('n'));
    assert_eq!(form(&board).0, 3, "All: the selected card's brain, with no question first");
    press(&mut board, KeyCode::Up);
    let (_, field, names) = form(&board);
    assert_eq!(field, Field::Project, "the project field comes before the title");
    assert_eq!(names, ["Central", "shop/api", "shop/web", "blog"], "every brain");
    let Mode::Form(open) = &board.mode else { unreachable!() };
    assert_eq!(open.project.as_ref().unwrap().sel, 3, "highlighting the form's own");
    "web".chars().for_each(|c| press(&mut board, KeyCode::Char(c)));
    assert_eq!(form(&board).2, ["shop/web"]);
    press(&mut board, KeyCode::Enter);
    assert_eq!(form(&board).0, 2, "enter picks it");
    assert_eq!(form(&board).1, Field::Title, "and goes back to the title");
    "cart".chars().for_each(|c| press(&mut board, KeyCode::Char(c)));
    press(&mut board, KeyCode::Enter);
    assert_eq!(board.brain(2).task(2).unwrap().title, "cart");
    assert_eq!(board.message(), Some("created shop/web #2"), "named as everywhere else");

    board.set_scope(Scope::Project("blog".into()));
    board.selected = None;
    press(&mut board, KeyCode::Char('n'));
    assert_eq!(form(&board).0, 3, "a one-repository project: its brain");
    let Mode::Form(open) = &mut board.mode else { unreachable!() };
    open.links.push(Key::new(3, 1));
    board.choose_form_project(0);
    let Mode::Form(open) = &board.mode else { unreachable!() };
    assert_eq!(open.src(), 0, "anywhere, whatever the scope");
    assert!(open.links.is_empty(), "they're picked among its brain's and project's, so they're dropped");
    press(&mut board, KeyCode::Esc);

    board.set_scope(Scope::Project("shop".into()));
    board.selected = Some(Key::new(2, 1));
    press(&mut board, KeyCode::Char('a'));
    let Mode::Where(asked) = &board.mode else { panic!("a asks where: {:?}", board.mode) };
    let names: Vec<String> = board.where_candidates(&asked.picker, asked.then).into_iter().map(|(_, n)| n).collect();
    assert_eq!(names, ["shop/api", "shop/web"], "quick add, with no form, asks among the repositories");
    assert_eq!(asked.picker.sel, 1, "starting at the selected entry's");
    press(&mut board, KeyCode::Esc);
    assert!(matches!(board.mode, Mode::Normal), "Esc creates nothing");
    board.set_scope(Scope::Central);
    press(&mut board, KeyCode::Char('a'));
    assert!(matches!(board.mode, Mode::QuickAdd(QuickAdd { src: 0, .. })), "one brain shown: no question");
}

#[test]
fn entries_related_across_a_projects_repositories_show_on_both() {
    use kanbrain_core::model::RepoLink;
    let (central, _code) = projects();
    let mut board = Board::open(central.path(), None).unwrap();
    let (api, web) = (Key::new(1, 1), Key::new(2, 1));
    let to_api = [RepoLink { repo: "api".into(), id: 1 }];
    board.sources[2].brain.link(1, &[], &to_api).unwrap();
    assert_eq!(board.repo_related(web), [api], "web's checkout links api's rate limit");
    assert_eq!(board.repo_related(api), [web], "and api's shows it back");
    assert!(board.repo_related(Key::new(3, 1)).is_empty(), "another project's entries never are");

    // The link picker offers the project's other repositories, and unlinks from either side.
    board.selected = Some(api);
    board.open_links();
    let Mode::Links(picker) = &board.mode else { panic!("r opens the picker") };
    let keys: Vec<Key> = board.link_candidates(picker).iter().map(|&(k, _)| k).collect();
    assert_eq!(keys, [web], "web's entry, related so first; never blog's");
    board.toggle_link(0);
    assert!(board.repo_related(web).is_empty(), "unlinked, though web stored the link");

    // The form links across repositories too, and a delete takes the links to it away.
    board.mode = Mode::Normal;
    board.selected = Some(web);
    board.edit_selected();
    let Mode::Form(form) = &mut board.mode else { panic!("e edits") };
    form.links.push(api);
    board.save_form();
    assert_eq!(board.brain(2).task(1).unwrap().repo_links, to_api, "saved on web's side");
    board.selected = Some(api);
    press(&mut board, KeyCode::Char('x'));
    press(&mut board, KeyCode::Char('y'));
    assert!(board.brain(1).task(1).is_none());
    assert!(board.brain(2).task(1).unwrap().repo_links.is_empty(), "web's link to it went with it");
    let file = std::fs::read_to_string(board.brain(2).task(1).unwrap().path.as_ref().unwrap()).unwrap();
    assert!(!file.contains("- links:"), "on disk too");
}

#[test]
fn an_entry_moves_to_another_project_and_is_renumbered_there() {
    let (central, _code) = projects();
    let mut board = Board::open(central.path(), None).unwrap();
    board.set_scope(Scope::Project("shop".into()));
    board.selected = Some(Key::new(1, 1));
    press(&mut board, KeyCode::Char('M'));
    let Mode::Where(asked) = &board.mode else { panic!("M asks where: {:?}", board.mode) };
    let names: Vec<String> = board.where_candidates(&asked.picker, asked.then).into_iter().map(|(_, n)| n).collect();
    assert_eq!(names, ["Central", "shop/web", "blog"], "every other brain, whatever the scope");
    "web".chars().for_each(|c| press(&mut board, KeyCode::Char(c)));
    press(&mut board, KeyCode::Enter);
    assert!(board.brain(1).task(1).is_none(), "gone from api");
    assert_eq!(board.brain(2).task(2).unwrap().title, "rate limit", "web's next id");
    assert_eq!(board.selected, Some(Key::new(2, 2)), "still shown in shop: it stays selected");

    press(&mut board, KeyCode::Char('M'));
    "blog".chars().for_each(|c| press(&mut board, KeyCode::Char(c)));
    press(&mut board, KeyCode::Enter);
    assert_eq!(board.brain(3).task(2).unwrap().title, "rate limit");
    assert_eq!(board.scope, Scope::Project("shop".into()), "the filter stays as it was");
    assert!(board.message().unwrap().contains("moved shop/web #2 to blog as #2"), "{:?}", board.message());
}

#[test]
fn each_project_runs_its_own_task() {
    let (central, _code) = projects();
    let mut board = Board::open(central.path(), None).unwrap();
    board.set_status(Key::new(1, 1), Status::Doing);
    board.set_status(Key::new(3, 1), Status::Doing);
    let running: Vec<Key> = board.running().into_iter().map(|(key, _)| key).collect();
    assert_eq!(running, [Key::new(1, 1), Key::new(3, 1)], "shop/api and blog both run");
    // Within one brain, you run one task at a time.
    let id =
        board.brain_mut(1).add(kanbrain_core::brain::NewEntry { title: "next".into(), ..Default::default() }).unwrap();
    board.set_status(Key::new(1, id), Status::Doing);
    assert_eq!(board.task(Key::new(1, id)).unwrap().status, Status::Todo);
    assert!(board.message().unwrap().contains("#1 is in doing"), "{:?}", board.message());
    board.park_and_quit();
    assert!(board.running().is_empty(), "Park & quit parks yours, in every brain");
}

#[test]
fn the_project_description_is_shared_by_its_repositories() {
    let (central, _code) = projects();
    let mut board = Board::open(central.path(), None).unwrap();
    assert_eq!(board.brain(2).project.as_ref().unwrap().description, "An online shop.");
    board.open_editor(Target::Project(1));
    press(&mut board, KeyCode::Char('!'));
    press(&mut board, KeyCode::Esc);
    assert_eq!(board.brain(2).project.as_ref().unwrap().description, "An online shop.!", "web sees api's edit");
}

#[test]
fn a_project_description_or_role_editor_says_kb_changed_it() {
    use kanbrain_core::projects;
    let (central, code) = projects();
    let mut board = Board::open(central.path(), None).unwrap();
    let api = code.path().join("api/.brain");
    let marked = |board: &Board| matches!(&board.mode, Mode::Edit(edit) if edit.changed_on_disk);
    board.open_editor(Target::Project(1));
    projects::write_description(central.path(), "shop", "A shop, described by kb.").unwrap();
    board.message = None;
    board.reload().unwrap();
    assert!(marked(&board));
    assert_eq!(board.message(), Some("the project's description changed on disk"));
    board.discard_edit();

    board.open_editor(Target::RepoNotes(1));
    let card = projects::read_repo_card(&api).unwrap();
    projects::write_repo_card(&api, &projects::RepoCard { notes: "The API, says kb.".into(), ..card }).unwrap();
    board.message = None;
    board.reload().unwrap();
    assert!(marked(&board));
    assert_eq!(board.message(), Some("this repository's role changed on disk"));
}

#[test]
fn the_editor_doesnt_replace_unseen_a_project_description_or_role_kb_wrote_meanwhile() {
    use kanbrain_core::projects;
    let (central, code) = projects();
    let mut board = Board::open(central.path(), None).unwrap();
    let api = code.path().join("api/.brain");
    let saving = |board: &mut Board| {
        board.message = None;
        press(board, KeyCode::Esc);
    };
    board.open_editor(Target::Project(1));
    press(&mut board, KeyCode::Char('!'));
    projects::write_description(central.path(), "shop", "A shop, described by kb.").unwrap();
    board.reload().unwrap();
    saving(&mut board);
    assert_eq!(
        board.message(),
        Some("the project's description changed meanwhile: pick yours or theirs where you both changed it")
    );
    saving(&mut board);
    saving(&mut board);
    assert_eq!(board.brain(2).project.as_ref().unwrap().description, "An online shop.!");

    board.open_editor(Target::RepoNotes(1));
    press(&mut board, KeyCode::Char('?'));
    let card = projects::read_repo_card(&api).unwrap();
    projects::write_repo_card(&api, &projects::RepoCard { notes: "The API, says kb.".into(), ..card }).unwrap();
    board.reload().unwrap();
    saving(&mut board);
    assert_eq!(
        board.message(),
        Some("this repository's role changed meanwhile: pick yours or theirs where you both changed it")
    );
    assert_eq!(projects::read_repo_card(&api).unwrap().notes, "The API, says kb.");
}

#[test]
fn manage_project_opens_for_a_shown_project() {
    let (central, _code) = projects();
    let mut board = Board::open(central.path(), None).unwrap();
    board.selected = Some(Key { src: 0, id: 1 });
    press(&mut board, KeyCode::Char('I'));
    assert!(matches!(board.mode, Mode::Normal), "All shows no single project, and the central brain isn't one");
    assert!(board.message().is_some_and(|m| m.contains("select one of its cards")));
    board.selected = Some(Key { src: 2, id: 1 });
    press(&mut board, KeyCode::Char('I'));
    assert!(matches!(board.mode, Mode::ManageProject(2)), "the selected card's project");
    press(&mut board, KeyCode::Esc);
    board.set_scope(Scope::Repo(2));
    press(&mut board, KeyCode::Char('I'));
    assert!(matches!(board.mode, Mode::ManageProject(2)));
    assert_eq!(board.project_repos(2), [1, 2], "it lists every repository of shop");
    press(&mut board, KeyCode::Esc);
    assert!(matches!(board.mode, Mode::Normal));
}

#[test]
fn an_open_dialog_follows_its_project_when_another_goes() {
    use kanbrain_core::projects;
    let (central, code) = projects();
    let mut board = Board::open(central.path(), None).unwrap();
    let src_of = |board: &Board, project: &str| {
        board.sources.iter().position(|s| s.project.as_deref() == Some(project)).unwrap()
    };
    let blog = src_of(&board, "blog");
    board.mode = Mode::ManageProject(blog);
    projects::unregister(central.path(), &code.path().join("api/.brain")).unwrap();
    board.reload().unwrap();
    assert!(matches!(board.mode, Mode::ManageProject(src) if src == blog - 1 && src == src_of(&board, "blog")));
    projects::unregister(central.path(), &code.path().join("blog/.brain")).unwrap();
    board.reload().unwrap();
    assert!(matches!(board.mode, Mode::Normal), "closed with its project");
    assert_eq!(board.message(), Some("closed: its project is gone"), "and said");
}

#[test]
fn the_tag_dropdown_offers_only_the_shown_projects_tags() {
    let (central, _code) = projects();
    let mut board = Board::open(central.path(), None).unwrap();
    let tag = |board: &mut Board, key: Key, tags: &str| {
        board.brain_mut(key.src).update(key.id, "tagged", kanbrain_core::model::parse_tags(tags)).unwrap();
    };
    tag(&mut board, Key::new(1, 1), "backend");
    tag(&mut board, Key::new(2, 1), "frontend");
    tag(&mut board, Key::new(3, 1), "writing");
    board.set_scope(Scope::Project("shop".into()));
    press(&mut board, KeyCode::Char('#'));
    let Mode::Tags(picker) = &board.mode else { panic!("# opens the tag dropdown") };
    assert_eq!(board.tag_candidates(picker), [None, Some("backend".into()), Some("frontend".into())], "no #writing");
    "front".chars().for_each(|c| press(&mut board, KeyCode::Char(c)));
    press(&mut board, KeyCode::Enter);
    assert_eq!(board.search.text(), "#frontend", "the tag goes into the search box");

    // A tag only on a note: the notes tab offers it, the board and the report (tasks) don't.
    let note = kanbrain_core::brain::NewEntry {
        kind: kanbrain_core::model::Kind::Note,
        title: "test card".into(),
        tags: vec!["stripe".into()],
        ..Default::default()
    };
    board.brain_mut(1).add(note).unwrap();
    assert_eq!(board.tag_uses("stripe"), [0, 1, 0]);
    let picker = Picker::default();
    let stripe = Some("stripe".to_string());
    assert!(!board.tag_candidates(&picker).contains(&stripe));
    board.switch_tab(Tab::Notes);
    assert!(board.tag_candidates(&picker).contains(&stripe));
    board.switch_tab(Tab::Report);
    assert!(!board.tag_candidates(&picker).contains(&stripe));
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
fn the_form_suggests_the_tags_of_its_own_kind() {
    use kanbrain_core::brain::NewEntry;
    use kanbrain_core::model::Kind as EntryKind;
    let mut f = Fixture::new(&["login bug"]);
    let brain = f.board.brain_mut(0);
    brain.update(1, "login bug", vec!["auth".into()]).unwrap();
    brain
        .add(NewEntry {
            kind: EntryKind::Note,
            title: "card".into(),
            tags: vec!["stripe".into()],
            ..NewEntry::default()
        })
        .unwrap();
    f.press(KeyCode::Char('n'));
    let Mode::Form(form) = &f.board.mode else { panic!("n opens the form") };
    assert_eq!(f.board.form_tags(form), (EntryKind::Task, vec![("auth".to_string(), 1)]), "a task's: no #stripe");
    f.press(KeyCode::Esc);
    f.board.switch_tab(Tab::Notes);
    f.press(KeyCode::Char('e'));
    let Mode::Form(form) = &f.board.mode else { panic!("e edits the note") };
    assert_eq!(
        f.board.form_tags(form),
        (EntryKind::Note, Vec::new()),
        "a note's, but not the #stripe it has: no #auth"
    );
}

#[test]
fn the_form_suggests_only_the_tags_containing_the_one_typed() {
    let mut f = Fixture::new(&["login bug", "slow page"]);
    let brain = f.board.brain_mut(0);
    brain.update(1, "login bug", vec!["auth".into(), "bug".into()]).unwrap();
    brain.update(2, "slow page", vec!["perf".into(), "prod".into()]).unwrap();
    let tags = |f: &Fixture| {
        let Mode::Form(form) = &f.board.mode else { panic!("the form is open") };
        f.board.form_tags(form).1.into_iter().map(|(tag, _)| tag).collect::<Vec<_>>()
    };
    f.press(KeyCode::Char('n'));
    f.type_text("p");
    assert_eq!(tags(&f), ["auth", "bug", "perf", "prod"], "what the title says filters nothing");
    f.press(KeyCode::Tab);
    f.type_text("#R");
    assert_eq!(tags(&f), ["perf", "prod"], "as saved (no #, lowercase), and contained, as the dropdowns filter");
    f.type_text("o");
    assert_eq!(tags(&f), ["prod"]);
    f.type_text("d, ");
    assert_eq!(tags(&f), ["auth", "bug", "perf", "prod"], "after a separator, #rod is held and a new tag: every one");
    f.type_text("x");
    assert!(tags(&f).is_empty(), "none in use has it");
    let Mode::Form(form) = &f.board.mode else { panic!("the form is open") };
    assert_eq!((form.tags.as_slice(), form.tag_input.text()), (["rod".to_string()].as_slice(), "x"));
}

#[test]
fn pasting_or_cutting_in_the_tags_field_highlights_as_typing_does() {
    let mut f = Fixture::new(&["login bug", "slow page"]);
    let brain = f.board.brain_mut(0);
    brain.update(1, "login bug", vec!["auth".into(), "prod".into()]).unwrap();
    brain.update(2, "slow page", vec!["perf".into(), "prod".into()]).unwrap();
    let sel = |f: &Fixture| {
        let Mode::Form(form) = &f.board.mode else { panic!("the form is open") };
        form.tag_sel.clone()
    };
    f.press(KeyCode::Char('n'));
    f.type_text("x");
    f.press(KeyCode::Tab);
    f.board.paste("pe");
    assert_eq!(sel(&f).as_deref(), Some("perf"), "pasted, the start of a tag is highlighted");
    f.board.select_all();
    assert_eq!(f.board.cut().as_deref(), Some("pe"));
    assert_eq!(sel(&f), None, "cut, nothing is typed: enter saves");
    f.board.paste("prod");
    assert_eq!(sel(&f), None, "a whole tag pasted: enter saves it as typed");
}

#[test]
fn the_tags_highlight_stays_on_its_tag_when_a_reload_adds_one_ahead() {
    let mut f = Fixture::new(&["login bug", "slow page"]);
    f.board.brain_mut(0).update(2, "slow page", vec!["perf".into(), "speed".into()]).unwrap();
    f.press(KeyCode::Char('n'));
    f.type_text("x");
    f.press(KeyCode::Tab);
    f.type_text("pe");
    // Another writer (an agent's `kb`) tags a task `ape`, which sorts ahead of `perf` and contains `pe`.
    let mut task = f.on_disk(1);
    task.tags = vec!["ape".into()];
    Store::at(f.dir.path()).unwrap().save(&mut task).unwrap();
    f.board.reload().unwrap();
    f.press(KeyCode::Enter);
    let Mode::Form(form) = &f.board.mode else { panic!("the form is open") };
    assert_eq!(form.tags, ["perf"], "enter picks the tag highlighted, not the one now in its row");
}

#[test]
fn the_form_picks_tags_from_its_list() {
    let mut f = Fixture::new(&["login bug", "slow page"]);
    let brain = f.board.brain_mut(0);
    brain.update(1, "login bug", vec!["auth".into(), "prod".into()]).unwrap();
    brain.update(2, "slow page", vec!["perf".into(), "prod".into()]).unwrap();
    let form = |f: &Fixture| {
        let Mode::Form(form) = &f.board.mode else { panic!("the form is open") };
        (f.board.form_tags(form).1, form.tag_sel.clone(), form.tags.join(" "))
    };
    let owned = |tags: &[(&str, usize)]| tags.iter().map(|&(t, n)| (t.to_string(), n)).collect::<Vec<_>>();
    f.press(KeyCode::Char('n'));
    f.type_text("x");
    f.press(KeyCode::Tab);
    assert_eq!(form(&f).0, owned(&[("auth", 1), ("perf", 1), ("prod", 2)]), "each with how many use it");
    assert_eq!(form(&f).1, None, "nothing highlighted until a tag is typed");
    f.type_text("prod");
    assert_eq!(form(&f).1, None, "a whole tag typed is taken as it is");
    (0..3).for_each(|_| f.press(KeyCode::Backspace));
    assert_eq!(form(&f).0, owned(&[("perf", 1), ("prod", 2)]));
    assert_eq!(form(&f).1.as_deref(), Some("perf"), "part of one highlights the first");
    f.press(KeyCode::Down);
    f.press(KeyCode::Down);
    assert_eq!(form(&f).1.as_deref(), Some("prod"), "the last stays highlighted");
    f.press(KeyCode::Up);
    f.press(KeyCode::Up);
    assert_eq!(form(&f).1, None, "up off the first leaves the list");
    assert!(matches!(&f.board.mode, Mode::Form(form) if form.field == Field::Tags), "and stays in the field");
    f.press(KeyCode::Down);
    f.press(KeyCode::Enter);
    assert_eq!((form(&f).1, form(&f).2), (None, "perf".to_string()), "enter picks it, ready for the next");
    assert_eq!(form(&f).0, owned(&[("auth", 1), ("prod", 2)]), "what the field holds isn't offered again");
    f.board.choose_form_tag("prod");
    assert_eq!(form(&f).2, "perf prod", "a click picks one too");
    f.board.choose_form_tag("ape");
    assert_eq!(form(&f).2, "perf prod", "but not one the list no longer holds");
    f.type_text("zz");
    assert!(form(&f).0.is_empty(), "nothing to pick");
    f.press(KeyCode::Down);
    assert!(
        matches!(&f.board.mode, Mode::Form(form) if form.field == Field::Links),
        "so down goes on to the next field"
    );
    f.press(KeyCode::BackTab);
    f.press(KeyCode::End);
    f.press(KeyCode::Enter);
    let task = f.board.brain(0).tasks.iter().find(|t| t.title == "x").expect("enter with nothing to pick saves");
    assert_eq!(task.tags, ["perf", "prod", "zz"]);
}

#[test]
fn enter_saves_the_form_unless_the_start_of_a_tag_was_typed() {
    let mut f = Fixture::new(&["login bug", "slow page"]);
    let brain = f.board.brain_mut(0);
    brain.update(1, "login bug", vec!["build".into(), "project".into()]).unwrap();
    brain.update(2, "slow page", vec!["prod".into()]).unwrap();
    let sel = |f: &Fixture| {
        let Mode::Form(form) = &f.board.mode else { panic!("the form is open") };
        form.tag_sel.clone()
    };
    f.board.selected = Some(Key { src: 0, id: 2 });
    f.press(KeyCode::Char('e'));
    f.press(KeyCode::Tab);
    f.type_text(" pro");
    assert_eq!(sel(&f).as_deref(), Some("project"), "typing the start of #project does");
    let Mode::Form(form) = &mut f.board.mode else { panic!("the form is open") };
    form.tag_sel = Some("gone".into());
    f.press(KeyCode::Enter);
    assert_eq!(f.on_disk(2).tags, ["prod", "pro"], "a highlighted tag no longer listed is none: enter saves");

    f.press(KeyCode::Char('n'));
    f.type_text("x");
    f.press(KeyCode::Tab);
    f.type_text("ui");
    assert_eq!(sel(&f), None, "#build only contains ui");
    f.press(KeyCode::Enter);
    let task = f.board.brain(0).tasks.iter().find(|t| t.title == "x").expect("enter saves a new tag");
    assert_eq!(task.tags, ["ui"]);
}

#[test]
fn a_picked_tag_takes_the_place_of_the_one_typed() {
    let mut form = Form::new(0, Kind::Task, Status::Todo, false);
    form.go_to(Field::Tags);
    form.tag_input = LineInput::new("#Pr");
    form.put_tag("prod");
    assert_eq!((form.tags.as_slice(), form.tag_input.text()), (["prod".to_string()].as_slice(), ""));
    form.tag_input = LineInput::new("au");
    form.put_tag("auth");
    form.put_tag("prod");
    assert_eq!(form.tags, ["prod", "auth"], "after the others, and once");
    form.tag_input = LineInput::new("bug");
    form.tag_input.select_all();
    assert_eq!(form.typed_tag(), "", "selected text isn't a tag being typed");
    form.tag_input = LineInput::new("#Bu");
    assert_eq!(form.typed_tag(), "bu", "as it's saved");
}

#[test]
fn editing_shows_the_tags_held_as_chips_and_takes_them_off_only_with_their_x() {
    let mut f = Fixture::new(&["login bug"]);
    f.board.brain_mut(0).update(1, "login bug", vec!["auth".into(), "prod".into(), "ui".into()]).unwrap();
    let field = |f: &Fixture| {
        let Mode::Form(form) = &f.board.mode else { panic!("the form is open") };
        (form.tags.join(" "), form.tag_input.text().to_string())
    };
    f.press(KeyCode::Char('e'));
    f.press(KeyCode::Tab);
    assert_eq!(field(&f), ("auth prod ui".into(), String::new()), "each tag a chip, the box empty to type one");
    f.board.untag_in_form("prod");
    assert_eq!(field(&f).0, "auth ui", "its × takes it off");
    f.type_text("x");
    f.press(KeyCode::Backspace);
    assert_eq!(field(&f), ("auth ui".into(), String::new()), "backspace deletes what's typed first");
    f.press(KeyCode::Backspace);
    assert_eq!(field(&f).0, "auth ui", "and in the empty box, nothing: a chip comes off only with its ×");
    f.press(KeyCode::Enter);
    assert_eq!(f.on_disk(1).tags, ["auth", "ui"]);
}

#[test]
fn a_separator_typed_or_pasted_turns_the_tags_before_it_into_chips() {
    let mut f = Fixture::new(&["login bug"]);
    f.board.brain_mut(0).update(1, "login bug", vec!["auth".into()]).unwrap();
    let field = |f: &Fixture| {
        let Mode::Form(form) = &f.board.mode else { panic!("the form is open") };
        (form.tags.join(" "), form.tag_input.text().to_string())
    };
    f.press(KeyCode::Char('e'));
    f.press(KeyCode::Tab);
    f.type_text("#Bug,");
    assert_eq!(field(&f), ("auth bug".into(), String::new()), "as it's saved: lowercase, without #");
    f.type_text(" auth ");
    assert_eq!(field(&f).0, "auth bug", "a tag held isn't held twice");
    f.board.paste("ui, prod  db");
    assert_eq!(field(&f), ("auth bug ui prod".into(), "db".into()), "pasted, the last is still being typed");
    f.board.paste(", ");
    f.board.paste(",");
    assert_eq!(field(&f), ("auth bug ui prod db".into(), String::new()), "a separator alone adds nothing more");
    f.board.paste("ops");
    f.press(KeyCode::Home);
    f.board.paste("x, ");
    assert_eq!(field(&f), ("auth bug ui prod db x".into(), "ops".into()), "nor empties what follows");
    f.press(KeyCode::Right);
    f.type_text(" ");
    assert_eq!(field(&f), ("auth bug ui prod db x o".into(), "ps".into()), "typed mid-word, it splits it there");
    f.type_text("1");
    assert_eq!(field(&f).1, "1ps", "the cursor where it was in what's left");
    f.press(KeyCode::Enter);
    assert_eq!(f.on_disk(1).tags, ["auth", "bug", "ui", "prod", "db", "x", "o", "1ps"], "and saved with the chips");
}

#[test]
fn a_tag_held_typed_again_is_neither_highlighted_nor_new_and_enter_saves() {
    let mut f = Fixture::new(&["login bug", "signup"]);
    let brain = f.board.brain_mut(0);
    brain.update(1, "login bug", vec!["auth".into()]).unwrap();
    brain.update(2, "signup", vec!["author".into()]).unwrap();
    f.board.selected = Some(Key { src: 0, id: 1 });
    f.press(KeyCode::Char('e'));
    f.press(KeyCode::Tab);
    f.type_text("auth");
    let Mode::Form(form) = &f.board.mode else { panic!("the form is open") };
    assert_eq!((form.tag_sel.as_deref(), form.typed_tag_held()), (None, true), "#author isn't picked");
    f.press(KeyCode::Enter);
    assert!(matches!(f.board.mode, Mode::Normal), "enter saves");
    assert_eq!(f.on_disk(1).tags, ["auth"]);
}

#[test]
fn the_form_relates_entries_from_a_searchable_list() {
    let mut f = Fixture::new(&["login bug", "export", "sso"]);
    f.press(KeyCode::Char('n'));
    f.type_text("hotfix");
    f.press(KeyCode::Tab);
    f.press(KeyCode::Tab);
    let Mode::Form(form) = &f.board.mode else { panic!("the form is open") };
    assert_eq!(form.field, Field::Links, "title, tags, then related");
    f.type_text("login");
    f.press(KeyCode::Enter);
    let Mode::Form(form) = &f.board.mode else { panic!("enter checks an entry, the form stays") };
    assert_eq!(form.links, [k(1)]);
    f.press(KeyCode::Tab);
    f.press(KeyCode::Enter);
    let hotfix = f.board.brain(0).tasks.iter().find(|t| t.title == "hotfix").map(|t| t.id).unwrap();
    assert!(f.board.brain(0).are_related(hotfix, 1), "created already linked");

    // Editing starts from what it's related to; unchecking unlinks, checking links.
    f.board.select(k(hotfix));
    f.press(KeyCode::Char('e'));
    let Mode::Form(form) = &f.board.mode else { panic!("e edits") };
    assert_eq!(form.links, [k(1)]);
    f.board.unlink_in_form(k(1));
    f.press(KeyCode::Tab);
    f.press(KeyCode::Tab);
    f.type_text("sso");
    f.press(KeyCode::Enter);
    f.board.save_form();
    let brain = f.board.brain(0);
    assert!(!brain.are_related(hotfix, 1) && brain.are_related(hotfix, 3));

    // It can stay empty.
    f.press(KeyCode::Char('n'));
    f.type_text("alone");
    f.press(KeyCode::Enter);
    let alone = f.board.brain(0).tasks.iter().find(|t| t.title == "alone").unwrap();
    assert_eq!(alone.links, Vec::<u32>::new());
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
fn a_picked_folder_becomes_a_project_as_kb_init_makes_it() {
    use kanbrain_core::projects;
    let (central, code) = projects();
    let mut board = Board::open(central.path(), None).unwrap();
    let options = |board: &Board| {
        let Mode::AddProject(add) = &board.mode else { panic!("the dialog is open") };
        board.add_project_options(&add.picker)
    };
    press(&mut board, KeyCode::Char('A'));
    assert_eq!(board.request.take(), Some(Request::Folder(None)), "A asks the window for the folder dialog");

    let notes = code.path().join("notes");
    std::fs::create_dir_all(&notes).unwrap();
    board.name_project(notes.clone(), None);
    assert_eq!(options(&board), [ProjectOption::New("notes".into())], "named after the folder to begin with");
    // The name starts selected, so typing replaces it.
    "journal".chars().for_each(|c| press(&mut board, KeyCode::Char(c)));
    assert_eq!(options(&board), [ProjectOption::New("journal".into())]);
    press(&mut board, KeyCode::Enter);
    assert!(matches!(board.mode, Mode::Normal));
    assert_eq!(board.scope, Scope::Project("journal".into()), "the new project is shown");
    assert_eq!(
        board.message(),
        Some("added project journal; its agent instructions tell agents to use kanbrain"),
        "a new project, not a repository added to one"
    );
    assert!(notes.join(".brain").is_dir());
    assert!(std::fs::read_to_string(notes.join("CLAUDE.md")).unwrap().contains("kanbrain:start"));
    let repos = projects::load(central.path()).unwrap();
    assert!(repos.iter().any(|r| r.project == "journal"), "registered in the central brain");

    // A repository's folder offers its own project, which there is, so no new one.
    board.name_project(code.path().join("api"), None);
    assert_eq!(options(&board), [ProjectOption::Existing("shop".into())]);
    // Nothing typed lists every project; typing searches them, after a new one of that name.
    let admin = code.path().join("admin");
    std::fs::create_dir_all(&admin).unwrap();
    board.name_project(admin, None);
    press(&mut board, KeyCode::Backspace);
    let names: Vec<String> = options(&board).iter().map(|o| o.name().to_string()).collect();
    assert_eq!(names, ["shop", "blog", "journal"]);
    "sh".chars().for_each(|c| press(&mut board, KeyCode::Char(c)));
    assert_eq!(options(&board), [ProjectOption::New("sh".into()), ProjectOption::Existing("shop".into())]);
    press(&mut board, KeyCode::Down);
    press(&mut board, KeyCode::Enter);
    assert_eq!(board.project_repos(board.project_src().unwrap()).len(), 3, "api, web and admin");
    assert!(board.message().unwrap().contains("to project shop"));
}

#[test]
fn the_user_answers_questions_and_decides_on_proposals() {
    let mut f = Fixture::new(&["cache", "css"]);
    let mut agent = kanbrain_core::brain::Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
    agent.author = Author::Agent;
    let new = kanbrain_core::brain::NewEntry { title: "Use Redis".into(), ..Default::default() };
    let decision = agent.propose(1, new).unwrap();
    agent.ask(2, "Which colour?").unwrap();
    f.press(KeyCode::Char('R'));
    assert_eq!(f.board.waiting(), [k(1), k(2)]);
    for expected in [1, 2, 1] {
        f.press(KeyCode::Char('w'));
        assert_eq!((f.board.selected, f.board.drawer), (Some(k(expected)), true), "w cycles through them");
    }

    f.board.open_entry(k(2));
    f.press(KeyCode::Char('c'));
    f.type_text("#fa0");
    f.board.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL));
    assert!(matches!(f.on_disk(2).asked(), Some(Asked::Answered(a)) if a.author == Author::Human));
    assert_eq!(f.board.message(), Some("answer sent: the agent picks it up"));
    f.board.notify("");
    f.press(KeyCode::Char('c'));
    f.type_text("x");
    f.press(KeyCode::Backspace);
    f.board.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL));
    assert_eq!(f.board.message(), Some(""), "a comment typed and taken back says nothing");
    f.press(KeyCode::Char('c'));
    f.type_text("  ");
    f.board.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL));
    assert_eq!(f.board.message(), Some("nothing added: the comment was empty"), "blank, said");

    f.board.open_entry(k(decision));
    let labels: Vec<&str> = f.board.menu_items(k(decision)).iter().map(|i| i.label).collect();
    assert!(labels.contains(&"Accept") && labels.contains(&"Reject"), "{labels:?}");
    f.press(KeyCode::Char('j'));
    f.type_text("too costly");
    f.board.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL));
    assert_eq!(f.on_disk(decision).decision, DecisionState::Rejected);
    let task = f.on_disk(1);
    assert_eq!(task.comments.last().unwrap().body, format!("Rejected decision #{decision}, Use Redis: too costly"));
    assert!(matches!(task.asked(), Some(Asked::Answered(_))));
    assert_eq!(f.board.waiting(), []);
    f.press(KeyCode::Char('y'));
    assert_eq!(f.on_disk(decision).decision, DecisionState::Accepted, "and reconsidered");
}

#[test]
fn while_the_board_acts_a_kb_run_meanwhile_waits() {
    let mut f = Fixture::new(&["feature"]);
    let locks = f.board.lock_brains(&mut Vec::new());
    assert_eq!(locks.len(), f.board.sources.len(), "every brain's");
    let dir = f.board.brain(0).store.dir().to_path_buf();
    let (taken, waiting) = std::sync::mpsc::channel();
    // As another `kb` process would: a lock of its own on the same brain.
    let other = std::thread::spawn(move || {
        let _lock = kanbrain_core::store::Store::at(&dir).unwrap().lock().unwrap();
        taken.send(()).unwrap();
    });
    assert!(waiting.recv_timeout(std::time::Duration::from_millis(200)).is_err(), "it waits for the board");
    drop(locks);
    assert!(waiting.recv_timeout(std::time::Duration::from_secs(5)).is_ok(), "and goes on once released");
    other.join().unwrap();
}

#[test]
fn a_brain_found_while_the_board_acts_is_locked_and_read_under_its_lock() {
    use kanbrain_core::{brain::Brain, projects};
    let (central, code) = projects();
    let mut board = Board::open(central.path(), None).unwrap();
    let mut tried = Vec::new();
    let mut locks = board.lock_brains(&mut tried);
    // Catching up finds a project registered meanwhile, read before its lock was taken.
    let shop = code.path().join("shop/.brain");
    std::fs::create_dir_all(&shop).unwrap();
    projects::register(central.path(), "shop", &shop).unwrap();
    board.reload_changed(&[board.sources[0].brain.store.dir().to_path_buf()]).unwrap();
    let src = board.src_at(&shop).expect("found");
    // A `kb` writing to it between that read and the lock.
    let new = |title: &str| kanbrain_core::brain::NewEntry { title: title.into(), ..Default::default() };
    Brain::open(Store::at(&shop).unwrap()).unwrap().add(new("Ship it")).unwrap();
    assert!(board.brain(src).task(1).is_none(), "not read yet");

    board.lock_found_brains(&mut locks, &mut tried);
    assert_eq!(locks.len(), board.sources.len(), "its lock too");
    assert_eq!(tried.len(), board.sources.len(), "each brain tried once, the central one first");
    assert_eq!(tried[0], board.brain(0).store.dir());
    let src = board.src_at(&shop).unwrap();
    assert_eq!(board.brain(src).task(1).unwrap().title, "Ship it", "read again under it");
    let mut again = Vec::new();
    board.lock_found_brains(&mut again, &mut tried);
    assert!(again.is_empty(), "none found since: nothing more to take");
    let (taken, waiting) = std::sync::mpsc::channel();
    let other = std::thread::spawn(move || {
        let _lock = Store::at(&shop).unwrap().lock().unwrap();
        taken.send(()).unwrap();
    });
    assert!(waiting.recv_timeout(std::time::Duration::from_millis(200)).is_err(), "a kb run waits for the board");
    drop(locks);
    assert!(waiting.recv_timeout(std::time::Duration::from_secs(5)).is_ok());
    other.join().unwrap();
}

#[test]
fn projects_and_repositories_are_managed_from_the_projects_info() {
    let (central, code) = projects();
    let mut board = Board::open(central.path(), None).unwrap();
    let type_in = |board: &mut Board, text: &str| text.chars().for_each(|c| press(board, KeyCode::Char(c)));
    let labels = |board: &Board| board.sources.iter().filter_map(|s| s.label.clone()).collect::<Vec<_>>();
    let src =
        |board: &Board, label: &str| board.sources.iter().position(|s| s.label.as_deref() == Some(label)).unwrap();

    board.set_scope(Scope::Project("shop".into()));
    board.ask_rename_project(src(&board, "shop/api"));
    type_in(&mut board, "store");
    press(&mut board, KeyCode::Enter);
    assert_eq!(labels(&board), ["store/api", "store/web", "blog"]);
    assert!(matches!(board.mode, Mode::ManageProject(_)), "back to Manage project");
    assert_eq!(board.scope, Scope::Project("store".into()), "the board follows the new name");

    board.ask_rename_repo(src(&board, "store/api"));
    type_in(&mut board, "web");
    press(&mut board, KeyCode::Enter);
    assert!(matches!(board.mode, Mode::Manage(_)), "a name the project has: the dialog stays, with why");
    assert!(board.message().unwrap().contains("already has a repository named web"), "{:?}", board.message());
    press(&mut board, KeyCode::Esc);
    assert!(matches!(board.mode, Mode::ManageProject(_)), "esc goes back to Manage project");
    board.ask_rename_repo(src(&board, "store/api"));
    type_in(&mut board, "backend");
    press(&mut board, KeyCode::Enter);
    assert_eq!(labels(&board), ["store/backend", "store/web", "blog"]);

    board.ask_move_repo(src(&board, "store/web"));
    type_in(&mut board, "blog");
    let Mode::Manage(Manage::MoveRepo { picker, .. }) = &board.mode else { panic!("the move dialog") };
    assert_eq!(board.move_options(src(&board, "store/web"), picker), [ProjectOption::Existing("blog".into())]);
    press(&mut board, KeyCode::Enter);
    assert_eq!(labels(&board), ["store", "blog/blog", "blog/web"]);

    board.ask_remove(src(&board, "blog/web"), false);
    press(&mut board, KeyCode::Char('y'));
    assert_eq!(labels(&board), ["store", "blog"]);
    assert!(code.path().join("web/.brain").is_dir(), "unlinked, its tasks kept");
    assert!(matches!(board.mode, Mode::ManageProject(at) if at == src(&board, "blog")), "back on blog's other one");
    board.mode = Mode::Normal;

    let blog = src(&board, "blog");
    board.add_repo_to(blog);
    let blog_dir = board.sources[blog].brain.store.dir().to_path_buf();
    assert_eq!(board.request.take(), Some(Request::Folder(Some(blog_dir.clone()))), "the folder joins this project");
    let mobile = code.path().join("mobile");
    std::fs::create_dir(&mobile).unwrap();
    board.name_project(mobile.clone(), Some(&blog_dir));
    press(&mut board, KeyCode::Esc);
    assert!(matches!(board.mode, Mode::ManageProject(at) if at == blog), "cancelled: back to it, on the same one");
    board.name_project(mobile, Some(&blog_dir));
    press(&mut board, KeyCode::Enter);
    assert!(
        matches!(board.mode, Mode::ManageProject(at) if at == src(&board, "blog/mobile")),
        "added: back to it, on the new repository: {:?}",
        board.mode
    );
    let said = board.message().unwrap();
    assert!(said.starts_with("added ") && said.contains("mobile to project blog;"), "{said}");
    board.mode = Mode::Normal;
    board.ask_remove(src(&board, "blog/mobile"), false);
    press(&mut board, KeyCode::Char('y'));

    board.ask_remove(src(&board, "store"), true);
    press(&mut board, KeyCode::Char('d'));
    assert_eq!(labels(&board), ["blog"]);
    assert!(!code.path().join("api/.brain").exists(), "its tasks deleted too");
    assert!(board.message().unwrap().starts_with("deleted "), "and said: {:?}", board.message());
    assert!(matches!(board.mode, Mode::Normal));
}

#[test]
fn a_repository_a_newer_kanbrain_wrote_is_left_out_and_said() {
    let (central, code) = projects();
    let newer = kanbrain_core::migrate::FORMAT + 1;
    std::fs::write(code.path().join("web/.brain").join(kanbrain_core::migrate::FORMAT_FILE), format!("{newer}\n"))
        .unwrap();
    let mut board = Board::open(central.path(), None).unwrap();
    let labels: Vec<_> = board.sources.iter().filter_map(|s| s.label.clone()).collect();
    assert_eq!(labels, ["shop/api", "blog"], "the rest still opens");
    assert!(board.message().unwrap().contains("written by a newer kanbrain"), "{:?}", board.message());

    // Still left out, it changes nothing: a write elsewhere reads only that brain again, not everything.
    let blog = code.path().join("blog/.brain");
    kanbrain_core::brain::Brain::open(Store::at(&blog).unwrap()).unwrap().set_status(1, Status::Done).unwrap();
    let api = code.path().join("api/.brain");
    board.reload_changed(&[api]).unwrap();
    let blog_src = board.sources.iter().position(|s| s.brain.store.dir() == blog).unwrap();
    assert_eq!(board.brain(blog_src).task(1).unwrap().status, Status::Todo, "blog wasn't read again");
}

#[test]
fn a_repository_that_cant_be_read_is_left_out_and_said() {
    let (central, code) = projects();
    let web = code.path().join("web/.brain");
    std::fs::write(web.join(kanbrain_core::migrate::FORMAT_FILE), "abc\n").unwrap();
    let mut board = Board::open(central.path(), None).expect("the others still open");
    let labels = |board: &Board| board.sources.iter().filter_map(|s| s.label.clone()).collect::<Vec<_>>();
    assert_eq!(labels(&board), ["shop/api", "blog"]);
    assert!(board.message().unwrap().contains("shop/web left out"), "{:?}", board.message());

    // Said once: an agent's write to another brain doesn't say it again.
    board.message = None;
    let blog = code.path().join("blog/.brain");
    kanbrain_core::brain::Brain::open(Store::at(&blog).unwrap()).unwrap().set_status(1, Status::Done).unwrap();
    board.reload_changed(&[blog]).unwrap();
    assert_eq!(board.message(), None);
    // Nor a write to it, which reads everything again, nor the full read a second after the window opens.
    std::fs::write(web.join("0002-from-a-newer-agent.md"), "# x\n\n- id: 2\n").unwrap();
    board.reload_changed(std::slice::from_ref(&web)).unwrap();
    board.reload().unwrap();
    assert_eq!(board.message(), None);
    // Watched, it's shown once repaired.
    assert!(board.watched_dirs().contains(&web), "{:?}", board.watched_dirs());
    std::fs::write(web.join(kanbrain_core::migrate::FORMAT_FILE), "1\n").unwrap();
    board.reload_changed(&[web]).unwrap();
    assert_eq!(labels(&board), ["shop/api", "shop/web", "blog"]);
}

#[test]
fn an_update_is_offered_once_found_and_waits_for_the_agents() {
    let mut f = Fixture::new(&["rate-limit the login"]);
    f.shift(KeyCode::Char('U'));
    assert!(f.board.message().unwrap().starts_with("no newer release"), "{:?}", f.board.message());
    assert!(matches!(f.board.mode, Mode::Normal));

    f.board.available = Some("9.0.0".into());
    f.board.brain_mut(0).author = Author::Agent;
    f.board.brain_mut(0).set_status(1, Status::Doing).unwrap();
    f.shift(KeyCode::Char('U'));
    let msg = f.board.message().unwrap();
    assert!(msg.contains("can't update to 9.0.0 yet") && msg.contains("#1 rate-limit the login"), "{msg}");
    assert!(matches!(f.board.mode, Mode::Normal), "not even asked");

    f.board.brain_mut(0).set_status(1, Status::Parked).unwrap();
    f.shift(KeyCode::Char('U'));
    assert!(matches!(f.board.mode, Mode::Confirm(Confirm::Update)));
    f.press(KeyCode::Char('y'));
    assert_eq!(f.board.request.take(), Some(Request::Update), "the window installs it");
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
fn a_task_waiting_on_you_is_answered_and_its_decision_opened_from_the_palette_and_menu() {
    let mut f = Fixture::new(&["cache", "css"]);
    let mut agent = kanbrain_core::brain::Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
    agent.author = Author::Agent;
    let new = kanbrain_core::brain::NewEntry { title: "Use Redis".into(), ..Default::default() };
    let decision = agent.propose(1, new).unwrap();
    agent.ask(2, "Which colour?").unwrap();
    f.press(KeyCode::Char('R'));
    let labels = |board: &Board, id| board.menu_items(k(id)).iter().map(|a| a.label).collect::<Vec<_>>();
    let rows = |board: &Board| board.commands(&Picker::default()).iter().map(|a| a.does).collect::<Vec<_>>();

    f.board.select(k(2));
    assert!(labels(&f.board, 2).contains(&"Answer") && !labels(&f.board, 2).contains(&"Add comment"));
    assert!(!labels(&f.board, 2).contains(&"Open proposed decision"), "a question, no decision");
    assert!(rows(&f.board).contains(&"Answer the agent's question"));
    f.press(KeyCode::Char('c'));
    assert!(matches!(f.board.mode, Mode::Edit(_)), "c answers it, as it comments");
    f.press(KeyCode::Esc);
    f.press(KeyCode::Esc);

    f.board.select(k(1));
    assert!(labels(&f.board, 1).contains(&"Open proposed decision"), "{:?}", labels(&f.board, 1));
    assert!(rows(&f.board).contains(&"Open the decision it waits on"));
    f.press(KeyCode::Char('D'));
    assert_eq!((f.board.tab, f.board.selected), (Tab::Decisions, Some(k(decision))), "D opens it");
    assert!(!labels(&f.board, decision).contains(&"Answer"), "a decision waits on nothing");
}

#[test]
fn e_edits_the_description_of_the_project_here() {
    let (central, _code) = projects();
    let mut board = Board::open(central.path(), None).unwrap();
    board.selected = None;
    press(&mut board, KeyCode::Char('E'));
    assert!(matches!(board.mode, Mode::Normal), "with All and nothing selected, no project is here");
    assert!(!board.commands(&Picker::default()).iter().any(|a| a.shown == "E"), "nor offered");
    let api = board.sources.iter().position(|s| s.label.as_deref() == Some("shop/api")).unwrap();
    board.select(Key::new(api, 1));
    assert!(board.commands(&Picker::default()).iter().any(|a| a.shown == "E"), "a card of shop is selected");
    press(&mut board, KeyCode::Char('E'));
    let Mode::Edit(edit) = &board.mode else { panic!("E opens the editor: {:?}", board.mode) };
    assert_eq!(edit.target, Target::Project(api));
}

#[test]
fn closing_an_answer_never_written_says_nothing_was_answered() {
    let mut f = Fixture::new(&["css"]);
    let mut agent = kanbrain_core::brain::Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
    agent.author = Author::Agent;
    agent.ask(1, "Which colour?").unwrap();
    f.press(KeyCode::Char('R'));
    f.board.open_entry(k(1));
    let waits = |f: &Fixture| matches!(f.on_disk(1).asked(), Some(Asked::Waiting(_)));

    f.press(KeyCode::Char('c'));
    f.press(KeyCode::Esc);
    assert!(matches!(f.board.mode, Mode::Normal) && waits(&f), "closed, and still waiting");
    assert_eq!(f.board.message(), Some("nothing answered: the question still waits"));

    f.press(KeyCode::Char('c'));
    f.type_text("  ");
    f.press(KeyCode::Esc);
    assert!(waits(&f) && f.on_disk(1).comments.len() == 1, "blanks aren't an answer");

    f.press(KeyCode::Char('c'));
    f.type_text("Blue");
    f.press(KeyCode::Esc);
    assert!(!waits(&f), "something said answers it");
}

#[test]
fn a_task_waiting_on_a_decision_is_answered_by_deciding_with_or_without_a_comment() {
    let mut f = Fixture::new(&["cache", "queue", "logs"]);
    let mut agent = kanbrain_core::brain::Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
    agent.author = Author::Agent;
    let propose = |agent: &mut kanbrain_core::brain::Brain, id, title: &str| {
        agent.propose(id, kanbrain_core::brain::NewEntry { title: title.into(), ..Default::default() }).unwrap()
    };
    let redis = propose(&mut agent, 1, "Use Redis");
    let kafka = propose(&mut agent, 2, "Use Kafka");
    let loki = propose(&mut agent, 3, "Use Loki");
    f.press(KeyCode::Char('R'));
    let labels = |board: &Board, id| board.menu_items(k(id)).iter().map(|a| a.label).collect::<Vec<_>>();
    let last = |f: &Fixture, id| f.on_disk(id).comments.last().unwrap().body.clone();

    f.board.select(k(1));
    let menu = labels(&f.board, 1);
    for offered in ["Accept", "Accept with a comment", "Reject", "Open proposed decision"] {
        assert!(menu.contains(&offered), "{offered}: {menu:?}");
    }
    assert!(!menu.contains(&"Answer"), "it's answered by deciding");
    f.press(KeyCode::Char('Y'));
    f.press(KeyCode::Esc);
    assert_eq!(f.on_disk(redis).decision, DecisionState::Proposed, "a comment never written accepts nothing");
    assert_eq!(f.board.message(), Some("not accepted: write the comment, or accept without one (y)"));
    f.press(KeyCode::Char('Y'));
    f.type_text("fine for now");
    f.press(KeyCode::Esc);
    assert_eq!(f.on_disk(redis).decision, DecisionState::Accepted);
    assert_eq!(last(&f, 1), format!("Accepted decision #{redis}, Use Redis: fine for now"), "the task gets it");

    f.board.select(k(2));
    f.press(KeyCode::Char('y'));
    assert_eq!(f.on_disk(kafka).decision, DecisionState::Accepted, "or without one");
    assert_eq!(last(&f, 2), format!("Accepted decision #{kafka}, Use Kafka."));

    f.board.select(k(3));
    f.press(KeyCode::Char('j'));
    f.press(KeyCode::Esc);
    assert_eq!(f.on_disk(loki).decision, DecisionState::Proposed, "rejecting always says why");
    assert_eq!(f.board.message(), Some("not rejected: say why it's rejected"));
    f.press(KeyCode::Char('j'));
    f.type_text("too heavy");
    f.press(KeyCode::Esc);
    assert_eq!(f.on_disk(loki).decision, DecisionState::Rejected);
    assert_eq!(f.board.message(), Some(format!("#{loki} rejected").as_str()), "named, as `y` names it");
    assert_eq!(last(&f, 3), format!("Rejected decision #{loki}, Use Loki: too heavy"));
}

#[test]
fn the_reject_or_accept_editor_doesnt_overturn_unseen_a_verdict_kb_gave_while_it_was_open() {
    use kanbrain_core::brain::{Brain, NewEntry};
    let mut f = Fixture::new(&["cache", "queue"]);
    let kb = |f: &Fixture| Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
    let mut agent = kb(&f);
    agent.author = Author::Agent;
    let redis = agent.propose(1, NewEntry { title: "Use Redis".into(), ..Default::default() }).unwrap();
    let kafka = agent.propose(2, NewEntry { title: "Use Kafka".into(), ..Default::default() }).unwrap();
    f.board.reload().unwrap();
    let saving = |f: &mut Fixture| {
        f.board.message = None;
        f.press(KeyCode::Esc);
    };
    let said = |f: &Fixture, id| f.on_disk(id).comments.len();

    // Accepted by kb while the user writes why it's rejected: the save stops and says so, the editor still open.
    f.board.select(k(redis));
    f.press(KeyCode::Char('j'));
    f.type_text("too costly");
    kb(&f).resolve(redis, DecisionState::Accepted, "").unwrap();
    f.board.reload().unwrap();
    let before = said(&f, redis);
    saving(&mut f);
    assert!(matches!(&f.board.mode, Mode::Edit(edit) if edit.editor.text() == "too costly"), "kept, with the reason");
    assert_eq!(
        f.board.message(),
        Some(format!("#{redis} was accepted meanwhile: save again to reject it anyway, ctrl+d to leave it").as_str())
    );
    assert_eq!(f.on_disk(redis).decision, DecisionState::Accepted, "not overturned unseen");
    assert_eq!(said(&f, redis), before);
    // Saved again, knowing it: the user's verdict.
    saving(&mut f);
    assert!(matches!(f.board.mode, Mode::Normal));
    assert_eq!(f.on_disk(redis).decision, DecisionState::Rejected);
    assert_eq!(f.board.message(), Some(format!("#{redis} rejected").as_str()));
    assert_eq!(said(&f, redis), before + 1, "with the reason");

    // Proposed again meanwhile, the same.
    f.board.select(k(redis));
    f.press(KeyCode::Char('Y'));
    f.type_text("fine after all");
    kb(&f).resolve(redis, DecisionState::Proposed, "reworked").unwrap();
    f.board.reload().unwrap();
    saving(&mut f);
    assert_eq!(
        f.board.message(),
        Some(
            format!("#{redis} was proposed again meanwhile: save again to accept it anyway, ctrl+d to leave it")
                .as_str()
        )
    );
    assert_eq!(f.on_disk(redis).decision, DecisionState::Proposed);
    saving(&mut f);
    assert_eq!(f.on_disk(redis).decision, DecisionState::Accepted);

    // Given the same verdict meanwhile: nothing a save can give, however many times, but the text typed is the user's
    // to copy.
    for (key, verdict, word) in [('j', DecisionState::Rejected, "reason"), ('Y', DecisionState::Accepted, "comment")] {
        kb(&f).resolve(kafka, DecisionState::Proposed, "again").unwrap();
        f.board.reload().unwrap();
        f.board.select(k(kafka));
        f.press(KeyCode::Char(key));
        f.type_text("mine");
        kb(&f).resolve(kafka, verdict, "theirs").unwrap();
        f.board.reload().unwrap();
        let before = said(&f, kafka);
        for _ in 0..2 {
            saving(&mut f);
            assert!(matches!(f.board.mode, Mode::Edit(_)));
            assert_eq!(
                f.board.message(),
                Some(format!("#{kafka} was {} meanwhile: copy your {word}, then ctrl+d", verdict.as_str()).as_str())
            );
        }
        assert_eq!(said(&f, kafka), before, "the verdict was theirs");
        f.board.discard_edit();
    }

    // Given it, then proposed again: the save stops again, saying so, rather than giving the reason over the new
    // proposal unseen.
    kb(&f).resolve(kafka, DecisionState::Proposed, "again").unwrap();
    f.board.reload().unwrap();
    f.board.select(k(kafka));
    f.press(KeyCode::Char('j'));
    f.type_text("mine");
    kb(&f).resolve(kafka, DecisionState::Rejected, "theirs").unwrap();
    f.board.reload().unwrap();
    saving(&mut f);
    kb(&f).resolve(kafka, DecisionState::Proposed, "reworked").unwrap();
    f.board.reload().unwrap();
    saving(&mut f);
    assert_eq!(
        f.board.message(),
        Some(
            format!("#{kafka} was proposed again meanwhile: save again to reject it anyway, ctrl+d to leave it")
                .as_str()
        )
    );
    assert_eq!(f.on_disk(kafka).decision, DecisionState::Proposed, "not rejected unseen");
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
fn a_comment_is_edited_and_deleted_from_the_details() {
    let mut f = Fixture::new(&["feature"]);
    f.board.brain_mut(0).add_comment(1, "first try").unwrap();
    f.board.brain_mut(0).add_comment(1, "second try").unwrap();
    f.board.open_editor(Target::Comment(k(1), 0));
    f.press(KeyCode::End);
    f.type_text(", failed");
    f.press(KeyCode::Esc);
    let bodies = |f: &Fixture| f.on_disk(1).comments.iter().map(|c| c.body.clone()).collect::<Vec<_>>();
    assert_eq!(bodies(&f), ["first try, failed", "second try"]);
    assert!(f.on_disk(1).comments[0].edited.is_some(), "it says it was edited");
    f.board.ask_delete_comment(k(1), 0);
    f.press(KeyCode::Char('n'));
    assert_eq!(bodies(&f).len(), 2, "n keeps it");
    f.board.ask_delete_comment(k(1), 0);
    f.press(KeyCode::Char('y'));
    assert_eq!(bodies(&f), ["second try"]);
}

#[test]
fn deleting_a_comment_deletes_the_one_asked_about_wherever_it_is_now() {
    use kanbrain_core::brain::Brain;
    use kanbrain_core::model::Comment;
    // What `kb` (or a hand, or a pull) does between the question and its answer.
    type Meanwhile = fn(&mut Brain);
    let mut f = Fixture::new(&["feature"]);
    let bodies = |f: &Fixture| f.on_disk(1).comments.iter().map(|c| c.body.clone()).collect::<Vec<_>>();
    let cases: [(Meanwhile, &[&str], Option<&str>); 3] = [
        // One before it gone: another is at its index now.
        (|kb| kb.delete_comment(1, 0).unwrap(), &["c"], Some("comment deleted")),
        (
            |kb| _ = kb.update_comment(1, 1, "b, edited").unwrap(),
            &["a", "b, edited", "c"],
            Some("not deleted: #1's comment was edited or deleted meanwhile"),
        ),
        (
            |kb| kb.delete_comment(1, 1).unwrap(),
            &["a", "c"],
            Some("not deleted: #1's comment was edited or deleted meanwhile"),
        ),
    ];
    // Emptied meanwhile: nothing there to ask about, and the list stays.
    f.board.brain_mut(0).add_comment(1, "only").unwrap();
    f.press(KeyCode::Char('C'));
    Brain::open(Store::at(f.dir.path()).unwrap()).unwrap().delete_comment(1, 0).unwrap();
    f.board.reload().unwrap();
    f.press(KeyCode::Char('x'));
    assert!(matches!(f.board.mode, Mode::Comments(_)), "{:?}", f.board.mode);
    f.press(KeyCode::Esc);
    for (meanwhile, left, said) in cases {
        let mut task = f.on_disk(1);
        task.comments = ["a", "b", "c"].map(|body| Comment::new(now(), body.into(), Author::Human)).to_vec();
        Store::at(f.dir.path()).unwrap().save(&mut task).unwrap();
        f.board.reload().unwrap();
        f.press(KeyCode::Char('C'));
        f.press(KeyCode::Up);
        f.press(KeyCode::Char('x'));
        meanwhile(&mut Brain::open(Store::at(f.dir.path()).unwrap()).unwrap());
        f.board.reload().unwrap();
        f.board.message = None;
        f.press(KeyCode::Char('y'));
        assert_eq!((bodies(&f), f.board.message()), (left.iter().map(ToString::to_string).collect(), said));
    }
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
fn a_card_changed_by_another_program_reads_again_with_everything() {
    use kanbrain_core::projects;
    let (central, _code) = projects();
    let mut board = Board::open(central.path(), None).unwrap();
    projects::write_description(central.path(), "shop", "A bigger shop.").unwrap();
    board.reload_changed(&[projects::cards_dir(central.path())]).unwrap();
    let shop = board.repos_of("shop").next().unwrap();
    assert_eq!(board.brain(shop).project.as_ref().unwrap().description, "A bigger shop.");
}

#[test]
fn a_shown_project_follows_command_line_changes_seen_by_the_watcher() {
    let (central, code) = projects();
    let mut board = Board::open(central.path(), None).unwrap();
    board.set_scope(Scope::Project("shop".into()));
    kanbrain_core::cli::rename_project(central.path(), "shop", "store").unwrap();
    // As the watcher hands it over: the cards' folder changed.
    board.reload_changed(&[kanbrain_core::projects::cards_dir(central.path())]).unwrap();
    assert_eq!(board.scope, Scope::Project("store".into()), "renamed");

    // A one-repository project's repository moved to a project of its own is the same project renamed.
    board.set_scope(Scope::Project("blog".into()));
    kanbrain_core::cli::move_repo(central.path(), &code.path().join("blog/.brain"), "journal").unwrap();
    board.reload().unwrap();
    assert_eq!(board.scope, Scope::Project("journal".into()));

    // Split across two projects before the board looks again: neither is it.
    board.set_scope(Scope::Project("store".into()));
    kanbrain_core::cli::move_repo(central.path(), &code.path().join("web/.brain"), "front").unwrap();
    kanbrain_core::cli::move_repo(central.path(), &code.path().join("api/.brain"), "back").unwrap();
    board.reload().unwrap();
    assert_eq!(board.scope, Scope::All, "store is gone, split in two");
}

#[test]
fn a_folder_of_another_project_added_to_one_is_said_moved() {
    let (central, code) = projects();
    let mut board = Board::open(central.path(), None).unwrap();
    board.name_project(code.path().join("web"), None);
    (0..10).for_each(|_| press(&mut board, KeyCode::Backspace));
    "blog".chars().for_each(|c| press(&mut board, KeyCode::Char(c)));
    press(&mut board, KeyCode::Enter);
    let said = board.message().unwrap();
    assert!(said.starts_with("moved web from project shop to blog"), "{said}");
}

#[test]
fn a_folder_added_from_manage_project_joins_that_project_though_the_board_changed_meanwhile() {
    let (central, code) = projects();
    let mut board = Board::open(central.path(), None).unwrap();
    let blog = board.sources.iter().position(|s| s.label.as_deref() == Some("blog")).unwrap();
    board.add_repo_to(blog);
    let Some(Request::Folder(from)) = board.request.take() else { panic!("asks for the folder") };
    // While the system's folder dialog is open, `kb deinit` in another shell takes out a repository before it.
    let api = code.path().join("api/.brain");
    kanbrain_core::projects::unregister(central.path(), &api).unwrap();
    board.reload().unwrap();
    let mobile = code.path().join("mobile");
    std::fs::create_dir(&mobile).unwrap();
    board.name_project(mobile, from.as_deref());
    let Mode::AddProject(add) = &board.mode else { panic!("the dialog is open") };
    assert_eq!(board.add_project_options(&add.picker), [ProjectOption::Existing("blog".into())]);
}

#[test]
fn the_forms_project_list_keeps_its_highlight_on_the_same_brain_when_the_board_is_read_again() {
    let (central, code) = projects();
    let mut board = Board::open(central.path(), None).unwrap();
    board.selected = Some(Key::new(3, 1));
    press(&mut board, KeyCode::Char('n'));
    press(&mut board, KeyCode::Up);
    // An agent's `kb deinit` takes out a repository listed before blog.
    kanbrain_core::projects::unregister(central.path(), &code.path().join("api/.brain")).unwrap();
    board.reload().unwrap();
    let Mode::Form(form) = &board.mode else { panic!("the form is still open: {:?}", board.mode) };
    let highlighted = board.form_projects(form)[form.project.as_ref().unwrap().sel].1.clone();
    assert_eq!(highlighted, "blog");
}

#[test]
fn a_decision_already_decided_is_named_with_its_repository() {
    use kanbrain_core::brain::{Brain, NewEntry};
    let (central, code) = projects();
    let api = code.path().join("api/.brain");
    let mut brain = Brain::open(Store::at(&api).unwrap()).unwrap();
    let new = |title: &str| NewEntry { kind: Kind::Decision, title: title.into(), ..NewEntry::default() };
    let rejected = brain.add(new("Use Redis")).unwrap();
    brain.resolve(rejected, DecisionState::Rejected, "too heavy").unwrap();
    let accepted = brain.add(new("Use Postgres")).unwrap();
    brain.resolve(accepted, DecisionState::Accepted, "").unwrap();
    let mut board = Board::open(central.path(), None).unwrap();
    // By folder: `/var` is `/private/var` on macOS, and Windows prefixes `\\?\`.
    let src = board.sources.iter().position(|s| kanbrain_core::projects::same_dir(s.brain.store.dir(), &api)).unwrap();
    board.selected = Some(Key::new(src, rejected));
    board.reject_selected();
    assert_eq!(board.message(), Some(format!("shop/api #{rejected} is already rejected").as_str()));
    board.selected = Some(Key::new(src, accepted));
    board.accept_with_comment();
    assert_eq!(board.message(), Some(format!("shop/api #{accepted} is already accepted").as_str()));
}

#[test]
fn whats_skipped_when_the_window_opens_is_replaced_by_the_first_keys_message() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("0009-broken.md"), "# no id\n").unwrap();
    let mut board = Board::open(dir.path(), None).unwrap();
    assert!(board.message().unwrap().contains("0009-broken.md"), "{:?}", board.message());
    board.notify("undone");
    assert_eq!(board.message(), Some("undone"));
}

#[test]
fn whats_skipped_is_said_with_its_repository_with_what_follows_and_again_on_r() {
    let (central, code) = projects();
    let mut board = Board::open(central.path(), None).unwrap();
    // The same file broken in two repositories: each is said.
    for repo in ["api", "web"] {
        std::fs::write(code.path().join(repo).join(".brain/0009-broken.md"), "# no id\n").unwrap();
    }
    board.reload().unwrap();
    // Said beside what the action that read the board says, not replaced by it.
    board.notify("undone");
    let said = board.message().unwrap().to_string();
    assert!(said.starts_with("undone"), "{said}");
    assert!(said.contains("shop/api: 0009-broken.md") && said.contains("shop/web: 0009-broken.md"), "{said}");
    board.message = None;
    board.reload().unwrap();
    assert_eq!(board.message(), None, "said already");
    // R lists what's still skipped.
    board.reload_from_disk();
    assert!(board.message().unwrap().contains("0009-broken.md"), "{:?}", board.message());
}

#[test]
fn an_edit_saves_only_what_was_changed_in_the_form_and_keeps_what_kb_changed_meanwhile() {
    use kanbrain_core::brain::Brain;
    let mut f = Fixture::new(&["Fix login", "Other", "Third"]);
    let kb = |f: &Fixture| Brain::open(Store::at(f.dir.path()).unwrap()).unwrap();
    let mut brain = kb(&f);
    brain.update(1, "Fix login", vec!["auth".into(), "wip".into()]).unwrap();
    brain.link(1, &[3], &[]).unwrap();
    f.board.reload().unwrap();
    let related = |f: &Fixture| {
        let mut keys = f.board.related_keys(k(1));
        keys.sort_by_key(|k| k.id);
        keys
    };
    let tags = |f: &Fixture| f.on_disk(1).tags;

    // Saved untouched while an agent renamed it, retagged it and changed what it's related to: nothing of theirs
    // is undone.
    f.board.select(k(1));
    f.press(KeyCode::Char('e'));
    let mut brain = kb(&f);
    brain.update(1, "Fix the SSO login", vec!["bug".into(), "auth".into()]).unwrap();
    brain.link(1, &[2], &[]).unwrap();
    brain.toggle_link(1, 3).unwrap();
    f.board.reload().unwrap();
    f.board.message = None;
    f.press(KeyCode::Enter);
    assert!(matches!(f.board.mode, Mode::Normal));
    assert_eq!(f.board.message(), None, "nothing of the form's to save: nothing said");
    assert_eq!(f.on_disk(1).title, "Fix the SSO login");
    assert_eq!(tags(&f), ["bug", "auth"], "theirs, in their order");
    assert_eq!(related(&f), [k(2)]);

    // What the form changed is saved, over what the agent did to the same field; the rest of theirs stays.
    f.press(KeyCode::Char('e'));
    let mut brain = kb(&f);
    brain.update(1, "Fix SSO", vec!["bug".into(), "auth".into(), "db".into()]).unwrap();
    brain.link(1, &[3], &[]).unwrap();
    f.board.reload().unwrap();
    f.type_text(" now");
    f.press(KeyCode::Tab);
    f.board.untag_in_form("auth");
    f.type_text("ui");
    // #2 unchecked in the Related field.
    let Mode::Form(form) = &f.board.mode else { panic!("the form is open") };
    let at = f.board.form_link_options(form).iter().position(|&(key, _)| key == k(2)).unwrap();
    f.board.toggle_form_link(at);
    // Both changed the title: the first save stops and says what it is now, saving nothing; the second replaces it.
    f.board.message = None;
    f.press(KeyCode::Enter);
    assert!(matches!(f.board.mode, Mode::Form(_)), "kept open with what was typed");
    assert_eq!(
        f.board.message(),
        Some("#1's title changed meanwhile, to “Fix SSO”: save again to keep yours, ctrl+t to take it")
    );
    assert_eq!((f.on_disk(1).title.as_str(), tags(&f).len()), ("Fix SSO", 3), "nothing saved");
    f.press(KeyCode::Enter);
    assert_eq!(f.on_disk(1).title, "Fix the SSO login now", "the title typed");
    assert_eq!(tags(&f), ["bug", "ui", "db"], "auth taken out in the form, db added by the agent");
    assert_eq!(related(&f), [k(3)], "#2 unrelated in the form, #3 related by the agent");

    // ctrl+t takes theirs instead, which then saves as the agent left it.
    let ctrl_t = |f: &mut Fixture| f.board.on_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL));
    f.press(KeyCode::Char('e'));
    ctrl_t(&mut f);
    assert!(
        matches!(&f.board.mode, Mode::Form(form) if form.title.text() == "Fix the SSO login now"),
        "nothing to take"
    );
    f.type_text("!");
    f.press(KeyCode::Tab);
    f.press(KeyCode::End);
    f.type_text(", api");
    kb(&f).update(1, "Fix SSO", vec!["bug".into(), "ui".into(), "db".into()]).unwrap();
    f.board.reload().unwrap();
    f.press(KeyCode::Enter);
    assert!(matches!(&f.board.mode, Mode::Form(form) if form.theirs_title.as_deref() == Some("Fix SSO")));
    // Renamed again before it's taken: the form shows, and takes, the title as it is now.
    kb(&f).update(1, "Fix SSO 2", vec!["bug".into(), "ui".into(), "db".into()]).unwrap();
    f.board.reload().unwrap();
    assert!(matches!(&f.board.mode, Mode::Form(form) if form.theirs_title.as_deref() == Some("Fix SSO 2")));
    kb(&f).update(1, "Fix SSO", vec!["bug".into(), "ui".into(), "db".into()]).unwrap();
    f.board.reload().unwrap();
    ctrl_t(&mut f);
    assert!(matches!(&f.board.mode, Mode::Form(form) if form.title.text() == "Fix SSO" && form.theirs_title.is_none()
            && form.field == Field::Title));
    f.press(KeyCode::Enter);
    assert!(matches!(f.board.mode, Mode::Normal));
    assert_eq!(f.on_disk(1).title, "Fix SSO");
    assert_eq!(tags(&f), ["bug", "ui", "db", "api"], "the form's other changes saved");
}

#[test]
fn an_edit_forms_entry_as_opened_follows_its_brain_when_the_list_of_brains_changes() {
    use kanbrain_core::brain::Brain;
    let (central, code) = projects();
    let blog = code.path().join("blog/.brain");
    let mut brain = Brain::open(Store::at(&blog).unwrap()).unwrap();
    let new = |title: &str| kanbrain_core::brain::NewEntry { title: title.into(), ..Default::default() };
    let (second, third) = (brain.add(new("draft")).unwrap(), brain.add(new("photos")).unwrap());
    brain.link(1, &[second], &[]).unwrap();
    let mut board = Board::open(central.path(), None).unwrap();
    let src = |board: &Board| board.sources.iter().position(|s| s.label.as_deref() == Some("blog")).unwrap();
    board.select(Key::new(src(&board), 1));
    press(&mut board, KeyCode::Char('e'));
    // An agent relates it to another entry instead, and a repository listed before blog leaves: blog's index moves.
    let mut brain = Brain::open(Store::at(&blog).unwrap()).unwrap();
    brain.toggle_link(1, second).unwrap();
    brain.link(1, &[third], &[]).unwrap();
    kanbrain_core::projects::unregister(central.path(), &code.path().join("api/.brain")).unwrap();
    board.reload().unwrap();
    press(&mut board, KeyCode::Enter);
    let related: Vec<_> = board.related_keys(Key::new(src(&board), 1)).iter().map(|k| k.id).collect();
    assert_eq!(related, [third], "saved untouched: the agent's links kept");
}

#[test]
fn an_edit_whose_links_fail_isnt_said_saved() {
    let mut f = Fixture::new(&["Fix login"]);
    f.board.select(k(1));
    f.press(KeyCode::Char('e'));
    f.type_text(" now");
    // An entry deleted meanwhile, among the related.
    let Mode::Form(form) = &mut f.board.mode else { panic!("the form is open") };
    form.links.push(k(99));
    f.press(KeyCode::Enter);
    assert_ne!(f.board.message(), Some("#1 saved"), "the link failed: that's what's said");
}

#[test]
fn a_new_note_isnt_started_whatever_column_the_form_was_opened_from() {
    let mut f = Fixture::new(&[]);
    f.board.switch_tab(Tab::Notes);
    f.board.new_entry(Status::Doing);
    f.type_text("Webhooks repeat");
    f.press(KeyCode::Enter);
    assert_eq!(f.board.message(), Some("created #1"), "no status to refuse");
}

#[test]
fn removing_one_of_a_projects_repositories_keeps_the_project_shown() {
    let (central, _code) = projects();
    let mut board = Board::open(central.path(), None).unwrap();
    board.set_scope(Scope::Project("shop".into()));
    let web = board.sources.iter().position(|s| s.label.as_deref() == Some("shop/web")).unwrap();
    board.ask_remove(web, false);
    press(&mut board, KeyCode::Char('y'));
    assert_eq!(board.scope, Scope::Project("shop".into()), "shop still has api");
}

#[test]
fn the_add_project_dialog_goes_back_to_its_repository_after_a_reload() {
    let (central, code) = projects();
    let mut board = Board::open(central.path(), None).unwrap();
    let blog = board.sources.iter().position(|s| s.label.as_deref() == Some("blog")).unwrap();
    let blog_dir = board.sources[blog].brain.store.dir().to_path_buf();
    let mobile = code.path().join("mobile");
    std::fs::create_dir(&mobile).unwrap();
    board.name_project(mobile, Some(&blog_dir));
    kanbrain_core::projects::unregister(central.path(), &code.path().join("api/.brain")).unwrap();
    board.reload().unwrap();
    let Mode::AddProject(add) = &board.mode else { panic!("still open: {:?}", board.mode) };
    assert_eq!(add.back, board.sources.iter().position(|s| s.brain.store.dir() == blog_dir));
}
