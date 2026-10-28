//! Several brains: the central one and projects' repositories, the scope filter, moving entries between them,
//! links across repositories, Manage project, and what's skipped.

use super::*;

#[test]
fn only_the_brains_that_changed_are_read_again() {
    use kanbrain_core::{brain::Brain, projects};
    let (central, code) = projects();
    let mut board = Board::open(central.path(), None).unwrap();
    let dirs: Vec<_> = board.sources.iter().map(|s| s.brain().store.dir().to_path_buf()).collect();
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
    let src = board.sources.iter().position(|s| s.brain().store.dir() == api).unwrap();
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
    kanbrain_core::cli::remove_repo_steps(central.path(), &code.path().join("blog/.brain"), false).unwrap();
    board.reload().unwrap();
    assert_eq!(board.scope, Vec::<Scope>::new(), "not a blank scope");
}

#[test]
fn a_shown_project_renamed_from_the_command_line_stays_shown() {
    let (central, code) = projects();
    let mut board = Board::open(central.path(), None).unwrap();
    board.set_scope(Scope::Project("shop".into()));
    kanbrain_core::cli::rename_project_steps(central.path(), "shop", "store").unwrap();
    board.reload().unwrap();
    assert_eq!(board.scope, [Scope::Project("store".into())], "under its new name");

    // Its last repository joining a project that has others is no rename: all of them.
    board.set_scope(Scope::Project("blog".into()));
    kanbrain_core::cli::move_repo_steps(central.path(), &code.path().join("blog/.brain"), "store").unwrap();
    board.reload().unwrap();
    assert_eq!(board.scope, Vec::<Scope>::new());
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
    assert!(matches!(board.mode, Mode::Scopes(_)), "Enter keeps it open, for several");
    assert_eq!((board.scope.clone(), titles(&board)), (vec![Scope::Repo(1)], vec!["rate limit".to_string()]));
    assert_eq!(board.source_label(1), None, "one brain shown: no labels");
    press(&mut board, KeyCode::Esc);
    assert!(matches!(board.mode, Mode::Normal), "esc closes it");
    board.set_scope(Scope::Central);
    assert_eq!(titles(&board), ["renew passport"]);
}

#[test]
fn the_project_filter_shows_several_at_once() {
    let (central, _code) = projects();
    let mut board = Board::open(central.path(), None).unwrap();
    let titles = |board: &Board| {
        let mut titles: Vec<String> = board.column(Status::Todo).iter().map(|(_, t)| t.title.clone()).collect();
        titles.sort();
        titles
    };
    let pick = |board: &mut Board, name: &str| {
        let Mode::Scopes(picker) = &board.mode else { panic!("the dropdown stays open") };
        let index = board.scope_candidates(picker).iter().position(|(_, n)| n == name).unwrap();
        board.choose_scope(index);
    };
    let (blog, shop) = (Scope::Project("blog".into()), Scope::Project("shop".into()));
    press(&mut board, KeyCode::Char('P'));
    pick(&mut board, "blog");
    assert_eq!(board.scope, std::slice::from_ref(&blog));
    pick(&mut board, "Central");
    assert_eq!(titles(&board), ["post", "renew passport"], "both shown");
    assert_eq!(board.source_label(3), Some("blog"), "cards say where they're from");
    pick(&mut board, "shop/api");
    assert_eq!(titles(&board), ["post", "rate limit", "renew passport"]);
    pick(&mut board, "shop");
    assert_eq!(board.scope, [blog.clone(), Scope::Central, shop.clone()], "a project takes its repositories' place");
    pick(&mut board, "shop/web");
    assert_eq!(board.scope, [blog, Scope::Central, Scope::Repo(2)], "and a repository its project's");
    assert_eq!(board.title(), "kanbrain", "several shown: none named");
    let checked: Vec<String> =
        board.scopes().into_iter().filter(|(s, _)| board.scope_checked(s)).map(|(_, name)| name).collect();
    assert_eq!(checked, ["Central", "shop/web", "blog"], "the rows picked are checked, not All");
    pick(&mut board, "blog");
    assert_eq!(board.scope, [Scope::Central, Scope::Repo(2)], "picking one again takes it out");
    assert_eq!(titles(&board), ["checkout", "renew passport"]);
    assert_eq!(board.project_src(), None, "no one project shown: nothing for its card or Manage project");
    pick(&mut board, "Central");
    board.selected = None;
    assert_eq!(board.title(), "shop/web · kanbrain");
    assert_eq!((board.project_src(), board.repo_here()), (Some(2), Some(2)), "one repository shown, none selected");
    pick(&mut board, "shop");
    assert_eq!(board.title(), "shop · kanbrain");
    pick(&mut board, "All");
    assert_eq!(board.scope, Vec::<Scope>::new(), "All shows every brain again");
    assert!(board.scope_checked(&Scope::All));
    assert_eq!(titles(&board), ["checkout", "post", "rate limit", "renew passport"]);
    press(&mut board, KeyCode::Esc);
    assert!(matches!(board.mode, Mode::Normal));
}

