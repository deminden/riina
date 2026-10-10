# Status · 2026-10-09

Local subtitle section ordering increment: Primary and Secondary track lists now
precede Load Subtitle and the shared timing/position editor, matching the pinned
SidebarSubtitlesPane section sequence. All subtitle sections share the scrolling
surface so growing track catalogs do not reserve fixed space outside it. Audio
loading and delay retain their existing placement. Keyboard handlers remain on
the relocated controls. Strict lint, formatting and all 25 unit tests pass;
runtime scrolling/focus traversal after relocation and rendered geometry remain
pending. Style sections, warnings and exact container spacing are still missing.
This is structural parity progress, not a pixel-perfect claim. Work stays local.

Local primary position native verification: the two-lane subtitle integration
test now applies primary positions 40, zero, 150 and 100 after secondary edits.
Observed primary position changes preserve secondary position 100, both delays,
both selected lane IDs, pause and an error-free native state. All eleven native
tests pass (9.68 seconds), with formatting and strict lint passing. This closes
the new absolute setter's native verification gap; actual rendered placement,
GUI routing and other-OS runtime remain unverified. Nothing was published.

Local subtitle editor consolidation: separate primary/secondary timing controls
are replaced with one shared delay/position section and a Primary/Secondary lane
selector, defaulting to Primary as in the pinned SubPositionDelayView. Switching
the editor changes only which observed values and command targets it exposes;
track selection is independent. Primary position now has an absolute typed setter
with the same finite 0–150 native validation as secondary; both UI sliders retain
the reference 0–100 integer range. A new core test verifies both property targets,
accepted boundaries and invalid values. Strict lint, formatting, all 25 unit tests
and all eleven native tests pass (9.68 seconds). Native regression tests do not
exercise the new primary absolute setter; GUI lane switching/routing, segment
geometry, section ordering, numeric entry and reset parity remain pending.
Changes remain local.

Local secondary subtitle position UI increment: the shared pane now exposes the
native secondary position through a keyboard-accessible slider. Its 0–100 range
and integer dispatch follow the pinned SubPositionDelayView; the engine's wider
0–150 range remains available through the existing typed command. Unknown native
position disables editing. Strict lint, formatting and all 24 unit tests pass.
Runtime GUI routing, rendered placement and the reference's shared segmented
primary/secondary position-delay layout remain pending. While auditing reset,
SidebarSubtitlesPane.swift SubDelayView.resetButtonAction was found to dispatch
audio delay zero; observable macOS reset behavior must be established before
implementing that action. This increment remains local.

Local secondary subtitle delay UI increment: the shared subtitle pane now exposes
a separate -5 to +5 second secondary delay slider with 0.1 second keyboard steps.
It dispatches the typed secondary command and displays native observations; an
unknown delay disables editing instead of presenting an assumed known zero.
The pinned SidebarSubtitlesPane defines the same slider range. All 24 unit tests,
strict lint and formatting pass, and all eleven native tests pass (9.62 seconds),
including independent shifted primary/secondary cues and reset. GUI routing and
rendered layout remain unverified; reference tick snapping, numeric entry, reset
button and section ordering remain incomplete. Nothing was published.

Local subtitle loading accessibility increment: the shared external audio/subtitle
load button now accepts keyboard focus, Enter and Space without consuming modified
player shortcuts. Windows release runtime testing with catalog.mkv confirmed Tab
reaches Load Subtitle, Enter opens its native picker and Escape cancels it and
restores focus to the load button. Further Tab navigation reaches Subtitle delay,
primary visibility, primary None, secondary visibility and secondary None in order.
Strict lint, formatting, release build and all 24 unit tests pass. This verifies
focus traversal and picker cancellation, not actual track activation or rendered
parity; external audio and other-OS runtime remain pending. Work stays local.

