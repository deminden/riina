# Verification record · native Windows · 2026-10-09

Commands below ran through `scripts/dev.ps1`, which sets process-local Rust/linker/LIB
variables. Array syntax preserves Cargo's `--` separator through PowerShell.

| Equivalent Cargo command | Result |
|---|---|
| `cargo build --workspace --locked` | Passed; native Windows debug executable |
| `cargo fmt --all -- --check` | Passed after formatting |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed with warnings denied |
| `cargo test --workspace --locked` | Passed: six domain tests; one native test explicitly ignored by default |
| `cargo test -p wiina-mpv --test native --locked -- --ignored --nocapture` | Passed: real local audiovisual load, progression, pause, exact seek preserving pause, volume, mute, EOF, stop, invalid file and shutdown; active native audio output observed |
| `cargo build --release --locked --target x86_64-pc-windows-msvc` | Passed; optimized Windows executable |
| `python scripts/check-parity.py` | Passed: 40 initial atomic rows; inventory incomplete |
| `./scripts/fetch-reference.ps1` | Passed: remote/tag/commit/clean-checkout verification |

Direct native test ran with the pinned DLL, not a mock. Headless in that test means
`vo=null`; audio output remains active. Audio audibility still needs a listening test.
Native tests finish in roughly 0.4 s; this is test duration, not a performance benchmark.

Actual GUI smoke observations using Computer Use:

| Test ID | Observed result |
|---|---|
| GUI-STARTUP | Debug and optimized EXEs open the original 5 s fixture in their own Slint window |
| GUI-PLAY | Coordinate click on the visible play button advances the backend from paused into Playing and reaches EOF |
| GUI-SEEK | Left key at EOF returns to frame 000, Paused; backend time updates |
| GUI-ACCESSIBLE-SEEK | Accessible Playback position set to 2 displays frame 048 and time 0:02 while paused |
| GUI-VOLUME | Accessible Volume set to 37 is reflected in observed UI |
| GUI-FULLSCREEN | `f` enters borderless fullscreen; Escape restores native titlebar, aspect ratio and composited controls |
| GUI-PERSISTENCE | Close saves volume 37; subsequent optimized EXE restores 37 |
| GUI-SHUTDOWN | Native Close exits the process; no wiina window remains |
| GUI-OPEN | Coordinate click opens native Open dialog; filename edit accepts the fixture path; Open reloads frame 000 in the existing player |

Found and corrected: window minimum/preferred sizing declaration; missing local MSVC
import-library prerequisites; upside-down texture; default toolkit slider accessibility
set-value not reaching its changed callback; stale EOF phase after seek; controls hiding
immediately after interaction; repeated preference replacement. Custom sliders now
define their own accessible set/increment/decrement actions.

Dialog reopening exposed mpv retaining the previous paused state. The worker now
explicitly resumes on Load to match pauseWhenOpen=false, and the native test verifies
reopening starts Playing with a fresh generation and cleared prior error. Its updated
run took 0.71 s. A release rebuild attempted while its EXE was running failed with
Windows access denied; close the tested process before rebuilding the release artifact.
The process was closed and the updated optimized artifact rebuilt successfully.

Indexed custom-button clicks did not consistently trigger actions in the automation
tool; coordinate pointer action verified Play and Open. Fullscreen was verified using
keyboard input. This is not full keyboard traversal/screen-reader certification.
Some captures of an occluded target showed unrelated foreground content; those were
excluded from saved evidence. Valid captures were made after target activation.

Visual basis: pinned source geometry and estimated colors/typography; no upstream
macOS screenshots or official release-image measurements. Windows captures demonstrate
runtime composition, orientation and regression states, not upstream pixel fidelity.
DPI/accessibility focus matrix, manual arbitrary resize, HDR/bitstream/hardware decoder
fallback and repeated GPU-lifecycle stress remain unverified.

## Catalog selection increment · 2026-10-09

