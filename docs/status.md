# Current project status

## Application state

Riina is an established Rust media-player application, beyond its initial feasibility
and playback milestones. The current development version is 0.3.0. It opens and plays local media through libmpv on Windows,
with native D3D11 video presentation, working transport controls, and broad player
and preferences workflows. Regression tests cover shared state, playback, settings,
file handling, and lifecycle behavior.

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
preferences and plugin behavior, controller-drag auto-hide suppression and live
validation of its source-matched fade, and source-matched menus, sidebars, materials, window
geometry, and complete interaction captures. Windows native-menu activation now
keeps focus on the menu owner during the initial click; live playback interaction
still needs verification.
Windows HDR and native-window work has runtime evidence, but calibrated end-to-end
HDR parity and broad hardware coverage remain unverified. Consult the parity
inventory for the exact state of each requirement.

## Licensing and binary distribution

Riina's original source is MIT. The combined application package is GPL-3.0-only.
This licensing choice is settled and is not a release blocker. The release package
must still include exact third-party notices and corresponding sources for its
pinned native engine and Cargo dependencies.

The latest published GitHub release is v0.2.2 and contains source archives; it has
no Windows application binary. Candidate
[workflow run 38152037885](https://github.com/deminden/riina/actions/runs/38152037885)
reused the native engine and corresponding-source artifacts from run 38138622911.
Source packaging and dependency notices passed, but the Windows package job stopped
in its native playback gate when the serial native test process hit a Windows access
violation while running `ab_repeat_observes_points_and_wraps_playback`. The exact
candidate engine artifact passes its runtime-manifest check and all 42 native tests
in a local serial Windows run; the hosted-runner crash remains unresolved. No binary
is available until the Windows package gate,
exact package checks, and corresponding-source audit pass and a versioned release
publishes its assets.

The x264 encoder is excluded from the release engine; FFmpeg's built-in H.264
decoder remains enabled. The development DLL is not the release runtime. The
settled license choices and candidate build inputs are recorded in
`docs/licensing.md` and `docs/native-distribution-audit.json`.

## Verification state

The parity traceability check accepts all 192 currently inventoried requirements,
while explicitly reporting that the inventory is incomplete. The previous
candidate's native engine and source artifacts are available to reuse for the next
package check. The Unix presentation marker
derives `Clone` to match the Windows adapter's shared timer ownership. The latest
macOS/Linux application run failed lint because the marker also derived `Copy`, making
the shared clone redundant; the marker now derives only `Clone`, and that correction
is awaiting the next normal CI run. Windows compilation/domain and portable playback
workflows are separate checks; consult their linked Actions runs for live conclusions.

Local verification evidence and exact limitations are recorded in
`docs/verification.md` and the requirement rows in `docs/parity.csv`. GUI, HDR,
platform, or release-package behavior is not considered verified unless its evidence
is recorded for that exact surface or artifact.
