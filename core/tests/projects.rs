use super::*;

/// A `.brain/` folder inside a repository folder named `repo`, under `parent`.
fn repo_dir(parent: &Path, repo: &str) -> PathBuf {
    let dir = parent.join(repo).join(".brain");
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn a_project_can_have_several_repositories() {
    let (central, code) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    assert!(load(central.path()).unwrap().is_empty(), "no registry yet");
    let (api, web, blog) = (repo_dir(code.path(), "api"), repo_dir(code.path(), "web"), repo_dir(code.path(), "blog"));
    register(central.path(), "shop", &api).unwrap();
    register(central.path(), "shop", &web).unwrap();
    register(central.path(), "blog", &blog).unwrap();
    register(central.path(), "shop", &api).unwrap();
    let repos = load(central.path()).unwrap();
    let labels: Vec<_> = repos.iter().map(|r| r.label(&repos)).collect();
    assert_eq!(labels, ["shop/api", "shop/web", "blog"]);
    assert_eq!(find(central.path(), "shop/web").unwrap().dir, web);
    assert_eq!(find(central.path(), "blog").unwrap().dir, blog);
    let ambiguous = format!("{:#}", find(central.path(), "shop").unwrap_err());
    assert_eq!(ambiguous, "project `shop` has several repositories: name one, shop/api, shop/web");
    assert_eq!(find(central.path(), "Shop/WEB").unwrap().dir, web, "in any case");
    register(central.path(), "Blog", &repo_dir(code.path(), "other")).unwrap();
    assert_eq!(find(central.path(), "Blog").unwrap().project, "Blog", "the exact one first");
    assert!(find(central.path(), "BLOG").is_err(), "two alike: neither");
}

#[test]
fn registering_moves_a_repository_and_refuses_twins_and_bad_names() {
    let (central, code, other) =
        (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let api = repo_dir(code.path(), "api");
    register(central.path(), "shop", &api).unwrap();
    register(central.path(), "store", &api).unwrap();
    assert_eq!(load(central.path()).unwrap(), [Repo { project: "store".into(), dir: api.clone() }], "moved");
    let twin = repo_dir(other.path(), "api");
    assert!(register(central.path(), "store", &twin).is_err(), "two live `api` repositories in one project");
    fs::remove_dir_all(code.path().join("api")).unwrap();
    register(central.path(), "store", &twin).unwrap();
    assert_eq!(
        load(central.path()).unwrap(),
        [Repo { project: "store".into(), dir: twin.clone() }],
        "the gone one gives way"
    );
    for bad in [CENTRAL, "a: b", "a/b", " a"] {
        assert!(register(central.path(), bad, &twin).is_err(), "{bad}");
    }
}

#[test]
fn repositories_share_the_project_card_and_keep_their_own_notes() {
    let (central, code) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (api, web) = (repo_dir(code.path(), "api"), repo_dir(code.path(), "web"));
    for dir in [&api, &web] {
        register(central.path(), "shop", dir).unwrap();
        write_repo_card(dir, &RepoCard { project: "shop".into(), repo: None, notes: String::new() }).unwrap();
    }
    assert!(ensure_description(central.path(), "shop").unwrap());
    assert!(!ensure_description(central.path(), "shop").unwrap(), "only once");
    assert_eq!(read_description(central.path(), "shop"), "", "the placeholder reads as no description");
    write_description(central.path(), "shop", "An online shop.").unwrap();
    write_repo_card(&web, &RepoCard { project: "shop".into(), repo: None, notes: "The React front end.".into() })
        .unwrap();
    let info = info(&web, Some(central.path())).unwrap();
    assert_eq!(
        (info.description.as_str(), info.repo.as_str(), info.notes.as_str()),
        ("An online shop.", "web", "The React front end.")
    );
    assert_eq!(info.siblings, [("api".to_string(), api.clone())]);
    assert_eq!(super::info(&api, None).unwrap().description, "", "without the central brain, only its own card");
    assert!(super::info(central.path(), Some(central.path())).is_none(), "the central brain isn't a project");
}

#[cfg(unix)]
#[test]
fn a_repository_registered_by_another_path_to_its_folder_moves_rather_than_doubles() {
    let (central, code) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let api = repo_dir(code.path(), "api");
    // Through a symlink, as macOS's `/var` is to `/private/var`.
    let link = code.path().join("link");
    std::os::unix::fs::symlink(code.path(), &link).unwrap();
    register(central.path(), "shop", &link.join("api/.brain")).unwrap();
    register(central.path(), "shop", &api).unwrap();
    assert_eq!(load(central.path()).unwrap().len(), 1, "already registered, by another path");
    register(central.path(), "store", &api).unwrap();
    assert_eq!(load(central.path()).unwrap(), [Repo { project: "store".into(), dir: api.clone() }], "moved, once");
}

#[test]
fn projects_whose_names_slug_alike_each_keep_their_own_card() {
    let central = tempfile::tempdir().unwrap();
    let names = ["my-app", "my_app", "shop", "Shop", "日記", "Проект"];
    for name in names {
        write_description(central.path(), name, &format!("About {name}.")).unwrap();
    }
    for name in names {
        assert_eq!(read_description(central.path(), name), format!("About {name}."));
    }
    assert!(remove_description(central.path(), "my_app").unwrap());
    assert_eq!(read_description(central.path(), "my-app"), "About my-app.", "only its own card goes");
    rename(central.path(), "日記", "Дневник").unwrap();
    assert_eq!(read_description(central.path(), "Дневник"), "About 日記.");
    assert_eq!(read_description(central.path(), "Проект"), "About Проект.");
}

#[test]
fn cards_written_before_each_had_its_own_place_are_read_and_moved() {
    let central = tempfile::tempdir().unwrap();
    let cards = cards_dir(central.path());
    fs::create_dir_all(&cards).unwrap();
    // `my_app`'s card under its slug alone, as before 0.4.0.
    fs::write(cards.join("my-app.md"), "# my_app\n\nThe app.\n").unwrap();
    assert_eq!(read_description(central.path(), "my_app"), "The app.");
    assert_eq!(read_description(central.path(), "my-app"), "", "titled with another project: not my-app's");
    write_description(central.path(), "my-app", "Another app.").unwrap();
    assert_eq!(read_description(central.path(), "my_app"), "The app.", "moved to its own place, not overwritten");
    assert_eq!(read_description(central.path(), "my-app"), "Another app.");
    write_description(central.path(), "my_app", "The app, again.").unwrap();
    let mut files: Vec<_> = fs::read_dir(&cards).unwrap().map(|e| e.unwrap().file_name()).collect();
    files.sort();
    assert_eq!(files.len(), 2, "one card each: {files:?}");
    assert!(!ensure_description(central.path(), "my_app").unwrap());
}

#[test]
fn names_are_one_line_without_separators_or_a_leading_dash() {
    for bad in ["", " shop", "a:b", "a/b", "two\nlines", "two\rlines", "--help", "-x", "central", "Central"] {
        assert!(check_name(bad).is_err(), "{bad:?} is a project name");
    }
    for bad in ["", "my app", "a,b", "a#b", "a:b", "a/b", "--help"] {
        assert!(check_repo_name(bad).is_err(), "{bad:?} is a repository name");
    }
    assert!(check_name("shop v2").is_ok() && check_name("日記").is_ok() && check_repo_name("back-app").is_ok());
}

#[test]
fn a_project_renamed_to_its_own_slug_keeps_one_card() {
    let central = tempfile::tempdir().unwrap();
    let cards = cards_dir(central.path());
    fs::create_dir_all(&cards).unwrap();
    // `Shop`'s card from before 0.4.0, under its slug alone.
    fs::write(cards.join("shop.md"), "# Shop\n\nThe shop.\n").unwrap();
    rename(central.path(), "Shop", "shop").unwrap();
    assert_eq!(read_description(central.path(), "shop"), "The shop.");
    let files: Vec<_> = fs::read_dir(&cards).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(files, ["shop.md"], "no card left behind");
}

#[test]
fn a_card_moved_aside_never_lands_on_that_projects_own() {
    let central = tempfile::tempdir().unwrap();
    write_description(central.path(), "Shop", "Shop, as it is now.").unwrap();
    // A copy from before 0.4.0 left where `shop`'s card goes.
    fs::write(cards_dir(central.path()).join("shop.md"), "# Shop\n\nShop, long ago.\n").unwrap();
    write_description(central.path(), "shop", "The other shop.").unwrap();
    assert_eq!(read_description(central.path(), "Shop"), "Shop, as it is now.", "not the old copy over it");
    assert_eq!(read_description(central.path(), "shop"), "The other shop.");
}

#[test]
fn a_projects_card_is_written_under_the_central_brains_lock() {
    use std::time::{Duration, Instant};
    type Write = fn(&Path);
    let central = tempfile::tempdir().unwrap();
    let writes: [(&str, Write); 3] = [
        ("write", |central| write_description(central, "shop", "An online shop.").unwrap()),
        ("ensure", |central| assert!(ensure_description(central, "blog").unwrap())),
        ("remove", |central| assert!(remove_description(central, "shop").unwrap())),
    ];
    for (what, write) in writes {
        // Held by another thread: this one's would let it take the lock again.
        let (held, wait) = std::sync::mpsc::channel();
        let dir = central.path().to_path_buf();
        let holder = std::thread::spawn(move || {
            let lock = Store::at(&dir).unwrap().lock().unwrap();
            held.send(()).unwrap();
            std::thread::sleep(Duration::from_millis(800));
            drop(lock);
        });
        wait.recv().unwrap();
        let started = Instant::now();
        write(central.path());
        assert!(started.elapsed() >= Duration::from_millis(500), "{what} didn't wait for the lock");
        holder.join().unwrap();
    }
}

#[test]
fn these_tests_fall_back_to_a_temporary_folder_not_the_users_data_folder() {
    // A mutant of how `KANBRAIN_DIR` is read once sent these tests' writes to the real central brain.
    let fallback = super::data_dir().unwrap();
    assert!(fallback.starts_with(std::env::temp_dir()), "{}", fallback.display());
    assert_ne!(Some(fallback), dirs::data_dir());
}

#[test]
fn kanbrain_dir_set_but_empty_is_unset_and_a_relative_one_is_refused() {
    let data = || Some(PathBuf::from(if cfg!(windows) { r"C:\data" } else { "/data" }));
    let fallback = data().unwrap().join("kanbrain");
    assert_eq!(central_dir_from(None, data).unwrap(), fallback);
    assert_eq!(central_dir_from(Some("".into()), data).unwrap(), fallback, "empty is unset, not the current folder");
    let absolute = if cfg!(windows) { r"D:\notes\todo" } else { "/home/me/todo" };
    assert_eq!(central_dir_from(Some(absolute.into()), || None).unwrap(), Path::new(absolute));
    let mut relative = vec!["todo", "./todo", "../todo"];
    if cfg!(windows) {
        assert_eq!(
            central_dir_from(Some(r"\\server\share\todo".into()), || None).unwrap(),
            Path::new(r"\\server\share\todo")
        );
        let verbatim = r"\\?\C:\notes\todo";
        assert_eq!(central_dir_from(Some(verbatim.into()), || None).unwrap(), Path::new(verbatim));
        // Each depends on the current folder: its drive, or the one on drive D.
        relative.extend([r"\todo", "D:todo"]);
    }
    for value in relative {
        let err = format!("{:#}", central_dir_from(Some(value.into()), data).unwrap_err());
        assert_eq!(
            err,
            format!("KANBRAIN_DIR must be an absolute path, not `{value}`: set it to the folder's full path")
        );
    }
    // Left unexpanded by a shell (quoted) or by what sets it without one (a systemd environment.d file).
    let err = format!("{:#}", central_dir_from(Some("~/todo".into()), data).unwrap_err());
    assert_eq!(
        err,
        "KANBRAIN_DIR must be an absolute path, not `~/todo`: `~` isn't expanded there (as when quoted, or in a \
         systemd environment.d file); write it with the home folder's full path"
    );
    let err = format!("{:#}", central_dir_from(None, || None).unwrap_err());
    assert_eq!(err, "could not determine the data directory; set KANBRAIN_DIR");
}
