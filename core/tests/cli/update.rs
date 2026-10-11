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