Eight unit tests passed, including x64 mpv node layout and zero-entry/unknown-tag
handling; formatting and strict workspace/all-target lint passed. Explicit native
integration passed with track-list/playlist observations, disabling audio and restoring
its reported ID, followed by the existing playback/lifecycle checks (0.77 s).
Release build passed. Computer Use verified Audio sidebar opening, None selection
and restoration of pcm_s16le; the selected indicator follows backend observations.
The visual check found a centered indicator; it was explicitly anchored to the leading
edge afterward. The rebuilt release was visually checked again and the corrected
marker captured in tests/visual/windows-audio-sidebar.png with its accessibility tree.
Catalog models now retain row identity across unrelated clock updates. Final explicit
native regression passed again (0.77 s), workspace tests and traceability passed, and
the tested application was closed with no wiina window/process remaining.

No subtitle/chapter/multi-entry playlist fixture was tested in this increment. Sidebar
material/geometry are estimates; no claim of macOS visual parity follows these checks.

## Daily catalog workflows · 2026-10-09

Native Windows 11 x64 with the same pinned DLL. `catalog.mkv` reuses original MJPEG/PCM
packets and adds named chapters at 0/2 s; manual-captions.srt contains original UTF-8
cues; external-tone.wav contains original PCM. No downloaded media or transcoder.
The generator follows official Matroska specifications; four manifest hashes verified.

Commands run through scripts/dev.ps1:

| Cargo arguments / command | Result |
|---|---|
| `fmt --all -- --check` | Passed |
| `clippy --workspace --all-targets --locked -- -D warnings` | Passed |
| `test --workspace --locked` | Ten unit tests passed; two native tests ignored in this command |
| `test -p wiina-mpv --test native --locked -- --ignored --nocapture --test-threads=1` | Both actual native tests passed (1.12 s combined) |
| `build --release --locked --target x86_64-pc-windows-msvc` | Passed, including actual UI/platform code |
| `python scripts/check-fixtures.py` | Four original fixture hashes passed |
| `python scripts/check-parity.py` | 59 initial rows passed; inventory remains incomplete |

Native workflow test checks chapters/names/timestamps; Unicode subtitle text; repeat
load without duplicate subtitles; primary visibility and delay; external audio selection
and delay; chapter activation resumes; appending preserves current generation; stable-ID
up/down movement including append position; remove; manual entry change; current-entry
reload starts a fresh generation near zero; loop off/file/playlist/off; clear preserves
the current file; final state has no error. Original playback regression still passes.

Computer Use observations (real optimized application): GUI-CHAPTER double-clicks the
2 s chapter and sees Playing/frame 075; GUI-SUBTITLE-LOAD opens native Load Subtitle,
loads original SRT, sets timeline to 0.5 s and sees café — Привет in the video;
GUI-SUBTITLE-DELAY sets 0.25 through accessibility and sees observed value; GUI-PLAYLIST-EDIT
selects two files in native picker, sees three entries with original playback paused,
and moves the selected WAV above AVI while retaining selection. Saved captures:
windows-subtitles.png and windows-playlist-editing.png, each with accessibility text.
The captured playlist was the earlier trailing/300-width prototype; afterward source
defaults were corrected to leading/360 width. The delay section's excessive stretch
was constrained and Tool accessibility now exposes enabled state. Final-layout check
is separate from the already verified workflow behavior. The final optimized build
was then checked again: windows-subtitles-layout.png shows the corrected 360-width
settings panel and compact delay section; windows-playlist-layout.png shows a leading
playlist and disabled edit actions when nothing is selected. These last captures use
tools/ui-smoke copies whose media/subtitle bytes match the original fixture hashes;
the matching-basename SRT is automatically loaded by mpv. Both inputs and this detail
are recorded in windows-captures.json. Persisted workflow images are PNG conversions
of the actual captured pixels. No upstream reference images were manufactured.

Final optimized EXE SHA-256:
`2d0a8dbaf9458506056d28cbaa2948658e4b8644e4c4cabf8df6f65b7bb37a59`.
Final lint/format checks, ten unit tests and both explicit native tests passed again
(1.12 s combined native duration). The app was closed and no wiina process remained.