#[test]
fn the_other_projects_shown_stay_when_one_changes() {
    let (central, _code) = projects();
    let mut board = Board::open(central.path(), None).unwrap();
    let (shop, blog) = (Scope::Project("shop".into()), Scope::Project("blog".into()));
    let src = |board: &Board, label: &str| board.sources.iter().position(|s| s.label.as_deref() == Some(label));
    let move_to = |board: &mut Board, label: &str, to: &str| {
        board.ask_move_repo(src(board, label).unwrap());
        to.chars().for_each(|c| press(board, KeyCode::Char(c)));
        press(board, KeyCode::Enter);
    };
    board.set_scope(shop.clone());
    board.toggle_scope(blog.clone());
    move_to(&mut board, "blog", "journal");
    let journal = Scope::Project("journal".into());
    assert_eq!(board.scope, [shop.clone(), journal.clone()], "blog under its new name");

    // Into a project shown, which stays shown: not picked again, which would take it out.
    move_to(&mut board, "shop/web", "journal");
    assert_eq!(board.scope, [shop.clone(), journal.clone()], "shop still has api, and web shows in journal");

    kanbrain_core::cli::remove_project_steps(central.path(), "journal", false).unwrap();
    board.reload().unwrap();
    assert_eq!(board.scope, [shop], "a project gone leaves the others shown");
}

#[test]
fn a_repository_shown_whose_project_is_left_with_it_alone_shows_as_the_project() {
    let (central, code) = projects();
    let mut board = Board::open(central.path(), Some(code.path().join("api/.brain"))).unwrap();
    assert_eq!(board.scope, [Scope::Repo(1)], "opened in shop/api");
    kanbrain_core::cli::remove_repo_steps(central.path(), &code.path().join("web/.brain"), false).unwrap();
    board.reload().unwrap();
    assert_eq!(board.scope, [Scope::Project("shop".into())], "shop/api is no row of the filter any more: shop is");

    // Moved into a project shown, a repository shown is that project's, picked once.
    let mut board = Board::open(central.path(), None).unwrap();
    board.set_scope(Scope::Project("blog".into()));
    let api = board.sources.iter().position(|s| s.label.as_deref() == Some("shop")).unwrap();
    board.toggle_scope(Scope::Repo(api));
    kanbrain_core::cli::move_repo_steps(central.path(), &code.path().join("api/.brain"), "blog").unwrap();
    board.reload().unwrap();
    assert_eq!(board.scope, [Scope::Project("blog".into())]);
}

#[test]
fn new_entries_go_to_the_repository_shown_and_ids_are_per_brain() {
    let (central, code) = projects();
    // Written differently from how it was registered, as `/var` and `/private/var` are on macOS: still the same
    // repository, not another brain.
    let web = code.path().join("web/../web/.brain");
    let mut board = Board::open(central.path(), Some(web)).unwrap();
    assert_eq!(board.scope, [Scope::Repo(2)], "opened in a repository of a project with several");
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
    board.sources[2].brain_mut().link(1, &[], &to_api).unwrap();
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
    let offered = board.menu_items(Key::new(1, 1)).iter().any(|a| a.label == "Move to project");
    assert!(offered, "the menu offers it with other brains to move to");
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
    assert_eq!(board.scope, [Scope::Project("shop".into())], "the filter stays as it was");
    assert!(board.message().unwrap().contains("moved shop/web #2 to blog as #2"), "{:?}", board.message());
}

