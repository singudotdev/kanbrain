use super::*;
use crate::model::Task;

/// A step of test format 2: every task's title in capitals, as a change to every entry would.
fn shout(store: &Store) -> Result<()> {
    let (tasks, _) = store.load_entries()?;
    for mut task in tasks {
        task.title = task.title.to_uppercase();
        store.save(&mut task)?;
    }
    Ok(())
}

/// A step of test format 3: a file only format 3 has.
fn mark(store: &Store) -> Result<()> {
    store::write_file(&store.dir().join("marked"), "yes\n")
}

const STEPS: &[Migration] =
    &[Migration { to: 2, what: "titles shouted", run: shout }, Migration { to: 3, what: "marked", run: mark }];

/// The title in the brain's one entry file, read directly: a brain in test format 3 is newer than [`FORMAT`], which
/// `Store::load_entries` refuses, as it should.
fn title_in(store: &Store) -> String {
    let file = fs::read_dir(store.dir())
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.extension().is_some_and(|e| e == "md"));
    let text = fs::read_to_string(file.unwrap()).unwrap();
    text.lines().next().unwrap().trim_start_matches("# ").to_string()
}

fn brain_with(title: &str) -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::at(dir.path()).unwrap();
    store.save(&mut Task::new(1, title.into(), crate::model::now())).unwrap();
    (dir, store)
}

#[test]
fn every_format_after_the_first_has_its_step() {
    assert_eq!(MIGRATIONS.len(), FORMAT as usize - 1, "a FORMAT bump comes with its migration");
    for (i, step) in MIGRATIONS.iter().enumerate() {
        assert_eq!(step.to as usize, i + 2, "MIGRATIONS[{i}] goes to format {}", i + 2);
    }
}

#[test]
fn an_old_brain_is_backed_up_then_migrated_step_by_step_once() {
    let (_dir, store) = brain_with("fix login");
    assert_eq!(format_of(store.dir()).unwrap(), 1, "no file: format 1");

    let done = migrate_with(&store, STEPS, 3).unwrap();
    assert_eq!(done.len(), 3, "the backup, then each step: {done:?}");
    assert!(done[2].ends_with("to data format 3: marked"), "{done:?}");
    assert_eq!(format_of(store.dir()).unwrap(), 3);
    assert_eq!(title_in(&store), "FIX LOGIN");
    let backup = PathBuf::from(done[0].split(" to ").nth(1).unwrap().trim_end_matches(" before migrating it"));
    assert!(fs::read_dir(&backup).unwrap().count() >= 1, "the files as they were, at {}", backup.display());
    let tmp = std::env::temp_dir().canonicalize().unwrap();
    let root = if cfg!(unix) && tmp.starts_with("/tmp") { PathBuf::from("/tmp") } else { tmp };
    assert!(backup.starts_with(root.join("kanbrain-cache")), "a brain in the temporary folder keeps its cache there");
    assert!(!backup.join(FORMAT_FILE).exists() && !backup.join("marked").exists(), "from before any step");

    assert!(migrate_with(&store, STEPS, 3).unwrap().is_empty(), "already there: nothing to do");
    let (_dir, store) = brain_with("half way");
    store::write_file(&store.dir().join(FORMAT_FILE), "2\n").unwrap();
    let done = migrate_with(&store, STEPS, 3).unwrap();
    assert_eq!(done.len(), 2, "only the steps it lacks: {done:?}");
    assert_eq!(title_in(&store), "half way", "format 2's step isn't run again");
}

#[test]
fn a_brain_a_newer_kanbrain_wrote_is_refused_entirely() {
    let (_dir, store) = brain_with("from the future");
    store::write_file(&store.dir().join(FORMAT_FILE), &format!("{}\n", FORMAT + 1)).unwrap();
    let too_new = |e: anyhow::Error| e.downcast::<TooNew>().unwrap().format;
    assert_eq!(too_new(store.load_entries().unwrap_err()), FORMAT + 1, "not even read");
    assert_eq!(too_new(migrate(&store).unwrap_err()), FORMAT + 1, "nor migrated down");
    assert!(crate::brain::Brain::open(Store::at(store.dir()).unwrap()).is_err(), "nor opened");
    assert!(crate::migrate::too_new(store.dir()));
    store::write_file(&store.dir().join(FORMAT_FILE), "two\n").unwrap();
    assert!(format_of(store.dir()).is_err(), "a format that isn't a number is an error, not format 1");
    // Damaged, not newer: left to what opens it to say, rather than taken for one a newer kanbrain wrote.
    assert!(!crate::migrate::too_new(store.dir()) && refuse_too_new(store.dir()).is_ok());
    store::write_file(&store.dir().join(FORMAT_FILE), "0\n").unwrap();
    assert!(migrate(&store).is_err(), "nor is format 0, which comes before any");
}
