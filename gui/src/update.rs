//! Updating the app from itself: whether a newer release is out, and installing it in place of this one, with
//! `kb` beside it and every project's agent instructions, as `kb update` does. Neither while an agent works.

use anyhow::{Context, Result, bail};
use kanbrain_core::cli::update::{self, Install};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// How often a running app asks whether a newer release is out, after once at start.
pub const CHECK_EVERY: Duration = Duration::from_hours(24);

/// Whether the app asks GitHub for the latest release: not when `KANBRAIN_NO_UPDATE_CHECK` is set, nor in tests.
pub fn checks() -> bool {
    !cfg!(test) && std::env::var_os("KANBRAIN_NO_UPDATE_CHECK").is_none()
}

/// Where a failed update on Windows leaves its error, for the app it starts again to show: by then the one that
/// asked for it has closed.
fn failure_note() -> PathBuf {
    std::env::temp_dir().join("kanbrain-update-failed.txt")
}

/// The error a failed update on Windows left, taken so it's shown once. Not in tests, which would take the
/// user's own.
pub fn take_failure() -> Option<String> {
    if cfg!(test) {
        return None;
    }
    let path = failure_note();
    let text = std::fs::read_to_string(&path).ok()?;
    std::fs::remove_file(&path).ok();
    Some(format!("the update failed: {}", text.trim()))
}

/// Installs the latest release over this app and starts it, unless an agent is working on a task in a project
/// under `central`. Once it returns, this app is to close: on Windows, the installer waits for that to replace it.
pub fn install(central: &Path) -> Result<()> {
    update::check_no_agent_working(central)?;
    // Before installing: on Linux, the running program's path reads as deleted once it's replaced.
    let exe = std::env::current_exe().context("finding this program's file")?;
    let Install::Installer { dir, app } = update::install_of(&exe)? else {
        bail!("kanbrain was installed with cargo: update it the same way");
    };
    if cfg!(windows) { install_after_exit(&exe, &dir, app) } else { install_now(&exe, &dir, app) }
}

/// Runs the installer, which replaces the binaries by renaming, so this one carries on, then starts the new one.
fn install_now(exe: &Path, dir: &Path, app: &str) -> Result<()> {
    update::run_installer(dir, app, true)?;
    Command::new(exe).spawn().with_context(|| format!("starting {}", exe.display()))?;
    Ok(())
}

/// Leaves the installer to PowerShell, once this app has exited, since Windows won't replace a running program's
/// file; it starts the app again whether it worked or not, with the error noted for it.
fn install_after_exit(exe: &Path, dir: &Path, app: &str) -> Result<()> {
    // In single quotes, where only a single quote is special, written twice.
    let quote = |path: &Path| format!("'{}'", path.display().to_string().replace('\'', "''"));
    let script = format!(
        "try {{ Wait-Process -Id {pid} -ErrorAction SilentlyContinue; $env:KANBRAIN_APP = '{app}'; \
         $env:KANBRAIN_INSTALL_DIR = {dir}; irm {url} | iex }} catch {{ Set-Content -LiteralPath {note} -Value \
         \"$_\" }} finally {{ Start-Process -FilePath {exe} }}",
        pid = std::process::id(),
        dir = quote(dir),
        url = update::INSTALL_PS1,
        note = quote(&failure_note()),
        exe = quote(exe),
    );
    let mut powershell = Command::new("powershell");
    powershell.args(["-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-Command", &script]);
    update::hide_console(&mut powershell);
    powershell.spawn().context("starting PowerShell, which runs the installer")?;
    Ok(())
}