Secondary subtitle selection/visibility/delay, audio-sync audibility, ASS/bitmap/network
and encoding matrix, multi-selection/clipboard/shuffle/sort, first-presented-frame manual
transition synchronization and macOS visual matching remain unverified or unimplemented.
No HDR or production-renderer conclusion follows these workflow tests.

## Native D3D11 experiment · 2026-10-09

Same IINA v1.5.0 baseline, Windows 11 Home x64 10.0.26300 and RTX 4070 Ti.
`d3d11-probe` is a separate feasibility binary, not a parity UI. Its observed
gpu-next/libplacebo swap chain is R16G16B16A16_FLOAT / RGB_FULL_G10_NONE_P709.
The engine queried a 3840×2160, 10-bit SDR desktop (RGB_FULL_G22_NONE_P709).
The optimized host independently reports scale 3 and physical client 2880×1800.
Earlier `windows-captures.json` scale 1 entries described normalized captures;
their actual DPI was unmeasured and has been corrected to unknown.

Actual Computer Use: seek back from EOF; coordinate play click produces Playing;
Unicode keyboard input appears above native video; fullscreen and restoration;
native maximize updates video and overlay positions; native Close tears down mpv
and owned controls. Indexed targeting of the related popup was inconsistent in
the capture service; successful coordinate actions are not a full accessibility
conformance result. A transition capture containing other desktop content was
discarded. Four retained images contain only the experiment and original fixture.
Evidence: `tests/visual/d3d11-probe-evidence.json`; ADR 0002 records limits.

Final commands through `scripts/dev.ps1`:

| Cargo arguments / command | Result |
|---|---|
| `fmt --all -- --check` | Passed |
| `clippy --workspace --all-targets --locked -- -D warnings` | Passed |
| `test --workspace --locked` | Ten unit tests passed; two native tests ignored |
| `test -p wiina-mpv --test native --locked -- --ignored --nocapture --test-threads=1` | Both actual DLL regressions passed; 1.12 s combined |
| `build --release --locked --target x86_64-pc-windows-msvc` | Both optimized binaries built |
| `python scripts/check-fixtures.py` | Four fixture hashes passed |
| `python scripts/check-parity.py` | 59 initial rows valid; incomplete inventory |

The new Slint features required updating Cargo.lock offline; pinned Slint remains
1.13.1. Final optimized probe was launched, playback observed, then closed.
No wiina/probe process remained. Initial probe startup failed because a zero-time
timer preceded HWND realization; final code awaits the realized native window.

Current application EXE SHA-256:
`e1d9445e75e9b50ba9df01214397e33d0cb6f994e902b1a5a56a60798e890b52`.
Probe EXE SHA-256:
`1f38ac722deb5f89e28f42b67aca4c042192764eee0600a8764aed37afb0720b`.

Next: original HDR fixtures/decoded metadata, native lifecycle on file replacement,
SDR/HDR output and display verification. No end-to-end HDR, hardware decode,
performance benchmark, mixed-monitor DPI or macOS visual matching claim is made.

## PQ/HLG fixtures and Windows HDR-on test · 2026-10-09

The previous interrupted run's captures/logs were retained without claiming its
close test passed. Original CC0 v210 Matroska fixtures now verify 10-bit 4:2:2,
BT.2020 nonconstant-luminance matrix, limited range, PQ/HLG and 3 s duration.
The generator uses no downloaded encoder; hdr-patches.json records mathematical
input values and codewords. Seven fixture/provenance inputs are hash-checked.

Source parameters and video-target-params are observed separately. Initial source
dimensions can precede decoded format/color fields. Unavailable decoder/audio
properties clear old values. Native target aspect/par can be non-finite; the ABI
decoder retains unknown values as null instead of setting a playback error.
HDR source classification reproduces VideoView.isHDRVideo: PQ/HLG plus BT.2020
or display-p3; BT.709/unknown metadata do not identify an HDR source.

