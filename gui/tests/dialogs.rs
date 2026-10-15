use super::*;
use crate::state::{Key, Target};
use kanbrain_core::model::now;
use kanbrain_core::projects::{self, RepoCard};
use kanbrain_core::store::Store;

#[test]
fn the_delete_question_names_the_entry() {
    let line = question("shop/api #1", &Task::new(1, "Fix login".into(), now()));
    assert_eq!(line.0.iter().map(|s| s.content.as_str()).collect::<String>(), "shop/api #1 Fix login");
}

#[test]
fn removing_says_what_goes_for_one_repository_or_several() {
    let says = |repos, whole| manage::remove_text("shop", "web", repos, whole);
    let (title, [does, keeps, _]) = says(2, true);
    assert_eq!(title, "Remove project shop?");
    assert!(does.contains("its 2 repositories") && does.contains("the links between them"), "{does}");
    assert!(keeps.starts_with("Unlink keeps each .brain/ folder") && keeps.contains("add them again"), "{keeps}");
    let (_, [does, keeps, purges]) = says(1, true);
    assert!(does.contains("its repository out") && !does.contains("between"), "{does}");
    assert!(keeps.contains("its .brain/ folder") && keeps.contains("add it again"), "{keeps}");
    assert!(purges.contains("removes its .brain/ folder"), "{purges}");
    let (title, [does, keeps, _]) = says(2, false);
    assert_eq!(title, "Remove repository web from shop?");
    assert!(does.contains("the project's other repositories have to it"), "{does}");
    assert!(keeps.contains("its .brain/ folder"), "only its own: {keeps}");
    let (_, [does, ..]) = says(1, false);
    assert!(does.contains("It's the project's last"), "{does}");
}

#[test]
fn manage_project_scrolls_to_the_block_of_the_repository_its_opened_from() {
    let repos = [3, 5, 7];
    let items = [3, 5, 7, 9].map(|src| project::block_item(&repos, src));
    assert_eq!(items, [Some(0), Some(5), Some(6), None], "the first goes back to the top, the rest after 4 children");
}

#[test]
fn a_dialog_whose_entry_kb_changed_says_so_in_its_title() {
    assert_eq!(titled("Edit #1".into(), true), "Edit #1 · changed on disk");
    assert_eq!(titled("Edit #1".into(), false), "Edit #1");
    // The editor's unsaved-changes dot stays last, after the mark, rather than between two separators.
    assert_eq!(editor::title("Description".into(), true, true), "Description · changed on disk •");
    assert_eq!(editor::title("Description".into(), true, false), "Description •");
    assert_eq!(editor::title("Description".into(), false, true), "Description · changed on disk");
}

#[test]
fn the_editor_names_what_its_text_belongs_to() {
    let (central, code) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    for repo in ["api", "web"] {
        let dir = code.path().join(repo).join(".brain");
        Store::at(&dir).unwrap().save(&mut Task::new(1, format!("{repo} task"), now())).unwrap();
        projects::register(central.path(), "shop", &dir).unwrap();
        projects::write_repo_card(&dir, &RepoCard { project: "shop".into(), repo: None, notes: String::new() })
            .unwrap();
    }
    let board = Board::open(central.path(), None).unwrap();
    let api = board.repos_of("shop").find(|&src| board.brain(src).project.as_ref().unwrap().repo == "api").unwrap();
    assert_eq!(editor::edited_name(&board, Target::RepoNotes(api)), "repository api of project shop");
    assert_eq!(editor::edited_name(&board, Target::Project(api)), "project shop, shared by its repositories");
    let entry = editor::edited_name(&board, Target::Description(Key::new(api, 1)));
    assert!(entry.ends_with("#1 api task"), "{entry}");
}
