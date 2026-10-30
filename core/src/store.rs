//! The data folder: one `NNNN-slug.md` file per task, `tags.md`, and exported reports in `reports/`.

use crate::model::{ParseError, Task, slugify};
use crate::projects;
use crate::tags::TagColors;
use anyhow::{Context, Result, bail};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::io::{ErrorKind, Read as _, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, SystemTime};

/// Tag color assignments; lives next to the task files but isn't a task.
pub(crate) const TAGS_FILE: &str = "tags.md";

/// A project's own data folder, found by walking up from the current directory like `.git`.
pub const PROJECT_DIR: &str = ".brain";

/// The highest id the folder has given an entry, so ids are never handed out twice: a link to a deleted
/// entry, left in a file edited by hand or in a copy that was offline, would otherwise point at the
/// unrelated entry that took its number. Not markdown, so it's never read as an entry.
pub(crate) const LAST_ID_FILE: &str = "last-id";

/// Subfolder for markdown report exports; never read back.
pub(crate) const REPORTS_DIR: &str = "reports";

/// The data folder where tasks are stored.
#[derive(Debug)]
pub struct Store {
    /// Absolute or relative path of the folder; created on open.
    dir: PathBuf,
    /// While recording (see [`Store::record`]), each file written or deleted as it was before.
    journal: RefCell<Option<Vec<Change>>>,
    /// Each entry's file as this store last read or wrote it: [`Store::save`] refuses to write over one another
    /// program changed since, which would lose what it wrote.
    read: RefCell<HashMap<PathBuf, Seen>>,
}

/// When a file last changed, and its size: what tells it changed without reading it.
type Seen = (Option<SystemTime>, u64);

/// The file at `path` as it is now, if it's there.
fn seen(path: &Path) -> Option<Seen> {
    fs::metadata(path).ok().as_ref().map(seen_in)
}

/// The file whose metadata this is, as [`seen`] says it.
fn seen_in(meta: &fs::Metadata) -> Seen {
    (meta.modified().ok(), meta.len())
}

/// Threads that read a brain's files at most: with four, `kb show` over 14,000 entries took as long as with eight.
const READ_THREADS: usize = 4;
/// Files a thread reads at least: a small brain is read by one, rather than wait for threads to start.
pub(crate) const READ_PER_THREAD: usize = 512;

/// An entry read from its file, with the file as it was before it was read (see [`seen`]), or why it isn't one.
type Read = Result<(Task, Option<Seen>), Skipped>;

/// Reads the entries in the files at `paths`.
fn read_entries(paths: &[PathBuf]) -> Vec<Read> {
    paths.iter().map(|path| read_entry(path)).collect()
}

/// Reads the entry in the file at `path`.
fn read_entry(path: &Path) -> Read {
    // Before reading, of the file opened: changed in between, it reads as changed since, never as read when it wasn't.
    // Opened once for both, as looking a file up by its path costs as much as reading it.
    let mut was = None;
    let read = fs::File::open(path).and_then(|mut file| {
        was = file.metadata().ok().as_ref().map(seen_in);
        let mut text = String::new();
        file.read_to_string(&mut text)?;
        Ok(without_bom(text))
    });
    let parsed = read.map_err(|e| SkipReason::Unreadable(e.to_string())).and_then(|text| {
        Task::parse(&text).map_err(|e| match e {
            ParseError::MissingId => SkipReason::NoId,
            ParseError::Line(why) => SkipReason::Broken(why),
        })
    });
    match parsed {
        Ok(mut task) => {
            task.path = Some(path.to_path_buf());
            Ok((task, was))
        }
        Err(why) => Err(Skipped { file: path.file_name().unwrap_or_default().to_string_lossy().into_owned(), why }),
    }
}

/// A markdown file [`Store::load_entries`] didn't read as an entry, said as `{file}: {why}`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Skipped {
    /// Its name in the brain's folder.
    pub file: String,
    /// Why it isn't an entry.
    pub why: SkipReason,
}

