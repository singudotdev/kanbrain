//! `kb update`: installs the latest release over this one, by running the installer the README gives,
//! which downloads it, checks its checksum and replaces the binaries, then runs the new version's `init --all`,
//! so none of that is repeated here.

use crate::model::{Author, Status};
use crate::projects;
use crate::store::Store;
use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The installer for Linux and macOS.
const INSTALL_SH: &str = "https://raw.githubusercontent.com/singudotdev/kanbrain/main/install.sh";

/// The installer for Windows.
const INSTALL_PS1: &str = "https://raw.githubusercontent.com/singudotdev/kanbrain/main/install.ps1";

/// Which apps are installed in `dir`, as the installer's `--app` names them: the one running is, and
/// the other is updated along with it when it's there too.
fn installed_app(dir: &Path) -> &'static str {
    let has = |name: &str| dir.join(format!("{name}{}", std::env::consts::EXE_SUFFIX)).is_file();
    match (has("kb"), has("kanbrain")) {
        (true, true) => "both",
        (false, true) => "gui",
        _ => "cli",
    }
}

/// Whether `dir` is cargo's folder of installed programs (`$CARGO_HOME/bin`, else `~/.cargo/bin`): a program
/// there came from `cargo install`, which keeps its own record of what it installed.
fn installed_by_cargo(dir: &Path, cargo_home: Option<PathBuf>) -> bool {
    let Some(home) = cargo_home.or_else(|| dirs::home_dir().map(|home| home.join(".cargo"))) else { return false };
    let canonical = |path: &Path| path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    canonical(dir) == canonical(&home.join("bin"))
}

/// The tasks agents are working on, in doing, in every registered repository and the central brain, as
/// `REPO#ID title`: an update waits for them. A brain that can't be read, such as one a newer kanbrain wrote, has
/// none to wait for.
pub fn agents_working(central: &Path) -> Result<Vec<String>> {
    let repos = projects::load(central)?;
    let brains = std::iter::once((String::new(), central.to_path_buf()))
        .chain(repos.iter().map(|repo| (repo.label(&repos), repo.dir.clone())));
    let mut working = Vec::new();
    for (label, dir) in brains {
        let Ok((tasks, _)) = Store::at(dir).and_then(|store| store.load()) else { continue };
        working.extend(
            tasks
                .iter()
                .filter(|t| t.is_task() && t.status == Status::Doing && t.started_by() == Author::Agent)
                .map(|t| format!("{label}#{} {}", t.id, t.title)),
        );
    }
    Ok(working)
}

/// Refuses while [`agents_working`] finds any, naming them.
pub fn check_no_agent_working(central: &Path) -> Result<()> {
    let working = agents_working(central)?;
    if !working.is_empty() {
        bail!(
            "agents are working on {}: update once they're done or parked, so their instructions and data don't \
             change under them",
            working.join(", ")
        );
    }
    Ok(())
}

/// Runs the installer for the apps installed where this program is, into the same folder, unless an agent is
/// working. On Windows it
/// prints the PowerShell command instead, since a running program's file can't be replaced there. Installed with
/// `cargo install`, it says to update the same way instead: the installer would replace the file behind cargo's back.
pub fn update(central: &Path) -> Result<()> {
    check_no_agent_working(central)?;
    let exe = std::env::current_exe().context("finding this program's file")?;
    let dir = exe.parent().context("finding this program's folder")?;
    if installed_by_cargo(dir, std::env::var_os("CARGO_HOME").map(PathBuf::from)) {
        println!(
            "kb was installed with cargo: update it, and then every project's agent instructions, with\n\n  \
             cargo install kanbrain && kb init --all"
        );
        return Ok(());
    }
    let app = installed_app(dir);
    if cfg!(windows) {
        println!(
            "Close kanbrain, then run in PowerShell:\n\n  $env:KANBRAIN_APP = \"{app}\"; \
             $env:KANBRAIN_INSTALL_DIR = \"{}\"; irm {INSTALL_PS1} | iex",
            dir.display()
        );
        return Ok(());
    }
    // The script goes from curl to sh through a pipe, and the folder as an argument of its own, so
    // nothing here is ever read by a shell as code.
    let mut curl = Command::new("curl")
        .args(["-fsSL", "--", INSTALL_SH])
        .stdout(Stdio::piped())
        .spawn()
        .context("running curl, which downloads the installer")?;
    let script = curl.stdout.take().context("reading curl's output")?;
    let installed = Command::new("sh")
        .args(["-s", "--", "--app", app, "--dir"])
        .arg(dir)
        .stdin(script)
        .status()
        .context("running the installer")?;
    let downloaded = curl.wait().context("waiting for curl")?;
    if !downloaded.success() {
        bail!("couldn't download the installer from {INSTALL_SH}");
    }
    if !installed.success() {
        bail!("the installer failed (its messages are above)");
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../tests/cli/update.rs"]
mod tests;
