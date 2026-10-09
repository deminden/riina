$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$url = 'https://github.com/shinchiro/mpv-winbuild-cmake/releases/download/20261009/mpv-dev-x86_64-20261009-git-36abaa32d0.7z'
$hash = '8a15fc07996a33235778256157151c42cbaceee976f0a98860f55047856935b2'
New-Item -ItemType Directory -Force "$root/tools" | Out-Null
$archive = "$root/tools/mpv-dev.7z"
if (!(Test-Path $archive)) { Invoke-WebRequest $url -OutFile $archive }
if ((Get-FileHash $archive -Algorithm SHA256).Hash.ToLowerInvariant() -ne $hash) { throw 'Pinned mpv archive integrity mismatch' }
$extractor = "$root/tools/7zip-full/7z.exe"
if (!(Test-Path $extractor)) { $extractor = (Get-Command 7z.exe -ErrorAction Stop).Source }
& $extractor x $archive "-o$root/tools/mpv" -y
if ($LASTEXITCODE) { throw 'mpv extraction failed' }
# The hash came from the release API, not an independently signed attestation.
# Development use only until binary dependencies and corresponding source are audited.
