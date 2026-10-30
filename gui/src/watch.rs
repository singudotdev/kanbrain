//! Noticing changes other programs make to the data folder, such as an agent running `kanbrain
//! start 3`, so the board follows them. It polls instead of subscribing to file system events: a
//! listing every second works the same on every platform and on network drives, where events are
//! unreliable, and it runs away from the window (see `ui::BrainView`), as listing ten thousand
//! entries takes milliseconds.

use std::cmp::Ordering;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// How often the folder is listed.
pub const INTERVAL: Duration = Duration::from_secs(1);

/// How long after a folder last changed its listing can't be trusted to hold until it changes again, on a file system
/// that keeps times to the second or two (FAT): a change made in the tick of a listing can leave the folder's time as
/// it was. Within it, the folder is listed again.
const TICK: Duration = Duration::from_secs(2);

/// [`TICK`] on a file system keeping times to the nanosecond, which the kernel advances by its clock's tick, a few
/// milliseconds.
const FINE_TICK: Duration = Duration::from_millis(50);

/// The markdown files of a folder (and its `format` file) by name, with when they last changed and their size.
type Snapshot = Vec<(String, Option<SystemTime>, u64)>;

/// A folder as listed: its files, when the folder itself last changed, and when it was listed.
#[derive(Clone, Default)]
pub struct Listing {
    /// Its files.
    files: Snapshot,
    /// When an entry of it was last added, removed or renamed: a save writes a temporary file and renames it, so
    /// every save `kb` and the app make changes it.
    changed: Option<SystemTime>,
    /// When it was listed.
    at: Option<SystemTime>,
}

/// The files of one folder changed, added or deleted since it was last looked at.
#[derive(Debug, PartialEq, Eq)]
pub struct Changes {
    /// The folder.
    pub dir: PathBuf,
    /// Its files that changed, by their paths.
    pub files: Vec<PathBuf>,
}

/// Watches data folders by comparing listings of them.
pub struct Watcher {
    /// The folders.
    dirs: Vec<PathBuf>,
    /// Their listings when last looked at, in the same order.
    last: Vec<Listing>,
    /// Listings the timer took that differ from those, for [`Watcher::changed`] to take rather than list again.
    noticed: Vec<Option<Listing>>,
}

impl Watcher {
    /// Watches `dirs`, none of them seen yet: one with files reads as changed at the first look, since the board
    /// was read before the watcher started (before the window opened), and a write in between would be missed.
    pub fn new(dirs: Vec<PathBuf>) -> Self {
        let last = vec![Listing::default(); dirs.len()];
        let noticed = vec![None; dirs.len()];
        Self { dirs, last, noticed }
    }

    /// The folders watched, for [`list`] to list away from the window's thread.
    pub fn dirs(&self) -> &[PathBuf] {
        &self.dirs
    }

    /// Whether `listings` of `dirs`, as [`list`] took them, show a change since the folders were last looked at,
    /// leaving it for [`Self::changed`] to take: from these listings while their folders' times say they still hold,
    /// else from listing them again, never by the folders' times alone, which a file written over in place leaves
    /// as they were. Folders watched since, or no longer, are left to it too.
    pub fn noticed(&mut self, dirs: &[PathBuf], listings: Vec<Listing>) -> bool {
        let mut noticed = false;
        for (dir, now) in dirs.iter().zip(listings) {
            if let Some(at) = self.dirs.iter().position(|d| d == dir)
                && self.last[at].files != now.files
            {
                self.last[at].at = None;
                self.noticed[at] = Some(now);
                noticed = true;
            }
        }
        noticed
    }

    /// The files changed in each folder since it was last looked at, taking their state as seen. A folder that hasn't
    /// changed since a tick after it was listed isn't listed again: a key pressed in a long brain took that long.
    /// Not a file written over in place, as some editors do, which only [`list`] finds, every [`INTERVAL`] (see
    /// [`Self::noticed`]): the store refuses to save over such a file meanwhile.
    pub fn changed(&mut self) -> Vec<Changes> {
        let mut changes = Vec::new();
        for ((dir, last), noticed) in self.dirs.iter().zip(&mut self.last).zip(&mut self.noticed) {
            let now = match noticed.take() {
                Some(noticed) if unchanged(dir, &noticed) => noticed,
                _ if unchanged(dir, last) => continue,
                _ => listing(dir),
            };
            let files = differences(&last.files, &now.files);
            if !files.is_empty() {
                changes.push(Changes { dir: dir.clone(), files: files.iter().map(|name| dir.join(name)).collect() });
            }
            *last = now;
        }
        changes
    }

