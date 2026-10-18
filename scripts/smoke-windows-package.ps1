[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Executable,
    [Parameter(Mandatory)][string]$Fixture,
    [Parameter(Mandatory)][string]$ReportPath,
    [switch]$CaptureDesktop
)
$ErrorActionPreference = 'Stop'
$Executable = (Resolve-Path -LiteralPath $Executable).Path
$Fixture = (Resolve-Path -LiteralPath $Fixture).Path
$ReportPath = [System.IO.Path]::GetFullPath($ReportPath)
$root = Join-Path ([System.IO.Path]::GetTempPath()) ('riina-package-smoke-' + [guid]::NewGuid())
$profile = Join-Path $root 'profile'
$media = Join-Path $root 'media'
New-Item -ItemType Directory -Path (Join-Path $profile 'riina'),$media -Force | Out-Null
$fixtureCopy = Join-Path $media ([System.IO.Path]::GetFileName($Fixture))
Copy-Item -LiteralPath $Fixture -Destination $fixtureCopy
# The null audio sink keeps the real video/window path usable on runners without
# speakers. It is scoped to this disposable profile, never the user's settings.
@{
    pause_when_open = $false
    fullscreen_when_open = $false
    playlist_auto_add = $false
    quit_when_no_opened_window = $true
    advanced = @{ enabled = $true; user_options = @(@{name='ao';value='null'},@{name='loop-file';value='inf'}) }
} | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $profile 'riina/preferences.json') -Encoding utf8NoBOM
$previousLocal = $env:LOCALAPPDATA
$previousReport = $env:RIINA_GUI_SMOKE_REPORT
$process = $null
function Capture-SmokeDesktop([string]$Stage) {
    $capture = & (Join-Path $PSScriptRoot 'capture-windows-desktop.ps1') -OutputPath ($ReportPath + '.' + $Stage + '.png')
    if (!$capture.HasNonBlackProbe -or $capture.Width -lt 1 -or $capture.Height -lt 1) { throw "Invalid full-desktop capture at $Stage" }
    Write-Output $capture
}
try {
    if (Test-Path -LiteralPath $ReportPath) { throw 'Refusing to use a stale smoke report; select a new report path.' }
    $env:LOCALAPPDATA = $profile
    $env:RIINA_GUI_SMOKE_REPORT = $ReportPath
    if ($CaptureDesktop) { Capture-SmokeDesktop 'before' }
    $process = Start-Process -FilePath $Executable -ArgumentList @('"' + $fixtureCopy + '"') -PassThru `
        -RedirectStandardError (Join-Path $root 'stderr.log') -RedirectStandardOutput (Join-Path $root 'stdout.log')
    $clock = [System.Diagnostics.Stopwatch]::StartNew()
    $captured = $false
    while (!$process.WaitForExit(200)) {
        if ($clock.Elapsed.TotalSeconds -gt 60) { throw 'Packaged application did not complete playback/fullscreen/shutdown smoke within 60 seconds.' }
        if ($CaptureDesktop -and !$captured -and (Test-Path -LiteralPath (Join-Path $root 'stderr.log')) -and
            (Get-Content -LiteralPath (Join-Path $root 'stderr.log') -Raw) -match 'riina-gui-smoke: fullscreen controller revealed') {
            Capture-SmokeDesktop 'fullscreen'
            $captured = $true
        }
    }
    if ($CaptureDesktop -and !$captured) { throw 'No fullscreen-controller desktop capture was made.' }
    if ($process.ExitCode -ne 0) { throw "Packaged application exited with code $($process.ExitCode)." }
    if (!(Test-Path -LiteralPath $ReportPath)) { throw "Application produced no smoke report. Diagnostics: $root" }
    & python (Join-Path $PSScriptRoot 'check-gui-smoke.py') $ReportPath --executable $Executable --pid $process.Id --fixture $Fixture
    if ($LASTEXITCODE -ne 0) { throw "Invalid application smoke evidence. Diagnostics: $root" }
    $saved = Get-Content -LiteralPath (Join-Path $profile 'riina/preferences.json') -Raw | ConvertFrom-Json
    if ($saved.video_threads -ne 7 -or $saved.load_icc_profile -ne $false -or $saved.audio.audio_threads -ne 3 -or
        $saved.network.user_agent -ne 'Riina GUI smoke' -or $saved.hdr_tone_mapping.algorithm -ne 'hable') {
        throw 'Normal application shutdown overwrote settings saved during the session.'
    }
    Write-Output "Packaged app smoke passed. Physical-display/HDR verification is separate. Report: $ReportPath"
} finally {
    $env:LOCALAPPDATA = $previousLocal
    $env:RIINA_GUI_SMOKE_REPORT = $previousReport
    if ($process -and !$process.HasExited) {
        # Terminate only the exact test instance; no other player's process is touched.
        Stop-Process -Id $process.Id -ErrorAction SilentlyContinue
        $process.WaitForExit(5000) | Out-Null
    }
    if ($CaptureDesktop) { Capture-SmokeDesktop 'after' }
}
