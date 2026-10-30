use super::*;

#[path = "perf.rs"]
mod perf;
use crate::state::{Board, Mode};
use gpui::MouseButton;
use kanbrain_core::model::{Author, Kind, Status, Task, now};
use kanbrain_core::store::Store;

/// A window over a fresh folder holding tasks with these titles (ids from 1).
fn window<'a>(
    cx: &'a mut gpui::TestAppContext,
    titles: &[&str],
) -> (gpui::Entity<BrainView>, &'a mut gpui::VisualTestContext, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::at(dir.path()).unwrap();
    for (title, id) in titles.iter().zip(1..) {
        store.save(&mut Task::new(id, (*title).to_string(), now())).unwrap();
    }
    let board = Board::open(dir.path(), None).unwrap();
    let (view, cx) = cx.add_window_view(|window, cx| BrainView::new(board, window, cx));
    cx.run_until_parked();
    (view, cx, dir)
}

#[test]
fn keystrokes_become_board_keys() {
    let key = |s: &str| key_events(&Keystroke::parse(s).unwrap());
    let char = |c| vec![KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)];
    assert_eq!(key("n"), char('n'));
    assert_eq!(key("shift-l"), char('L'));
    assert_eq!(key("space"), char(' '));
    assert_eq!(key("escape"), [KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)]);
    assert_eq!(key("ctrl-c"), [KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)]);
    assert_eq!(key("shift-right"), [KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT)]);
    assert_eq!(key("shift-tab"), [KeyEvent::new(KeyCode::BackTab, KeyModifiers::NONE)]);
    assert_eq!(key("cmd-q"), Vec::new());
}

#[test]
fn shortcuts_are_named_for_tooltips() {
    assert_eq!(key_name(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE)), "n");
    assert_eq!(key_name(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE)), "Space");
    assert_eq!(key_name(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)), "Ctrl+C");
}

#[gpui::test]
fn keys_reach_the_board(cx: &mut gpui::TestAppContext) {
    let (view, cx, _dir) = window(cx, &["a"]);
    // "s" and "q" are board keys; inside the form they're just letters.
    cx.simulate_keystrokes("n s q enter");
    cx.simulate_keystrokes("s");
    view.read_with(cx, |view, _| {
        let running: Vec<_> = view.board.brain(0).running().map(|t| t.title.as_str()).collect();
        assert_eq!(running, ["sq"], "the new task is selected, and s started it");
    });
}

/// Clicks twice at `at`, as a double-click: the first click, then the second counted as such.
fn double_click(cx: &mut gpui::VisualTestContext, at: gpui::Point<gpui::Pixels>) {
    for click_count in [1, 2] {
        let (button, modifiers, first_mouse) = (MouseButton::Left, gpui::Modifiers::none(), false);
        cx.simulate_event(gpui::MouseDownEvent { position: at, button, modifiers, click_count, first_mouse });
        cx.simulate_event(gpui::MouseUpEvent { position: at, button, modifiers, click_count });
    }
}

#[gpui::test]
fn cards_select_on_click_and_open_on_double_click(cx: &mut gpui::TestAppContext) {
    let (view, cx, _dir) = window(cx, &["a", "b"]);
    let card = cx.debug_bounds("card 0/2").expect("card 2 is drawn").center();
    cx.simulate_click(card, gpui::Modifiers::none());
    view.read_with(cx, |view, _| {
        assert_eq!((view.board.selected, view.board.drawer), (Some(crate::state::Key::new(0, 2)), false));
    });
    double_click(cx, card);
    view.read_with(cx, |view, _| assert!(view.board.drawer, "double-click opens the details"));
    let doing = cx.debug_bounds("drawer status doing").expect("the drawer has status buttons").center();
    cx.simulate_click(doing, gpui::Modifiers::none());
    view.read_with(cx, |view, _| assert_eq!(view.board.brain(0).task(2).unwrap().status, Status::Doing));
}

#[gpui::test]
fn a_right_click_opens_the_cards_menu_and_its_items_act_on_it(cx: &mut gpui::TestAppContext) {
    let (view, cx, _dir) = window(cx, &["a", "b"]);
    let card = cx.debug_bounds("card 0/1").expect("card 1 is drawn").center();
    cx.simulate_mouse_down(card, MouseButton::Right, gpui::Modifiers::none());
    cx.simulate_mouse_up(card, MouseButton::Right, gpui::Modifiers::none());
    view.read_with(cx, |view, _| {
        assert!(matches!(view.board.mode, crate::state::Mode::Menu(_)), "{:?}", view.board.mode);
        assert_eq!(view.board.selected, Some(crate::state::Key::new(0, 1)), "it selects the card");
    });
    // Row 1 is "Start": the first status move a to-do task doesn't have.
    let start = cx.debug_bounds("menu 1").expect("the menu is drawn").center();
    cx.simulate_click(start, gpui::Modifiers::none());
    view.read_with(cx, |view, _| {
        assert!(matches!(view.board.mode, crate::state::Mode::Normal));
        assert_eq!(view.board.brain(0).task(1).unwrap().status, Status::Doing);
    });
}

#[gpui::test]
fn clipboard_shortcuts_copy_cut_and_paste(cx: &mut gpui::TestAppContext) {
    let (view, cx, _dir) = window(cx, &[]);
    let ctrl = if cfg!(target_os = "macos") { "cmd" } else { "ctrl" };
    cx.simulate_keystrokes("n a b");
    cx.simulate_keystrokes(&format!("{ctrl}-a {ctrl}-x"));
    assert_eq!(cx.read_from_clipboard().and_then(|item| item.text()).as_deref(), Some("ab"));
    cx.simulate_keystrokes(&format!("{ctrl}-v {ctrl}-v enter"));
    view.read_with(cx, |view, _| {
        let titles: Vec<&str> = view.board.brain(0).tasks.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(titles, ["abab"]);
    });
}

#[gpui::test]
fn dragging_a_card_onto_a_column_moves_it(cx: &mut gpui::TestAppContext) {
    let (view, cx, dir) = window(cx, &["a", "b"]);
    let card = cx.debug_bounds("card 0/1").expect("card 1 is drawn").center();
    let done = cx.debug_bounds("column done").expect("the done column is drawn").center();
    cx.simulate_mouse_down(card, MouseButton::Left, gpui::Modifiers::none());
    // Two moves: the first passes GPUI's drag threshold and starts the drag, the second hovers the target.
    cx.simulate_mouse_move(card + gpui::point(px(10.), px(10.)), MouseButton::Left, gpui::Modifiers::none());
    cx.simulate_mouse_move(done, MouseButton::Left, gpui::Modifiers::none());
    cx.simulate_mouse_up(done, MouseButton::Left, gpui::Modifiers::none());
    view.read_with(cx, |view, _| assert_eq!(view.board.selected, Some(crate::state::Key::new(0, 1))));
    let (tasks, _) = Store::at(dir.path()).unwrap().load_entries().unwrap();
    let moved = tasks.iter().find(|t| t.id == 1).unwrap();
    assert_eq!(moved.status, Status::Done, "the move is saved");
}

#[gpui::test]
fn the_list_layout_draws_rows_that_drag_onto_a_section(cx: &mut gpui::TestAppContext) {
    let (view, cx, dir) = window(cx, &["a", "b"]);
    cx.simulate_keystrokes("v");
    assert!(cx.debug_bounds("card 0/1").is_none(), "no columns in the list layout");
    let row = cx.debug_bounds("row 0/1").expect("task 1 is a row").center();
    let done = cx.debug_bounds("section done").expect("the done section is drawn").center();
    cx.simulate_mouse_down(row, MouseButton::Left, gpui::Modifiers::none());
    cx.simulate_mouse_move(row + gpui::point(px(10.), px(10.)), MouseButton::Left, gpui::Modifiers::none());
    cx.simulate_mouse_move(done, MouseButton::Left, gpui::Modifiers::none());
    cx.simulate_mouse_up(done, MouseButton::Left, gpui::Modifiers::none());
    let (tasks, _) = Store::at(dir.path()).unwrap().load_entries().unwrap();
    assert_eq!(tasks.iter().find(|t| t.id == 1).unwrap().status, Status::Done, "the move is saved");
    cx.simulate_keystrokes("v");
    assert!(cx.debug_bounds("card 0/1").is_some(), "v brings the columns back");
    view.read_with(cx, |view, _| assert_eq!(view.board.layout, crate::state::Layout::Kanban));
}

#[gpui::test]
fn a_columns_plus_opens_the_form_with_that_column_picked(cx: &mut gpui::TestAppContext) {
    let (view, cx, dir) = window(cx, &["a"]);
    let plus = cx.debug_bounds("add parked").expect("parked's + is drawn").center();
    cx.simulate_click(plus, gpui::Modifiers::none());
    view.read_with(cx, |view, _| {
        let crate::state::Mode::Form(form) = &view.board.mode else { panic!("+ opens the form") };
        assert_eq!(form.status(), Some(Status::Parked));
    });
    cx.simulate_keystrokes("x");
    let doing = cx.debug_bounds("form-status doing").expect("the column picker is drawn").center();
    cx.simulate_click(doing, gpui::Modifiers::none());
    cx.simulate_keystrokes("enter");
    let (tasks, _) = Store::at(dir.path()).unwrap().load_entries().unwrap();
    let new = tasks.iter().find(|t| t.title == "x").expect("created");
    assert_eq!(new.status, Status::Doing, "the column clicked in the picker");
}

#[gpui::test]
fn add_project_asks_for_a_folder_then_its_name(cx: &mut gpui::TestAppContext) {
    let (view, cx, _dir) = window(cx, &["a"]);
    let code = tempfile::tempdir().unwrap();
    let add = cx.debug_bounds("add-project-button").expect("shown while there are no projects").center();
    cx.simulate_click(add, gpui::Modifiers::none());
    assert!(cx.did_prompt_for_paths(), "the system's folder dialog opens");
    let folder = code.path().to_path_buf();
    cx.simulate_path_prompt_response(move |options| {
        assert!(options.directories && !options.files && !options.multiple, "one folder");
        Some(vec![folder])
    });
    cx.run_until_parked();
    view.read_with(cx, |view, _| {
        assert!(matches!(view.board.mode, crate::state::Mode::AddProject(_)), "then its name");
    });
    cx.simulate_keystrokes("enter");
    view.read_with(cx, |view, _| assert_eq!(view.board.sources.len(), 2, "the central brain and the new project"));
    assert!(code.path().join(".brain").is_dir());
}

#[gpui::test]
fn changes_made_by_other_programs_show_up(cx: &mut gpui::TestAppContext) {
    let (view, cx, dir) = window(cx, &["a"]);
    // What `kb done 1` from an agent does.
    let mut cli = kanbrain_core::brain::Brain::open(Store::at(dir.path()).unwrap()).unwrap();
    cli.set_status(1, Status::Done).unwrap();
    cx.executor().advance_clock(crate::watch::INTERVAL);
    cx.run_until_parked();
    view.read_with(cx, |view, _| {
        assert_eq!(view.board.brain(0).task(1).unwrap().status, Status::Done);
        assert_eq!(view.board.selected, Some(crate::state::Key::new(0, 1)), "the selection follows the card");
    });
}

#[gpui::test]
fn acting_right_after_another_program_keeps_its_change(cx: &mut gpui::TestAppContext) {
    let (view, cx, dir) = window(cx, &["a"]);
    // An agent comments, and the task is started before the watcher's next look.
    let mut cli = kanbrain_core::brain::Brain::open(Store::at(dir.path()).unwrap()).unwrap();
    cli.add_comment(1, "found the cause").unwrap();
    cx.simulate_keystrokes("s");
    let on_disk = kanbrain_core::brain::Brain::open(Store::at(dir.path()).unwrap()).unwrap();
    let task = on_disk.task(1).unwrap();
    assert_eq!((task.status, task.comments.len()), (Status::Doing, 1), "started, and the comment kept");
    view.read_with(cx, |view, _| assert_eq!(view.board.brain(0).task(1).unwrap().comments.len(), 1));
}

#[gpui::test]
fn the_calendar_opens_an_entry_from_its_interactions(cx: &mut gpui::TestAppContext) {
    let (view, cx, _dir) = window(cx, &["a", "b"]);
    cx.simulate_keystrokes("5 m");
    let today = chrono::Local::now().date_naive();
    // `debug_bounds` takes a `&'static str`.
    let selector = format!("day {today}").leak();
    let day = cx.debug_bounds(selector).expect("today is in the month").center();
    cx.simulate_click(day, gpui::Modifiers::none());
    view.read_with(cx, |view, _| assert_eq!(view.board.calendar.period.unit, kanbrain_core::period::Unit::Day));
    let event = cx.debug_bounds("event 0/2").expect("#2's creation is listed").center();
    cx.simulate_click(event, gpui::Modifiers::none());
    view.read_with(cx, |view, _| {
        assert_eq!(
            (view.board.tab, view.board.selected, view.board.drawer),
            (Tab::Calendar, Some(crate::state::Key::new(0, 2)), true)
        );
    });
}

#[gpui::test]
fn cards_with_the_same_id_in_different_brains_each_take_clicks(cx: &mut gpui::TestAppContext) {
    let (central, code) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let repo = code.path().join("blog/.brain");
    for (dir, title) in [(central.path(), "renew passport"), (repo.as_path(), "first post")] {
        Store::at(dir).unwrap().save(&mut Task::new(1, title.to_string(), now())).unwrap();
    }
    kanbrain_core::projects::register(central.path(), "blog", &repo).unwrap();
    let board = Board::open(central.path(), None).unwrap();
    let (view, cx) = cx.add_window_view(|window, cx| BrainView::new(board, window, cx));
    cx.run_until_parked();
    for (selector, src) in [("card 1/1", 1), ("card 0/1", 0)] {
        let card = cx.debug_bounds(selector).expect("both #1 cards are drawn").center();
        cx.simulate_click(card, gpui::Modifiers::none());
        let expected = Some(crate::state::Key::new(src, 1));
        view.read_with(cx, |view, _| assert_eq!(view.board.selected, expected, "{selector}"));
    }
}

#[gpui::test]
fn every_card_in_a_column_takes_clicks(cx: &mut gpui::TestAppContext) {
    let (view, cx, _dir) = window(cx, &["a", "b", "c"]);
    // To do lists the newest first: #3, #2, #1.
    for id in [1, 2, 3] {
        let selector: &'static str = format!("card 0/{id}").leak();
        let card = cx.debug_bounds(selector).expect("every card is drawn").center();
        cx.simulate_click(card, gpui::Modifiers::none());
        view.read_with(cx, |view, _| assert_eq!(view.board.selected, Some(crate::state::Key::new(0, id))));
    }
}

#[gpui::test]
fn the_project_dropdown_filters_and_picks_with_the_mouse(cx: &mut gpui::TestAppContext) {
    let (central, code) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    for repo in ["api", "web"] {
        let dir = code.path().join(repo).join(".brain");
        Store::at(&dir).unwrap().save(&mut Task::new(1, format!("{repo} task"), now())).unwrap();
        kanbrain_core::projects::register(central.path(), "shop", &dir).unwrap();
    }
    let board = Board::open(central.path(), None).unwrap();
    let (view, cx) = cx.add_window_view(|window, cx| BrainView::new(board, window, cx));
    cx.run_until_parked();
    cx.simulate_keystrokes("P w e b");
    // All, Central, shop, shop/api, shop/web: "web" leaves only shop/web, the first row.
    let row = cx.debug_bounds("project 0").expect("the dropdown lists what matches").center();
    cx.simulate_click(row, gpui::Modifiers::none());
    view.read_with(cx, |view, _| {
        assert_eq!(view.board.scope, [crate::state::Scope::Repo(2)]);
        assert!(matches!(view.board.mode, crate::state::Mode::Scopes(_)), "it stays open, for several");
    });
}

#[gpui::test]
fn manage_project_draws_every_repository(cx: &mut gpui::TestAppContext) {
    let (central, code) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    for repo in ["api", "web"] {
        let dir = code.path().join(repo).join(".brain");
        Store::at(&dir).unwrap().save(&mut Task::new(1, format!("{repo} task"), now())).unwrap();
        kanbrain_core::projects::register(central.path(), "shop", &dir).unwrap();
        kanbrain_core::projects::write_repo_card(
            &dir,
            &kanbrain_core::projects::RepoCard { project: "shop".into(), repo: None, notes: "notes".into() },
        )
        .unwrap();
    }
    let mut board = Board::open(central.path(), None).unwrap();
    board.set_scope(crate::state::Scope::Project("shop".into()));
    let (view, cx) = cx.add_window_view(|window, cx| BrainView::new(board, window, cx));
    cx.run_until_parked();
    assert!(cx.debug_bounds("button edit-project").is_some(), "the bar shows the project's card");
    let row = cx.debug_bounds("scope bar").expect("the projects row is drawn");
    let today = cx.debug_bounds("today total").expect("today's total shows beside a project too");
    assert!(in_row(today, row), "{today:?} in {row:?}");
    let info = cx.debug_bounds("manage-project-button").expect("the bar has the manage button").center();
    cx.simulate_click(info, gpui::Modifiers::none());
    view.read_with(cx, |view, _| assert!(matches!(view.board.mode, crate::state::Mode::ManageProject(1))));
    let folder = view.read_with(cx, |view, _| {
        let brain = view.board.brain(1).store.dir();
        brain.parent().expect("a repository's .brain/ is in it").display().to_string()
    });
    for (text, says) in [("text repo 1 notes", "notes".to_string()), ("text repo 1 folder", folder)] {
        triple_click(cx, text);
        assert_eq!(copied(cx), Some(says), "{text}");
    }
    cx.simulate_keystrokes("escape");
    view.read_with(cx, |view, _| assert!(matches!(view.board.mode, crate::state::Mode::Normal)));
}

