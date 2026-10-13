use super::*;
use crate::state::Board;
use gpui::MouseButton;
use kanbrain_core::model::{Status, Task, now};
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
    assert_eq!(key("shift-tab"), Vec::new());
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
    let (tasks, _) = Store::at(dir.path()).unwrap().load().unwrap();
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
    let (tasks, _) = Store::at(dir.path()).unwrap().load().unwrap();
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
    let (tasks, _) = Store::at(dir.path()).unwrap().load().unwrap();
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
        assert_eq!(view.board.scope, crate::state::Scope::Repo(2));
        assert!(matches!(view.board.mode, crate::state::Mode::Normal), "choosing closes it");
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
    let info = cx.debug_bounds("manage-project-button").expect("the bar has the manage button").center();
    cx.simulate_click(info, gpui::Modifiers::none());
    view.read_with(cx, |view, _| assert!(matches!(view.board.mode, crate::state::Mode::ManageProject(1))));
    cx.simulate_keystrokes("escape");
    view.read_with(cx, |view, _| assert!(matches!(view.board.mode, crate::state::Mode::Normal)));
}

#[gpui::test]
fn calendar_lines_stay_inside_narrow_week_columns(cx: &mut gpui::TestAppContext) {
    let (_view, cx, _dir) = window(cx, &["Stripe test card: 4242 4242 4242 4242"]);
    // Started, so its line reads `00:00 todo → doing`, too wide for a column of a narrow week.
    cx.simulate_keystrokes("s 5");
    cx.simulate_resize(gpui::size(px(760.), px(700.)));
    cx.run_until_parked();
    let today = chrono::Local::now().date_naive();
    let column = cx.debug_bounds(format!("week day {today}").leak()).expect("the week shows today");
    let what = cx.debug_bounds("event what 0/1").expect("the task's move is drawn");
    assert!(what.right() <= column.right(), "the move wraps under the time, inside the column: {what:?} in {column:?}");
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

#[test]
fn the_installer_gives_the_desktop_the_logo() {
    let installer = include_str!("../../install.sh");
    assert!(installer.contains(include_str!("../../docs/images/logo.svg")), "install.sh carries docs/images/logo.svg");
    assert!(installer.contains(&format!("Icon={APP_ID}")) && installer.contains(&format!("apps/{APP_ID}.svg")));
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
