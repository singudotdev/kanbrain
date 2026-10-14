//! Noticing changes other programs make to the data folder, such as an agent running `kanbrain
//! start 3`, so the board follows them. It polls instead of subscribing to file system events: a
//! listing of a few hundred small files every second is cheap, and works the same on every platform
//! and on network drives, where events are unreliable.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// How often the folder is listed.
pub const INTERVAL: Duration = Duration::from_secs(1);

/// The markdown files of a folder (and its `format` file) by name, with when they last changed and their size.
type Snapshot = Vec<(String, Option<SystemTime>, u64)>;

/// Watches data folders by comparing listings of them.
pub struct Watcher {
    /// The folders.
    dirs: Vec<PathBuf>,
    /// Their listings when last looked at, in the same order.
    last: Vec<Snapshot>,
}

impl Watcher {
    /// Watches `dirs`, none of them seen yet: one with files reads as changed at the first look, since the board
    /// was read before the watcher started (before the window opened), and a write in between would be missed.
    pub fn new(dirs: Vec<PathBuf>) -> Self {
        let last = vec![Snapshot::new(); dirs.len()];
        Self { dirs, last }
    }

    /// Whether any folder changed since it was last looked at, leaving that for [`Self::changed`] to take.
    pub fn dirty(&self) -> bool {
        self.dirs.iter().zip(&self.last).any(|(dir, last)| snapshot(dir) != *last)
    }

    /// The folders that changed since they were last looked at; either way, takes their state as seen.
    pub fn changed(&mut self) -> Vec<PathBuf> {
        let mut changed = Vec::new();
        for (dir, last) in self.dirs.iter().zip(&mut self.last) {
            let now = snapshot(dir);
            if now != *last {
                changed.push(dir.clone());
                *last = now;
            }
        }
        changed
    }

    /// Watches `dirs` from now on, taking the state of those already watched as seen without asking: after the
    /// board wrote to them itself, so its own saves don't read as outside changes and reload it for nothing, and
    /// after a reload, which may have found projects added or gone. A folder new to it reads as changed unless
    /// empty, since the board read it without holding its lock: it's read again, under the locks, next time.
    pub fn sync(&mut self, dirs: Vec<PathBuf>) {
        let last = dirs.iter().map(|d| if self.dirs.contains(d) { snapshot(d) } else { Snapshot::new() }).collect();
        *self = Self { dirs, last };
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
