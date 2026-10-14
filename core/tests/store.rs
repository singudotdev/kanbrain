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
fn saving_never_writes_over_a_file_that_didnt_parse() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::at(dir.path()).unwrap();
    let mut task = Task::new(1, "Fix".into(), now());
    store.save(&mut task).unwrap();
    // A sync conflict: the entry's file broken, a readable copy beside it.
    let copy = dir.path().join("0001-fix (conflicted copy).md");
    fs::rename(dir.path().join("0001-fix.md"), &copy).unwrap();
    fs::write(dir.path().join("0001-fix.md"), "not an entry").unwrap();
    let (mut tasks, warnings) = store.load().unwrap();
    assert_eq!((tasks.len(), warnings.len()), (1, 1), "{warnings:?}");
    tasks[0].description = "mine".into();
    let refused = store.save(&mut tasks[0]).unwrap_err().to_string();
    assert!(refused.contains("couldn't be read as #1"), "{refused}");
    assert_eq!(fs::read_to_string(dir.path().join("0001-fix.md")).unwrap(), "not an entry", "kept");
    assert!(copy.exists(), "and the copy too");
}

#[test]
fn a_file_renamed_only_in_case_keeps_its_name_and_its_content() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::at(dir.path()).unwrap();
    store.save(&mut Task::new(5, "Fix login".into(), now())).unwrap();
    // As a sync tool or a hand rename leaves it; on macOS and Windows it's the same file as 0005-fix-login.md.
    let renamed = dir.path().join("0005-Fix-Login.md");
    fs::rename(dir.path().join("0005-fix-login.md"), &renamed).unwrap();
    let (mut tasks, _) = store.load().unwrap();
    tasks[0].description = "kept".into();
    store.save(&mut tasks[0]).unwrap();

    let names: Vec<String> =
        fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(names, ["0005-Fix-Login.md"], "saved where it is, not removed as an old name");
    assert!(fs::read_to_string(renamed).unwrap().contains("kept"));
}

#[test]
fn files_an_editor_saved_with_a_byte_order_mark_read_as_without() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::at(dir.path()).unwrap();
    let mut task = Task::new(2, "Fix login".into(), now());
    store.save(&mut task).unwrap();
    let path = dir.path().join("0002-fix-login.md");
    fs::write(&path, format!("\u{feff}{}", fs::read_to_string(&path).unwrap())).unwrap();
    fs::write(dir.path().join(LAST_ID_FILE), "\u{feff}7\n").unwrap();
    fs::write(dir.path().join(TAGS_FILE), "\u{feff}- prod: #112233\n").unwrap();

    let (tasks, warnings) = store.load().unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(tasks[0].title, "Fix login", "the title, not untitled");
    assert_eq!(store.last_id(), 7);
    assert!(store.load_tag_colors().unwrap().chip("prod").is_some(), "its first color too");
    let repo = dir.path().join("repo");
    fs::create_dir(&repo).unwrap();
    fs::write(repo.join(crate::projects::PROJECT_FILE), "\u{feff}# shop\n").unwrap();
    assert_eq!(crate::projects::read_repo_card(&repo).unwrap().project, "shop");
    fs::write(dir.path().join(crate::migrate::FORMAT_FILE), "\u{feff}1\n").unwrap();
    assert_eq!(crate::migrate::format_of(dir.path()).unwrap(), 1);
}

