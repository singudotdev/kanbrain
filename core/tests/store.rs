use super::*;
use crate::model::now;

#[test]
fn save_renames_on_title_change_and_loads_back() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::at(dir.path()).unwrap();
    let mut task = Task::new(3, "First title".into(), now());
    store.save(&mut task).unwrap();
    task.title = "Second title".into();
    store.save(&mut task).unwrap();

    let names: Vec<String> =
        fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(names, ["0003-second-title.md"]);

    let (tasks, warnings) = store.load().unwrap();
    assert_eq!(warnings, Vec::<String>::new());
    assert_eq!(tasks[0].title, "Second title");
}

#[test]
fn unreadable_files_become_warnings() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::at(dir.path()).unwrap();
    fs::write(dir.path().join("README.md"), "# not a task").unwrap();
    fs::write(dir.path().join(TAGS_FILE), "- prod: red").unwrap();
    let (tasks, warnings) = store.load().unwrap();
    assert!(tasks.is_empty());
    assert_eq!(warnings.len(), 1, "tags.md is not a task: {warnings:?}");
}

#[test]
fn projects_are_found_from_any_subfolder() {
    let root = tempfile::tempdir().unwrap();
    let nested = root.path().join("src/deep");
    fs::create_dir_all(&nested).unwrap();
    assert_eq!(find_project(&nested), None);
    let brain = root.path().join(PROJECT_DIR);
    fs::create_dir(&brain).unwrap();
    assert_eq!(find_project(&nested), Some(brain.clone()));
    let name = root.path().file_name().unwrap().to_string_lossy().into_owned();
    assert_eq!(Store::at(&brain).unwrap().project(), Some(name));
    assert_eq!(Store::at(root.path()).unwrap().project(), None);
}

#[test]
fn files_are_written_whole_with_their_folder_and_no_temporary_left() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("projects").join("shop.md");
    write_file(&path, "# shop\n").unwrap();
    write_file(&path, "# shop\n\nAn online shop.\n").unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "# shop\n\nAn online shop.\n");
    let names: Vec<_> = fs::read_dir(path.parent().unwrap()).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(names, ["shop.md"]);
}

#[test]
fn the_hash_is_fnv1a_on_every_build() {
    // Lock files and tag colors depend on it: a change would split a brain's lock between two kb versions.
    assert_eq!(fnv1a(b""), 0xcbf2_9ce4_8422_2325);
    assert_eq!(fnv1a(b"a"), 0xaf63_dc4c_8601_ec8c);
}