Local catalog keyboard increment: audio, primary subtitle, secondary subtitle,
chapter and playlist rows now expose keyboard focus and an Enter activation path.
Mouse selection retains the existing single/double-click behavior. Modified keys
and Space bubble to the player shortcuts rather than changing a focused track.
Secondary rows remain disabled without loaded media. Strict lint, formatting and
all 24 workspace unit tests pass; Slint compilation validates the focus handlers.
Runtime focus traversal, activation and rendered focus geometry remain unverified.
No public GitHub update was made.

Local secondary selector UI increment: the shared subtitle pane now exposes
Primary and Secondary track lists and independent visibility controls, following
the pinned pane's two-lane structure. Secondary rows retain native lane metadata
and stable ID tokens; None disables only the secondary lane. Unknown visibility
disables its control until a native observation arrives. Strict lint and all
24 workspace unit tests pass. GUI selection/routing and rendered parity remain
pending; load/timing section ordering and switch geometry are still provisional.
Work remains local.

Local secondary positioning increment: typed native secondary position accepts
finite values from zero to 150 and observes an optional validated position.
The two-lane SRT test verifies positions 25, zero, 150 and 100 while retaining
primary position, primary delay/selection, secondary delay and pause without
native errors. All eleven native tests pass (10.19 seconds), with 24 unit tests
and strict lint. Positioning UI, rendered geometry and other-OS runtime remain
pending; work remains local.

Local secondary timing increment: typed absolute secondary delay and optional
native observation are implemented. A real two-lane SRT test applies one second
of secondary delay and observes the primary second cue alongside the secondary
first cue at 2.3 seconds, preserving primary delay and selection. Reset restores
zero delay and aligned first cues. All eleven native tests pass (10.20 seconds),
with 24 unit tests and strict lint. Secondary timing controls, rendered output
and other-OS runtime remain pending. Work remains local.

Local secondary-subtitle increment: typed selection, independent visibility and
secondary text observation are implemented, with Alt+v connected in shared UI.
Track catalogs retain native lane metadata so secondary-only decoding cannot
highlight a primary selection. Native verification observes distinct primary/
secondary text together, independent visibility and disabling either selector,
retained pause and cleared secondary text on replacement. A distinct original
CC0 SRT fixture is generated and hashed. Secondary ASS override now defaults to
scale as in pinned IINA rather than mpv's strip default. The secondary selector
UI, GUI Alt+v routing, visual styling and other-OS runtime remain pending.
All eleven Windows native tests pass (9.43 seconds), with 24 unit tests, strict
lint, formatting and the release build. All original/new fixture hashes match.
Work remains local.

Local subtitle layout increment: G/F adjust native scale by +/-0.1, r raises
subtitle position by one point, and R/t lower it by one point, matching the
pinned default input config. Optional observed scale/position retain unknown
values as unknown. Ten Windows native tests pass (9.46 seconds), including
queued scale 1 → 2 → 1 and position 100 → 85 → 100, native range clamping and
retained pause/track/delay. The larger shortcut tree exposed a build-script
stack overflow; Slint compilation now uses a joined portable worker with an
explicit eight-MiB stack. GUI routing, actual rendered layout and other-OS
runtime remain pending. All 24 unit tests, strict lint, formatting and the
Windows release build pass after the compiler fix. Work remains local.

Local track-shortcut increment: native cycling now covers video, audio and
primary subtitles, retaining the engine's disabled sentinel and external-track
ordering. Shared routing adds macOS Command+Control+A/S/V and Windows/Linux
Control+Alt+A/S/V, Shift+primary+A/S panel toggles and plain v subtitle visibility.
Ten Windows native tests pass (9.34 seconds), including internal/external audio
cycling through disabled, external subtitle disable/restore and video disable/
restore with pause and unrelated selections retained. All 24 unit tests, strict
lint and the release build pass. GUI routing, other layouts/AltGr, visual feedback
and other-OS runtime remain pending. This increment stays local.