#[gpui::test]
fn calendar_lines_stay_inside_narrow_week_columns(cx: &mut gpui::TestAppContext) {
    let (_view, cx, _dir) = window(cx, &["Stripe test card: 4242 4242 4242 4242"]);
    // Started and parked, so its line reads `00:00 doing → parked`, too wide for a column of a narrow week.
    cx.simulate_keystrokes("s p 5");
    cx.simulate_resize(gpui::size(px(700.), px(700.)));
    cx.run_until_parked();
    let today = chrono::Local::now().date_naive();
    let column = cx.debug_bounds(format!("week day {today}").leak()).expect("the week shows today");
    let what = cx.debug_bounds("event what 0/1").expect("the task's move is drawn");
    assert!(what.right() <= column.right(), "the move wraps under the time, inside the column: {what:?} in {column:?}");
}

#[gpui::test]
fn a_long_title_in_a_narrow_week_is_cut_to_a_few_lines(cx: &mut gpui::TestAppContext) {
    let long = "a long title ".repeat(40);
    let (_view, cx, _dir) = window(cx, &[long.as_str()]);
    cx.simulate_keystrokes("5");
    cx.simulate_resize(gpui::size(px(700.), px(700.)));
    cx.run_until_parked();
    let event = cx.debug_bounds("event 0/1").expect("the task's creation is drawn");
    // Its time, what happened and three lines of title: unclamped, the title alone would take dozens.
    assert!(event.size.height < px(160.), "{event:?}");
}

#[gpui::test]
fn buttons_clicked_while_searching_act_instead_of_typing(cx: &mut gpui::TestAppContext) {
    let (view, cx, _dir) = window(cx, &["a"]);
    cx.simulate_keystrokes("/ l o g");
    let button = cx.debug_bounds("tag-filter").expect("the tag filter is drawn").center();
    cx.simulate_click(button, gpui::Modifiers::none());
    view.read_with(cx, |view, _| {
        assert_eq!(view.board.search.text(), "log", "no # typed into the search");
        assert!(matches!(view.board.mode, crate::state::Mode::Tags(_)), "the tag dropdown opens");
    });
}

#[gpui::test]
fn the_form_links_by_clicking_and_saves_with_its_button(cx: &mut gpui::TestAppContext) {
    let (view, cx, _dir) = window(cx, &["a", "b"]);
    cx.simulate_keystrokes("n x tab tab");
    let option = cx.debug_bounds("link option 0").expect("the related field lists the entries").center();
    cx.simulate_click(option, gpui::Modifiers::none());
    view.read_with(cx, |view, _| {
        let crate::state::Mode::Form(form) = &view.board.mode else { panic!("still in the form") };
        assert_eq!(form.links.len(), 1, "the click checked one");
    });
    let save = cx.debug_bounds("form-save").expect("the save button is drawn").center();
    cx.simulate_click(save, gpui::Modifiers::none());
    view.read_with(cx, |view, _| {
        let brain = view.board.brain(0);
        let x = brain.tasks.iter().find(|t| t.title == "x").expect("saved");
        assert_eq!(brain.related(x.id).len(), 1);
    });
}

#[gpui::test]
fn long_lists_say_how_many_more_and_follow_the_highlight(cx: &mut gpui::TestAppContext) {
    let titles: Vec<String> = (1..=13).map(|i| format!("task {i}")).collect();
    let titles: Vec<&str> = titles.iter().map(String::as_str).collect();
    let (_view, cx, _dir) = window(cx, &titles);
    // The link picker of the selected task lists the 12 others: 8 shown, 4 more below.
    cx.simulate_keystrokes("r");
    assert!(cx.debug_bounds("list more").is_some(), "it says there are more below");
    let list = cx.debug_bounds("links-list").expect("the list is drawn");
    for _ in 0..11 {
        cx.simulate_keystrokes("down");
    }
    // The scroll is applied as the next frame is laid out; draw one more to measure it.
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let bottom_row = cx.debug_bounds("link 1").expect("the last row is drawn");
    assert!(
        bottom_row.bottom() <= list.bottom() + px(1.),
        "the highlight scrolled into view: {bottom_row:?} in {list:?}"
    );
    assert!(cx.debug_bounds("list more").is_none(), "nothing more below the end");
    assert!(cx.debug_bounds("list above").is_some(), "but it says the first rows are above");
}

#[gpui::test]
fn a_long_list_draws_only_the_rows_near_its_view(cx: &mut gpui::TestAppContext) {
    use super::dropdown::MAX_ROWS;
    let titles: Vec<String> = (1..=300).map(|i| format!("task {i}")).collect();
    let titles: Vec<&str> = titles.iter().map(String::as_str).collect();
    let (_view, cx, _dir) = window(cx, &titles);
    cx.simulate_keystrokes("r");
    let list = cx.debug_bounds("links-list").expect("the list is drawn");
    let drawn = |cx: &mut gpui::VisualTestContext| {
        let rows: Vec<_> = (1..=300).filter_map(|id| cx.debug_bounds(Box::leak(format!("link {id}").into()))).collect();
        let shown = rows.iter().filter(|row| row.top() >= list.top() && row.bottom() <= list.bottom() + px(1.)).count();
        (rows.len(), shown)
    };
    let (rows, shown) = drawn(cx);
    assert!(rows <= 3 * MAX_ROWS, "a view of rows either side at most, not 299: {rows}");
    assert_eq!(shown, MAX_ROWS, "and the list full");
    for _ in 0..150 {
        cx.simulate_keystrokes("down");
    }
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let (rows, shown) = drawn(cx);
    assert_eq!((rows, shown), (3 * MAX_ROWS, MAX_ROWS), "followed down, a view drawn either side of the one shown");
    // The boxes standing in for the rest are their rows' height, so the list scrolls as far as its rows go.
    let (above, below) = (cx.debug_bounds("links-above").unwrap(), cx.debug_bounds("links-below").unwrap());
    let rest = super::dropdown::ROW_HEIGHT * f32::from(u16::try_from(299 - rows).unwrap());
    assert_eq!(f32::from(above.size.height + below.size.height), rest, "the 299 rows' height in all");
    let highlighted = cx.debug_bounds("link 150").expect("the 151st row, newest first, is drawn");
    assert!(highlighted.bottom() <= list.bottom() + px(1.) && highlighted.top() >= list.top(), "and in view");
    // The wheel takes the list where no row was drawn: those there are, by the frame it shows.
    let modifiers = gpui::Modifiers::none();
    // Fifty rows down, so none is cut at its edges.
    let delta = gpui::ScrollDelta::Pixels(gpui::point(px(0.), px(-50. * super::dropdown::ROW_HEIGHT)));
    let touch_phase = gpui::TouchPhase::Moved;
    cx.simulate_event(gpui::ScrollWheelEvent { position: list.center(), delta, modifiers, touch_phase });
    next_frames(cx);
    let (rows, shown) = drawn(cx);
    assert!(rows <= 3 * MAX_ROWS && shown == MAX_ROWS, "scrolled: {rows} drawn, {shown} shown");
    let row = cx.debug_bounds("link 100").expect("the row fifty below the highlight's is drawn");
    assert!(row.top() >= list.top() && row.bottom() <= list.bottom() + px(1.), "and in view");
}

#[gpui::test]
fn a_list_opened_on_a_row_far_down_shows_it_at_once(cx: &mut gpui::TestAppContext) {
    use kanbrain_core::projects;
    let (central, code) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    for i in 10..40 {
        let dir = code.path().join(format!("repo{i}")).join(".brain");
        Store::at(&dir).unwrap().save(&mut Task::new(1, format!("task in p{i}"), now())).unwrap();
        projects::register(central.path(), &format!("p{i}"), &dir).unwrap();
        let card = projects::RepoCard { project: format!("p{i}"), repo: None, notes: String::new() };
        projects::write_repo_card(&dir, &card).unwrap();
    }
    let board = Board::open(central.path(), None).unwrap();
    let (view, cx) = cx.add_window_view(|window, cx| BrainView::new(board, window, cx));
    cx.run_until_parked();
    let far = view.read_with(cx, |view, _| view.board.sources.len() - 3);
    view.update(cx, |view, _| view.board.select(crate::state::Key::new(far, 1)));
    // "Which project?" for a quick add, its own scroll used for the first time, highlights the selected entry's.
    cx.simulate_keystrokes("a");
    let list = cx.debug_bounds("where-list").expect("the list is drawn");
    let shown = (0..40)
        .filter_map(|i| cx.debug_bounds(Box::leak(format!("where {i}").into())))
        .filter(|row| row.top() >= list.top() && row.bottom() <= list.bottom() + px(1.))
        .count();
    assert_eq!(shown, super::dropdown::MAX_ROWS, "rows in view on the first frame, not the room left for them");
}

#[gpui::test]
fn a_list_that_fits_says_nothing_under_it(cx: &mut gpui::TestAppContext) {
    let titles: Vec<String> = (0..=super::dropdown::MAX_ROWS).map(|i| format!("task {i}")).collect();
    let titles: Vec<&str> = titles.iter().map(String::as_str).collect();
    let (_view, cx, _dir) = window(cx, &titles);
    // The link picker lists the others: as many as it shows.
    cx.simulate_keystrokes("r");
    assert!(cx.debug_bounds("links-list").is_some(), "the list is drawn");
    assert!(cx.debug_bounds("list more").is_none() && cx.debug_bounds("list above").is_none());
}

/// How many rows of the list whose first row is `first` show before it scrolls: from that row's top to
/// the "N more" line under the list.
fn rows_shown(cx: &mut gpui::VisualTestContext, first: &'static str) -> f32 {
    let top = cx.debug_bounds(first).unwrap_or_else(|| panic!("{first} is drawn")).top();
    let more = cx.debug_bounds("list more").unwrap_or_else(|| panic!("{first}'s list scrolls")).top();
    f32::from(more - top) / super::dropdown::ROW_HEIGHT
}

#[gpui::test]
fn every_dropdown_with_a_search_scrolls_past_its_most_rows(cx: &mut gpui::TestAppContext) {
    use super::dropdown::MAX_ROWS;
    use kanbrain_core::projects;
    // Nine tasks with a tag each, and nine projects: more than any dropdown shows at once, even a move's,
    // which leaves out the entry's own brain.
    let central = tempfile::tempdir().unwrap();
    let code = tempfile::tempdir().unwrap();
    let store = Store::at(central.path()).unwrap();
    for id in 1..=9 {
        let mut task = Task::new(id, format!("task {id}"), now());
        task.tags = vec![format!("tag{id}")];
        store.save(&mut task).unwrap();
    }
    for i in 1..=9 {
        let dir = code.path().join(format!("repo{i}")).join(".brain");
        Store::at(&dir).unwrap();
        projects::register(central.path(), &format!("p{i}"), &dir).unwrap();
        projects::write_repo_card(
            &dir,
            &projects::RepoCard { project: format!("p{i}"), repo: None, notes: String::new() },
        )
        .unwrap();
    }
    let board = Board::open(central.path(), None).unwrap();
    let (_view, cx) = cx.add_window_view(|window, cx| BrainView::new(board, window, cx));
    cx.run_until_parked();
    let form_rows = 6.;
    let most = f32::from(u16::try_from(MAX_ROWS).unwrap());
    cx.simulate_keystrokes("P");
    assert!((rows_shown(cx, "project 0") - most).abs() < 1., "the project filter");
    cx.simulate_keystrokes("escape #");
    assert!((rows_shown(cx, "tag 0") - most).abs() < 1., "the tag filter");
    cx.simulate_keystrokes("escape a");
    assert!((rows_shown(cx, "where 0") - most).abs() < 1., "which project, for quick add");
    cx.simulate_keystrokes("escape escape M");
    assert!((rows_shown(cx, "where 0") - most).abs() < 1., "which project, for a move");
    cx.simulate_keystrokes("escape n up");
    assert!((rows_shown(cx, "project option 0") - form_rows).abs() < 1., "the form's project field");
    cx.simulate_keystrokes("tab tab tab");
    assert!((rows_shown(cx, "link option 0") - form_rows).abs() < 1., "the form's related field");
    cx.simulate_keystrokes("escape ?");
    assert!((rows_shown(cx, "command 0") - 12.).abs() < 1., "the command palette, the longest");
}

#[gpui::test]
fn the_palette_names_each_section_on_its_first_row(cx: &mut gpui::TestAppContext) {
    let (view, cx, _dir) = window(cx, &["one"]);
    cx.simulate_keystrokes("?");
    let firsts: Vec<usize> = view.read_with(cx, |view, _| {
        let crate::state::Mode::Palette(picker) = &view.board.mode else { panic!("the palette is open") };
        let commands = view.board.commands(picker);
        (0..commands.len()).filter(|&i| i == 0 || commands[i - 1].section() != commands[i].section()).collect()
    });
    assert!(firsts.len() > 1, "several sections: {firsts:?}");
    // Every row drawn: the palette shows twelve, and a view either side of them is drawn.
    let headed: Vec<usize> =
        (0..36).filter(|i| cx.debug_bounds(Box::leak(format!("heading {i}").into_boxed_str())).is_some()).collect();
    assert_eq!(headed, firsts.into_iter().filter(|&i| i < 36).collect::<Vec<_>>());
}

#[test]
fn the_installer_gives_the_desktop_the_logo() {
    let installer = include_str!("../../install.sh");
    assert!(installer.contains(include_str!("../../docs/images/logo.svg")), "install.sh carries docs/images/logo.svg");
    assert!(installer.contains(&format!("Icon={APP_ID}")) && installer.contains(&format!("apps/{APP_ID}.svg")));
}

