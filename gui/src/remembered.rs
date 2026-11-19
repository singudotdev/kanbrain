//! What the app remembers between sessions for whoever runs it, apart from the brains, which are shared: the
//! details' width. One file per value in [`folder`]. See `docs/decisions/0015-the-app-remembers-in-the-state-folder.md`.

use std::io;
use std::path::{Path, PathBuf};

/// The details' width, in pixels, as text.
const DRAWER_WIDTH: &str = "drawer-width";

/// `kanbrain-app` in the state folder (`$XDG_STATE_HOME`, `~/.local/state`), else in the local data folder (macOS
/// and Windows have no state folder). Not `kanbrain`: on macOS the local data folder is the data folder, whose
/// `kanbrain` is the central brain.
#[cfg(not(test))]
pub fn folder() -> Option<PathBuf> {
    dirs::state_dir().or_else(dirs::data_local_dir).map(|dir| dir.join("kanbrain-app"))
}

#[cfg(test)]
thread_local! {
    static FOLDER: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}

/// In tests, none, so nothing is remembered in the real one, unless the test names its own ([`remember_in`]).
#[cfg(test)]
pub fn folder() -> Option<PathBuf> {
    FOLDER.with_borrow(Clone::clone)
}

/// Makes `dir` this test's (this thread's) [`folder`].
#[cfg(test)]
pub fn remember_in(dir: &Path) {
    FOLDER.set(Some(dir.to_path_buf()));
}

/// The details' width remembered in `folder`: none unless finite and positive, as one written by hand may not be; one
/// too wide or narrow for the window is fitted to it when drawn, as a dragged one.
pub fn drawer_width(folder: &Path) -> Option<f32> {
    let text = std::fs::read_to_string(folder.join(DRAWER_WIDTH)).ok()?;
    text.trim().parse().ok().filter(|width: &f32| width.is_finite() && *width > 0.)
}

/// Remembers the details' `width` in `folder`, made if missing, through a temporary file renamed over the last: an
/// app stopped mid-write leaves the last width whole. Not synced to disk: a power cut can lose it, and the details
/// then open 420 px wide. Two at once would share the temporary file: the caller runs them one after another.
pub fn remember_drawer_width(folder: &Path, width: f32) -> io::Result<()> {
    std::fs::create_dir_all(folder)?;
    let temporary = folder.join(format!(".{DRAWER_WIDTH}.tmp"));
    std::fs::write(&temporary, width.to_string())?;
    std::fs::rename(temporary, folder.join(DRAWER_WIDTH))
}

#[cfg(test)]
#[path = "../tests/remembered.rs"]
mod tests;
