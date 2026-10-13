//! The data folder: one `NNNN-slug.md` file per task, `tags.md`, and exported reports in `reports/`.

use crate::model::{Task, slugify};
use crate::projects;
use crate::tags::TagColors;
use anyhow::{Context, Result, bail};
use std::cell::RefCell;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

/// Tag color assignments; lives next to the task files but isn't a task.
const TAGS_FILE: &str = "tags.md";

/// A project's own data folder, found by walking up from the current directory like `.git`.
pub const PROJECT_DIR: &str = ".brain";

/// Settings file of older versions (`- track: on|off`); time is always tracked now, so it's only
/// kept out of the task files.
const CONFIG_FILE: &str = "config.md";

/// The highest id the folder has given an entry, so ids are never handed out twice: a link to a deleted
/// entry, left in a file edited by hand or in a copy that was offline, would otherwise point at the
/// unrelated entry that took its number. Not markdown, so it's never read as an entry.
const LAST_ID_FILE: &str = "last-id";

/// Subfolder for markdown report exports; never read back.
const REPORTS_DIR: &str = "reports";

/// The data folder where tasks are stored.
#[derive(Debug)]
pub struct Store {
    /// Absolute or relative path of the folder; created on open.
    dir: PathBuf,
    /// While recording (see [`Store::record`]), each file written or deleted as it was before.
    journal: RefCell<Option<Vec<Change>>>,
}

/// A file a change wrote or deleted: what it held before and after, `None` where it didn't exist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    /// The file.
    pub path: PathBuf,
    /// Its contents before the change.
    pub before: Option<String>,
    /// Its contents after the change, filled in by [`Store::recorded`].
    pub after: Option<String>,
}

/// Why changes can't be undone.
#[derive(Debug, PartialEq, Eq)]
pub enum UndoError {
    /// Another program, or a later change, wrote this file since: undoing would lose that.
    ChangedSince(String),
}

impl std::fmt::Display for UndoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ChangedSince(name) => write!(f, "can't undo: {name} changed since"),
        }
    }
}

impl std::error::Error for UndoError {}

/// Puts the files `changes` wrote back as they were before, last change first. Nothing is written
/// unless every file still holds what the changes left in it.
///
/// # Errors
///
/// [`UndoError::ChangedSince`] when a file was changed since, or the error of a write that failed.
pub fn undo(changes: &[Change]) -> Result<()> {
    for change in changes {
        if fs::read_to_string(&change.path).ok() != change.after {
            let name = change.path.file_name().unwrap_or_default().to_string_lossy().into_owned();
            return Err(UndoError::ChangedSince(name).into());
        }
    }
    for change in changes.iter().rev() {
        match &change.before {
            Some(text) => write_file(&change.path, text)?,
            None => fs::remove_file(&change.path).with_context(|| format!("removing {}", change.path.display()))?,
        }
    }
    Ok(())
}

impl Store {
    /// The nearest `.brain/` in the current directory or above it, so each project's repository
    /// keeps its own; else the central brain (`$KANBRAIN_DIR`, else the platform data dir, e.g.
    /// `~/.local/share/kanbrain`).
    pub fn open() -> Result<Self> {
        let cwd = std::env::current_dir().context("reading the current directory")?;
        match find_project(&cwd) {
            Some(dir) => Self::at(dir),
            None => Self::at(projects::central_dir()?),
        }
    }

    /// The brain of the project registered as `name` (`central` for the central one), from anywhere;
    /// without a name, the one [`Store::open`] finds.
    pub fn open_named(name: Option<&str>) -> Result<Self> {
        match name {
            None => Self::open(),
            Some(projects::CENTRAL) => Self::at(projects::central_dir()?),
            Some(label) => Self::at(projects::find(&projects::central_dir()?, label)?.dir),
        }
    }

