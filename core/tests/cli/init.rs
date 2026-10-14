use super::*;

#[test]
fn creates_the_brain_and_a_claude_md_when_there_is_none() {
    let (root, central) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let done = init(root.path(), None, None, central.path()).unwrap();
    assert!(root.path().join(".brain").is_dir());
    let claude = fs::read_to_string(root.path().join("CLAUDE.md")).unwrap();
    assert!(claude.starts_with(START) && claude.contains("kb context"), "{claude}");
    // Not necessarily the last line: without `kb` on PATH, a warning about it follows.
    assert!(done.iter().any(|line| line.starts_with("created CLAUDE.md")), "{done:?}");
    assert!(!root.path().join("AGENTS.md").exists(), "only one file is created");
}

#[test]
fn adds_to_existing_agent_files_and_updates_in_place() {
    let (root, central) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    fs::write(root.path().join("CLAUDE.md"), "# Rules\n\nUse tabs.\n").unwrap();
    fs::write(root.path().join("AGENTS.md"), "Be brief.").unwrap();
    init(root.path(), None, None, central.path()).unwrap();
    let claude = fs::read_to_string(root.path().join("CLAUDE.md")).unwrap();
    assert!(claude.starts_with("# Rules\n\nUse tabs.\n\n<!-- kanbrain:start -->\n## Project memory"));
    assert!(fs::read_to_string(root.path().join("AGENTS.md")).unwrap().contains("## Project memory"));

    // An older section, with text after it, is replaced; the rest stays as it was.
    let old = format!("# Rules\n\n{START}\nold advice\n{END}\n\n## More rules\n");
    fs::write(root.path().join("CLAUDE.md"), old).unwrap();
    let done = init(root.path(), None, None, central.path()).unwrap();
    let claude = fs::read_to_string(root.path().join("CLAUDE.md")).unwrap();
    assert!(!claude.contains("old advice") && claude.ends_with(&format!("{END}\n\n## More rules\n")), "{claude}");
    assert_eq!(claude.matches(START).count(), 1);
    assert!(done.iter().any(|l| l.starts_with("updated CLAUDE.md")), "{done:?}");
    assert!(done.iter().any(|l| l == "AGENTS.md already tells agents about kanbrain"), "{done:?}");
}

#[test]
fn an_agent_file_it_cant_read_is_left_as_it_is() {
    let (root, central) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    // Latin-1, not UTF-8: read as nothing, it was written over with the section alone.
    let latin1 = b"My notes caf\xe9\nbuild: make\n";
    fs::write(root.path().join("CLAUDE.md"), latin1).unwrap();
    let done = init(root.path(), None, None, central.path()).unwrap();
    assert!(fs::read(root.path().join("CLAUDE.md")).unwrap() == latin1, "written over");
    assert!(done.iter().any(|l| l.starts_with("left CLAUDE.md as it is")), "{done:?}");
}

#[test]
fn an_unclosed_marker_is_left_alone() {
    let text = format!("{START}\nmy notes\n");
    let once = with_section(&text);
    assert!(once.starts_with(text.trim_end()) && once.ends_with(&format!("{END}\n")));
    assert_eq!(with_section(&once), once, "and running again doesn't eat it");
}

#[test]
fn the_docs_show_the_section_as_written() {
    // On Windows, git may check the docs out with CRLF line endings.
    let docs = include_str!("../../../docs/agents.md").replace("\r\n", "\n");
    assert!(docs.contains(SECTION), "update docs/agents.md with SECTION");
}

#[test]
fn the_section_asks_for_the_description_the_hook_and_context_ask_for() {
    // The section wraps it to the width of its lines; the hook and `context` print it on one.
    let words = |text: &str| text.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(words(SECTION).contains(crate::projects::description_scope!()));
}

