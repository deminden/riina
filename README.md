# Riina

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/branding/svg/riina-logo-horizontal-dark.svg">
  <img src="assets/branding/svg/riina-logo-horizontal-light.svg" alt="Riina logo" width="420">
</picture>

Riina is a Rust media player targeting the behavior and interface of official
IINA v1.5.0. Windows 11 x64 is the supported platform. macOS and Linux ports
are in development and are not yet supported releases.

[![Latest release](https://img.shields.io/github/v/release/deminden/riina)](https://github.com/deminden/riina/releases/latest)

The current implementation uses Slint for the interface and libmpv for media
playback. Windows video output uses D3D11; the shared OpenGL route is available
with `--shared-gl`. The architecture keeps application behavior separate from
platform-specific windowing and video output.

## Status

The application supports local media playback, file selection, play/pause,
stop, seeking, volume and mute, playback speed, frame stepping, fullscreen,
chapters, playlists, audio and subtitle track selection, subtitle timing,
video aspect and transform controls, screenshots, and General preferences.
Volume is limited to 100% by default; an optional setting allows up to 200%.

Behavioral and visual parity with IINA v1.5.0 remain acceptance goals. Some
IINA features, interface details, and platform behaviors are not implemented or
verified yet. Current evidence and known gaps are listed in
[project status](docs/status.md) and the [parity inventory](docs/parity.csv).
HDR color calibration, video blur, and full window and DPI coverage remain
unverified.

The [latest GitHub Release](https://github.com/deminden/riina/releases/latest)
currently contains source archives. A Windows portable binary will be attached
after its playback, dependency notices, and corresponding-source bundle pass
release checks.

## Contributors

Contributions are welcome. GitHub maintains the current list on the
[contributors graph](https://github.com/deminden/riina/graphs/contributors).

## Build on Windows

Requirements: Windows 11 x64, Rust 1.99.0, MSVC C++ build tools, Windows SDK,
and 7-Zip.

```powershell
./scripts/fetch-reference.ps1
./scripts/fetch-mpv.ps1
cargo build --workspace --locked
cargo run -p riina --locked -- "C:\media\movie.mkv"
```

The development build uses the pinned local libmpv files. `./scripts/dev.ps1`
also supports workspace-local tools prepared during development.

## Verification

Run the Rust checks from the repository root:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

Native playback tests require the pinned Windows libmpv bundle. GUI and HDR
behavior must be tested on the corresponding desktop hardware; automated
workspace checks do not establish those results.

## Project references

- [Product specification](docs/product-spec.md)
- [Parity inventory](docs/parity.csv)
- [Visual specification](docs/visual-spec.md)
- [Brand usage and asset notices](assets/branding/BRAND_GUIDE.md)
- [Port development](docs/port-development.md)
- [Upstream behavior audit](docs/upstream-audit.md)
- [Third-party notices](THIRD_PARTY_NOTICES.md)
- [Native D3D11 presentation record](docs/adr/0002-native-d3d11-experiment.md)

Riina's independently written source is licensed under [MIT](LICENSE). The
combined application package is distributed under GPLv3; see
[LICENSE-GPL-3.0.txt](LICENSE-GPL-3.0.txt) and the
[distribution policy](docs/licensing.md). Third-party components retain their
own licenses and notices.