Local file-repeat increment: Shift+Command+L (macOS) / Shift+Control+L
(Windows/Linux) toggles only current-file repeat. Native integration verifies
actual EOF-to-start playback wrapping, file-repeat priority when both modes
are enabled, retention of playlist repeat when file repeat is removed, and
file-only on/off toggling without resuming pause. All nine Windows native tests
pass (9.28 seconds), with 24 unit tests, strict lint and the release build.
Windows keyboard/accessibility observations verify Off → File → Off with
timeline focus and unchanged paused position. Rendered feedback, repeat OSD,
other keyboard layouts and other-OS runtime remain pending. Evidence is in
tests/visual/file-repeat-windows-evidence.json. No public update was made.

Local chapter increment: Shift+primary+> / < invoke native relative chapter
navigation; Shift+primary+C / P toggle the chapter and playlist panels. Primary
is Command on macOS and Control on Windows/Linux. Eight Windows native tests
pass (9.40 seconds), verifying forward/backward chapter seeks, pause retention
and backward navigation at the first chapter, alongside 24 unit tests, strict
lint and the release build. Actual Windows keyboard/accessibility evidence
verifies navigation with timeline focus and both panel toggles. Last-chapter
transitions, backward restart threshold, rendered parity and other-OS runtime
remain open. See tests/visual/chapter-shortcut-windows-evidence.json. Work stays
local; no public GitHub update was made.

Local volume/speed increment: shortcut deltas and speed multipliers now operate
on native current values rather than a coalesced UI snapshot. Native volume
starts capped at 100; enabling boost permits 200, and disabling it clamps the
current value without overwriting queued changes with a stale display value.
Eight Windows native tests pass (9.19 seconds), including queued coarse volume
changes, both caps, boost removal, repeated speed multiplication and reset while
paused. All 24 unit tests, strict lint and the release build pass. Shared Rust
and Slint paths preserve platform boundaries. This increment has not been GUI
tested or run on macOS/Linux and remains local.

Local audio timing increment: Shift+( and Shift+) adjust audio delay by
+/-0.5 seconds; Alt adds fine +/-0.1-second steps, and Shift+_ resets it.
Decoder-relative commands preserve rapid adjustments. All eight explicit
Windows native tests pass (8.98 seconds), alongside 24 unit tests, strict lint
and the release build. Actual Windows keyboard/accessibility observations
verify audio and uppercase subtitle timing steps and reset with slider focus,
retaining pause at two seconds. Evidence is in
tests/visual/timing-shortcut-windows-evidence.json. Screen capture still fails;
rendered feedback, acoustic synchronization and other-OS runtime remain open.
No public GitHub update was made.

Local subtitle timing increment: uppercase Z/X adjust delay by -/+0.5 seconds,
Alt+Z/X use -/+0.1 seconds, and C resets it. Relative commands accumulate on
the decoder rather than reusing a stale UI snapshot. Eight Windows native
tests pass (8.85 seconds), including queued fine/coarse bursts and reset while
paused; all 24 unit tests and strict lint pass. Keyboard routing and other-OS
runtime verification remain pending. This work stays local.

Local subtitle increment: shared Shift+Right/Left shortcuts invoke native cue
navigation. Observed-clock/text tests verify next/previous cues with retained
pause and unchanged delay, then repeat with a 0.25-second delay and restart the
current cue. All eight explicit Windows native tests pass (8.75 seconds), with
24 workspace unit tests and strict lint. GUI routing, embedded-subtitle prefetch,
secondary cue selection, boundaries and rendering/style parity remain open.

Local Windows GUI evidence now verifies focused-slider Alt+Right/Left and
period/comma frame navigation, fine volume increments and Control+period Stop.
The seek clock also appears in accessibility observations. Capture still fails
with the monitor helper's 0x80070057, so rendered appearance and exact OSD timing
remain unverified. See tests/visual/frame-shortcut-windows-evidence.json.

Local shortcut increment: backward frame stepping now accompanies forward
stepping. Alt+arrows and comma/period invoke the frame actions; primary-modifier
arrows navigate the playlist, and primary-period stops. Modified arrows bubble
out of sliders. Default pause/mute/speed shortcuts and fine volume increments
are connected; Enter enters fullscreen without toggling it off. Seven native
tests pass, including observed one-frame forward/backward movement on the
24-fps fixture while paused. GUI input routing, boundary/VFR behavior and
other-OS runtime verification remain open. No public update was made.