The user enabled Windows HDR. A fresh actual D3D11 instance queried DISPLAY1:
3840×2160, 10 bits, RGB_FULL_G2084_NONE_P2020. The prior SDR query was
RGB_FULL_G22_NONE_P709. The observed HDR-on target is rgba16hf, full-range RGB,
BT.709/scRGB, 2880×1620 video viewport, engine max-luma 1499 nits. This peak is a
reported target, not measured physical luminance. The native swap chain reports
R16G16B16A16_FLOAT / RGB_FULL_G10_NONE_P709. Effective PQ and HLG source metadata
reach that target. Stop clears source/target and removes the child video surface;
SDR load recreates native output and preserves the usable overlay.

GUI-HDR-OFF/GUI-HDR-ON captures and selected source/target/renderer logs are in
tests/visual/hdr-lifecycle-evidence.json. Ordinary screenshot pixels cannot prove
HDR color accuracy; some HDR-on captures include only the visible portion of a
window extending beyond the screen edge. Capture sizes/clipping are recorded.
Unrelated content in transient captures was discarded. Mouse/indexed targeting
became unreliable after the desktop mode change. Tab/Shift+Tab focus was observed,
then Enter activated Next/Stop successfully. This is a keyboard-path check, not
a complete screen-reader conformance result.

Alt+F4 originally hid only the focused popup. After the fix, a fresh optimized
probe with an existing video VO was focused on Stop by Tab, then Alt+F4 closed
both windows. Native VO uninit was logged and no probe process remained.

Historical validation for the shared-native capture binary through scripts/dev.ps1:

| Command / Cargo arguments | Result |
|---|---|
| `fmt --all -- --check` | Passed |
| `clippy --workspace --all-targets --locked -- -D warnings` | Passed |
| `test --workspace --locked` | Twelve unit tests passed; three native tests ignored |
| `test -p wiina-mpv --test native --locked -- --ignored --nocapture --test-threads=1` | All three actual DLL tests passed, 1.58 s combined |
| `build --release --locked --target x86_64-pc-windows-msvc` | Both binaries built |
| `python scripts/check-fixtures.py` | Seven hashes passed |
| `python scripts/check-parity.py` | 63 initial rows valid; denominator still incomplete |

Native metadata tests use vo=null and do not prove presentation. The separate
native-window observations provide presentation evidence. Per-player HDR toggle,
tone-mapping preferences, P3/compressed HDR, hardware decoding, calibrated color,
live OS mode changes, full DPI/ownership matrix, blur/clipping and full player UI
integration remain open. The ordinary player is still the provisional SDR path.

Current optimized artifact SHA-256 hashes:

- wiina.exe: `f863fae2673045273ead08df8ed1683bc59e041e882b92a4d7a45ea8eff8896d`
- d3d11-probe.exe: `f1e8f752b7b7d111c78accf4f4a750b3460cec628e9e3932a6d7b628a45d9627`

## Rust 1.99 and shared native player UI · 2026-10-09

The user requested current Rust capabilities. Official Rust release announcements
identify 1.99.0 (2026-10-01) as the current stable release. Installed locally with
rustup: `rustc 1.99.0 (b940084d7 2026-09-28)`. The toolchain and CI pin are 1.99.0;
all crates inherit edition 2024, rust-version 1.99 and the explicit-unsafe-block lint.
New let chains express dependent state/ownership checks. Existing pinned Slint and
libmpv versions were retained. CI was configured, not executed on GitHub.

`wiina --native-video <media>` runs D3D11 behind the shared player UI. There is one
controller for commands, state coalescing and catalog models. GUI callbacks hold
command senders; only the session owns the native player, allowing synchronous
shutdown before either HWND disappears. GL keeps its render-context owner invariant.

Actual GUI checks through Computer Use:

- PQ and HLG display with shared floating controls on the HDR-enabled desktop.
- Shared playlist mouse activation and subtitle toolbar Tab/Enter activation.
- Actual host/popup realization before sizing fixes initially missing lower controls.
  The startup log reports scale 3, 2880×1800 physical / 960×600 logical client size.
