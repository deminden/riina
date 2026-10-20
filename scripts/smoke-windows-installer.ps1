[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Installer,
    [Parameter(Mandatory)][string]$Inventory,
    [Parameter(Mandatory)][string]$Fixture,
    [Parameter(Mandatory)][string]$ReportPath,
    [switch]$HostedGraphics
)
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true
if (Get-Process -Name riina -ErrorAction SilentlyContinue) { throw 'Close Riina before installer verification to keep profile preservation checks isolated' }
$Installer = (Resolve-Path -LiteralPath $Installer).Path
$Inventory = (Resolve-Path -LiteralPath $Inventory).Path
$Fixture = (Resolve-Path -LiteralPath $Fixture).Path
$ReportPath = [IO.Path]::GetFullPath($ReportPath)
$record = Get-Content -LiteralPath $Inventory -Raw | ConvertFrom-Json
if ((Get-FileHash -LiteralPath $Installer).Hash.ToLower() -ne $record.installer_sha256) { throw 'Installer checksum differs' }
if (Test-Path -LiteralPath $ReportPath) { throw 'Refusing a stale installer report' }
$uninstallKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\{575C5AEC-C763-49F6-BF2A-0D34BB908A94}_is1'
if (Test-Path -LiteralPath $uninstallKey) { throw 'An installed Riina already exists; do not replace the user installation in a smoke test' }
$root = Join-Path ([IO.Path]::GetTempPath()) ('riina-installer-' + [guid]::NewGuid())
$destination = Join-Path $root 'Unicode test é\Riina player'
$group = 'Riina'
$shortcutFolder = Join-Path ([Environment]::GetFolderPath('Programs')) $group
$desktopShortcut = Join-Path ([Environment]::GetFolderPath('Desktop')) 'Riina.lnk'
if ((Test-Path -LiteralPath $shortcutFolder) -or (Test-Path -LiteralPath $desktopShortcut)) {
    throw 'Existing Riina shortcuts must not be replaced during an installer smoke test'
}
$profile = Join-Path $env:LOCALAPPDATA 'riina'
function Profile-Hashes {
    $hashes = @{}
    if (Test-Path -LiteralPath $profile) {
        Get-ChildItem -LiteralPath $profile -Recurse -File | ForEach-Object {
            $hashes[$_.FullName] = (Get-FileHash -LiteralPath $_.FullName).Hash
        }
    }
    return $hashes
}
$profileBefore = Profile-Hashes
New-Item -ItemType Directory -Path $root -Force | Out-Null
function Run-Setup([string]$Stage, [string]$Tasks = '') {
    $process = Start-Process -FilePath $Installer -WindowStyle Hidden -PassThru -ArgumentList @(
        '/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART','/SP-',
        ('/DIR="' + $destination + '"'),('/GROUP="' + $group + '"'),
        ('/TASKS="' + $Tasks + '"'),('/LOG="' + (Join-Path $root ($Stage + '.log')) + '"'))
    if (!$process.WaitForExit(90000)) { Stop-Process -Id $process.Id; throw 'Installer timed out' }
    if ($process.ExitCode -ne 0) { throw "Installer $Stage failed: $($process.ExitCode). Logs: $root" }
}
function Verify-Payload {
    foreach ($entry in $record.payload_files.psobject.Properties) {
        $file = Join-Path $destination $entry.Name
        if (!(Test-Path -LiteralPath $file -PathType Leaf) -or
            (Get-FileHash -LiteralPath $file).Hash.ToLower() -ne $entry.Value) {
            throw "Installed payload differs: $($entry.Name)"
        }
    }
    $expected = @($record.payload_files.psobject.Properties.Name)
    foreach ($file in Get-ChildItem -LiteralPath $destination -Recurse -File) {
        $relative = [IO.Path]::GetRelativePath($destination, $file.FullName).Replace('\','/')
        if ($relative -notin $expected -and $relative -notmatch '^unins[0-9]+\.(exe|dat|msg)$') {
            throw "Unexpected installed payload: $relative"
        }
    }
    $registration = Get-ItemProperty -LiteralPath $uninstallKey
    if ($registration.DisplayVersion -ne $record.version -or $registration.InstallLocation.TrimEnd('\') -ne $destination) {
        throw 'Windows uninstall registration differs'
    }
    $shortcut = Join-Path $shortcutFolder 'Riina.lnk'
    if (!(Test-Path -LiteralPath $shortcut)) { throw 'Start menu shortcut is missing' }
    $shell = New-Object -ComObject WScript.Shell
    if ($shell.CreateShortcut($shortcut).TargetPath -ne (Join-Path $destination 'riina.exe')) { throw 'Start menu target differs' }
}
function Run-Uninstall {
    $uninstaller = Join-Path $destination 'unins000.exe'
    if (Test-Path -LiteralPath $uninstaller) {
        $process = Start-Process -FilePath $uninstaller -WindowStyle Hidden -PassThru -ArgumentList @(
            '/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART',('/LOG="' + (Join-Path $root 'uninstall.log') + '"'))
        if (!$process.WaitForExit(90000)) { Stop-Process -Id $process.Id; throw 'Uninstall timed out' }
        if ($process.ExitCode -ne 0) { throw 'Uninstall failed' }
    }
}
try {
    Run-Setup 'install'
    Verify-Payload
    if (Test-Path -LiteralPath $desktopShortcut) { throw 'Desktop shortcut must be opt-in' }
    # Reinstall exercises stable identity and same-version replacement, before the
    # test-only hosted graphics files are added to the installed directory.
    Run-Setup 'upgrade' 'desktopicon'
    Verify-Payload
    if (!(Test-Path -LiteralPath $desktopShortcut)) { throw 'Optional desktop shortcut is missing' }
    if ($HostedGraphics) {
        python (Join-Path $PSScriptRoot 'prepare-windows-ci-graphics.py') --application $destination --archive (Join-Path $root 'mesa.7z')
    }
    & (Join-Path $PSScriptRoot 'smoke-windows-package.ps1') -Executable (Join-Path $destination 'riina.exe') -Fixture $Fixture -ReportPath ($ReportPath + '.playback.json')
    # Remove only the two files staged by the hosted check. They are not installer
    # payload, so the uninstaller must never be responsible for deleting them.
    if ($HostedGraphics) {
        foreach ($name in @('opengl32.dll','libgallium_wgl.dll')) {
            Remove-Item -LiteralPath (Join-Path $destination $name)
        }
    }
    Run-Uninstall
    if ((Test-Path -LiteralPath (Join-Path $destination 'riina.exe')) -or
        (Test-Path -LiteralPath $uninstallKey) -or (Test-Path -LiteralPath $shortcutFolder) -or
        (Test-Path -LiteralPath $desktopShortcut)) {
        throw 'Uninstall left application files, registration or shortcuts'
    }
    $profileAfter = Profile-Hashes
    if ($profileBefore.Count -ne $profileAfter.Count) { throw 'User profile changed during installer test' }
    foreach ($name in $profileBefore.Keys) {
        if ($profileBefore[$name] -ne $profileAfter[$name]) { throw 'User preferences/history changed during installer test' }
    }
    @{
        version = $record.version; source_commit = $record.source_commit
        installer_sha256 = $record.installer_sha256
        installed_payload_matches_portable = $true; same_version_reinstall = $true
        start_menu_shortcut = $true; uninstall_registration = $true
        desktop_shortcut_opt_in = $true
        installed_application_smoke = $true; uninstall = $true; profile_preserved = $true
        hosted_software_graphics = [bool]$HostedGraphics
    } | ConvertTo-Json | Set-Content -LiteralPath $ReportPath -Encoding utf8NoBOM
    Write-Output "Installer install/reinstall/playback/uninstall passed: $ReportPath"
} finally {
    Run-Uninstall
}