    /// The project's name: the one on its card (`project.md`), else the folder holding `.brain/`;
    /// `None` for the central brain.
    pub fn project(&self) -> Option<String> {
        if let Some((name, _)) = projects::read_repo_card(&self.dir) {
            return Some(name);
        }
        if self.dir.file_name()? != PROJECT_DIR {
            return None;
        }
        let root = self.dir.parent()?;
        // `.brain` given relatively has an empty parent: the current directory.
        let root = if root.as_os_str().is_empty() { std::env::current_dir().ok()? } else { root.to_path_buf() };
        Some(root.file_name()?.to_string_lossy().into_owned())
    }

    /// A store in `dir`, created if missing.
    pub fn at(dir: impl Into<PathBuf>) -> Result<Self> {
        let dir = dir.into();
        fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
        Ok(Self { dir, journal: RefCell::new(None) })
    }

    /// Takes this brain's write lock, waiting while another program holds it, until the returned guard is
    /// dropped. Each `kb` that changes entries reads every file, changes some and writes them back: an agent
    /// running several at once (one per subagent) would otherwise have the last one write over the others'
    /// changes, or two `add`s take the same id. The lock is released when the process ends, however it ends.
    ///
    /// The lock file lives in the user's cache folder, named after the brain's folder, so `.brain/` (often
    /// committed) gets no extra file.
    pub fn lock(&self) -> Result<BrainLock> {
        let path = lock_path(&self.dir)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        }
        let file = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)
            .with_context(|| format!("opening {}", path.display()))?;
        fs4::FileExt::lock(&file).with_context(|| format!("locking {}", path.display()))?;
        Ok(BrainLock(file))
    }

    /// Starts recording the files written or deleted from now on, so they can be put back (see [`undo`]).
    pub fn record(&self) {
        *self.journal.borrow_mut() = Some(Vec::new());
    }

    /// Stops recording, and returns what was recorded, with what each file holds now.
    pub fn recorded(&self) -> Vec<Change> {
        let mut changes = self.journal.borrow_mut().take().unwrap_or_default();
        for change in &mut changes {
            change.after = fs::read_to_string(&change.path).ok();
        }
        changes.retain(|c| c.before != c.after);
        changes
    }

    /// Keeps `path` as it is now, the first time it's about to change while recording.
    fn remember(&self, path: &Path) {
        if let Some(journal) = self.journal.borrow_mut().as_mut()
            && !journal.iter().any(|c| c.path == path)
        {
            journal.push(Change { path: path.to_path_buf(), before: fs::read_to_string(path).ok(), after: None });
        }
    }

    /// The data folder.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Where report exports are written.
    pub fn reports_dir(&self) -> PathBuf {
        self.dir.join(REPORTS_DIR)
    }

    /// Returns the parsed tasks plus a warning for every file that could not be read.
    pub fn load(&self) -> Result<(Vec<Task>, Vec<String>)> {
        let mut tasks = Vec::new();
        let mut warnings = Vec::new();
        let entries = fs::read_dir(&self.dir).with_context(|| format!("reading {}", self.dir.display()))?;
        for entry in entries {
            let path = entry?.path();
            if !is_task_file(&path) {
                continue;
            }
            let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
            let parsed =
                fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|text| Task::from_markdown(&text));
            match parsed {
                Ok(mut task) => {
                    task.path = Some(path);
                    tasks.push(task);
                }
                Err(e) => warnings.push(format!("{name}: {e}")),
            }
        }
        Ok((tasks, warnings))
    }

    /// Writes atomically and renames the file when the title (slug) changed.
    pub fn save(&self, task: &mut Task) -> Result<()> {
        let name = format!("{:04}-{}.md", task.id, slugify(&task.title));
        let path = self.dir.join(&name);
        self.remember(&path);
        if let Some(old) = &task.path {
            self.remember(old);
        }
        write_file(&path, &task.to_markdown())?;
        // A new title means a new file name: remove the file saved under the old one.
        if let Some(old) = task.path.replace(path)
            && Some(&old) != task.path.as_ref()
        {
            // The old name only matters if it still exists; a failure here leaves a stale copy, not lost data.
            let _ = fs::remove_file(old);
        }
        Ok(())
    }

    /// Removes the task's file. Refuses anything outside the data folder.
    pub fn delete(&self, task: &Task) -> Result<()> {
        let Some(path) = &task.path else { return Ok(()) };
        if !path.starts_with(&self.dir) {
            bail!("refusing to delete {}: not in {}", path.display(), self.dir.display());
        }
        self.remember(path);
        fs::remove_file(path).with_context(|| format!("deleting {}", path.display()))
    }

    /// The highest id given so far, as `last-id` keeps it; 0 when it's missing (a folder from an older
    /// version) or unreadable, and then the entries' own ids are all there is to go by.
    pub fn last_id(&self) -> u32 {
        fs::read_to_string(self.dir.join(LAST_ID_FILE)).ok().and_then(|text| text.trim().parse().ok()).unwrap_or(0)
    }

    /// Writes `last-id`.
    pub fn save_last_id(&self, id: u32) -> Result<()> {
        let path = self.dir.join(LAST_ID_FILE);
        write_file(&path, &format!("{id}\n"))
    }

    /// Reads `tags.md`; no file means no colors assigned yet.
    pub fn load_tag_colors(&self) -> Result<TagColors> {
        let path = self.dir.join(TAGS_FILE);
        match fs::read_to_string(&path) {
            Ok(text) => Ok(TagColors::parse(&text)),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(TagColors::default()),
            Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
        }
    }

    /// Writes `tags.md`.
    pub fn save_tag_colors(&self, colors: &TagColors) -> Result<()> {
        let path = self.dir.join(TAGS_FILE);
        self.remember(&path);
        write_file(&path, &colors.to_markdown())
    }
}

