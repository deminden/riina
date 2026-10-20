# Riina

<!-- 4:20 — a good time to press play. -->

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/branding/svg/riina-logo-horizontal-dark.svg">
  <img src="assets/branding/svg/riina-logo-horizontal-light.svg" alt="Riina" width="420">
</picture>

**An independent Rust media player bringing the IINA experience to Windows.**

Riina follows the interface and behavior of **IINA 1.5.0**, with libmpv for
playback and Slint for the interface. Windows 11 x64 is the supported platform;
macOS and Linux application ports are in development.

[![Latest release](https://img.shields.io/github/v/release/deminden/riina)](https://github.com/deminden/riina/releases/latest)

[**Download for Windows**](https://github.com/deminden/riina/releases/latest) ·
[What's working](docs/status.md) ·
[Build from source](#build-from-source) ·
[Documentation](docs/README.md)

## Watch, listen, adjust

- **Play files, folders and URLs.** Use playlists, chapters, playback history and
  resume to pick up where you left off.
- **Keep playback controls close.** Seek, change speed, repeat a section, or open
  Quick Settings from the player. Music Mode and Picture in Picture provide
  alternative ways to keep media playing.
- **Choose your tracks.** Select audio and subtitles, load external tracks, adjust
  timing, and change subtitle appearance.
- **Tune the picture and sound.** Adjust aspect ratio, crop, rotation, video
  filters and audio equalization. Capture screenshots and preview the timeline.
- **Make it fit your workflow.** Configure shortcuts, sidebars and Preferences.
  Volume defaults to 0–100%; an optional boost extends the range to 200%.

Plugin support, online subtitles and online-media helpers are implemented, with
compatibility and service coverage still in progress. See the
[feature inventory](docs/parity.csv) and [plugin compatibility](docs/plugin-compatibility.csv)
for individual requirements and limitations.

## Download and run

1. Open the [latest release](https://github.com/deminden/riina/releases/latest).
2. Download the **Windows x86_64 ZIP** from the release assets.
3. Extract the whole archive to a folder, then run `riina.exe`.
4. Open a media file through the player or drag it onto the window.

Keep the executable beside the libraries and other files supplied in the archive.
The portable package includes its playback engine; you do not need a separate mpv
installation. Available formats depend on the bundled engine.

The release page includes checksums, release notes and a verification report.
GitHub's automatically generated source archives contain source code, not a
ready-to-run application.

## Recent improvements

Quick Settings has consistent numeric editing, keyboard track navigation and
equalizer profile naming. Play symbols have an optical centering correction,
Music Mode keeps its compact frame, and playlist editing and history resume
handle more everyday workflows. Narrow settings pages retain their scroll positions.

See the [4.2.0 release notes](docs/release-notes/4.2.0.md) for the complete changes;
[GitHub Releases](https://github.com/deminden/riina/releases) shows published packages.

## Where the project stands

Riina is a working player under active development. Full IINA feature, behavior
and visual parity remain the goal; they are not a claim of completion.

| Platform | Availability |
|---|---|
| **Windows 11 x64** | Supported application and portable releases; native D3D11 video by default |
| macOS | Application port in development; no supported release yet |
| Linux | Application port in development; no supported release yet |
| Other Windows versions and ARM64 | Not verified |

Current work includes interface consistency, missing IINA behaviors, plugin
compatibility and application ports. HDR/color validation, broader DPI and
multi-monitor coverage, accessibility and localization still need work.

[Current status](docs/status.md) summarizes evidence and known gaps. The
[implementation plan](docs/implementation-plan.md) tracks the work order;
[verification](docs/verification.md) records what was actually checked.

## Build from source

On Windows, install **Rust 1.99.0**, **MSVC C++ build tools**, the **Windows SDK**
and **7-Zip**. The Rust toolchain and dependencies are pinned in the repository.

From the repository root:

```powershell
./scripts/fetch-mpv.ps1
cargo build --workspace --locked
cargo run -p riina --locked -- "C:\media\movie.mkv"
```

The setup script downloads the pinned development engine. To inspect the pinned
IINA reference for compatibility work, also run `./scripts/fetch-reference.ps1`.
The reference checkout is separate from the application and is not needed to run it.

See [Development](docs/development.md) for workspace-local tool setup, architecture,
checks and release packaging. See [Port development](docs/port-development.md) for
macOS and Linux. The optional `--shared-gl` flag selects the shared SDR renderer.

## Help improve Riina

[Report a bug or request a feature](https://github.com/deminden/riina/issues).
For a playback or interface problem, include the Riina version, Windows version,
steps to reproduce and the expected result. Screenshots are useful for layout
issues; check them for private information before sharing.

Contributions are welcome. Start with the [development guide](docs/development.md)
and [project instructions](AGENTS.md). Compatibility work targets the pinned
IINA 1.5.0 baseline; behavior and visual comparisons should have reproducible evidence.

## License and credits

Riina's independently written source is [MIT licensed](LICENSE). The combined
application package is distributed under [GPLv3 terms](LICENSE-GPL-3.0.txt).
Third-party components retain their own licenses and
[notices](THIRD_PARTY_NOTICES.md).

Riina takes its compatibility baseline from IINA and uses libmpv for playback and
[Slint](https://slint.dev) for its interface. It is an independent project.

[![Made with Slint](https://raw.githubusercontent.com/slint-ui/slint/master/logo/MadeWithSlint-logo-whitebg.png)](https://slint.dev)

Each release identifies its application source through an immutable Git tag.
The packaged `SOURCE-MANIFEST.tsv` and `SOURCE-ACCESS.md` identify native sources
and rebuild instructions; `Cargo.lock` pins Rust dependencies. See
[licensing](docs/licensing.md) and [source access](docs/SOURCE-ACCESS.md) for details.
