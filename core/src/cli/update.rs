//! `kb update`, and the app's update: installs the latest release over this one, by running the installer the
//! README gives, which downloads it, checks its checksum and replaces the binaries, then runs the new version's
//! `init --all`, so none of that is repeated here. Never while an agent works on a task: `init --all` would rewrite
//! its instructions and a migration its data mid-session.

use super::Program;
use crate::model::Author;
use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The installer for Linux and macOS, as the latest release has it: `main`'s may already expect a release that
/// isn't out.
const INSTALL_SH: &str = "https://github.com/singudotdev/kanbrain/releases/latest/download/install.sh";

/// The installer for Windows, likewise.
pub const INSTALL_PS1: &str = "https://github.com/singudotdev/kanbrain/releases/latest/download/install.ps1";

/// Redirects to the latest release's page, whose address ends in its tag. A redirect, not GitHub's API, which
/// limits how often an address may ask.
const LATEST: &str = "https://github.com/singudotdev/kanbrain/releases/latest";

/// Where the installer's macOS app bundle says the binaries are, relative to its executable: the bundle's is a
/// link to the app in the install folder, which is the one to replace.
const BUNDLE_INSTALL_DIR: &str = "../Resources/install-dir";

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

/// How the running program was installed, which says how to update it.
#[derive(Debug, PartialEq, Eq)]
pub enum Install {
    /// With `cargo install`, which keeps its own record of what it installed: the installer would replace the
    /// file behind cargo's back.
    Cargo,
    /// By the installer, into `dir`, with `app` (its `--app`) there.
    Installer {
        /// The install folder.
        dir: PathBuf,
        /// `cli`, `gui` or `both`.
        app: &'static str,
    },
}

/// How the program at `exe` was installed.
pub fn install_of(exe: &Path) -> Result<Install> {
    let dir = exe.parent().context("finding this program's folder")?;
    if installed_by_cargo(dir, std::env::var_os("CARGO_HOME").map(PathBuf::from)) {
        return Ok(Install::Cargo);
    }
    let dir = match std::fs::read_to_string(dir.join(BUNDLE_INSTALL_DIR)) {
        Ok(text) => PathBuf::from(text.trim_end()),
        Err(_) => dir.to_path_buf(),
    };
    Ok(Install::Installer { app: installed_app(&dir), dir })
}

