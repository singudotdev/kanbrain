use super::*;

#[test]
fn the_apps_found_beside_this_one_are_updated() {
    let dir = tempfile::tempdir().unwrap();
    let put =
        |name: &str| std::fs::write(dir.path().join(format!("{name}{}", std::env::consts::EXE_SUFFIX)), "").unwrap();
    put("kb");
    assert_eq!(installed_app(dir.path()), "cli");
    put("kanbrain");
    assert_eq!(installed_app(dir.path()), "both");
}

#[test]
fn a_program_cargo_installed_is_left_to_cargo() {
    let home = tempfile::tempdir().unwrap();
    let bin = home.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    assert!(installed_by_cargo(&bin, Some(home.path().to_path_buf())), "$CARGO_HOME/bin");
    assert!(!installed_by_cargo(home.path(), Some(home.path().to_path_buf())), "another folder");
}

#[test]
fn a_release_is_newer_by_its_numbers_not_its_text() {
    assert!(newer("0.3.10", "0.3.9"));
    assert!(newer("1.0.0", "0.9.9"));
    assert!(!newer("0.3.1", "0.3.1"));
    assert!(!newer("0.3.0", "0.3.1"));
    let headers = "HTTP/2 302\r\nLocation: https://github.com/singudotdev/kanbrain/releases/tag/v0.3.2\r\n\r\n";
    assert_eq!(redirected_version(headers).as_deref(), Some("0.3.2"));
    assert_eq!(redirected_version("HTTP/2 404\r\n\r\n"), None);
}

#[test]
fn the_macos_bundle_updates_the_folder_it_links_to() {
    let home = tempfile::tempdir().unwrap();
    let (bin, bundle) = (home.path().join("bin"), home.path().join("kanbrain.app/Contents"));
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::create_dir_all(bundle.join("MacOS")).unwrap();
    std::fs::create_dir_all(bundle.join("Resources")).unwrap();
    std::fs::write(bin.join(format!("kanbrain{}", std::env::consts::EXE_SUFFIX)), "").unwrap();
    let exe = bundle.join("MacOS/kanbrain");
    std::fs::write(bundle.join("Resources/install-dir"), format!("{}\n", bin.display())).unwrap();
    assert_eq!(install_of(&exe).unwrap(), Install::Installer { dir: bin, app: "gui" });
}

#[test]
fn an_update_waits_for_the_agents_tasks_in_doing_only() {
    use crate::brain::Brain;
    use crate::brain::NewEntry;
    use crate::model::{Author, Status};
    let (central, repo) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let brain_dir = repo.path().join(".brain");
    crate::projects::register(central.path(), "shop", &brain_dir).unwrap();
    let mut brain = Brain::open(Store::at(&brain_dir).unwrap()).unwrap();
    let mine = brain.add(NewEntry { title: "my own work".into(), ..NewEntry::default() }).unwrap();
    brain.set_status(mine, Status::Doing).unwrap();
    brain.author = Author::Agent;
    let parked = brain.add(NewEntry { title: "parked by the agent".into(), ..NewEntry::default() }).unwrap();
    brain.set_status(parked, Status::Doing).unwrap();
    brain.set_status(parked, Status::Parked).unwrap();
    assert!(check_no_agent_working(central.path()).is_ok(), "a person's task, and an agent's parked one");

    let working = brain.add(NewEntry { title: "rate-limit the login".into(), ..NewEntry::default() }).unwrap();
    brain.set_status(working, Status::Doing).unwrap();
    let err = check_no_agent_working(central.path()).unwrap_err().to_string();
    assert!(err.contains(&format!("#{working} rate-limit the login")), "{err}");
    assert!(!err.contains("my own work"), "{err}");

    std::fs::remove_dir_all(repo.path()).unwrap();
    assert!(check_no_agent_working(central.path()).is_ok(), "a deleted repository has no agent at work");
    assert!(!repo.path().exists(), "nor is it made again");
}
