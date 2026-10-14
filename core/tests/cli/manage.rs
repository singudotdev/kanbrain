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
    assert!(done.iter().any(|l| l.contains("rewrote the links to it in 1 entry of")), "{done:?}");
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

    let card = projects::read_repo_card(&web).unwrap();
    projects::write_repo_card(&web, &RepoCard { notes: "The shop's front end.".into(), ..card }).unwrap();
    let done = move_repo(central.path(), &web, "storefront").unwrap();
    assert!(done.iter().any(|l| l.contains("kept its role's notes")), "{done:?}");
    assert_eq!(done[0], "moved web from project `shop` to `storefront`", "said as a move");
    assert!(!done.iter().any(|l| l.ends_with(" already exists")), "not as the `init` it runs: {done:?}");
    assert!(done.contains(&"dropped 1 link between it and project `shop`'s repositories".into()), "{done:?}");
    assert_eq!(names(central.path()), ["shop", "storefront"]);
    assert!(brain(central.path(), &web).entry(header).unwrap().repo_links.is_empty(), "api is another project's");
    assert_eq!(projects::read_repo_card(&web).unwrap().project, "storefront");

    // shop's last repository, named on its own, leaves: its name goes with it, shop's card stays behind no more.
    rename_repo(central.path(), &api, "backend").unwrap();
    projects::write_description(central.path(), "shop", "An online shop.").unwrap();
    let done = move_repo(central.path(), &api, "storefront").unwrap();
    assert!(!done.iter().any(|l| l.starts_with("dropped")), "no link left to drop: {done:?}");
    assert_eq!(names(central.path()), ["storefront/web", "storefront/backend"]);
    assert_eq!(projects::read_description(central.path(), "shop"), "", "the card went with its last repository");
    let carried = projects::read_description(central.path(), "storefront");
    assert_eq!(carried, "An online shop.", "its description, to a project that had none");

    // A name taken where it goes: named after its folder there, and said; still named as it was where it left.
    // Its project's description gives way to the one where it goes, and that's said too.
    let other = repo(central.path(), code.path(), "mobile", "admin");
    rename_repo(central.path(), &other, "web").unwrap();
    projects::write_description(central.path(), "admin", "The back office.").unwrap();
    let done = move_repo(central.path(), &other, "storefront").unwrap();
    assert_eq!(done[0], "moved web from project `admin` to `storefront`");
    assert!(done[1].contains("already has a repository named"), "{done:?}");
    assert!(done.contains(&"project `admin`'s description is dropped: `storefront` keeps its own".into()), "{done:?}");
    assert_eq!(projects::read_description(central.path(), "storefront"), "An online shop.");

    // A name a version before 0.4.0 kept that no repository can have now: said as that, not as taken.
    let old = repo(central.path(), code.path(), "docs", "admin");
    let card = projects::read_repo_card(&old).unwrap();
    projects::write_repo_card(&old, &RepoCard { repo: Some("-docs".into()), ..card }).unwrap();
    let done = move_repo(central.path(), &old, "storefront").unwrap();
    assert!(done[1].contains("`-docs` can't be a repository name"), "{done:?}");
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
    drop(Store::at(&blog).unwrap().lock().unwrap());
    let [lock, backups] = store::cache_of(&blog).unwrap();
    std::fs::create_dir_all(&backups).unwrap();
    assert!(lock.is_file(), "the lock leaves its file");
    let done = remove_repo(central.path(), &blog, true).unwrap();
    assert!(!blog.exists() && done.iter().any(|l| l.starts_with("deleted ")), "{done:?}");
    assert!(!lock.exists() && !backups.exists(), "nor anything about it in the cache");
    assert_eq!(names(central.path()), ["shop"]);

    remove_project(central.path(), "shop", true).unwrap();
    assert!(names(central.path()).is_empty() && !api.exists());
    assert_eq!(projects::read_description(central.path(), "shop"), "", "its card went with its last repository");
    assert!(remove_project(central.path(), "shop", false).is_err(), "nothing left");

    for folder in ["duo/a", "duo/b"] {
        repo(central.path(), code.path(), folder, "duo");
    }
    let done = remove_project(central.path(), "duo", false).unwrap();
    assert!(!done.iter().any(|l| l.contains("keeps its card")), "not said as its card goes: {done:?}");
    assert!(done.iter().any(|l| l.contains("removed project `duo`'s card")), "{done:?}");
}

#[test]
fn init_into_another_project_moves_the_repository_as_project_move_does() {
    let (central, code) = folders();
    let api = repo(central.path(), code.path(), "api", "shop");
    let web = repo(central.path(), code.path(), "web", "shop");
    let target = brain(central.path(), &api).add(NewEntry { title: "Cart total".into(), ..NewEntry::default() });
    let mut web_brain = brain(central.path(), &web);
    let header = web_brain.add(NewEntry { title: "Show the total".into(), ..NewEntry::default() }).unwrap();
    web_brain.link(header, &[], &[RepoLink { repo: "api".into(), id: target.unwrap() }]).unwrap();

    let done = init::init(&code.path().join("web"), Some("blog"), None, central.path()).unwrap();
    assert_eq!(done[0], "moved web from project `shop` to `blog`", "{done:?}");
    assert!(brain(central.path(), &web).entry(header).unwrap().repo_links.is_empty(), "api is another project's");

    // A project's last repository leaving through init takes its card along, as a move does.
    let solo = repo(central.path(), code.path(), "solo", "solo");
    projects::write_description(central.path(), "solo", "Solo.").unwrap();
    init::init(&code.path().join("solo"), Some("blog"), None, central.path()).unwrap();
    assert_eq!(projects::read_description(central.path(), "solo"), "", "no card left behind");
    assert_eq!(projects::read_repo_card(&solo).unwrap().project, "blog");
}

