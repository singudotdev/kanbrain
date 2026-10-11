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
    assert_eq!(watcher.changed(), [b.path()]);
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