- Native fullscreen and Escape retain chrome keyboard focus; first attempt lost
  focus to the host, so owner activation now forwards focus to the chrome.
- Tab visits each toolbar control and volume before Stop. Enter destroys the VO.
  First implementation lost shortcuts when the focused OSC was removed; change
  handlers now restore the parent key scope. Ctrl+O after Stop opens the real picker.
- Selecting SDR recreates D3D11 output on the HDR desktop. Alt+F4 with Open File
  focused closes both native windows; VO uninit was logged and no process remained.
- Default GL playback and observed chapter catalog still work after extraction;
  Tab/Enter opens Chapters and Escape closes it while the toolbar retains focus.

`tests/visual/native-shared-evidence.json` records actual JPEG hashes, executable
hashes, fixture hashes, selected renderer/lifecycle logs and earlier capture versions.
Final native captures show HLG, Stop, SDR recreation and fullscreen. Earlier PQ and
playlist captures identify their earlier executable hashes. GL regression captures
are `gl-shared-regression.jpg` and `gl-shared-chapters.jpg`. Unrelated pixels in
transient related-window captures were discarded. These are ordinary SDR screenshot
pixels, not luminance/color measurements or macOS reference goldens.

Final validation through scripts/dev.ps1:

| Command / Cargo arguments | Result |
|---|---|
| `fmt --all -- --check` | Passed |
| `clippy --workspace --all-targets --locked -- -D warnings` | Passed |
| `test --workspace --locked` | Twelve unit tests passed; three native tests ignored |
| Explicit ignored native DLL tests on Rust 1.99 | All three passed; 1.61 s combined, before later UI-only fixes |
| `build --release --locked --target x86_64-pc-windows-msvc` | Both optimized binaries built |
| `python scripts/check-fixtures.py` | Seven hashes passed |
| `python scripts/check-parity.py` | 63 initial rows valid; inventory incomplete |

Shared-native capture binary SHA-256 (historical):

- wiina.exe: `bc3bcb963631f0db2d7822b83106c24b29ad5048c3484342275c5ece20577ccb`
- d3d11-probe.exe: `9ab54604b5fc7eea79da6bcc418ee8b065cdd785d77afa3821847f83fc011005`

Native output remains opt-in. The attempted minimum-width drag did not produce an
observed size change; narrow-window testing remains pending. Related-window indexed
clicks/coordinate input were unreliable, so observed keyboard paths were used.
Modal ownership, video blur, full DPI/ownership coverage, compressed HDR/hardware
decode, calibrated HDR/SDR UI luminance and source-matched geometry/icons are open.
Full portable IINA parity remains incomplete.

File-selection increment (2026-10-09): `test --workspace --locked` passes
14 unit tests; four native tests are opt-in. The explicit native suite passed
all four in the preceding batch-opening increment (2.10 seconds), before this
dialog-only filtering change. Strict Clippy, formatting and optimized build pass.
The new application tests cover numeric natural order, duplicate removal,
case-insensitive extension blacklist, unknown explicitly selected extensions,
single-playlist bypass and NUL rejection. These tests invoke Windows Shell's
comparator and do not exercise dialog acceptance or prove macOS locale equivalence.
Current release wiina.exe SHA-256:
`3297a6d8a042a2da757a1059fb06b211554bfe7df22c23f504c215e4b83c313b`.
The inventory now has 64 traced rows; the complete denominator remains open.

Folder-opening increment (2026-10-09): 16 unit tests, strict Clippy and optimized
Windows build pass. Recursive discovery filters media extensions, handles duplicate
paths and natural ordering, and preserves single disc structures for mpv. Filesystem
preparation runs off the GUI thread; a request generation prevents stale results
from committing after a newer selection or Stop. Stale scans still run to completion.
Both GL and native-video CLI folder startup showed the same five ordered fixture
entries, playback, keyboard pause, sidebar activation and clean shutdown at 300% DPI.
Evidence: tests/visual/folder-opening-evidence.json. Raw accessibility captures remain
local. GUI acceptance, folder picker, cancellation stress and actual disc playback
remain unverified. This binary SHA-256: `9bbbf5a2ae6d0e0b15752f863ac966833c74dfbc89d0bf14a08c1a8e1043631f`.

