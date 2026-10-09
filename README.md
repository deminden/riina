# riina

Rust IINA reimplementation, targeting
official **IINA v1.5.0**. This is an early development build, not a parity release.

Windows 11 is the current supported platform; macOS and Linux ports are near-term work.
Feature parity and pixel-perfect interface parity are primary goals, with original
MIT code and assets. Volume is capped at 100% by default; an optional audio setting permits up to 200%.

Application code is Rust (edition 2024); Slint supplies the retained UI and libmpv
supplies real playback. The default renderer uses a shared OpenGL texture.
There is no web frontend or telemetry.

On Windows 11 x64, install Rust 1.99.0 and MSVC C++ build tools with Windows SDK.
Run `./scripts/fetch-reference.ps1`, then `./scripts/fetch-mpv.ps1` (requires 7-Zip).
Build with `cargo build --workspace --locked`; run with
`cargo run -p riina --locked -- "C:\media\movie.mkv"`.
The executable uses the pinned development DLL or a DLL beside the executable.
`./scripts/dev.ps1` also supports the workspace-local tools prepared during development.

Implemented slice: local-file dialog with batch selection, file filtering and
natural ordering; CLI file opening; play/pause, stop, seeking, volume/mute, speed,
frame step, timeline and fullscreen. Chapter, playlist, audio and subtitle sidebars
support selection, playlist editing, external tracks and timing offsets. Verification
and outstanding gaps are recorded in [status](docs/status.md). Open controls are
enabled only after the renderer is ready. Preferences live in `%LOCALAPPDATA%/riina`.

See [product specification](docs/product-spec.md), [source audit](docs/upstream-audit.md),
[visual specification](docs/visual-spec.md), and [third-party notices](THIRD_PARTY_NOTICES.md).
Native binary redistribution is not ready; this checkout is a development workspace.
Original riina source is [MIT licensed](LICENSE). Third-party dependencies and
references retain their own licenses; see [third-party notices](THIRD_PARTY_NOTICES.md)
and the [independent implementation policy](docs/licensing.md). The MIT grant does
not relicense IINA or the development libmpv DLL. Binary distribution remains gated
on the dependency and corresponding-source audit.

[![Made with Slint](https://raw.githubusercontent.com/slint-ui/slint/v1.13.1/logo/MadeWithSlint-logo-whitebg.png)](https://slint.dev)
The repository contains source and original test fixtures. Downloaded tools, native
binaries and raw desktop captures stay local. Evidence summaries describe their
tested binary and known limitations; screenshot paths refer to local observations.

The current release target is still in development: full IINA behavior, preferences,
menus, plugins, visual fidelity and broad hardware/DPI coverage remain incomplete.
Issues and focused contributions are welcome. Include reproduction steps, Windows
version and media format when reporting a bug; use a shareable sample where possible.

Native renderer experiment: `cargo run -p riina --bin d3d11-probe --locked --
"C:\media\movie.mkv"`. This separate development surface tests D3D11 video with
owned Slint controls and writes `tools/d3d11-probe.log`. Its current evidence and
remaining HDR gates are in [the experiment record](docs/adr/0002-native-d3d11-experiment.md).
Pass several media paths to cycle them with Next / Reload. Stop and keyboard
Tab/Enter allow testing native output destruction/recreation. The fixture set
includes original PQ and HLG 10-bit samples; the ordinary player still uses its
provisional SDR renderer.

To test the native HDR candidate with the shared player controls and sidebars:
`cargo run -p riina --locked -- --native-video "C:\media\movie.mkv"`.
This option uses a native D3D11 video host and the same player UI in a transparent
owned window. Renderer logs go to `tools/native-player.log`. It remains experimental;
calibrated HDR color, video blur and complete window/DPI coverage are open gates.