/// A brain's write lock, held until dropped (see [`Store::lock`]).
#[derive(Debug)]
pub struct BrainLock(#[expect(dead_code, reason = "held only for its lock, released on drop")] fs::File);

/// Where the lock of the brain in `dir` is: `<cache>/kanbrain/locks/<hash>.lock`, the hash of the folder's
/// canonical path. FNV-1a rather than std's hasher, whose output may change between Rust versions: two `kb`
/// builds must agree on the file to exclude each other.
fn lock_path(dir: &Path) -> Result<PathBuf> {
    let dir = dir.canonicalize().with_context(|| format!("reading {}", dir.display()))?;
    let hash = dir
        .as_os_str()
        .as_encoded_bytes()
        .iter()
        .fold(0xcbf2_9ce4_8422_2325_u64, |hash, &byte| (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3));
    let cache = dirs::cache_dir().unwrap_or_else(std::env::temp_dir);
    Ok(cache.join("kanbrain").join("locks").join(format!("{hash:016x}.lock")))
}

/// Writes `text` to `path`, creating its folder if missing. It goes to a hidden temporary file beside it
/// first, then takes the file's place in one rename: a crash or a full disk can never leave a file half
/// written, be it an entry, `tags.md` or a repository's `CLAUDE.md`. The watcher ignores hidden files.
pub(crate) fn write_file(path: &Path, text: &str) -> Result<()> {
    let dir = path.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
    fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    let name = path.file_name().with_context(|| format!("{} isn't a file", path.display()))?;
    let tmp = dir.join(format!(".{}.tmp", name.to_string_lossy()));
    fs::write(&tmp, text).with_context(|| format!("writing {}", tmp.display()))?;
    fs::rename(&tmp, path).with_context(|| format!("writing {}", path.display()))
}

/// The nearest `.brain/` folder in `start` or one of its parents.
pub fn find_project(start: &Path) -> Option<PathBuf> {
    start.ancestors().map(|dir| dir.join(PROJECT_DIR)).find(|dir| dir.is_dir())
}

/// Task files are the `.md` files in the folder, except the tag colors, the project's card, the
/// central brain's list of projects and the old settings.
fn is_task_file(path: &Path) -> bool {
    let other = [TAGS_FILE, CONFIG_FILE, projects::PROJECT_FILE, projects::REGISTRY_FILE];
    path.extension().is_some_and(|e| e == "md") && path.file_name().is_some_and(|n| !other.iter().any(|o| n == *o))
}

#[cfg(test)]
#[path = "../tests/store.rs"]
mod tests;