#[test]
fn each_project_runs_its_own_task() {
    let (central, _code) = projects();
    let mut board = Board::open(central.path(), None).unwrap();
    board.set_status(Key::new(1, 1), Status::Doing);
    board.set_status(Key::new(3, 1), Status::Doing);
    let running: Vec<Key> = board.running().map(|(key, _)| key).collect();
    assert_eq!(running, [Key::new(1, 1), Key::new(3, 1)], "shop/api and blog both run");
    // Within one brain, you run one task at a time.
    let id =
        board.brain_mut(1).add(kanbrain_core::brain::NewEntry { title: "next".into(), ..Default::default() }).unwrap();
    board.set_status(Key::new(1, id), Status::Doing);
    assert_eq!(board.task(Key::new(1, id)).unwrap().status, Status::Todo);
    assert!(board.message().unwrap().contains("#1 is in doing"), "{:?}", board.message());
    board.park_and_quit();
    assert!(!board.any_running(), "Park & quit parks yours, in every brain");
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
    assert_eq!(board.scope, [Scope::Project("journal".into())], "the new project is shown");
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
    board.reload_changed(&[board.sources[0].brain().store.dir().to_path_buf()]).unwrap();
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
    assert_eq!(board.scope, [Scope::Project("store".into())], "the board follows the new name");

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

    // As the registry has it, which the messages name.
    let web = board.brain(src(&board, "blog/web")).store.dir().to_path_buf();
    board.ask_remove(src(&board, "blog/web"), false);
    press(&mut board, KeyCode::Char('y'));
    assert_eq!(labels(&board), ["store", "blog"]);
    let said = format!("unregistered {}: the kanban app no longer shows it", web.display());
    assert_eq!(board.message(), Some(said.as_str()), "kept: what it did first, nothing deleted said");
    assert!(code.path().join("web/.brain").is_dir(), "unlinked, its tasks kept");
    assert!(matches!(board.mode, Mode::ManageProject(at) if at == src(&board, "blog")), "back on blog's other one");
    board.mode = Mode::Normal;

    let blog = src(&board, "blog");
    board.add_repo_to(blog);
    let blog_dir = board.sources[blog].brain().store.dir().to_path_buf();
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

    let api = board.brain(src(&board, "store")).store.dir().to_path_buf();
    board.ask_remove(src(&board, "store"), true);
    press(&mut board, KeyCode::Char('d'));
    assert_eq!(labels(&board), ["blog"]);
    assert!(!code.path().join("api/.brain").exists(), "its tasks deleted too");
    let said = format!("deleted {}: its tasks, notes and decisions are gone", api.display());
    assert_eq!(board.message(), Some(said.as_str()), "and said");
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
    let blog_src = board.sources.iter().position(|s| s.brain().store.dir() == blog).unwrap();
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
    kanbrain_core::cli::rename_project_steps(central.path(), "shop", "store").unwrap();
    // As the watcher hands it over: the cards' folder changed.
    board.reload_changed(&[kanbrain_core::projects::cards_dir(central.path())]).unwrap();
    assert_eq!(board.scope, [Scope::Project("store".into())], "renamed");

    // A one-repository project's repository moved to a project of its own is the same project renamed.
    board.set_scope(Scope::Project("blog".into()));
    kanbrain_core::cli::move_repo_steps(central.path(), &code.path().join("blog/.brain"), "journal").unwrap();
    board.reload().unwrap();
    assert_eq!(board.scope, [Scope::Project("journal".into())]);

    // Split across two projects before the board looks again: neither is it.
    board.set_scope(Scope::Project("store".into()));
    kanbrain_core::cli::move_repo_steps(central.path(), &code.path().join("web/.brain"), "front").unwrap();
    kanbrain_core::cli::move_repo_steps(central.path(), &code.path().join("api/.brain"), "back").unwrap();
    board.reload().unwrap();
    assert_eq!(board.scope, Vec::<Scope>::new(), "store is gone, split in two");
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
    let src =
        board.sources.iter().position(|s| kanbrain_core::projects::same_dir(s.brain().store.dir(), &api)).unwrap();
    board.selected = Some(Key::new(src, rejected));
    board.reject_selected();
    assert_eq!(board.message(), Some(format!("shop/api #{rejected} is already rejected").as_str()));
    board.selected = Some(Key::new(src, accepted));
    board.accept_with_comment();
    assert_eq!(board.message(), Some(format!("shop/api #{accepted} is already accepted").as_str()));
}

#[test]
fn whats_skipped_when_the_window_opens_is_replaced_by_a_message_once_it_was_shown() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("0009-broken.md"), "# no id\n").unwrap();
    let mut board = Board::open(dir.path(), None).unwrap();
    let skipped = board.message().unwrap().to_string();
    assert!(skipped.contains("0009-broken.md"), "{skipped}");
    let later = Instant::now() + watch::INTERVAL;
    board.clock = Clock::new(move || later);
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
fn removing_one_of_a_projects_repositories_keeps_the_project_shown() {
    let (central, _code) = projects();
    let mut board = Board::open(central.path(), None).unwrap();
    board.set_scope(Scope::Project("shop".into()));
    let web = board.sources.iter().position(|s| s.label.as_deref() == Some("shop/web")).unwrap();
    board.ask_remove(web, false);
    press(&mut board, KeyCode::Char('y'));
    assert_eq!(board.scope, [Scope::Project("shop".into())], "shop still has api");
}

#[test]
fn the_add_project_dialog_goes_back_to_its_repository_after_a_reload() {
    let (central, code) = projects();
    let mut board = Board::open(central.path(), None).unwrap();
    let blog = board.sources.iter().position(|s| s.label.as_deref() == Some("blog")).unwrap();
    let blog_dir = board.sources[blog].brain().store.dir().to_path_buf();
    let mobile = code.path().join("mobile");
    std::fs::create_dir(&mobile).unwrap();
    board.name_project(mobile, Some(&blog_dir));
    kanbrain_core::projects::unregister(central.path(), &code.path().join("api/.brain")).unwrap();
    board.reload().unwrap();
    let Mode::AddProject(add) = &board.mode else { panic!("still open: {:?}", board.mode) };
    assert_eq!(add.back, board.sources.iter().position(|s| s.brain().store.dir() == blog_dir));
}
