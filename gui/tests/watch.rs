use super::*;

/// How many folders changed since the last look.
fn changed(watcher: &mut Watcher) -> usize {
    watcher.changed().len()
}

#[test]
fn changes_to_markdown_files_are_noticed_once() {
    let dir = tempfile::tempdir().unwrap();
    let mut watcher = Watcher::new(vec![dir.path().to_path_buf()]);
    assert_eq!(changed(&mut watcher), 0);
    fs::write(dir.path().join("0001-a.md"), "# a").unwrap();
    assert_eq!(changed(&mut watcher), 1, "a new file");
    assert_eq!(changed(&mut watcher), 0, "only once");
    fs::write(dir.path().join("0001-a.md"), "# a, longer").unwrap();
    assert_eq!(changed(&mut watcher), 1, "an edit");
    fs::rename(dir.path().join("0001-a.md"), dir.path().join("0001-b.md")).unwrap();
    assert_eq!(changed(&mut watcher), 1, "a rename");
    fs::remove_file(dir.path().join("0001-b.md")).unwrap();
    assert_eq!(changed(&mut watcher), 1, "a delete");
}

#[test]
fn only_the_folders_that_changed_are_named() {
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let mut watcher = Watcher::new(vec![a.path().to_path_buf(), b.path().to_path_buf()]);
    fs::write(b.path().join("0001-a.md"), "# a").unwrap();
    let dirs: Vec<_> = watcher.changed().into_iter().map(|c| c.dir).collect();
    assert_eq!(dirs, [b.path()]);
}

#[test]
fn the_files_changed_added_or_deleted_are_named() {
    let dir = tempfile::tempdir().unwrap();
    let mut watcher = Watcher::new(vec![dir.path().to_path_buf()]);
    let files = |watcher: &mut Watcher| -> Vec<String> {
        let changes = watcher.changed();
        changes.iter().flat_map(|c| &c.files).map(|f| f.file_name().unwrap().to_string_lossy().into_owned()).collect()
    };
    for name in ["0001-a.md", "0002-b.md", "0003-c.md"] {
        fs::write(dir.path().join(name), "# x").unwrap();
    }
    assert_eq!(files(&mut watcher), ["0001-a.md", "0002-b.md", "0003-c.md"]);
    fs::write(dir.path().join("0002-b.md"), "# b, longer").unwrap();
    assert_eq!(files(&mut watcher), ["0002-b.md"], "an edit");
    fs::rename(dir.path().join("0003-c.md"), dir.path().join("0000-c.md")).unwrap();
    assert_eq!(files(&mut watcher), ["0000-c.md", "0003-c.md"], "a rename: the new name and the old");
    fs::remove_file(dir.path().join("0001-a.md")).unwrap();
    fs::write(dir.path().join("0004-d.md"), "# d").unwrap();
    assert_eq!(files(&mut watcher), ["0001-a.md", "0004-d.md"], "a delete and an add");
}

#[test]
fn a_folder_unchanged_a_tick_after_its_listing_is_not_listed_again() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("0001-a.md"), "# a").unwrap();
    let mut watcher = Watcher::new(vec![dir.path().to_path_buf()]);
    changed(&mut watcher);
    // Listed a tick after the folder last changed: its time alone says whether it did since.
    let listed = &mut watcher.last[0];
    listed.at = listed.changed.map(|changed| changed + TICK * 2);
    // Written over in place, as some editors do: the folder's time stays.
    fs::write(dir.path().join("0001-a.md"), "# a, edited").unwrap();
    assert_eq!(changed(&mut watcher), 0, "not listed again");
    let dirs = watcher.dirs().to_vec();
    assert!(watcher.noticed(&dirs, list(&dirs)), "the timer's listing finds it");
    assert_eq!(changed(&mut watcher), 1, "and the look under the locks that follows takes it");
    // A file added: the folder's time changes.
    let listed = &mut watcher.last[0];
    listed.at = listed.changed.map(|changed| changed + TICK * 2);
    std::thread::sleep(Duration::from_millis(20));
    fs::write(dir.path().join("0002-b.md"), "# b").unwrap();
    assert_eq!(changed(&mut watcher), 1, "listed again");
}