Local A–B lifecycle increment: the backend now resets A/B options on new media,
matching the pinned reference configuration, rather than only clearing cached
markers. Native coverage verifies replacement, Stop/reopen and playlist advance
by playing past the previous B point. Stop/failure clears cached points and
end-file invalidates pending seek completion. UI parity and other-OS runtime
validation remain open; this increment has not been pushed publicly.

Local playback increment: typed A–B point setting and native point cycling are
connected to shared playback, with observed optional numeric points. Command–L
on macOS and Control–L on Windows/Linux invoke the default action. Five Windows
native integration tests pass (2.56 seconds), including an actual wrap from B
back to A, point cycling and clearing. Point editing preserves the pinned minimum
nonzero timestamp for zero. Menus, timeline markers, A–B OSD, persistence rules
and other-OS runtime comparison remain incomplete. Incremental work stays local;
public GitHub updates are reserved for substantial verified milestones.

Shared preferences now retain the OSD enable flag and timeout, defaulting to true
and one second for existing settings. Timeout validation rejects values the
event-loop timer cannot represent. Windows strict lint and 23 unit tests pass;
matching settings controls and OSD types beyond seeking remain incomplete.

Reference run 38007160214 succeeded operationally but its images are rejected
for matching-state comparison: early capture shows OSD over black video, while
the settled capture is white. Both IPC brackets remain paused at two seconds.
Rendered media content must be verified independently from the engine clock.

Real macOS arm64 libmpv integration now passes all four tests (run 38005017036,
3.45 seconds) with Homebrew mpv 0.41.0 on macOS 26.6.2. See
tests/visual/macos-native-evidence.json for the exact runtime hash and commit.
Full application builds and the subsequent concurrency/failure storage tests
also pass on macOS/Linux in run 38004680216. These tests use null video output;
displayed video, dialogs, audible output and HDR presentation remain unverified.
Official IINA 1.5.0 reference run 38005949498 now provides a reviewed paused
seek state at 2 seconds. It establishes the seek clock/progress panel, with the
main controls hidden; matched-size riina comparison remains pending.

The shared application now displays a transient seek clock/progress panel from
observed libmpv SEEK/PLAYBACK_RESTART events. A retained completion counter
survives coalesced snapshots; failed command submission does not fabricate a
clock update. Replacement/Stop hides the panel. Geometry, colors and the 2.5-second
timeout were provisional. The timeout now uses the pinned IINA default of one
second; rendering, fade and actual event-relative timing remain unverified.
Windows strict lint, 22 unit tests and four real native integration tests pass;
the paused-seek test now requires a native completion notification as well as
the observed 2-second clock. GUI rendering and port runtime validation are open.
The shared OSD reducer now defers coalesced completion notifications until the
clock is usable and no newer seek is in flight. Tests cover replacement, Stop,
failure suppression, rapid completions and clock ticks that must not extend the
display deadline. Native integration results above are from the preceding
backend change; the reducer-only increment passes workspace tests and lint.

Full macOS/Linux application lint, workspace unit tests and linked development
builds now pass in run 38004581488 at a48a1de. Evidence is recorded in
tests/visual/application-port-ci-evidence.json. Subsequent storage tests are
still running on macOS; native decoding run 38005017036 has been dispatched.
No GUI runtime, HDR presentation, packaging or exact interface parity is implied.

macOS file discovery now delegates filename ordering to Foundation's documented
localizedStandardCompare, using retained strings and a worker autorelease pool.
The dedicated Unicode/numeric worker test awaits macOS application CI. Windows
checks/lint pass locally; Linux retains its provisional numeric ordering.

2026-10-10 application port increment: macOS/Linux preference, library discovery
and asynchronous owned picker backends now connect to the shared GL player.
Dialog intent is shared; Windows retains its existing COM implementation.
A new application CI matrix lints/tests/links on macOS/Linux. Results and native
runtime behavior are pending; these systems are not yet supported targets.
See ADR 0004 for remaining collation, modality, packaging and HDR gaps.

