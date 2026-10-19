# kanbrain installer for Windows (PowerShell 5.1+ or PowerShell 7).
#
#   irm <url>/install.ps1 | iex
#
# Options are environment variables, set before running:
#   $env:KANBRAIN_APP         = "gui"         # cli (default): the command line `kb`; gui: the kanban app
#                                           # `kanbrain`; both. Both use the same tasks.
#                                           # With UNINSTALL/PURGE the default is both.
#   $env:KANBRAIN_VERSION     = "v0.6.4"      # default: latest
#   $env:KANBRAIN_INSTALL_DIR = "D:\tools"    # default: %LOCALAPPDATA%\Programs\kanbrain
#   $env:KANBRAIN_UNINSTALL   = "1"           # remove the app(s) (your tasks are kept)
#   $env:KANBRAIN_PURGE       = "1"           # remove the app(s) AND your central brain's data: entries, tags.md,
#                                                 # the project registry and cards, exported reports, and kanbrain's
#                                                 # cache: write locks, backups taken before migrations (projects'
#                                                 # own .brain/ folders are kept)
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
            # Escaped: -RedirectStandardOutput reads `[` `]` in a path as wildcards, and the file exists already.
            Start-Process -FilePath $exe -ArgumentList $arguments -Wait -NoNewWindow `
                -RedirectStandardOutput ([WildcardPattern]::Escape($out)) -ErrorAction Stop
            Get-Content -LiteralPath $out -Encoding UTF8 -ErrorAction SilentlyContinue | Select-Object -First 1
        } catch {
            $null
        } finally {
            Remove-Item -LiteralPath $out -Force -ErrorAction SilentlyContinue
        }
    }

    # Deletes one of the installed files, saying what to do if Windows refuses because it's open.
    function Remove-Binary($path, $what) {
        try {
            Remove-Item -LiteralPath $path -Force -ErrorAction Stop
        } catch {
            throw "kanbrain: could not remove $path ($($_.Exception.Message)). Is kanbrain open? Close it and try again."
        }
        Say "removed $what"
    }

    # Whether $file, in the central brain whose full path is $data, is one kanbrain made, by its name and what it holds
    # (as install.sh's is_owned), so a purge of a data folder shared with other notes never takes theirs: an entry
    # carries `- id: N` matching its name (2024-01-15.md doesn't), tags.md and projects.md their headers, last-id and
    # format a number, a card the name of a registered project, a report its header; temp files are .NAME.PID-N.tmp.
    function Test-Owned($data, $file) {
        $name = $file.Name
        if ($name -match '^\..*\.\d+-\d+\.tmp$') { return $true }
        $first = Get-Content -LiteralPath $file.FullName -TotalCount 1 -Encoding UTF8 -ErrorAction SilentlyContinue
        if ($file.DirectoryName -eq (Join-Path $data 'projects')) {
            return [bool](($first -match '^# (.+)$') -and (Test-Registered $data $Matches[1]))
        }
        # ASCII only in this file: Windows PowerShell reads a script without a byte-order mark as ANSI.
        if ($file.DirectoryName -eq (Join-Path $data 'reports')) { return [bool]($first -like '# Time report *') }
        if ($file.DirectoryName -ne $data) { return $false }
        if ($name -match '^([0-9]{4,})-.*\.md$') {
            $id = $Matches[1].TrimStart('0')
            if (-not $id) { $id = '0' }
            return [bool](Select-String -LiteralPath $file.FullName -Pattern "^- id: $id$" -Encoding UTF8 -Quiet)
        }
        if ($name -eq 'tags.md') { return $first -eq '# Tag colors' }
        if ($name -eq 'projects.md') { return $first -eq '# Projects' }
        if ($name -in 'last-id', 'format') { return [bool]((Get-Content -LiteralPath $file.FullName -Raw) -match '^\d+\s*$') }
        return $false
    }

    # Whether project $project is in the registry of the central brain at $data (`- shop: C:\...\.brain`).
    function Test-Registered($data, $project) {
        $registry = Join-Path $data 'projects.md'
        return [bool]((Test-Path -LiteralPath $registry) -and (Select-String -LiteralPath $registry -SimpleMatch "- ${project}: " -Encoding UTF8 -Quiet))
    }

    # Removes what versions before 0.3 installed in place of $bin: the app kanbrain-gui for the app, and for
    # kb the old command line kanbrain.exe. Any kanbrain.exe older than 0.3 is the command line (the app
    # was kanbrain-gui then, and has no console to answer --version anyway).
    function Remove-Old($bin) {
        if ($bin -eq 'kanbrain') {
            $old = Join-Path $installDir 'kanbrain-gui.exe'
            if (Test-Path -LiteralPath $old) { Remove-Binary $old "$old (from before 0.3)" }
        } elseif ($bin -eq 'kb') {
            $old = Join-Path $installDir 'kanbrain.exe'
            $v = $null
            if (Test-Path -LiteralPath $old) { $v = Get-Output $old @('--version') }
            if ($v -match '^kanbrain 0\.[0-2]\.') { Remove-Binary $old "the old command line $old" }
        }
    }

    # ---- uninstall ----
    if ($uninstall) {
        # Where the central brain is: ask the installed command line, else mirror its defaults. `--project central`,
        # because run inside a project `path` names that project's .brain\, which purge must not touch.
        $data = $null
        foreach ($cli in @((Join-Path $installDir 'kb.exe'), (Join-Path $installDir 'kanbrain.exe'))) {
            if (-not $data -and (Test-Path -LiteralPath $cli)) { $data = Get-Output $cli @('--project', 'central', 'path') }
        }
        if (-not $data) { $data = if ($env:KANBRAIN_DIR) { $env:KANBRAIN_DIR } else { Join-Path $env:APPDATA 'kanbrain' } }

        $owned = @()
        if ($purge) {
            # As kb refuses it: a relative path (`\todo` and `D:todo` too) names a different folder from each folder.
            if ($env:KANBRAIN_DIR -and $env:KANBRAIN_DIR -notmatch '^([A-Za-z]:[\\/]|[\\/]{2})') {
                throw "kanbrain: KANBRAIN_DIR must be an absolute path, not '$env:KANBRAIN_DIR': set it to the folder's full path"
            }
            $full = [IO.Path]::GetFullPath($data).TrimEnd('\', '/')
            $homes = @("$env:USERPROFILE".TrimEnd('\'), "$HOME".TrimEnd('\', '/')) | Where-Object { $_ }
            $unsafe = @([IO.Path]::GetPathRoot($full).TrimEnd('\', '/')) + $homes
            if (-not $full -or $unsafe -contains $full) { throw "kanbrain: refusing to delete data in '$data'" }
            foreach ($h in $homes) {
                if ("$h\".StartsWith("$full\", [StringComparison]::OrdinalIgnoreCase)) {
                    throw "kanbrain: refusing to delete data in '$data': it holds your home folder"
                }
            }
            if (Test-Path -LiteralPath $data -PathType Container) {
                # Only files kanbrain made (see Test-Owned), listed whole before deleting any: whose cards they are is
                # read from the registry, which goes too.
                $owned = @(foreach ($dir in $full, (Join-Path $full 'projects'), (Join-Path $full 'reports')) {
                    if (Test-Path -LiteralPath $dir -PathType Container) {
                        Get-ChildItem -LiteralPath $dir -File -Force | Where-Object { Test-Owned $full $_ }
                    }
                })
                $entries = @($owned | Where-Object { $_.DirectoryName -eq $full -and $_.Name -match '^\d{4,}-.*\.md$' }).Count
                Say "data folder: $data"
                Say "this permanently deletes $entries entry file(s), tags.md, the project registry and cards, exported reports, and the backups taken before migrations"
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
            if (Test-Path -LiteralPath $exe) { Remove-Binary $exe $exe } else { Say "no binary at $exe" }
            if ($bin -eq 'kanbrain' -and (Test-Path -LiteralPath $shortcut)) { Remove-Item -LiteralPath $shortcut -Force; Say "removed $shortcut" }
        }
        # Once neither app is left, the folder and its PATH entry go too.
        if ((Test-Path -LiteralPath $installDir) -and -not (Get-ChildItem -LiteralPath $installDir -Force)) { Remove-Item -LiteralPath $installDir -Force }
        $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
        if ($userPath -and -not (Test-Path -LiteralPath $installDir)) {
            $kept = ($userPath -split ';') | Where-Object { $_ -and ($_.TrimEnd('\') -ne $installDir.TrimEnd('\')) }
            [Environment]::SetEnvironmentVariable('Path', ($kept -join ';'), 'User')
        }

        if ($purge -and $data) {
            $owned | Remove-Item -Force
            foreach ($sub in 'projects', 'reports') {
                $dir = Join-Path $data $sub
                if ((Test-Path -LiteralPath $dir) -and -not (Get-ChildItem -LiteralPath $dir -Force)) { Remove-Item -LiteralPath $dir -Force }
            }
            if (-not (Get-ChildItem -LiteralPath $data -Force)) { Remove-Item -LiteralPath $data -Force; Say "removed $data" }
            else { Say "deleted kanbrain's files; kept $data because it contains other files" }
        } elseif (-not $purge) {
            Say "your tasks were kept in $data (set `$env:KANBRAIN_PURGE = `"1`" to delete them too)"
        }
        # kanbrain's folder in the user's cache, as kb finds it: each brain's write lock, and the copies taken before
        # migrating one, which hold its data. Only those go.
        $cache = Join-Path $env:LOCALAPPDATA 'kanbrain'
        if ($purge -and (Test-Path -LiteralPath $cache -PathType Container)) {
            foreach ($sub in 'locks', 'backups') {
                $dir = Join-Path $cache $sub
                if (Test-Path -LiteralPath $dir) { Remove-Item -LiteralPath $dir -Recurse -Force }
            }
            if (-not (Get-ChildItem -LiteralPath $cache -Force)) { Remove-Item -LiteralPath $cache -Force; Say "removed $cache" }
            else { Say "deleted kanbrain's locks and backups; kept $cache because it contains other files" }
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

    # The latest release's tag, asked once: two downloads each following `latest` could straddle a release and
    # install a command line and an app of different versions. `latest` stands when it can't be told.
    if (-not $env:KANBRAIN_BASE_URL -and $hostKind -eq 'github' -and $version -eq 'latest') {
        try {
            $answer = Invoke-WebRequest -Uri "https://github.com/$repo/releases/latest" -Method Head -UseBasicParsing
            # Where the redirect ended: Windows PowerShell and PowerShell 7 keep it in different places.
            $final = if ($answer.BaseResponse.ResponseUri) { $answer.BaseResponse.ResponseUri } else { $answer.BaseResponse.RequestMessage.RequestUri }
            if ("$final" -match '/tag/(v[^/?#]+)$') {
                $version = $Matches[1]
                $base = "https://github.com/$repo/releases/download/$version"
            }
        } catch { }
    }

    New-Item -ItemType Directory -Path $installDir -Force | Out-Null
    # Downloads $url to $path, written by .NET: Windows PowerShell 5.1's -OutFile reads `[` `]` in a path (a profile
    # folder's) as wildcards, and has no literal form.
    function Save-Url($url, $path) {
        $response = Invoke-WebRequest -Uri $url -UseBasicParsing
        [IO.File]::WriteAllBytes($path, $response.RawContentStream.ToArray())
    }
    $tmp = Join-Path ([IO.Path]::GetTempPath()) ("kanbrain-" + [Guid]::NewGuid())
    New-Item -ItemType Directory -Path $tmp | Out-Null
    try {
        # Every app is downloaded, checked and extracted before any is put in place: a failed download leaves the
        # installed ones as they were, of one version.
        foreach ($bin in $bins) {
            $asset = "$bin-x86_64-pc-windows-msvc.zip"
            $zip = Join-Path $tmp $asset
            Say "downloading $asset ($version)"
            try {
                Save-Url "$base/$asset" $zip
            } catch {
                throw "kanbrain: download failed: $base/$asset ($($_.Exception.Message))"
            }

            # ---- verify checksum ----
            $sumFile = Join-Path $tmp "$asset.sha256"
            # Every release publishes one; without it, nothing says the download is the release's.
            try {
                Save-Url "$base/$asset.sha256" $sumFile
            } catch {
                throw "kanbrain: no checksum found for ${asset}: $base/$asset.sha256"
            }
            $expected = ((Get-Content -LiteralPath $sumFile -Encoding UTF8 -Raw).Trim() -split '\s+')[0].ToLower()
            $actual = (Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash.ToLower()
            if ($actual -ne $expected) { throw "kanbrain: checksum mismatch for $asset (expected $expected, got $actual)" }
            Say 'checksum ok'
            $unpacked = Join-Path $tmp $bin
            Expand-Archive -LiteralPath $zip -DestinationPath $unpacked -Force
            if (-not (Test-Path -LiteralPath (Join-Path $unpacked "$bin.exe"))) { throw "kanbrain: archive does not contain $bin.exe" }
        }

        # ---- install ----
        foreach ($bin in $bins) {
            $exe = Join-Path $installDir "$bin.exe"
            Remove-Old $bin
            try {
                Copy-Item -LiteralPath (Join-Path $tmp "$bin\$bin.exe") $exe -Force
            } catch {
                throw "kanbrain: could not write $exe. Is $bin running? Close it and try again."
            }

            # Not into $version, which names the release installed.
            $said = Get-Output $exe @('--version')
            if ($said) { Say "installed $said to $exe" } else { Say "warning: installed $exe, but it doesn't start here" }

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
    } finally {
        Remove-Item -LiteralPath $tmp -Recurse -Force -ErrorAction SilentlyContinue
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
        # Escaped, as in Get-Output.
        $logPattern = [WildcardPattern]::Escape($log)
        Start-Process -FilePath $first -ArgumentList '--help' -Wait -NoNewWindow -RedirectStandardOutput $logPattern
        if ((Get-Content -LiteralPath $log -Encoding UTF8 -Raw) -match 'init --all') {
            $run = Start-Process -FilePath $first -ArgumentList 'init', '--all' -Wait -NoNewWindow -PassThru `
                -RedirectStandardOutput $logPattern
            if ($run.ExitCode -eq 0) {
                Get-Content -LiteralPath $log -Encoding UTF8 | Where-Object { $_ } | ForEach-Object { Say "$_" }
            } else {
                Say "warning: couldn't update your projects' agent instructions; run: $firstBin init --all"
            }
        }
    } catch {
        Say "warning: couldn't update your projects' agent instructions; run: $firstBin init --all"
    } finally {
        Remove-Item -LiteralPath $log -Force -ErrorAction SilentlyContinue
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
