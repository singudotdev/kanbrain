# End-to-end check of install.ps1 as a user runs it, on Windows: this build of `kb` is packaged as a release asset,
# served over HTTP, installed into a throwaway folder of the user profile, and then:
#
# - the download is checked against its checksum, and refused when that doesn't match;
# - the installed `kb` runs, and `init --all` has brought a registered project's agent section up to date;
# - uninstalling takes it out again, and out of the user PATH;
# - both apps install together (KANBRAIN_APP=both), the kanban app with its Start Menu shortcut, and uninstall
#   together; `kb` stands in for the app, which takes the same commands, so GPUI needn't be built;
# - purging deletes the central brain's files and kanbrain's locks and backups in the cache, keeping a file it
#   didn't create.
#
# CI runs it on every push (the `installers` job), as an administrator: hence KANBRAIN_ALLOW_ADMIN. It needs
# cargo and python.
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
cargo build -q --release --manifest-path "$repo/Cargo.toml" --bin kb
if ($LASTEXITCODE -ne 0) { throw 'build failed' }
$built = Join-Path $repo 'target/release/kb.exe'

# Inside the user profile, since the installer only installs there.
$work = Join-Path $env:USERPROFILE "kb-install-$(Get-Random)"
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
    $port = python -c 'import socket; s = socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1])'
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
    Set-Content -Path (Join-Path $env:KANBRAIN_DIR 'notes-of-mine.txt') -Value 'mine'
    Set-Content -Path (Join-Path $env:KANBRAIN_DIR '.0001-left-by-a-crash.md.4242-0.tmp') -Value ''
    Set-Content -Path (Join-Path $env:KANBRAIN_DIR 'projects\.demo.md.4242-1.tmp') -Value ''
    $env:KANBRAIN_PURGE = '1'
    $env:KANBRAIN_YES = '1'
    & (Join-Path $repo 'install.ps1') *>&1 | Out-Null
    if (Test-Path $cache) { Fail "purge left kanbrain's cache" }
    if (-not (Test-Path (Join-Path $env:KANBRAIN_DIR 'notes-of-mine.txt'))) { Fail "purge deleted a file it didn't create" }
    if (Test-Path (Join-Path $env:KANBRAIN_DIR 'projects.md')) { Fail 'purge kept the project registry' }
    if (Test-Path (Join-Path $env:KANBRAIN_DIR '.0001-left-by-a-crash.md.4242-0.tmp')) { Fail 'purge kept a leftover temporary file' }
    if (Test-Path (Join-Path $env:KANBRAIN_DIR 'projects')) { Fail "purge kept the project cards' folder" }
    if ((Get-Content $claude -Raw) -notmatch '## Project memory: kanbrain') { Fail 'purge touched the project' }

    Write-Host 'PASS install.ps1'
} finally {
    if ($server) { Stop-Process -Id $server.Id -Force -ErrorAction SilentlyContinue }
    Remove-Item $work -Recurse -Force -ErrorAction SilentlyContinue
}