riina rename and volume extension (2026-10-09): historical wiina names/hashes
above remain intact. Rust packages now use riina, riina-core and riina-mpv.
17 unit tests, strict Clippy and four explicit native tests pass. The native
playback test observes volume at 150% and 200%, then returns to 25%. Preferences
default boost off, cap legacy excessive volume at 100%, and preserve enabled
values through 200%. GUI checks now observe enabled 200%, keyboard disabling to 100%, disabled
150% requests capped at 100%, readable dark checkbox and normal shutdown saving
boost off with restored volume 37. Evidence: tests/visual/riina-volume-evidence.json.
The native-window GUI route and other OS builds remain unverified.

Floating controls geometry (2026-10-09): the panel height is now 67 logical
points, with recorded 14/8 vertical insets, 8-point row spacing and 460-point
width. Original row allocation remains provisional until matching reference
captures. Actual GL and native client captures each measure a 460 x 67 border
rectangle. GL absolute seeking and native keyboard seeking preserve pause.
Both players closed normally. Formatting, strict Clippy, 17 unit tests and the
optimized build pass. Evidence: tests/visual/riina-osc-geometry-evidence.json.
This is a specific geometry correction, not full pixel-perfect verification.

Portable core workflow now runs its tests and lint on Windows, macOS and Linux.
Its jobs exclude the GUI/native runtime and do not certify application ports.

Portable core CI evidence: run 38002087280, commit
`06fd91e2bff20af31bb0704d299d5ac1dd6ae73b`, completed successfully for
macOS (job 114062308068), Windows (114062308249) and Linux (114062308306).
Each job runs the actual core tests and strict lint.
https://github.com/deminden/riina/actions/runs/38002087280
This proves core portability only; GUI/rendering/OS services remain untested there.

Vector controls (2026-10-10): original SVG controls render in GL and native
presentation at 300% DPI. Named accessible buttons retain their actions; the
native mute icon visibly changes with keyboard mute. GL absolute seek still
observes pause. Seventeen unit tests, strict Clippy, formatting, optimized build
and all 13 asset hashes pass. Runtime evidence records each tested binary
separately in tests/visual/riina-vector-evidence.json. Exact IINA icon matching,
other DPI values and other OS interface rendering remain unverified.

Expanded portable interface CI (2026-10-10): run 38002664122 passes on macOS
(job 114064174025), Windows (114064174241) and Linux (114064174258). Each job
runs core and mpv library unit tests and strict lint. The suite has 13 tests;
no native decoding or GUI test runs in these jobs. Recorded source commit and
job scope: tests/visual/portable-interface-ci-evidence.json.

Presentation boundary (2026-10-10): 17 unit tests, strict Clippy, formatting,
optimized Windows build and all four explicit native tests pass (2.06 s). The
shared controller now takes an opaque presentation instead of a Windows integer;
its retained native window is released after the sole native player owner joins.
Native PQ startup, keyboard seeking, fullscreen/Escape and normal close were
observed at 300% DPI. New app-data logs report the expected 10-bit PQ desktop
and float scRGB swapchain and end with native VO uninit. Default GL startup,
seeking and close also passed. Tested binary and capture hashes are in
tests/visual/presentation-boundary-evidence.json. Other OS application playback
and forced-lifecycle stress are not covered by these checks.

Asset publication correction (2026-10-10): Windows CI run 38002664116 failed
the asset integrity step because the manifest had hashes of local CRLF SVG
files while the published Git bytes use LF. All 13 records now match published
SVG bytes. Checkout validation, Git-index validation and a clean staged-tree
copy pass. check-assets.py --staged is the publication gate for future asset
changes. This corrects provenance bytes; SVG geometry is unchanged.
