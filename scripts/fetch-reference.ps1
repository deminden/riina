$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$destination = Join-Path $root 'reference/iina-upstream'
$commit = 'fbcff4df7183b335880396af0655f6bb89d46442'
if (!(Test-Path $destination)) {
    & git clone --branch v1.5.0 --depth 1 https://github.com/iina/iina.git $destination
    if ($LASTEXITCODE) { throw 'Official reference clone failed; use the official v1.5.0 archive and record its SHA-256.' }
}
$remote = & git -C $destination remote get-url origin
$actual = & git -C $destination rev-parse HEAD
$tag = & git -C $destination describe --tags --exact-match
$dirty = & git -C $destination status --porcelain
if ($remote -ne 'https://github.com/iina/iina.git' -or $actual -ne $commit -or $tag -ne 'v1.5.0' -or $dirty) {
    throw 'Reference differs from the pinned baseline. Preserving the checkout; inspect it manually.'
}
Write-Output "Verified IINA v1.5.0 $actual"
