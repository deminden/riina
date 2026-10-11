# Current project status

## Application state

Riina is an established Rust media-player application, beyond its initial feasibility
and playback milestones. The current development version is 0.3.0. It opens and plays local media through libmpv on Windows,
with native D3D11 video presentation, working transport controls, and broad player
and preferences workflows. Regression tests cover shared state, playback, settings,
file handling, and lifecycle behavior. The pinned Windows libmpv also passes a
native HTTP playback check that serves the repository's moving-square fixture over
localhost and verifies advancing playback, duration, and the selected audio track.
This does not verify physical audio hardware, public network streams, or macOS playback.

Windows 11 x64 is the supported application target. macOS and Linux application
ports are in development and are not supported yet. Full application builds, visible
playback, input, dialogs, window lifecycle, and shutdown evidence are required for
support.

## Parity state

The pinned baseline is official IINA v1.5.0. The parity inventory currently traces
192 atomic requirements, and the inventory is still incomplete. Many requirements
remain partial or unverified. There is no defensible overall parity percentage.
Full behavioral coverage and measured, full-surface visual parity remain primary
acceptance goals. Use `docs/parity.csv`, `docs/interface-parity-plan.md`, and the
captured reference surfaces to choose and verify work.

Known substantial gaps include live verification of the new Windows
Picture-in-Picture surface and lifecycle, the macOS PiP surface, unfinished
preferences and plugin behavior, live validation of controller fade timing, and
source-matched menus, sidebars, materials, window
geometry, and complete interaction captures. Plugin enable/disable now persists
state; a background host discovers enabled packages during player startup and owns
bounded runtime sessions until disable or shutdown. Player and optional global
entry files run in separate bounded sessions. Granted log, transient OSD and alert
events reach the UI. Global-to-player API linking,
timers, module loading, updates, complete host APIs, overlay webview rendering,
capability-failure feedback in Settings and live GUI behavior remain open. The user confirmed that Windows
Playback and Video menu clicks now work. The menu is now removed when the native
host enters fullscreen and restored in windowed mode; that visibility change still
needs a live check. Other playback interaction remains under verification.
Windows HDR and native-window work has runtime evidence, but calibrated end-to-end
HDR parity and broad hardware coverage remain unverified. Consult the parity
inventory for the exact state of each requirement.

The PiP lifecycle now follows IINA's distinction between closing and stopping:
closing restores the main-window state without changing playback, while the
explicit Stop control pauses playback. Shared lifecycle tests and the Windows
application compile pass. The macOS surface is still not implemented, and the
Windows projection and interaction still need live GUI verification. The Mac
rendering constraint and required surface experiment are recorded in
`docs/adr/0005-macos-picture-in-picture.md`.

The playlist sidebar now supports additive and range selection, stable-order
multi-row drag reordering, and multi-entry Copy, Cut, and Remove actions. Removal
resolves stable native IDs against one playlist snapshot and adjusts indices as
rows are removed; core tests cover order and stale selections. The Windows app
compile and live selection/removal still need verification, as do drag behavior,
external file drops, edge autoscroll, and macOS interaction comparison.

Music Mode now replaces its normal speed-adjustment controls with previous/next
playlist navigation while compact mode is active, with direct playlist and
album-art visibility toggles. The compact controller spans the 300-point window
and hides the normal title controls; standard playback retains speed, stop, and
fullscreen actions. Music Mode has hover-revealed close-player and return-to-player
buttons. The volume slider now opens from the compact volume control
and closes when the pointer leaves the controller. Its playlist selects on a
single click and starts the selected entry on a double click. When controls hide, the
transport row gives way to centered title and artist/album metadata while the
timeline remains visible. Embedded art uses a reserved upper viewport, the
visibility toggle covers or reveals that surface, and an original Riina
placeholder appears for audio without embedded art or visible subtitles. Music
Mode artwork height follows embedded artwork dimensions for audio and the
display aspect ratio for video, falling back to square artwork when dimensions
are unknown. The new navigation and artwork graphics are original Riina SVGs.
Artwork sizing has unit coverage for portrait, wide, square and unknown
dimensions, along with optional artwork and playlist height combinations.
Source-matched materials, live transition and popover behavior, native-video
viewport behavior, and macOS capture comparison remain open. The metadata and
transport cross-fade uses IINA's 200 ms interval; its live appearance remains
unverified.

## Licensing and binary distribution

Riina's original source is MIT. The combined application package is GPL-3.0-only.
This licensing choice is settled and is not a release blocker. The release package
must still include exact third-party notices and corresponding sources for its
pinned native engine and Cargo dependencies.

The latest published GitHub release is v0.2.2 and contains source archives; it has
no Windows application binary. The current v0.3.0 candidate has passed the hosted
native playback and lifecycle gate using the pinned release DLL. That gate covers
the previously failing interlaced-video shutdown path, queued paused-frame close,
independent players, and reopening. The Windows app build, package assembly,
runtime-import check, and playback test from the extracted ZIP also passed in run
38174384779. GitHub skipped the corresponding-source audit and publishing jobs
because its default job condition treated the intentionally skipped native-engine
reuse as a failure. The workflow now explicitly runs those gates when package and
source jobs succeed; a corrected candidate is required before any binary is public.
Earlier native shutdown failures and their resolution remain in
`docs/native-distribution-audit.json` as release-engineering evidence.

The x264 encoder is excluded from the release engine; FFmpeg's built-in H.264
decoder remains enabled. The development DLL is not the release runtime. The
settled license choices and candidate build inputs are recorded in
`docs/licensing.md` and `docs/native-distribution-audit.json`.

## Verification state

The parity traceability check accepts all 192 currently inventoried requirements,
while explicitly reporting that the inventory is incomplete. The 0.3.0 candidate
workflow reuses the verified native engine and corresponding-source artifacts
from run 38138622911. The Unix presentation marker
derives `Clone` to match the Windows adapter's shared timer ownership. The full
macOS/Linux application workflow passed Clippy, workspace tests, and application
builds for both matrix hosts at commit `007e410` in
[run 38167050195](https://github.com/deminden/riina/actions/runs/38167050195).
This establishes successful linking on those runners, not native media playback,
PiP, display behavior, or supported-platform status. Windows compilation/domain
and portable playback workflows are separate checks; consult their linked Actions
runs for live conclusions.

Local verification evidence and exact limitations are recorded in
`docs/verification.md` and the requirement rows in `docs/parity.csv`. GUI, HDR,
platform, or release-package behavior is not considered verified unless its evidence
is recorded for that exact surface or artifact.