#[test]
fn registers_the_project_and_joins_repositories_of_the_same_one() {
    let (code, central) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (api, web) = (code.path().join("api"), code.path().join("web"));
    fs::create_dir_all(&api).unwrap();
    fs::create_dir_all(&web).unwrap();
    let done = init(&api, None, None, central.path()).unwrap();
    assert!(done.iter().any(|l| l.starts_with("registered as project `api`")), "{done:?}");
    assert!(done.iter().any(|l| l.starts_with("created project `api`'s card")), "{done:?}");

    // `init shop` moves it to project shop, and the second service joins it.
    init(&api, Some("shop"), None, central.path()).unwrap();
    let done = init(&web, Some("shop"), None, central.path()).unwrap();
    assert!(
        done.iter()
            .any(|l| l == "registered as `shop/web`, a repository of project `shop` with api: they share its card"),
        "{done:?}"
    );
    let repos = projects::load(central.path()).unwrap();
    let labels: Vec<_> = repos.iter().map(|r| r.label(&repos)).collect();
    assert_eq!(labels, ["shop/api", "shop/web"]);
    let api_brain = api.join(".brain").canonicalize().unwrap();
    assert_eq!(projects::read_repo_card(&api_brain).unwrap().project, "shop");
    // Running it again without a name keeps the project the card names.
    init(&web, None, None, central.path()).unwrap();
    assert_eq!(projects::load(central.path()).unwrap().len(), 2);
}

#[test]
fn a_new_repository_named_into_a_project_is_registered_beside_others_rather_than_moved() {
    let (code, central) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (api, web, docs) = (code.path().join("api"), code.path().join("web"), code.path().join("docs"));
    for root in [&api, &web, &docs] {
        fs::create_dir(root).unwrap();
    }
    init(&api, Some("shop"), None, central.path()).unwrap();
    // Another project's repository doesn't make this one a move, and shop's api, still there, isn't said to be gone.
    init(&docs, Some("wiki"), None, central.path()).unwrap();
    let done = init(&web, Some("shop"), None, central.path()).unwrap();
    assert!(done.iter().any(|l| l.starts_with("registered as `shop/web`")), "{done:?}");
    assert!(!done.iter().any(|l| l.contains("is gone")), "{done:?}");
    // Run again, it stays where it is and still brings its agent files up to date.
    fs::write(web.join("CLAUDE.md"), format!("{START}\nold advice\n{END}\n")).unwrap();
    let done = init(&web, Some("shop"), None, central.path()).unwrap();
    assert!(done.iter().any(|l| l.starts_with("updated CLAUDE.md")), "{done:?}");
    let repos = projects::load(central.path()).unwrap();
    assert_eq!(repos.iter().map(|r| r.label(&repos)).collect::<Vec<_>>(), ["shop/api", "wiki", "shop/web"]);
}

#[test]
fn running_init_again_keeps_the_name_given_to_a_repository_of_the_same_project() {
    let (code, central) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let api = code.path().join("api");
    fs::create_dir(&api).unwrap();
    init(&api, Some("shop"), None, central.path()).unwrap();
    let dir = api.join(".brain").canonicalize().unwrap();
    crate::cli::manage::rename_repo(central.path(), &dir, "backend").unwrap();
    init(&api, None, None, central.path()).unwrap();
    let repos = projects::load(central.path()).unwrap();
    assert_eq!(repos.iter().map(projects::Repo::name).collect::<Vec<_>>(), ["backend"]);
}

#[test]
fn deinit_undoes_init_and_keeps_the_brain() {
    let (root, central) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    fs::write(root.path().join("AGENTS.md"), "# Rules\n\nBe brief.\n").unwrap();
    init(root.path(), Some("shop"), None, central.path()).unwrap();
    projects::write_description(central.path(), "shop", "An online shop.").unwrap();
    let dir = root.path().join(".brain").canonicalize().unwrap();
    let done = deinit(&dir, central.path()).unwrap();
    assert_eq!(projects::load(central.path()).unwrap(), [], "{done:?}");
    assert!(projects::read_description(central.path(), "shop").is_empty(), "the last repository takes the card");
    assert_eq!(fs::read_to_string(root.path().join("AGENTS.md")).unwrap(), "# Rules\n\nBe brief.\n");
    assert!(dir.join(projects::PROJECT_FILE).is_file(), "the brain is kept");
    assert!(deinit(&dir, central.path()).is_err(), "only once");
}

#[test]
fn deinit_removes_a_claude_md_it_created_and_keeps_a_shared_card() {
    let (code, central) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (api, web) = (code.path().join("api"), code.path().join("web"));
    for root in [&api, &web] {
        fs::create_dir(root).unwrap();
        init(root, Some("shop"), None, central.path()).unwrap();
    }
    projects::write_description(central.path(), "shop", "An online shop.").unwrap();
    deinit(&web.join(".brain"), central.path()).unwrap();
    assert!(!web.join("CLAUDE.md").exists(), "it only held the section");
    assert_eq!(projects::load(central.path()).unwrap().len(), 1);
    assert!(!projects::read_description(central.path(), "shop").is_empty(), "api still uses the card");
}