2026-10-10 portability increment: folder discovery, extension filtering,
exact-path deduplication and playlist/disc bypass rules now live in riina-core.
The application supplies its native filename comparator through a small adapter.
Four filesystem/selection tests now join the three-OS portable CI matrix;
Windows retains a separate native filename-order test. Local formatting, strict
workspace lint and all 18 unit tests pass. Portable checks now contain 17 tests;
full macOS/Linux application builds and playback remain open near-term gates.

Milestone 0 foundation is implemented and built. Initial inventory is intentionally
incomplete: 64 atomic parity rows and 19 plugin surfaces are tracked. Milestone 1
is in progress; real SDR playback works, but HDR architecture, DPI coverage and
full accessibility/visual fidelity gates are not closed.

Published source: https://github.com/deminden/riina. At the user's request,
original riina contributions now use MIT and an explicit independent-development
policy (docs/licensing.md). Slint uses Royalty-free 2.0 with the official badge
in the public README; mpv ABI notices remain ISC. Third-party references and
native binaries retain their own terms. This source grant does not certify
clean-room provenance or close the native redistribution audit.

The separate D3D11 experiment now runs native gpu-next video with an owned Slint
overlay. Runtime logs report a 16-bit float scRGB swap chain on the observed SDR
desktop. Mouse/keyboard controls, Unicode input, fullscreen, maximize and shutdown
were exercised. This is presentation feasibility, not verified end-to-end HDR;
see ADR 0002 and `tests/visual/d3d11-probe-evidence.json`.

Original PQ/HLG 10-bit fixtures now verify effective BT.2020 metadata and
replacement/stop behavior. After the user enabled Windows HDR, the native probe
queried a 10-bit PQ/BT.2020 desktop and reported a rgba16hf/scRGB target with
1499-nit engine peak. PQ/HLG/SDR presentation and keyboard overlay controls were
exercised; actual captures/metadata are in `tests/visual/hdr-lifecycle-evidence.json`.
This is an observed HDR route, not calibrated color accuracy or full UI parity.
Undefined native target ratios no longer cause playback errors.

`riina --native-video <media>` now uses this candidate behind the same player
controls and sidebars as the default renderer. The shared controller is in
player_ui.rs; native_window.rs handles HWND ownership, client sizing and focus.
Actual native GUI checks covered PQ/HLG, fullscreen/Escape, toolbar Tab/Enter,
Stop, Ctrl+O after Stop, SDR recreation and popup-focused shutdown at 300% scale.
Initial focus/layout failures were corrected and rechecked. The default GL route
also passed playback and keyboard chapter-sidebar checks. Current evidence:
tests/visual/native-shared-evidence.json. Video blur and full window/DPI coverage
remain open; native output is experimental and opt-in.

At the user's request, the local toolchain and CI pin are now Rust 1.99.0, the
current stable release, using edition 2024. Crates inherit rust-version and deny
unsafe operations without explicit unsafe blocks. Current idioms include let
chains for state/ownership checks. Slint/libmpv remain independently pinned.

File pickers now retain their native owner, serialize requests and protect the
native video host while the Shell dialog is open. Cancellation and dispatch
failure restore modality without recursively scheduling a dropped guard.
Formatting, strict Clippy, 12 unit tests and the optimized Windows build pass.
Interactive testing caught and fixed event-thread-only handle acquisition and
cross-thread owner disabling. Default GL cancellation/reopening and final native
HDR cancellation/shutdown now have evidence in
tests/visual/dialog-lifecycle-evidence.json. File acceptance and external-track
dialog routes remain unverified because the automation helper cannot reliably
address the owned transient controls. Earlier native captures retain their own
binary hashes.

Open File now allows multiple selections and submits one validated OpenFiles
intent: replace with the first item, append the rest, and resume playback. A real
libmpv test verifies paused-playlist replacement, retained order and advancing to
HLG video (0.52 seconds). GUI acceptance remains unverified. IINA's file filtering,
deduplication/natural sorting, folder expansion and separate-player preferences
are still required; this implements its current-player batch branch only.
The full four-test native suite passes (2.10 seconds), alongside all 12 unit
tests, strict Clippy and the optimized Windows build. Historical accessibility
text captures mark their branding redaction; their image captures remain historical.

