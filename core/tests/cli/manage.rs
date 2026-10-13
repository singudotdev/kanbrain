use super::*;
use crate::brain::{Brain, NewEntry};
use crate::model::RepoLink;
use std::path::PathBuf;

/// A central brain and a code folder; the folders live as long as the guards.
fn folders() -> (tempfile::TempDir, tempfile::TempDir) {
    (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap())
}

/// Makes `code/<path>` a repository of `project`, as `kb init` does, and returns its `.brain/`.
fn repo(central: &Path, code: &Path, path: &str, project: &str) -> PathBuf {
    let root = code.join(path);
    fs::create_dir_all(&root).unwrap();
    init::init(&root, Some(project), None, central).unwrap();
    root.join(PROJECT_DIR).canonicalize().unwrap()
}

fn brain(central: &Path, dir: &Path) -> Brain {
    Brain::open_in(Store::at(dir).unwrap(), central).unwrap()
}

fn names(central: &Path) -> Vec<String> {
    let repos = projects::load(central).unwrap();
    repos.iter().map(|r| r.label(&repos)).collect()
}

#[test]
fn renaming_a_repository_rewrites_its_siblings_links() {
    let (central, code) = folders();
    let api = repo(central.path(), code.path(), "api", "shop");
    let web = repo(central.path(), code.path(), "web", "shop");
    let target = brain(central.path(), &api).add(NewEntry { title: "Cart total".into(), ..NewEntry::default() });
    let target = target.unwrap();
    let mut web_brain = brain(central.path(), &web);
    let header = web_brain.add(NewEntry { title: "Show the total".into(), ..NewEntry::default() }).unwrap();
    web_brain.link(header, &[], &[RepoLink { repo: "api".into(), id: target }]).unwrap();

    let done = rename_repo(central.path(), &api, "backend").unwrap();
    assert!(done.iter().any(|l| l.contains("rewrote the links to it in 1 entries")), "{done:?}");
    assert_eq!(names(central.path()), ["shop/backend", "shop/web"]);
    let links = brain(central.path(), &web).entry(header).unwrap().repo_links.clone();
    assert_eq!(links, [RepoLink { repo: "backend".into(), id: target }]);
    assert!(rename_repo(central.path(), &api, "web").is_err(), "the project has one");
    assert!(rename_repo(central.path(), &api, "two words").is_err());
    rename_repo(central.path(), &api, "api").unwrap();
    assert_eq!(projects::read_repo_card(&api).unwrap().repo, None, "its folder's name again: none on the card");
}

#[test]
fn renaming_a_project_moves_its_card_and_repositories() {
    let (central, code) = folders();
    let api = repo(central.path(), code.path(), "api", "shop");
    repo(central.path(), code.path(), "web", "shop");
    repo(central.path(), code.path(), "blog", "blog");
    projects::write_description(central.path(), "shop", "An online shop.").unwrap();

    assert!(rename_project(central.path(), "shop", "blog").is_err(), "taken");
    rename_project(central.path(), "shop", "store").unwrap();
    assert_eq!(names(central.path()), ["store/api", "store/web", "blog"]);
    assert_eq!(projects::read_description(central.path(), "store"), "An online shop.");
    assert_eq!(projects::read_description(central.path(), "shop"), "", "the old card is gone");
    assert_eq!(projects::read_repo_card(&api).unwrap().project, "store");
}

#[test]
fn moving_a_repository_drops_its_links_to_the_old_project() {
    let (central, code) = folders();
    let api = repo(central.path(), code.path(), "api", "shop");
    let web = repo(central.path(), code.path(), "web", "shop");
    let target = brain(central.path(), &api).add(NewEntry { title: "Cart total".into(), ..NewEntry::default() });
    let target = target.unwrap();
    let mut web_brain = brain(central.path(), &web);
    let header = web_brain.add(NewEntry { title: "Show the total".into(), ..NewEntry::default() }).unwrap();
    web_brain.link(header, &[], &[RepoLink { repo: "api".into(), id: target }]).unwrap();

    move_repo(central.path(), &web, "storefront").unwrap();
    assert_eq!(names(central.path()), ["shop", "storefront"]);
    assert!(brain(central.path(), &web).entry(header).unwrap().repo_links.is_empty(), "api is another project's");
    assert_eq!(projects::read_repo_card(&web).unwrap().project, "storefront");
}

#[test]
fn removing_keeps_or_deletes_the_tasks() {
    let (central, code) = folders();
    let api = repo(central.path(), code.path(), "api", "shop");
    let web = repo(central.path(), code.path(), "web", "shop");
    let blog = repo(central.path(), code.path(), "blog", "blog");
    brain(central.path(), &web).add(NewEntry { title: "Kept".into(), ..NewEntry::default() }).unwrap();

    let done = remove_repo(central.path(), &web, false).unwrap();
    assert!(web.is_dir() && done.iter().any(|l| l.starts_with("kept ")), "{done:?}");
    let done = remove_repo(central.path(), &blog, true).unwrap();
    assert!(!blog.exists() && done.iter().any(|l| l.starts_with("deleted ")), "{done:?}");
    assert_eq!(names(central.path()), ["shop"]);

    remove_project(central.path(), "shop", true).unwrap();
    assert!(names(central.path()).is_empty() && !api.exists());
    assert_eq!(projects::read_description(central.path(), "shop"), "", "its card went with its last repository");
    assert!(remove_project(central.path(), "shop", false).is_err(), "nothing left");
}
