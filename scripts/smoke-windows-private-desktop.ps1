[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Executable,
    [Parameter(Mandatory)][string]$Fixture,
    [Parameter(Mandatory)][string]$ReportPath,
    [Parameter(DontShow)][string]$DesktopName
)
$ErrorActionPreference = 'Stop'
if (!('RiinaPrivateDesktop' -as [type])) {
    Add-Type -Path (Join-Path $PSScriptRoot 'smoke-windows-private-desktop.cs')
}
$log = $ReportPath + '.desktop-driver.log'
if ($DesktopName) {
    if ([RiinaPrivateDesktop]::CurrentThreadDesktop() -ne $DesktopName -or
        [RiinaPrivateDesktop]::CurrentInputDesktop() -eq $DesktopName) {
        throw 'The smoke driver is outside its isolated desktop.'
    }
    try {
        & (Join-Path $PSScriptRoot 'smoke-windows-package.ps1') -Executable $Executable -Fixture $Fixture -ReportPath $ReportPath *> $log
        exit 0
    } catch {
        $_ | Out-String | Add-Content -LiteralPath $log
        exit 1
    }
}

$name = 'RiinaSmoke' + [guid]::NewGuid().ToString('N')
$inputDesktop = [RiinaPrivateDesktop]::CurrentInputDesktop()
$driver = (Get-Command pwsh.exe).Source
$arguments = [string[]]@(
    '-NoProfile', '-NonInteractive', '-File', $PSCommandPath,
    '-Executable', $Executable, '-Fixture', $Fixture, '-ReportPath', $ReportPath,
    '-DesktopName', $name
)
New-Item -ItemType Directory -Path (Split-Path -Parent $ReportPath) -Force | Out-Null
try {
    $result = [RiinaPrivateDesktop]::Run($name, $driver, $arguments, $PSScriptRoot, 75000)
} finally {
    if ([RiinaPrivateDesktop]::CurrentInputDesktop() -ne $inputDesktop) {
        throw 'The active Windows desktop changed during the smoke test.'
    }
    if (Test-Path -LiteralPath $log) {
        Get-Content -LiteralPath $log
    }
}
if ($result -ne 0) {
    throw "Isolated smoke driver exited with status $result. Diagnostics at $log"
}
