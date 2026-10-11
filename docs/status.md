# Current project status

## Application state

Riina is an established Rust media-player application, beyond its initial feasibility
and playback milestones. It opens and plays local media through libmpv on Windows,
with native D3D11 video presentation, working transport controls, and broad player
and preferences workflows. Regression tests cover shared state, playback, settings,
file handling, and lifecycle behavior.

Windows 11 x64 is the supported application target. macOS and Linux application
ports are in development and are not supported yet. Full application builds, visible
playback, input, dialogs, window lifecycle, and shutdown evidence are required for
support.

## Parity state

The pinned baseline is official IINA v1.5.0. The parity inventory currently traces
191 atomic requirements, and the inventory is still incomplete. Many requirements
remain partial or unverified. There is no defensible overall parity percentage.
Full behavioral coverage and measured, full-surface visual parity remain primary
acceptance goals. Use `docs/parity.csv`, `docs/interface-parity-plan.md`, and the
captured reference surfaces to choose and verify work.

Known substantial gaps include the separate Picture-in-Picture surface and its
window lifecycle, unfinished preferences and plugin behavior, and source-matched
menus, sidebars, materials, window geometry, and complete interaction captures.
Windows HDR and native-window work has runtime evidence, but calibrated end-to-end
HDR parity and broad hardware coverage remain unverified. Consult the parity
inventory for the exact state of each requirement.

## Licensing and binary distribution

Riina's original source is MIT. The combined application package is GPL-3.0-only.
This licensing choice is settled and is not a release blocker. The release package
must still include exact third-party notices and corresponding sources for its
pinned native engine and Cargo dependencies.

The latest published GitHub release is v0.2.2 and contains source archives; it has
no Windows application binary. The current Windows release candidate is
[workflow run 38144973846](https://github.com/deminden/riina/actions/runs/38144973846).
Its metadata job succeeded, but artifact verification is still false. The native-engine job is still running, so package
assembly, extracted-package playback, and final corresponding-source verification
have not yet completed. No binary is available to download until those checks pass
and the release workflow publishes the assets.

The x264 encoder is excluded from the release engine; FFmpeg's built-in H.264
decoder remains enabled. The development DLL is not the release runtime. The
settled license choices and candidate build inputs are recorded in
`docs/licensing.md` and `docs/native-distribution-audit.json`.

## Verification state

The parity traceability check accepts all 191 currently inventoried requirements,
while explicitly reporting that the inventory is incomplete. Candidate artifact
verification and native engine completion are pending. The Unix presentation marker
derives `Clone` to match the Windows adapter's shared timer ownership; full
macOS/Linux application verification remains pending. Windows compilation/domain and portable playback workflows are separate
checks; consult their linked Actions runs for live conclusions.

Local verification evidence and exact limitations are recorded in
`docs/verification.md` and the requirement rows in `docs/parity.csv`. GUI, HDR,
platform, or release-package behavior is not considered verified unless its evidence
is recorded for that exact surface or artifact.
