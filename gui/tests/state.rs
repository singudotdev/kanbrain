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
        self.board.on_key(KeyEvent::new(code, KeyModifiers::NONE));
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
            "Link…",
            "Open file",
            "Delete…"
        ],
        "no move to its own status, and no Move… with one brain"
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
    assert!(matches!(f.board.mode, Mode::Normal), "f is the board's, for done tasks; not a second tag filter");
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
    assert!(matches!(f.board.mode, Mode::Next(_)), "stopping the running task asks what's next");
    f.press(KeyCode::Esc);
    f.shift(KeyCode::Right);
    assert_eq!(f.status(1), Status::Done);
    f.shift(KeyCode::Right);
    assert_eq!(f.status(1), Status::Done, "done is the last column");
    let steps: Vec<_> = f.on_disk(1).history.iter().map(|c| (c.from, c.to)).collect();
    assert_eq!(steps, [(Status::Todo, Status::Doing), (Status::Doing, Status::Parked), (Status::Parked, Status::Done)]);
    assert_eq!(f.board.selected, Some(k(1)), "the card stays selected as it moves");
}

#[test]
fn a_brain_left_with_nothing_running_asks_whats_next() {
    let mut f = Fixture::new(&["feature", "bug", "docs"]);
    f.board.set_status(k(1), Status::Doing);
    f.board.set_status(k(1), Status::Parked);
    let Mode::Next(_) = &f.board.mode else { panic!("#1 stopped and nothing runs: it asks") };
    f.press(KeyCode::Esc);
    f.board.set_status(k(2), Status::Doing);
    f.board.set_status(k(2), Status::Done);
    let Mode::Next(picker) = &f.board.mode else { panic!("#2 stopped and nothing runs: it asks") };
    let choices = f.board.next_choices(picker);
    assert_eq!(
        choices,
        [NextChoice::Start(k(1)), NextChoice::Start(k(3)), NextChoice::New, NextChoice::Nothing],
        "parked #1 first, then to-do #3"
    );
    f.type_text("do");
    let Mode::Next(picker) = &f.board.mode else { panic!("typing searches") };
    let choices = f.board.next_choices(picker);
    assert_eq!(choices, [NextChoice::Start(k(3)), NextChoice::New, NextChoice::Nothing], "docs; new and nothing stay");
    f.type_text("n");
    assert!(matches!(&f.board.mode, Mode::Next(p) if p.search.text() == "don"), "n is a letter once searching");
    for _ in 0..3 {
        f.press(KeyCode::Backspace);
    }
    f.press(KeyCode::Enter);
    assert!(f.board.brain(0).task(1).unwrap().is_running(), "Enter resumes the parked one");

    f.board.set_status(k(1), Status::Parked);
    f.press(KeyCode::Char('n'));
    f.type_text("hotfix");
    f.press(KeyCode::Enter);
    let running: Vec<_> = f.board.brain(0).running().map(|t| t.title.as_str()).collect();
    assert_eq!(running, ["hotfix"], "n creates a task there, already started");

    f.board.open_entry(k(4));
    f.press(KeyCode::Char('x'));
    f.press(KeyCode::Char('y'));
    assert!(matches!(f.board.mode, Mode::Next(_)), "deleting the running task asks too");
    f.press(KeyCode::Esc);
    assert!(matches!(f.board.mode, Mode::Normal), "Esc: nothing for now");
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
    f.board.set_status(k(1), Status::Parked);
    assert!(matches!(f.board.mode, Mode::Next(_)), "what's next is about yours, whatever the agent runs");
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
    let task = f.on_disk(1);
    assert_eq!(task.comments[0].body, "found it");
    assert_eq!(task.description, "", "discarded");
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
    f.type_text("u");
    assert!(f.board.task(k(2)).is_none(), "and redone");

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
    let key = |board: &mut Board, code| board.on_key(KeyEvent::new(code, KeyModifiers::NONE));
    key(&mut board, KeyCode::Char('P'));
    let Mode::Scopes(picker) = &board.mode else { panic!("P opens the project dropdown") };
    assert_eq!(board.scope_candidates(picker)[picker.sel].1, "shop");
    "api".chars().for_each(|c| key(&mut board, KeyCode::Char(c)));
    let Mode::Scopes(picker) = &board.mode else { panic!("typing keeps it open") };
    let names: Vec<String> = board.scope_candidates(picker).into_iter().map(|(_, n)| n).collect();
    assert_eq!(names, ["shop/api"]);
    key(&mut board, KeyCode::Enter);
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
    board.on_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE));
    "cart".chars().for_each(|c| board.on_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)));
    board.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(board.selected, Some(Key::new(2, 2)), "#2 in web, though api has a #1 too");
    assert_eq!(board.brain(2).task(2).unwrap().title, "cart");
}

