# Verification record · native Windows · 2026-10-09

## Video aspect and rotation · 2026-10-10

Repaired Windows shared-OpenGL source f3cfed8 passes the custom-field focus
retest: entering 8:6 and Return, then Tab focuses the 0° rotation control.
At an interior paused position of 2 seconds, Right and Left on that control
leave position 2 and volume 37 unchanged; no player seek/pause/volume shortcut
leaks through. A Default custom submission also retains that state. Normal
close exits 0 with empty stderr. Scoped observations and binary digest are in
tests/visual/video-settings-repair-evidence.json. The AX tree omits selected
segment state, so these observations prove focus and input routing, not the
displayed angle or native aspect value. Those effects are separately exercised
by the native test. Strict lint and optimized release build pass after repair.
Current GUI material/pixels, full native segmented focus behavior, horizontal
overflow indicator/wheel mapping, Unicode decimal-digit syntax, video track
dimensions/FPS labels, OSD and native-video/macOS/Linux runtime remain open.

Initial GUI at 9a9d1ad exposes the Video shortcut/panel, video catalog, all six
aspect choices, editable custom ratio and four rotation choices. Setting 2.39:1
and Return retains Play and volume 37 without an error, but the AX snapshot
does not prove native ratio or selection. Tab then remains trapped in the
custom field. Scoped initial evidence is video-settings-initial-evidence.json;
normal close exits 0 with empty stderr. The repair passes Tab through the edit
scope, exposes selectable segment state and consumes unmodified Left/Right as
choice navigation rather than playback seeks. Its GUI retest remains pending.

Original portable video intent and a shared trailing Video panel now include the
reference's six aspect presets in order, 40x24 custom ratio field, four rotation
choices and video track catalog. Shift+primary+V opens the panel (Control on
Windows/Linux, Meta on macOS). Exact custom text is retained for selection:
8:6 does not select the 4:3 segment. Empty custom input is ignored; invalid
nonempty ratio syntax requests Default (-1). Rotation selection uses observed
native state. The original film icon has MIT provenance; no Apple art is copied.

Two core checks pass for ratio syntax, exact text/preset identity and quarter
turn validation. The native video_geometry_controls_preserve_pause_and_unrelated_options
test passes on the original catalog fixture: all five non-default presets,
2.39:1 and 8:6 change both video-aspect-override and video-out-params display
ratio (within integer display-size rounding). Invalid custom input restores -1;
90/180/270/0 rotation properties are observed with pause and volume 37 retained
and no playback error. Initial mpv "no" override is -2; it remains unchanged
until an action, while explicit Default uses IINA's -1 container preference.
All 17 explicitly enabled native tests and workspace unit checks pass.

This native test verifies output display parameters and rotation option values,
not displayed rotated pixels. GUI controls, overflow behavior, native segmented
keyboard semantics, custom-entry focus handoff, aspect/rotation OSD, menus,
crop/speed/hardware/equalizer sections and other OS runtime remain open. Current
materials, typography and row metrics remain provisional until reference capture
comparison. GUI retest is pending. Work remains local.

## Localized track badges · 2026-10-10

Repaired source 3b81c9d is verified in both Windows application routes: shared
OpenGL and experimental native gpu-next/D3D11 expose the same readable audio,
primary and secondary subtitle language labels. Both retain Play and volume 37
and close normally with exit 0; shared GL stderr is empty and native stderr has
only its existing chrome-size diagnostic. Exact binary hashes and scoped AX
observations are in language-badges-repair-evidence.json and
language-badges-native-evidence.json under tests/visual. Formatting, strict lint,
all workspace unit checks, all 16 explicitly enabled native tests, optimized
Windows build, 11 fixture hashes and traceability checks pass. This proves the
tested normal-close repair, not context-loss/resume or repeated shutdown stress.
Rendered geometry, punctuation/extension equivalence and macOS/Linux application
runtime remain open. Work is committed locally; nothing was published.

Initial default-OpenGL GUI run at 7622217 exposes English/German/French,
Chinese (Traditional), Portuguese (Brazil) and unknown qzz badges in both
subtitle lists, plus English for the audio track; unspecified language has no
badge. Playback controls retain Play and volume 37. Pointer geometry is
unavailable, so keyboard shortcuts open the panels. Closing exposes a renderer
teardown crash (exit 101): clearing the image property during backend suspend
re-enters its mutable window borrow. Scoped observations and stderr digest are
in tests/visual/language-badges-initial-evidence.json. This is a failed shutdown
run, not an overall GUI pass. The repair clears the borrowed frame before
ordinary hide and queues context-loss UI invalidation until the borrow ends,
guarded against a subsequent context setup. Runtime retest remains pending.

