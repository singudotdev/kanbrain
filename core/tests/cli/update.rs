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
}