#[test]
fn a_new_entry_goes_where_the_filter_points_and_its_form_picks_another_project() {
    let (central, _code) = projects();
    let mut board = Board::open(central.path(), None).unwrap();
    let key = |board: &mut Board, code| board.on_key(KeyEvent::new(code, KeyModifiers::NONE));
    let form = |board: &Board| {
        let Mode::Form(form) = &board.mode else { panic!("the form is open: {:?}", board.mode) };
        (form.src(), form.field, board.form_projects(form).into_iter().map(|(_, n)| n).collect::<Vec<_>>())
    };
    board.selected = Some(Key::new(3, 1));
    key(&mut board, KeyCode::Char('n'));
    assert_eq!(form(&board).0, 3, "All: the selected card's brain, with no question first");
    key(&mut board, KeyCode::Up);
    let (_, field, names) = form(&board);
    assert_eq!(field, Field::Project, "the project field comes before the title");
    assert_eq!(names, ["Central", "shop/api", "shop/web", "blog"], "every brain");
    let Mode::Form(open) = &board.mode else { unreachable!() };
    assert_eq!(open.project.as_ref().unwrap().sel, 3, "highlighting the form's own");
    "web".chars().for_each(|c| key(&mut board, KeyCode::Char(c)));
    assert_eq!(form(&board).2, ["shop/web"]);
    key(&mut board, KeyCode::Enter);
    assert_eq!(form(&board).0, 2, "enter picks it");
    assert_eq!(form(&board).1, Field::Title, "and goes back to the title");
    "cart".chars().for_each(|c| key(&mut board, KeyCode::Char(c)));
    key(&mut board, KeyCode::Enter);
    assert_eq!(board.brain(2).task(2).unwrap().title, "cart");

    board.set_scope(Scope::Project("blog".into()));
    board.selected = None;
    key(&mut board, KeyCode::Char('n'));
    assert_eq!(form(&board).0, 3, "a one-repository project: its brain");
    let Mode::Form(open) = &mut board.mode else { unreachable!() };
    open.links.push(Key::new(3, 1));
    board.choose_form_project(0);
    let Mode::Form(open) = &board.mode else { unreachable!() };
    assert_eq!(open.src(), 0, "anywhere, whatever the scope");
    assert!(open.links.is_empty(), "they're picked among its brain's and project's, so they're dropped");
    key(&mut board, KeyCode::Esc);

    board.set_scope(Scope::Project("shop".into()));
    key(&mut board, KeyCode::Char('a'));
    let Mode::Where(asked) = &board.mode else { panic!("a asks where: {:?}", board.mode) };
    let names: Vec<String> = board.where_candidates(&asked.picker, asked.then).into_iter().map(|(_, n)| n).collect();
    assert_eq!(names, ["shop/api", "shop/web"], "quick add, with no form, asks among the repositories");
    key(&mut board, KeyCode::Esc);
    assert!(matches!(board.mode, Mode::Normal), "Esc creates nothing");
    board.set_scope(Scope::Central);
    key(&mut board, KeyCode::Char('a'));
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
    board.on_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
    board.on_key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));
    assert!(board.brain(1).task(1).is_none());
    assert!(board.brain(2).task(1).unwrap().repo_links.is_empty(), "web's link to it went with it");
    let file = std::fs::read_to_string(board.brain(2).task(1).unwrap().path.as_ref().unwrap()).unwrap();
    assert!(!file.contains("- links:"), "on disk too");
}

