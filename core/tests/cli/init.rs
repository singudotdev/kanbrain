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
    assert_eq!(init_all(central.path()).unwrap(), ["1 more already had this version's instructions"]);
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
}