#[test]
fn a_brain_made_now_says_its_format_and_one_already_there_is_left_as_it_is() {
    let dir = tempfile::tempdir().unwrap();
    let new = dir.path().join("new");
    Store::at(&new).unwrap();
    let format = fs::read_to_string(new.join(crate::migrate::FORMAT_FILE)).unwrap();
    assert_eq!(format, format!("{}\n", crate::migrate::FORMAT));
    Store::at(dir.path()).unwrap();
    assert!(!dir.path().join(crate::migrate::FORMAT_FILE).exists(), "an older brain reads as format 1 until migrated");
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

#[test]
fn a_temporary_brains_cache_goes_once_it_does() {
    let (gone, kept) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let lock = |dir: &Path| drop(Store::at(dir).unwrap().lock().unwrap());
    lock(gone.path());
    let [gone_lock, _] = cache_of(gone.path()).unwrap();
    let brain = fs::read_to_string(&gone_lock).unwrap();
    assert_eq!(brain, format!("{}\n", gone.path().canonicalize().unwrap().display()), "it names its brain");
    drop(gone);
    lock(kept.path());
    assert!(!gone_lock.exists(), "tidied as another brain there is locked");
    assert!(cache_of(kept.path()).unwrap()[0].exists());
}

#[test]
fn a_thread_holding_a_brains_lock_takes_it_again_without_waiting_for_itself() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::at(dir.path()).unwrap();
    let outer = store.lock().unwrap();
    let inner = store.lock().unwrap();
    drop(inner);
    let path = outer.path.clone();
    assert!(HELD.with(|held| held.borrow().contains(&path)), "the outer one still holds it");
    drop(outer);
    assert!(!HELD.with(|held| held.borrow().contains(&path)));
}

#[test]
fn a_relative_brain_is_named_after_the_current_folder() {
    let here = std::env::current_dir().unwrap();
    let name = here.file_name().unwrap().to_string_lossy().into_owned();
    assert_eq!(Store { dir: PathBuf::from(PROJECT_DIR), journal: RefCell::new(None) }.project(), Some(name));
}

#[test]
fn what_a_crash_left_of_a_write_goes_once_the_lock_is_taken() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("reports")).unwrap();
    let left = |name: &str, age: Duration| {
        let path = dir.path().join(name);
        let file = fs::File::create(&path).unwrap();
        file.set_modified(std::time::SystemTime::now() - age).unwrap();
        path
    };
    let crashed = left(".0001-a.md.4242-0.tmp", Duration::from_secs(2 * 60 * 60));
    let in_report = left("reports/.week.md.4242-1.tmp", Duration::from_secs(2 * 60 * 60));
    let writing = left(".0002-b.md.4243-0.tmp", Duration::ZERO);
    // As versions before 0.4.0 named them.
    let old_id = left(".last-id.tmp", Duration::from_secs(2 * 60 * 60));
    // Another program's, in a central brain that is a notes vault.
    let editors =
        [".notes.swp.tmp", ".sync.tmp", "reports/.backup.4242-1.tmp", ".notes.md.backup-1.tmp", ".notes.md.4242-x.tmp"]
            .map(|name| left(name, Duration::from_secs(2 * 60 * 60)));
    drop(Store::at(dir.path()).unwrap().lock().unwrap());
    assert!(!crashed.exists() && !in_report.exists() && !old_id.exists());
    assert!(writing.exists(), "maybe still being written, by a write made without the lock");
    assert!(editors.iter().all(|path| path.exists()), "not kanbrain's");
}

#[test]
fn a_write_that_fails_leaves_the_file_as_it_was_and_no_temporary() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::at(dir.path()).unwrap();
    let mut task = Task::new(1, "Fix login".into(), now());
    store.save(&mut task).unwrap();
    let path = dir.path().join("0001-fix-login.md");
    let before = fs::read_to_string(&path).unwrap();
    // Its place taken by a folder, as a sync tool's conflict can leave it.
    let blocked = dir.path().join("0001-blocked.md");
    fs::create_dir(&blocked).unwrap();
    assert!(write_file(&blocked, "text").is_err());
    let leftovers = |dir: &Path| {
        fs::read_dir(dir)
            .unwrap()
            .filter(|e| e.as_ref().unwrap().file_name().to_string_lossy().ends_with(".tmp"))
            .count()
    };
    assert_eq!(leftovers(dir.path()), 0, "no temporary left behind");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // A folder that can't be written to, as a read-only mount or a permissions slip leaves it.
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o555)).unwrap();
        task.description = "lost?".into();
        let failed = store.save(&mut task);
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o755)).unwrap();
        // Root writes anyway; the rest of the checks still hold.
        if failed.is_err() {
            assert_eq!(fs::read_to_string(&path).unwrap(), before, "the file as it was");
        }
        assert_eq!(leftovers(dir.path()), 0);
    }
}