#[gpui::test]
fn windows_open_centered_with_the_apps_id_and_logo(cx: &mut gpui::TestAppContext) {
    let options = cx.update(|cx| window_options(size(px(560.), px(280.)), cx));
    let Some(WindowBounds::Windowed(bounds)) = options.window_bounds else { panic!("{:?}", options.window_bounds) };
    assert_eq!(bounds.size, size(px(560.), px(280.)));
    assert_eq!(options.app_id.as_deref(), Some(APP_ID));
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    assert!(options.icon.is_some(), "the logo for X11");
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
#[test]
fn the_window_carries_the_logo_for_x11() {
    let icon = window_icon().expect("the built-in PNG decodes");
    assert_eq!(icon.dimensions(), (128, 128));
}

#[gpui::test]
fn closing_the_details_opens_nothing(cx: &mut gpui::TestAppContext) {
    let (view, cx, _dir) = window(cx, &["a", "b"]);
    let open = |cx: &mut gpui::VisualTestContext| {
        let card = cx.debug_bounds("card 0/2").expect("card 2 is drawn").center();
        double_click(cx, card);
    };
    open(cx);
    view.read_with(cx, |view, _| assert!(view.board.drawer, "a double-click opens it"));
    let close = cx.debug_bounds("close drawer").expect("the drawer has a close button").center();
    cx.simulate_click(close, gpui::Modifiers::none());
    for add in ["add todo", "add doing", "add parked", "add done"] {
        let plus = cx.debug_bounds(add).expect("each column has a +");
        assert!(!plus.contains(&close), "{add} is where the close button was");
    }
    view.read_with(cx, |view, _| assert!(!view.board.drawer && matches!(view.board.mode, crate::state::Mode::Normal)));
    open(cx);
    double_click(cx, close);
    view.read_with(cx, |view, _| {
        assert!(!view.board.drawer, "the close button closes it");
        assert!(matches!(view.board.mode, crate::state::Mode::Normal), "and opens nothing: {:?}", view.board.mode);
    });
    open(cx);
    cx.simulate_keystrokes("escape");
    view.read_with(cx, |view, _| {
        assert!(!view.board.drawer, "esc closes it");
        assert!(matches!(view.board.mode, crate::state::Mode::Normal), "and opens nothing: {:?}", view.board.mode);
    });
}

/// Clicks the element `selector` names, which must be drawn.
fn click(cx: &mut gpui::VisualTestContext, selector: &'static str) {
    let at = cx.debug_bounds(selector).unwrap_or_else(|| panic!("{selector} is drawn")).center();
    cx.simulate_click(at, gpui::Modifiers::none());
    cx.run_until_parked();
}

/// The board's mode, as `{:?}` writes it, to check which dialog is open.
fn mode(view: &gpui::Entity<BrainView>, cx: &mut gpui::VisualTestContext) -> String {
    view.read_with(cx, |view, _| format!("{:?}", view.board.mode))
}

#[gpui::test]
fn the_details_buttons_act_when_clicked(cx: &mut gpui::TestAppContext) {
    let (view, cx, _dir) = window(cx, &["a", "b"]);
    let card = cx.debug_bounds("card 0/1").expect("card 1 is drawn").center();
    double_click(cx, card);
    click(cx, "drawer status doing");
    view.read_with(cx, |view, _| assert_eq!(view.board.brain(0).task(1).unwrap().status, Status::Doing));
    click(cx, "button edit");
    assert!(mode(&view, cx).starts_with("Form("), "{}", mode(&view, cx));
    cx.simulate_keystrokes("escape");
    click(cx, "button relate");
    assert!(mode(&view, cx).starts_with("Links("));
    click(cx, "button links-done");
    assert_eq!(mode(&view, cx), "Normal");

    click(cx, "button edit-description");
    cx.simulate_keystrokes("x");
    click(cx, "button discard");
    assert_eq!(mode(&view, cx), "Normal");
    click(cx, "button add-comment");
    cx.simulate_keystrokes("h i");
    click(cx, "button save");
    view.read_with(cx, |view, _| {
        let task = view.board.brain(0).task(1).unwrap();
        assert_eq!((task.description.as_str(), task.comments[0].body.as_str()), ("", "hi"), "discarded, then added");
    });
    click(cx, "button edit-comment-0");
    assert!(mode(&view, cx).contains("Comment("), "{}", mode(&view, cx));
    cx.simulate_keystrokes("escape");
    click(cx, "button delete-comment-0");
    click(cx, "button confirm");
    view.read_with(cx, |view, _| assert_eq!(view.board.brain(0).task(1).unwrap().comments, []));

    click(cx, "button delete");
    assert!(mode(&view, cx).starts_with("Confirm(Delete"));
    click(cx, "button cancel");
    view.read_with(cx, |view, _| assert!(view.board.brain(0).task(1).is_some(), "cancelled"));
    click(cx, "close drawer");
    view.read_with(cx, |view, _| assert!(!view.board.drawer));
}

#[gpui::test]
fn the_top_bar_acts_when_clicked(cx: &mut gpui::TestAppContext) {
    let (view, cx, _dir) = window(cx, &["a", "b"]);
    click(cx, "segment tab-1");
    view.read_with(cx, |view, _| assert_eq!(view.board.tab, crate::state::Tab::Notes));
    click(cx, "segment tab-0");
    click(cx, "segment layout-1");
    view.read_with(cx, |view, _| assert_eq!(view.board.layout, crate::state::Layout::List));
    click(cx, "segment layout-0");
    click(cx, "button new");
    assert!(mode(&view, cx).starts_with("Form("));
    cx.simulate_keystrokes("escape");
    click(cx, "button commands");
    assert!(mode(&view, cx).starts_with("Palette("));
    click(cx, "button palette-close");
    click(cx, "button reload");
    assert_eq!(mode(&view, cx), "Normal");

    // An agent's question on #2: the bar's button goes to it.
    view.update(cx, |view, cx| {
        let brain = view.board.brain_mut(0);
        brain.author = kanbrain_core::model::Author::Agent;
        brain.ask(2, "Which one?").unwrap();
        cx.notify();
    });
    cx.run_until_parked();
    click(cx, "button waiting");
    view.read_with(cx, |view, _| assert_eq!(view.board.selected, Some(crate::state::Key::new(0, 2))));
}

/// Whether `inner` lies within `outer`'s height, as an element in a row does.
fn in_row(inner: gpui::Bounds<gpui::Pixels>, outer: gpui::Bounds<gpui::Pixels>) -> bool {
    inner.top() >= outer.top() && inner.bottom() <= outer.bottom()
}

#[gpui::test]
fn todays_total_and_park_and_quit_sit_in_the_projects_row_with_no_bar_of_running_tasks(cx: &mut gpui::TestAppContext) {
    let (view, cx, _dir) = window(cx, &["a", "b"]);
    let ends_at_projects_row = |cx: &mut gpui::VisualTestContext| {
        let top = cx.debug_bounds("top bar").expect("the top bar is drawn");
        let row = cx.debug_bounds("scope bar").expect("the projects row is drawn");
        assert_eq!(top.bottom(), row.bottom(), "nothing under the projects row: the columns take the room");
        row
    };
    let row = ends_at_projects_row(cx);
    let today = cx.debug_bounds("today total").expect("today's total shows with nothing running");
    assert!(in_row(today, row), "{today:?} in {row:?}");
    assert!(cx.debug_bounds("button park-quit").is_none(), "nothing to park");

    cx.simulate_keystrokes("1 escape s");
    view.read_with(cx, |view, _| assert_eq!(view.board.running().count(), 1, "a task runs"));
    let row = ends_at_projects_row(cx);
    let quit = cx.debug_bounds("button park-quit").expect("Park & quit shows while a task runs");
    assert!(in_row(quit, row), "{quit:?} in {row:?}");
    assert!(in_row(cx.debug_bounds("today total").unwrap(), row));
    assert!(cx.debug_bounds("button park 0/2").is_none() && cx.debug_bounds("button park 0/1").is_none());
}

#[test]
fn an_empty_doing_says_where_tasks_running_elsewhere_show() {
    use crate::state::Scope;
    let dir = tempfile::tempdir().unwrap();
    Store::at(dir.path()).unwrap();
    let mut board = Board::open(dir.path(), None).unwrap();
    assert_eq!(
        super::board::empty_hint(&board, Status::Doing),
        "Nothing running. Drop a card here to start its clock."
    );
    for scope in [Scope::Central, Scope::Project("shop".into()), Scope::Repo(0)] {
        board.set_scope(scope.clone());
        let hint = super::board::empty_hint(&board, Status::Doing);
        assert_eq!(
            hint, "Nothing running here: All shows what runs elsewhere. Drop a card here to start its clock.",
            "{scope:?}"
        );
    }
}

#[gpui::test]
fn park_and_quit_shows_only_while_a_task_of_yours_runs(cx: &mut gpui::TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::at(dir.path()).unwrap();
    let mut agents = Task::new(1, "agent's".to_string(), now());
    agents.set_status(Status::Doing, now(), Author::Agent);
    agents.start_session(now());
    store.save(&mut agents).unwrap();
    store.save(&mut Task::new(2, "mine".to_string(), now())).unwrap();
    let board = Board::open(dir.path(), None).unwrap();
    let (view, cx) = cx.add_window_view(|window, cx| BrainView::new(board, window, cx));
    cx.run_until_parked();
    view.read_with(cx, |view, _| assert!(view.board.any_running(), "the agent's task runs"));
    assert!(cx.debug_bounds("button park-quit").is_none(), "it would park nothing: agents' tasks keep running");

    view.update(cx, |view, cx| {
        view.board.set_status(crate::state::Key::new(0, 2), Status::Doing);
        cx.notify();
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("button park-quit").is_some(), "yours runs: it parks it");
}

#[gpui::test]
fn manage_projects_buttons_open_their_dialogs_and_come_back(cx: &mut gpui::TestAppContext) {
    let (central, code) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    for repo in ["api", "web"] {
        let dir = code.path().join(repo).join(".brain");
        Store::at(&dir).unwrap().save(&mut Task::new(1, format!("{repo} task"), now())).unwrap();
        kanbrain_core::projects::register(central.path(), "shop", &dir).unwrap();
        kanbrain_core::projects::write_repo_card(
            &dir,
            &kanbrain_core::projects::RepoCard { project: "shop".into(), repo: None, notes: "notes".into() },
        )
        .unwrap();
    }
    let mut board = Board::open(central.path(), None).unwrap();
    board.set_scope(crate::state::Scope::Project("shop".into()));
    let (view, cx) = cx.add_window_view(|window, cx| BrainView::new(board, window, cx));
    cx.run_until_parked();
    click(cx, "manage-project-button");
    // The project's buttons first: coming back from a repository's dialog highlights that repository, whose index
    // the project's buttons then carry.
    for (button, opens) in [
        ("button rename-project 1", "RenameProject"),
        ("button remove-project", "Remove"),
        ("button rename-repo 2", "RenameRepo"),
        ("button move-repo 2", "MoveRepo"),
        ("button remove-repo 2", "Remove"),
    ] {
        click(cx, button);
        assert!(mode(&view, cx).contains(opens), "{button}: {}", mode(&view, cx));
        click(cx, "button cancel");
        assert!(mode(&view, cx).starts_with("ManageProject("), "{button} goes back: {}", mode(&view, cx));
    }
    for edit in ["button edit-notes 2", "button edit-project 2"] {
        click(cx, edit);
        assert!(mode(&view, cx).starts_with("Edit("), "{edit}");
        click(cx, "button save");
        assert!(mode(&view, cx).starts_with("ManageProject("), "{edit}'s editor goes back: {}", mode(&view, cx));
    }
    click(cx, "button close-manage-project");
    assert_eq!(mode(&view, cx), "Normal");
}

#[gpui::test]
fn the_report_and_calendar_buttons_act_when_clicked(cx: &mut gpui::TestAppContext) {
    let (view, cx, dir) = window(cx, &["a"]);
    cx.simulate_keystrokes("s 4");
    let period = |view: &gpui::Entity<BrainView>, cx: &mut gpui::VisualTestContext| {
        view.read_with(cx, |view, _| view.board.report.period)
    };
    let now = period(&view, cx);
    click(cx, "button prev");
    assert_ne!(period(&view, cx), now);
    click(cx, "button now");
    assert_eq!(period(&view, cx), now);
    click(cx, "segment unit-100");
    view.read_with(cx, |view, _| assert_eq!(view.board.report.period.unit, kanbrain_core::period::Unit::Day));
    click(cx, "button group");
    view.read_with(cx, |view, _| assert_eq!(view.board.report.group, kanbrain_core::brain::Group::Tag));
    click(cx, "button export");
    assert!(std::fs::read_dir(dir.path().join("reports")).unwrap().next().is_some(), "exported");

    cx.simulate_keystrokes("5");
    click(cx, "segment unit-109");
    view.read_with(cx, |view, _| assert_eq!(view.board.calendar.period.unit, kanbrain_core::period::Unit::Month));
    let show = view.read_with(cx, |view, _| view.board.calendar.show);
    click(cx, "button show");
    view.read_with(cx, |view, _| assert_ne!(view.board.calendar.show, show));
}

#[gpui::test]
fn the_comment_list_edits_the_comment_clicked(cx: &mut gpui::TestAppContext) {
    let (view, cx, _dir) = window(cx, &["a"]);
    cx.simulate_keystrokes("c o n e escape c t w o escape shift-c");
    assert!(mode(&view, cx).starts_with("Comments("));
    click(cx, "comment 0");
    view.read_with(cx, |view, _| {
        assert!(matches!(&view.board.mode, crate::state::Mode::Edit(edit)
            if edit.target == crate::state::Target::Comment(crate::state::Key::new(0, 1), 0)));
    });
}

#[test]
fn long_text_is_cut_at_a_words_end() {
    assert_eq!(truncate_words("short", 10), "short");
    assert_eq!(truncate_words("accept or reject it, please", 20), "accept or reject…");
    assert_eq!(truncate_words("Too much ops, for now", 15), "Too much ops…", "no comma before the ellipsis");
    assert_eq!(truncate_words("unbreakableword", 6), "unbre…", "one word is cut as it can be");
}

#[gpui::test]
fn the_form_says_when_the_tag_typed_is_new(cx: &mut gpui::TestAppContext) {
    let (view, cx, _dir) = window(cx, &["a"]);
    view.update(cx, |view, cx| {
        view.board.brain_mut(0).update(1, "a", vec!["auth".into()]).unwrap();
        cx.notify();
    });
    cx.simulate_keystrokes("n");
    cx.simulate_keystrokes("tab");
    cx.run_until_parked();
    assert!(cx.debug_bounds("new tag").is_none(), "nothing typed yet");
    cx.simulate_keystrokes("a");
    cx.run_until_parked();
    assert!(cx.debug_bounds("new tag").is_none(), "#auth has it");
    cx.simulate_keystrokes("x");
    cx.run_until_parked();
    assert!(cx.debug_bounds("new tag").is_some(), "no tag has ax");
}

#[gpui::test]
fn the_form_says_a_tag_typed_again_is_held_not_new(cx: &mut gpui::TestAppContext) {
    let (view, cx, _dir) = window(cx, &["a"]);
    view.update(cx, |view, cx| {
        view.board.brain_mut(0).update(1, "a", vec!["auth".into()]).unwrap();
        cx.notify();
    });
    cx.simulate_keystrokes("e tab a u t h");
    cx.run_until_parked();
    assert!(cx.debug_bounds("new tag").is_none(), "the entry has #auth already");
    assert!(cx.debug_bounds("held tag").is_some(), "which the form says");
}

#[gpui::test]
fn the_form_lists_its_tags_while_typing_them_and_a_click_picks_one(cx: &mut gpui::TestAppContext) {
    let (view, cx, _dir) = window(cx, &["a"]);
    view.update(cx, |view, cx| {
        view.board.brain_mut(0).update(1, "a", vec!["auth".into(), "bug".into()]).unwrap();
        cx.notify();
    });
    cx.simulate_keystrokes("n");
    cx.run_until_parked();
    assert!(cx.debug_bounds("tag-options-list").is_none(), "not while typing the title");
    cx.simulate_keystrokes("tab");
    cx.run_until_parked();
    let bug = cx.debug_bounds("tag option 1").expect("the tags drop down under the field").center();
    cx.simulate_click(bug, gpui::Modifiers::none());
    view.read_with(cx, |view, _| {
        let Mode::Form(form) = &view.board.mode else { panic!("the form stays open") };
        assert_eq!(form.tags, ["bug"]);
    });
}

#[gpui::test]
fn a_click_on_the_tags_list_puts_the_tag_clicked_though_a_reload_moved_it(cx: &mut gpui::TestAppContext) {
    let (view, cx, dir) = window(cx, &["a", "b"]);
    view.update(cx, |view, cx| {
        view.board.brain_mut(0).update(1, "a", vec!["perf".into(), "speed".into()]).unwrap();
        cx.notify();
    });
    cx.simulate_keystrokes("n tab p e");
    cx.run_until_parked();
    let perf = cx.debug_bounds("tag option 0").expect("listed first").center();
    // Another program tags an entry with one sorting ahead, which the click's catching up reads.
    let store = Store::at(dir.path()).unwrap();
    let mut other = store.load_entries().unwrap().0.into_iter().find(|t| t.id == 2).unwrap();
    other.tags = vec!["ape".into()];
    store.save(&mut other).unwrap();
    cx.simulate_click(perf, gpui::Modifiers::none());
    view.read_with(cx, |view, _| {
        let Mode::Form(form) = &view.board.mode else { panic!("the form is open") };
        assert_eq!(form.tags, ["perf"], "the tag clicked, not the row's new one");
    });
}

#[gpui::test]
fn the_forms_tags_are_chips_in_their_colors_and_a_click_on_one_s_x_takes_it_off(cx: &mut gpui::TestAppContext) {
    let (view, cx, _dir) = window(cx, &["a"]);
    view.update(cx, |view, cx| {
        view.board.brain_mut(0).update(1, "a", vec!["auth".into(), "bug".into()]).unwrap();
        cx.notify();
    });
    cx.simulate_keystrokes("e");
    cx.run_until_parked();
    let auth = cx.debug_bounds("form tag auth").expect("each tag a chip");
    let field = cx.debug_bounds("tags").expect("the box to type one");
    assert!(auth.bottom() <= field.top(), "above the box, as the related field's: {auth:?} over {field:?}");
    let x = cx.debug_bounds("untag bug").expect("each with its ×").center();
    cx.simulate_click(x, gpui::Modifiers::none());
    view.read_with(cx, |view, _| {
        let Mode::Form(form) = &view.board.mode else { panic!("the form stays open") };
        assert_eq!(form.tags, ["auth"]);
    });
}

#[gpui::test]
fn the_forms_tags_list_follows_its_highlight(cx: &mut gpui::TestAppContext) {
    let (view, cx, _dir) = window(cx, &["a"]);
    view.update(cx, |view, cx| {
        view.board.brain_mut(0).update(1, "a", (1..=9).map(|i| format!("tag{i}")).collect()).unwrap();
        cx.notify();
    });
    // Nine tags, six rows shown: the last highlighted is scrolled to.
    cx.simulate_keystrokes("n tab");
    (0..9).for_each(|_| cx.simulate_keystrokes("down"));
    next_frames(cx);
    let list = cx.debug_bounds("tag-options-list").expect("the tags drop down under the field");
    let bottom_row = cx.debug_bounds("tag option 8").expect("the last row is drawn");
    assert!(
        bottom_row.bottom() <= list.bottom() + px(1.),
        "the highlight scrolled into view: {bottom_row:?} in {list:?}"
    );
}

#[gpui::test]
fn a_comments_buttons_stay_in_the_details_however_long_its_heading(cx: &mut gpui::TestAppContext) {
    let (view, cx, _dir) = window(cx, &["a"]);
    view.update(cx, |view, cx| {
        let brain = view.board.brain_mut(0);
        brain.author = kanbrain_core::model::Author::Agent;
        brain.add_comment(1, "first").unwrap();
        brain.update_comment(1, 0, "first, edited").unwrap();
        cx.notify();
    });
    cx.simulate_resize(gpui::size(px(1000.), px(800.)));
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    let close = cx.debug_bounds("close drawer").expect("the details are open");
    let delete = cx.debug_bounds("button delete-comment-0").expect("the comment has Delete");
    assert!(delete.right() <= close.right() + px(1.), "within the panel: {delete:?}, its edge {close:?}");
}

#[gpui::test]
fn the_top_bar_wraps_alike_on_every_tab(cx: &mut gpui::TestAppContext) {
    let (view, cx, _dir) = window(cx, &["a", "b"]);
    view.update(cx, |view, cx| {
        let brain = view.board.brain_mut(0);
        brain.author = kanbrain_core::model::Author::Agent;
        brain.ask(2, "Which one?").unwrap();
        cx.notify();
    });
    for width in (90_u16..=160).map(|w| f32::from(w) * 10.) {
        cx.simulate_resize(gpui::size(px(width), px(800.)));
        let mut places = Vec::new();
        for tab in ["1", "2", "3", "4", "5"] {
            cx.simulate_keystrokes(tab);
            cx.run_until_parked();
            places.push(cx.debug_bounds("button new").expect("the New button is drawn").origin.y);
        }
        assert!(places.windows(2).all(|p| p[0] == p[1]), "at {width} px the bar moves between tabs: {places:?}");
    }
}

#[gpui::test]
fn dialogs_name_the_entry_with_its_brain_when_several_are_shown(cx: &mut gpui::TestAppContext) {
    let (central, code) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let repo = code.path().join("blog/.brain");
    for (dir, title) in [(central.path(), "renew passport"), (repo.as_path(), "first post")] {
        let mut task = Task::new(1, title.to_string(), now());
        task.comments.push(kanbrain_core::model::Comment::new(now(), "draft done".into(), Author::Human));
        Store::at(dir).unwrap().save(&mut task).unwrap();
    }
    kanbrain_core::projects::register(central.path(), "blog", &repo).unwrap();
    let board = Board::open(central.path(), None).unwrap();
    let (_view, cx) = cx.add_window_view(|window, cx| BrainView::new(board, window, cx));
    cx.run_until_parked();
    let card = cx.debug_bounds("card 1/1").expect("blog's #1 is drawn").center();
    cx.simulate_click(card, gpui::Modifiers::none());
    for (keys, title) in [
        ("e", "dialog Edit blog #1"),
        ("M", "dialog Move blog #1 first post to which project?"),
        ("r", "dialog Related to blog #1 first post"),
        ("C", "dialog Comments on blog #1 first post"),
    ] {
        cx.simulate_keystrokes(keys);
        cx.run_until_parked();
        assert!(cx.debug_bounds(title).is_some(), "`{keys}` opens `{title}`");
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
    }
}

#[test]
fn the_editor_is_visual_then_editor_skipping_one_set_empty() {
    let some = |s: &str| Some(s.to_string());
    assert_eq!(editor_of(some("code -w"), some("nano")), "code -w");
    assert_eq!(editor_of(some(" "), some("nano")), "nano", "set but empty names none");
    assert_eq!(editor_of(None, some("nano")), "nano");
    assert_eq!(editor_of(None, some("")), if cfg!(windows) { "notepad" } else { "vi" });
}

#[test]
fn a_done_cards_title_is_dimmed() {
    assert_eq!(board::title_color(Status::Done), theme::DIM);
    assert_eq!(board::title_color(Status::Doing), theme::TEXT);
}

#[gpui::test]
fn a_card_shows_the_question_its_task_waits_on(cx: &mut gpui::TestAppContext) {
    let (view, cx, _dir) = window(cx, &["Rate-limit login"]);
    assert!(cx.debug_bounds("asked 0/1").is_none(), "nothing asked yet");
    view.update(cx, |view, cx| {
        view.board.brain_mut(0).ask(1, "Per IP or per account?").unwrap();
        cx.notify();
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("asked 0/1").is_some(), "the question is on its card");
}

#[gpui::test]
fn a_card_shows_its_tags_only_when_it_has_some(cx: &mut gpui::TestAppContext) {
    let (view, cx, _dir) = window(cx, &["Rate-limit login"]);
    assert!(cx.debug_bounds("tags 0/1").is_none(), "no tags, no row for them");
    view.update(cx, |view, cx| {
        let brain = view.board.brain_mut(0);
        let mut task = brain.task(1).unwrap().clone();
        task.tags = vec!["backend".into()];
        brain.store.save(&mut task).unwrap();
        view.board.reload().unwrap();
        cx.notify();
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("tags 0/1").is_some(), "the tags are on its card");
}

#[test]
fn a_tags_uses_name_only_the_kinds_it_is_on() {
    assert_eq!(topbar::uses([2, 1, 0]), "2 tasks · 1 note");
    assert_eq!(topbar::uses([0, 0, 1]), "1 decision");
}

#[test]
fn the_project_card_shows_a_descriptions_first_line_that_isnt_blank() {
    assert_eq!(topbar::first_line("\n  \nSells things.\nMore."), "Sells things.");
    assert_eq!(topbar::first_line(""), "");
}

#[test]
fn a_list_counts_the_rows_out_of_view_from_its_scroll_or_the_highlight() {
    use super::dropdown::{ROW_HEIGHT, followed, hidden};
    assert_eq!(hidden(12, 8, 0.), (0, 4));
    assert_eq!(hidden(12, 8, ROW_HEIGHT * 1.4), (1, 3), "a row more than half hidden is out of view");
    assert_eq!(hidden(12, 8, ROW_HEIGHT * 2.4), (2, 2), "one less than half hidden isn't");
    assert_eq!(hidden(12, 8, ROW_HEIGHT * 1.6), (2, 2), "nor one more than half hidden");
    assert_eq!(hidden(12, 8, followed(12, 9, 8, 0.)), (2, 2), "the highlight below the view scrolls it just enough");
    assert_eq!(hidden(12, 8, followed(12, 1, 8, ROW_HEIGHT * 4.)), (1, 3), "and above it, just enough too");
    assert_eq!(hidden(12, 8, ROW_HEIGHT * 4.), (4, 0));
    assert_eq!(followed(12, 5, 8, ROW_HEIGHT * 2.), ROW_HEIGHT * 2., "one in view stays where it is");
    assert_eq!(followed(12, 30, 8, 0.), ROW_HEIGHT * 4., "and none is past the end, as a list cut short leaves it");
    assert_eq!(hidden(12, 8, ROW_HEIGHT * 40.), (12, 0), "nor counted past it");
}

/// Draws the frames the window asked for since, as the platform would: tests have no frame loop. A few at most,
/// so a window that never stops asking fails the test instead of hanging it.
fn next_frames(cx: &mut gpui::VisualTestContext) {
    let settled = (0..10).any(|_| cx.update(gpui::Window::simulate_next_frame) == 0);
    assert!(settled, "the window still asks for frames after 10: it redraws for ever");
    cx.run_until_parked();
}

/// Where a click at `x` in the form's title field, then `x` typed, puts it: the character shown there. The `x` is
/// taken out again.
fn type_at(view: &gpui::Entity<BrainView>, cx: &mut gpui::VisualTestContext, x: gpui::Pixels) -> usize {
    next_frames(cx);
    let field = cx.debug_bounds("title").expect("the form's title field");
    cx.simulate_click(gpui::point(x, field.center().y), gpui::Modifiers::none());
    cx.simulate_keystrokes("x");
    let at = view.read_with(cx, |view, _| {
        let crate::state::Mode::Form(form) = &view.board.mode else { panic!("the form stays open") };
        form.title.text().find('x').expect("typed")
    });
    cx.simulate_keystrokes("backspace");
    cx.run_until_parked();
    at
}

#[gpui::test]
fn a_line_longer_than_its_field_scrolls_to_keep_the_cursor_in_view(cx: &mut gpui::TestAppContext) {
    let (view, cx, _dir) = window(cx, &[]);
    cx.simulate_keystrokes("n");
    let long = format!("{}end", "word ".repeat(60));
    let keys: Vec<String> = long.chars().map(|c| if c == ' ' { "space".into() } else { c.to_string() }).collect();
    cx.simulate_keystrokes(&keys.join(" "));
    cx.run_until_parked();
    let field = cx.debug_bounds("title").expect("the form's title field");
    assert_eq!(
        type_at(&view, cx, field.right() - gpui::px(9.)),
        long.len(),
        "the end shows, by the right edge (inside its 8-pixel padding)"
    );
    // Moved left past the left edge, only as far as the cursor: what's left of it starts there.
    cx.simulate_keystrokes(&["left"; 100].join(" "));
    cx.run_until_parked();
    assert_eq!(type_at(&view, cx, field.left() + gpui::px(9.)), long.len() - 100, "the cursor shows, by the left edge");
    // Moving inside what shows doesn't scroll.
    cx.simulate_keystrokes("home");
    cx.simulate_keystrokes(&["right"; 10].join(" "));
    cx.run_until_parked();
    assert_eq!(type_at(&view, cx, field.left() + gpui::px(9.)), 0, "the start shows, with the cursor in it");
}

#[gpui::test]
fn a_field_shown_again_starts_at_its_start_not_where_it_was_left(cx: &mut gpui::TestAppContext) {
    let long = format!("{}end", "word ".repeat(60));
    let (view, cx, _dir) = window(cx, &[&long]);
    cx.simulate_keystrokes("e");
    next_frames(cx);
    // Left scrolled partway: the cursor 100 characters back from the end, by the left edge.
    cx.simulate_keystrokes(&["left"; 100].join(" "));
    next_frames(cx);
    cx.simulate_keystrokes("escape");
    next_frames(cx);
    // Opened again and drawn once, before the cursor at its end is scrolled to: what a click there hits is what
    // that frame shows.
    view.update(cx, |view, cx| {
        view.board.edit_selected();
        cx.notify();
    });
    let field = cx.debug_bounds("title").expect("the form's title field");
    cx.simulate_click(gpui::point(field.left() + gpui::px(9.), field.center().y), gpui::Modifiers::none());
    cx.simulate_keystrokes("x");
    let at = view.read_with(cx, |view, _| {
        let crate::state::Mode::Form(form) = &view.board.mode else { panic!("the form stays open") };
        form.title.text().find('x').expect("typed")
    });
    assert_eq!(at, 0, "its start, not the 203rd character it was left scrolled to");
}

#[gpui::test]
fn a_field_typed_past_its_edge_a_key_at_a_time_stops_redrawing(cx: &mut gpui::TestAppContext) {
    let (_view, cx, _dir) = window(cx, &[]);
    cx.simulate_keystrokes("n");
    // Each length lays the end out a little differently against the field's edge: one of them, where the field
    // can't scroll as far as its padding asks, redrew for ever.
    for key in "iiiiiiiiiiwwwwwwwwwwmmmmmmmmmmlilili".chars().cycle().take(120) {
        cx.simulate_keystrokes(&key.to_string());
        next_frames(cx);
    }
    for _ in 0..120 {
        cx.simulate_keystrokes("backspace");
        next_frames(cx);
    }
}

#[gpui::test]
fn the_details_list_changes_among_the_comments_by_time(cx: &mut gpui::TestAppContext) {
    let (view, cx, _dir) = window(cx, &["a"]);
    view.update(cx, |view, cx| {
        let t0 = chrono::Local::now() - chrono::Duration::hours(1);
        let at = |minutes| t0 + chrono::Duration::minutes(minutes);
        let task = &mut view.board.brain_mut(0).tasks[0];
        task.set_title("b".into(), at(0), Author::Agent);
        task.comments.push(kanbrain_core::model::Comment::new(at(1), "why".into(), Author::Human));
        task.set_description("steps".into(), at(2), Author::Human);
        view.board.selected = Some(crate::state::Key::new(0, 1));
        view.board.drawer = true;
        cx.notify();
    });
    cx.run_until_parked();
    let mut top = |selector: &'static str| cx.debug_bounds(selector).unwrap_or_else(|| panic!("{selector}")).top();
    let (title, comment, description) = (top("thread change 0"), top("thread comment 0"), top("thread change 2"));
    assert!(title < comment && comment < description, "{title:?} {comment:?} {description:?}");
}

#[test]
fn the_thread_says_who_wrote_and_who_changed() {
    use kanbrain_core::model::{Activity, Changed, Comment};
    let at = chrono::Local::now();
    let when = when(at);
    let mut comment = Comment::new(at, "x".into(), Author::Human);
    assert_eq!(drawer::comment_label(&comment), when, "yours carry no mark");
    comment.author = Author::Agent;
    assert_eq!(drawer::comment_label(&comment), format!("Agent comment · {when}"));
    comment.question = true;
    assert_eq!(drawer::comment_label(&comment), format!("Agent question · {when}"));
    let change = |by| Activity::new(at, by, Changed::Description);
    assert_eq!(drawer::change_label(&change(Author::Human)), format!("You changed the description · {when}"));
    assert_eq!(drawer::change_label(&change(Author::Agent)), format!("An agent changed the description · {when}"));
}

#[gpui::test]
fn the_details_of_a_task_waiting_on_a_question_offer_to_answer_it(cx: &mut gpui::TestAppContext) {
    let (view, cx, _dir) = window(cx, &["a"]);
    view.update(cx, |view, cx| {
        view.board.brain_mut(0).author = Author::Agent;
        view.board.brain_mut(0).ask(1, "Per IP or per account?").unwrap();
        view.board.selected = Some(crate::state::Key::new(0, 1));
        view.board.drawer = true;
        cx.notify();
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("button answer").is_some(), "the question, with Answer");
}

#[gpui::test]
fn conflicts_show_both_sides_and_are_picked_with_the_mouse(cx: &mut gpui::TestAppContext) {
    use kanbrain_core::brain::Brain;
    let (view, cx, dir) = window(cx, &["a"]);
    let kb = || Brain::open(Store::at(dir.path()).unwrap()).unwrap();
    let catch_up = |cx: &mut gpui::VisualTestContext| {
        cx.executor().advance_clock(crate::watch::INTERVAL);
        cx.run_until_parked();
    };
    kb().set_description(1, "a\nb\nc\nd\ne").unwrap();
    catch_up(cx);
    cx.simulate_keystrokes("m");
    view.update(cx, |view, _| {
        view.board.select_all();
        view.board.paste("A\nb\nc\nd\nE");
    });
    kb().set_description(1, "a2\nb\nc\nd\ne2").unwrap();
    catch_up(cx);
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(cx.debug_bounds("conflict 1").is_some(), "both conflicts drawn");
    // Their lines are text to copy, and a drag over a side's doesn't pick it.
    triple_click(cx, "text merge settled 1");
    assert_eq!(copied(cx).as_deref(), Some("b\nc\nd"));
    let mine = cx.debug_bounds("text merge-mine 0").expect("yours is drawn");
    drag(cx, mine.origin + gpui::point(px(1.), px(5.)), mine.bottom_right() - gpui::point(px(1.), px(1.)));
    assert_eq!(copied(cx).as_deref(), Some("A"));
    view.read_with(cx, |view, _| {
        let Mode::Edit(edit) = &view.board.mode else { panic!("the editor is open") };
        assert!(edit.merge.as_ref().is_some_and(|m| m.conflicts().all(|c| c.pick.is_none())), "nothing picked");
    });
    // A double-click's first click picks the side, as any click does; its second selects the word under it.
    double_click(cx, mine.origin + gpui::point(px(1.), px(5.)));
    assert_eq!(copied(cx).as_deref(), Some("A"));
    view.read_with(cx, |view, _| {
        let Mode::Edit(edit) = &view.board.mode else { panic!("the editor is open") };
        let picks: Vec<_> = edit.merge.as_ref().expect("still merging").conflicts().map(|c| c.pick).collect();
        assert_eq!(picks, [Some(crate::text::merge::Pick::Mine), None], "yours picked by the first click");
    });
    // A side, or a segment (conflict 1's third: both).
    click(cx, "merge-theirs-0");
    click(cx, "segment merge-pick-5");
    click(cx, "button merge-use");
    view.read_with(cx, |view, _| {
        let Mode::Edit(edit) = &view.board.mode else { panic!("the editor is open") };
        assert_eq!((edit.merge.is_none(), edit.editor.text().as_str()), (true, "a2\nb\nc\nd\nE\ne2"));
    });
    // Yours where nothing's picked, from its button.
    view.update(cx, |view, _| {
        view.board.select_all();
        view.board.paste("mine");
    });
    kb().set_description(1, "theirs").unwrap();
    catch_up(cx);
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    click(cx, "button merge-mine");
    view.read_with(cx, |view, _| {
        assert!(matches!(&view.board.mode, Mode::Edit(edit) if edit.merge.is_none() && edit.editor.text() == "mine"));
    });
}

#[gpui::test]
fn the_form_shows_the_title_kb_gave_meanwhile_to_take(cx: &mut gpui::TestAppContext) {
    use kanbrain_core::brain::Brain;
    let (view, cx, dir) = window(cx, &["a"]);
    cx.simulate_keystrokes("e end x");
    let mut agent = Brain::open(Store::at(dir.path()).unwrap()).unwrap();
    agent.author = Author::Agent;
    agent.update(1, "a, the agent's", vec![]).unwrap();
    cx.executor().advance_clock(crate::watch::INTERVAL);
    cx.run_until_parked();
    assert!(cx.debug_bounds("theirs-title").is_none(), "only once a save stops over it");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert!(cx.debug_bounds("theirs-title").is_some());
    click(cx, "button take-title");
    view.read_with(cx, |view, _| {
        assert!(matches!(&view.board.mode, Mode::Form(form) if form.title.text() == "a, the agent's"));
    });
    assert!(cx.debug_bounds("theirs-title").is_none(), "taken");
}

#[gpui::test]
fn the_conflict_being_picked_stands_out_and_is_scrolled_to(cx: &mut gpui::TestAppContext) {
    use kanbrain_core::brain::Brain;
    let (view, cx, dir) = window(cx, &["a"]);
    let kb = || Brain::open(Store::at(dir.path()).unwrap()).unwrap();
    let lines: Vec<String> = (0..80).map(|n| format!("line {n}")).collect();
    let text = |first: &str, last: &str| {
        let mut lines = lines.clone();
        lines[0] = first.into();
        lines[79] = last.into();
        lines.join("\n")
    };
    kb().set_description(1, &lines.join("\n")).unwrap();
    cx.executor().advance_clock(crate::watch::INTERVAL);
    cx.run_until_parked();
    cx.simulate_keystrokes("m");
    view.update(cx, |view, _| {
        view.board.select_all();
        view.board.paste(&text("first, mine", "last, mine"));
    });
    kb().set_description(1, &text("first, theirs", "last, theirs")).unwrap();
    cx.executor().advance_clock(crate::watch::INTERVAL);
    cx.run_until_parked();
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    let current = |cx: &mut gpui::VisualTestContext| cx.debug_bounds("current conflict").expect("one is current");
    let first = cx.debug_bounds("conflict 0").unwrap();
    assert!(first.contains(&current(cx).center()), "the first, to begin with");
    let window = cx.debug_bounds("merge").expect("the conflicts are drawn");
    let last = cx.debug_bounds("conflict 1").unwrap();
    assert!(last.top() > window.bottom(), "the last starts out of view");
    cx.simulate_keystrokes("tab");
    cx.run_until_parked();
    let last = cx.debug_bounds("conflict 1").unwrap();
    assert!(last.contains(&current(cx).center()), "the next is current");
    assert!(last.top() < window.bottom() && last.bottom() > window.top(), "and scrolled into view");
}

#[gpui::test]
fn what_kb_changed_goes_beside_the_inputs_message_and_the_next_inputs_replaces_it(cx: &mut gpui::TestAppContext) {
    let (view, cx, dir) = window(cx, &["a"]);
    let kb = || kanbrain_core::brain::Brain::open(Store::at(dir.path()).unwrap()).unwrap();
    let said = |view: &gpui::Entity<BrainView>, cx: &mut gpui::VisualTestContext| {
        view.read_with(cx, |view, _| view.board.message().map(str::to_string))
    };
    let card = cx.debug_bounds("card 0/1").expect("card 1 is drawn").center();
    cx.simulate_click(card, gpui::Modifiers::none());
    cx.simulate_keystrokes("e !");
    kb().set_status(1, Status::Doing).unwrap();
    cx.simulate_keystrokes("enter");
    assert_eq!(said(&view, cx).as_deref(), Some("#1 saved · #1's status changed on disk"));
    cx.simulate_keystrokes("e");
    kb().set_status(1, Status::Todo).unwrap();
    cx.executor().advance_clock(crate::watch::INTERVAL);
    cx.run_until_parked();
    assert_eq!(said(&view, cx).as_deref(), Some("#1's status changed on disk"));
    // Shown for as long as the timer takes to read again: the next save's message replaces it.
    cx.executor().advance_clock(crate::watch::INTERVAL);
    cx.run_until_parked();
    cx.simulate_keystrokes("? enter");
    assert_eq!(said(&view, cx).as_deref(), Some("#1 saved"));
}

/// Opens the details of task 1, its description `description` and its comments `comments`, saved: the next input
/// reads the files again.
fn details(view: &gpui::Entity<BrainView>, cx: &mut gpui::VisualTestContext, description: &str, comments: &[&str]) {
    view.update(cx, |view, cx| {
        let brain = view.board.brain_mut(0);
        brain.set_description(1, description).unwrap();
        for body in comments {
            brain.add_comment(1, body).unwrap();
        }
        view.board.selected = Some(crate::state::Key::new(0, 1));
        view.board.drawer = true;
        cx.notify();
    });
    cx.run_until_parked();
}

/// What the clipboard holds after ctrl+c (cmd+c on macOS).
fn copied(cx: &mut gpui::VisualTestContext) -> Option<String> {
    let ctrl = if cfg!(target_os = "macos") { "cmd" } else { "ctrl" };
    cx.simulate_keystrokes(&format!("{ctrl}-c"));
    cx.read_from_clipboard().and_then(|item| item.text())
}

/// Presses at `from`, drags to `to` and lets go there.
fn drag(cx: &mut gpui::VisualTestContext, from: gpui::Point<gpui::Pixels>, to: gpui::Point<gpui::Pixels>) {
    cx.simulate_mouse_down(from, MouseButton::Left, gpui::Modifiers::none());
    cx.simulate_mouse_move(to, MouseButton::Left, gpui::Modifiers::none());
    cx.simulate_mouse_up(to, MouseButton::Left, gpui::Modifiers::none());
}

#[gpui::test]
fn a_drag_over_a_description_copies_its_blocks_as_shown_on_lines_of_their_own(cx: &mut gpui::TestAppContext) {
    let (view, cx, _dir) = window(cx, &["a"]);
    details(&view, cx, "First **paragraph**\n\nSecond one\n\n- an item\n\n```\nlet code;\n```", &[]);
    let text = cx.debug_bounds("text 0/1 description").expect("the description is drawn");
    drag(cx, text.origin + gpui::point(px(1.), px(5.)), text.bottom_right() - gpui::point(px(1.), px(1.)));
    assert_eq!(copied(cx).as_deref(), Some("First paragraph\nSecond one\nan item\nlet code;"));
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    {
        let primary = view.update(cx, |_, cx| cx.read_from_primary()).and_then(|item| item.text());
        assert_eq!(primary.as_deref(), Some("First paragraph\nSecond one\nan item\nlet code;"), "and the primary");
    }
}

#[gpui::test]
fn a_double_click_in_a_description_selects_a_word(cx: &mut gpui::TestAppContext) {
    let (view, cx, _dir) = window(cx, &["a"]);
    details(&view, cx, "First paragraph", &[]);
    let text = cx.debug_bounds("text 0/1 description").expect("the description is drawn");
    double_click(cx, text.origin + gpui::point(px(2.), px(5.)));
    assert_eq!(copied(cx).as_deref(), Some("First"));
}

#[gpui::test]
fn a_drag_stays_in_the_comment_it_started_in(cx: &mut gpui::TestAppContext) {
    let (view, cx, _dir) = window(cx, &["a"]);
    details(&view, cx, "", &["one", "two"]);
    let first = cx.debug_bounds("thread comment 0").expect("the first comment is drawn");
    let second = cx.debug_bounds("thread comment 1").expect("the second comment is drawn");
    drag(cx, first.origin + gpui::point(px(9.), px(9.)), second.center());
    let copied = copied(cx).expect("the first comment, from its heading");
    assert!(copied.ends_with("\none") && !copied.contains("two"), "{copied:?}");
}

#[gpui::test]
fn a_click_on_a_link_in_shown_text_still_opens_it(cx: &mut gpui::TestAppContext) {
    let (view, cx, _dir) = window(cx, &["a"]);
    details(&view, cx, "[docs](https://x.dev)", &[]);
    let text = cx.debug_bounds("text 0/1 description").expect("the description is drawn");
    cx.simulate_click(text.origin + gpui::point(px(2.), px(5.)), gpui::Modifiers::none());
    assert_eq!(cx.opened_url().as_deref(), Some("https://x.dev"));
}

#[gpui::test]
fn copying_with_nothing_selected_leaves_the_clipboard_and_a_click_elsewhere_or_closing_drops_it(
    cx: &mut gpui::TestAppContext,
) {
    let (view, cx, dir) = window(cx, &["a", "b"]);
    cx.write_to_clipboard(gpui::ClipboardItem::new_string("before".into()));
    assert_eq!(copied(cx).as_deref(), Some("before"), "nothing selected, nothing copied");
    details(&view, cx, "Some text", &[]);
    let select = |cx: &mut gpui::VisualTestContext| {
        let text = cx.debug_bounds("text 0/1 description").expect("the description is drawn");
        double_click(cx, text.origin + gpui::point(px(2.), px(5.)));
    };
    select(cx);
    // A card dragged meanwhile still moves: cards aren't text to select.
    let card = cx.debug_bounds("card 0/2").expect("card 2 is drawn").center();
    let done = cx.debug_bounds("column done").expect("the done column is drawn").center();
    cx.simulate_mouse_down(card, MouseButton::Left, gpui::Modifiers::none());
    cx.simulate_mouse_move(card + gpui::point(px(10.), px(10.)), MouseButton::Left, gpui::Modifiers::none());
    cx.simulate_mouse_move(done, MouseButton::Left, gpui::Modifiers::none());
    cx.simulate_mouse_up(done, MouseButton::Left, gpui::Modifiers::none());
    let (tasks, _) = Store::at(dir.path()).unwrap().load_entries().unwrap();
    assert_eq!(tasks.iter().find(|t| t.id == 2).unwrap().status, Status::Done, "the card moved");
    view.read_with(cx, |view, _| assert_eq!(view.board.shown_text(), None, "the press on the card dropped it"));
    // Shown no more, the selection goes with it, though no click said so.
    details(&view, cx, "Some text", &[]);
    select(cx);
    assert_eq!(copied(cx).as_deref(), Some("Some"));
    cx.write_to_clipboard(gpui::ClipboardItem::new_string("before".into()));
    cx.simulate_keystrokes("escape");
    next_frames(cx);
    assert_eq!(copied(cx).as_deref(), Some("before"), "the details are closed");
}

/// Clicks three times near the start of the text `selector` names, as a triple-click: past a change line's padding.
fn triple_click(cx: &mut gpui::VisualTestContext, selector: &'static str) {
    let at = cx.debug_bounds(selector).unwrap_or_else(|| panic!("{selector} is drawn")).origin
        + gpui::point(px(10.), px(5.));
    for click_count in [1, 2, 3] {
        let (button, modifiers, first_mouse) = (MouseButton::Left, gpui::Modifiers::none(), false);
        cx.simulate_event(gpui::MouseDownEvent { position: at, button, modifiers, click_count, first_mouse });
        cx.simulate_event(gpui::MouseUpEvent { position: at, button, modifiers, click_count });
    }
}

#[test]
fn a_selection_is_highlighted_across_spans_keeping_their_styles() {
    use theme::{Span, Style};
    let line = theme::Line::from(vec![Span::styled("ab", Style::new().bold()), Span::raw("cdé"), Span::raw("f")]);
    let lit = text::highlighted(line, Some((1, 4)));
    let spans: Vec<_> = lit.0.iter().filter(|s| !s.content.is_empty()).map(|s| (s.content.as_str(), s.style)).collect();
    let selected = |style: Style| style.bg(theme::SELECTION);
    assert_eq!(
        spans,
        [
            ("a", Style::new().bold()),
            ("b", selected(Style::new().bold())),
            ("cd", selected(Style::new())),
            ("é", Style::new()),
            ("f", Style::new()),
        ]
    );
    assert_eq!(text::highlighted(theme::Line::raw("ab"), None).0.len(), 1, "nothing selected, nothing split");
}

#[gpui::test]
fn every_part_of_the_details_is_a_text_of_its_own_to_copy(cx: &mut gpui::TestAppContext) {
    let (view, cx, _dir) = window(cx, &["Fix login"]);
    view.update(cx, |view, cx| {
        let brain = view.board.brain_mut(0);
        brain.author = Author::Agent;
        brain.update(1, "Fix login 500", vec!["auth".into()]).unwrap();
        brain.ask(1, "Per IP or per account?").unwrap();
        view.board.selected = Some(crate::state::Key::new(0, 1));
        view.board.drawer = true;
        cx.notify();
    });
    cx.run_until_parked();
    let parts = [
        ("text 0/1 title", "Fix login 500"),
        ("text 0/1 tags", "auth"),
        ("text 0/1 created", "created "),
        ("text 0/1 question", "An agent asks, "),
        ("text 0/1 status", "Parked for "),
        ("text 0/1 time", "Tracked today "),
        ("thread change 0", "An agent changed the title"),
        ("text 0/1 history", ""),
    ];
    for (text, starts) in parts {
        triple_click(cx, text);
        let copied = copied(cx).unwrap_or_default();
        assert!(!copied.is_empty() && copied.starts_with(starts), "{text}: {copied:?}");
    }
    // The question's comment, from its heading to its end.
    let comment = cx.debug_bounds("thread comment 0").expect("the question is a comment too");
    drag(cx, comment.origin + gpui::point(px(9.), px(9.)), comment.bottom_right() - gpui::point(px(1.), px(1.)));
    assert!(copied(cx).is_some_and(|c| c.starts_with("Agent question") && c.ends_with("\nPer IP or per account?")));
    // A delete's question, and the message saying what was done.
    cx.simulate_keystrokes("x");
    triple_click(cx, "text confirm");
    assert!(copied(cx).is_some_and(|c| c.ends_with("Fix login 500")));
    cx.simulate_keystrokes("escape s");
    let said = view.read_with(cx, |view, _| view.board.message().map(str::to_string)).expect("s says what it did");
    triple_click(cx, "text message");
    assert_eq!(copied(cx), Some(said));
}

#[gpui::test]
fn shift_click_extends_and_a_paragraphs_second_line_takes_clicks(cx: &mut gpui::TestAppContext) {
    let (view, cx, _dir) = window(cx, &["a"]);
    details(&view, cx, "one\ntwo words", &[]);
    let text = cx.debug_bounds("text 0/1 description").expect("the description is drawn");
    cx.simulate_click(text.origin + gpui::point(px(1.), px(5.)), gpui::Modifiers::none());
    cx.simulate_click(text.bottom_right() - gpui::point(px(1.), px(1.)), gpui::Modifiers::shift());
    assert_eq!(copied(cx).as_deref(), Some("one\ntwo words"));
    let at = gpui::point(text.left() + px(2.), text.bottom() - px(4.));
    double_click(cx, at);
    assert_eq!(copied(cx).as_deref(), Some("two"));
    // A click without shift in the same text drops the selection, extending nothing.
    cx.simulate_click(text.origin + gpui::point(px(1.), px(5.)), gpui::Modifiers::none());
    view.read_with(cx, |view, _| assert_eq!(view.board.shown_text(), None));
    // A paragraph after the first that wraps: its first row is under the first paragraph, not past its own end.
    let row = text.size.height / 2.;
    details(&view, cx, &format!("one\nalpha {}omega", "word ".repeat(40)), &[]);
    let text = cx.debug_bounds("text 0/1 description").expect("the description is drawn");
    double_click(cx, gpui::point(text.left() + px(2.), text.top() + row * 1.5));
    assert_eq!(copied(cx).as_deref(), Some("alpha"), "{text:?}");
}

#[gpui::test]
fn a_selection_under_a_dialog_isnt_copied_from_it(cx: &mut gpui::TestAppContext) {
    let (view, cx, _dir) = window(cx, &["a"]);
    for keys in ["m", "e", "x"] {
        details(&view, cx, "Some text", &[]);
        let text = cx.debug_bounds("text 0/1 description").expect("the description is drawn");
        double_click(cx, text.origin + gpui::point(px(2.), px(5.)));
        assert_eq!(copied(cx).as_deref(), Some("Some"), "selected while nothing is over it");
        cx.write_to_clipboard(gpui::ClipboardItem::new_string("before".into()));
        cx.simulate_keystrokes(keys);
        next_frames(cx);
        view.read_with(cx, |view, _| assert!(!matches!(view.board.mode, Mode::Normal), "`{keys}` opens a dialog"));
        assert_eq!(copied(cx).as_deref(), Some("before"), "ctrl+c in the dialog opened by `{keys}`, nothing selected");
        cx.simulate_keystrokes("escape");
        next_frames(cx);
    }
}

/// A window smaller than the board: 30 tasks to do, more than their column shows, and four columns wider than it.
fn small_board(
    cx: &mut gpui::TestAppContext,
) -> (gpui::Entity<BrainView>, &mut gpui::VisualTestContext, tempfile::TempDir) {
    small_board_of(cx, 30)
}

/// [`small_board`] with `tasks` to do.
fn small_board_of(
    cx: &mut gpui::TestAppContext,
    tasks: u32,
) -> (gpui::Entity<BrainView>, &mut gpui::VisualTestContext, tempfile::TempDir) {
    let titles: Vec<String> = (1..=tasks).map(|n| format!("task {n}")).collect();
    let (view, cx, dir) = window(cx, &titles.iter().map(String::as_str).collect::<Vec<_>>());
    cx.simulate_resize(gpui::size(px(600.), px(400.)));
    next_frames(cx);
    (view, cx, dir)
}

/// How far the to-do column has scrolled down and the board to the right.
fn scrolled(view: &gpui::Entity<BrainView>, cx: &mut gpui::VisualTestContext) -> (f32, f32) {
    view.read_with(cx, |view, _| {
        (-view.scroll.columns[0].scroll.offset().y.as_f32(), -view.scroll.board.offset().x.as_f32())
    })
}

/// Turns a wheel or swipes a touchpad by `delta` over the to-do column, with shift held or not: how far that
/// scrolled the column down and the board to the right.
fn wheel(
    view: &gpui::Entity<BrainView>,
    cx: &mut gpui::VisualTestContext,
    delta: gpui::ScrollDelta,
    shift: bool,
) -> (f32, f32) {
    let column = cx.debug_bounds("column todo").expect("the to-do column is drawn");
    wheel_at(view, cx, column.center(), delta, shift)
}

/// [`wheel`] at `position`.
fn wheel_at(
    view: &gpui::Entity<BrainView>,
    cx: &mut gpui::VisualTestContext,
    position: gpui::Point<gpui::Pixels>,
    delta: gpui::ScrollDelta,
    shift: bool,
) -> (f32, f32) {
    let before = scrolled(view, cx);
    let modifiers = gpui::Modifiers { shift, ..gpui::Modifiers::none() };
    let touch_phase = gpui::TouchPhase::Moved;
    cx.simulate_event(gpui::ScrollWheelEvent { position, delta, modifiers, touch_phase });
    next_frames(cx);
    let after = scrolled(view, cx);
    (after.0 - before.0, after.1 - before.1)
}

#[gpui::test]
fn a_vertical_wheel_scrolls_a_column_and_not_the_board(cx: &mut gpui::TestAppContext) {
    let (view, cx, _dir) = small_board(cx);
    let (down, right) = wheel(&view, cx, gpui::ScrollDelta::Lines(gpui::point(0., -3.)), false);
    assert!(down > 0., "the column scrolls down: {down}");
    assert_eq!(right, 0., "the board stays where it is");
}

#[gpui::test]
fn a_sideways_wheel_or_shift_with_the_wheel_scrolls_the_board_and_not_a_column(cx: &mut gpui::TestAppContext) {
    let (view, cx, _dir) = small_board(cx);
    let (down, right) = wheel(&view, cx, gpui::ScrollDelta::Lines(gpui::point(-3., 0.)), false);
    assert!(right > 0., "a horizontal wheel scrolls the board: {right}");
    assert_eq!(down, 0., "and not the column under the pointer");
    // Every platform hands shift and the wheel over already turned sideways: it isn't turned again.
    let (down, right) = wheel(&view, cx, gpui::ScrollDelta::Lines(gpui::point(-3., 0.)), true);
    assert!(right > 0., "shift and the wheel scroll the board: {right}");
    assert_eq!(down, 0., "and not the column");
}

#[gpui::test]
fn a_touchpad_scrolls_each_way_its_fingers_go(cx: &mut gpui::TestAppContext) {
    let (view, cx, _dir) = small_board(cx);
    let pixels = |x, y| gpui::ScrollDelta::Pixels(gpui::point(px(x), px(y)));
    let (down, right) = wheel(&view, cx, pixels(0., -40.), false);
    assert!(down > 0. && right == 0., "up and down scrolls the column only: {down}, {right}");
    let (down, right) = wheel(&view, cx, pixels(-40., 0.), false);
    assert!(down == 0. && right > 0., "sideways scrolls the board only: {down}, {right}");
    let (down, right) = wheel(&view, cx, pixels(-30., -20.), false);
    assert!(down > 0. && right > 0., "a slanting swipe scrolls both, not the way it mostly goes: {down}, {right}");
}
/// Draws the window again, as a move of it does: the first frame of a list draws every row, and lays them out for the
/// next to know which are in view.
fn redraw(cx: &mut gpui::VisualTestContext) {
    cx.update(|window, _| window.refresh());
    next_frames(cx);
}

/// Whether `selector` is drawn, and inside the area `within` is drawn in.
fn shown_in(cx: &mut gpui::VisualTestContext, selector: &'static str, within: &'static str) -> bool {
    let area = cx.debug_bounds(within).expect("the area is drawn");
    cx.debug_bounds(selector).is_some_and(|row| row.top() >= area.top() && row.bottom() <= area.bottom())
}

#[gpui::test]
fn a_long_columns_cards_out_of_view_stand_in_at_their_height(cx: &mut gpui::TestAppContext) {
    let (view, cx, _dir) = small_board(cx);
    // To do lists the newest first: #30 on top, #1 at the bottom, far out of view.
    assert!(shown_in(cx, "card 0/30", "column todo"), "the cards in view are drawn");
    assert!(cx.debug_bounds("card 0/1").is_none(), "one far out of view isn't");
    // Every card is a line high: 30 of them, a gap between each and the column's padding around them.
    let card = cx.debug_bounds("card 0/30").expect("the top card is drawn").size.height;
    let (shown, max) = view.read_with(cx, |view, _| {
        let scroll = &view.scroll.columns[0].scroll;
        (scroll.bounds().size.height, scroll.max_offset().y)
    });
    let (gap, padding) = (px(8.), px(8.));
    let long = card * 30. + gap * 29. + padding * 2.;
    assert!((max - (long - shown)).abs() < px(1.), "each stands in as tall as it is: {max:?} for {long:?} long");
    wheel(&view, cx, gpui::ScrollDelta::Pixels(gpui::point(px(0.), -max)), false);
    assert!(shown_in(cx, "card 0/1", "column todo"), "scrolled to, it is");
    assert!(cx.debug_bounds("card 0/30").is_none(), "and the one now far out of view isn't");
}

/// How many rows of the to-do column stand in where the column shows them, as the last frame laid it out.
fn blanks(view: &gpui::Entity<BrainView>, cx: &mut gpui::VisualTestContext) -> usize {
    view.read_with(cx, |view, _| {
        let rows = &view.scroll.columns[0];
        let shown = rows.scroll.bounds();
        let top = shown.top() - rows.scroll.offset().y;
        let shown = |row: gpui::Bounds<gpui::Pixels>| row.bottom() > top && row.top() < top + shown.size.height;
        rows.stood_in().into_iter().filter(|&ix| rows.scroll.bounds_for_item(ix).is_some_and(shown)).count()
    })
}

/// How many frames so far drew a blank in the to-do column where a card should be.
fn blank_frames(view: &gpui::Entity<BrainView>, cx: &mut gpui::VisualTestContext) -> usize {
    view.read_with(cx, |view, _| view.scroll.columns[0].blank_frames())
}

/// Scrolls the to-do column to its end.
fn to_the_end(view: &gpui::Entity<BrainView>, cx: &mut gpui::VisualTestContext) {
    let max = view.read_with(cx, |view, _| view.scroll.columns[0].scroll.max_offset().y);
    wheel(view, cx, gpui::ScrollDelta::Pixels(gpui::point(px(0.), -max)), false);
}

#[gpui::test]
fn a_search_that_brings_rows_into_view_draws_them_in_the_same_frame(cx: &mut gpui::TestAppContext) {
    let (view, cx, _dir) = small_board(cx);
    to_the_end(&view, cx);
    assert!(!view.read_with(cx, |view, _| view.scroll.columns[0].stood_in()).is_empty(), "the top cards stand in");
    let before = blank_frames(&view, cx);
    // Matches #2 and #20 to #29, which were at the top, far out of view: the column is short now, back at its top.
    cx.simulate_keystrokes("/ t a s k space 2");
    next_frames(cx);
    assert_eq!(blank_frames(&view, cx), before, "no frame showed a blank where a card goes");
    assert!(shown_in(cx, "card 0/29", "column todo"), "the first match is drawn");
}

#[gpui::test]
fn a_column_left_shorter_than_its_scroll_draws_what_it_scrolls_back_to(cx: &mut gpui::TestAppContext) {
    let (view, cx, dir) = small_board(cx);
    to_the_end(&view, cx);
    view.update(cx, |view, _| view.board.selected = None);
    let before = blank_frames(&view, cx);
    // An agent finishes the top 20, far out of view: the column's 10 left are shorter than it was scrolled.
    let mut cli = kanbrain_core::brain::Brain::open(Store::at(dir.path()).unwrap()).unwrap();
    for id in 11..=30 {
        cli.set_status(id, Status::Done).unwrap();
    }
    cx.executor().advance_clock(crate::watch::INTERVAL);
    cx.run_until_parked();
    next_frames(cx);
    assert_eq!(blank_frames(&view, cx), before, "no frame showed a blank where a card goes");
    assert!(cx.debug_bounds("card 0/1").is_some(), "the last card is drawn");
}

#[gpui::test]
fn adding_at_the_top_of_a_column_draws_the_cards_under_the_field(cx: &mut gpui::TestAppContext) {
    let (view, cx, _dir) = small_board(cx);
    to_the_end(&view, cx);
    let card = cx.debug_bounds("card 0/1").expect("the bottom card is drawn");
    cx.simulate_click(card.center(), gpui::Modifiers::none());
    next_frames(cx);
    let before = blank_frames(&view, cx);
    // The last card selected, at the bottom: the field opens at the top, which the column scrolls to.
    cx.simulate_keystrokes("a x");
    next_frames(cx);
    assert_eq!(blank_frames(&view, cx), before, "no frame showed a blank where a card goes");
    // The column is shorter than the field and a card.
    let (card, column) = (cx.debug_bounds("card 0/30"), cx.debug_bounds("column todo").expect("the column is drawn"));
    assert!(card.is_some_and(|card| column.contains(&card.origin)), "the card under the field is drawn: {card:?}");
}

#[gpui::test]
fn selecting_a_card_far_out_of_view_draws_it_where_the_column_jumps_to(cx: &mut gpui::TestAppContext) {
    let (view, cx, _dir) = small_board(cx);
    let before = blank_frames(&view, cx);
    // What `w` or a reference's link does: the selection lands far down, and the column follows it there.
    view.update(cx, |view, cx| {
        view.board.selected = Some(crate::state::Key::new(0, 3));
        view.follow = true;
        cx.notify();
    });
    next_frames(cx);
    assert_eq!(blank_frames(&view, cx), before, "no frame showed a blank where a card goes");
    assert!(shown_in(cx, "card 0/3", "column todo"), "the card is drawn, in view");
}

#[gpui::test]
fn tall_rows_added_far_above_the_view_draw_no_blank(cx: &mut gpui::TestAppContext) {
    let (view, cx, dir) = small_board(cx);
    to_the_end(&view, cx);
    let before = blank_frames(&view, cx);
    // An agent adds tasks with long titles, on top of the column, far above the view: they stand in, reckoned as tall
    // as the cards are on average, and draw only once scrolled near.
    let store = Store::at(dir.path()).unwrap();
    for id in 31..=40 {
        let title = "a task with a title long enough to wrap onto several lines of a narrow column".to_string();
        store.save(&mut Task::new(id, title, now())).unwrap();
    }
    cx.executor().advance_clock(crate::watch::INTERVAL);
    cx.run_until_parked();
    next_frames(cx);
    assert_eq!(blank_frames(&view, cx), before, "no frame showed a blank where a card goes");
    assert!(cx.debug_bounds("card 0/40").is_none(), "the new cards, far above, stand in");
}

#[gpui::test]
fn a_column_scrolled_to_its_end_stays_there_as_its_cards_turn_out_taller(cx: &mut gpui::TestAppContext) {
    // The top cards are a line each; the 100 under them, never laid out until scrolled to, wrap onto two lines.
    let titles: Vec<String> = (1..=200)
        .map(|n| match n {
            ..=100 => format!("task {n} with a title long enough to wrap onto several lines of a narrow column"),
            _ => format!("task {n}"),
        })
        .collect();
    let (view, cx, _dir) = window(cx, &titles.iter().map(String::as_str).collect::<Vec<_>>());
    cx.simulate_resize(gpui::size(px(600.), px(400.)));
    next_frames(cx);
    // Reckoned as tall as the cards laid out, those at the end make the column longer once drawn: GPUI keeps the
    // scroll where it was, short of the end.
    to_the_end(&view, cx);
    let (offset, max) = view.read_with(cx, |view, _| {
        let scroll = &view.scroll.columns[0].scroll;
        (-scroll.offset().y, scroll.max_offset().y)
    });
    assert!((offset - max).abs() < px(1.), "still at its end: {offset:?} of {max:?}");
    // Taller than the column shows, the last card ends in view, at its bottom.
    let column = cx.debug_bounds("column todo").expect("the column is drawn");
    let last = cx.debug_bounds("card 0/1");
    let at_bottom = last.is_some_and(|card| card.bottom() <= column.bottom() && card.bottom() > column.top());
    assert!(at_bottom, "the last card ends in view: {last:?} in {column:?}");
    assert_eq!(blanks(&view, cx), 0, "no blank in view");
}

#[gpui::test]
fn a_window_made_taller_draws_the_cards_it_brings_into_view_the_next_frame(cx: &mut gpui::TestAppContext) {
    // Ten cards: more than the view holds, fewer than the window made taller does.
    let (view, cx, _dir) = small_board_of(cx, 10);
    let before = blank_frames(&view, cx);
    // Maximized: the frame after is laid out for the old height, the box standing in for the cards under the view
    // now in it.
    cx.simulate_resize(gpui::size(px(600.), px(1600.)));
    next_frames(cx);
    assert!(blank_frames(&view, cx) > before, "a frame showed the cards brought into view as blanks");
    assert_eq!(blanks(&view, cx), 0, "the next drew them");
    assert!(shown_in(cx, "card 0/1", "column todo"), "every card in view");
}

#[gpui::test]
fn a_card_the_keys_select_out_of_view_is_drawn_where_it_scrolls_to(cx: &mut gpui::TestAppContext) {
    let (view, cx, _dir) = small_board(cx);
    let card = cx.debug_bounds("card 0/30").expect("the top card is drawn");
    cx.simulate_click(card.center(), gpui::Modifiers::none());
    next_frames(cx);
    for _ in 0..29 {
        cx.simulate_keystrokes("down");
    }
    next_frames(cx);
    view.read_with(cx, |view, _| assert_eq!(view.board.selected, Some(crate::state::Key::new(0, 1))));
    assert!(shown_in(cx, "card 0/1", "column todo"), "the last card is drawn, in view");
}

#[gpui::test]
fn the_list_layout_draws_the_rows_in_view(cx: &mut gpui::TestAppContext) {
    // Rows are a line each: it takes more than a column's cards to run far out of view.
    let (view, cx, _dir) = small_board_of(cx, 100);
    cx.simulate_keystrokes("v");
    next_frames(cx);
    redraw(cx);
    assert!(shown_in(cx, "row 0/100", "task-list"), "the rows in view are drawn");
    assert!(cx.debug_bounds("row 0/1").is_none(), "one far out of view isn't");
    let max = view.read_with(cx, |view, _| view.scroll.tasks.scroll.max_offset().y);
    let list = cx.debug_bounds("task-list").expect("the list is drawn");
    wheel_at(&view, cx, list.center(), gpui::ScrollDelta::Pixels(gpui::point(px(0.), -max)), false);
    // The last to do, with the other sections' headers and hints under it at the end.
    assert!(cx.debug_bounds("row 0/1").is_some(), "scrolled to near it, it is");
}

/// The most children the to-do column laid out in a frame since last asked.
fn most_laid_out(view: &gpui::Entity<BrainView>, cx: &mut gpui::VisualTestContext) -> usize {
    view.read_with(cx, |view, _| view.scroll.columns[0].take_most_laid_out())
}

#[gpui::test]
fn a_long_columns_length_is_that_of_its_cards_as_they_wrap(cx: &mut gpui::TestAppContext) {
    // 300 cards alike, a few of them laid out: those never laid out are reckoned as tall as those that were.
    let titles: Vec<String> = (1..=300)
        .map(|n| format!("task {n:03} with a title as long as one line of a wide column, and some more"))
        .collect();
    let (view, cx, _dir) = window(cx, &titles.iter().map(String::as_str).collect::<Vec<_>>());
    let pitch_and_length = |view: &gpui::Entity<BrainView>, cx: &mut gpui::VisualTestContext| {
        let mut card = |id: u32| cx.debug_bounds(format!("card 0/{id}").leak()).expect("the newest cards are drawn");
        // Over five cards past the first, which is selected and shows more.
        let pitch = (card(294).top() - card(299).top()) / 5.;
        let length = view.read_with(cx, |view, _| {
            let scroll = &view.scroll.columns[0].scroll;
            scroll.max_offset().y + scroll.bounds().size.height
        });
        (pitch, length)
    };
    next_frames(cx);
    let (wide, length) = pitch_and_length(&view, cx);
    // Within ten cards: the column's padding and the items that aren't cards add a few hundred pixels.
    assert!((length - wide * 300.).abs() < wide * 10., "300 cards of {wide:?} each: {length:?} long");
    // Narrower, the cards laid out wrap onto more lines: so do those reckoned.
    cx.simulate_resize(gpui::size(px(600.), px(800.)));
    next_frames(cx);
    // Learnt from a layout, the heights reckon the frame after it.
    redraw(cx);
    let (narrow, length) = pitch_and_length(&view, cx);
    assert!(narrow > wide, "the cards wrap onto more lines: {narrow:?}, were {wide:?}");
    assert!((length - narrow * 300.).abs() < narrow * 10., "300 cards of {narrow:?} each: {length:?} long");
}

#[gpui::test]
fn a_long_column_lays_out_only_the_cards_near_the_view(cx: &mut gpui::TestAppContext) {
    let (view, cx, _dir) = small_board_of(cx, 2000);
    // A view holds about ten cards: those, a view's worth on either side, and a box above and below for the rest.
    let near = 60;
    let opened = most_laid_out(&view, cx);
    assert!(opened < near, "opening lays out {opened} children for 2,000 cards");
    assert!(shown_in(cx, "card 0/2000", "column todo"), "the top card is drawn");
    to_the_end(&view, cx);
    assert!(shown_in(cx, "card 0/1", "column todo"), "scrolled to the end, the last card is drawn");
    let scrolled = most_laid_out(&view, cx);
    assert!(scrolled < near, "scrolling lays out {scrolled}");
    // Matches #1, #10 to #19, #100 to #199 and #1000 to #1999, most of them never laid out; then every card again.
    cx.simulate_keystrokes("/ t a s k space 1");
    next_frames(cx);
    let searched = most_laid_out(&view, cx);
    assert!(searched < near, "a search lays out {searched}");
    cx.simulate_keystrokes("escape escape");
    next_frames(cx);
    let cleared = most_laid_out(&view, cx);
    assert!(cleared < near, "clearing it lays out {cleared}");
    assert_eq!(blanks(&view, cx), 0, "no blank in view");
}

#[gpui::test]
fn a_cleared_search_puts_every_card_back_where_it_was(cx: &mut gpui::TestAppContext) {
    // Every other card wraps onto several lines: one card's height says little of another's.
    let titles: Vec<String> = (1..=20)
        .map(|n| match n % 2 {
            0 => format!("task {n} with a title long enough to wrap onto several lines of a narrow column"),
            _ => format!("task {n}"),
        })
        .collect();
    let (view, cx, _dir) = window(cx, &titles.iter().map(String::as_str).collect::<Vec<_>>());
    cx.simulate_resize(gpui::size(px(600.), px(400.)));
    next_frames(cx);
    // Each card laid out once, the column knows its length.
    to_the_end(&view, cx);
    wheel(&view, cx, gpui::ScrollDelta::Pixels(gpui::point(px(0.), px(100_000.))), false);
    let length = |view: &gpui::Entity<BrainView>, cx: &mut gpui::VisualTestContext| {
        view.read_with(cx, |view, _| view.scroll.columns[0].scroll.max_offset().y)
    };
    let before = length(&view, cx);
    // Only the long ones: the cards laid out are all much taller than the short ones left out.
    cx.simulate_keystrokes("/ w r a p");
    next_frames(cx);
    cx.simulate_keystrokes("escape escape");
    next_frames(cx);
    let after = length(&view, cx);
    assert!((after - before).abs() < px(1.), "the column as long as before the search: {after:?}, not {before:?}");
}

#[gpui::test]
fn a_column_cut_to_one_card_by_a_search_keeps_its_length_once_cleared(cx: &mut gpui::TestAppContext) {
    // One card far down matching: the column cut to it has no gap between two cards to show.
    let titles: Vec<String> =
        (1..=300).map(|n| if n == 150 { format!("task {n} needle") } else { format!("task {n}") }).collect();
    let (view, cx, _dir) = window(cx, &titles.iter().map(String::as_str).collect::<Vec<_>>());
    cx.simulate_resize(gpui::size(px(600.), px(400.)));
    next_frames(cx);
    cx.simulate_keystrokes("/ n e e d l e");
    next_frames(cx);
    click(cx, "card 0/150");
    // Cleared, the column follows the card selected, far from its top: the first child is the box above it.
    cx.simulate_keystrokes("escape escape");
    next_frames(cx);
    redraw(cx);
    let pitch = cx.debug_bounds("card 0/148").unwrap().top() - cx.debug_bounds("card 0/149").unwrap().top();
    let length = view.read_with(cx, |view, _| {
        let scroll = &view.scroll.columns[0].scroll;
        scroll.max_offset().y + scroll.bounds().size.height
    });
    assert!((length - pitch * 300.).abs() < pitch * 3., "300 cards {pitch:?} apart: {length:?} long");
}

#[gpui::test]
fn a_list_scrolled_part_way_stays_put_frame_after_frame(cx: &mut gpui::TestAppContext) {
    // The list layout: a section's header above the view, as tall as no row, and rows reckoned, not laid out.
    let (view, cx, _dir) = small_board_of(cx, 300);
    cx.simulate_keystrokes("v");
    next_frames(cx);
    let list = cx.debug_bounds("task-list").expect("the list is drawn");
    wheel_at(&view, cx, list.center(), gpui::ScrollDelta::Pixels(gpui::point(px(0.), px(-3000.))), false);
    next_frames(cx);
    let offset = |view: &gpui::Entity<BrainView>, cx: &mut gpui::VisualTestContext| {
        view.read_with(cx, |view, _| view.scroll.tasks.scroll.offset().y)
    };
    let before = offset(&view, cx);
    for _ in 0..10 {
        redraw(cx);
    }
    assert_eq!(offset(&view, cx), before, "frames that change nothing leave the scroll where it was");
}

#[gpui::test]
fn a_column_that_fits_stays_at_its_top_as_it_grows(cx: &mut gpui::TestAppContext) {
    let (view, cx, dir) = small_board_of(cx, 1);
    // An agent adds forty: the column, at its top and its end both before, is long now, the newest on top.
    let store = Store::at(dir.path()).unwrap();
    for id in 2..=41 {
        store.save(&mut Task::new(id, format!("task {id}"), now())).unwrap();
    }
    cx.executor().advance_clock(crate::watch::INTERVAL);
    cx.run_until_parked();
    next_frames(cx);
    assert_eq!(scrolled(&view, cx).0, 0., "still at its top");
    assert!(shown_in(cx, "card 0/41", "column todo"), "the newest card in view");
}

/// The first card wholly in the to-do column's view, of those with ids up to `last`, and its top.
fn first_in_view(cx: &mut gpui::VisualTestContext, last: u32) -> (u32, gpui::Pixels) {
    let column = cx.debug_bounds("column todo").expect("the column is drawn");
    (1..=last)
        .filter_map(|id| {
            let card = cx.debug_bounds(format!("card 0/{id}").leak())?;
            (card.top() >= column.top() && card.bottom() <= column.bottom()).then_some((id, card.top()))
        })
        .min_by(|a, b| a.1.as_f32().total_cmp(&b.1.as_f32()))
        .expect("a card in view")
}

#[gpui::test]
fn the_card_in_view_stays_put_as_the_cards_above_it_are_reckoned_anew(cx: &mut gpui::TestAppContext) {
    // The top 20 are a line each, laid out on opening; the 280 under them, never laid out, wrap onto two.
    let titles: Vec<String> = (1..=300)
        .map(|n| match n {
            ..=280 => format!("task {n} with a title long enough to wrap onto several lines of a narrow column"),
            _ => format!("task {n}"),
        })
        .collect();
    let (_view, cx, _dir) = window(cx, &titles.iter().map(String::as_str).collect::<Vec<_>>());
    cx.simulate_resize(gpui::size(px(600.), px(800.)));
    next_frames(cx);
    // One turn of the wheel far down, and the frame it draws: the cards near the view laid out, those above it never
    // laid out reckoned as tall as the short ones. Taller than a 400 px window shows, no card would be wholly in view.
    let column = cx.debug_bounds("column todo").expect("the column is drawn");
    let delta = gpui::ScrollDelta::Pixels(gpui::point(px(0.), px(-8000.)));
    let (modifiers, touch_phase) = (gpui::Modifiers::none(), gpui::TouchPhase::Moved);
    cx.simulate_event(gpui::ScrollWheelEvent { position: column.center(), delta, modifiers, touch_phase });
    let (card, top) = first_in_view(cx, 300);
    // The cards reckoned above it now as tall as those laid out on average: the column longer above the view.
    next_frames(cx);
    redraw(cx);
    let moved = cx.debug_bounds(format!("card 0/{card}").leak()).map(|c| c.top() - top);
    assert!(moved.is_some_and(|d| d.abs() < px(1.)), "card {card} stays where it showed: moved {moved:?}");
}

#[gpui::test]
fn a_column_scrolled_part_way_left_shorter_draws_what_it_scrolls_back_to(cx: &mut gpui::TestAppContext) {
    let (view, cx, dir) = small_board(cx);
    wheel(&view, cx, gpui::ScrollDelta::Pixels(gpui::point(px(0.), px(-2000.))), false);
    let (offset, max) = view.read_with(cx, |view, _| {
        let scroll = &view.scroll.columns[0].scroll;
        (-scroll.offset().y, scroll.max_offset().y)
    });
    assert!(offset > px(0.) && offset < max, "part way down, not at its end: {offset:?} of {max:?}");
    view.update(cx, |view, _| view.board.selected = None);
    let before = blank_frames(&view, cx);
    // An agent finishes the top 20: the column's 10 left are shorter than it was scrolled.
    let mut cli = kanbrain_core::brain::Brain::open(Store::at(dir.path()).unwrap()).unwrap();
    for id in 11..=30 {
        cli.set_status(id, Status::Done).unwrap();
    }
    cx.executor().advance_clock(crate::watch::INTERVAL);
    cx.run_until_parked();
    next_frames(cx);
    assert_eq!(blank_frames(&view, cx), before, "no frame showed a blank where a card goes");
    assert!(cx.debug_bounds("card 0/1").is_some(), "the last card is drawn");
}

/// A window over a fresh folder holding 100 notes, smaller than their list, on the notes tab.
fn many_notes(
    cx: &mut gpui::TestAppContext,
) -> (gpui::Entity<BrainView>, &mut gpui::VisualTestContext, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::at(dir.path()).unwrap();
    for id in 1..=100 {
        let mut note = Task::new(id, format!("note {id}"), now());
        note.kind = Kind::Note;
        store.save(&mut note).unwrap();
    }
    let board = Board::open(dir.path(), None).unwrap();
    let (view, cx) = cx.add_window_view(|window, cx| BrainView::new(board, window, cx));
    cx.simulate_resize(gpui::size(px(600.), px(400.)));
    cx.simulate_keystrokes("2");
    next_frames(cx);
    redraw(cx);
    (view, cx, dir)
}

#[gpui::test]
fn the_notes_list_draws_the_entries_in_view(cx: &mut gpui::TestAppContext) {
    let (view, cx, _dir) = many_notes(cx);
    let (first, last) = view.read_with(cx, |view, _| {
        let entries = view.board.entries();
        (entries[0].1.id, entries[entries.len() - 1].1.id)
    });
    let [first, last]: [&'static str; 2] = [first, last].map(|id| format!("entry 0/{id}").leak() as &str);
    assert!(shown_in(cx, first, "entries"), "the entries in view are drawn");
    assert!(cx.debug_bounds(last).is_none(), "one far out of view isn't");
    let max = view.read_with(cx, |view, _| view.scroll.list.scroll.max_offset().y);
    let area = cx.debug_bounds("entries").expect("the list is drawn");
    wheel_at(&view, cx, area.center(), gpui::ScrollDelta::Pixels(gpui::point(px(0.), -max)), false);
    assert!(shown_in(cx, last, "entries"), "scrolled to, it is");
}

#[gpui::test]
fn the_notes_list_scrolls_to_an_entry_selected_far_out_of_view(cx: &mut gpui::TestAppContext) {
    let (view, cx, _dir) = many_notes(cx);
    let last = view.read_with(cx, |view, _| view.board.entries().last().unwrap().0);
    view.update(cx, |view, cx| {
        view.board.selected = Some(last);
        view.follow = true;
        cx.notify();
    });
    next_frames(cx);
    let selector: &'static str = format!("entry 0/{}", last.id).leak();
    assert!(shown_in(cx, selector, "entries"), "the entry is drawn, in view");
}

#[gpui::test]
fn a_rows_line_of_text_starts_under_its_title(cx: &mut gpui::TestAppContext) {
    let (central, code) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let repo = code.path().join("blog/.brain");
    for dir in [central.path(), repo.as_path()] {
        let mut note = Task::new(1, "pool size".to_string(), now());
        note.kind = Kind::Note;
        note.description = "Ten connections.".to_string();
        Store::at(dir).unwrap().save(&mut note).unwrap();
    }
    kanbrain_core::projects::register(central.path(), "blog", &repo).unwrap();
    let board = Board::open(central.path(), None).unwrap();
    let (view, cx) = cx.add_window_view(|window, cx| BrainView::new(board, window, cx));
    cx.simulate_keystrokes("2");
    next_frames(cx);
    let left = |cx: &mut gpui::VisualTestContext, selector: String| {
        cx.debug_bounds(selector.clone().leak()).unwrap_or_else(|| panic!("{selector} is drawn")).left()
    };
    // Both brains shown: each row names its own, which the line starts past too.
    for src in [0, 1] {
        assert_eq!(left(cx, format!("entry {src}/1 line")), left(cx, format!("entry {src}/1 title")), "brain {src}");
    }
    view.update(cx, |view, cx| {
        view.board.scope = vec![crate::state::Scope::Central];
        cx.notify();
    });
    next_frames(cx);
    assert_eq!(left(cx, "entry 0/1 line".into()), left(cx, "entry 0/1 title".into()), "one brain, no label");
}

#[test]
fn a_rows_line_is_its_texts_first_else_its_first_heading() {
    use super::entries::first_line;
    assert_eq!(first_line("\n  \n# Pool\n\nTen connections.\nMore."), Some("Ten connections."));
    assert_eq!(first_line("# Pool\n## Size"), Some("# Pool"), "headings alone: the first");
    assert_eq!(first_line(" \n"), None, "nothing but blanks");
}

#[gpui::test]
fn only_the_done_column_offers_its_older_tasks(cx: &mut gpui::TestAppContext) {
    let titles: Vec<String> = (1..=crate::state::DONE_LIMIT + 3).map(|n| format!("task {n}")).collect();
    let (view, cx, _dir) = window(cx, &titles.iter().map(String::as_str).collect::<Vec<_>>());
    view.update(cx, |view, cx| {
        for id in 1..=u32::try_from(titles.len()).unwrap() {
            view.board.brain_mut(0).set_status(id, Status::Done).unwrap();
        }
        cx.notify();
    });
    next_frames(cx);
    let button = cx.debug_bounds("all done").expect("the done column offers the older ones");
    let done = cx.debug_bounds("column done").expect("the done column is drawn");
    // At the column's end, under its cards: below the window, but in the column, not another.
    assert!((done.left()..done.right()).contains(&button.center().x), "in the done column");
    // Tall enough for the button to show, and take a click.
    cx.simulate_resize(gpui::size(px(1400.), px(3000.)));
    next_frames(cx);
    let button = cx.debug_bounds("all done").expect("the button is drawn");
    cx.simulate_click(button.center(), gpui::Modifiers::none());
    view.read_with(cx, |view, _| assert!(view.board.all_done, "a click shows them all"));
    next_frames(cx);
    let button = cx.debug_bounds("all done").expect("the button is drawn");
    cx.simulate_click(button.center(), gpui::Modifiers::none());
    view.read_with(cx, |view, _| assert!(!view.board.all_done, "another shows the latest again"));
}

#[gpui::test]
fn the_list_layout_hints_only_at_empty_sections(cx: &mut gpui::TestAppContext) {
    let (_view, cx, _dir) = window(cx, &["a"]);
    cx.simulate_keystrokes("v");
    next_frames(cx);
    assert!(cx.debug_bounds("hint todo").is_none(), "to do has a task");
    assert!(cx.debug_bounds("hint doing").is_some(), "doing has none");
}

/// Comes to `from`, presses the left button there, drags to `to` by way of halfway, lets go and draws what that
/// asked for.
fn drag_on(cx: &mut gpui::VisualTestContext, from: gpui::Point<gpui::Pixels>, to: gpui::Point<gpui::Pixels>) {
    let none = gpui::Modifiers::none();
    point_at(cx, from);
    cx.simulate_mouse_down(from, MouseButton::Left, none);
    cx.simulate_mouse_move(gpui::point((from.x + to.x) / 2., (from.y + to.y) / 2.), MouseButton::Left, none);
    cx.simulate_mouse_move(to, MouseButton::Left, none);
    cx.simulate_mouse_up(to, MouseButton::Left, none);
    next_frames(cx);
}

/// The to-do column's thumb, where it starts along its bar and how long it is, and how far the column scrolls for
/// each pixel the thumb moves.
fn todo_thumb(view: &gpui::Entity<BrainView>, cx: &mut gpui::VisualTestContext) -> (gpui::Pixels, gpui::Pixels, f32) {
    let bar = cx.debug_bounds("scrollbar column todo").expect("the long column has a scrollbar");
    let (shown, max, offset) = view.read_with(cx, |view, _| {
        let scroll = &view.scroll.columns[0].scroll;
        (scroll.bounds().size.height, scroll.max_offset().y, scroll.offset().y)
    });
    let (start, len) = super::scrollbar::thumb(bar.size.height, shown, max, offset).expect("a thumb");
    (start, len, max / (bar.size.height - len))
}

#[gpui::test]
fn a_columns_scrollbar_drags_it_up_and_down(cx: &mut gpui::TestAppContext) {
    let (view, cx, _dir) = small_board(cx);
    let bar = cx.debug_bounds("scrollbar column todo").expect("the long column has a scrollbar");
    let column = cx.debug_bounds("column todo").unwrap();
    assert!(bar.size.width < px(10.) && bar.right() <= column.right(), "a thin bar at its right: {bar:?} {column:?}");
    // Scrolled a little first, so the thumb is grabbed away from the bar's top.
    wheel(&view, cx, gpui::ScrollDelta::Lines(gpui::point(0., -3.)), false);
    let (start, len, per_pixel) = todo_thumb(&view, cx);
    assert!(start > px(0.) && per_pixel > 1., "{start:?} {per_pixel}");
    let (down, _) = scrolled(&view, cx);
    let selected = view.read_with(cx, |view, _| view.board.selected);
    let grabbed = gpui::point(bar.center().x, bar.top() + start + px(3.));
    drag_on(cx, grabbed, grabbed + gpui::point(px(0.), px(30.)));
    let (moved, right) = scrolled(&view, cx);
    assert!((moved - down - 30. * per_pixel).abs() < 0.5, "the thumb followed the pointer: {down} → {moved}");
    assert_eq!(right, 0.);
    view.read_with(cx, |view, _| assert_eq!(view.board.selected, selected, "the press reached no card"));
    // Pressed below the thumb, it jumps there, centered on the pointer.
    let at = px(30.) + len * 1.5 + start;
    let below = gpui::point(bar.center().x, bar.top() + at);
    cx.simulate_mouse_down(below, MouseButton::Left, gpui::Modifiers::none());
    cx.simulate_mouse_up(below, MouseButton::Left, gpui::Modifiers::none());
    next_frames(cx);
    let (start, _, _) = todo_thumb(&view, cx);
    assert!((start + len / 2. - at).abs() < px(0.5), "centered at {at:?}: {start:?}");
    // A right press does nothing, and nor does dragging the card in the column's middle.
    let before = scrolled(&view, cx);
    cx.simulate_mouse_down(
        gpui::point(bar.center().x, bar.bottom() - px(2.)),
        MouseButton::Right,
        gpui::Modifiers::none(),
    );
    cx.simulate_mouse_up(
        gpui::point(bar.center().x, bar.bottom() - px(2.)),
        MouseButton::Right,
        gpui::Modifiers::none(),
    );
    let card = column.center();
    drag_on(cx, card, card + gpui::point(px(0.), px(40.)));
    assert_eq!(scrolled(&view, cx), before);
    // Dragged past its end, the column stops at its last card; and back past its start, at its first.
    let bottom = gpui::point(bar.center().x, bar.bottom() - px(2.));
    drag_on(cx, bottom, bottom + gpui::point(px(0.), px(200.)));
    let max = view.read_with(cx, |view, _| view.scroll.columns[0].scroll.max_offset().y.as_f32());
    assert_eq!(scrolled(&view, cx).0, max, "at its end");
    drag_on(cx, bottom, gpui::point(bar.center().x, bar.top() - px(100.)));
    assert_eq!(scrolled(&view, cx).0, 0., "and back at its start");
}

#[gpui::test]
fn the_boards_scrollbar_drags_it_sideways(cx: &mut gpui::TestAppContext) {
    let (view, cx, _dir) = small_board(cx);
    let bar = cx.debug_bounds("scrollbar board").expect("the board too wide for the window has a scrollbar");
    let board = cx.debug_bounds("column todo").unwrap();
    assert!(bar.size.height < px(10.) && bar.top() >= board.bottom(), "a thin bar under the columns: {bar:?}");
    let left = gpui::point(bar.left() + px(4.), bar.center().y);
    drag_on(cx, left, left + gpui::point(px(50.), px(0.)));
    let (down, right) = scrolled(&view, cx);
    assert!(right > 50., "the board scrolled: {right}");
    assert_eq!(down, 0.);
    // Under the columns, outside what scrolls, the wheel over it still scrolls the board, as far as over the columns:
    // its own way or the other.
    let (_, over_columns) = wheel(&view, cx, gpui::ScrollDelta::Lines(gpui::point(-1., 0.)), false);
    assert!(over_columns > 0.);
    for delta in [gpui::point(-1., 0.), gpui::point(0., -1.)] {
        let (down, right) = wheel_at(&view, cx, bar.center(), gpui::ScrollDelta::Lines(delta), false);
        assert!((right - over_columns).abs() < 0.01 && down == 0., "{delta:?} over the bar: {down}, {right}");
    }
}

#[gpui::test]
fn the_wheel_over_a_columns_scrollbar_scrolls_the_column_once(cx: &mut gpui::TestAppContext) {
    let (view, cx, _dir) = small_board(cx);
    let bar = cx.debug_bounds("scrollbar column todo").expect("the long column has a scrollbar");
    let lines = gpui::ScrollDelta::Lines(gpui::point(0., -1.));
    let (over_cards, _) = wheel(&view, cx, lines, false);
    let (down, right) = wheel_at(&view, cx, bar.center(), lines, false);
    assert!(down > 0. && right == 0., "{down}, {right}");
    assert_eq!(down, over_cards, "as far as over its cards, not twice as far");
}

/// Drags the card at `card` onto `onto`, as the pointer would: past GPUI's drag threshold, then over the target.
fn drag_card(cx: &mut gpui::VisualTestContext, card: gpui::Point<gpui::Pixels>, onto: gpui::Point<gpui::Pixels>) {
    let none = gpui::Modifiers::none();
    cx.simulate_mouse_down(card, MouseButton::Left, none);
    cx.simulate_mouse_move(card + gpui::point(px(10.), px(10.)), MouseButton::Left, none);
    cx.simulate_mouse_move(onto, MouseButton::Left, none);
    cx.simulate_mouse_up(onto, MouseButton::Left, none);
    next_frames(cx);
}

#[gpui::test]
fn a_card_dropped_on_a_columns_scrollbar_lands_in_the_column(cx: &mut gpui::TestAppContext) {
    let (view, cx, _dir) = small_board(cx);
    // The newest is selected; started, it's the one card in Doing.
    let key = view.read_with(cx, |view, _| view.board.selected).expect("a card is selected");
    cx.simulate_keystrokes("s");
    next_frames(cx);
    let card = cx.debug_bounds(format!("card 0/{}", key.id).leak()).expect("it's in Doing").center();
    let bar = cx.debug_bounds("scrollbar column todo").expect("the long column has a scrollbar");
    drag_card(cx, card, bar.center());
    view.read_with(cx, |view, _| assert_eq!(view.board.brain(0).task(key.id).unwrap().status, Status::Todo));
}

#[gpui::test]
fn a_scrollbar_with_nothing_to_scroll_leaves_its_strip_to_the_column(cx: &mut gpui::TestAppContext) {
    let (view, cx, _dir) = window(cx, &["a"]);
    cx.simulate_resize(gpui::size(px(1400.), px(800.)));
    next_frames(cx);
    let card = cx.debug_bounds("card 0/1").expect("card 1 is drawn").center();
    let bar = cx.debug_bounds("scrollbar column done").expect("its strip is kept");
    drag_card(cx, card, bar.center());
    view.read_with(cx, |view, _| assert_eq!(view.board.brain(0).task(1).unwrap().status, Status::Done));
}

#[gpui::test]
fn a_scrollbar_lights_up_as_the_pointer_comes_and_goes(cx: &mut gpui::TestAppContext) {
    let (_view, cx, _dir) = small_board(cx);
    let bar = cx.debug_bounds("scrollbar column todo").expect("the long column has a scrollbar");
    let frames = |cx: &mut gpui::VisualTestContext| cx.update(|_, cx| super::widgets::frames(cx));
    let moved = |cx: &mut gpui::VisualTestContext, to| {
        let before = frames(cx);
        cx.simulate_mouse_move(to, None, gpui::Modifiers::none());
        next_frames(cx);
        frames(cx) > before
    };
    // In the gap between the columns: nothing there lights up of its own.
    let outside = gpui::point(bar.right() + px(5.), bar.center().y);
    moved(cx, outside);
    assert!(moved(cx, bar.center()), "drawn again as the pointer comes onto it");
    assert!(!moved(cx, bar.center() + gpui::point(px(0.), px(3.))), "not while it moves on it");
    assert!(moved(cx, outside), "and again as it leaves");
}

/// Which of the to-do column's bar and the board's drew their thumb in the last frame.
fn bars_shown(cx: &mut gpui::VisualTestContext) -> (bool, bool) {
    let todo: gpui::ElementId = ("scrollbar column", Status::Todo as usize).into();
    cx.update(|_, cx| (super::scrollbar::drawn(&todo, cx), super::scrollbar::drawn(&"scrollbar board".into(), cx)))
}

/// Moves the pointer to `to`, no button held, and draws what that asked for.
fn point_at(cx: &mut gpui::VisualTestContext, to: gpui::Point<gpui::Pixels>) {
    cx.simulate_mouse_move(to, None, gpui::Modifiers::none());
    next_frames(cx);
}

#[gpui::test]
fn a_scrollbar_shows_only_while_the_pointer_is_over_what_it_scrolls(cx: &mut gpui::TestAppContext) {
    let (_view, cx, _dir) = small_board(cx);
    let todo = cx.debug_bounds("column todo").unwrap();
    let doing = cx.debug_bounds("column doing").unwrap();
    let board = cx.debug_bounds("scrollbar board").expect("the board too wide for the window has a scrollbar");
    point_at(cx, gpui::point(todo.left(), px(2.)));
    assert_eq!(bars_shown(cx), (false, false), "the pointer above the board shows neither");
    point_at(cx, todo.center());
    assert_eq!(bars_shown(cx), (true, true), "over the column, its bar and the board's");
    point_at(cx, gpui::point(doing.center().x, doing.top() - px(10.)));
    assert_eq!(bars_shown(cx), (false, true), "over the column's header too: {doing:?}");
    point_at(cx, doing.center());
    assert_eq!(bars_shown(cx), (false, true), "over another column, only the board's");
    point_at(cx, board.center());
    assert_eq!(bars_shown(cx), (false, true), "on the board's own bar, under the columns");
    point_at(cx, gpui::point(todo.left(), px(2.)));
    assert_eq!(bars_shown(cx), (false, false), "and gone with the pointer");
}

#[gpui::test]
fn a_scrollbar_stays_while_keys_scroll_its_column_and_goes_with_the_pointer_out_of_the_window(
    cx: &mut gpui::TestAppContext,
) {
    let (view, cx, _dir) = small_board(cx);
    let todo = cx.debug_bounds("column todo").unwrap();
    point_at(cx, todo.center());
    (0..10).for_each(|_| cx.simulate_keystrokes("down"));
    next_frames(cx);
    assert!(scrolled(&view, cx).0 > 0., "the keys scrolled the column to the card they picked");
    assert_eq!(bars_shown(cx), (true, true), "the pointer still over it, typing hides no bar");
    // Left fast enough for no move to land outside: the window hears only that the pointer went.
    let none = gpui::Modifiers::none();
    cx.simulate_event(gpui::MouseExitEvent { position: todo.center(), pressed_button: None, modifiers: none });
    next_frames(cx);
    assert_eq!(bars_shown(cx), (false, false), "gone with the pointer");
    point_at(cx, todo.center());
    assert_eq!(bars_shown(cx), (true, true), "and back with it");
}

#[gpui::test]
fn a_scrollbar_held_stays_while_the_pointer_leaves(cx: &mut gpui::TestAppContext) {
    let (view, cx, _dir) = small_board(cx);
    let bar = cx.debug_bounds("scrollbar column todo").expect("the long column has a scrollbar");
    let none = gpui::Modifiers::none();
    let thumb = gpui::point(bar.center().x, bar.top() + px(4.));
    point_at(cx, thumb);
    cx.simulate_mouse_down(thumb, MouseButton::Left, none);
    let away = gpui::point(bar.center().x, px(2.));
    cx.simulate_mouse_move(away + gpui::point(px(0.), px(60.)), MouseButton::Left, none);
    cx.simulate_mouse_move(away, MouseButton::Left, none);
    next_frames(cx);
    assert!(bars_shown(cx).0, "dragged above the column, it still shows");
    cx.simulate_mouse_up(away, MouseButton::Left, none);
    next_frames(cx);
    assert!(!bars_shown(cx).0, "let go there, it goes");
    assert_eq!(scrolled(&view, cx), (0., 0.), "dragged back to its start");
}

#[gpui::test]
fn a_drag_cut_short_by_the_bar_going_ends_with_it(cx: &mut gpui::TestAppContext) {
    let (view, cx, _dir) = small_board(cx);
    let bar = cx.debug_bounds("scrollbar column todo").expect("the long column has a scrollbar");
    let none = gpui::Modifiers::none();
    let thumb = gpui::point(bar.center().x, bar.top() + px(4.));
    point_at(cx, thumb);
    cx.simulate_mouse_down(thumb, MouseButton::Left, none);
    // Grown tall enough for every card, the bar goes, and the button comes up with it gone.
    cx.simulate_resize(gpui::size(px(600.), px(4000.)));
    next_frames(cx);
    assert!(view.read_with(cx, |view, _| view.scroll.columns[0].scroll.max_offset().y) < px(1.), "it all fits");
    cx.simulate_mouse_up(thumb, MouseButton::Left, none);
    cx.simulate_resize(gpui::size(px(600.), px(400.)));
    next_frames(cx);
    // Back, a card dragged in the column moves the card, not the bar's old grab.
    let column = cx.debug_bounds("column todo").unwrap().center();
    drag_on(cx, column, column + gpui::point(px(0.), px(80.)));
    assert_eq!(scrolled(&view, cx), (0., 0.));
}

#[test]
fn a_thumb_is_as_long_as_what_shows_and_where_the_scroll_is() {
    use super::scrollbar::thumb;
    assert_eq!(thumb(px(100.), px(100.), px(0.), px(0.)), None, "nothing to scroll");
    assert_eq!(thumb(px(100.), px(100.), px(100.), px(0.)), Some((px(0.), px(50.))), "half shows, at its start");
    assert_eq!(thumb(px(100.), px(100.), px(100.), px(-100.)), Some((px(50.), px(50.))), "at its end");
    assert_eq!(thumb(px(100.), px(100.), px(300.), px(-150.)), Some((px(37.5), px(25.))), "a quarter, halfway");
    // However long the content, the thumb stays long enough to grab.
    let (start, len) = thumb(px(100.), px(100.), px(100_000.), px(-100_000.)).unwrap();
    assert_eq!((start, len), (px(100.) - super::scrollbar::MIN_THUMB, super::scrollbar::MIN_THUMB));
    assert!(thumb(px(100.), px(100.), px(1.), px(0.)).is_some(), "a pixel's more to see is something to scroll to");
    assert!(thumb(px(100.), px(100.), px(0.9), px(0.)).is_none(), "less is layout rounding");
    assert_eq!(thumb(px(20.), px(20.), px(100.), px(-50.)), Some((px(0.), px(20.))), "a bar shorter than a thumb");
    // Whose thumb has nowhere to go: a press on it scrolls nowhere, rather than to the top.
    assert_eq!(super::scrollbar::offset_at(px(20.), px(20.), px(100.), px(5.)), None);
    assert_eq!(super::scrollbar::offset_at(px(100.), px(20.), px(100.), px(40.)), Some(px(-50.)));
}

#[test]
fn a_thumb_lights_up_while_held_or_pointed_at() {
    use super::scrollbar::thumb_color;
    use super::theme::{DIM, TEXT};
    assert_eq!([thumb_color(false, false), thumb_color(true, false), thumb_color(false, true)], [DIM, TEXT, TEXT]);
}

#[test]
fn a_bar_draws_its_track_and_thumb_inside_its_edges() {
    use gpui::{Axis, Bounds, point, size};
    let bounds = Bounds::new(point(px(10.), px(20.)), size(px(8.), px(100.)));
    let (strip, thumb, radius) = super::scrollbar::painted(bounds, Axis::Vertical, px(30.), px(25.));
    assert_eq!(radius, px(3.), "round ends");
    assert_eq!(strip, Bounds::new(point(px(11.), px(20.)), size(px(6.), px(100.))));
    assert_eq!(thumb, Bounds::new(point(px(11.), px(50.)), size(px(6.), px(25.))));
    let bounds = Bounds::new(point(px(10.), px(20.)), size(px(100.), px(8.)));
    let (strip, thumb, _) = super::scrollbar::painted(bounds, Axis::Horizontal, px(30.), px(25.));
    assert_eq!(strip, Bounds::new(point(px(10.), px(21.)), size(px(100.), px(6.))));
    assert_eq!(thumb, Bounds::new(point(px(40.), px(21.)), size(px(25.), px(6.))));
}

#[test]
fn a_panic_says_its_message() {
    let formatted: Box<dyn std::any::Any + Send> = Box::new(format!("{} failed", "X11Client::new"));
    assert_eq!(panic_message(formatted.as_ref()), "X11Client::new failed");
    let literal: Box<dyn std::any::Any + Send> = Box::new("NoCompositor");
    assert_eq!(panic_message(literal.as_ref()), "NoCompositor");
    let other: Box<dyn std::any::Any + Send> = Box::new(101);
    assert_eq!(panic_message(other.as_ref()), "it panicked");
}

#[gpui::test]
fn a_reference_in_a_text_opens_the_entry_it_names_whatever_its_kind(cx: &mut gpui::TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::at(dir.path()).unwrap();
    for (id, title, kind) in
        [(1, "Pool the connections", Kind::Task), (2, "Use Postgres", Kind::Decision), (3, "Pool size", Kind::Note)]
    {
        let mut entry = Task::new(id, title.to_string(), now());
        entry.kind = kind;
        store.save(&mut entry).unwrap();
    }
    let board = Board::open(dir.path(), None).unwrap();
    let (view, cx) = cx.add_window_view(|window, cx| BrainView::new(board, window, cx));
    view.update(cx, |view, cx| {
        view.board.brain_mut(0).add_comment(1, "#2").unwrap();
        view.board.brain_mut(0).add_comment(2, "PR #1, then #3").unwrap();
        view.board.open_entry(crate::state::Key::new(0, 1));
        cx.notify();
    });
    cx.run_until_parked();
    // The reference starts the text: a click a few pixels in is on it.
    let follow = |cx: &mut gpui::VisualTestContext, selector: &'static str, x: f32| {
        let text = cx.debug_bounds(selector).unwrap_or_else(|| panic!("{selector} is drawn"));
        cx.simulate_click(text.origin + gpui::point(gpui::px(x), text.size.height / 2.), gpui::Modifiers::none());
        cx.run_until_parked();
    };
    follow(cx, "links comment 0 1", 5.);
    view.read_with(cx, |view, _| {
        assert_eq!(
            (view.board.tab, view.board.selected, view.board.drawer),
            (Tab::Decisions, Some(crate::state::Key::new(0, 2)), true)
        );
    });
    // `PR #1` is text: a click on it opens nothing.
    follow(cx, "links comment 0 1", 30.);
    view.read_with(cx, |view, _| assert_eq!(view.board.selected, Some(crate::state::Key::new(0, 2))));
    let note = cx.debug_bounds("links comment 0 1").unwrap();
    // Comments are drawn at 13px.
    let width = cx.update(|window, _| {
        let run = window.text_style().to_run("PR #1, then ".len());
        window.text_system().layout_line("PR #1, then ", gpui::px(13.), &[run], None).width
    });
    cx.simulate_click(note.origin + gpui::point(width + gpui::px(3.), note.size.height / 2.), gpui::Modifiers::none());
    cx.run_until_parked();
    view.read_with(cx, |view, _| {
        assert_eq!((view.board.tab, view.board.selected), (Tab::Notes, Some(crate::state::Key::new(0, 3))));
    });
}

#[gpui::test]
fn following_a_reference_leaves_manage_project(cx: &mut gpui::TestAppContext) {
    let (view, cx, _dir) = window(cx, &["a", "b"]);
    view.update(cx, |view, _| {
        view.board.mode = Mode::ManageProject(0);
        view.board.follow(crate::state::Key::new(0, 2));
        assert!(matches!(view.board.mode, Mode::Normal), "{:?}", view.board.mode);
        assert_eq!((view.board.selected, view.board.drawer), (Some(crate::state::Key::new(0, 2)), true));
    });
}

#[gpui::test]
fn a_projects_description_names_entries_only_with_their_repository(cx: &mut gpui::TestAppContext) {
    let (view, cx, _dir) = window(cx, &["a"]);
    view.read_with(cx, |view, _| {
        let here = crate::ui::markdown::written_in(&view.board, 0);
        assert_eq!(here(None, 1), Some(crate::state::Key::new(0, 1)), "a repository's text names its own entries");
        let project = crate::ui::markdown::written_for_project(&view.board, 0);
        assert_eq!(project(None, 1), None, "the project's is no one repository's");
    });
}

#[test]
fn a_repositorys_reference_names_the_entry_of_that_repository_of_its_own_project() {
    let (central, shop, blog) =
        (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    for (code, project, repo) in [(&shop, "shop", "api"), (&shop, "shop", "web"), (&blog, "blog", "api")] {
        let dir = code.path().join(repo).join(".brain");
        Store::at(&dir).unwrap().save(&mut Task::new(1, format!("{project} {repo}"), now())).unwrap();
        kanbrain_core::projects::register(central.path(), project, &dir).unwrap();
        let card = kanbrain_core::projects::RepoCard { project: project.into(), repo: None, notes: String::new() };
        kanbrain_core::projects::write_repo_card(&dir, &card).unwrap();
    }
    let board = Board::open(central.path(), None).unwrap();
    let src = |project: &str, repo: &str| {
        (0..board.sources.len())
            .find(|&s| board.brain(s).project.as_ref().is_some_and(|p| p.name == project && p.repo == repo))
            .unwrap_or_else(|| panic!("{project}/{repo} is loaded"))
    };
    let (shop_api, shop_web) = (src("shop", "api"), src("shop", "web"));
    let named = |from, repo: &str| board.resolve(from, Some(repo), 1).map(|k| k.src);
    assert_eq!(named(shop_web, "api"), Some(shop_api), "its own project's api, not blog's");
    assert_eq!(named(shop_api, "web"), Some(shop_web));
    assert_eq!(named(shop_api, "api"), Some(shop_api), "a repository names itself too");
    assert_eq!(board.resolve(shop_api, Some("web"), 2), None, "an entry that isn't there");
    assert_eq!(named(src("blog", "api"), "web"), None, "blog has no web");
    assert_eq!(named(shop_api, "shop/web"), Some(shop_web), "named with its project, as the app names it");
    assert_eq!(named(src("blog", "api"), "shop/web"), None, "not from another project");
    assert_eq!(named(src("blog", "api"), "shop/api"), None, "nor its own repository of that name");
    let project = crate::ui::markdown::written_for_project(&board, shop_api);
    assert_eq!(project(Some("web"), 1).map(|k| k.src), Some(shop_web), "a project's description names them so");
}