/// Why a file isn't an entry.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SkipReason {
    /// It has no `- id:` line: other markdown, which a central brain kept in a notes vault holds, rather than an entry.
    NoId,
    /// An entry with a line that doesn't read (a session or history line), saying which.
    Broken(String),
    /// The file couldn't be read, with the system's error.
    Unreadable(String),
}

impl std::fmt::Display for Skipped {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.file, self.why)
    }
}

impl std::fmt::Display for SkipReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SkipReason::NoId => f.write_str(crate::model::MISSING_ID),
            SkipReason::Broken(why) | SkipReason::Unreadable(why) => f.write_str(why),
        }
    }
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
            None => remove_file(&change.path).with_context(|| format!("removing {}", change.path.display()))?,
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
            Some(label) => {
                let central = projects::central_dir()?;
                let repo = match projects::find(&central, label) {
                    Ok(repo) => repo,
                    // In any case, as projects' names are, unless a project has it.
                    Err(_) if label.eq_ignore_ascii_case(projects::CENTRAL) => return Self::at(central),
                    Err(e) => return Err(e),
                };
                if !repo.dir.is_dir() {
                    bail!("{} is gone: `kb --project {label} deinit` takes it out", repo.dir.display());
                }
                Self::at(repo.dir)
            }
        }
    }

    /// The project's name: the one on its card (`project.md`), else the folder holding `.brain/`;
    /// `None` for the central brain.
    pub fn project(&self) -> Option<String> {
        if let Some(projects::RepoCard { project: name, .. }) = projects::read_repo_card(&self.dir) {
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
        if !dir.is_dir() {
            fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
            // A brain made now is in this version's format: without the file it would read as format 1 once there's
            // a 2, and have the migrations it doesn't need run over it.
            let format = format!("{}\n", crate::migrate::FORMAT);
            write_file(&dir.join(crate::migrate::FORMAT_FILE), &format)?;
        }
        Ok(Self { dir, journal: RefCell::new(None), read: RefCell::default() })
    }

    /// Takes this brain's write lock, waiting while another program holds it, until the returned guard is
    /// dropped. Each `kb` that changes entries reads every file, changes some and writes them back: an agent
    /// running several at once (one per subagent) would otherwise have the last one write over the others'
    /// changes, or two `add`s take the same id. The lock is released when the process ends, however it ends.
    ///
    /// The lock file lives in the user's cache folder, named after the brain's folder, so `.brain/` (often
    /// committed) gets no extra file. Taking it again on a thread that holds it gets a guard that does nothing:
    /// the app holds every brain's while it acts, and managing projects takes some of them itself.
    pub fn lock(&self) -> Result<BrainLock> {
        let path = lock_path(&self.dir)?;
        if HELD.with(|held| held.borrow().contains(&path)) {
            return Ok(BrainLock { file: None, path });
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        }
        let mut file = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)
            .with_context(|| format!("opening {}", path.display()))?;
        fs4::FileExt::lock(&file).with_context(|| format!("locking {}", path.display()))?;
        // Which brain it's for, in one write ending in a newline, so the cache can be tidied once the brain is gone.
        if file.metadata().is_ok_and(|m| m.len() == 0) {
            let brain = self.dir.canonicalize().with_context(|| format!("reading {}", self.dir.display()))?;
            file.write_all(format!("{}\n", brain.display()).as_bytes())
                .with_context(|| format!("writing {}", path.display()))?;
        }
        // Only once nothing can fail: an error drops `file`, releasing the lock, and a path left in `HELD` would
        // have every later `lock` on this thread hand back a guard that holds nothing.
        HELD.with(|held| held.borrow_mut().push(path.clone()));
        forget_gone_temp_brains(&path);
        forget_cut_writes(&self.dir);
        Ok(BrainLock { file: Some(file), path })
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

    /// The file at `path` as this store last read or wrote it: when it last changed, and its size. `None` for a file
    /// it hasn't, or deleted since. What it is now differing, another program changed it.
    pub fn known(&self, path: &Path) -> Option<(Option<SystemTime>, u64)> {
        self.read.borrow().get(path).copied()
    }

    /// The data folder.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Where report exports are written.
    pub fn reports_dir(&self) -> PathBuf {
        self.dir.join(REPORTS_DIR)
    }

    /// The ids of the entries' files, by their names (`0005-fix-login.md` is 5), read or not: an entry whose file
    /// doesn't parse is still there, and links to it must stay.
    pub(crate) fn file_ids(&self) -> std::collections::HashSet<u32> {
        let Ok(entries) = fs::read_dir(&self.dir) else { return std::collections::HashSet::new() };
        entries
            .filter_map(|entry| entry.ok()?.path().file_name()?.to_str().map(str::to_string))
            .filter(|name| is_task_file(Path::new(name)))
            .filter_map(|name| name.split('-').next()?.parse().ok())
            .collect()
    }

    /// Returns the parsed tasks plus a warning for every file that could not be read.
    #[deprecated(since = "0.6.3", note = "use `Store::load_entries`, which says why each file was skipped")]
    pub fn load(&self) -> Result<(Vec<Task>, Vec<String>)> {
        let (tasks, skipped) = self.load_entries()?;
        Ok((tasks, skipped.iter().map(ToString::to_string).collect()))
    }

    /// Returns the parsed tasks, and every markdown file that didn't read as one, with why.
    pub fn load_entries(&self) -> Result<(Vec<Task>, Vec<Skipped>)> {
        // Every read goes through here: a brain a newer kanbrain wrote is refused before anything misreads it.
        crate::migrate::check(&self.dir)?;
        let entries = fs::read_dir(&self.dir).with_context(|| format!("reading {}", self.dir.display()))?;
        let mut files = Vec::new();
        for entry in entries {
            let path = entry?.path();
            if is_task_file(&path) {
                files.push(path);
            }
        }
        self.read.borrow_mut().clear();
        // Side by side: a command reads every entry of the brain, and one after another ten thousand took most of the
        // time of a `kb show`.
        let mut shares = files.chunks(files.len().div_ceil(READ_THREADS).max(READ_PER_THREAD));
        let read = std::thread::scope(|scope| {
            let first = shares.next().unwrap_or_default();
            let spawn = |share| std::thread::Builder::new().spawn_scoped(scope, move || read_entries(share));
            let others: Vec<_> = shares.map(|share| (share, spawn(share))).collect();
            let mut read = read_entries(first);
            for (share, other) in others {
                read.extend(match other {
                    Ok(other) => other.join().unwrap_or_else(|panic| std::panic::resume_unwind(panic)),
                    // No thread to be had (a container's limit on them): read here, as a brain was before.
                    Err(_) => read_entries(share),
                });
            }
            read
        });
        Ok(self.keep_read(read))
    }

    /// [`Store::load_entries`] of only the files named for entry `id` (`0007-…`, as [`file_name`] names them): what a
    /// command on that entry alone reads, a listing of the folder and its file rather than every entry's.
    pub(crate) fn load_named(&self, id: u32) -> Result<(Vec<Task>, Vec<Skipped>)> {
        crate::migrate::check(&self.dir)?;
        let named = format!("{id:04}-");
        let entries = fs::read_dir(&self.dir).with_context(|| format!("reading {}", self.dir.display()))?;
        let mut files = Vec::new();
        for entry in entries {
            let entry = entry?;
            if entry.file_name().to_str().is_some_and(|name| name.starts_with(&named)) && is_task_file(&entry.path()) {
                files.push(entry.path());
            }
        }
        Ok(self.keep_read(files.iter().map(|path| read_entry(path))))
    }

    /// [`Store::load_entries`] of only `files`, entries' files of this folder: those gone since are left out.
    pub(crate) fn load_files(&self, files: &[PathBuf]) -> (Vec<Task>, Vec<Skipped>) {
        let mut read = self.read.borrow_mut();
        for path in files {
            read.remove(path);
        }
        drop(read);
        self.keep_read(files.iter().filter(|path| path.exists()).map(|path| read_entry(path)))
    }

    /// The entries read, each file remembered as read (see [`Store::known`]), and the files that didn't read as one.
    fn keep_read(&self, read: impl IntoIterator<Item = Read>) -> (Vec<Task>, Vec<Skipped>) {
        let (mut tasks, mut skipped) = (Vec::new(), Vec::new());
        let mut known = self.read.borrow_mut();
        for one in read {
            match one {
                Ok((task, was)) => {
                    if let (Some(path), Some(was)) = (&task.path, was) {
                        known.insert(path.clone(), was);
                    }
                    tasks.push(task);
                }
                Err(why) => skipped.push(why),
            }
        }
        (tasks, skipped)
    }

    /// Writes atomically and renames the file when the title (slug) changed.
    pub fn save(&self, task: &mut Task) -> Result<()> {
        let path = self.dir.join(file_name(task));
        // A name that differs only in case (renamed by hand or by a sync tool) is kept: on a case-insensitive file
        // system it's the same file, and removing the "old" one after writing the new would delete the only copy.
        let path = match &task.path {
            Some(old) if old.parent() == Some(self.dir.as_path()) && same_ignoring_case(old, &path) => old.clone(),
            _ => path,
        };
        // A file there that isn't this entry's is one that didn't parse (a sync conflict's, a hand edit's): writing
        // over it would lose what it holds.
        if task.path.as_ref() != Some(&path) && path.exists() {
            bail!("{} is there and couldn't be read as #{}: fix or move it, then try again", path.display(), task.id);
        }
        // Changed by another program since it was read (an editor writing over the file, which the app's watcher
        // takes a second to notice) or deleted: written from memory, what it holds now would be lost.
        if let Some(old) = &task.path
            && let Some(was) = self.read.borrow().get(old).copied()
            && seen(old) != Some(was)
        {
            bail!(
                "{} changed or deleted on disk since it was read: not saved over it, read it again first",
                old.display()
            );
        }
        self.remember(&path);
        if let Some(old) = &task.path {
            self.remember(old);
        }
        write_file(&path, &task.to_markdown())?;
        let mut read = self.read.borrow_mut();
        if let Some(old) = &task.path {
            read.remove(old);
        }
        if let Some(now) = seen(&path) {
            read.insert(path.clone(), now);
        }
        drop(read);
        // A new title means a new file name: remove the file saved under the old one.
        if let Some(old) = task.path.replace(path)
            && Some(&old) != task.path.as_ref()
        {
            // The old name only matters if it still exists; a failure here leaves a stale copy, not lost data.
            let _ = remove_file(&old);
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
        self.read.borrow_mut().remove(path);
        remove_file(path).with_context(|| format!("deleting {}", path.display()))
    }

    /// The highest id given so far, as `last-id` keeps it; 0 when it's missing (a folder from an older
    /// version) or unreadable, and then the entries' own ids are all there is to go by.
    pub fn last_id(&self) -> u32 {
        read_text(&self.dir.join(LAST_ID_FILE)).ok().and_then(|text| text.trim().parse().ok()).unwrap_or(0)
    }

    /// Writes `last-id`.
    pub fn save_last_id(&self, id: u32) -> Result<()> {
        let path = self.dir.join(LAST_ID_FILE);
        write_file(&path, &format!("{id}\n"))
    }

    /// Reads `tags.md`; no file means no colors assigned yet.
    pub fn load_tag_colors(&self) -> Result<TagColors> {
        let path = self.dir.join(TAGS_FILE);
        match read_text(&path) {
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

thread_local! {
    /// The lock files this thread holds. `flock` on a second handle of the same file waits for the first, so a
    /// thread taking a lock it holds would wait for itself forever.
    static HELD: RefCell<Vec<PathBuf>> = const { RefCell::new(Vec::new()) };
}

/// A brain's write lock, held until dropped (see [`Store::lock`]).
#[derive(Debug)]
pub struct BrainLock {
    /// The locked file, whose handle holds the lock; `None` for a lock the thread already held.
    file: Option<fs::File>,
    /// The lock file.
    path: PathBuf,
}

impl Drop for BrainLock {
    fn drop(&mut self) {
        // The handle closes after this, releasing the lock.
        if self.file.is_some() {
            HELD.with(|held| held.borrow_mut().retain(|p| *p != self.path));
        }
    }
}

/// Where the lock of the brain in `dir` is: `<cache>/kanbrain/locks/<key>.lock` (see [`cache_dir_of`]).
fn lock_path(dir: &Path) -> Result<PathBuf> {
    Ok(cache_dir_of(dir, "locks")?.with_extension("lock"))
}

/// `<cache>/kanbrain/<kind>/<key>`, where the user's cache keeps something about the brain in `dir` (its lock, its
/// backups) out of `.brain/`, which is often committed. The key is [`fnv1a`] of the folder's canonical path, which
/// two `kb` builds must agree on. A brain in
/// the system's temporary folder (a test's, a try-out) keeps it in `<tmp>/kanbrain-cache/<kind>/<key>` instead, so
/// a throwaway brain leaves nothing in the user's cache, and what it leaves there goes once it's gone (see
/// [`forget_gone_temp_brains`]); under `/tmp`, in `/tmp/kanbrain-cache`.
pub(crate) fn cache_dir_of(dir: &Path, kind: &str) -> Result<PathBuf> {
    let dir = dir.canonicalize().with_context(|| format!("reading {}", dir.display()))?;
    let hash = fnv1a(dir.as_os_str().as_encoded_bytes());
    // `/tmp` first, whatever `TMPDIR` says: two processes on one brain must agree on its lock, and some set
    // `TMPDIR` to a folder inside `/tmp` (Claude Code does) while others don't.
    let tmp = std::env::temp_dir();
    let roots = [cfg!(unix).then(|| PathBuf::from("/tmp")), Some(tmp.clone())];
    let cache = roots
        .into_iter()
        .flatten()
        .filter_map(|root| root.canonicalize().ok())
        .find(|root| dir.starts_with(root))
        .map_or_else(|| dirs::cache_dir().unwrap_or(tmp).join("kanbrain"), |root| root.join(TEMP_CACHE));
    Ok(cache.join(kind).join(format!("{hash:016x}")))
}

/// The folder of the system's temporary folder where brains there keep what the user's cache would.
const TEMP_CACHE: &str = "kanbrain-cache";

/// The 64-bit FNV-1a hash of `bytes`: for what must come out the same from every build, which std's hasher doesn't
/// promise across Rust versions.
pub(crate) fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, &byte| (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3))
}

/// Deletes what brains in the system's temporary folder left in its cache once each is gone, as a test's or a
/// try-out's is: nothing can ask for it after. Run as one of them is locked (`lock`, its lock file), so that cache
/// tidies itself as throwaway brains come and go; the user's own is left alone. A lock file names its brain on its
/// first line; one that doesn't yet (being written, or from an older version) is kept.
fn forget_gone_temp_brains(lock: &Path) {
    let Some(locks) = lock.parent().filter(|locks| locks.parent().is_some_and(|c| c.ends_with(TEMP_CACHE))) else {
        return;
    };
    let Ok(entries) = fs::read_dir(locks) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let named = fs::read_to_string(&path).ok().and_then(|text| Some(PathBuf::from(text.strip_suffix('\n')?)));
        if named.is_some_and(|brain| !brain.exists()) {
            // Another `kb` may be tidying too: whichever gets there first deletes them.
            let _ = fs::remove_file(&path);
            if let (Some(cache), Some(key)) = (locks.parent(), path.file_stem()) {
                let _ = fs::remove_dir_all(cache.join("backups").join(key));
            }
        }
    }
}

/// How old a write's temporary file has to be to count as a crash's leftover: no write takes that long.
const CUT_WRITE_AGE: Duration = Duration::from_secs(60 * 60);

/// Deletes the temporary files of writes a crash cut short (see [`write_file`]) in the brain in `dir` and its
/// folders, best effort. Each has a name of its own, so no later write replaces it. Run once its lock is taken,
/// when nothing else writes there; the age spares writes made without the lock, as `init` does.
fn forget_cut_writes(dir: &Path) {
    let folders = fs::read_dir(dir).into_iter().flatten().flatten().filter(|e| e.file_type().is_ok_and(|t| t.is_dir()));
    let files = std::iter::once(dir.to_path_buf())
        .chain(folders.map(|folder| folder.path()))
        .filter_map(|folder| fs::read_dir(folder).ok())
        .flatten()
        .flatten();
    for file in files {
        let name = file.file_name();
        let name = name.to_string_lossy();
        // By the name first: the lock is taken for every write, and reading the metadata of every entry's file for
        // the few temporary files among them made it cost as much as the entries are many.
        if !is_cut_write(&name) {
            continue;
        }
        let age = file.metadata().and_then(|m| m.modified()).ok().and_then(|at| at.elapsed().ok());
        if age.is_some_and(|age| age > CUT_WRITE_AGE) {
            let _ = fs::remove_file(file.path());
        }
    }
}

/// Whether `name` is a temporary file of [`write_file`]'s: `.NAME.PID-N.tmp`, or `.NAME.tmp` as versions before 0.4.0
/// named them, where NAME is a file kanbrain writes (`*.md`, `format`, `last-id`). A central brain may be a notes
/// vault, where other programs leave hidden `.tmp` files of their own.
fn is_cut_write(name: &str) -> bool {
    let Some(written) = name.strip_prefix('.').and_then(|n| n.strip_suffix(".tmp")) else { return false };
    let written = match written.rsplit_once('.') {
        Some((file, write)) if write.split_once('-').is_some_and(|(pid, n)| is_number(pid) && is_number(n)) => file,
        _ => written,
    };
    Path::new(written).extension().is_some_and(|ext| ext == "md")
        || written == crate::migrate::FORMAT_FILE
        || written == LAST_ID_FILE
}

/// Whether `text` is digits only, and some.
fn is_number(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit())
}