Original shared presentation now resolves language codes independently of the
raw mpv track metadata. Windows reads the user's locale name; Linux uses system
locale discovery; the macOS route calls Foundation directly. Three unit checks
pass for bibliographic/terminologic ISO aliases, underscore/BCP 47 script and
region names, German/French/Japanese display locales, absent/invalid identifiers,
explicit und and unknown qzz fallback. The pinned ICU4X formatter is experimental:
non-English locale punctuation, locale extensions and exact macOS equivalence
remain open; these unit expectations do not certify Foundation equivalence.

The original track-languages.mkv fixture is hash-verified. Its native libmpv test
passes: seven subtitle tracks retain eng/ger/fre/zh-Hant/pt-BR/qzz metadata; und
is absent in the observed catalog. German primary and Traditional Chinese
secondary selections coexist while paused, with all title/language metadata
unchanged and no playback error. This does not prove displayed subtitle text,
badge geometry, other OS runtime or a pixel comparison. GUI verification remains
limited to the label and ordinary-close observations recorded above. No public
update was made.

Normal-playback diagnostic removal: formatting, strict workspace Clippy and the
optimized Windows application build pass. Actual default-GL accessibility state
shows catalog.mkv, end position 5.0, volume 37 and playback controls without the
previous phase/speed text. Left seeks to 0.0 and close removes the process.
Screenshot output was blank, followed by monitor capture error 0x80070057;
therefore no visual-rendering pass or screenshot comparison is claimed. Error
status remains bound to playback errors; an error-state GUI check was not rerun.

Official reference run 38005949498 passes at 3504129. Reviewed image frame 048
matches paused 2.0 seconds observed before and after capture. The seek OSD is
visible while the main OSC is hidden. Reference metadata/hashes and bounded
observations are recorded in tests/visual/reference-captures.json. This establishes
a real reference state, not full pixel parity or a matched-size comparison.

macOS integration run 38005017036 completed successfully at 4b21133. Job
114071692801 used macOS 26.6.2 (25G83), arm64, Homebrew mpv 0.41.0 and
libmpv SHA-256 f701babc6b30630ce0e59243dcafce6dc8d6380024c18c231da15b2ed524d162.
All four actual native tests pass in 3.45 seconds. Dynamic loading, original
fixture decoding, PQ/HLG source metadata, replacement, seeking, stop/failure,
catalogs, tracks and playlist commands are exercised through null video output.
This does not verify GUI rendering, native panels, sound, displayed HDR or hardware
decoding. The checked public summary is tests/visual/macos-native-evidence.json.
Subsequent full application run 38004680216 also passes on both macOS/Linux,
including concurrent preference replacement and failure cleanup tests at f5c265f.

Application run 38004581488 completed successfully at a48a1de: macOS job
114070320144 and Linux job 114070320349 passed full workspace lint, workspace
unit tests and cargo build -p riina --locked. See application-port-ci-evidence.json.
New storage-test run 38004680216 has a successful Linux job; macOS remains live.
Manual real libmpv macOS run 38005017036 is queued at 4b21133. These statuses are
separate from native GUI/playback/HDR evidence and preserve exact tested revisions.

Native-test portability increment: tests can explicitly select an existing
absolute library with RIINA_TEST_MPV_LIBRARY; Windows keeps its pinned DLL default.
The first reruns exposed a real duplicate-subtitle bug: mpv reported a filename
without the Windows verbatim prefix while the requested filename retained it.
The command planner now treats ordinary/verbatim drive and UNC spellings as
equivalent, with a Windows regression test; unrelated filenames remain distinct.
A separate transient zero-track reload snapshot exposed an insufficient test
barrier. Reload checks now wait for the selected external track as well as delay.
All 19 Windows unit tests and strict lint pass; all four native tests passed in
1.90 seconds before the barrier correction, then eight consecutive corrected
catalog test runs passed. The complete corrected suite then passed (1.92 seconds),
with formatting and strict lint passing again before publication.

The manual macOS libmpv workflow provisions Homebrew's runtime and records formula
metadata, mpv version, OS/architecture, library hash and linkage before running
the same original-fixture tests. Its runtime is not the Windows pinned DLL;
results cannot establish native window composition, displayed HDR or audible output.

The macOS/Linux storage backend now has two native-target tests: simultaneous
Unicode preference replacements must leave a complete payload and no temporary
files; replacement into an existing directory must fail, preserve its contents
and clean the owned temporary file. These tests await the application matrix.
Windows formatting and strict lint pass, but exclude this target-gated code and
therefore are not storage-runtime evidence for macOS/Linux.

