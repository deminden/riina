# Current status

Updated 2026-10-19. Development version: **0.4.0**. Reference: **IINA v1.5.0**
at `fbcff4df7183b335880396af0655f6bb89d46442`.

Riina is a functioning media player. Full feature, behavior and visual parity
remain active goals; the [requirement inventory](parity.csv) is incomplete.

## Platforms

| Platform | State |
|---|---|
| Windows 11 x64 | Supported application target; native D3D11 video is the default |
| macOS | Application port in development; full build, visible playback and lifecycle acceptance remain open |
| Linux | Application port in development; full desktop playback and lifecycle acceptance remain open |
| Other Windows versions and ARM64 | Not verified |

`--shared-gl` selects the shared OpenGL path. Its SDR output and Windows HDR
composition limitations are documented in the [rendering decisions](README.md#architecture-and-ports).

## Implemented workflows

Implementation does not imply every state or platform is verified.

- **Playback:** files, folders and URLs; play/pause, seeking, volume/mute, speed,
  frame stepping, loops, chapters, playlist editing, history and resume.
- **Video/audio/subtitles:** track selection and external tracks, geometry and
  crop controls, filters/equalizers, subtitle styling/timing, screenshots,
  thumbnails, Music Mode and Picture in Picture.
- **Interface:** floating/top/bottom controls, Quick Settings, configurable
  sidebars, native menus and shortcut hints, Welcome and Preferences windows.
  All eleven Preferences categories have implementation; completeness varies.
- **Extensions and services:** plugin management, QuickJS host APIs and WebView
  surfaces, online-subtitle providers, online-media helpers, OCR and system-media
  adapters. Compatibility and platform coverage remain partial.

Volume defaults to 0-100%; the optional boost setting allows 200%.

## Recent reliability work

- Preference saves merge changed fields under a cross-process lock, preserve
  unrelated newer settings, and remain retryable after failed writes.
- Accepted playback commands keep FIFO order; a full queue returns an error.
- Plugin HTTP transports share redirect validation and credential handling.
- Windows volume changes update the native menu label in place. Fullscreen and
  Music Mode retain and detach/restore the native menu.
- The fullscreen controls surface is one physical scanline shorter than the
  native video surface. The user confirmed visible video on controller reveal
  with this workaround; other drivers and HDR remain separate checks.
- Release validation exercises the extracted executable, persisted settings and
  clean shutdown, including when a matching native-engine artifact is reused.
- Preferences standard controls use the same light palette as their panels.
  Audio sections share one scroll page; auxiliary windows retain their width
  when only their height needs to fit the desktop.
- Quick Settings keeps every category reachable in narrow windows. Audio uses
  one scroll page with horizontal equalizer scrolling when required. Unused panel
  input no longer changes playback volume or dismisses the sidebar.
- Light Quick Settings tints its sampled video before drawing controls. Fullscreen
  Escape also works when the framed playback host has keyboard focus.

## Verification summary

The [0.4.0 candidate preparation](../tests/visual/windows-0.4.0-candidate-2026-10-19.json)
records a locked build, formatting, strict Clippy, 601 workspace tests and 34 Python
tests. The matching published release engine passed 51 integration and two library
checks. A debug executable with that engine passed the six-step playback smoke in
5858 ms. Normal/narrow Audio scrolling, unchanged volume, light Quick Settings,
OS-delivered fullscreen double-click and host Escape were inspected. All test
instances exited normally. This is local candidate evidence; the final release
archive and remote workflow still require their own checks.

The [Preferences navigation/subtitle follow-up](../tests/visual/windows-preferences-subtitles-2026-10-18.json)
records exact source/binary hashes: locked build, formatting, strict all-target
Clippy, 600 workspace tests and 31 Python tests passed. The default Rust run
explicitly ignores native/clipboard checks and a helper launched by another test.

The local playback/fullscreen smoke passed in 5750 ms. A GUI sweep inspected all
eleven Preferences categories' initial viewports, Audio section jumps and sidebar
scrolling. Contrast, auxiliary width, Utilities row overlap, Advanced path/button
overlap and truncated shortcut labels are corrected. A numeric GUI edit survived
normal exit. Subtitle overflow, disclosure spacing and General section jumps are
also corrected. The repeat selector uses a separate row. Theme coverage and
matched comparisons remain open in the [visual ledger](visual-reference-audit.csv).
The [Network/font follow-up](../tests/visual/windows-network-font-2026-10-19.json)
also verifies continuous Network scrolling/search and readable dark font-picker
contrast with initial keyboard focus and Escape cancellation.

[Verification](verification.md) records the exact scope and older native/package
evidence. HDR, other drivers and revised remote CI workflows still need evidence.

## Remaining acceptance work

1. Close fully missing IINA behavior using the requirement and plugin inventories.
2. Compare every interface surface against matched IINA captures, including
   narrow windows, both themes, compact layouts, scrolling and interaction.
3. Complete plugin conformance, localization and native accessibility coverage.
4. Verify macOS/Linux application playback, dialogs, window ownership and shutdown.
5. Validate HDR/color output, audible/pass-through audio, display changes,
   system-material policy and broader DPI/multi-monitor behavior on hardware.
6. Verify each final release package, its notices and exact source/build identity.

See the [implementation plan](implementation-plan.md) for the work order and
[platform differences](platform-differences.md) for specific adaptations.
