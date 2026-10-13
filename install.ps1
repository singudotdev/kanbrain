# kanbrain installer for Windows (PowerShell 5.1+ or PowerShell 7).
#
#   irm <url>/install.ps1 | iex
#
# Options are environment variables, set before running:
#   $env:KANBRAIN_APP         = "gui"         # cli (default): the command line `kb`; gui: the kanban app
#                                           # `kanbrain`; both. Both use the same tasks.
#                                           # With UNINSTALL/PURGE the default is both.
#   $env:KANBRAIN_VERSION     = "v0.3.1"      # default: latest
#   $env:KANBRAIN_INSTALL_DIR = "D:\tools"    # default: %LOCALAPPDATA%\Programs\kanbrain
#   $env:KANBRAIN_UNINSTALL   = "1"           # remove the app(s) (your tasks are kept)
#   $env:KANBRAIN_PURGE       = "1"           # remove the app(s) AND your central brain's data: entries, tags.md,
#                                                 # the project registry and cards, exported reports (projects' own
#                                                 # .brain/ folders are kept)
#   $env:KANBRAIN_YES         = "1"           # with PURGE: don't ask for confirmation
# Also: KANBRAIN_REPO, KANBRAIN_HOST (github | gitlab), KANBRAIN_GITLAB_URL, KANBRAIN_BASE_URL.
#
# kanbrain is installed and used per user: this script refuses to run elevated ("Run as administrator")
# and only installs inside your user profile. (Admin-only accounts: $env:KANBRAIN_ALLOW_ADMIN = "1".)