Application port run 38004319189 reached application compilation on macOS/Linux
but failed strict lint: UiSession's retained player field was only explicitly read
by Windows native shutdown. UiSession now releases that owner explicitly in Drop
on all targets before releasing its presentation. GL callback ownership continues
through renderer teardown. The corrected application CI remains pending.

2026-10-10 shared GL port services: Windows strict workspace lint, 18 unit tests,
optimized x86_64 MSVC application build and all four explicit native libmpv tests
pass (native tests: 2.03 seconds). Windows GUI picker behavior was not rerun for
this extraction. macOS/Linux service source is target-gated and is not exercised
by those local Windows checks. Application build run 38004319189 is active with
macOS job 114069482844 and Linux job 114069483016; no result is claimed yet.
Earlier portable run 38004060826 confirms all 17 core/interface tests and strict
lint on all three systems at commit 7da08a1. This excludes application compilation,
native engine loading and GUI runtime support.

2026-10-10 shared file-selection extraction: `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets --locked -- -D warnings` and
`cargo test --workspace --locked` pass. Eighteen unit tests comprise fourteen
core tests (including four moved filesystem/selection tests), three mpv-interface
tests and one Windows native filename-order adapter test. The four explicitly
ignored native playback tests were not run for this extraction. Shared tests use
an injected synthetic ordering to verify collation delegation; they do not prove
macOS localized ordering or Linux locale equivalence. The existing portable CI
automatically runs all seventeen core/interface tests on its three targets;
results for this change must be read from its actual run before claiming success.

Windows run 38003518018 passed asset integrity, formatting and lint, then failed
a folder test because parallel fixtures shared a clock-derived directory name
(Windows error 183, AlreadyExists). Fixture allocation now includes an atomic
sequence and retries existing names without claiming or removing their contents.
This fixes test ownership; it does not change application discovery behavior.

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
# Seek OSD state verification · 2026-10-10

