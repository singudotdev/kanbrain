use super::*;

#[test]
fn the_apps_found_beside_this_one_are_updated() {
    let dir = tempfile::tempdir().unwrap();
    let put =
        |name: &str| std::fs::write(dir.path().join(format!("{name}{}", std::env::consts::EXE_SUFFIX)), "").unwrap();
    put("kb");
    assert_eq!(installed_app(dir.path()), "cli");
    put("kanbrain");
    assert_eq!(installed_app(dir.path()), "both");
}

#[test]
fn a_program_cargo_installed_is_left_to_cargo() {
    let home = tempfile::tempdir().unwrap();
    let bin = home.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    assert!(installed_by_cargo(&bin, Some(home.path().to_path_buf())), "$CARGO_HOME/bin");
    assert!(!installed_by_cargo(home.path(), Some(home.path().to_path_buf())), "another folder");
}

#[test]
fn a_release_is_newer_by_its_numbers_not_its_text() {
    assert!(newer("0.3.10", "0.3.9"));
    assert!(newer("1.0.0", "0.9.9"));
    assert!(!newer("0.3.1", "0.3.1"));
    assert!(!newer("0.3.0", "0.3.1"));
    let headers = "HTTP/2 302\r\nLocation: https://github.com/singudotdev/kanbrain/releases/tag/v0.3.2\r\n\r\n";
    assert_eq!(redirected_version(headers).as_deref(), Some("0.3.2"));
    assert_eq!(redirected_version("HTTP/2 404\r\n\r\n"), None);
}

#[test]
fn the_macos_bundle_updates_the_folder_it_links_to() {
    let home = tempfile::tempdir().unwrap();
    let (bin, bundle) = (home.path().join("bin"), home.path().join("kanbrain.app/Contents"));
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::create_dir_all(bundle.join("MacOS")).unwrap();
    std::fs::create_dir_all(bundle.join("Resources")).unwrap();
    std::fs::write(bin.join(format!("kanbrain{}", std::env::consts::EXE_SUFFIX)), "").unwrap();
    let exe = bundle.join("MacOS/kanbrain");
    std::fs::write(bundle.join("Resources/install-dir"), format!("{}\n", bin.display())).unwrap();
    assert_eq!(install_of(&exe).unwrap(), Install::Installer { dir: bin, app: "gui" });
}

#[test]
fn an_update_waits_for_the_agents_tasks_in_doing_only() {
    use crate::brain::Brain;
    use crate::brain::NewEntry;
    use crate::model::{Author, Status};
    let (central, repo) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let brain_dir = repo.path().join(".brain");
    crate::projects::register(central.path(), "shop", &brain_dir).unwrap();
    let mut brain = Brain::open(Store::at(&brain_dir).unwrap()).unwrap();
    let mine = brain.add(NewEntry { title: "my own work".into(), ..NewEntry::default() }).unwrap();
    brain.set_status(mine, Status::Doing).unwrap();
    brain.author = Author::Agent;
    let parked = brain.add(NewEntry { title: "parked by the agent".into(), ..NewEntry::default() }).unwrap();
    brain.set_status(parked, Status::Doing).unwrap();
    brain.set_status(parked, Status::Parked).unwrap();
    assert!(check_no_agent_working(central.path()).is_ok(), "a person's task, and an agent's parked one");

    let working = brain.add(NewEntry { title: "rate-limit the login".into(), ..NewEntry::default() }).unwrap();
    brain.set_status(working, Status::Doing).unwrap();
    // `kb update` itself says so, before it looks for a release.
    let err = update(central.path()).unwrap_err().to_string();
    assert!(err.contains(&format!("shop #{working} rate-limit the login")), "as kb status names it: {err}");
    assert!(!err.contains("my own work"), "{err}");

    std::fs::remove_dir_all(repo.path()).unwrap();
    assert!(check_no_agent_working(central.path()).is_ok(), "a deleted repository has no agent at work");
    assert!(!repo.path().exists(), "nor is it made again");
}