#[test]
fn deinit_takes_out_a_repository_whose_folder_is_gone() {
    let (code, central) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let root = code.path().join("api");
    fs::create_dir(&root).unwrap();
    init(&root, None, None, central.path()).unwrap();
    let dir = root.join(".brain").canonicalize().unwrap();
    fs::remove_dir_all(&root).unwrap();
    let done = deinit(&dir, central.path()).unwrap();
    assert!(done.last().unwrap().contains("is gone"), "{done:?}");
    assert_eq!(projects::load(central.path()).unwrap(), []);
}

#[test]
fn the_section_comes_out_with_its_blank_line() {
    let text = "# Rules\n\nUse tabs.\n";
    assert_eq!(without_section(&with_section(text)), text);
    let between = format!("# Rules\n\n{START}\nold\n{END}\n\n## More\n");
    assert_eq!(without_section(&between), "# Rules\n\n## More\n");
    assert_eq!(without_section("no section\n"), "no section\n");
}

#[test]
fn init_says_when_agents_cant_run_kb() {
    let dir = tempfile::tempdir().unwrap();
    let path = |dirs: &[&std::path::Path]| Some(std::env::join_paths(dirs).unwrap());
    assert!(!on_path(path(&[dir.path()]), "kb"));
    std::fs::write(dir.path().join(format!("kb{}", std::env::consts::EXE_SUFFIX)), "").unwrap();
    assert!(on_path(path(&[std::path::Path::new("/nowhere"), dir.path()]), "kb"));
    assert!(!on_path(None, "kb"));
}

#[test]
fn init_all_updates_every_registered_repository_that_has_the_section() {
    let (central, code) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let [api, web, gone] = ["api", "web", "gone"].map(|name| code.path().join(name));
    for root in [&api, &web, &gone] {
        fs::create_dir_all(root).unwrap();
        init(root, Some("shop"), None, central.path()).unwrap();
    }
    // api has an older section; web had it taken out by hand; gone was deleted.
    let old = format!("# Rules\n\n{START}\nold advice\n{END}\n");
    fs::write(api.join("CLAUDE.md"), &old).unwrap();
    fs::write(web.join("CLAUDE.md"), "# Rules\n").unwrap();
    fs::remove_dir_all(&gone).unwrap();

    let done = init_all(central.path()).unwrap();
    assert_eq!(done, ["shop/api: updated CLAUDE.md: the \"Project memory\" section tells agents how to use the brain"]);
    assert!(fs::read_to_string(api.join("CLAUDE.md")).unwrap().contains("## Project memory: kanbrain"));
    assert_eq!(fs::read_to_string(web.join("CLAUDE.md")).unwrap(), "# Rules\n", "left without the section");
    assert_eq!(init_all(central.path()).unwrap(), ["1 repository already had this version's instructions"]);
}

#[test]
fn one_repository_that_cant_be_brought_up_to_date_doesnt_stop_the_others() {
    let (central, code) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let [api, web] = ["api", "web"].map(|name| code.path().join(name));
    for root in [&api, &web] {
        fs::create_dir_all(root).unwrap();
        init(root, Some("shop"), None, central.path()).unwrap();
    }
    fs::write(api.join(".brain").join(crate::migrate::FORMAT_FILE), "abc\n").unwrap();
    let old = format!("# Rules\n\n{START}\nold advice\n{END}\n");
    fs::write(web.join("CLAUDE.md"), &old).unwrap();
    let done = init_all(central.path()).unwrap();
    assert!(done.iter().any(|l| l.starts_with("shop/api: not brought up to date")), "{done:?}");
    assert!(fs::read_to_string(web.join("CLAUDE.md")).unwrap().contains("## Project memory: kanbrain"), "{done:?}");
}