/// The tasks agents are working on, in doing, in every registered repository and the central brain, as
/// `shop/api #3 title` (`#3 title` in the central brain), as `kb status` names them: an update waits for them. A
/// brain that can't be read, such as one a newer kanbrain wrote, has none to wait for.
pub fn agents_working(central: &Path) -> Result<Vec<String>> {
    let mut working = Vec::new();
    for (label, tasks) in super::every_brain(central)? {
        working.extend(
            tasks.iter().filter(|t| t.doing_by(Author::Agent)).map(|t| format!("{label}#{} {}", t.id, t.title)),
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

/// The latest release's version, `X.Y.Z`, when it's newer than this one. Asks GitHub, through curl, for where its
/// latest release is.
pub fn newer_release() -> Result<Option<String>> {
    let mut curl = Command::new("curl");
    curl.args(["-fsSI", "--max-time", "20", "--", LATEST]);
    hide_console(&mut curl);
    let out = curl.output().context("running curl, which asks for the latest release")?;
    if !out.status.success() {
        bail!("couldn't ask {LATEST} for the latest release");
    }
    let latest = redirected_version(&String::from_utf8_lossy(&out.stdout))
        .context("the latest release's address has no version")?;
    Ok(newer(&latest, env!("CARGO_PKG_VERSION")).then_some(latest))
}

/// What `program`'s `update` says instead of reinstalling, when the latest release is this one (or older, a build
/// ahead of it); `None` to install: a newer release, or none found (the installer then says why it can't download).
fn up_to_date(newer: &Result<Option<String>>, program: Program) -> Option<String> {
    matches!(newer, Ok(None))
        .then(|| format!("{} {} is the latest release: nothing to update", program.name, env!("CARGO_PKG_VERSION")))
}

/// What `program`'s `update` says when cargo installed it, which keeps its own record of what it installed: the
/// command that updates it, [`cargo_update_command`], and then the projects.
fn cargo_update(program: Program, release: Option<&str>) -> String {
    format!(
        "{} was installed with cargo: update it, and then every project's agent instructions, with\n\n  {} && {} init \
         --all",
        program.name,
        cargo_update_command(program, release),
        program.name
    )
}

/// The `cargo install` that updates `program` to `release` (`X.Y.Z`), or to the latest when it isn't known. The
/// crate `kanbrain` on crates.io is the command line, whose latest cargo installs; the kanban app's, `kanbrain-app`,
/// isn't there (GPUI comes from git), so it comes from the repository, at the release's tag, else at `main`, which is
/// the latest release. The app is told apart by its name, `kanbrain`: [`Program`] says nothing else about it, and a
/// field it gained would break code building one.
pub fn cargo_update_command(program: Program, release: Option<&str>) -> String {
    if program.name != "kanbrain" {
        return "cargo install kanbrain".to_string();
    }
    let tag = release.map(|version| format!(" --tag v{version}")).unwrap_or_default();
    format!("cargo install --locked --git https://github.com/singudotdev/kanbrain{tag} kanbrain-app")
}

/// The version at the end of the address a response's headers redirect to, `…/tag/vX.Y.Z`.
fn redirected_version(headers: &str) -> Option<String> {
    let location = headers.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.eq_ignore_ascii_case("location").then(|| value.trim())
    })?;
    location.rsplit_once("/tag/v").map(|(_, version)| version.to_string())
}

/// Whether version `a` comes after `b`, both `X.Y.Z`; a part that isn't a number counts as 0.
fn newer(a: &str, b: &str) -> bool {
    let parts = |v: &str| -> Vec<u64> { v.split('.').map(|p| p.parse().unwrap_or(0)).collect() };
    parts(a) > parts(b)
}

/// Keeps a console window from flashing up when the app, which has none, runs a program on Windows.
pub fn hide_console(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    let _ = command;
}

/// The installer's variables that say to do something else than install the latest release: left set in a shell
/// from an uninstall or a pinned install, they'd have an update uninstall, purge without asking, or reinstall
/// that version instead. An update runs the installer without them.
pub(crate) const INSTALLER_CHOICES: [&str; 4] =
    ["KANBRAIN_UNINSTALL", "KANBRAIN_PURGE", "KANBRAIN_YES", "KANBRAIN_VERSION"];

/// PowerShell that clears [`INSTALLER_CHOICES`] in its session, to run before the installer there.
pub fn clear_installer_choices_ps1() -> String {
    let vars: Vec<String> = INSTALLER_CHOICES.iter().map(|v| format!("Env:{v}")).collect();
    format!("Remove-Item {} -ErrorAction SilentlyContinue", vars.join(", "))
}

/// `text` as a PowerShell single-quoted string literal, which PowerShell reads back to exactly `text`, whatever its
/// characters: for a folder or file in a command or script made for PowerShell. Nothing expands in single quotes,
/// neither `$` nor a backtick; only a single quote is special, written twice, and PowerShell takes the typographic
/// ones (`‘ ’ ‚ ‛`) for one too.
pub fn powershell_string(text: &str) -> String {
    let mut quoted = String::with_capacity(text.len() + 2);
    quoted.push('\'');
    for c in text.chars() {
        if matches!(c, '\'' | '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}') {
            quoted.push(c);
        }
        quoted.push(c);
    }
    quoted.push('\'');
    quoted
}

/// The PowerShell command that installs the latest release of `app` into `dir`, on Windows, where a running
/// program's file can't be replaced, so `update` prints it, to run once the program and the app are closed.
fn windows_update_command(dir: &Path, app: &str) -> String {
    format!(
        "{}; $env:KANBRAIN_APP = {}; $env:KANBRAIN_INSTALL_DIR = {}; irm {INSTALL_PS1} | iex",
        clear_installer_choices_ps1(),
        powershell_string(app),
        powershell_string(&dir.display().to_string())
    )
}

/// The shell that runs the installer script given on its stdin, for `app` into `dir`, without
/// [`INSTALLER_CHOICES`].
fn installer_shell(dir: &Path, app: &str) -> Command {
    let mut sh = Command::new("sh");
    sh.args(["-s", "--", "--app", app, "--dir"]).arg(dir);
    for var in INSTALLER_CHOICES {
        sh.env_remove(var);
    }
    sh
}

/// Runs the installer for `app` into `dir`, on Linux and macOS: the binaries are replaced by renaming, so the
/// running ones carry on. `captured`, its messages are kept out of sight and the last of them make up the error
/// when it fails, as the app wants; otherwise they're printed.
pub fn run_installer(dir: &Path, app: &str, captured: bool) -> Result<()> {
    // The script goes from curl to sh through a pipe, and the folder as an argument of its own, so
    // nothing here is ever read by a shell as code.
    let mut curl = Command::new("curl")
        .args(["-fsSL", "--", INSTALL_SH])
        .stdout(Stdio::piped())
        .spawn()
        .context("running curl, which downloads the installer")?;
    let script = curl.stdout.take().context("reading curl's output")?;
    let mut sh = installer_shell(dir, app);
    sh.stdin(script);
    let (installed, said) = if captured {
        let out = sh.output().context("running the installer")?;
        let said = String::from_utf8_lossy(&out.stderr).trim().lines().last().unwrap_or_default().to_string();
        (out.status, said)
    } else {
        (sh.status().context("running the installer")?, "its messages are above".to_string())
    };
    let downloaded = curl.wait().context("waiting for curl")?;
    if !downloaded.success() {
        bail!("couldn't download the installer from {INSTALL_SH}");
    }
    if !installed.success() {
        bail!("the installer failed ({said})");
    }
    Ok(())
}

/// [`update_program`] for `kb`, which this always names.
#[deprecated(since = "0.6.3", note = "use `update_program`, which names the program running in what it says")]
pub fn update(central: &Path) -> Result<()> {
    update_program(central, Program { name: "kb" })
}

/// `program`'s `update`: runs the installer for the apps installed where this program is, into the same folder,
/// unless an agent is working. On Windows it prints the PowerShell command instead, since a running program's file
/// can't be replaced there. Installed with `cargo install`, it says how cargo updates it instead.
pub fn update_program(central: &Path, program: Program) -> Result<()> {
    check_no_agent_working(central)?;
    let newer = newer_release();
    if let Some(said) = up_to_date(&newer, program) {
        println!("{said}");
        return Ok(());
    }
    let exe = std::env::current_exe().context("finding this program's file")?;
    let Install::Installer { dir, app } = install_of(&exe)? else {
        println!("{}", cargo_update(program, newer.ok().flatten().as_deref()));
        return Ok(());
    };
    if cfg!(windows) {
        println!("Close kanbrain, then run in PowerShell:\n\n  {}", windows_update_command(&dir, app));
        return Ok(());
    }
    run_installer(&dir, app, false)
}

#[cfg(test)]
#[path = "../../tests/cli/update.rs"]
mod tests;