    /// Watches `dirs` from now on, taking the files of those already watched as seen where they are as `known`
    /// says the board read or wrote them (`None` for none, or deleted): after the board wrote to them itself, so
    /// its own saves don't read as outside changes and reload it for nothing, and after a reload, which may have
    /// found projects added or gone. Any other change reads as one at the next look: a file another program wrote
    /// over in place, which the last look passed over by the folder's time, would otherwise be taken as read. A
    /// folder new to it reads as changed unless empty, since the board read it without holding its lock: it's read
    /// again, under the locks, next time.
    pub fn sync(&mut self, dirs: Vec<PathBuf>, known: impl Fn(&Path) -> Option<(Option<SystemTime>, u64)>) {
        let last = dirs
            .iter()
            .map(|dir| match self.dirs.iter().position(|d| d == dir) {
                // Not written to since it was listed, by the folder's time: a write of another program in that tick
                // reads as a change at the next look, rather than as seen.
                Some(at) if folder_time(dir) == self.last[at].changed => std::mem::take(&mut self.last[at]),
                Some(at) => own(dir, &self.last[at], listing(dir), &known),
                None => Listing::default(),
            })
            .collect();
        let noticed = vec![None; dirs.len()];
        *self = Self { dirs, last, noticed };
    }
}

/// Lists `dirs`: what the window's timer has done away from its thread, as listing a long brain takes a while.
pub fn list(dirs: &[PathBuf]) -> Vec<Listing> {
    dirs.iter().map(|dir| listing(dir)).collect()
}

/// `now`, but with each file that differs from `was` and isn't as the board read or wrote it (`known`) as `was` had
/// it, and then to be listed again.
fn own(
    dir: &Path,
    was: &Listing,
    mut now: Listing,
    known: impl Fn(&Path) -> Option<(Option<SystemTime>, u64)>,
) -> Listing {
    let state = |files: &Snapshot, name: &str| {
        files.binary_search_by(|file| file.0.as_str().cmp(name)).ok().map(|at| (files[at].1, files[at].2))
    };
    let names: Vec<String> = differences(&was.files, &now.files).into_iter().map(str::to_owned).collect();
    for name in names {
        let then = state(&was.files, &name);
        if known(&dir.join(&name)) == state(&now.files, &name) {
            continue;
        }
        match (now.files.binary_search_by(|file| file.0.cmp(&name)), then) {
            (Ok(at), Some((time, len))) => now.files[at] = (name, time, len),
            (Ok(at), None) => {
                now.files.remove(at);
            }
            (Err(at), Some((time, len))) => now.files.insert(at, (name, time, len)),
            (Err(_), None) => {}
        }
        now.at = None;
    }
    now
}

/// Whether the folder's entries are as `last` listed them, by the folder's time alone: it hasn't changed since, and
/// was listed a tick after it last did.
fn unchanged(dir: &Path, last: &Listing) -> bool {
    let (Some(changed), Some(at)) = (last.changed, last.at) else { return false };
    // Kept to the second or two, a time lands on a whole second; kept finer, it does only once in a while, and is
    // then taken as coarse, which lists the folder again.
    let coarse = changed.duration_since(SystemTime::UNIX_EPOCH).is_ok_and(|since| since.subsec_nanos() == 0);
    let tick = if coarse { TICK } else { FINE_TICK };
    folder_time(dir) == Some(changed) && at.duration_since(changed).is_ok_and(|since| since > tick)
}

/// When an entry of the folder was last added, removed or renamed.
fn folder_time(dir: &Path) -> Option<SystemTime> {
    fs::metadata(dir).and_then(|meta| meta.modified()).ok()
}

/// The folder listed now.
fn listing(dir: &Path) -> Listing {
    // Its time before its files: changed in between, it reads as changed at the next look.
    let changed = folder_time(dir);
    let at = Some(SystemTime::now());
    Listing { files: snapshot(dir), changed, at }
}

/// The names of the files in one listing and not the other, or in both but changed: both sorted by name.
fn differences<'a>(was: &'a Snapshot, now: &'a Snapshot) -> Vec<&'a str> {
    // Walked by iterators, not by indices: each step takes a file off one or both, so the walk ends.
    let (mut was, mut now) = (was.iter().peekable(), now.iter().peekable());
    let mut names = Vec::new();
    loop {
        let order = match (was.peek(), now.peek()) {
            (Some(a), Some(b)) => a.0.cmp(&b.0),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => return names,
        };
        match order {
            Ordering::Equal => {
                if let (Some(a), Some(b)) = (was.next(), now.next())
                    && a != b
                {
                    names.push(b.0.as_str());
                }
            }
            // Gone: before the next one listed now, or after the last.
            Ordering::Less => names.extend(was.next().map(|a| a.0.as_str())),
            // Added: before the next one listed then, or after the last.
            Ordering::Greater => names.extend(now.next().map(|b| b.0.as_str())),
        }
    }
}

/// The folder's markdown files and its `format` file, sorted by name: a `format` file repaired lets a repository
/// left out for it be read again. Hidden files are left out since saves write a hidden temporary file first and
/// rename it. An unreadable folder lists as empty, so it reads as a change
/// once it's back.
fn snapshot(dir: &Path) -> Snapshot {
    let Ok(entries) = fs::read_dir(dir) else { return Vec::new() };
    let mut files: Snapshot = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let markdown = Path::new(&name).extension().is_some_and(|ext| ext.eq_ignore_ascii_case("md"));
            if name.starts_with('.') || !(markdown || name == kanbrain_core::migrate::FORMAT_FILE) {
                return None;
            }
            let meta = entry.metadata().ok()?;
            Some((name, meta.modified().ok(), meta.len()))
        })
        .collect();
    files.sort_unstable();
    files
}

#[cfg(test)]
#[path = "../tests/watch.rs"]
mod tests;
