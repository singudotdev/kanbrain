//! kanbrain: the kanban board over the same markdown files as the `kb` command line. Given arguments, it is
//! that command line instead, so `kanbrain context` works wherever the app is installed.
//! What's on screen and every action is [`state`]; [`ui`] draws it with GPUI and turns input into it.
//! See `docs/development.md` for an overview of the modules.

// A desktop app: no console window on Windows, so the commands print nothing there: use `kb`. An error opening the
// board is shown in a window (`ui::startup`).
#![cfg_attr(windows, windows_subsystem = "windows")]

mod state;
mod text;
mod ui;
mod update;
mod watch;

use kanbrain_core::cli::Program;
use kanbrain_core::platform;
use kanbrain_core::{projects, store};

fn main() -> anyhow::Result<()> {
    start(std::env::args().skip(1).collect(), open, |e| ui::startup::fail(e))
}

/// The command line given `args`, its errors on stderr only; else the app, opened by `open`, whose failure to open
/// the board `failed` shows as well. Started from a menu, Finder or a desktop's launcher, the app has no console to
/// print to (on Windows none at all): the failure would go unseen.
fn start(
    args: Vec<String>,
    open: impl FnOnce() -> anyhow::Result<()>,
    failed: impl FnOnce(&anyhow::Error),
) -> anyhow::Result<()> {
    // With arguments it's the command line, so `kanbrain` takes `kb`'s commands where only the app is installed.
    if !args.is_empty() {
        return Program { name: "kanbrain" }.main(args);
    }
    open().inspect_err(failed)
}

/// Opens the board in the window, and runs until it's closed.
fn open() -> anyhow::Result<()> {
    platform::ensure_not_root()?;
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    ui::display::ensure_display()?;
    // Opened in a project's repository, it starts on that project; it shows them all anyway.
    let here = std::env::current_dir()?;
    // Canonical, as `init` registers it.
    let opened_in = store::find_project(&here).and_then(|dir| projects::canonical(&dir).ok());
    ui::run(state::Board::open(&projects::central_dir()?, opened_in)?);
    Ok(())
}

#[cfg(test)]
#[path = "../tests/main.rs"]
mod tests;