Dialog media/playlist selections now apply the pinned blacklist, exact-path
deduplication and natural filename sorting within lexically ordered parent paths.
A single media playlist file bypasses the blacklist for mpv expansion. Fourteen
unit tests, strict Clippy and the optimized build pass. Explorer comparison is a
Windows adaptation; locale equivalence, aliases and directory expansion remain
open, along with GUI acceptance and separate-player preferences.

Reference: IINA v1.5.0, **fbcff4df7183b335880396af0655f6bb89d46442**.
Acquisition and native binary hashes: reference/upstream-lock.json and native-lock.json.
No existing repository code was present. Git was initialized without committing.

Environment: native Windows 11 Home x64, version 10.0.26300; NVIDIA GeForce RTX
4070 Ti, driver 32.0.16.1656. Rust 1.99.0 MSVC (edition 2024), Slint/slint-build 1.13.1. Build
tools were downloaded/extracted under tools only; no system-wide installation,
security changes, default-app registration or administrator steps.

Implemented: three Cargo workspace crates; typed commands and state; validated
preference parsing; isolated dynamically loaded mpv ABI; event worker/property
observation; bounded intent queue and coalesced UI snapshots; asynchronous command
IDs; EOF/load-error handling; GL render context ownership and GPU FBO surface;
Slint custom OSC and grayscale sliders; open-file dialog and first CLI media argument;
play/pause/stop/relative and exact seek/volume/mute/speed/frame-step; observed timeline;
fullscreen/resize wiring; atomic local volume persistence; synthetic audiovisual fixture;
reference fetch, native dependency fetch, build helper and traceability checker.

Added event-owned mpv node decoding for track, chapter and playlist catalogs; typed
command plans; Audio/Subtitles/Chapters/Playlist sidebars. Playlist supports multi-file
append, single selection, remove, up/down reordering, clear except current entry,
repeat cycling and current-entry reload. Stable mpv IDs protect queued UI selection.
External subtitle loading/reload, primary visibility/delay and external audio/delay
are implemented. Chapter activation seeks to observed time and resumes playback.
Twelve unit tests and three explicit native integration tests pass. GUI verified chapter
activation, manual UTF-8 subtitle loading/rendering, accessible delay, multi-file
append and reordering. Final layout/disabled accessibility states were captured after
source width/side defaults and delay spacing were corrected. Appearance remains provisional; secondary subtitles, multi-row
selection, clipboard, shuffle/sort, numeric delay input and full styling are pending.

Artifacts:

- `target/x86_64-pc-windows-msvc/release/riina.exe`: optimized unsigned development executable.
- `target/debug/riina.exe`: earlier debug executable; rebuild before relying on its latest state.
- `tools/mpv/libmpv-2.dll`: pinned development runtime, loaded from the workspace or beside the executable.
- `tests/visual/`: actual Windows captures and capture manifest; not upstream references.

Build/run:

```powershell
./scripts/dev.ps1 -CargoArguments @('build','--release','--locked','--target','x86_64-pc-windows-msvc')
& ./target/x86_64-pc-windows-msvc/release/riina.exe ./tests/fixtures/moving-square.avi
```

The wrapper selects workspace-local Rust and import libraries when normal MSVC tools
are unavailable. In an ordinary MSVC developer shell use equivalent direct Cargo commands.
Native binary redistribution is not yet ready; don't move the EXE alone and expect playback.

Use top-right sidebar buttons to open Audio/Subtitles settings (trailing) or
Chapters/Playlist (leading). Double-click chapter/playlist entries to play them.
Add Files opens a multi-selection native picker. Remove/Clear affect playlist references,
not the files on disk. Original catalog.mkv, manual-captions.srt and external-tone.wav
fixtures are generated locally and hash-checked by scripts/check-fixtures.py.

