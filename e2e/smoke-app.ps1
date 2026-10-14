# Smoke check of the kanban app as built for release, on Windows: it starts on a few demo tasks, stays up, keeps
# idle (nothing running, no message showing: it redraws once a minute), and a screenshot of the screen is saved for
# a person to look at. Linux and macOS have the same check in e2e/smoke-app.sh.
#
# Usage: e2e/smoke-app.ps1 [APP] [OUT]: APP is the app's binary (target\release\kanbrain.exe), OUT the folder the
# screenshot goes to (target\smoke).
param(
    [string]$App = (Join-Path (Split-Path $PSScriptRoot -Parent) 'target\release\kanbrain.exe'),
    [string]$Out = (Join-Path (Split-Path $PSScriptRoot -Parent) 'target\smoke')
)
$ErrorActionPreference = 'Stop'
# Percent of one core the idle app may use, averaged over the measure.
$idleLimit = 3
$work = Join-Path ([IO.Path]::GetTempPath()) "kb-smoke-$(Get-Random)"
$env:KANBRAIN_DIR = Join-Path $work 'central'
foreach ($var in 'CLAUDECODE', 'AI_AGENT', 'AGENT', 'KANBRAIN_AUTHOR') { Remove-Item "Env:$var" -ErrorAction SilentlyContinue }
New-Item -ItemType Directory -Force -Path $env:KANBRAIN_DIR, $Out | Out-Null
$process = $null

function Fail($why) { throw "FAIL: $why" }

# The app takes kb's commands with arguments, through Start-Process: it has no console, so `&` wouldn't wait.
function Invoke-App([string[]]$arguments) {
    $log = Join-Path $work 'out.txt'
    $run = Start-Process -FilePath $App -ArgumentList $arguments -Wait -NoNewWindow -PassThru -RedirectStandardOutput $log
    if ($run.ExitCode -ne 0) { Fail "kanbrain $arguments failed" }
    Get-Content $log -Raw
}

try {
    Invoke-App @('add', '--tag', 'auth', '"Rate-limit the login endpoint"') | Out-Null
    Invoke-App @('add', '--note', '--body', '"Retries are idempotent."', '"Webhooks arrive more than once"') | Out-Null
    $id = ([regex]::Match((Invoke-App @('add', '"Paginate GET /orders"')), '#(\d+)')).Groups[1].Value
    Invoke-App @('done', $id) | Out-Null

    $process = Start-Process -FilePath $App -PassThru
    # Its first frames, fonts loaded.
    Start-Sleep -Seconds 15
    if ($process.HasExited) { Fail "the app exited ($($process.ExitCode))" }

    $process.Refresh()
    $before = $process.TotalProcessorTime
    Start-Sleep -Seconds 20
    $process.Refresh()
    $used = [math]::Round(($process.TotalProcessorTime - $before).TotalSeconds / 20 * 100)
    Write-Host "idle: $used% of a core"
    if ($used -gt $idleLimit) { Fail "the idle app used $used% of a core, over $idleLimit%" }

    # A runner without an interactive desktop can't capture it: the app's check stands without it.
    try {
        Add-Type -AssemblyName System.Windows.Forms, System.Drawing
        $screen = [Windows.Forms.Screen]::PrimaryScreen.Bounds
        $shot = New-Object Drawing.Bitmap $screen.Width, $screen.Height
        [Drawing.Graphics]::FromImage($shot).CopyFromScreen($screen.Location, [Drawing.Point]::Empty, $screen.Size)
        $shot.Save((Join-Path $Out 'app-windows.png'), [Drawing.Imaging.ImageFormat]::Png)
    } catch {
        Write-Host "no screenshot: $($_.Exception.Message)"
    }
    if ($process.HasExited) { Fail 'the app exited while idle' }
    Write-Host 'PASS smoke-app.ps1'
} finally {
    if ($process -and -not $process.HasExited) { Stop-Process -Id $process.Id -Force }
    Remove-Item $work -Recurse -Force -ErrorAction SilentlyContinue
}
