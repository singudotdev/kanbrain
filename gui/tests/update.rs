use super::*;

#[test]
fn a_program_replaced_since_it_started_restarts_from_the_new_file() {
    let replaced = running_file(PathBuf::from("/home/me/.local/bin/kanbrain (deleted)"));
    assert_eq!(replaced, PathBuf::from("/home/me/.local/bin/kanbrain"));
    assert_eq!(running_file(PathBuf::from("/opt/kanbrain")), PathBuf::from("/opt/kanbrain"));
}

/// The app's update script, run by PowerShell with the download failing, reads every path back exactly: the folder
/// and app go to the installer, the error to the note, and the app is started again from its file. The cmdlets that
/// would wait, download or start a program are stood in for by functions, which PowerShell runs before them: pwsh
/// (7), and on Windows also Windows PowerShell (5.1), which the app runs. Skipped without `pwsh`, which GitHub's
/// runners have on every OS: CI never skips.
#[test]
fn powershell_reads_the_apps_update_script_back_to_every_path() {
    let tmp = tempfile::tempdir().unwrap();
    let awkward = tmp.path().join("Zoë O'Neil $HOME`s ‘bin’ ‚x‛");
    std::fs::create_dir(&awkward).unwrap();
    let (exe, note) = (awkward.join("kanbrain.exe"), awkward.join("update failed.txt"));
    let script = after_exit_script(1, &exe, &awkward, "both", &note);
    let stand_ins = "[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false); \
        function Wait-Process {}; \
        function Invoke-RestMethod { [Console]::Out.Write($env:KANBRAIN_APP + '|' + $env:KANBRAIN_INSTALL_DIR + '|'); \
        throw 'no download' }; \
        function Start-Process { param($FilePath) [Console]::Out.Write($FilePath) }";
    let shells: &[&str] = if cfg!(windows) { &["pwsh", "powershell"] } else { &["pwsh"] };
    for &shell in shells {
        let _ = std::fs::remove_file(&note);
        // The script goes by the environment, so no command line's quoting comes between it and PowerShell.
        let output = match Command::new(shell)
            .args(["-NoProfile", "-NonInteractive", "-Command", "Invoke-Expression $env:KB_SCRIPT"])
            .env("KB_SCRIPT", format!("{stand_ins}; {script}"))
            .output()
        {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && std::env::var_os("CI").is_none() => return,
            output => output.unwrap(),
        };
        assert!(output.status.success(), "{shell}: {}", String::from_utf8_lossy(&output.stderr));
        let said = String::from_utf8(output.stdout).unwrap();
        assert_eq!(said, format!("both|{}|{}", awkward.display(), exe.display()), "{shell}");
        let noted = std::fs::read_to_string(&note).unwrap_or_else(|e| panic!("{shell}: no note: {e}"));
        assert_eq!(noted.trim_end(), "no download", "{shell}");
    }
}