#[test]
fn an_entry_moves_to_another_project_and_is_renumbered_there() {
    let (central, _code) = projects();
    let mut board = Board::open(central.path(), None).unwrap();
    let key = |board: &mut Board, code| board.on_key(KeyEvent::new(code, KeyModifiers::NONE));
    board.set_scope(Scope::Project("shop".into()));
    board.selected = Some(Key::new(1, 1));
    key(&mut board, KeyCode::Char('M'));
    let Mode::Where(asked) = &board.mode else { panic!("M asks where: {:?}", board.mode) };
    let names: Vec<String> = board.where_candidates(&asked.picker, asked.then).into_iter().map(|(_, n)| n).collect();
    assert_eq!(names, ["Central", "shop/web", "blog"], "every other brain, whatever the scope");
    "web".chars().for_each(|c| key(&mut board, KeyCode::Char(c)));
    key(&mut board, KeyCode::Enter);
    assert!(board.brain(1).task(1).is_none(), "gone from api");
    assert_eq!(board.brain(2).task(2).unwrap().title, "rate limit", "web's next id");
    assert_eq!(board.selected, Some(Key::new(2, 2)), "still shown in shop: it stays selected");

    key(&mut board, KeyCode::Char('M'));
    "blog".chars().for_each(|c| key(&mut board, KeyCode::Char(c)));
    key(&mut board, KeyCode::Enter);
    assert_eq!(board.brain(3).task(2).unwrap().title, "rate limit");
    assert_eq!(board.scope, Scope::Project("shop".into()), "the filter stays as it was");
    assert!(board.message().unwrap().contains("moved #2 to blog as #2"), "{:?}", board.message());
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
    board.on_key(KeyEvent::new(KeyCode::Char('!'), KeyModifiers::NONE));
    board.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(board.brain(2).project.as_ref().unwrap().description, "An online shop.!", "web sees api's edit");
    let report = board.report();
    assert!(report.rows.is_empty(), "nothing tracked yet");
}

#[test]
fn project_info_opens_for_a_shown_project() {
    let (central, _code) = projects();
    let mut board = Board::open(central.path(), None).unwrap();
    let key = |board: &mut Board, code| board.on_key(KeyEvent::new(code, KeyModifiers::NONE));
    board.selected = Some(Key { src: 0, id: 1 });
    key(&mut board, KeyCode::Char('I'));
    assert!(matches!(board.mode, Mode::Normal), "All shows no single project, and the central brain isn't one");
    assert!(board.message().is_some_and(|m| m.contains("select one of its cards")));
    board.selected = Some(Key { src: 2, id: 1 });
    key(&mut board, KeyCode::Char('I'));
    assert!(matches!(board.mode, Mode::ProjectInfo(2)), "the selected card's project");
    key(&mut board, KeyCode::Esc);
    board.set_scope(Scope::Repo(2));
    key(&mut board, KeyCode::Char('I'));
    assert!(matches!(board.mode, Mode::ProjectInfo(2)));
    assert_eq!(board.project_repos(2), [1, 2], "it lists every repository of shop");
    key(&mut board, KeyCode::Esc);
    assert!(matches!(board.mode, Mode::Normal));
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
    let press = |board: &mut Board, code| board.on_key(KeyEvent::new(code, KeyModifiers::NONE));
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
    assert_eq!(f.board.form_tags(form), (EntryKind::Task, vec!["auth".to_string()]), "a task's: no #stripe");
    f.press(KeyCode::Esc);
    f.board.switch_tab(Tab::Notes);
    f.press(KeyCode::Char('e'));
    let Mode::Form(form) = &f.board.mode else { panic!("e edits the note") };
    assert_eq!(f.board.form_tags(form), (EntryKind::Note, vec!["stripe".to_string()]), "a note's: no #auth");
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
    let keys: Vec<&str> = f.board.commands(picker).iter().map(|c| c.key).collect();
    assert!(keys.contains(&"n") && !keys.contains(&"s"), "with no card selected, a card's commands are left out");
    f.press(KeyCode::Esc);
    f.board.select(k(1));
    f.press(KeyCode::Char('?'));
    f.type_text("start");
    let Mode::Palette(picker) = &f.board.mode else { panic!("typing filters") };
    let keys: Vec<&str> = f.board.commands(picker).iter().map(|c| c.key).collect();
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
    assert_eq!(f.board.commands(picker).first().map(|c| c.key), Some("e"), "with the report's own commands");
}