#[test]
fn a_repository_whose_folder_is_gone_keeps_its_name() {
    let (central, code) = folders();
    let api = repo(central.path(), code.path(), "api", "shop");
    repo(central.path(), code.path(), "web", "shop");
    fs::rename(code.path().join("web"), code.path().join("web.bak")).unwrap();
    let refused = rename_repo(central.path(), &api, "web").unwrap_err().to_string();
    assert!(refused.contains("whose folder is gone") && refused.contains("kb --project shop/web deinit"), "{refused}");
    // Another project's gone repository keeps nothing here.
    let blog = repo(central.path(), code.path(), "blog", "blog");
    repo(central.path(), code.path(), "notes", "blog");
    fs::rename(code.path().join("notes"), code.path().join("notes.bak")).unwrap();
    rename_repo(central.path(), &api, "notes").unwrap();
    assert!(blog.is_dir());
}

#[test]
fn registering_again_a_folder_moved_by_hand_says_its_old_place_is_left() {
    let (central, code) = folders();
    repo(central.path(), code.path(), "api", "shop");
    repo(central.path(), code.path(), "web", "shop");
    fs::rename(code.path().join("web"), code.path().join("web.bak")).unwrap();
    let done = init::init(&code.path().join("web.bak"), None, None, central.path()).unwrap();
    assert!(done.iter().any(|l| l.contains("`shop/web`'s folder is gone")), "{done:?}");
}

#[test]
fn a_repository_away_during_a_rename_follows_its_project_when_back() {
    let (central, code) = folders();
    let api = repo(central.path(), code.path(), "api", "shop");
    repo(central.path(), code.path(), "web", "shop");
    fs::rename(code.path().join("web"), code.path().join("away")).unwrap();
    rename_project(central.path(), "shop", "store").unwrap();
    fs::rename(code.path().join("away"), code.path().join("web")).unwrap();
    let web = code.path().join("web").join(PROJECT_DIR);
    assert_eq!(projects::info(&web, Some(central.path())).unwrap().name, "store", "the registry's name");
    init::init(&code.path().join("web"), None, None, central.path()).unwrap();
    assert_eq!(names(central.path()), ["store/api", "store/web"], "not split off into a new shop");
    projects::write_repo_card(&api, &RepoCard { project: "shop".into(), repo: None, notes: String::new() }).unwrap();
    let done = init::init_all(central.path()).unwrap();
    assert!(done.iter().any(|l| l.contains("its card named project `shop`, now `store`: fixed")), "{done:?}");
    assert_eq!(projects::read_repo_card(&api).unwrap().project, "store");
}

#[test]
fn a_folder_whose_name_links_cant_hold_gives_a_repository_name_they_can() {
    let (central, code) = folders();
    let spaced = repo(central.path(), code.path(), "my app", "shop");
    let web = repo(central.path(), code.path(), "web", "shop");
    assert_eq!(names(central.path()), ["shop/my-app", "shop/web"]);
    let target = brain(central.path(), &spaced).add(NewEntry { title: "Cart".into(), ..NewEntry::default() }).unwrap();
    let mut web_brain = brain(central.path(), &web);
    let id = web_brain.add(NewEntry { title: "Show it".into(), ..NewEntry::default() }).unwrap();
    web_brain.link(id, &[], &[RepoLink { repo: "my-app".into(), id: target }]).unwrap();
    assert_eq!(brain(central.path(), &web).entry(id).unwrap().repo_links.len(), 1, "read back whole");

    // Registered by an older version under its folder's name.
    let old = code.path().join("old app").join(PROJECT_DIR);
    fs::create_dir_all(&old).unwrap();
    projects::write_repo_card(&old, &RepoCard { project: "shop".into(), repo: None, notes: String::new() }).unwrap();
    projects::register(central.path(), "shop", &old).unwrap();
    let done = init::init_all(central.path()).unwrap();
    assert!(done.iter().any(|l| l.contains("to `old-app`")), "{done:?}");
    assert!(names(central.path()).contains(&"shop/old-app".to_string()), "{:?}", names(central.path()));
}

#[test]
fn init_moving_a_repository_says_the_move_when_its_new_name_is_taken() {
    let (central, code) = folders();
    repo(central.path(), code.path(), "web", "shop");
    repo(central.path(), code.path(), "front", "blog");
    let done = init::init(&code.path().join("web"), Some("blog"), Some("front"), central.path()).unwrap();
    assert_eq!(done[0], "moved web from project `shop` to `blog`");
    assert!(done.last().unwrap().starts_with("kept the name it has:"), "{done:?}");
}

#[test]
fn renaming_a_project_checks_the_name_under_the_lock_it_renames_under() {
    let (central, code) = folders();
    repo(central.path(), code.path(), "api", "shop");
    let blog = code.path().join("blog").join(PROJECT_DIR);
    fs::create_dir_all(&blog).unwrap();
    // Another program, holding the lock, makes a project `store` meanwhile.
    let (held, wait) = std::sync::mpsc::channel();
    let dir = central.path().to_path_buf();
    let other = std::thread::spawn(move || {
        let lock = crate::store::Store::at(&dir).unwrap().lock().unwrap();
        held.send(()).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(500));
        projects::register(&dir, "store", &blog).unwrap();
        drop(lock);
    });
    wait.recv().unwrap();
    let refused = rename_project(central.path(), "shop", "store");
    other.join().unwrap();
    assert!(refused.unwrap_err().to_string().contains("already a project `store`"), "merged into it");
}