#[test]
fn a_listing_the_same_as_the_last_notices_nothing() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("0001-a.md"), "# a").unwrap();
    let mut watcher = Watcher::new(vec![dir.path().to_path_buf()]);
    let dirs = watcher.dirs().to_vec();
    assert!(watcher.noticed(&dirs, list(&dirs)), "not seen yet");
    changed(&mut watcher);
    assert!(!watcher.noticed(&dirs, list(&dirs)));
    let other = tempfile::tempdir().unwrap();
    fs::write(other.path().join("0001-a.md"), "# a").unwrap();
    let others = vec![other.path().to_path_buf()];
    assert!(!watcher.noticed(&others, list(&others)), "a folder no longer watched is left to the next look");
}

#[test]
fn temporary_and_other_files_are_ignored() {
    let dir = tempfile::tempdir().unwrap();
    let mut watcher = Watcher::new(vec![dir.path().to_path_buf()]);
    fs::write(dir.path().join(".0001-a.md.tmp"), "# a").unwrap();
    fs::create_dir(dir.path().join("reports")).unwrap();
    fs::write(dir.path().join("notes.txt"), "x").unwrap();
    assert_eq!(changed(&mut watcher), 0);
}

#[test]
fn sync_takes_own_writes_as_seen() {
    let dir = tempfile::tempdir().unwrap();
    let mut watcher = Watcher::new(vec![dir.path().to_path_buf()]);
    fs::write(dir.path().join("0001-a.md"), "# a").unwrap();
    watcher.sync(vec![dir.path().to_path_buf()]);
    assert_eq!(changed(&mut watcher), 0);
}

#[test]
fn a_folder_new_to_sync_is_read_again() {
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let mut watcher = Watcher::new(vec![a.path().to_path_buf()]);
    fs::write(b.path().join("0001-a.md"), "# a").unwrap();
    watcher.sync(vec![a.path().to_path_buf(), b.path().to_path_buf()]);
    let dirs: Vec<_> = watcher.changed().into_iter().map(|c| c.dir).collect();
    assert_eq!(dirs, [b.path()], "read before its lock was held");
    assert_eq!(changed(&mut watcher), 0);
}

#[test]
fn a_folder_is_read_again_once_it_is_watched_from_the_start() {
    let dir = tempfile::tempdir().unwrap();
    // Written between the board's first read and the watcher's start (the window opening).
    fs::write(dir.path().join("0001-a.md"), "# a").unwrap();
    let mut watcher = Watcher::new(vec![dir.path().to_path_buf()]);
    assert_eq!(changed(&mut watcher), 1, "read again, as what it holds may be newer than the board");
    assert_eq!(changed(&mut watcher), 0);
}

#[test]
fn a_brains_format_file_changing_is_noticed() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("format"), "abc\n").unwrap();
    let mut watcher = Watcher::new(vec![dir.path().to_path_buf()]);
    changed(&mut watcher);
    // Repaired, a repository left out for it can be read again.
    fs::write(dir.path().join("format"), "1\n").unwrap();
    assert_eq!(changed(&mut watcher), 1);
}

#[test]
fn the_timers_listing_is_taken_under_the_locks_while_the_folder_holds_still() {
    let dir = tempfile::tempdir().unwrap();
    for name in ["0001-a.md", "0002-b.md"] {
        fs::write(dir.path().join(name), "# x").unwrap();
    }
    let mut watcher = Watcher::new(vec![dir.path().to_path_buf()]);
    changed(&mut watcher);
    fs::write(dir.path().join("0001-a.md"), "# a, edited").unwrap();
    let dirs = watcher.dirs().to_vec();
    let mut listings = list(&dirs);
    // Taken a tick after the folder last changed.
    listings[0].at = listings[0].changed.map(|changed| changed + TICK * 2);
    assert!(watcher.noticed(&dirs, listings));
    // After the timer's listing, in place: the folder's time stays, so its listing stands.
    fs::write(dir.path().join("0002-b.md"), "# b, edited").unwrap();
    let files: Vec<_> = watcher.changed().into_iter().flat_map(|c| c.files).collect();
    assert_eq!(files, [dir.path().join("0001-a.md")], "as the timer listed it");
    assert!(watcher.noticed(&dirs, list(&dirs)), "the later edit, at the timer's next look");
}
