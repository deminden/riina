# Riina

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/branding/svg/riina-logo-horizontal-dark.svg">
  <img src="assets/branding/svg/riina-logo-horizontal-light.svg" alt="Riina logo" width="420">
</picture>

Riina is a Rust media player targeting the behavior and interface of official
IINA v1.5.0. Windows 11 x64 is the supported platform. macOS and Linux ports
are in development and are not yet supported releases.

[![Latest release](https://img.shields.io/github/v/release/deminden/riina)](https://github.com/deminden/riina/releases/latest)

The implementation uses Slint for the interface and libmpv for media
playback. Windows video output uses D3D11; the shared OpenGL route is available
with `--shared-gl`. The architecture keeps application behavior separate from
platform-specific windowing and video output.

## Status

The application includes local and URL playback, playlists and history,
audio/subtitle selection, video controls, screenshots, Quick Settings and
Preferences, Music Mode, Picture in Picture and plugin workflows. Coverage and
verification vary by feature and platform.
Volume is limited to 100% by default; an optional setting allows up to 200%.

Behavioral and visual parity with IINA v1.5.0 remain acceptance goals. Some
IINA features, interface details, and platform behaviors are not implemented or
verified yet. Current evidence and known gaps are listed in
[project status](docs/status.md) and the [parity inventory](docs/parity.csv).
HDR/color calibration, matched visual comparisons and broader window/DPI
coverage remain open.

The [latest GitHub Release](https://github.com/deminden/riina/releases/latest)
contains the Windows portable binary. Its corresponding Riina source is available
from the exact release tag; `SOURCE-MANIFEST.tsv` and `SOURCE-ACCESS.md` in the
package identify the pinned native sources and rebuild instructions.

## Build on Windows

Requirements: Windows 11 x64, Rust 1.99.0, MSVC C++ build tools, Windows SDK,
and 7-Zip.

```powershell
./scripts/fetch-reference.ps1
./scripts/fetch-mpv.ps1
cargo build --workspace --locked
cargo run -p riina --locked -- "C:\media\movie.mkv"
```

See [Development](docs/development.md) for setup details, workspace-local tools,
repository layout and release preparation.

## Verification

The [verification guide](docs/verification.md) lists check commands, native
prerequisites, GUI procedures and revision-specific results. Automated workspace
checks do not establish physical-display, audible-output or HDR correctness.

## Documentation

- [Documentation index](docs/README.md)
- [Current status](docs/status.md) and [implementation plan](docs/implementation-plan.md)
- [Product contract](docs/product-spec.md) and [parity inventory](docs/parity.csv)

## Contributors and licensing

Contributions are welcome. GitHub maintains the
[contributor record](https://github.com/deminden/riina/graphs/contributors).

Riina's independently written source is licensed under [MIT](LICENSE). The
combined application package is distributed under GPLv3; see
[LICENSE-GPL-3.0.txt](LICENSE-GPL-3.0.txt) and the
[distribution policy](docs/licensing.md). Third-party components retain their
own licenses and [notices](THIRD_PARTY_NOTICES.md).
