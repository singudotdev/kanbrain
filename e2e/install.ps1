# End-to-end check of install.ps1 as a user runs it, on Windows: this build of `kb` is packaged as a release asset,
# served over HTTP, installed into a throwaway folder of the user profile, and then:
#
# - the download is checked against its checksum, and refused when that doesn't match;
# - the installed `kb` runs, and `init --all` has brought a registered project's agent section up to date;
# - uninstalling takes it out again, and out of the user PATH;
# - both apps install together (KANBRAIN_APP=both), the kanban app with its Start Menu shortcut, and uninstall
#   together; `kb` stands in for the app, which takes the same commands, so GPUI needn't be built;
# - purging deletes the central brain's files and kanbrain's locks and backups in the cache, keeping files it
#   didn't create though named like its own, and refusing the user profile however it's written and a relative
#   KANBRAIN_DIR.
#
# CI runs it when a change touches it (the `installers` job), as an administrator: hence KANBRAIN_ALLOW_ADMIN. It
# needs cargo and python.
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
cargo build -q --release --manifest-path "$repo/Cargo.toml" --bin kb
if ($LASTEXITCODE -ne 0) { throw 'build failed' }
$built = Join-Path $repo 'target/release/kb.exe'

# Inside the user profile, since the installer only installs there; with an accented letter in its name, as a
# profile like Jose's with its accent has, which the installer reads back from `kb` (written as its code point: this
# file is ASCII, for Windows PowerShell 5.1).
$work = Join-Path $env:USERPROFILE "kb-install-$([char]0xE9)-$(Get-Random)"
$dist = Join-Path $work 'dist'
New-Item -ItemType Directory -Force -Path $dist, (Join-Path $work 'pkg') | Out-Null
$server = $null

function Fail($why) { throw "FAIL: $why" }