CI configuration is in `.github/workflows/windows.yml`: pinned checkout action,
Rust toolchain, formatting/lint/domain tests, traceability and actual Windows application
compilation. The two initial Windows CI runs completed successfully on GitHub. Hosted compilation is not Windows 11 GUI
or native playback evidence; those checks remain explicitly separate.

See verification.md for exact commands/results and GUI limitations. No live macOS
reference, visual difference scores, calibrated HDR accuracy, bitstream audio, ARM64 or performance
thresholds were tested. Audio output was observed through libmpv; no independent
auditory listening test was performed. No claims of exact portable parity are made.

Known gaps: initial requirement/menu/preference inventory; source-matched light/compact/
docked layouts; actual sampled-video blur and licensed vector icons; full playlist/chapter/
track/subtitle workflows; multiple windows/history/resume; plugins/WebView2; OCR;
localization; drag/drop; network fixtures and helpers; Windows media/power/shell
integration; structured diagnostics; codec/native license audit and portable packaging;
HDR presentation, DPI matrix and stress/performance baselines. The command queue reports
overload rather than dropping consequential intents; seek-specific coalescing remains pending.

Fresh check after the user's HDR confirmation again detected DISPLAY1 as 10-bit
PQ/BT.2020 and presented the PQ fixture through rgba16hf/scRGB with the owned
controls visible. Logs: tools/hdr-current.stderr and tools/d3d11-probe.log.
These logs are local development artifacts; screenshots do not measure HDR luminance.

Next concrete implementation step: close native blur/composition, narrow-window,
modal ownership and mixed-monitor DPI gaps, then choose the production direction
in ADR 0001. Continue source-matched control geometry/icons and observable behavior inventory.

Recursive folder startup now discovers and orders media on a background worker in
both playback routes. Sixteen unit tests pass; both routes have observed folder
playlist/pause/shutdown evidence. GUI folder selection, scan cancellation/resource
limits and link/permission edge cases remain open. See folder-opening-evidence.json.

The project is now riina. Original state/commands/preferences stay in riina-core,
which has no Windows or GUI dependency. Windows dialogs, window ownership,
preferences storage, library resolution and Shell comparison live under
crates/app/src/platform/windows. The Windows dependency is target-scoped.
Shared Slint/controller behavior remains above these native services. Other OS
backends are near-term work, not implemented or tested. Volume boost is opt-in and
defaults off; disabling it restores the 100% cap.

GUI volume extension verified on the default renderer at 300% DPI: enabled
200%, disable/clamp to 100%, disabled-range enforcement and persisted off state.
Dark control contrast was corrected and visually inspected. See
tests/visual/riina-volume-evidence.json. Exact IINA visual parity remains open.

The shared floating panel now uses the recorded 460 x 67 outer geometry; actual
GL/native captures confirm these bounds and seeking input. Full material/icon/
typography parity still needs matching reference captures. The portable core
workflow tests and lint passed on Windows/macOS/Linux (run 38002087280).
Full application ports still require native services and actual playback evidence.

Original MIT vector controls now replace font-dependent playback and main
sidebar glyphs. GL/native rendering and native mute-state changes were observed
at 300% DPI; exact reference shapes remain unverified. The asset manifest now
contains 13 checked author/license/hash records. The portable matrix is expanded
to mpv interface unit tests, and HWND startup is gated to Windows.

Expanded portable CI passes 13 core/mpv-interface tests and strict lint on
Windows/macOS/Linux (run 38002664122). Native engine loading, GUI and application
ports are excluded from this evidence. See portable-interface-ci-evidence.json.

The shared controller no longer accepts a Windows window identifier. Native
startup/handle validation and diagnostics live in the presentation backend;
UiSession retains its target until the player shuts down. Native PQ, seeking,
fullscreen/Escape and close plus GL startup/seeking/close passed at 300% DPI.
Native logs use %LOCALAPPDATA%/riina/native-player.log. See ADR 0003 and
presentation-boundary-evidence.json. Other OS presentations remain open.