#[test]
fn folders_with_the_same_name_get_distinct_repository_names() {
    let (central, code) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let folder = |path: &str| {
        let root = code.path().join(path);
        fs::create_dir_all(&root).unwrap();
        root
    };
    let labels = || {
        let repos = projects::load(central.path()).unwrap();
        repos.iter().map(|r| r.label(&repos)).collect::<Vec<_>>()
    };
    init(&folder("front/app"), Some("shop"), None, central.path()).unwrap();
    let back = folder("back/app");
    init(&back, Some("shop"), None, central.path()).unwrap();
    assert_eq!(labels(), ["shop/app", "shop/back-app"], "the folder above makes it unique");
    let card = projects::read_repo_card(&back.join(".brain")).unwrap();
    assert_eq!(card.repo.as_deref(), Some("back-app"), "kept on its card");
    let third = folder("other/back/app");
    let refused = init(&third, Some("shop"), Some("app"), central.path()).unwrap_err().to_string();
    assert!(refused.contains("already has a repository named `app`"), "{refused}");
    init(&third, Some("shop"), Some("worker"), central.path()).unwrap();
    assert_eq!(labels(), ["shop/app", "shop/back-app", "shop/worker"]);
    // A folder above whose name links can't hold is made one they can, as the repository's own folder is.
    init(&folder("My Work/app"), Some("shop"), None, central.path()).unwrap();
    assert_eq!(labels().last().unwrap(), "shop/My-Work-app");
}

#[test]
fn init_all_renames_a_project_a_version_before_0_4_0_named_as_no_project_can_be_now() {
    use std::fmt::Write;
    let (central, code) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let folders = ["api", "notes", "web", "wiki"].map(|name| code.path().join(name));
    for root in &folders {
        fs::create_dir(root).unwrap();
        init(root, None, None, central.path()).unwrap();
    }
    // Written as those versions did: names starting with `-`, `Central` beside the central brain, and one project's
    // repositories apart.
    let names = ["-shop", "--shop", "Central", "-shop"];
    let mut registry = String::from("# Projects\n\n");
    for (root, name) in folders.iter().zip(names) {
        let dir = root.join(".brain").canonicalize().unwrap();
        writeln!(registry, "- {name}: {}", dir.display()).unwrap();
        let card = projects::read_repo_card(&dir).unwrap();
        projects::write_repo_card(&dir, &projects::RepoCard { project: name.into(), ..card }).unwrap();
    }
    fs::write(central.path().join(projects::REGISTRY_FILE), registry).unwrap();
    for root in [&folders[0], &folders[2]] {
        let refused = init(root, None, None, central.path()).unwrap_err().to_string();
        assert!(refused.contains("`kb init --all` renames such projects"), "kb init says how: {refused}");
    }
    let typed = init(&folders[0], Some("-shop"), None, central.path()).unwrap_err().to_string();
    assert!(typed.starts_with("`-shop` can't be a project name"), "a name typed is refused as typed: {typed}");

    let done = init_all(central.path()).unwrap();
    assert!(
        done.contains(&"renamed project `-shop` to `shop-project`; its repositories and card follow".into()),
        "{done:?}"
    );
    let repos = projects::load(central.path()).unwrap();
    let projects: Vec<_> = repos.iter().map(|r| r.project.as_str()).collect();
    assert_eq!(projects, ["shop-project", "shop", "Central-project", "shop-project"], "{done:?}");
    assert_eq!(projects::read_repo_card(&folders[2].join(".brain")).unwrap().project, "Central-project");
    assert!(!init_all(central.path()).unwrap().iter().any(|l| l.starts_with("renamed")), "once");
    init(&folders[0], None, None, central.path()).expect("and kb init takes it again");
}

#[test]
fn init_all_renames_a_lone_central_project_and_one_with_a_slash() {
    use std::fmt::Write;
    for name in ["Central", "shop/web", ":"] {
        let (central, code) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let root = code.path().join("api");
        fs::create_dir(&root).unwrap();
        init(&root, None, None, central.path()).unwrap();
        let dir = root.join(".brain").canonicalize().unwrap();
        let mut registry = String::from("# Projects\n\n");
        writeln!(registry, "- {name}: {}", dir.display()).unwrap();
        fs::write(central.path().join(projects::REGISTRY_FILE), registry).unwrap();
        let card = projects::read_repo_card(&dir).unwrap();
        projects::write_repo_card(&dir, &projects::RepoCard { project: name.into(), ..card }).unwrap();

        let done = init_all(central.path()).expect("the only project's name refused is no panic");
        let renamed = &projects::load(central.path()).unwrap()[0].project;
        projects::check_name(renamed).unwrap_or_else(|e| panic!("{name} became {renamed}: {e}, {done:?}"));
    }
}