Local subtitle cue navigation: 24 Windows unit tests, strict lint and eight
explicit native tests pass (8.75 seconds). The external original SRT fixture's
Unicode first cue and English second cue are verified through observed subtitle
text as well as position and completed native seek. Next/previous keep pause and
do not change delay. With sub-delay=0.25, navigation follows the adjusted second
cue, and `sub-seek 0` returns from inside it to its adjusted start. Cue positions
allow decoded-frame rounding; requested positions are not reported as actual.
The engine owns cue resolution, matching its documented
[subtitle seek command](https://mpv.io/manual/stable/#command-interface-sub-seek).
Shared Shift+Arrow routes compile but are not GUI-tested in this increment.
Embedded subtitle prefetch, secondary selection, boundaries and rendering parity
remain unverified.

Windows GUI shortcut verification at edeb05c: with the playback slider focused,
Alt+Right moves the observed position from 2.000 to 2.042 seconds and Alt+Left
returns to 2.000. Comma moves to 1.958; period returns to 2.000. The pause button
remains Play. Alt+Up/Down changes volume 37→38→37 without moving playback.
Control+period shows the stopped/open-file state and returns focus to the root.
The seek OSD clock appears in the accessibility tree after seeking/backward
stepping, then is absent later; exact timeout and fade were not measured.
Screenshot capture fails with CreateForMonitor 0x80070057; no rendered-frame or
pixel-parity conclusion is made. Clean shutdown leaves no player process.
Binary hash and scope: tests/visual/frame-shortcut-windows-evidence.json.

Local frame navigation: 24 Windows unit tests, strict lint and seven explicit
native tests pass (8.43 seconds). Starting at the observed paused two-second
frame, two forward and two backward commands each move by 1/24 second within
0.002 seconds and return to two seconds, retaining pause and no native error.
The shared UI maps Alt+Right/Left and period/comma to these actions. Modifier
priority separates playlist navigation, Stop, speed and fine volume controls
from plain arrows, including a focused slider. Enter requests fullscreen only
when it is not already active. These routes compile but were not exercised
through the GUI in this increment. Variable-rate media, frame boundaries and
displayed frame capture remain unverified. See pinned
iina/config/iina-default-input.conf and the documented
[mpv frame commands](https://mpv.io/manual/stable/#command-interface-frame-step).

A–B lifecycle: the independent backend selects `reset-on-next-file` for just
`ab-loop-a,ab-loop-b`, matching the option contract recorded in pinned
MPVController.swift line 575. The native test loads media with an active loop,
replaces it and requires observed playback past the former B point. It repeats
that check after Stop/reopen and playlist advance, with both observed points
cleared. This proves the decoder reset as well as cache invalidation. The
[mpv reset option](https://mpv.io/manual/stable/#options) documents the per-file
reset mechanism. These null-video tests do not establish displayed GUI parity
or persistence behavior across application sessions.
Final Windows checks pass 24 workspace unit tests, all six explicit native tests
(8.05 seconds), strict lint and the optimized application build.

Local A–B repeat increment: Windows workspace checks pass 24 unit tests and
strict lint. All five explicit native tests pass in 2.56 seconds. The added test
observes A=0.000001 and B=0.6, resumes from 0.25 and requires a completed native
seek with position below 0.2 to establish actual looping. It then verifies native
clear/A/B/clear cycling. Frame-rounded B is compared with the observed paused
clock, after native seek completion; a requested decimal is not assumed to be
the decoded frame time. Unit coverage rejects invalid point pairs before any
submission, preserves zero through the pinned minimum and clears cached points
on replacement. Other-OS native runtime, GUI shortcuts, loop markers, menus and
OSD are unverified. Commands use the documented
[mpv A–B loop interface](https://mpv.io/manual/stable/#command-interface).

OSD preference increment: 23 Windows workspace tests and strict lint pass.
Legacy preference JSON receives enabled/one-second defaults; disabled OSD and
fractional timeout values round-trip while preserving saved volume. Zero is
accepted by the pinned reference field's nonnegative range; negative and
unrepresentable durations are rejected. Settings UI and runtime timeout checks
remain open.

The shared application reducer tests cover coalesced native completion counters,
an overlapping seek, delayed/unknown clock values, ordinary ticks, file replacement,
Stop and errors. Notifications remain pending until seeking ends and the observed
clock/duration are usable; replacement and failure discard stale notifications.
Windows workspace tests pass 22 tests with strict lint. The preceding backend
increment's four native integration tests pass in 2.13 seconds, including an
observed paused two-second seek and native completion counter advancement.
These checks establish state handling, not rendered appearance or timeout parity.

Windows GUI seek observation: a temporary instrumented shared-GL build confirms
the native completion counter advances from 0 to 1 after a seek from EOF to
2.0 seconds, with seeking false, phase Paused and no error. The accessible
timeline agrees. Panel text was absent from the sampled accessibility tree;
its rendered visibility remains unproven. Both screenshot attempts failed with
the computer-use helper's CreateForMonitor error 0x80070057. Exact scope and
diagnostic binary hash are in tests/visual/seek-gui-observation.json. Temporary
tracing was removed; no capture or pixel comparison is claimed.
# Local subtitle timing verification · 2026-10-10

The Windows native subtitle navigation test now queues twenty +0.1-second
adjustments without consuming playback snapshots and observes the delay rise
from 0.25 to 2.25 seconds. Four -0.5-second adjustments restore 0.25, then an
absolute reset restores zero. Pause is retained and no native error is reported.
All eight explicit native tests pass in 8.85 seconds; 24 workspace unit tests and
strict lint pass. Shared uppercase Z/X/C and Alt+Z/X bindings are implemented.
Subsequent Windows keyboard observations verify these bindings; rendered
feedback and macOS/Linux runtime remain unverified.

## Audio timing and shared keyboard verification · 2026-10-10

The native catalog test queues twelve +0.1-second audio adjustments from -0.2,
observes 1.0, then queues two -0.5-second adjustments and observes zero. A negative
fine adjustment followed by reset is also observed; pause is retained and no
native error is reported. All eight native tests pass in 8.98 seconds, with
24 workspace unit tests, strict lint and a successful Windows release build.

Real keyboard inputs observe audio delay 0 → 0.5 → 0.6 → 0.5 → 0 using shifted
parentheses and Alt fine steps, then reset -1 to zero with Shift+underscore.
Uppercase subtitle shortcuts observe 0 → -0.5 → -0.6 → -0.5 → 0 and reset 1 to
zero with C. Both resets work with the delay slider focused. Playback remains
paused at two seconds throughout. The fixture has no selected subtitle track
in this GUI check; native SRT coverage separately verifies delay accumulation.
The app closes cleanly with no remaining window or process. Exact binary and
canonical source hashes and observations are in
tests/visual/timing-shortcut-windows-evidence.json. No screenshot was obtained:
CreateForMonitor failed with 0x80070057. This verifies input/state routing, not
rendered feedback, acoustic synchronization or macOS/Linux runtime behavior.

## Native volume and speed accumulation · 2026-10-10

The shared UI now issues relative volume deltas and native speed multipliers;
it no longer computes either from the last displayed snapshot. The native
volume range starts at 100 and changes to 200 only with explicit boost. Disabling
boost sets the range back to 100 and applies a zero relative adjustment to clamp
the current engine value. Startup restores this policy before saved volume.
These operations use the documented [mpv property manipulation commands](https://mpv.io/manual/stable/#property-manipulation).

The Windows playback lifecycle test queues twelve +5 volume commands from 50
without reading snapshots and observes 100, then queues twenty-two -5 commands
and observes zero. With boost enabled it observes absolute 150 and 200, then
queues twelve +5 adjustments from 150 and observes 200. Disabling boost restores
100. Three queued speed doublings reach 8; three halvings restore 1; ten 1.1
multipliers reach 1.1^10; reset restores 1. Playback retains pause and reports no
native error. Eight explicit native tests pass in 9.19 seconds, alongside
24 unit tests, strict lint, formatting and the Windows release build.
Current GUI routing, speed/volume feedback appearance and other-OS runtime
remain unverified. Earlier GUI evidence belongs to its recorded source hashes.

## Relative chapter navigation and panel shortcuts · 2026-10-10

The shared shortcut route delegates relative chapter changes to native `add
chapter`, preserving pause and the engine's [chapter restart threshold](https://mpv.io/manual/stable/#options-chapter-seek-threshold).
This differs intentionally from the existing sidebar chapter selection action,
which resumes playback according to the pinned reference. The native catalog
test verifies previous/next seeks between zero and two seconds while paused,
then backward navigation from half a second inside the first chapter returns
to zero without changing media generation or reporting an error. All eight
explicit native tests pass in 9.40 seconds; 24 workspace unit tests, strict lint,
formatting and the Windows release build pass.

Actual Windows Control+Shift+period/comma inputs with the timeline focused seek
from 0.5 to 2 and back to zero; the Play button continues to indicate pause.
Control+Shift+C and Control+Shift+P each open and close the corresponding panel
without moving the timeline or resuming playback. Evidence with canonical source
hashes, binary hash and seven observations is in
tests/visual/chapter-shortcut-windows-evidence.json. The app closes with no
remaining window or process. No screenshot/pixel comparison was attempted in
this check. Last-chapter/playlist transition, backward behavior beyond the
five-second threshold, other keyboard layouts and other-OS runtime are pending.

## File repeat shortcut and actual playback wrapping · 2026-10-10

The pinned default Shift+Meta+L command cycles file repeat between infinite
and off, leaving playlist repeat alone. The shared typed intent uses native
`cycle-values loop-file inf no`; macOS uses Command and Windows/Linux use
Control for the primary modifier. Plain primary+L remains the A–B loop action.

The explicit Windows native test enables playlist repeat, toggles file repeat
on, seeks to 4.8 seconds in the five-second fixture and resumes. It observes
playback wrap below half a second without changing media generation, with both
repeat properties still infinite and no error. After pausing, toggling file
repeat off leaves playlist repeat infinite and restores its effective mode.
It then clears playlist repeat and tests file-only on/off transitions while
paused. All nine native tests pass in 9.28 seconds, with 24 workspace unit tests,
strict lint, formatting and the release build.

Actual Windows Control+Shift+L inputs with the timeline focused change the
accessible repeat control Off → File → Off. Position remains two seconds and
the Play button retains its paused indication. The three observations and exact
binary/canonical source hashes are recorded in
tests/visual/file-repeat-windows-evidence.json. The app closes with no remaining
window or process. This check has no screenshot or pixel comparison; repeat
OSD, rendering, alternate layouts and macOS/Linux runtime remain unverified.

## Native track cycling · 2026-10-10

Typed cycle intents delegate to native video/audio/subtitle ID properties rather
than calculating the next ID from a possibly stale catalog. A new explicit
Windows native test adds an external audio file, observes its selection, cycles
through disabled → internal → external audio, and checks video selection is
retained. It adds an external SRT, cycles disabled → selected subtitle and checks
audio is unchanged, then cycles disabled → selected video while retaining both
audio and subtitle selection. Each observed transition retains pause and reports
no native error. All ten native tests pass in 9.34 seconds, with 24 workspace unit
tests, strict lint, formatting and a successful release build.

Shared shortcuts implement IINA Command+Control+A/S/V on macOS; Windows/Linux
use Control+Alt+A/S/V as described in docs/platform-differences.md. Audio and
subtitle panel toggles use Shift+primary+A/S, and plain v toggles subtitle
visibility. The video panel remains missing. Current GUI input routing,
rendered feedback, AltGr/alternate layouts, multiple embedded subtitle/video
tracks and other-OS runtime are unverified; native track-property observation
does not prove actual audible or visible output.

## Subtitle scale and vertical position · 2026-10-10

Shared G/F and r/R/t shortcuts issue typed native relative commands for
subtitle scale and position. The backend observes both properties as optional
numbers; unavailable, invalid or nonfinite values stay unknown rather than
inventing a default. Session option observations survive media replacement;
track catalogs and subtitle text still have file ownership.

The explicit Windows SRT test queues ten +0.1 scale changes without consuming
snapshots, observes 1 → 2, then queues ten -0.1 changes to restore 1. Fifteen
-1 position changes move 100 → 85 and fifteen +1 changes restore 100. Large
relative adjustments verify native scale clamping at 0 and 100, and position
clamping at 0 and 150, then restore scale 1 and position 100. These ranges match
the [mpv subtitle options](https://mpv.io/manual/stable/#options-sub-scale).
The final states retain pause, zero delay, selected external subtitles and no
native error. All ten native integration tests pass in 9.46 seconds.

The expanded Slint shortcut expression tree initially exhausted the Windows
build script's default stack (STATUS_STACK_OVERFLOW). Interface compilation
now runs on a joined standard Rust thread with a bounded eight-MiB stack on
every host. This fixes the observed compiler failure without adding platform
linker assumptions. Windows strict lint, all 24 unit tests, formatting and the
release build pass after the fix. Current GUI
shortcut routing, settings controls, actual rendered subtitle geometry/font
parity and macOS/Linux build/runtime verification remain open.

## Independent secondary subtitles · 2026-10-10

Typed secondary selection targets `secondary-sid`; Alt+v cycles only secondary
visibility. Backend observations include secondary text and an optional visibility
flag. Track decoding now retains [mpv's main-selection metadata](https://mpv.io/manual/stable/#properties-track-list),
where lane zero is primary and lane one secondary. Shared primary catalog rows
use this metadata, preventing a secondary-only track from appearing selected
in the primary list. Unknown lane metadata is not asserted to be primary.

The native test loads two distinct original CC0 SRT files, assigns independent
lanes and observes Unicode primary text and distinct secondary text at 0.5 seconds
while paused. Hiding secondary retains primary visibility and selection; hiding
primary leaves secondary visible. Disabling primary retains a selected secondary
lane, then disabling secondary removes both selections. Replacement clears old
secondary text and tracks. No native error occurs. The secondary override option
uses scale, matching IINA's pinned preference default; native initialization accepts
it, but styled ASS output has not been visually verified. GUI selector controls,
Alt+v routing, subtitle OSD, native-rendered styling and other-OS runtime remain
pending. The original fixture has a reproducible generator and manifest hash.
All eleven explicit Windows native tests pass in 9.43 seconds after the default
override change. The 24 unit tests, strict lint, formatting, Windows release
build and all fixture hashes pass. Public GitHub was not updated.

## Secondary subtitle delay · 2026-10-10

Typed secondary delay sets the independent native property and observes an
optional finite value. Unavailable/nonfinite values clear the observation.
The two-lane SRT test applies a one-second secondary offset and seeks to 2.3
seconds: primary text contains Chapter two while secondary text still contains
bonjour from the first cue. Primary delay and primary selection remain unchanged,
pause is retained and no native error occurs. Reset to zero and seek to 0.5
seconds restore the aligned first cues before the visibility tests continue.
All eleven explicit Windows native tests pass in 10.20 seconds; all 24 unit
tests, strict lint and formatting pass. Secondary timing UI, visual rendering
and macOS/Linux runtime remain unverified. No public update was made.

## Independent secondary position · 2026-10-10

The subsequent primary absolute setter is now covered by the same native test:
after secondary positions finish at 100, primary positions 40, zero, 150 and 100
are observed independently. Every observation retains secondary position 100,
primary delay, secondary delay one second, primary and secondary track IDs,
pause and no native error. All eleven Windows native tests pass (9.68 seconds).
These are property observations, not proof of the rendered subtitle placement.

A typed secondary position setter validates finite values in the native zero
to 150 range before submission. Optional native observations clear unavailable,
nonfinite or out-of-range values. The two-lane subtitle test changes secondary
position to 25, zero, 150 and 100 while its delay remains one second. Each
observed transition retains primary position, primary delay and selection,
secondary delay and pause with no native error. All eleven explicit native
tests pass in 10.19 seconds; 24 unit tests, strict lint and formatting pass.
This proves independent property handling, not rendered placement. Shared UI
position controls, actual visual geometry and macOS/Linux runtime remain open.

## Subtitle font engine contract · 2026-10-10

`subtitle_font_selection_preserves_other_style_options` verifies the initial
sans-serif option, then loads the original catalog and external SRT. Configured
serif, monospace and a custom Unicode name are observed without disturbing
size, border, colors, timing, selection or pause. Empty entry restores sans-serif.
All 15 native tests pass in 10.32 seconds. The two portable font tests preserve
names as a single native argument and reject control characters before dispatch;
all 32 unit tests and strict lint pass. This is configured-font evidence, not
font installation, libass resolution/fallback or rendered glyph evidence.
The font picker UI and actual installed families/faces/search/preview remain open.

Font chooser repair retest (Windows 11 x64, shared OpenGL, source 321225e):
the release build succeeds. Accessibility set-value edits the preview to a
Unicode sample. After committing Verdana, reopening exposes that late family
in the virtualized list, and Tab into faces focuses Regular. Family focus is
reported as the list container, so its exact keyboard cursor remains unproven.
Restoring the blank custom entry returns sans-serif, with font size 55, paused
playback and volume 37 unchanged. Both owned test windows were closed.
Scoped observations and exact executable SHA-256 are recorded in
tests/visual/font-picker-repair-windows-evidence.json. No screenshot, glyph
identity, native modality, exact geometry or other OS runtime pass is implied.

Font chooser lifecycle retest (Windows 11 x64, shared OpenGL, source 80b4000):
the player font-size picker accepts Down 55→60 and Up→55 while the ordinary
chooser stays open. Opening again preserves the same native chooser window and
its Unicode preview draft. Parent Alt+F4 dismisses the chooser; after settling,
native window inventory and process inspection contain no riina instance.
Exact binary hash and scoped observations are saved in
tests/visual/font-picker-lifecycle-windows-evidence.json. An indexed parent
set-value attempt encountered the automation cache; fresh parent activation and
keyboard navigation succeeded. These findings do not verify native-video
lifecycle, other OS runtime, application-wide reusable picker, Settings sheets
or exact visual metrics. Formatting, strict workspace lint, all workspace unit
checks and the optimized Windows release build passed for this source.

Native font chooser lifecycle retest (Windows 11 x64, source ebf6d9c):
the native-video attributes hook consumes its pending owner once, isolating the
transparent overlay attributes from subsequently constructed auxiliary windows.
The corrected chooser is exposed as its own targetable Choose a Font window.
Parent Left changes the paused five-second fixture from endpoint to zero while
the chooser remains open, with volume 37 and its Unicode preview draft retained.
Repeated opening reuses the same chooser and preview. Parent activation followed
by Alt+F4 dismisses the chooser and host; settled native inventory and process
inspection are empty. Scoped evidence and exact binary SHA-256:
tests/visual/font-picker-native-lifecycle-evidence.json. The pre-fix chooser was
nested beneath the host and activating the host left chooser focus in place.
One indexed parent setter hit the automation cache; fresh keyboard navigation
was used instead. Formatting, strict lint, workspace unit checks and release
build pass. No screenshot, pixel comparison, calibrated HDR, other OS runtime,
global reusable picker or Settings sheet completion is implied.

Reusable chooser state retest (Windows 11 x64, shared OpenGL, source 37c4623):
the Unicode preview sample and Verdana search survive Escape Cancel, accepted
custom-entry Enter and native Alt+F4 close/reopen. Cancelling Uncommitted-Font
keeps playback sans-serif; reopening refreshes the entry from that observed font.
Accepting Verdana and reopening restores the readable face list while retaining
search/sample. Blank custom-entry Enter restores sans-serif, with size 55, pause
and volume 37 retained. Settled player shutdown leaves no riina windows.
Scoped observations and exact executable hash:
tests/visual/font-picker-reuse-windows-evidence.json. Formatting, strict lint,
unit checks and release build pass. The hidden-scan race, already-visible custom
font refresh, native-video reuse, multiple-player/Settings reuse and other OS
runtime remain untested. No glyph identity or pixel parity is proved.

Font chooser layout/Return retest (Windows 11 x64, shared OpenGL, source 10d6623):
accessibility exposes Choose a font, Typeface, preview and Or enter the font name
in the corrected order. Return from preview commits custom Verdana; Return from
filter and the restored Regular face accepts and closes the chooser. Preview
sample persists on reopening. Blank custom entry restores sans-serif, with size
55, pause and volume 37 unchanged. Settled close leaves no riina windows.
Exact executable hash and scoped observations:
tests/visual/font-picker-return-windows-evidence.json. Formatting, strict lint,
unit checks, release build and 108-row traceability check pass. Labels/order and
keyboard callbacks are tested; rendered geometry, clipping, materials, modified
Return, native-video/other OS runtime and pixel parity remain unproved.
Double-click face acceptance remains missing and separately inventoried.

Original font row checks and repairs (Windows 11 x64, shared OpenGL):
ca9000d verifies filtered family/face draft selection, but accessible focus reports
the window; double-click injection fails because coordinate geometry is unavailable.
893e8f5 fixes filtered focus, but End/Home triggers an AccessKit consumer 0.29.0
missing-focus-ID panic and diagnostic exit -1073740791. Retaining rows alone on
be545eb still fails first family Tab, exiting 101. These exact-binary failure
observations are saved in font-rows-initial-windows-evidence.json,
font-rows-focus-windows-evidence.json and font-rows-retention-failure-evidence.json
under tests/visual. Failures are preserved rather than relabeled as shutdown passes.

ae7f7c5 delays row focus publication until after model-tree updates. First family
Tab, End/Home, PageDown/PageUp, filtered model replacement and face Tab/Down now
report the corresponding rows after settling. Regular Return commits Verdana;
diagnostic shutdown exits 0 with empty stderr. That binary then reveals a separate
reopen race which selected first-face Italic before queued Regular restoration.
tests/visual/font-rows-deferred-focus-evidence.json contains scoped observations.
5d865e7 suppresses automatic first selection while restoration is pending; the
same Regular commit/reopen sequence now retains Verdana and Regular focus.
Blank entry restores sans-serif, preserving size 55, pause and volume 37;
shutdown exits 0 with empty stderr. Exact hash and scoped observations are in
tests/visual/font-rows-selection-restore-evidence.json. Formatting, strict lint,
workspace unit checks, release build and 108-row traceability checks pass.
Native toolkit double-click wiring is implemented, but injected pointer testing
remains unavailable. The 32ms accessibility transition, retained-row memory and
performance, rendered geometry/materials, native-video and other OS runtime
still need evidence; this is not full accessibility or pixel parity.

## Subtitle color engine contract · 2026-10-10

Shared UI follow-up: the Windows keyboard opens each well. Changing text red
to 64 and opacity to 128, closing with Escape and reopening reports those
observed native values; Escape restores focus to the well. Text is restored to
opaque white. Border and shadow editors report independent black/clear defaults.
Pause and volume 37 remain unchanged. See subtitle-color-wells-windows-evidence.json
for exact binary/source hashes and accessibility observations. No tracks were
loaded, screenshots captured or GUI border/shadow changes tested. The original
shared RGBA popup is provisional; full native color panel and pixel parity remain
open. Release build, strict lint and all 30 unit tests pass for the shared wiring.

`subtitle_colors_preserve_alpha_and_other_style_options` observes white/black/
clear startup defaults, then loads the original catalog and external SRT. Text,
border and shadow changes preserve their independently chosen alpha and RGB
channels. Restoring defaults retains the selected subtitle, delay zero, font
size 55, border width 3 and pause with no native error. All 14 native tests pass
in 9.93 seconds. Two portable color tests cover alpha-first hex, floating-channel
precision, omitted alpha, malformed/nonfinite/out-of-range input. The 30 unit
tests and strict lint pass. No color picker UI, rendered color or other OS pass
is claimed.

Color wire contract: [mpv manual](https://mpv.io/manual/master/#options-sub-color)
documents RGB and alpha-first ARGB hex, or normalized slash-separated channels.
riina keeps validated RGBA doubles without routing through toolkit precision;
mpv may quantize its effective colors, which native observations report.

## Ten-minute relative seeks · 2026-10-10

`long_relative_seeks_preserve_pause_and_volume` uses the original 1300-second
silent PCM fixture. Paused native relative +600/-600 seeks from 100 seconds
observe 700 then 100 within 0.1 seconds, with volume 37 and no native error.
All 13 native tests pass in 9.98 seconds; 28 unit tests and strict lint pass.
The Windows keyboard check independently exercises Shift+Page Up/Down from
the focused timeline and observes 100 → 700 → 99.99996, preserving pause and
volume. Adding Control does not seek. Binary, source and fixture hashes accompany
the accessibility observations in tests/visual/long-seek-interior-windows-evidence.json.
This closes the earlier short-fixture displacement gap. Audio PCM does not prove
video keyframe seek accuracy; other OS runtime and rendered OSD remain open.

## Shared secondary selector controls · 2026-10-10

The subtitle pane adds a Secondary catalog after Primary with separate visibility
controls, based on the pinned SidebarSubtitlesPane two-lane requirement. The
Rust catalog maps main-selection 1 to secondary highlights and provides a None
entry. The shared selection callback submits a typed secondary setter with
stable string IDs. Visibility is disabled until the optional native flag is
known, then follows observed state. Strict lint and all 24 unit tests pass.
Existing native two-lane tests verify the target command semantics, but the new
UI wiring has not been exercised through GUI input. Row/switch geometry, complete
reference ordering, keyboard selection, secondary timing/position controls and
other-OS runtime remain pending. No rendered or pixel-perfect pass is claimed.