/// What the cache keeps about the brain in `dir`, its lock file and its backups, to delete once the brain is: nothing
/// asks for them after. Found while the brain is still there, since the key comes from its canonical path.
pub(crate) fn cache_of(dir: &Path) -> Result<[PathBuf; 2]> {
    Ok([lock_path(dir)?, cache_dir_of(dir, "backups")?])
}

/// Reads a text file people may edit, without the byte-order mark some editors (Notepad, older Windows ones) put
/// first: left in, it hides a file's first line, an entry's `# title` among them.
pub(crate) fn read_text(path: &Path) -> std::io::Result<String> {
    fs::read_to_string(path).map(without_bom)
}

/// `text` without its byte-order mark, as [`read_text`] reads it.
fn without_bom(text: String) -> String {
    match text.strip_prefix('\u{feff}') {
        Some(rest) => rest.to_string(),
        None => text,
    }
}

/// Writes `text` to `path`, creating its folder if missing. It goes to a hidden temporary file beside it
/// first, flushed to disk, then takes the file's place in one rename: a crash or a full disk can never leave a
/// file half written, be it an entry, `tags.md` or a repository's `CLAUDE.md`. The temporary file's name is this
/// write's own, so two programs writing the same file can't swap halves of each other's. The watcher ignores
/// hidden files.
pub(crate) fn write_file(path: &Path, text: &str) -> Result<()> {
    static WRITES: AtomicU32 = AtomicU32::new(0);
    let dir = path.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
    fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    let name = path.file_name().with_context(|| format!("{} isn't a file", path.display()))?;
    let write = WRITES.fetch_add(1, Ordering::Relaxed);
    let tmp = dir.join(format!(".{}.{}-{write}.tmp", name.to_string_lossy(), std::process::id()));
    let written = fs::File::create(&tmp).and_then(|mut file| {
        file.write_all(text.as_bytes())?;
        file.sync_all()
    });
    if let Err(e) = written.and_then(|()| rename(&tmp, path)) {
        let _ = remove_file(&tmp);
        return Err(e).with_context(|| format!("writing {}", path.display()));
    }
    Ok(())
}