#[test]
fn a_repository_whose_folder_is_gone_keeps_its_name_from_a_new_one() {
    let (central, code) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (front, back) = (code.path().join("front/app"), code.path().join("back/app"));
    for root in [&front, &back] {
        fs::create_dir_all(root).unwrap();
    }
    init(&front, Some("shop"), None, central.path()).unwrap();
    // On a disk that isn't mounted: it may come back, still registered and named.
    let away = code.path().join("away");
    fs::rename(code.path().join("front"), &away).unwrap();
    let done = init(&back, Some("shop"), None, central.path()).unwrap();
    let repos = projects::load(central.path()).unwrap();
    assert_eq!(repos.iter().map(|r| r.label(&repos)).collect::<Vec<_>>(), ["shop/app", "shop/back-app"]);
    let gone = done.iter().find(|l| l.starts_with("`shop/app`'s folder is gone")).expect("said");
    // Moved here by hand, it takes its name back once the old place is out.
    assert!(gone.contains("then `kb project repo app` here"), "{gone}");
}

#[test]
fn init_all_renames_nothing_in_a_central_brain_a_newer_kanbrain_wrote() {
    let (central, code) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let api = code.path().join("api");
    fs::create_dir(&api).unwrap();
    init(&api, None, None, central.path()).unwrap();
    let registry = central.path().join(projects::REGISTRY_FILE);
    let old = format!("# Projects\n\n- -shop: {}\n", api.join(".brain").canonicalize().unwrap().display());
    fs::write(&registry, &old).unwrap();
    let newer = crate::migrate::FORMAT + 1;
    fs::write(central.path().join(crate::migrate::FORMAT_FILE), format!("{newer}\n")).unwrap();
    assert!(init_all(central.path()).is_err());
    assert_eq!(fs::read_to_string(&registry).unwrap(), old, "left as that kanbrain wrote it");
}

#[test]
fn init_all_renames_a_repository_whose_card_keeps_a_name_links_cant_hold() {
    let (central, code) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let root = code.path().join("app");
    fs::create_dir(&root).unwrap();
    init(&root, Some("shop"), None, central.path()).unwrap();
    // As versions before 0.4.0 named it after the folder above, `My Work`, or as a name may start with `-`.
    for (kept, fixed) in [("My Work-app", "My-Work-app"), ("-app", "app")] {
        let dir = root.join(".brain");
        let card = projects::read_repo_card(&dir).unwrap();
        projects::write_repo_card(&dir, &projects::RepoCard { repo: Some(kept.into()), ..card }).unwrap();
        let done = init_all(central.path()).unwrap();
        assert!(done.iter().any(|l| l.contains(&format!("to `{fixed}`"))), "{done:?}");
        let repos = projects::load(central.path()).unwrap();
        assert_eq!(repos.iter().map(projects::Repo::name).collect::<Vec<_>>(), [fixed]);
        let again = init_all(central.path()).unwrap();
        assert!(!again.iter().any(|l| l.contains("rename") || l.contains("name")), "once: {again:?}");
    }
}

#[test]
fn the_central_brain_and_paths_with_line_breaks_are_never_registered() {
    let (code, central) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    // The central brain kept in a folder's `.brain/`, as KANBRAIN_DIR=~/x/.brain makes it.
    let own = code.path().join("x");
    fs::create_dir_all(own.join(PROJECT_DIR)).unwrap();
    let error = init(&own, Some("x"), None, &own.join(PROJECT_DIR)).unwrap_err().to_string();
    assert!(error.contains("is the central brain"), "{error}");

    // Windows refuses line breaks in names, so only elsewhere can a folder have one.
    if cfg!(unix) {
        let odd = code.path().join("x\n- evil: /tmp");
        fs::create_dir_all(&odd).unwrap();
        let error = init(&odd, Some("odd"), None, central.path()).unwrap_err().to_string();
        assert!(error.contains("line break"), "{error}");
        assert!(projects::load(central.path()).unwrap().is_empty(), "nothing registered");
        assert!(!odd.join(PROJECT_DIR).exists(), "nor created");
    }
}