#[test]
fn an_update_runs_the_installer_without_the_choices_left_from_an_uninstall() {
    let sh = installer_shell(Path::new("/home/me/.local/bin"), "both");
    let removed: Vec<_> = sh.get_envs().filter(|(_, value)| value.is_none()).map(|(name, _)| name).collect();
    for var in INSTALLER_CHOICES {
        assert!(removed.contains(&std::ffi::OsStr::new(var)), "{var} is passed on");
    }
    let ps1 = clear_installer_choices_ps1();
    assert!(INSTALLER_CHOICES.iter().all(|var| ps1.contains(&format!("Env:{var}"))), "{ps1}");
}

#[test]
fn an_update_to_the_release_already_installed_installs_nothing() {
    let said = up_to_date(&Ok(None)).expect("nothing newer");
    assert_eq!(said, format!("kb {} is the latest release: nothing to update", env!("CARGO_PKG_VERSION")));
    assert_eq!(up_to_date(&Ok(Some("9.0.0".into()))), None, "a newer one is installed");
    assert_eq!(up_to_date(&Err(anyhow::anyhow!("offline"))), None, "the installer says why it can't download");
}

/// A Windows folder with every character special in PowerShell's strings: `$` and a backtick, which a double-quoted
/// string expands, and the quotes, plain and typographic, which PowerShell reads as the end of a single-quoted one.
const AWKWARD_FOLDER: &str = r"C:\Users\Zoë O'Neil\my $HOME`s ‘bin’ ‚x‛";

#[test]
fn a_folder_goes_to_powershell_as_a_literal_whatever_its_characters() {
    assert_eq!(powershell_string(AWKWARD_FOLDER), r"'C:\Users\Zoë O''Neil\my $HOME`s ‘‘bin’’ ‚‚x‛‛'");
    let command = windows_update_command(Path::new(AWKWARD_FOLDER), "both");
    assert!(command.starts_with(&clear_installer_choices_ps1()), "{command}");
    assert!(command.contains("$env:KANBRAIN_APP = 'both';"), "{command}");
    assert!(
        command.contains(r"$env:KANBRAIN_INSTALL_DIR = 'C:\Users\Zoë O''Neil\my $HOME`s ‘‘bin’’ ‚‚x‛‛';"),
        "{command}"
    );
    assert!(command.ends_with(&format!("; irm {INSTALL_PS1} | iex")), "{command}");
}

/// PowerShell itself reads the command `kb update` prints back to the very folder and app, the installer's
/// download swapped for printing them: pwsh (7), and on Windows also Windows PowerShell (5.1), which every Windows
/// has. Skipped without `pwsh`, which GitHub's runners have on every OS: CI never skips.
#[test]
fn powershell_reads_the_update_command_back_to_the_folder() {
    let command = windows_update_command(Path::new(AWKWARD_FOLDER), "gui");
    let shown = command.replace(
        &format!("irm {INSTALL_PS1} | iex"),
        "[Console]::Out.Write($env:KANBRAIN_APP + '|' + $env:KANBRAIN_INSTALL_DIR)",
    );
    assert_ne!(shown, command, "the download is swapped out");
    // UTF-8 out, which Windows' OEM code pages, PowerShell's default there, would garble.
    let shown = format!("[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false); {shown}");
    let shells: &[&str] = if cfg!(windows) { &["pwsh", "powershell"] } else { &["pwsh"] };
    for &shell in shells {
        // The script goes by the environment, so no command line's quoting comes between it and PowerShell.
        let output = match Command::new(shell)
            .args(["-NoProfile", "-NonInteractive", "-Command", "Invoke-Expression $env:KB_SCRIPT"])
            .env("KB_SCRIPT", &shown)
            .output()
        {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && std::env::var_os("CI").is_none() => return,
            output => output.unwrap(),
        };
        assert!(output.status.success(), "{shell}: {}", String::from_utf8_lossy(&output.stderr));
        assert_eq!(String::from_utf8(output.stdout).unwrap(), format!("gui|{AWKWARD_FOLDER}"), "{shell}");
    }
}
