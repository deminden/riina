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

## Licensing and binary distribution

Riina's original source is MIT. The combined application package is GPL-3.0-only.
This licensing choice is settled and is not a release blocker. The release package
must still include exact third-party notices and corresponding sources for its
pinned native engine and Cargo dependencies.

The latest published GitHub release is v0.2.2 and contains source archives; it has
no Windows application binary. The 0.3.0 package candidate fails during native
shutdown on the hosted Windows runner, despite passing local playback checks
against the exact release DLL. Selecting a headless audio backend did not resolve
the crash. The current lifecycle implementation retains the native module for
the application's lifetime while independently destroying player cores, and
records shutdown stages to distinguish native destruction from worker teardown.
The release checks exercise independent players and reopening, preserve the full
playback assertions, and run ahead of the slower application build. Hosted
verification of this change is pending. A binary remains unavailable until the hosted
playback gate, exact extracted-package checks, and corresponding-source audit
pass and a versioned release publishes its assets.

The x264 encoder is excluded from the release engine; FFmpeg's built-in H.264
decoder remains enabled. The development DLL is not the release runtime. The
settled license choices and candidate build inputs are recorded in
`docs/licensing.md` and `docs/native-distribution-audit.json`.

## Verification state

The parity traceability check accepts all 192 currently inventoried requirements,
while explicitly reporting that the inventory is incomplete. The previous
candidate's native engine and source artifacts are available to reuse for the next
package check. The Unix presentation marker
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
