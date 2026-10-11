param([string[]]$CargoArguments)
# Positional calls bind the first argument to this array and leave the remainder
# in $args. Preserve flags such as --release instead of silently building debug.
$CargoArguments = @($CargoArguments) + @($args)
if ($CargoArguments.Count -eq 0) { $CargoArguments = @('build','--workspace','--locked') }
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
Set-Location $root
# These variables affect this process only. Ordinary MSVC developer shells need no fallback.
if (Test-Path "$root/tools/cargo/bin/cargo.exe") {
    $env:CARGO_HOME = "$root/tools/cargo"
    $env:RUSTUP_HOME = "$root/tools/rustup"
    $env:PATH = "$root/tools/cargo/bin;" + $env:PATH
}
if (!(Get-Command link.exe -ErrorAction SilentlyContinue) -and (Test-Path "$root/tools/sdk-lib/kernel32.lib")) {
    $taskSysroot = (& rustc --print sysroot).Trim()
    if ($LASTEXITCODE -ne 0) { throw 'The pinned Rust toolchain is unavailable.' }
    $env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER = "$taskSysroot/lib/rustlib/x86_64-pc-windows-msvc/bin/rust-lld.exe"
    $env:LIB = "$root/tools/sdk-lib;$root/tools/vc-crt/Contents/VC/Tools/MSVC/14.44.35207/lib/x64"
}
& cargo @CargoArguments
exit $LASTEXITCODE
