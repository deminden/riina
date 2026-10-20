# Current status

Updated 2026-10-20. Published release: **4.2.0**. Reference: **IINA v1.5.0**
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

- The local playback-input follow-up preserves fractional Windows wheel lines
  instead of promoting every report to a notch. Hide Riina is enabled, using
  restorable taskbar minimization on Windows. Click defaults remain IINA's:
  single hides controls, double toggles fullscreen, right toggles pause. Native
  checks cover those defaults, configured single-click Pause/Resume, horizontal
  and timeline wheel amounts, and Hide/restore; physical Magic Mouse feel and
  live macOS comparison remain open. See the
  [input record](../tests/visual/windows-playback-input-2026-10-20.json). These
  changes are subsequent local work, outside the published 4.2.0 package.
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
- Native menu shortcut hints include shifted punctuation and prefer the primary
  default chord while retaining aliases. Subtitle scale menu actions use the
  reference multiplicative steps; information labels refresh in place on opening.
- Light Quick Settings tints its sampled video before drawing controls. Fullscreen
  Escape also works when the framed playback host has keyboard focus.

## Changes in 4.2.0

Quick Settings now shares one track-list control across Audio, Video and both
subtitle lanes. Repeated arrows advance without waiting for playback observations;
Home/End and Page Up/Down reveal the focused row. Each list is one Tab stop, while
the active-track marker continues to follow the engine. Language badges fit their
text and reserve space for the track name, including in the Video pane.

The [track-list record](../tests/visual/windows-quick-settings-tracks-2026-10-19.json)
identifies that earlier executable and its scoped native checks. The newer
[workflow record](../tests/visual/windows-workflows-2026-10-20.json) separates the
current checks from intermediate failures and remaining acceptance work.

The follow-up adds keyboard-operated equalizer Save/Rename, Unicode profile
persistence, semantic light-theme handles and subtitle warning colors. The play
triangle has an independent optical correction; Music Mode owns separate transport
geometry and top-left window actions. Pane scroll positions have shared ownership
and headless coverage, but the exact-package check at 285 x 141 and 300% scaling
reset Audio after a Layout round trip. Fit to Screen also produced an undersized
window in that session; both native sizing/scrolling cases remain open.
Playlist rows take their available width, with six footer actions
and less frequent editing in row context menus.

First-pointer slider/color input, paused-catalog Music Mode debounce and Windows
watch-later path identity are corrected. History accepts late duration discovery
without recording another visit or resetting saved progress. Shared auxiliary-window acrylic fills
use neutral RGBA colors rather than opaque cyan ARGB spellings. Folder discovery
now has real temporary-filesystem regressions for ordering, exclusions and stale results. Configurable
Control bindings from the framed host share the existing dispatch route; native-
menu-only accelerators from every focus owner remain separate work.

Only Windows was available for desktop checks. Linux full-application CI installs
and probes GTK and WebKitGTK dependencies. A separate main-source follow-up passed
Linux lint, unit tests and the full application build, and macOS strict lint;
macOS alias-path handling was then corrected after a unit-test failure. That
[source follow-up](port-development.md) has separate verification in progress.
Native macOS/Linux playback, physical audio/HDR,
other DPI, complete focus/accessibility behavior and matched reference pixels
remain unverified. The Windows release collects the playback/settings changes
above; the platform CI repairs are subsequent work on main. Exact-package evidence
is separate from the development checks below.

The workflow batch passed build, formatting, strict Clippy, 656 workspace tests
(56 ignored), 40 widget regressions, 62 Python tests and a 5832 ms debug playback
smoke. The record separates native engine checks, GUI candidates and remaining
artwork underlay/clipping, focus, shortcut and visual-comparison gaps.

## Previous release

The [4.1.0 notes](release-notes/4.1.0.md) and
[release record](../tests/visual/windows-release-4.1.0-2026-10-19.json) retain the
earlier numeric-editing, category-navigation, Windows Alt-state and Layout work.
Those dated checks are separate from the newer workflow and package evidence.

## Verification summary

**[4.2.0](https://github.com/deminden/riina/releases/tag/v4.2.0) is published.** Its
[release record](../tests/visual/windows-release-4.2.0-2026-10-20.json) identifies source
`5910648`, archive hashes, the unchanged reused engine and exact-package checks.
Hosted and local extracted-application playback smokes passed in 6211 ms and
5843 ms respectively. Full-desktop captures and scoped native controls were
inspected; no Riina process remained after shutdown. The hosted software OpenGL
driver was test-only and is absent from the ZIP. Publication reused the verified
archive without rebuilding.

The development checks for the new changes are listed above. Earlier settings,
menu and playback results remain dated in the [verification register](verification.md),
including the [0.4.0 release record](../tests/visual/windows-release-0.4.0-2026-10-19.json).
The [visual ledger](visual-reference-audit.csv) records current interface gaps and
links historical evidence separately. Physical compositor, audible-output and HDR
coverage remain separate.

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
