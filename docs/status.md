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
geometry, and complete interaction captures. The user confirmed that Windows
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
playlist navigation while compact mode is active, retaining speed controls in the
standard player. The navigation icons are original Riina SVGs. Compact-player
control styling, album-art behavior, and matched macOS capture comparison remain
open.

## Licensing and binary distribution

Riina's original source is MIT. The combined application package is GPL-3.0-only.
The v0.3.0 package includes the exact third-party notices and corresponding
sources for its pinned native engine and Cargo dependencies.

The latest GitHub release is Riina 0.3.0 and includes a Windows x86_64 portable
application, its complete corresponding-source archive, checksums, and a release
verification record. The package passed the extracted-ZIP import and native
playback/shutdown checks. The audit verified 101,748 native source files across
88 repositories and confirmed the exact libmpv runtime hash. The [release workflow](https://github.com/deminden/riina/actions/runs/38180094023)
and full artifact record are in `docs/native-distribution-audit.json`.

Release assets:

- [Windows portable ZIP](https://github.com/deminden/riina/releases/download/v0.3.0/riina-0.3.0-windows-x86_64.zip)
- [Complete source archive](https://github.com/deminden/riina/releases/download/v0.3.0/riina-0.3.0-source.tar.zst)
- [SHA-256 checksums](https://github.com/deminden/riina/releases/download/v0.3.0/SHA256SUMS.txt)
- [Verification record](https://github.com/deminden/riina/releases/download/v0.3.0/RELEASE-VERIFICATION.json)

The x264 encoder is excluded from the release engine; FFmpeg's built-in H.264
decoder remains enabled. The development DLL is not the release runtime. License
terms and package provenance are recorded in `docs/licensing.md` and
`docs/native-distribution-audit.json`.

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
