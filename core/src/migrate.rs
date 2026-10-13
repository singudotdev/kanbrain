//! Data format versions and migrations. A brain's files (entries, `tags.md`, `last-id`, a project's cards and
//! registry) follow a format numbered in its `format` file, [`FORMAT`] for those this version writes. Formats
//! change only with a migration, which brings a brain from the one before: when a brain is opened to be changed,
//! and for every registered brain on `kb update`, the migrations it lacks run in order, under its write lock,
//! after a copy of it goes to the user's cache. A brain in a newer format than this version knows is refused
//! entirely, read or written: an older `kb` reading a teammate's `.brain/` from git would misread what it doesn't
//! know, and rewriting it would lose it.
//!
//! To change the format: add the step to [`MIGRATIONS`], bump [`FORMAT`], add a fixture of the old format to the
//! tests, and say so in the release notes.

use crate::store::{self, Store};
use anyhow::{Context, Result};
use std::fmt;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

/// The data format this version reads and writes.
pub const FORMAT: u32 = 1;

/// The file beside a brain's entries holding its format. A brain without one is format 1: from before formats
/// were numbered, which the first format is.
pub const FORMAT_FILE: &str = "format";

/// One step from a format to the next.
pub struct Migration {
    /// The format it brings a brain to, from the one before.
    pub to: u32,
    /// What it changes, as `kb update` reports it.
    pub what: &'static str,
    /// Changes the brain's files. It runs under the brain's write lock, once: the format is written after it.
    pub run: fn(&Store) -> Result<()>,
}

/// Every step, in order: `MIGRATIONS[i]` brings a brain from format `i + 1` to `i + 2`.
pub const MIGRATIONS: &[Migration] = &[];

/// A brain in a format newer than [`FORMAT`]: written by a newer kanbrain, so this one leaves it alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TooNew {
    /// The brain's folder.
    pub dir: PathBuf,
    /// Its format.
    pub format: u32,
}

impl fmt::Display for TooNew {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} was written by a newer kanbrain (data format {}; this one knows up to {FORMAT}): update it with \
             `kb update`",
            self.dir.display(),
            self.format
        )
    }
}

impl std::error::Error for TooNew {}

/// The format of the brain in `dir`.
pub fn format_of(dir: &Path) -> Result<u32> {
    let path = dir.join(FORMAT_FILE);
    match fs::read_to_string(&path) {
        Ok(text) => text.trim().parse().with_context(|| format!("{} isn't a format number", path.display())),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(1),
        Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
    }
}

/// Refuses, with [`TooNew`], a brain in a format newer than [`FORMAT`].
pub fn check(dir: &Path) -> Result<()> {
    match format_of(dir)? {
        format if format > FORMAT => Err(TooNew { dir: dir.to_path_buf(), format }.into()),
        _ => Ok(()),
    }
}

/// Brings the brain in `store` to [`FORMAT`], and returns what it did, one line each; nothing for a brain already
/// there.
pub fn migrate(store: &Store) -> Result<Vec<String>> {
    migrate_with(store, MIGRATIONS, FORMAT)
}

/// [`migrate`], with the steps and the format they lead to given, so tests can run steps of their own.
fn migrate_with(store: &Store, migrations: &[Migration], target: u32) -> Result<Vec<String>> {
    let dir = store.dir();
    let behind = |format: u32| -> Result<bool> {
        if format > target {
            return Err(TooNew { dir: dir.to_path_buf(), format }.into());
        }
        Ok(format < target)
    };
    if !behind(format_of(dir)?)? {
        return Ok(Vec::new());
    }
    let _lock = store.lock()?;
    // Again under the lock: another `kb` may have migrated it meanwhile.
    let from = format_of(dir)?;
    if !behind(from)? {
        return Ok(Vec::new());
    }
    let backup = back_up(dir, from, target)?;
    let mut done = vec![format!("backed up {} to {} before migrating it", dir.display(), backup.display())];
    for step in &migrations[(from - 1) as usize..(target - 1) as usize] {
        (step.run)(store).with_context(|| format!("migrating {} to format {}", dir.display(), step.to))?;
        store::write_file(&dir.join(FORMAT_FILE), &format!("{}\n", step.to))?;
        done.push(format!("migrated {} to data format {}: {}", dir.display(), step.to, step.what));
    }
    Ok(done)
}

/// Copies the brain in `dir`, every file and folder in it, to `<cache>/kanbrain/backups/<key>/<time>-<from>-<to>`,
/// and returns where.
fn back_up(dir: &Path, from: u32, to: u32) -> Result<PathBuf> {
    let at = crate::model::now().format("%Y%m%dT%H%M%S");
    let backup = store::cache_dir_of(dir, "backups")?.join(format!("{at}-format-{from}-to-{to}"));
    copy_dir(dir, &backup)?;
    Ok(backup)
}

/// Copies `from` into `to`, created, recursively.
fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to).with_context(|| format!("creating {}", to.display()))?;
    for entry in fs::read_dir(from).with_context(|| format!("reading {}", from.display()))? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target).with_context(|| format!("copying {}", entry.path().display()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "../tests/migrate.rs"]
mod tests;