try {
    # The release asset install.ps1 downloads, as the Release workflow names and hashes it.
    $asset = 'kb-x86_64-pc-windows-msvc.zip'
    Copy-Item $built, (Join-Path $repo 'LICENSE') (Join-Path $work 'pkg')
    Compress-Archive -Path (Join-Path $work 'pkg/kb.exe'), (Join-Path $work 'pkg/LICENSE') -DestinationPath (Join-Path $dist $asset)
    $hash = (Get-FileHash (Join-Path $dist $asset) -Algorithm SHA256).Hash.ToLower()
    $sum = Join-Path $dist "$asset.sha256"
    Set-Content -NoNewline -Path $sum -Value "$hash  $asset`n"
    $appAsset = 'kanbrain-x86_64-pc-windows-msvc.zip'
    New-Item -ItemType Directory -Force -Path (Join-Path $work 'app') | Out-Null
    Copy-Item $built (Join-Path $work 'app/kanbrain.exe')
    Copy-Item (Join-Path $repo 'LICENSE') (Join-Path $work 'app')
    Compress-Archive -Path (Join-Path $work 'app/kanbrain.exe'), (Join-Path $work 'app/LICENSE') -DestinationPath (Join-Path $dist $appAsset)
    $appHash = (Get-FileHash (Join-Path $dist $appAsset) -Algorithm SHA256).Hash.ToLower()
    Set-Content -NoNewline -Path (Join-Path $dist "$appAsset.sha256") -Value "$appHash  $appAsset`n"

    # A free port, as another program may hold any fixed one.
    # Single quotes inside: Windows PowerShell 5.1 drops double quotes from a native program's argument.
    $port = python -c 'import socket; s = socket.socket(); s.bind((''127.0.0.1'', 0)); print(s.getsockname()[1])'
    $server = Start-Process python -ArgumentList '-m', 'http.server', "$port", '--bind', '127.0.0.1', '--directory', $dist `
        -PassThru -WindowStyle Hidden
    $up = $false
    # Python can take a while to start the first time on a fresh machine.
    foreach ($i in 1..120) {
        try { Invoke-WebRequest -Uri "http://127.0.0.1:$port/$asset.sha256" -UseBasicParsing | Out-Null; $up = $true; break }
        catch { Start-Sleep -Milliseconds 500 }
    }
    if (-not $up) { Fail 'the test server did not start' }

    $env:KANBRAIN_DIR = Join-Path $work 'central'
    $env:KANBRAIN_BASE_URL = "http://127.0.0.1:$port"
    $env:KANBRAIN_INSTALL_DIR = Join-Path $work 'bin'
    $env:KANBRAIN_APP = 'cli'
    $env:KANBRAIN_ALLOW_ADMIN = '1'
    # The installer downloads into the temporary folder, under the user profile: one with `[` `]` in its name, which
    # PowerShell reads as wildcards wherever a path isn't taken literally.
    $env:TMP = $env:TEMP = [IO.Directory]::CreateDirectory((Join-Path $work 't[1]')).FullName

    # A project registered by an older version: its agent section is out of date.
    $project = Join-Path $work 'code/demo'
    New-Item -ItemType Directory -Force -Path $project | Out-Null
    Push-Location $project
    try { & $built init demo | Out-Null } finally { Pop-Location }
    $claude = Join-Path $project 'CLAUDE.md'
    Set-Content -Path $claude -Value "<!-- kanbrain:start -->`nold advice`n<!-- kanbrain:end -->"

    $out = & (Join-Path $repo 'install.ps1') *>&1 | Out-String
    Write-Host $out
    $installed = Join-Path $env:KANBRAIN_INSTALL_DIR 'kb.exe'
    if ($out -notmatch 'checksum ok') { Fail "the checksum wasn't checked" }
    if ((& $installed --version) -ne (& $built --version)) { Fail "the installed kb isn't this build" }
    if ($out -notmatch 'demo: updated CLAUDE.md') { Fail "init --all didn't run" }
    if ((Get-Content $claude -Raw) -notmatch '## Project memory: kanbrain') { Fail "the project's section wasn't updated" }

    # A download that doesn't match its checksum is refused, and the installed kb is left as it was.
    $good = Get-Content $sum -Raw
    Set-Content -NoNewline -Path $sum -Value "$('0' * 64)  $asset`n"
    $refused = $false
    try { & (Join-Path $repo 'install.ps1') *>&1 | Out-Null } catch { $refused = "$_" -match 'checksum mismatch' }
    if (-not $refused) { Fail 'a wrong checksum was installed, or failed for another reason' }
    & $installed --version | Out-Null
    Set-Content -NoNewline -Path $sum -Value $good

    # Both apps are fetched before either is put in place: the app's download failing leaves kb as it was.
    (Get-Item $installed).LastWriteTime = [datetime]'2000-01-01'
    $away = Join-Path $work 'app-asset.away'
    Move-Item (Join-Path $dist $appAsset) $away
    $env:KANBRAIN_APP = 'both'
    $refused = $false
    try { & (Join-Path $repo 'install.ps1') *>&1 | Out-Null } catch { $refused = $true }
    Remove-Item Env:KANBRAIN_APP
    Move-Item $away (Join-Path $dist $appAsset)
    if (-not $refused) { Fail "an install missing the app's download succeeded" }
    if ((Get-Item $installed).LastWriteTime.Year -ne 2000) { Fail 'a failed install replaced kb before fetching the app' }

    $env:KANBRAIN_UNINSTALL = '1'
    & (Join-Path $repo 'install.ps1') *>&1 | Out-Null
    if (Test-Path $installed) { Fail "uninstall left $installed" }
    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    if ($userPath -and ($userPath -split ';') -contains $env:KANBRAIN_INSTALL_DIR) { Fail 'uninstall left the PATH entry' }
    if ((Get-Content $claude -Raw) -notmatch '## Project memory: kanbrain') { Fail 'uninstall touched the project' }

    # Both apps at once: each downloads the release asked for, and the kanban app gets its Start Menu shortcut.
    Remove-Item Env:KANBRAIN_UNINSTALL
    $env:KANBRAIN_APP = 'both'
    $out = & (Join-Path $repo 'install.ps1') *>&1 | Out-String
    $app = Join-Path $env:KANBRAIN_INSTALL_DIR 'kanbrain.exe'
    $shortcut = Join-Path ([Environment]::GetFolderPath('Programs')) 'kanbrain.lnk'
    if (-not ((Test-Path $installed) -and (Test-Path $app))) { Fail "both apps weren't installed: $out" }
    if ($out -notmatch "downloading $([regex]::Escape($appAsset)) \(latest\)") { Fail "the app's download named another release: $out" }
    if (-not (Test-Path $shortcut)) { Fail 'the kanban app got no Start Menu shortcut' }
    $env:KANBRAIN_UNINSTALL = '1'
    & (Join-Path $repo 'install.ps1') *>&1 | Out-Null
    if ((Test-Path $installed) -or (Test-Path $app) -or (Test-Path $shortcut)) { Fail 'uninstall left an app or the shortcut' }

    # Purging deletes the central brain's data and kanbrain's locks and backups in the cache, nothing else of either.
    $env:LOCALAPPDATA = Join-Path $work 'localappdata'
    $cache = Join-Path $env:LOCALAPPDATA 'kanbrain'
    New-Item -ItemType Directory -Force -Path (Join-Path $cache 'locks'), (Join-Path $cache 'backups\0123456789abcdef') | Out-Null
    Set-Content -Path (Join-Path $cache 'locks\0123456789abcdef.lock') -Value ''
    # Shared with notes of the user's own, as in an Obsidian vault: purge takes only what kanbrain made, by what each
    # file holds, not by its name.
    $data = $env:KANBRAIN_DIR
    New-Item -ItemType Directory -Force -Path (Join-Path $data 'reports') | Out-Null
    $files = @{
        '0007-mine.md' = "# Mine`n`n- id: 7`n- status: todo`n- tags: `n"; 'notes-of-mine.txt' = 'mine'
        '2024-01-15.md' = "# Daily`n"; '2024-01-15-meeting.md' = "# Meeting`n"; 'format' = "my own format notes`n"
        'projects\website-redesign.md' = "# Website redesign`n"; 'reports\q3.md' = "# Q3`n"
        'reports\kanbrain-week.md' = "# Time report $([char]0xB7) week 42`n"; '.0001-left-by-a-crash.md.4242-0.tmp' = ''
        'projects\.demo.md.4242-1.tmp' = ''
    }
    foreach ($name in $files.Keys) {
        [IO.File]::WriteAllText((Join-Path $data $name), $files[$name], [Text.UTF8Encoding]::new($false))
    }
    $env:KANBRAIN_PURGE = '1'
    $env:KANBRAIN_YES = '1'
    # A data folder that's the user profile, however it's written, is refused before anything is deleted.
    $env:KANBRAIN_DIR = "$env:USERPROFILE\."
    $refused = $false
    try { & (Join-Path $repo 'install.ps1') *>&1 | Out-Null } catch { $refused = $true }
    $env:KANBRAIN_DIR = $data
    if (-not $refused) { Fail 'purge took the user profile as data' }
    # A relative one, which names another folder from each folder kb runs in, is refused as kb refuses it.
    $env:KANBRAIN_DIR = Split-Path -Leaf $data
    $refused = $false
    Push-Location (Split-Path -Parent $data)
    try { & (Join-Path $repo 'install.ps1') *>&1 | Out-Null } catch { $refused = $true } finally { Pop-Location }
    $env:KANBRAIN_DIR = $data
    if (-not $refused) { Fail 'purge took a relative KANBRAIN_DIR' }
    if (-not (Test-Path (Join-Path $data '0007-mine.md'))) { Fail 'a refused purge deleted something' }
    & (Join-Path $repo 'install.ps1') *>&1 | Out-Null
    if (Test-Path $cache) { Fail "purge left kanbrain's cache" }
    foreach ($gone in '0007-mine.md', 'projects.md', '.0001-left-by-a-crash.md.4242-0.tmp', 'projects\demo.md',
        'projects\.demo.md.4242-1.tmp', 'reports\kanbrain-week.md') {
        if (Test-Path (Join-Path $data $gone)) { Fail "purge kept kanbrain's $gone" }
    }
    foreach ($kept in 'notes-of-mine.txt', '2024-01-15.md', '2024-01-15-meeting.md', 'format', 'projects\website-redesign.md',
        'reports\q3.md') {
        if (-not (Test-Path (Join-Path $data $kept))) { Fail "purge deleted $kept, which kanbrain didn't make" }
    }
    if ((Get-Content $claude -Raw) -notmatch '## Project memory: kanbrain') { Fail 'purge touched the project' }

    Write-Host 'PASS install.ps1'
} finally {
    if ($server) { Stop-Process -Id $server.Id -Force -ErrorAction SilentlyContinue }
    Remove-Item $work -Recurse -Force -ErrorAction SilentlyContinue
}
