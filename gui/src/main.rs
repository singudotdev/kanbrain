//! kanbrain: the kanban board over the same markdown files as the `kb` command line. Given arguments, it is
//! that command line instead, so `kanbrain context` works wherever the app is installed.
//! What's on screen and every action is [`state`]; [`ui`] draws it with GPUI and turns input into it.
//! See `docs/development.md` for an overview of the modules.

// A desktop app: no console window on Windows, so the commands print nothing there: use `kb`.
#![cfg_attr(windows, windows_subsystem = "windows")]

mod state;
mod text;
mod ui;
mod watch;

use kanbrain_core::cli::Program;
use kanbrain_core::platform;
use kanbrain_core::{projects, store};

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // With arguments it's the command line: agent sections and hooks written as `kanbrain …` before the
    // command line was `kb` keep working wherever the app is installed.
    if !args.is_empty() {
        return Program { name: "kanbrain" }.main(args);
    }
    platform::ensure_not_root()?;
    // Opened in a project's repository, it starts on that project; it shows them all anyway.
    let here = std::env::current_dir()?;
    // Canonical, as `init` registers it.
    let opened_in = store::find_project(&here).and_then(|dir| dir.canonicalize().ok());
    ui::run(state::Board::open(&projects::central_dir()?, opened_in)?);
    Ok(())
}