#[test]
fn a_picked_folder_becomes_a_project_as_sb_init_makes_it() {
    use kanbrain_core::projects;
    let (central, code) = projects();
    let mut board = Board::open(central.path(), None).unwrap();
    let key = |board: &mut Board, code| board.on_key(KeyEvent::new(code, KeyModifiers::NONE));
    let options = |board: &Board| {
        let Mode::AddProject(add) = &board.mode else { panic!("the dialog is open") };
        board.add_project_options(&add.picker)
    };
    key(&mut board, KeyCode::Char('A'));
    assert_eq!(board.request.take(), Some(Request::Folder), "A asks the window for the folder dialog");

    let notes = code.path().join("notes");
    std::fs::create_dir_all(&notes).unwrap();
    board.name_project(notes.clone());
    assert_eq!(options(&board), [ProjectOption::New("notes".into())], "named after the folder to begin with");
    // The name starts selected, so typing replaces it.
    "journal".chars().for_each(|c| key(&mut board, KeyCode::Char(c)));
    assert_eq!(options(&board), [ProjectOption::New("journal".into())]);
    key(&mut board, KeyCode::Enter);
    assert!(matches!(board.mode, Mode::Normal));
    assert_eq!(board.scope, Scope::Project("journal".into()), "the new project is shown");
    assert!(notes.join(".brain").is_dir());
    assert!(std::fs::read_to_string(notes.join("CLAUDE.md")).unwrap().contains("kanbrain:start"));
    let repos = projects::load(central.path()).unwrap();
    assert!(repos.iter().any(|r| r.project == "journal"), "registered in the central brain");

    // A repository's folder offers its own project, which there is, so no new one.
    board.name_project(code.path().join("api"));
    assert_eq!(options(&board), [ProjectOption::Existing("shop".into())]);
    // Nothing typed lists every project; typing searches them, after a new one of that name.
    let admin = code.path().join("admin");
    std::fs::create_dir_all(&admin).unwrap();
    board.name_project(admin);
    key(&mut board, KeyCode::Backspace);
    let names: Vec<String> = options(&board).iter().map(|o| o.name().to_string()).collect();
    assert_eq!(names, ["shop", "blog", "journal"]);
    "sh".chars().for_each(|c| key(&mut board, KeyCode::Char(c)));
    assert_eq!(options(&board), [ProjectOption::New("sh".into()), ProjectOption::Existing("shop".into())]);
    key(&mut board, KeyCode::Down);
    key(&mut board, KeyCode::Enter);
    assert_eq!(board.project_repos(board.project_src().unwrap()).len(), 3, "api, web and admin");
    assert!(board.message().unwrap().contains("to project `shop`"));
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

    f.board.open_entry(k(decision));
    let labels: Vec<&str> = f.board.menu_items(k(decision)).iter().map(|i| i.label).collect();
    assert!(labels.contains(&"Accept") && labels.contains(&"Reject…"), "{labels:?}");
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
    let locks = f.board.lock_brains();
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
