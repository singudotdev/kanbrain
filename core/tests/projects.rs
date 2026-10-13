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
    assert!(
        ambiguous.contains("several repositories") && ambiguous.contains("shop/api, shop/web, blog"),
        "{ambiguous}"
    );
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
    write_description(central.path(), "shop", OLD_DESCRIPTION_TEMPLATE).unwrap();
    assert_eq!(read_description(central.path(), "shop"), "", "so does an older version's");
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