/// How long to wait before each new try at a file another program holds, 1.27 s in all. Tools that do the same wait
/// longer (rustup up to about 28 s for antiviruses, rust-lang/rustup#1870; Go's `cmd/internal/robustio` 2 s), but
/// the app writes on its UI thread, which a long wait would freeze.
const BUSY_DELAYS: [Duration; 7] = [
    Duration::from_millis(10),
    Duration::from_millis(20),
    Duration::from_millis(40),
    Duration::from_millis(80),
    Duration::from_millis(160),
    Duration::from_millis(320),
    Duration::from_millis(640),
];

/// The waits of [`retry_while_busy`] on this system. Only Windows has any: there a file another program has open
/// (an antivirus, the search indexer, an editor) can't be replaced or deleted, and the rename
/// or delete fails (see [`busy`]) until it lets go. Elsewhere it can, so a denied permission is a real one and fails
/// at once.
fn busy_delays() -> &'static [Duration] {
    if cfg!(windows) { &BUSY_DELAYS } else { &[] }
}

/// Runs `op`, and runs it again after each of `delays` while it fails as a file another program holds makes it fail
/// on Windows (see [`busy_delays`]). Any other error, or the last try's, is returned.
fn retry_while_busy<T>(delays: &[Duration], mut op: impl FnMut() -> std::io::Result<T>) -> std::io::Result<T> {
    let mut delays = delays.iter();
    loop {
        match op() {
            Err(e) if busy(&e) => match delays.next() {
                Some(delay) => std::thread::sleep(*delay),
                None => return Err(e),
            },
            result => return result,
        }
    }
}