# Everything lives in a function so `iex` never leaks variables or closes the caller's shell.
function Install-Kanbrain {
    $ErrorActionPreference = 'Stop'
    $ProgressPreference = 'SilentlyContinue'   # Invoke-WebRequest is very slow with the progress bar

    # ---- where releases are published: edit these two lines for your repository ----
    $repo = if ($env:KANBRAIN_REPO) { $env:KANBRAIN_REPO } else { 'singudotdev/kanbrain' }
    $hostKind = if ($env:KANBRAIN_HOST) { $env:KANBRAIN_HOST } else { 'github' }

    $version = if ($env:KANBRAIN_VERSION) { $env:KANBRAIN_VERSION } else { 'latest' }
    $installDir = if ($env:KANBRAIN_INSTALL_DIR) { $env:KANBRAIN_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA 'Programs\kanbrain' }

    function Say($msg) { Write-Host "kanbrain: $msg" }

    # ---- per-user only ----
    $onWindows = ($PSVersionTable.PSEdition -eq 'Desktop') -or $IsWindows
    if ($onWindows -and $env:KANBRAIN_ALLOW_ADMIN -ne '1') {
        $principal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
        if ($principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
            throw 'kanbrain: installed per user: run this in a normal PowerShell, not "Run as administrator". (Admin-only accounts: $env:KANBRAIN_ALLOW_ADMIN = "1")'
        }
    }
    $sep = [IO.Path]::DirectorySeparatorChar
    $userRoot = [IO.Path]::GetFullPath($(if ($env:USERPROFILE) { $env:USERPROFILE } else { $HOME })).TrimEnd('\', '/')
    $installDir = [IO.Path]::GetFullPath($installDir).TrimEnd('\', '/')    # also resolves '..'
    if (-not "$installDir$sep".StartsWith("$userRoot$sep", [StringComparison]::OrdinalIgnoreCase)) {
        throw "kanbrain: install folder must be inside your user profile ($userRoot), got $installDir"
    }

    # ---- which apps ----
    $purge = $env:KANBRAIN_PURGE -eq '1'
    $uninstall = $purge -or $env:KANBRAIN_UNINSTALL -eq '1'
    $app = if ($env:KANBRAIN_APP) { $env:KANBRAIN_APP } elseif ($uninstall) { 'both' } else { 'cli' }
    $bins = switch ($app) {
        'cli' { @('kb') }
        'gui' { @('kanbrain') }
        'both' { @('kb', 'kanbrain') }
        default { throw "kanbrain: KANBRAIN_APP must be cli, gui or both, got $app" }
    }
    # The desktop app's Start Menu entry.
    $shortcut = Join-Path ([Environment]::GetFolderPath('Programs')) 'kanbrain.lnk'

    # The first line $exe prints for $arguments, once it has exited; $null if it can't be run. Not `& $exe`: the app
    # is a GUI program, which PowerShell doesn't wait for, and while it's still running Windows keeps its file
    # locked, so replacing or removing it right after fails with "access denied".
    function Get-Output($exe, [string[]]$arguments) {
        $out = [IO.Path]::GetTempFileName()
        try {
            Start-Process -FilePath $exe -ArgumentList $arguments -Wait -NoNewWindow -RedirectStandardOutput $out -ErrorAction Stop
            Get-Content $out -ErrorAction SilentlyContinue | Select-Object -First 1
        } catch {
            $null
        } finally {
            Remove-Item $out -Force -ErrorAction SilentlyContinue
        }
    }

    # Deletes one of the installed files, saying what to do if Windows refuses because it's open.
    function Remove-Binary($path, $what) {
        try {
            Remove-Item $path -Force -ErrorAction Stop
        } catch {
            throw "kanbrain: could not remove $path ($($_.Exception.Message)). Is kanbrain open? Close it and try again."
        }
        Say "removed $what"
    }

    # Removes what versions before 0.3 installed in place of $bin: the app kanbrain-gui for the app, and for
    # kb the old command line kanbrain.exe. Any kanbrain.exe older than 0.3 is the command line (the app
    # was kanbrain-gui then, and has no console to answer --version anyway).
    function Remove-Old($bin) {
        if ($bin -eq 'kanbrain') {
            $old = Join-Path $installDir 'kanbrain-gui.exe'
            if (Test-Path $old) { Remove-Binary $old "$old (from before 0.3)" }
        } elseif ($bin -eq 'kb') {
            $old = Join-Path $installDir 'kanbrain.exe'
            $v = $null
            if (Test-Path $old) { $v = Get-Output $old @('--version') }
            if ($v -match '^kanbrain 0\.[0-2]\.') { Remove-Binary $old "the old command line $old" }
        }
    }

    # ---- uninstall ----
    if ($uninstall) {
        # Where the central brain is: ask the installed command line, else mirror its defaults. `--project central`,
        # because run inside a project `path` names that project's .brain\, which purge must not touch.
        $data = $null
        foreach ($cli in @((Join-Path $installDir 'kb.exe'), (Join-Path $installDir 'kanbrain.exe'))) {
            if (-not $data -and (Test-Path $cli)) { $data = Get-Output $cli @('--project', 'central', 'path') }
        }
        if (-not $data) { $data = if ($env:KANBRAIN_DIR) { $env:KANBRAIN_DIR } else { Join-Path $env:APPDATA 'kanbrain' } }

        $owned = @()
        if ($purge) {
            $full = [IO.Path]::GetFullPath($data).TrimEnd('\', '/')
            $unsafe = @([IO.Path]::GetPathRoot($full).TrimEnd('\', '/'), "$env:USERPROFILE".TrimEnd('\'), "$HOME".TrimEnd('\', '/'))
            if (-not $full -or $unsafe -contains $full) { throw "kanbrain: refusing to delete data in '$data'" }
            if (Test-Path $data -PathType Container) {
                # Only files kanbrain creates: NNNN-slug.md entries, tags.md, last-id, the project registry (projects.md)
                # and cards (projects\*.md), exported reports, older versions' config.md, leftover temp files.
                $owned = @(Get-ChildItem $data -File -Force | Where-Object {
                    $_.Name -match '^\d{4,}-.*\.md$' -or $_.Name -in 'tags.md', 'projects.md', 'last-id', 'config.md', '.last-id.tmp' -or
                    $_.Name -match '^\..*\.md\.tmp$' })
                $entries = @($owned | Where-Object { $_.Name -match '^\d{4,}-.*\.md$' }).Count
                foreach ($sub in 'projects', 'reports') {
                    $dir = Join-Path $data $sub
                    if (Test-Path $dir -PathType Container) { $owned += @(Get-ChildItem $dir -File -Force -Filter '*.md') }
                }
                Say "data folder: $data"
                Say "this permanently deletes $entries entry file(s), tags.md, the project registry and cards, and exported reports"
                Say "projects' own .brain folders are kept"
                if ($env:KANBRAIN_YES -ne '1') {
                    $answer = ''
                    try { $answer = Read-Host 'kanbrain: delete your kanbrain data? [y/N]' } catch {
                        throw 'kanbrain: no terminal to confirm on; set $env:KANBRAIN_YES = "1" to delete your data'
                    }
                    if ($answer -notmatch '^(y|yes)$') { Say 'cancelled, nothing was removed'; return }
                }
            } else {
                Say "no data folder at $data"
                $data = $null
            }
        }

        foreach ($bin in $bins) {
            # Before kb, the command line itself was kanbrain.exe: take that out first, while it can still be told
            # apart from the app.
            Remove-Old $bin
            $exe = Join-Path $installDir "$bin.exe"
            if (Test-Path $exe) { Remove-Binary $exe $exe } else { Say "no binary at $exe" }
            if ($bin -eq 'kanbrain' -and (Test-Path $shortcut)) { Remove-Item $shortcut -Force; Say "removed $shortcut" }
        }
        # Once neither app is left, the folder and its PATH entry go too.
        if ((Test-Path $installDir) -and -not (Get-ChildItem $installDir -Force)) { Remove-Item $installDir -Force }
        $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
        if ($userPath -and -not (Test-Path $installDir)) {
            $kept = ($userPath -split ';') | Where-Object { $_ -and ($_.TrimEnd('\') -ne $installDir.TrimEnd('\')) }
            [Environment]::SetEnvironmentVariable('Path', ($kept -join ';'), 'User')
        }

        if ($purge -and $data) {
            $owned | Remove-Item -Force
            foreach ($sub in 'projects', 'reports') {
                $dir = Join-Path $data $sub
                if ((Test-Path $dir) -and -not (Get-ChildItem $dir -Force)) { Remove-Item $dir -Force }
            }
            if (-not (Get-ChildItem $data -Force)) { Remove-Item $data -Force; Say "removed $data" }
            else { Say "deleted kanbrain's files; kept $data because it contains other files" }
        } elseif (-not $purge) {
            Say "your tasks were kept in $data (set `$env:KANBRAIN_PURGE = `"1`" to delete them too)"
        }
        return
    }

    # ---- detect platform ----
    # Windows on ARM runs the x64 build through emulation.
    $arch = if ($env:PROCESSOR_ARCHITEW6432) { $env:PROCESSOR_ARCHITEW6432 } else { $env:PROCESSOR_ARCHITECTURE }
    if ($arch -notin @('AMD64', 'ARM64')) { throw "kanbrain: unsupported CPU architecture: $arch (64-bit Windows is required)" }

    # ---- download location ----
    if ($env:KANBRAIN_BASE_URL) {
        $base = $env:KANBRAIN_BASE_URL
    } elseif ($hostKind -eq 'github') {
        $base = if ($version -eq 'latest') { "https://github.com/$repo/releases/latest/download" } else { "https://github.com/$repo/releases/download/$version" }
    } elseif ($hostKind -eq 'gitlab') {
        $gl = if ($env:KANBRAIN_GITLAB_URL) { $env:KANBRAIN_GITLAB_URL } else { 'https://gitlab.com' }
        $base = if ($version -eq 'latest') { "$gl/$repo/-/releases/permalink/latest/downloads" } else { "$gl/$repo/-/releases/$version/downloads" }
    } else {
        throw 'kanbrain: KANBRAIN_HOST must be github or gitlab'
    }

    # Windows PowerShell 5.1 defaults to old TLS versions.
    try { [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12 } catch { }

    New-Item -ItemType Directory -Path $installDir -Force | Out-Null
    foreach ($bin in $bins) {
        $asset = "$bin-x86_64-pc-windows-msvc.zip"
        $exe = Join-Path $installDir "$bin.exe"
        $tmp = Join-Path ([IO.Path]::GetTempPath()) ("kanbrain-" + [Guid]::NewGuid())
        New-Item -ItemType Directory -Path $tmp | Out-Null
        try {
            $zip = Join-Path $tmp $asset
            Say "downloading $asset ($version)"
            try {
                Invoke-WebRequest -Uri "$base/$asset" -OutFile $zip -UseBasicParsing
            } catch {
                throw "kanbrain: download failed: $base/$asset ($($_.Exception.Message))"
            }

            # ---- verify checksum ----
            $sumFile = Join-Path $tmp "$asset.sha256"
            # Every release publishes one; without it, nothing says the download is the release's.
            try {
                Invoke-WebRequest -Uri "$base/$asset.sha256" -OutFile $sumFile -UseBasicParsing
            } catch {
                throw "kanbrain: no checksum found for ${asset}: $base/$asset.sha256"
            }
            $expected = ((Get-Content $sumFile -Raw).Trim() -split '\s+')[0].ToLower()
            $actual = (Get-FileHash $zip -Algorithm SHA256).Hash.ToLower()
            if ($actual -ne $expected) { throw "kanbrain: checksum mismatch for $asset (expected $expected, got $actual)" }
            Say 'checksum ok'

            # ---- install ----
            Remove-Old $bin
            Expand-Archive -Path $zip -DestinationPath $tmp -Force
            $newExe = Join-Path $tmp "$bin.exe"
            if (-not (Test-Path $newExe)) { throw "kanbrain: archive does not contain $bin.exe" }
            try {
                Copy-Item $newExe $exe -Force
            } catch {
                throw "kanbrain: could not write $exe. Is $bin running? Close it and try again."
            }
        } finally {
            Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue
        }

        $installed = $bin
        $version = Get-Output $exe @('--version')
        if ($version) { $installed = $version }
        Say "installed $installed to $exe"

        # The desktop app goes in the Start Menu.
        if ($bin -eq 'kanbrain') {
            try {
                $link = (New-Object -ComObject WScript.Shell).CreateShortcut($shortcut)
                $link.TargetPath = $exe
                $link.WorkingDirectory = $installDir
                $link.Description = 'kanbrain: kanban board'
                $link.Save()
                Say "added kanbrain to the Start Menu"
            } catch {
                Say "warning: could not add a Start Menu shortcut ($($_.Exception.Message)); run $exe"
            }
        }
    }

    # ---- projects' agents ----
    # They follow the new version: their section and hooks are refreshed in every registered repository. Only by a
    # version that has `init --all`: an older one would take `--all` for a project's name.
    # Through Start-Process, as Get-Output does: the kanban app has no console, so `&` wouldn't wait or capture it.
    # @(): with one app, PowerShell has made $bins a string, whose [0] is its first letter.
    $firstBin = @($bins)[0]
    $first = Join-Path $installDir "$firstBin.exe"
    $log = [IO.Path]::GetTempFileName()
    try {
        Start-Process -FilePath $first -ArgumentList '--help' -Wait -NoNewWindow -RedirectStandardOutput $log
        if ((Get-Content $log -Raw) -match 'init --all') {
            $run = Start-Process -FilePath $first -ArgumentList 'init', '--all' -Wait -NoNewWindow -PassThru `
                -RedirectStandardOutput $log
            if ($run.ExitCode -eq 0) {
                Get-Content $log | Where-Object { $_ } | ForEach-Object { Say "$_" }
            } else {
                Say "warning: couldn't update your projects' agent instructions; run: $firstBin init --all"
            }
        }
    } catch {
        Say "warning: couldn't update your projects' agent instructions; run: $firstBin init --all"
    } finally {
        Remove-Item $log -Force -ErrorAction SilentlyContinue
    }

    # ---- PATH ----
    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    $entries = if ($userPath) { $userPath -split ';' } else { @() }
    if (-not ($entries | Where-Object { $_.TrimEnd('\') -eq $installDir.TrimEnd('\') })) {
        $newPath = (@($entries | Where-Object { $_ }) + $installDir) -join ';'
        [Environment]::SetEnvironmentVariable('Path', $newPath, 'User')
        Say "added $installDir to your user PATH (new terminals pick it up)"
    }
    if (-not (($env:Path -split ';') -contains $installDir)) { $env:Path = "$env:Path;$installDir" }
    Say "run: $($bins -join ' or ')"
}

Install-Kanbrain