/// Whether `error` may be a file another program holds: a denied permission (`ERROR_ACCESS_DENIED` on Windows), or on
/// Windows `ERROR_SHARING_VIOLATION`, which std leaves uncategorized (32 is another error elsewhere).
fn busy(error: &std::io::Error) -> bool {
    const ERROR_SHARING_VIOLATION: i32 = 32;
    error.kind() == ErrorKind::PermissionDenied
        || (cfg!(windows) && matches!(error.raw_os_error(), Some(ERROR_SHARING_VIOLATION)))
}

/// Deletes the file at `path`, waiting a little for a program holding it to let go (see [`retry_while_busy`]).
pub(crate) fn remove_file(path: &Path) -> std::io::Result<()> {
    retry_while_busy(busy_delays(), || fs::remove_file(path))
}

/// Renames `from` to `to`, replacing it, waiting a little for a program holding either to let go (see
/// [`retry_while_busy`]).
pub(crate) fn rename(from: &Path, to: &Path) -> std::io::Result<()> {
    retry_while_busy(busy_delays(), || fs::rename(from, to))
}

/// The name `task`'s file is saved under: `0005-fix-login.md`.
pub(crate) fn file_name(task: &Task) -> String {
    format!("{:04}-{}.md", task.id, slugify(&task.title))
}

/// Whether `name` starts as [`file_name`] names an entry: four digits or more, `-`, a lowercase ASCII letter (a slug's).
/// A slug starting with a digit is left out, so that a notes vault's `2026-10-15.md` isn't taken for one.
pub(crate) fn entry_named(name: &str) -> bool {
    let digits = name.chars().take_while(char::is_ascii_digit).count();
    digits >= 4
        && name[digits..].strip_prefix('-').is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_lowercase()))
}

/// Whether two file names in one folder are the same but for case, which a case-insensitive file system (macOS,
/// Windows) takes for one file.
fn same_ignoring_case(a: &Path, b: &Path) -> bool {
    a.file_name()
        .zip(b.file_name())
        .is_some_and(|(a, b)| a.to_string_lossy().eq_ignore_ascii_case(&b.to_string_lossy()))
}

/// The nearest `.brain/` folder in `start` or one of its parents.
pub fn find_project(start: &Path) -> Option<PathBuf> {
    start.ancestors().map(|dir| dir.join(PROJECT_DIR)).find(|dir| dir.is_dir())
}

/// Task files are the `.md` files in the folder, except the tag colors, the project's card and the
/// central brain's list of projects.
pub(crate) fn is_task_file(path: &Path) -> bool {
    let other = [TAGS_FILE, projects::PROJECT_FILE, projects::REGISTRY_FILE];
    path.extension().is_some_and(|e| e == "md") && path.file_name().is_some_and(|n| !other.iter().any(|o| n == *o))
}

#[cfg(test)]
#[path = "../tests/store.rs"]
mod tests;
