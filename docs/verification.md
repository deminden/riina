# Verification

Results apply to the named revision, environment and test type. A unit test,
a native decode test and a desktop playback check establish different things.
Dates, platforms and exact scope are recorded per batch.

## Windows 4.2.1 release 2026-11-09

The [published release](https://github.com/deminden/riina/releases/tag/v4.2.1)
uses source `9fe324d` and the unchanged verified 4.2.0 engine. Its installer and
portable archive passed the exact-source, runtime, notices and checksum audits,
hosted native playback, installation/reinstallation/uninstallation and extracted
application checks. The original downloads also passed desktop playback,
fullscreen/controller reveal and normal shutdown on the Windows test PC.

All twelve HDR color patches match with controls shown, hidden and restored in
both Windows HDR modes. The reported movie was compared against VLC near 600
seconds with paused off/on/off/on switching. Windows display state was restored.
Full-desktop captures were inspected and remain private. Physical HDR calibration,
identical VLC tone mapping and audible output are separate acceptance work.

The release's [verification record](../tests/visual/windows-release-4.2.1-2026-11-09.json)
contains exact asset hashes, source/build identities and scoped checks. The shared
application import errors discovered during candidate checks were corrected;
the final Windows, Linux and both macOS application build/test checks passed.
This compilation evidence does not establish supported macOS/Linux playback.

## HDR display lifecycle review 2026-11-09

The later [controls composition diagnosis](../tests/visual/windows-hdr-overlay-review-2026-11-09.json)
identified a second, independent cause of the remaining contrast difference.
On the same paused frame, showing the transparent controls HWND lifted captured
black from RGB 0 to 4 and reduced HDR white from 255 to 196. Full negative DWM
frame margins remove that client tint; the fix changes window composition without
changing mpv's curve, brightness or saturation. Earlier comparison captures below
include this tint and are retained as historical evidence, not final color acceptance.
The rebuilt binary produced identical values for all 12 color patches with the
controls shown, hidden and restored in both Windows HDR modes. The reported movie
was compared again against VLC near 600 seconds; paused off/on/off/on switching,
native fullscreen/controller reveal, restoration and normal shutdown passed.
Build, formatting, strict Clippy, 723 workspace tests, 54 explicit native tests
and 68 Python tests passed. The native suite used the pinned development DLL and
a null audio sink. Physical HDR calibration and identical VLC tone mapping are
not established by these desktop captures. Windows HDR was restored to off;
the pre-existing installed-player process was left untouched.

The [VLC comparison](../tests/visual/windows-hdr-vlc-comparison-2026-11-09.json)
subsequently tested the fixed binary against VLC 3.0.24 on the reported movie
and original HDR color patches with Windows HDR off and on. Riina also passed
off/on/off/on switching while paused: its reported target peak followed the
desktop from 80 to 1499 nits. Neutral grays and primary/secondary color identities
passed in both players; their tone mapping, brightness and saturation differ.
HDR desktop captures flatten bright patches, so physical highlight clipping and
calibrated accuracy cannot be inferred. Windows HDR was restored to off. Raw
movie/private-desktop images remain local.

The subsequent [HDR contrast review](../tests/visual/windows-hdr-contrast-review-2026-11-09.json)
checks the reported HDR10 movie at the same paused timestamp before and after
the fix. Native Windows HDR conversion now retains the linear scRGB target
instead of applying an automatic SDR ICC profile. The measured shadow region
dropped from 12.929 to 6.485 on the captured 8-bit RGB scale; full-desktop captures
were inspected locally. Those captures contain private desktop context and are
not published. Live PQ, HLG and SDR file transitions confirmed that ICC processing
is excluded for native HDR sources and restored for SDR video. These observations
establish the reported contrast improvement on this SDR desktop; calibrated HDR
output and other display profiles still need hardware acceptance.

HDR output now responds to the native display observer as well as source changes.
Previously the observer updated only UI capability: a paused video could retain
PQ output after Windows HDR was disabled or the window moved to an SDR display.
The shared HDR controller sends the existing mpv reset command on that transition,
retains a manual HDR-off choice across display changes, and renews the default
policy for the next file. UI state changes only after the command is accepted.

Three headless controller regressions cover paused HDR-to-SDR transitions,
restoring requested output, manual disable, SDR-source gating and send failures.
These tests simulate display observations; they do not certify physical HDR
presentation or establish the cause of the reported movie contrast difference.
The [review record](../tests/visual/windows-hdr-display-review-2026-11-09.json)
records checks and remaining runtime acceptance.

## Input and plugin preferences review 2026-10-23

The [input review record](../tests/visual/windows-input-review-2026-10-23.json)
identifies this local batch based on `fc17fed`. The 1616-line key binding module
now separates profile data/loading, event conversion, compiled bindings, typed
dispatch, menus and editing. Each editor operation reads a profile once. The
plugin preferences coordinator separates inventory, update/recovery and permission
review/storage. Existing callbacks and all 18 tests across both modules remain.

Slint sends encoded special-key scalars, while the dispatcher/recorder expected
printable names. The event boundary now maps arrows, navigation, function and
editing keys to portable mpv names and ignores modifier-only events. Keyboard
lookup reuses a compiled normalized index. Menu accelerators use the last mapping
for each key, retaining the first key's menu order as required by IINA v1.5.0;
shadowed entries cannot override a profile through the native menu. Malformed
modifier sequences no longer silently override unmodified keys.

Plugin grant saves now share one read/modify/write path under a stable file lock.
The lock coordinates independent player processes; installation, updates and
rollback preserve other plugins' entries. The old code's concurrent regression
retained only 12 of 72 updates. Existing permission denials remain denied on update,
and malformed persisted state is rejected without being overwritten.

Formatting, strict all-target Clippy and the locked workspace build passed.
The normal parallel workspace suite passed 709 tests with 57 default ignores;
four successful child-helper invocations are excluded. All nine new regressions
passed. All 52 native integration and two native library tests passed explicitly
with the pinned development DLL and null audio. Script tests passed 68 checks.
Traceability passed for 260 requirements, 69 visual identities, 15 evidence links
and 68 local image hashes. Staged asset provenance passed for 74 records.

All ten automated Windows state checks passed in 9432 ms, including encoded Enter
and Escape dispatch through the active profile into native fullscreen and back.
The run used the pinned development DLL, null audio and an isolated profile/fixture;
normal shutdown left no Riina process. No desktop images were captured. These are
callback/state checks: physical keyboard layouts/delivery, interactive profile/plugin
dialogs, visible pixel/HDR parity, shared-GL and macOS/Linux application acceptance
remain separate work. Grant
concurrency is stressed with six independent file handles in threads; the new grant
path was not separately exercised across processes.

## Preferences and feedback review 2026-10-22

The [preferences review record](../tests/visual/windows-preferences-review-2026-10-22.json)
records this local batch based on `e561502`, completed 2026-10-23. The former
1957-line screenshot
preferences module now handles only screenshot intent. Preferences window ownership,
key binding actions, live decoder changes, refresh and edit commit handling have
separate modules. Shared validation routing replaces 21 repeated error branches;
callback-owned saved values no longer use redundant shared reference counts.
All 23 existing preferences callback registrations are retained.

Music Mode's automatic switching, playlist and artwork choices now save immediately.
Their live setters run after successful persistence, so a failed write cannot change
playback policy or resize the Music Mode window. Two persistence regressions failed
against the old save behavior, including a failed-write retry. Queued speed and
pause feedback is now discarded when its category is disabled, preserving unrelated
messages. OSD observation rules and regression coverage have separate modules;
all 20 existing OSD tests are retained, with two queue regressions added.

The plugin file module also separates player snapshots/event diffs, strict encoding
and tests from file capabilities. Accepted identifiers differing beyond character
128 previously shared a private storage directory. A standalone harness compiled
from the original storage-name function demonstrates the collision. Conversion
now hashes the complete identifier instead of truncating/replacing characters;
uppercase and Windows filename aliases are handled too. Ordinary paths stay stable;
ambiguous legacy directories are not migrated. New tests check private file
isolation for accepted long identifiers and non-aliasing portable storage keys.

Formatting, strict all-target Clippy and the locked workspace build passed.
The normal parallel workspace suite passed 700 tests with 57 default ignores;
four successful child-helper invocations are excluded. All six new regressions
passed, along with 68 Python script tests. Traceability passed for 260 requirements,
69 visual identities, 15 evidence links and 68 local image hashes. Staged asset
provenance passed for 74 records. The native integration suite was not rerun:
native/core behavior is unchanged; the preceding batch records 54 native tests.

All eight automated Windows playback/window checks passed in 8109 ms with the
pinned development DLL, null audio and an isolated profile/fixture. Playback,
volume, menus, fullscreen, controls, automatic minimize/restore and clean shutdown
were checked; no Riina process remained. No desktop images were captured. These
checks do not establish visible pixel/compositor/HDR acceptance. The Preferences
editor regressions run headlessly; interactive pane/dialog acceptance remains open.
No shared-GL, macOS or Linux desktop check was run in this batch.

## Player controller review 2026-10-22

The [player-controller record](../tests/visual/windows-player-controller-review-2026-10-22.json)
identifies the exact source and executable based on local commit `af5c906`.
The player root falls from 4826 to 3061 lines. Input/focus, lifecycle/system media,
controller feedback, LiveText, media discovery and rendering now have separate
modules. Both presentation paths use one session constructor, retaining native
presentation/HDR timers, GL ownership and resource teardown order. All 19 existing
player-module tests and the source's 118 direct plus 14 macro callback registrations
are retained.

This batch corrects automatic restore/focus resume, including a restore before its
pause acknowledgement, and polls the framed Windows playback owner for minimization.
LiveText waits for fresh capture results, rejects recognition from earlier paused
positions/seeks and copies pixels only for new jobs. Native captures acknowledge
rejection, success and failure so serial capture can recover. Settings panels now
override stale hover without bypassing controller-hide guards. Subtitle scans use
request identities; older files/settings cannot replace newer completion state,
and disabling/re-enabling discovery starts a fresh scan.

Formatting, strict all-target Clippy and the locked workspace build passed.
The final workspace suite passed 694 tests with 57 default ignores, using two
threads; its normal parallel rerun also passed. Four successful child-helper runs
are excluded from the total. Thirteen regressions were added and existing discovery
and native capture tests expanded. The two resume regressions failed before the
policy fix. All 52 native integration and two native library tests passed explicitly
with the pinned development DLL and null audio; 68 script tests also passed.
Traceability passed for 260 requirements, 69 visual identities, 15 evidence links
and 68 local image hashes.

One earlier default-parallel application test process terminated in `ucrtbase.dll`
with `0xc0000409`. The final runs did not reproduce it; the cause is unestablished
and the failed run remains in the local evidence rather than being counted as passed.

The byte-identical rebuilt application passed eight automated Windows checks in
7879 ms, including minimizing the actual native owner, observing playback pause,
restoring it and observing resume. The isolated fixture/profile used null audio.
Normal shutdown left no Riina process. No desktop captures were taken; visual
parity, audible output and HDR are unverified by this batch. Native macOS lifecycle,
manual OCR interaction and a new shared-GL desktop run remain separate acceptance
work. This focused review does not establish an exhaustive repository audit.

## History and plugin quality review 2026-10-22

The [quality-review record](../tests/visual/windows-quality-review-2026-10-22.json)
identifies the exact source and executable based on local commit `8c713ac`.
This focused review fixes an inverted history/recent-media lock order, expired
plugin timers being postponed by queued events, per-row history saves during
selection removal, and external-file creation races. Plugin text reads now bound
the actual read as well as checking file size. History clear actions serialize
disk and memory changes and preserve plugin-refresh failures in their status.

History persistence and plugin snapshot conversion have separate modules. The
plugin host falls from 2350 to 1023 lines, retaining all 12 existing tests across
the extracted modules. The player and history-window callback registrations and
session resource order are unchanged. Seven new regressions cover scheduling,
lock contention, successful/failed history batches and file-operation limits.
The timer regression failed before the fix, with its callback delayed until all
eight queued player events had run.

Formatting, strict all-target Clippy, the locked workspace build and 681 workspace
tests passed (57 default ignores; four successful child-helper invocations
excluded). Core/native source is unchanged; ignored native tests were not repeated.
Traceability passed for 260 requirements, 69 visual identities, 15 evidence links
and 68 local image hashes. Inventory completeness remains unchanged.

A byte-identical copy of the rebuilt application with the pinned development DLL
passed all six automated playback/settings/fullscreen/shutdown checks in 5989 ms.
The run used an isolated profile and media fixture with null audio, and normal
shutdown left no Riina process. No desktop captures were taken in this batch;
these checks do not establish visual parity, physical audio or HDR acceptance.
Manual history-window interaction, third-party plugin GUI behavior and macOS/Linux
application verification remain separate work. This is not an exhaustive audit.

## Runtime and player review 2026-10-22

The [continuation record](../tests/visual/windows-runtime-review-2026-10-22.json)
identifies the exact source and executable based on local commit `41dda63`.
This batch fixes stalled Promise continuations, the deleted package `require()`
bridge, shutdown waiting behind plugin callbacks, and network workers releasing
their concurrency slots before result delivery. Nine regressions cover these
boundaries; the first five failed against the preceding implementation.

Player observations, media catalogs, materials, plugin integration and
audio/video/subtitle callbacks now have separate modules. The player root falls
from 7416 to 4840 lines, preserving all 130 callback registrations, 13 existing
tests and the exact session resource layout. The plugin-runtime root falls from
4016 to 448 lines, with 24 existing runtime tests retained beside the extracted
APIs and worker. The remaining player setup is still a candidate for further
subsystem extraction; this is not an exhaustive repository audit.

Formatting, strict all-target Clippy, the locked workspace build and 674 workspace
tests passed (57 default ignores; four successful child-helper runs excluded).
The 18 previously verified core/native files are unchanged; the 54 native tests
were not repeated in this continuation. Traceability checks passed for all 260
requirement records and 69 visual identities; inventory completeness is unchanged.

A byte-identical copy of the rebuilt executable with the pinned development DLL
passed all six automated playback/settings/fullscreen/shutdown checks in 5956 ms
using an isolated profile and null audio. Normal shutdown left no Riina process.
The three 3840 × 2160 desktop captures passed non-black probes but showed unrelated
fullscreen content before, during and after the test. They were inspected and
rejected as Riina visual evidence; no visible-playback or compositor acceptance is
claimed. Raw captures stay local. Physical audio/HDR, third-party plugin GUI,
matched IINA pixels and macOS/Linux application acceptance remain unverified.

## Local review 2026-10-22

The [review record](../tests/visual/windows-code-review-2026-10-22.json) identifies
the local source changes based on `0de1be3`, executable and pinned development DLL.
The review fixes premature public shutdown, lost Auto seek decisions, disabled
plugin-menu descendants and stale non-finite clock observations. It separates
core contracts, native property decoding and plugin menus into focused modules;
all 20 exported core types and the 70 extracted tests are retained.

Locked build, formatting and strict all-target Clippy passed. The workspace run
passed 665 tests with 57 ignored entries; 52 native integration and two native
library tests also passed explicitly with null audio. The remaining default
ignores are two clipboard checks and a child-process helper whose parent runs it
four times. All 68 Python tooling tests, 260 requirement records, 69 visual
identities, 16 fixture hashes and 74 asset provenance records passed their checks.

A byte-identical copy of the rebuilt executable, paired with the pinned development
DLL in a fresh test folder, passed the desktop playback/settings/fullscreen/shutdown
smoke in 5896 ms. Full-desktop before/fullscreen/after captures were inspected at
3840 × 2160 with valid non-black probes. Video and the revealed controller were
visible together; normal exit left no Riina window or process. Raw desktop images
remain local. This is development verification, not release-package, physical
audio/HDR, live plugin-menu GUI, matched IINA pixel or macOS/Linux acceptance.

## Latest evidence

| Check | Revision | Result and limits |
|---|---|---|
| Refreshed 4.2.0 installer and portable release | [Refresh identity](../tests/visual/windows-release-4.2.0-refresh-2026-10-20.json) | Source `2e9aa46`; build/fmt/strict lint, 656 workspace tests (56 ignored), 51 native integration checks and 68 packaging tests passed. Saved binaries passed corrected install/reinstall/shortcut/playback/uninstall checks. Hosted portable/installed smokes: 6261/6878 ms; local upgrade preserved profile and passed installed smoke in 5890 ms. Full-desktop captures inspected; physical audio/HDR and full parity remain separate |
| Local settings and native artwork | [Settings/viewport identity](../tests/visual/windows-settings-viewport-2026-10-20.json) | Locked build, fmt, strict Clippy and 656 workspace tests passed (56 ignored). At 300% DPI, narrow Audio restores after Layout/Video; Fit recenters a moved 285 x 160 client unchanged. Artwork fills its own 300 x 169 region above controls, hides completely with/without playlist, restores, and retains advancing playback. Main-player/fullscreen return and normal shutdown checked; broader geometry, reference pixels, ports/audio/HDR remain separate |
| Local playback input follow-up | [Input identity](../tests/visual/windows-playback-input-2026-10-20.json) | Locked build, formatting, workspace tests and strict Clippy passed. Native single/right/double clicks, configured single-click Pause/Resume, fractional and full-notch horizontal/timeline seeks, Hide menu/Ctrl+H and restore checked at 300% DPI. Timeline measurements used a visible controller and Exact seek in a disposable profile. Physical Magic Mouse/driver feel, live macOS/Linux and multi-session Hide remain separate checks |
| Original 4.2.0 portable package (superseded) | [Release identity](../tests/visual/windows-release-4.2.0-2026-10-20.json) | Locked build, formatting, strict Clippy, workspace/native regressions and imports/notices/source checks passed. Engine reused from 4.1.0; exact archive tested locally (5843 ms) and in CI (6211 ms), inspected on the desktop and published unchanged. Scoped GUI results and remaining artwork/focus/hardware/port gaps are in the record |
| Local settings/playback workflows | [Workflow identity](../tests/visual/windows-workflows-2026-10-20.json) | Build/fmt/strict Clippy; 656 workspace tests (56 ignored, one thread), 40 widget regressions, 62 Python and 53 native engine checks. Unicode Save/focus, light handles, playlist Move Down, paused auto Music Mode, first-click seeking, restored History progress and 300 x 259/90 artwork sizing checked in scoped candidates. Final debug smoke passed in 5832 ms; three full-desktop captures inspected. Artwork underlay/clipping, remaining focus/accelerators, matched pixels and other hardware/ports remain open |
| Quick Settings track lists | [Track-list identity](../tests/visual/windows-quick-settings-tracks-2026-10-19.json) | Eight new widget regressions (28 total); build/fmt/strict Clippy, 634 workspace tests with two test threads (56 ignored), 60 Python tests and 5835 ms smoke passed. Native Audio/Video arrows, both subtitle lanes, Tab/reverse Tab and narrow light Audio checked. Full-desktop captures inspected; matched macOS behavior, other DPI and accessibility action bounds remain separate |
| Published 4.1.0 exact package | [Release identity](../tests/visual/windows-release-4.1.0-2026-10-19.json) | Locked build, strict Clippy, workspace/native tests, imports/notices/source checks passed; engine reused from 0.4.0; same archive tested locally (5863 ms) and in CI (6226 ms), then published unchanged. Full-desktop captures inspected; physical display, audible output and HDR remain separate |
| Player proportions and Layout symbols | [Proportion identity](../tests/visual/windows-player-proportions-2026-10-19.json) | Twenty widget regressions; final executable inspected at normal and 285 x 160 sizes, Light/Dark/Auto, standard/compact Layout, horizontal/vertical scrolling and centered Play/Pause. Build/fmt/strict Clippy passed; 626 Rust tests, 56 ignored; 60 Python tests; 5841 ms smoke and three inspected full-desktop captures. Physical pixels/HDR, other DPI and matched macOS states remain separate |
| Windows native menu ownership and Alt return | [Menu input identity](../tests/visual/windows-system-menu-alt-2026-10-19.json) | Baseline still frame-stepped with plain Right after full menu cancellation; corrected executable seeks, retains Alt+Right, opens Audio, traverses F10 menus and resumes after exit. Normal two-level Escape behavior preserved. Build/fmt/strict Clippy; 621 Rust tests, 56 ignored; 60 Python tests; 5766 ms smoke and three inspected valid full-desktop captures. Other input layouts, IME and cross-application cancellation remain open |
| Quick Settings category traversal and narrow layouts | [Navigation identity](../tests/visual/windows-quick-settings-navigation-2026-10-19.json) | Twelve widget regressions; native light/trailing and dark/compact/leading checks, including a 285 x 160 client; direct Tab into numeric editor, reverse-Tab submission, visible focus and scrolling checked |
| Navigation batch build and regressions | Navigation identity | Locked build, formatting and strict Clippy passed; 616 Rust tests passed with two test threads, 56 ignored; 60 Python tests; playback smoke 5918 ms; three valid full-desktop captures inspected and normal exit. Other DPI and matched macOS interaction remain open; menu input has the separate follow-up above |
| Quick Settings numeric input/focus | [Input identity](../tests/visual/windows-quick-settings-input-2026-10-19.json) | Seven generated-widget regressions; native dark Audio/speed and light independent subtitle timing checked; playback/fullscreen smoke 5761 ms; normal shutdown |
| Input batch build/lint/workspace | Input identity | Build, formatting and strict all-target Clippy passed; 611 Rust tests passed, 56 ignored with two test threads; 60 Python tests passed. Initial default parallel run hit two worker timeouts, both passed individually; no timeout code changed |
| Published 0.4.0 exact package | [Release identity](../tests/visual/windows-release-0.4.0-2026-10-19.json) | Source/notices/import checks passed; same original ZIP verified and published; local playback smoke 5779 ms, hosted smoke 6048 ms; test-only software OpenGL in CI; physical pixels/audible output/HDR separate |
| Recovery tooling | `b25ad9d` | 60 Python tests passed; saved-package workflow completed successfully without application or native compilation |
| Menu regression batch | [Menu identity](../tests/visual/windows-menu-parity-2026-10-19.json) | Build, formatting, strict Clippy; 604 workspace tests, 56 ignored; 53 Python tests including launcher failures; 51 native integration and two library checks |
| Live menus and local debug smoke | Menu identity | Default shortcut hints, scale/delay values, F/Ctrl+Alt+F inspected; smoke passed in 5784 ms; full desktop before/during/after inspected, normal exit; this run covered the debug executable; exact archive evidence is above |
| 0.4.0 build, lint and regressions | [Candidate preparation identity](../tests/visual/windows-0.4.0-candidate-2026-10-19.json) | Locked build, formatting and strict Clippy passed; 601 workspace tests, 56 ignored; 34 Python tests |
| Reused release engine | Candidate preparation identity | 51 native integration and two library tests passed against the v0.3.8 runtime, with null audio |
| 0.4.0 local GUI and playback smoke | Candidate preparation identity | Normal/narrow Audio, light Quick Settings, OS double-click fullscreen and host Escape inspected; six-step debug-executable smoke passed in 5858 ms; not the final release archive |
| Full-desktop candidate captures | Candidate preparation identity | Before/fullscreen/after inspected at 3840 x 2160 with non-black probes; scripted capture shows video and controller together; no persistent surface after exit |
| Subtitle, equalizer and light font GUI | [Workflow identity](../tests/visual/windows-subtitle-equalizer-workflows-2026-10-19.json) | Simultaneous SRT lanes, lower styling controls, Classical preset/manual edit, light font filtering/focus/cancellation checked; independent delay keyboard entry not verified |
| Network/font build and GUI | [Network/font identity](../tests/visual/windows-network-font-2026-10-19.json) | Build, formatting, strict Clippy and 600 workspace tests passed; continuous section/search routes and dark font-picker contrast/focus/Escape checked |
| Build, formatting, strict Clippy, workspace/Python checks | [Subtitle and navigation identity](../tests/visual/windows-preferences-subtitles-2026-10-18.json) | Passed; 600 workspace tests, 56 ignored; 31 Python tests and navigation inventory |
| General/Subtitles GUI follow-up | Subtitle and navigation identity | General repeat row and live section jumps, subtitle disclosures, search, provider layouts and independent ASS lanes inspected; saved lane values confirmed after normal exit |
| Build, formatting, strict all-target Clippy and workspace tests | [Preferences row follow-up identity](../tests/visual/windows-preferences-rows-2026-10-18.json) | Passed; 600 workspace tests, 56 ignored; shortcut presentation/raw-binding regressions added |
| Preferences row GUI checks | Preferences row follow-up identity | Utilities, Advanced, Key Bindings, Audio, Network, Plugins and Control inspected; numeric edit persisted after normal exit |
| Locked workspace build, formatting, strict all-target Clippy | [Preferences follow-up identity](../tests/visual/windows-preferences-readability-2026-10-18.json) | Passed with Rust 1.99.0 |
| Workspace tests | Preferences follow-up identity | 598 passed; 56 ignored in the default invocation; includes auxiliary-axis fitting regressions |
| Python packaging/report tests and settings navigation inventory | Preferences follow-up identity | 31 tests passed; navigation check passed |
| Parity traceability | `994f14c` | Passed for the current, incomplete inventory |
| Local debug-application smoke | Preferences follow-up identity | Passed in 5750 ms; same six playback/persistence/surface checks below |
| Preferences GUI sweep | Preferences follow-up identity | Eleven initial viewports, Audio section jumps and Interface sidebar scrolling inspected; corrected contrast/width, with further layout failures recorded |
| Local debug-application smoke | `994f14c` | Passed in 5772 ms; preference writers, native playback/Audio Track menu, volume surface identity, fullscreen callback, controller reveal and exit/progress |
| Local GUI inspection | `994f14c` | Video Quick Settings, app menu, General scrolling, Audio Hardware/Volume inspected; light widget contrast and expanded sidebar clipping fail visual acceptance |
| Full-desktop smoke captures | `994f14c` | Before/fullscreen/after inspected at 3840 x 2160; non-black probes passed; no persistent Riina surface after exit |
| Native libmpv integration | `e6c5d70` | 51 passed against the pinned development DLL with null audio |
| Native library tests | `e6c5d70` | 2 passed against the same DLL |
| Extracted debug-application smoke | `e6c5d70` | Passed in 6034 ms; playback, controls, preferences and normal exit |
| Full-desktop smoke captures | `e6c5d70` | Before/fullscreen/after images inspected at 3840 x 2160; private local evidence |
| Physical fullscreen controller reveal | Baseline `d1ba5ae` | User confirmed visible video with the one-scanline workaround; other drivers/HDR remain open |

The 56 default ignored entries comprise 53 native checks, two clipboard checks
and a child-process helper. Its parent explicitly runs that helper four times.
The candidate preparation separately ran all 53 native checks against the reused
release DLL. It also checked OS-delivered double-click and host Escape; its smoke
still uses the shared fullscreen callback. The newer workflow batch corrects light
equalizer-thumb contrast; matched IINA geometry, material-policy transitions and wider hardware coverage
remain acceptance work. Each release archive requires its own evidence.
The older Preferences runs used a local executable rather than an extracted
release package. Their scripted fullscreen entry invokes the double-click
callback; those reports do not establish OS-delivered double-click input. The
earlier extracted archive was a development package. The published 0.4.0 archive
has separate local and remote verification in its release record above.

The [initial GUI record](../tests/visual/windows-ledger-review-2026-10-18.json) is
dated evidence of the failures before the Preferences fixes. The follow-up record
identifies exact modified source files, executable/library hashes, scope and
remaining failures; its source hashes avoid referring to a future commit.
The newer row follow-up records the additional fixes and its narrower GUI scope;
earlier failures remain dated evidence rather than current claims.
The subtitle/navigation record covers two settings builds and identifies which
checks were repeated after the General-only follow-up. The Network/font follow-up
corrects those two findings. The later workflow record checks the light font
chooser; material-policy transitions still need verification.
Null audio and captured pixels do not establish audible output, physical display
behavior or HDR. Both test processes exited; no Riina process remained.

Local logs: `target/smarter-tests-*`, `target/review-fixes-*`,
`target/ledger-cleanup-*`, `target/preferences-sizing-*` and `target/preferences-rows-*`.
Raw desktop images remain untracked and must be reviewed for private content
before any publication. Older investigations remain in Git history.

The visual-ledger integrity checks include 15 Python regressions. The current
Python suite contains 60 tests across ledger, packaging and report validation.
They reject dropped or changed original reference identities, broken evidence
links, changed local capture bytes, malformed records and unsupported acceptance
claims. All 69 records and 68 available capture hashes passed locally. These
record checks do not establish matched geometry or pixel parity.

## Linux virtual display 2026-11-21

The x64 application at `f9171e4` passed strict workspace Clippy and 793
workspace tests on Ubuntu 26.04 under WSL. The application linked against
the staged GTK and WebKitGTK libraries. The runtime check used Ubuntu's
libmpv 0.41.0 beside the executable, a private Xvfb display and Mesa
llvmpipe. It opened two isolated copies of the moving-square fixture in
successive sessions. In the first session, a 7,099-pixel video region stayed
identical in both paused comparisons and changed by 187 pixels between two
playing captures. The second session displayed the second fixture. Both quit
through the app shortcut, logged the native worker join and exited.

An archive of `135d0d1` with the system-library search and appearance
adapter changes passed strict Clippy, 794 workspace tests and a locked full
application build in the same Ubuntu environment. A follow-up put libmpv in
the test namespace's `/usr/lib/x86_64-linux-gnu`, leaving the application
directory without a bundled copy. Both process maps confirmed the installed
path. The same 7,099-pixel region changed by 236 pixels during playback and
stayed identical in both paused comparisons. The second fixture rendered and
both test processes quit and joined their native workers.

An archive of `c74bc0b` with the shared framebuffer, dialog dispatch and
platform session changes passed strict Clippy, 796 workspace tests and a
locked application build. The resulting player loaded the installed libmpv
in the same private display. The video sample changed by 373 pixels during
playback and stayed identical during both paused comparisons. Both sessions
again rendered their own fixtures and exited after joining the native worker.

An archive with the PiP platform-session changes from `628186d` passed strict
Clippy, 799 workspace tests and a locked x64 application build. The private
display confirmed the installed libmpv path and worker shutdown. Two runs
with the default logging settings showed a black video region in the second
player even after resuming. The earlier platform-boundary executable also
reproduced black video with logging disabled. With native debug logging enabled,
the PiP candidate rendered in all three sessions, including a relaunch with
the same profile and a launch with a fresh profile. The 7,099-pixel sample
changed by 187, 424 and 407 pixels during playback, respectively, and both
paused comparisons in the first session stayed identical. The cause of the
intermittent rendering failure remains open.

The controller and Layout-sidebar candidate at `3924dc9` passed strict
Clippy, 799 workspace tests and locked application builds on Linux. It also
passed the locked Windows workspace build. A Linux diagnostic using the same
private display, with hardware decoding and debug logging disabled, still
showed black video on its first launch. A relaunch with that profile and a
launch with a fresh profile each changed 224 of 7,099 sampled pixels during
playback. All three sessions joined their native workers and exited. Software
decoding alone did not resolve the rendering failure.

The Settings and session candidate at `2135aa7` passed strict Clippy, 800
workspace tests and a locked x64 application build in the same Linux environment.
With native debug logging enabled, the first session showed black video while
the playback clock advanced. The relaunch and fresh-profile sessions each changed
399 of 7,099 sampled pixels during playback. All three loaded the installed libmpv
and joined their workers on exit. The first session logged that mpv had shown a
frame, although the full virtual display capture still showed black video.

A temporary readback probe based on `fe9115f` sampled the framebuffer directly
after successful mpv render calls. All three sampled pixels stayed opaque black
through frame 128 on the first launch. The relaunch and fresh-profile sessions
returned the fixture's blue background and moving square in the same samples.
The black output was already present before Slint sampled the video texture.
The probe also recorded occasional mpv render timing warnings in both the
failing and working sessions. All three sessions quit and joined their workers.

The startup-stage candidate at `7a81835` passed strict Clippy, 802 workspace
tests and a locked x64 application build. In its private display check, video
remained black during playback in the first two sessions. The fresh-profile
session changed 429 of 7,099 sampled pixels during playback. All three opened
their own fixture, loaded the installed libmpv and joined their workers on exit.

Resetting the GL texture unit and blend state around the mpv call still left
one of three test sessions black. Disabling thumbnail generation also left
one of three sessions black. An explicit zero timing offset rendered all six
sessions across two paused-start runs. A build with that default rendered all
three paused-start sessions, but with ordinary unpaused startup it showed a
black fresh-profile session and a black first capture in the initial session.
The timing changes were evaluated in isolated builds. The default timing in
the application is unchanged.

A separate profile that explicitly selected null audio advanced the playback
clock with a black video surface in the virtual display. That run also exposed
a desktop GLSL version directive in the optional blur shader. The updated
shader linked in both Mesa OpenGL 4.5 and OpenGL ES 3.2 software contexts.
Linux package, visual blur, real display and audio endpoint verification remain
open.

In an isolated unpaused run of `da50ecd`, the first 1280×800 virtual desktop
capture showed the player controls and a black video area at 0.375 s. An mpv
video-only screenshot at that point contained the fixture's moving square.
The desktop showed video at 1.375 s. Relaunch and fresh-profile captures also
showed motion. The fixture filename and playing state were checked through
mpv for each capture. All three players quit and joined their workers. This
narrows the intermittent failure to presentation after decode.

Removing the optional mpv swap notification passed an initial three-session
probe. A paired run covered two ordinary starts and two paused starts for each
build. Each build still showed black video in two late captures while the
decoded screenshots contained the fixture. All eight isolated sessions exited.
The swap notification remains in the shared renderer.

A standalone libmpv client reproduced the symptom in the private Xvfb display
using a GLX context, Mesa llvmpipe and the moving-square fixture. Ten starts
sampled the client-owned 320 × 192 framebuffer between 0.15 and 1.55 seconds.
Two starts stayed black at all four samples. libmpv reported 38 and 36 frames
ready, respectively. Each accompanying video-only screenshot had 57,600
non-black pixels in its 320 × 180 image. This reproduces the rendering failure
with Riina and Slint absent from the process.

A paired follow-up used fresh profiles and caches for twelve starts. All six
default starts and four of six starts with decoder direct rendering disabled
showed a static black or nearly black framebuffer. The decoder setting alone
therefore leaves the failure reproducible.

A separate twelve-start comparison used clean profiles and alternated between
fresh and shared cache directories. Two of six fresh-cache starts stayed
black through the last sample. All six shared-cache starts rendered motion.
The result leaves the role of the cache unresolved.

Two further standalone comparisons each alternated six starts per condition
using libmpv 0.41.0. The cache comparison found a black framebuffer in two
fresh-cache starts, zero starts with only Mesa's shader cache warmed, two
starts with only mpv's cache warmed, and five starts with llvmpipe's worker
threads disabled. The Mesa cache result is limited to this sample.

With fresh caches in every GLX context, the bound llvmpipe framebuffer was
black in three starts and nearly black in one. The default window
framebuffer was black in two starts, and the client framebuffer left
unbound on entry was black in five. All six standalone softpipe starts
rendered motion. The fully black cases also returned a decoded 320 × 180
video-only screenshot with 57,600 non-black pixels. The failure can occur
with either framebuffer target and both client-framebuffer binding states.

An isolated full-app comparison used the locked Linux build from `d8421ed`,
the same libmpv version, and a new profile and Xvfb display per start. Three
llvmpipe starts showed the fixture in both desktop captures. Three softpipe
starts showed a black video area at the first capture, taken at playback
positions between 0.25 and 0.42 seconds. One of those showed the fixture
at the second capture near 1.54 seconds. All six sessions quit normally.
An additional llvmpipe session showed a black video area through 1.5 seconds.
Although 1,440 sampled pixels changed between its captures, both full
1280 × 800 images had zero pixels in the blue range of the fixture's
background. Control changes alone can satisfy a motion check. The app
comparison checked the active fixture and playing state before each capture,
verified the full desktop dimensions, and checked for pixels from the fixture.
Linux startup presentation remains intermittent on this setup.

The earlier standalone probes read playback position synchronously on the
render thread. A follow-up used an asynchronous load request and polled
position from libmpv property events. It alternated eight cold starts per
condition with libmpv 0.41.0 and Mesa 26.0.8 llvmpipe. Three desktop GL
starts, five desktop GL starts with advanced render control, and three
OpenGL ES 3.2 starts stayed black or nearly black at all four samples.
All eight starts that converted video to packed RGB24 before upload rendered
changing frames. The basic desktop and ES cases with dark output also
returned software screenshots with all 57,600 video pixels non-black.
Advanced render control can route screenshots through the same OpenGL
renderer, so the dark screenshots in that condition leave decoded pixels
unverified. [libmpv render threading contract](https://github.com/mpv-player/mpv/blob/v0.41.0/include/mpv/render.h),
[libmpv screenshot dispatch](https://github.com/mpv-player/mpv/blob/v0.41.0/video/out/vo_libmpv.c),
[software screenshot fallback](https://github.com/mpv-player/mpv/blob/v0.41.0/player/screenshot.c).

The locked app at `4f7ee4e` also compared three ordinary
`yuv420p` starts with three RGB24 starts using isolated Linux profiles.
The player reported the requested format and active fixture at each
capture. One ordinary start had zero fixture-blue pixels in the full
1280 × 800 desktop at 0.417 seconds and recovered by 1.5 seconds.
The first capture of each RGB24 start showed the fixture, at playback
positions of 0.375, 0.458 and 0.500 seconds. All six sessions exited through
Quit. The conversion was a diagnostic setting in the test profiles.

A separate standalone run alternated six cold starts for each of five
video formats. Two default `yuv420p` starts stayed dark through 1.5 seconds
and returned software screenshots with 57,600 non-black pixels. All 24
starts using `nv12`, `yuv444p`, `gbrp` or `rgb24` showed changing
framebuffer contents at the sampled positions. NV12 shares the 4:2:0
chroma subsampling with the default format, while YUV444P and GBRP also
use three planes. Converting the input changes the upload data and
processing, so the exact failing OpenGL operation remains unresolved.

A fixed-format comparison kept `yuv420p` for 32 cold GLX starts.
Four of eight default starts, three of eight with FFmpeg's copy filter,
three of eight with pixel-buffer uploads, and four of eight with the full
GPU shader path stayed black at all four sample positions. Every black
start produced a software screenshot with 57,600 non-black video pixels.
In all direct-upload cases, nine samples from each of the three OpenGL
plane textures matched the bytes submitted by the client. The buffer-upload
cases also had nine nonzero samples per plane. The sampled plane contents
therefore survived upload even when the final framebuffer was black.

A second comparison recorded intermediate draw outputs only after the final
unmodified framebuffer sample. Four of eight default starts, two of eight
full-shader starts and six of eight starts that finished GL work after each
plane upload stayed black or nearly black. In all twelve dark starts, the
draw that merged the two chroma planes returned nonzero color samples. The
vertical or horizontal chroma upscaling draw then returned zeros, and the
final video draw was black at the sampled positions. This locates the first
sampled failure before conversion to RGB and presentation to the window.
[libmpv chroma merging and upscaling](https://github.com/mpv-player/mpv/blob/v0.41.0/video/out/gpu/video.c#L2033-L2204).

With `yuv420p` fixed, all eight standalone starts using `cscale=bilinear`
showed moving video. Four of eight starts with inherited chroma scaling,
five of eight with explicit Lanczos scaling and three of eight with
`GALLIVM_PERF=nopt` stayed black or nearly black. In this libmpv build the
inherited scaler is Lanczos. Bilinear uses regular texture sampling and
skips the explicit chroma upscaling draws, providing a diagnostic bypass
on this setup.
[libmpv scaler defaults](https://github.com/mpv-player/mpv/blob/v0.41.0/video/out/gpu/video.c#L337-L357),
[Mesa shader optimization flags](https://gitlab.freedesktop.org/mesa/mesa/-/blob/mesa-26.0.8/src/gallium/auxiliary/gallivm/lp_bld_init_common.c).

The locked Linux app at `4f7ee4e` alternated five default starts with five
bilinear starts on private X11 displays. Each capture verified the active
fixture, playing state, `yuv420p` output and scaler setting. Two default
starts had zero blue fixture pixels in their video region at both
captures, through 1.5 seconds. All five bilinear starts showed the fixture
in both captures, with movement between them. The first bilinear captures
were at 0.667–0.708 seconds. The comparison used full 1280 × 800 desktop
captures and temporary profiles. All ten sessions exited through Quit.

The upstream defect was uninitialized padding in libmpv's scaler lookup
texture. The six Lanczos weights occupy eight float slots per row.
A standalone comparison intercepted only that texture's initial upload.
The six weight values in every row had the same byte signature across all
18 runs. Six of six starts with zeroed padding showed moving video. Six
of six starts with NaN padding stayed black, as did one of six starts with
ordinary allocation. The upstream fix initializes the padding from the
corresponding channels in the first texel and is included in `v0.42.0`.
[Upstream fix](https://github.com/mpv-player/mpv/commit/72d43dc9c999a21d867cdc0f934f3e4cd2195aa9),
[v0.42.0 release source](https://github.com/mpv-player/mpv/blob/v0.42.0/video/out/gpu/video.c#L1750-L1766).

With the upstream fix backported to libmpv `v0.41.0`, all six standalone
starts on Ubuntu 26.04 x64 in WSL showed moving video at the four samples
from 0.15 through 1.5 seconds. Two of six starts with the distribution
library stayed black. The backport initialized the padding in every run
and produced the same six filter weights as the distribution build.

The locked Riina build at `4f7ee4e` loaded the staged libraries through
its default startup route with the inherited scaler. With the backport,
all six app sessions showed black at the first capture at 0.417 seconds
and displayed the fixture at 1.5 seconds. With the distribution library,
one of six sessions stayed black at both captures. Its first captures were
at 0.583–0.708 seconds. Every app session exited through Quit. The backport
library SHA-256 was
`cdce854e1811942602587310026306d92d30c0f593f2be676f07659ae43061d8`.

With playback positions matched at 0.75 and 1.5 seconds, all four backport
sessions showed the moving fixture in both full 1280 × 800 captures. One of
four distribution-library sessions stayed black at both positions. All eight
sessions exited through Quit, and the private displays stopped. This verifies
the fix for sustained black output on the tested Linux setup and records the
earlier startup blanking separately.

Two Openbox sessions on the same private X11 setup used the locked x64
application at `4f7ee4e` and the backported library above. Keyboard input
paused and resumed playback, sought backward while paused, entered
fullscreen, restored the normal window with Escape, and stopped playback.
An X11 resize request produced a 720 × 405 client and the video filled the
new playback area. The fullscreen and resize checks matched fixture pixels
across the expanded region after the window manager changed the geometry.
Reopening through libmpv IPC restored visible playback. Both sessions
exited through Quit.

A separate private X11 session used the GTK file chooser through the XDG
desktop portal. Ctrl+O opened a dialog with the player as its transient
parent. An EWMH close request left the player active while the dialog was
open. Escape dismissed the chooser and retained the file. After stopping,
selecting that file through the reopened chooser restored visible playback.
The session exited through Quit. The application SHA-256 for these checks was
`f0bd38db347be624413ca42ef72a813b9d033f4edf8baa77d4637f00c5947d7d`.
These checks used null audio and a single 1280 × 800 virtual display.

A follow-up X11 session at `63dfb72` used the same backported library.
The GTK portal chooser loaded and selected an external WAV track and an
external SRT track. Add to Playlist appended a video while the current
file kept playing. Open in New Window opened a second player in the
same process. A window-manager close request closed only the second
player. The first player continued to show advancing video and exited
through Quit.

The same `63dfb72` executable ran on a private 1280 × 800 Sway 1.11
Wayland display with XWayland disabled. A private mount namespace hid
the WSL markers that otherwise select X11 in Slint. Software GL showed
advancing frames in the native Wayland window. Keyboard input paused,
resumed, sought backward, entered and left fullscreen, and quit the
player. A compositor resize produced a 720 × 405 window. Stopping and
reopening through libmpv IPC restored visible playback at that size.
The short-lived virtual keyboard did not deliver the Control modifier.

A follow-up with the same executable held a virtual keyboard and its
modifier state for the entire session. Ctrl+O opened the native GTK file
chooser. The protocol trace matched the parent exported by Riina to the
parent imported by GTK. Escape cancelled the chooser and retained the
playing file. Ctrl+Period unloaded that file. Reopening the chooser and
selecting a second fixture restored visible playback. Ctrl+Q exited
normally. Both sessions used null audio. Wayland plugin embedding and
physical displays remain unverified.

The locked build at `96aa8c3` passed playback, pause, backward seek,
fullscreen, resize, the parented file chooser, Stop and Quit in one
Wayland session. The selected file played in the 720 × 405 window.
The player and private compositor exited normally.

## Windows private-desktop smoke 2026-11-21

The locked Windows x64 development build at `da50ecd` passed the ten native
playback and lifecycle checks in the package smoke driver. The test staged the
executable beside the pinned development DLL and used an isolated profile with
null audio. It exercised native menus, playback progress, fullscreen, controller
reveal, minimize and restore, and settings persisted through normal shutdown.
The test process exited, and the active Windows desktop stayed unchanged.
This run used a temporary Win32 desktop. Physical display pixels, HDR output,
and the release archive still need separate verification.

The locked build at `7000a73` also passed all ten checks in 9.056 s using
the reusable `-PrivateDesktop` driver. Minimize and restore exercised the native
owner's property-change events. The executable and report paths contained
spaces, the input desktop stayed unchanged, and the Riina process exited.

The locked Windows x64 builds at `ee3ce7a` and `8bdb6e3` also passed all ten
checks after extracting host events and then assigning ownership to `NativeApp`.
The separate-desktop runs took 9.149 s and 9.331 s, respectively, and each player
exited. `ee3ce7a` also passed the 53 pinned-libmpv playback tests with null audio.

`d6c0916` and `9929e0a` passed the same ten native-window checks. The locked
`9929e0a` build also started with `--shared-gl` on an isolated Windows desktop.
The fixture's playback position advanced from 0.250 s to 0.583 s, and mpv
saved a 320×180 decoded frame through its test-local IPC pipe. Closing that
exact window exited normally and the engine worker joined. The input desktop
stayed unchanged. The decoded frame is evidence for playback, while physical
presentation remains open.

The shared-video adapter at `9929e0a` passed strict Clippy targeting
`aarch64-apple-darwin` in the isolated source probe. It used the actual renderer,
AVKit Picture-in-Picture code and libmpv crate, with stubs for Slint and native
display-profile access. A full macOS build, presentation and lifecycle check
remain open.

The macOS Now Playing projection from `4684478` and the input and document
adapters from `7226679` also passed strict `aarch64-apple-darwin` source probes.
Those checks used the real AppKit scroll monitor, trackpad dispatch and
represented-document code with stubbed Slint and Winit window access.
Native linking and interactive input on macOS remain open.

## Windows hardening integration 2026-11-23

An exact `47c8f73` checkout passed `cargo xtask check` with 849 Rust tests,
78 Python tests, and all 51 software-rendered reference comparisons. The
separate `cargo xtask native` run passed 53 tests against the pinned development
DLL with null audio output. `cargo build --workspace --locked` completed, and
the resulting executable passed all ten private-desktop playback and window
checks in 9.263 seconds. The test process exited normally.

The follow-up application batch through `1dde0ee` passed strict Clippy
and 548 application and widget tests through `cargo xtask check --fast`.
The updated `cargo xtask native lifecycle::` reused the workspace test
binary and passed all four lifecycle cases.

At `a83c518`, the Windows workspace gate passed 849 Rust tests and 78 Python
tests. The 58 preference change tests covered runtime state saved on close,
concurrent preference writers and the mixed-edit volume rollback sequence.
The locked workspace build passed all ten private-desktop checks in 9.303
seconds. The test process exited normally.

At `845f8c0`, `cargo xtask check --fast` ran strict app linting and passed
549 application and widget tests. The new callback test verified that a
discarded callback releases its queue slot and allows the next delivery.

At `17bd471`, the Windows workspace gate passed 853 Rust tests and 78 Python
tests, including the 51 software-rendered reference comparisons. The headless
timer tests verified that thumbnails wake for relevant changes and that native
transition capture waits for its 80 ms tick and stops after the reply. All 53
pinned native tests passed with null audio output. The locked workspace build
passed all ten private-desktop checks in 9.245 seconds and exited normally.

At `5aff11a`, `cargo xtask check --fast` passed strict app linting and 552
application and widget tests. The thumbnail cache test verified that a cache
hit refreshes the manifest's modification time and preserves its bytes.

At `9871d17`, `cargo xtask check` passed 857 Rust tests and 78 Python tests,
including the 51 software-rendered reference comparisons. The headless tests
checked when online subtitle lookup, subtitle discovery, playlist metadata,
Live Text and Windows video material sampling start and stop. All 53 pinned
native tests passed with null audio output. The locked workspace build passed
all ten private-desktop checks in 9.227 seconds and exited normally.

At `e887960`, `cargo xtask check` passed 858 Rust tests and 78 Python tests.
The new headless PiP case verified that rejecting the UI task releases the
pending entry state. The 53 pinned native tests passed with null audio output.

At `0887a00`, the Windows workspace gate passed 859 Rust tests and 78 Python
tests. A headless PiP regression checked the timer at player initialization,
when automatic minimize handling is enabled and disabled, during PiP display,
and after shutdown. The 51 software-rendered reference comparisons passed.
The locked build passed all ten private-desktop checks in 9.149 seconds,
then the test player exited normally.

At `2df984f`, `cargo xtask check --fast` passed strict app Clippy and all
558 app and widget tests after the native-window and capture cleanup audit.

At `223656d`, the Windows workspace gate passed 860 Rust tests and 78 Python
tests. The native visibility regression covered show, hide, minimize, restore
and teardown, using a controlled Slint clock for alignment ticks. The locked
build passed all ten private-desktop checks in 9.156 seconds and exited normally.

At `63c10a6`, the Windows workspace gate passed strict Clippy, 860 Rust tests,
78 Python tests, and the headless player and Settings pixel comparisons.
This grouped check covers the track language and mouse wheel adapter moves.

At `84fbd90`, `cargo xtask check` passed 861 Rust tests, 78 Python tests,
and the headless pixel comparisons on Windows. The snapshot wait test
verified that a disconnected worker fails immediately. The pinned DLL passed
all 55 `cargo xtask native` tests with null audio in 35.71 seconds. This
batch includes the libavformat field-order and AVIF filename checks that
previously ran separately.

The saved-frame checker passed all 36 explicit libavformat captures. The
default-demuxer comparison reproduced the six known field-pattern mismatches
for the original v210 BFF fixture with deinterlacing enabled. The other 66
default-demuxer frames passed. Separate checkers passed all six native image
formats and confirmed that the hidden-subtitle capture exactly matched the
video-only capture. These tests exercised engine output without launching
Riina's application window.

At `a6e9da3`, the Windows workspace gate passed 861 Rust tests and 78
Python tests, including the headless player and Settings pixel comparisons.
The Win32 visibility test verified that the two-second HDR display timer
pauses when the owner is hidden or minimized, resumes on restore and stops
at teardown. An earlier build of the same timer change, before moving the
shared display update into the platform module, passed the locked workspace
build, all 55 pinned DLL checks and all ten private-desktop smoke checks
in 9.336 seconds. The test process exited normally. Physical HDR output was
outside this run's scope.

At `4f7ee4e`, the Windows workspace gate passed 861 Rust tests and 78
Python tests, including the headless pixel comparisons. Startup tests covered
both explicit renderer switches and file order. The locked workspace build
used the default native launch route for all ten private-desktop smoke checks
in 9.230 seconds. The test process exited normally.

The fast check compared the working tree with the commit before three local
commits and selected the app and test runner. Strict Clippy and 559 app
and widget tests passed. At `4555ff8`, the full workspace gate passed
861 Rust tests and 78 Python tests, including the headless pixel comparisons.
All 55 pinned native engine tests passed with null audio output.

At `47f4380`, the Windows workspace gate passed 862 Rust tests and 78 Python
tests. The new startup regression verifies the name of a required option and
the original native error. All 55 pinned native engine tests passed.

At `caf37b5`, `cargo xtask check` passed 862 Rust tests and 78 Python tests,
including the headless pixel comparisons. `cargo build --workspace --locked`
passed. The Windows private-desktop smoke passed all ten checks in 9.124
seconds and exited normally. The check covered native controls, menu state,
fullscreen entry and exit, minimize and restore after the last function splits.

At `96aa8c3`, `cargo xtask check` passed 862 Rust tests and 78 Python tests,
including the sizing, transition-cover, HDR command and headless pixel
regressions. The locked Windows workspace build passed. The private-desktop
smoke passed all ten checks in 9.147 seconds and exited normally.
`cargo xtask native` passed all 55 cases with the pinned DLL.

At `10d824d`, `cargo xtask check` passed 865 Rust tests and 78 Python tests.
An untracked Slint probe exercised the UI-only `cargo xtask check --fast`
route. It ran the 48 player interaction tests and the Windows pixel suite in
4.61 seconds with the workspace build cache primed. The probe measured
check selection and cache reuse.

At `55fe191`, the Windows workspace gate passed 867 Rust tests and 78 Python
tests. The plugin regressions verified the idle presenter interval, a wake
for queued messages and late activation with the original lifecycle events.
The locked Windows workspace build passed, and the private-desktop smoke
passed all ten checks in 9.154 seconds before the player exited normally.

At `e84cf67`, the Windows workspace gate passed 868 Rust tests and 78 Python
tests. A plugin regression reproduced the loss of the loaded flag and window
frame after a playback update. It passed after the host preserved window
observations across playback snapshots, both at plugin activation and before
the closing callback.

At `7b0ce17`, the Windows workspace gate passed 868 Rust tests and 78 Python
tests. The locked application build passed all ten private-desktop smoke
checks and exited normally in 9.327 seconds.
At `a6c1062`, the Windows workspace gate again passed 868 Rust tests and
78 Python tests.

At `0dc5222`, `cargo xtask check` passed 868 Rust tests and 78 Python tests
on Windows.

On `954d4d6`, separate private-desktop runs reopened and played media after
recovering a profile with an invalid setting type, a non-object JSON document,
or invalid UTF-8. Each run showed the recovery notice, preserved the original
bytes in quarantine, and exited normally. A separate startup with an invalid
DLL recorded the failure, showed the native startup dialog, and exited with
code 1. These runs used the isolated desktop and disposable profiles.

### App-only compilation time

On the Windows development machine, `cargo check -p riina --locked` took
61.70 and 11.33 seconds at `9c7f48c` and 1.95 and 1.83 seconds at `a83c518`
after refreshing the modification time of `seek_osd.rs`. Each checkout used
its own warmed build directory, four build jobs, and the pinned toolchain.
The source bytes were identical before and after each trial. The prep check
took 14.06 seconds for the baseline and 69.07 seconds for the current tree.
These timings measure an app-file edit with built dependencies. Background
builds during the measurements and the baseline's spread limit precision.

The earlier profile comparison used the same `9c7f48c` source for both
configurations and independent empty build directories. A locked application
build took 358.68 seconds and used 7.41 GiB with Cargo defaults. The line-table
profile took 565.16 seconds and used 3.92 GiB. After priming both caches,
repeated app-file checks took 38.89 and 14.99 seconds with Cargo defaults, and
14.67 and 15.91 seconds with the line-table profile. This comparison isolates
the profile change from the later separation of application and UI compilation.

## Linux x64 application checks

### Application-wide Hide Riina 2026-11-23

On Ubuntu 26.04 in WSL, the isolated X11 baseline at `a393255` minimized only
the invoking player. The new build minimized two independent players and the
Settings window in the same process. The window manager restored all three
with their original window IDs. The fixture stayed loaded and its playback
position advanced after restore. Quit ended the process normally.

The same executable issued one `xdg_toplevel.set_minimized` request per
visible toplevel on isolated native Wayland, with a player and Settings open.
Sway left both toplevels visible, the playback clock advanced, and Quit ended
the process normally. This verifies request delivery on Wayland. Desktop
environments that honor the request still need visible minimize/restore checks.

The executable SHA-256 was
`63347838ded3951968891999a3e6e47d967303b07aca3e123a52d1bc1e3b7e5e`.
Both runs used the backported libmpv runtime with SHA-256
`cdce854e1811942602587310026306d92d30c0f593f2be676f07659ae43061d8` and null audio.

Strict workspace Clippy and all 830 workspace tests passed on Linux, including
the window registration test for duplicate, hidden and released components.
`cargo xtask check` passed 868 workspace tests and 78 Python tests on Windows.

### Workspace and application builds

At `0dc5222`, strict workspace Clippy, 829 Rust tests and the locked x86-64
application build passed on Ubuntu 26.04 in WSL. The X11 plugin shortcut
check used that executable.

At `7b0ce17`, strict workspace Clippy, 829 Rust tests and the locked x86-64
application build passed on Ubuntu 26.04 in WSL. The executable SHA-256 was
`e8c701a7da28926e982cd75d07ef22baffb7cd7280c11234fd779c814238c6b9`.

At `55fe191`, strict workspace Clippy, 828 Rust tests and the locked x86-64
application build passed on Ubuntu 26.04 in WSL. The executable SHA-256 was
`eff4811dd8c88ee24bfe33e827bb7cc47ac9368d4bb8d85d723ea3f53822b0a2`.

At `96aa8c3`, strict workspace Clippy, 823 Rust tests and the locked x86-64
application build passed on Ubuntu 26.04 in WSL. The executable SHA-256 was
`73d818d1d2f4e0189fa3417ecbd06ad370f2f589ceb2862150aa839deda5c933`.

At `63dfb72`, strict workspace Clippy, 823 Rust tests and a locked x86-64
application build passed on Ubuntu 26.04 in WSL. The executable SHA-256 was
`c5f5c1cb64a2009c5fde4dc1526ca3d8359181817b9a02ca1d18a28c24279f70`.

At `17bd471`, strict workspace Clippy, 818 workspace tests, and the locked
x86-64 application build passed on Ubuntu 26.04 in WSL. This includes the
thumbnail and transition timer tests.

At `5aff11a`, all three thumbnail cache tests passed in WSL, including the
manifest timestamp and byte-preservation checks.

At `9871d17`, Ubuntu 26.04 in WSL passed strict workspace Clippy, 821
workspace tests and the locked x86-64 application build. The timer tests
exercised the portable subtitle, playlist metadata and Live Text scheduling
paths.

At `e887960`, strict workspace Clippy, 821 workspace tests and the locked
x86-64 application build also passed in WSL. The PiP rejection test ran on
Windows because it exercises that platform's window controller.

For `0887a00`, the Linux workspace passed strict Clippy, 821 tests, and the
locked x86-64 build. The source copy used an earlier version of the Windows-only
PiP timer test. Linux compiles the shared Slint bindings and excludes that test.

At `2df984f`, the focused Linux app check passed strict app Clippy and 473
library tests. The earlier workspace check covered the shared widget suite.

At `d8421ed`, the Linux workspace passed strict Clippy, 821 tests and the
locked x86-64 application build.

At `63c10a6`, Ubuntu 26.04 in WSL passed strict workspace Clippy, 821
workspace tests and the locked x86-64 application build with the track
language and mouse wheel adapters.

At `4f7ee4e`, strict application Clippy, all 522 application and widget
tests, and the locked x86-64 application build passed in WSL. An isolated
launch with `--shared-gl` opened the moving-square fixture. Playback was
active in both full 1280 × 800 captures. The first, at 0.417 seconds, showed
black video while the controls were visible. The second, at 1.5 seconds,
showed the fixture. Riina exited through Quit and the virtual display
stopped. First-frame presentation remains intermittent.

At `a83c518`, Ubuntu 26.04 in WSL passed strict workspace Clippy, 814
workspace tests, and the locked x86-64 application build.

The locked x86-64 build at `845f8c0` passed compilation. In a private Xvfb
session it showed black video at 0.375 seconds and visible video at 2.042
seconds. The active fixture and playback state were verified at capture.
Riina exited through Quit and the virtual display stopped.

An exact `47c8f73` source archive passed strict workspace Clippy, 814
workspace tests, and a locked x86-64 Linux application build on November 23
under Ubuntu 26.04 in WSL. An isolated Xvfb run opened the moving-square
fixture with playback active. The desktop capture at 0.375 seconds showed a
black video region, and the capture at 2 seconds showed the decoded fixture.
Riina exited through Quit, and the virtual display shut down. Later comparisons
identified the [upstream scaler defect](#linux-virtual-display-2026-11-21).

On November 21, an exact `da50ecd` checkout in WSL completed strict Clippy,
803 workspace tests, and a locked x86-64 Linux application build. The executable
is a 64-bit ELF and its dynamic dependencies resolved in the disposable test setup.
This cohort verifies the Settings refactor through compilation and tests.

An exact `9929e0a` checkout also completed strict Clippy, 807 workspace tests,
and a locked application build. Its x86-64 ELF had no unresolved dynamic
dependencies in the test setup. Three isolated Xvfb sessions used the same
video fixture with an initial profile, a relaunch, and a fresh profile. Playback
reported `pause=false`, and early and late 1280×800 desktop captures showed
changes in the fixture's video region in all three sessions. Each Riina process
and virtual display exited. The previously reproduced intermittent black-video
failure remains open.

The native session adapter commit `7226679` passed Linux x64 strict Clippy,
808 workspace tests and a locked application build. Three isolated sessions
opened the fixture and exited through Quit. The first session captured a black
video region at 0.083 s and 1.292 s while mpv returned decoded video. The relaunch
and fresh-profile sessions showed motion in 197 and 242 of 7,099 video-region
pixels, respectively. Riina and Xvfb exited after the cohort.

## Running checks

### Portable and workspace checks

```powershell
cargo fmt --all -- --check
python scripts/check-line-length.py
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --locked
python -m unittest discover -s scripts/tests -p 'test_*.py'
python scripts/check-parity.py
python scripts/check-fixtures.py
python scripts/check-assets.py
```

Use [the development wrapper](development.md#windows-setup) when the toolchain
is checkout-local. Run the asset check with `--staged` after staging asset changes
and before publishing. Launch long checks asynchronously, continue independent
work and record the revision covered. Do not start duplicate runs for status.

The current workspace runs `plugin_panel_opens_on_the_saved_side` as a pure
sidebar routing test. The test at the immutable `v0.3.6` commit constructs a
renderer-backed `PlayerWindow`, which requires a graphical session. During that
historical candidate's test step, the workflow marks the test ignored with this
reason and restores the exact source bytes before compiling the package.

### Native playback

```powershell
$env:RIINA_TEST_MPV_LIBRARY = (Resolve-Path tools/mpv/libmpv-2.dll).Path
$env:RIINA_TEST_MPV_AUDIO_OUTPUT = 'null'
cargo test -p riina-mpv --test native --locked -- --ignored --test-threads=1
cargo test -p riina-mpv --lib --locked -- --ignored --test-threads=1
```

These tests exercise real decoding and commands. Null output does not prove
visible presentation, audible output, pass-through audio or HDR luminance.
See [port development](port-development.md) for other-platform library paths.

### Extracted application and desktop checks

```powershell
./scripts/smoke-windows-package.ps1 `
  -Executable C:\package\riina.exe `
  -Fixture tests/fixtures/moving-square.avi `
  -ReportPath C:\evidence\fresh-smoke.json `
  -CaptureDesktop
```

Use a new absolute report path. The runner creates a disposable profile and
isolated media folder, launches the exact executable, enforces a deadline,
validates its report and waits for normal exit. It checks persisted settings
after shutdown and cleans up only its own process on failure.

For local lifecycle checks, use `-PrivateDesktop` in place of
`-CaptureDesktop`. The driver runs the native player on a separate Win32
desktop while the active desktop stays unchanged. A Job Object bounds the
driver and its child processes. The usual JSON report and driver log remain
available for inspection.

The application driver checks native menu availability, volume geometry,
fullscreen/controller reveal, surface alignment and restoration. It invokes the
same deferred action as double-click fullscreen; it does not inject a physical
OS double-click. Captures and geometry cannot certify physical-display pixels.

For manual testing, follow [the GUI session procedure](development.md#gui-sessions)
and [the regression sweep](implementation-plan.md#regression-gates). Verify the
actual display, sound and HDR separately. Close the test instance and confirm
no stale Riina process remains before rebuilding.

## Regression test design

| Area | What the tests check |
|---|---|
| Preference ownership | 288 deterministic stale-writer saves against an explicit typed-field oracle; nested siblings, no-op saves and resets |
| Process locking | Four gated processes perform 48 contended updates with a 15-second parent deadline; also passed eight consecutive repetitions |
| Failed saves | Cancelled/invalid changes preserve bytes and writer state; a Windows replacement failure can retry the identical candidate |
| Optional/collection fields | Theme removal and history clearing preserve newer unrelated settings; JSON cases cover absent/null, arrays and type changes |
| Playback commands | 36,864 modeled operations across capacities 1, 2 and 64; FIFO admission, rejection, wake coalescing/rearming and disconnection; five-second outer deadline |
| Plugin HTTP | Thirty status/method/transport combinations inspect actual requests; origin reentry, redirect limits and invalid options are checked separately |
| Release evidence | Missing/duplicate checks, wrong identity, malformed types, contradictory results, durations and fixture titles fail; CLI subprocesses are bounded |

An isolated harness using the actual merge/queue functions and tests passed
normally and failed for four deliberate faults: whole-snapshot replacement,
absent/null conflation, silent queue overflow and wake rearming failure.
Inputs and logs are under `target/smarter-tests-sensitivity/`. This is targeted
sensitivity evidence, not exhaustive mutation coverage. No test dependency was added.

### Windows native suite execution 2026-11-23

The 56 null-output libmpv tests passed in 35.17 s with one worker and in
11.03 s with four workers on the same Windows host and pinned DLL. A second
four-worker run passed in 11.01 s with capture retention enabled. Each of the
five capture-producing tests wrote to its own directory. `cargo xtask native`
now caps the Windows default at four, and `RUST_TEST_THREADS` can override it.

## Feature-specific records

These retained records support links from the requirement inventory. Their
native/GUI scope remains narrower than complete interface parity.

### Mixed media and subtitle drops 2026-10-12

The typed open intent defers subtitle attachment until replacement media loads;
a later open/stop cancels it. The native test
`opening_dropped_media_with_subtitles_attaches_them_to_the_new_file` passed.
Bounded Windows `.url` parsing rejects malformed/out-of-section targets and
unsupported schemes. OS drag/drop, raw text/URL payloads and playlist insertion
remain separate GUI checks.

### Dropped 3D LUTs and player input 2026-11-23

IINA v1.5.0 routes a single dropped `.3dl`, `.cube`, `.dat` or `.m3d` file to
its existing video filter chain with nearest interpolation. Riina now uses
the same labeled replacement behavior. The manual preset and drop route
share the filter builder. It escapes the filename at the two FFmpeg parser
levels and quotes the graph by byte length for mpv. [FFmpeg filtergraph escaping](https://ffmpeg.org/ffmpeg-filters.html#Notes-on-filtergraph-escaping), [mpv fixed-length quoting](https://mpv.io/manual/stable/#escaping-spaces-and-other-special-characters)

On Ubuntu 26.04 in WSL, a GTK drag source sent native X11 file drops to the
player. At `9c0f8c7` the drop left the blue fixture unchanged. The first LUT
build had the same result because the separate resize registration replaced
the player's Winit input callback. The verified build combined those routes.
It applied a green LUT, replaced it with a red LUT under the same label,
and kept the active filter and media after a malformed LUT was dropped.
Full 1280 x 800 captures showed both color changes and the confirmed filter
OSD. During playback, a window resize kept the video aspect ratio. Both LUT
drops kept the resulting window geometry. Wheel input changed volume. A
subtitle-only drop kept the existing media, and a mixed drop opened the second
fixture and attached its subtitle. Quit exited normally. The Linux executable
SHA-256 was
`d64fa0c6e79d4a44d8a2c6cef13fa9ecfe8d0a4b3ef5fdb9041e923e077e04d8`.

The pinned Windows `cargo xtask native` suite passed 56 null-output tests,
including a paused-playback test that replaced a LUT with paths containing
Unicode, spaces, brackets, commas and an apostrophe. It kept the loaded media
and the unrelated horizontal flip filter. Native drag-and-drop evidence
in this batch covers X11.

The Windows workspace gate passed 869 Rust and 78 Python tests. The Linux
locked build, strict workspace Clippy and 832 Rust tests passed. Strict
workspace Clippy also passed the `aarch64-apple-darwin` source check with
Zig 0.15.2. Native macOS input and playback still need a desktop run.

### Welcome window file drops 2026-11-23

The Welcome window accepts native file drops through the same batching and
media preparation used by the player. It defers the open until the playback
renderer is ready. Plain media batches use the current window preference.
Mixed media and subtitles stay together in one playlist.

On Ubuntu 26.04 in WSL, a GTK drag source sent a subtitle-only drop to the
Welcome window. The window stayed open and displayed guidance. A second drop
with video and subtitles opened the player, presented the blue video fixture
and attached the dropped subtitle. The Welcome window hid, and Quit exited
normally. The three 1280 × 800 desktop captures showed the original Welcome
window, the error state and the playing fixture. The Linux executable SHA-256
was `2c1b1fa405c002e6eda59a9ac34377a0f4fbd5edb76ea259ea76510732282b0d`.
Dropping the same two video files opened two players with grouping disabled and
one two-entry playlist with grouping enabled. Both configurations exited
normally and left no Riina process.

The Windows workspace gate passed 871 Rust and 78 Python tests. The locked
Windows build passed ten native playback and shutdown checks on an isolated
desktop in 9.094 s. The locked Linux build, strict workspace Clippy and 834 Rust
tests passed. Native Welcome drop verification in this batch covers X11.

### Files dropped into the playlist 2026-11-23

The playlist sidebar resolves the visible insertion gap at drop time, including
the current scroll offset. Folder scanning runs in the background. The mpv
worker resolves the captured playlist item ID against the current order before
inserting. A removed target produces an unavailable-position error. Drops
below the final row append. The video surface retains its media-open action.

On Ubuntu 26.04 in WSL, a GTK drag inserted a folder before the second playlist
entry. The engine reported `Clip 2 — café.avi` followed by `Clip 10.avi`, with
the original video still paused at the same position. Dropping beneath the
rows appended another video, a subtitle-only drop left the list and tracks
alone, and a drop onto the video opened the new file. The two 1280 × 800 desktop
captures showed the list before and after insertion. Quit exited normally and
left no Riina process. The Linux executable SHA-256 was
`decf995355140df5c46ef956961d4d3670f527f55446467e57aad3bd1849f135`.

The Windows workspace gate passed 873 Rust and 78 Python tests, including
left- and right-sidebar hit testing, scroll offsets and stale insertion IDs.
The locked Windows build passed ten native playback and shutdown checks on an
isolated desktop in 9.227 s. The pinned libmpv playlist group passed four tests.
The locked Linux build, strict workspace Clippy and 836 Rust tests passed.
Strict workspace Clippy passed the Apple Silicon source check with Zig 0.15.2.
Native drag-and-drop verification in this batch covers X11. Raw URL and text
drops remain open.

### Playlist reveal during file drags 2026-11-23

Riina now checks the rightmost fifth of the player during a native file drag.
Winit supplies paths on entry, so Riina samples the cursor during the drag.
The playlist action runs after the cursor holds still for 300 ms with an
eligible file or folder. Directory checks run off the UI thread and check for
cancellation as they scan. The player reads the insertion location before closing the
temporary playlist. Music Mode keeps its own playlist control.

On Ubuntu 26.04 in WSL, a GTK drag confirmed that a subtitle-only payload left
the playlist hidden. Moving away before the delay kept it hidden. Holding a
folder at the edge revealed the playlist, and canceling closed it. A second
drag revealed it again, inserted the folder's videos before the second row
and closed the playlist. The original video stayed paused at 1.25 s. The
1280 × 800 desktop captures showed the video with the playlist hidden,
revealed and closed after cancellation. Quit left no Riina process.
The Linux executable SHA-256 was
`21fb04932fd8d7c18639a0d646d40d9d5670761311f38c93e84e9667aa626380`.

The Windows locked build and workspace gate passed, including 875 Rust and
78 Python tests. The Linux locked build, strict workspace Clippy and 838 Rust
tests passed. Native drag-and-drop verification in this batch covers X11.

### Wayland file and URL drops 2026-11-23

Riina negotiates the Wayland data offer on Winit's display. It reads URI lists
and plain-text media references off the UI thread. It uses compositor coordinates
for hover and playlist insertion. During the first Welcome test the compositor
delivered the drop, but the player stayed unmapped until it received an explicit
first-frame redraw. Riina now requests that redraw when it shows a Wayland
toplevel.

On Ubuntu 26.04 in WSL, Sway ran with XWayland disabled and no X11 display.
A native GTK file drag opened the fixture from Welcome and presented video.
A folder drag revealed the playlist, appended both files and closed the
temporary playlist. Playback stayed paused at 1.25 s. A plain-text HTTP URL
drop opened a second fixture served over loopback. The 1280 × 800 desktop
captures showed video and the revealed playlist. Quit closed Riina, and the
isolated compositor shut down. The verified Linux executable SHA-256 was
`9b8c7576ed925d776f28e2c1455d082722fa2686ef4bf8089fe7cae7d4b70ec1`.

The Windows workspace gate passed 875 Rust and 78 Python tests, and the locked
build passed. The Linux locked build, strict workspace Clippy and 839 Rust
tests passed. On X11, the same executable opened a mixed media and subtitle
drop from Welcome, attached the subtitle and quit normally.

### Windows file and URL drops 2026-11-23

The Windows OLE target handles Explorer file lists, browser URL formats and
plain Unicode text. File lists take precedence when a source offers both files
and a URL. The framed video window and its aligned controls share the same
playlist hover state. The target uses the release position to capture a playlist
insertion row before closing a temporary playlist.
[Windows Shell transfer formats](https://learn.microsoft.com/en-us/windows/win32/shell/clipboard)

Windows tests used a Shell data object to decode a Unicode file list and both
browser URL and text formats. A hidden HWND test verified registration and
revocation during destruction. A handoff test exercised crossing from video
to controls and capturing the target before closing the hover state. A pointer
drag from a Windows browser remains to be checked on an interactive desktop.

The Windows workspace gate passed 878 Rust and 78 Python tests. The locked
build passed ten playback, window and shutdown checks on an isolated Windows
desktop in 9.216 s. The test process exited normally. The checked Windows
executable SHA-256 was
`3a080c4cf0b985b4d5582c1eeab461a3dfad328d860814282a7ad13eed30aa16`.

The Linux workspace gate passed 839 Rust tests. On isolated native Wayland, a GTK
drag reopened media from Welcome, revealed the playlist and appended a folder
while playback stayed paused at 1.25 s. A plain-text URL opened a loopback
stream, and Quit closed Riina normally.

### macOS file and URL drops 2026-11-23

The AppKit destination reads modern and legacy file lists, browser URLs and
plain-text media references. File lists take priority. The destination supplies
the release position to the shared playlist target resolver before the
temporary playlist closes.

Strict workspace Clippy passed for `aarch64-apple-darwin` from Windows. The
Windows workspace gate passed 878 Rust and 78 Python tests. A linked macOS
build and desktop verification of Finder and Safari drops, playlist insertion
and cancellation remain open.

### Audio equalizer profile menu 2026-10-14

Core/native coverage checks gains and the dedicated filter. Profile identity
separates user names from built-in presets and migrates older saved selections.
macOS native prompts, localized labels and matched rendering remain open.

### Localized delay entry 2026-10-12

ICU tests cover localized decimal/grouping symbols, malformed grouping and
Arabic digits/bidi round trips. Values preserve entered precision while display
uses three significant digits. Runtime locale changes and matched field geometry
remain unverified.

### Manual-open repeat mode

`media_open` tests cover default-off behavior, playlist/file loop mapping and
validation only when enabled. Live preference interaction and exact native loop
behavior require their own evidence.

### Plugin playlist operations 2026-11-23

Player plugins can read the native playlist and queue additions, removals,
moves and playback navigation. QuickJS tests cover row fields, idle reads,
malformed arguments, partial additions and permission checks. The native test
inserts a fourth item and resolves both move IDs against the updated playlist.
Plugin requests used the same engine route as the playlist controls.

An isolated Windows plugin added two files, moved a row, played the next and
previous files, selected a specific row, and removed the other two rows. The
first run exposed that manual playlist transitions resumed playback with Pause
When Media Is Opened enabled. The worker now checks the current preference at
load completion. The repeated workflow kept the last file paused. Native
tests cover both preserving pause and resuming with the preference disabled.
Closing the framed player window ended the isolated Windows process normally.
The tested executable SHA-256 was
`4cee3b306fa3528f09cd27ab453dc46693aa57b8d846b5414d0118c124b69357`.
The separate Windows playback and controls smoke passed all ten checks and
exited normally in 9.066 s.

The Windows workspace gate passed 880 Rust and 78 Python tests. The locked
Windows build passed, and the pinned libmpv suite passed all 57 tests in
10.84 s. Linux x64 passed strict workspace Clippy, the locked application
build and 841 Rust tests. The Linux native loading and playlist group passed
nine of ten cases, including both playlist changes. The same Linux libmpv
build rejected `tone-mapping-param` in the unrelated UI-open regression on
both the current and earlier test binaries. Strict Apple Silicon workspace
Clippy passed from Windows. The playlist context-menu builder and native
macOS execution remain open.

### Linux tone-mapping default 2026-11-23

Direct libmpv calls isolated the failure to parsing `default` for the float
option. The affected Linux build accepted the numerical override and rejected
the reset both at startup and during playback. A local 0.41.0 rebuild added
the upstream float-option fix alongside the existing scaler-padding fix.
The rebuilt library accepted both reset paths, and all 57 native cases passed
with null audio output in 32.62 s. The checked revision was `e0a9cb6`.
This verifies option handling and playback state. Physical HDR is separate.
The tested libmpv SHA-256 was
`84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`.
[Upstream float-option fix](https://github.com/mpv-player/mpv/commit/ae119448b2c388f4fdee30e59bb1ab84f2cde428).

### Plugin preferences 2026-11-23

QuickJS tests exercised manifest defaults, shared global and player state,
plugin isolation and explicit persistence. Store tests exercised two
independent writers, lock contention, retry and malformed-file preservation.

On isolated Windows and Linux X11 desktops, a plugin read its manifest defaults
and saw a value from its global entry. It saved an updated setting and a nested
object, then made a second change without syncing. The saved values survived
a complete restart with the same profile and the pending change was absent.
All four player instances exited normally. The executable SHA-256 values were
`c5f89aa2566b8c8007454b508d395733809a219d321f16acd0a0d4b391aaa6a5`
on Windows and `3c34829bd17ae577d84faa6c400095c8e4ea095d6fa688599792de045f1d9887`
on Linux. The Linux run used the rebuilt runtime above.

The locked Windows workspace build, strict Clippy, 882 Rust tests and 78 Python
tests passed. Linux x64 passed the locked application build, strict workspace
Clippy and 843 Rust tests. Strict Apple Silicon workspace Clippy passed from
Windows. The plugin settings WebView and macOS desktop verification remain open.

### Plugin settings pages 2026-11-23

The manifest regression covers unavailable optional pages and external or
traversing paths. A Linux linked-package test verifies that the page serves
resources from the link target and uses the same installed preference store as
the plugin runtime.

The opt-in desktop test invoked the plugin row's Settings callback directly
on Windows, Linux X11 and native Wayland. A package page loaded defaults in
the WebView, changed text, checkbox, integer, decimal and radio fields, and
received its structured value through the preferences bridge. The shared
runtime store saw the edits before the dialog closed, and the persisted JSON
matched after close. The test used isolated desktops and fresh stores.
The follow-up below covers pointer routing. Matched macOS sheet presentation
remains open.

Windows passed its locked workspace build, strict workspace Clippy, 908 Rust
tests and 78 Python tests. Linux passed strict workspace Clippy, 120
plugin-related tests and its locked application build. Strict Apple Silicon
workspace Clippy passed from Windows.

The standalone window regression passed on Windows, X11 and Wayland after
sharing the window shell. The applications loaded their two plugin windows,
retained the reopened page, applied Simple Mode content and exited normally.
The Windows and Linux executable hashes were
`3bb7c9974473f1c53e1818976572ea2b19094272bbb0a992ee77edf8bacfdcff` and
`d591596ff14d708b593a8c6bd761ec8305873883c73fcf55058473c841e7bd70`.

### Settings navigation and plugin form dismissal 2026-11-23

The native Windows test initially failed when it replaced a direct Settings
callback with pointer events. The category's keyboard focus handler consumed
the first press. The category and subsection rows now move focus from their
click handlers. The headless regression clicks History, selects Plugins and
verifies that the Settings button opens the row that declared a form.

The native desktop test passed after the fix on Windows, X11 and native
Wayland. It dispatched pointer events to the Settings controls, verified
edits in the real WebView, then selected General and checked the persisted
JSON. Windows passed its locked workspace build, strict Clippy, 909 Rust
tests and 78 Python tests. Linux passed its locked application build, strict
workspace Clippy and 120 plugin tests. Apple Silicon cross-target Clippy also
passed.

An isolated X11 run clicked Plugins and the installed plugin's Settings button
through the application windows. It verified form values after a process
restart and closed both player instances normally. The Linux executable
SHA-256 was `2412bfdf3876beb714df01622f15df1211e128306a5d77dec8d31fab89a4e5ea`.
The same run found that the General switch for grouping files still needed a
second click to toggle.

### First-click controls and focused widget checks 2026-11-23

Settings switches, choice rows and disclosures now give pointer handling to
their click areas, which also move keyboard focus. The same pattern applies
to the affected Welcome, Quick Settings, crop and screenshot controls. The
headless tests click the General grouping switch and disclosure, open Welcome
actions, and dispatch audio and subtitle commands. Space still toggles the
focused switch, and Return repeats the subtitle search.

The fast gate groups changes to these widget tests with the UI interaction and
pixel comparisons. The scope tests also verify that application source edits
include the app unit tests. The focused Windows run took 26.5 seconds including
linting, 54 widget tests and the software-rendered pixel comparison. The Windows
locked workspace gate and build passed, including 78 Python tests. All 58
native engine tests passed with null audio output. The native Windows plugin
settings test saved WebView form edits when switching panes on a private desktop.

Linux passed the locked application build, strict workspace Clippy and 120
plugin tests. On a private X11 desktop, the production app's grouping switch
read false before the first click, true after it and false after the second.
The plugin form reopened with saved values after restarting the player, and
both player processes exited normally. The Linux executable SHA-256 was
`fa5e6a9052655661a2dcd0306de521f2c32f3021ab6d1ff89757019bff923d42`. The
engine SHA-256 was
`84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`.
Apple Silicon cross-target Clippy passed. Native macOS interaction remains
unverified.

### Plugin console 2026-11-23

The QuickJS tests checked variadic logs, indented objects and arrays, dates,
symbols, null and undefined. Warnings and errors retained their levels and
used the first argument. A circular object produced a bounded fallback message
and the entry continued. The installed player and global entry points use
the same API. The application log follows Riina's existing whitespace and
redaction rules.

The Windows workspace gate and locked build passed. An isolated playback run
recorded DEBUG and ERROR from the player entry and WARN and INFO from the
global entry, then exited normally. The Windows executable SHA-256 was
`bfa9d48d3f3c243468a2af9b39c316be979b91db2572433d1684a89c2850ecfc`.

Linux passed strict workspace Clippy, the locked application build and 121
plugin tests. Isolated X11 playback recorded each of the four messages once per
launch, restored the saved Settings form on the second launch and closed both
player instances. The Linux executable and pinned engine SHA-256 values were
`33a93a58410024b67942be2bb107d67a8a5356bf410e68c2240a133efcdece8f` and
`84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`.
Strict Apple Silicon cross-target Clippy passed. The IINA Developer Tool
console and native macOS behavior remain open.

### Plugin Developer Tool 2026-11-23

The Plugin menu lists live player and global entries under Developer Tool.
Each console owns the window and transcript for one QuickJS entry. Commands
run on that entry's existing worker and use its execution limits and reviewed
permissions. Reopening the window keeps its transcript. Reloading the plugin
closes the previous console and exposes the new entry in the menu.

The runtime regression evaluated commands in separate player and global
entries, received leveled logs and a thrown exception, listed global names,
cleared the console, and confirmed the old console could no longer submit
commands after its entry was replaced.

Windows passed the workspace checks and the compact toolbar follow-up passed
the focused app and UI checks and locked workspace build. On a private desktop,
the native menu opened both named consoles. Their Run buttons sent commands
to separate live entries, which wrote `11` and `101` to the test package's
data directory. The player exited normally with both consoles open. The
executable SHA-256 was
`04935f8af8fcc2a870d49366f61bd422a2c18f6c1f1e6cf59ad89f1d5753c1bd`.

Linux passed strict workspace Clippy, 122 plugin tests and the locked
application build. An isolated X11 run evaluated commands through Ctrl+Enter
in both console windows. Reload closed the old windows, and the replacement
player entry evaluated with fresh state. The compact toolbar follow-up passed
the headless layout regression and the same X11 workflow. The final X11
executable SHA-256 was
`1ee23fa2991ab0f67cd9454e47bee978fccef19a2f4d098c021fbf9552045537`.
It used engine SHA-256
`84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`.
Strict Apple Silicon cross-target Clippy passed before the toolbar follow-up.
Expandable object inspection, transcript spacing and native macOS interaction
remain open.

#### Expandable results and command recall

The follow-up shows numbered commands and compact result rows. Array, object
and global previews capture members on the entry's existing worker. Previews
are shallow and capped at 100 members, with an omitted count when applicable.
New output preserves expanded rows. Up Arrow recalls commands from the current
transcript, and Clear History clears recall.

The grouped Windows source gate passed strict Clippy, 917 Rust tests including
the software-rendered pixel comparison, and 78 Python tests. The locked
workspace build passed. The runtime regression covers both entry scopes,
exception stacks, cyclic and sparse values, a throwing property getter,
global values and reload. The headless widget regression checks row spacing,
expansion, recall and the compact toolbar in the same run.

On a private Windows desktop, the native menu opened both consoles. Windows
UI Automation expanded an object and confirmed it stayed expanded after new
output. Commands reached their respective entries, and the exact player
process exited normally with both consoles open. The executable SHA-256 was
`f06eb5af7693861bf6420a72fa55347d79c2508dd04723b05fce6c6144ec76f0`.

Linux passed strict workspace Clippy, 123 focused plugin tests and the locked
application build. On isolated X11, a pointer click opened the object and
revealed `total: 11` and `nested: [Object]` immediately under its summary.
Up Arrow recalled the command and Ctrl+Enter returned `12`. The global entry
returned `101`. Reload closed both old tools and the replacement player entry
returned `11` with fresh state. The first synthetic close request timed out.
The rerun focused the playback window and synchronized delivery of the X11
close request. Both windows closed and the exact process exited normally.
The Linux executable and engine SHA-256 values were
`11e00b51d5feea0a325fb4725415c3af671ca542855afa2f0fe71463e47cb8c6` and
`84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`.
Strict Apple Silicon cross-target Clippy passed. Native macOS interaction and
matched console appearance remain open.

### Plugin application menu hierarchy 2026-11-23

The Plugin menu preserves each package's global and player item order, with
submenus, nested separators and disabled branches. It places contributions
after the fifth top-level item under More. Slint's menu provider supplies
the same tree to the native menu, the in-window menu and shortcut handling.
The runtime still dispatches leaf actions to the entry that contributed them.

The Windows workspace gate passed 918 Rust and 78 Python tests and the locked
build. The headless regression reached a deepest-level action through its
shortcut, tried a branch's shortcut, replaced the contribution model and
verified that removal retired the old shortcut. The existing console layout
and recall regression now runs in the 56-test widget batch used by UI-only
checks. The software-rendered pixel comparison passed in the workspace gate.

A private Windows desktop run checked the native Plugin submenu order, the
More subtree, a nested separator, a checked leaf and a disabled branch with
its disabled child. It invoked four native menu commands and verified receipts
from the correct player or global entry. The selected state persisted after
activation. The player exited normally. A second isolated run of the same
binary passed all ten playback, native-menu, fullscreen and shutdown checks.
The Windows executable SHA-256 was
`217c79894e0623da7cf184d296bc6d3d492f1e7b490b701772a34c2ce2033812`.

Linux passed strict workspace Clippy, 511 app unit tests, 53 widget tests and
the locked application build. On an isolated X11 desktop, pointer navigation
opened Global tools, Player tools and More and ran a leaf in each. Captures
showed the checkmark and nested separator. The player exited normally. The
Linux executable and engine SHA-256 values were
`049c100df270d6b0fa29c57d129851794c696a1db906597a2d0f87717bc54857` and
`84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`.
Strict Apple Silicon cross-target Clippy passed. Native Wayland and macOS
menu interaction and the matched macOS comparison remain open.

### Plugin menu shortcut conflicts 2026-11-23

The headless profile regression changed the active profile from IINA Default to
mpv Default while nested plugin menu items were present. It verified that the
newly free accelerator was restored, the other conflict stayed visible, and
unloading cleared the count. The grouped Windows app and UI gate passed strict
Clippy, 550 app unit tests, 56 interaction tests and the headless pixel check.

A private Windows player showed two conflicts at the top of its native Plugin
menu. Both actions remained selectable with their accelerators removed. An
unrelated accelerator was still displayed. A player action changed its
shortcut and the warning count dropped to one. The global action removed its
own menu item and the warning cleared. The exact player closed normally. The
locked workspace build's executable SHA-256 was
`c3d3ce4719b377e6eccb524d06012989249b7b3d011954d999594f74c0f36653`.

The locked Linux build passed strict workspace Clippy, 517 app tests and
53 interaction tests. On isolated X11 and native Wayland displays, Space
retained its playback action and F8 reached the nested plugin item. Both menu
captures included the conflict count. Both test players closed normally. The
Linux executable SHA-256 was
`1afabe50aa482a84161de3650a72ff609eef44d5d6b17c520d79f2d9bee0fb86`.
Strict Apple Silicon cross-target Clippy passed. Native macOS interaction and
the matched Plugin menu remain open.

### Multiple native Windows players 2026-11-23

The Windows application owns the native players opened from its windows.
Each frame retains its own engine and playback state. New-window actions use
the application's duplicate policy. Close releases one frame and Quit releases
all remaining frames. The Windows process exits after its last player closes.

The locked Windows workspace build passed, along with strict Clippy, 918 Rust
tests and 78 Python tests. On a private desktop, the native Open in New Window
menu and file picker opened a second video in the same process. The first
video resumed while the second stayed paused. Closing the first left the
second open with quit-on-last enabled. Opening the second file again from a
third player reused the second frame with duplicates disabled. Quit closed
both remaining players. In a separate launch, closing the sole frame exited
normally and left no replacement process.

The same binary passed the native Plugin menu action checks and the ten
private-desktop playback, fullscreen, menu and shutdown checks in 9.295 s.
The executable SHA-256 was
`0c5520fe8411829c651e42a298534232e96d4a6754ff6a366cc88a668ac8a596`.
Application-level global plugin entries and plugin-managed player creation
remain open.

### Application-wide plugin entries 2026-11-23

The application owns one global realm per enabled package. Player realms stay
with their playback sessions. The headless multi-player regression exercises
shared state, both directions of messaging, global menu actions, window close,
reload, update suspension and reactivation. An early-start test covers package
activation while a player is initializing.

On a private Windows desktop, opening a second player left both videos in one
process. The global menu incremented one counter from either player and both
player realms received the broadcasts. The same Developer Tool and standalone
WebView opened from both menus. The WebView answered a page message. Closing
its opening player preserved both auxiliary windows and the surviving player
received the next count. Duplicate detection, application Quit and shutdown
after closing the last frame passed. The exact test processes exited normally.

Windows passed the locked workspace build, strict Clippy, 919 Rust tests and
78 Python tests. The Rust suite included the headless Windows player and Settings
pixel comparisons. The same executable passed all ten private-desktop playback,
menu and fullscreen checks in 9.074 s. Its SHA-256 was
`86484b8d8a5b990e9316fcb9cb9a9fc072ded54efef6bde2dfbf806140337a60`.

Linux passed strict workspace Clippy, 512 app tests, 53 interaction tests and the
locked application build. Its first run exposed a race between package activation
and player registration. Registering the player before exposing its host handle
resolved the failure. An isolated X11 desktop opened Global tools, Player tools
and More, dispatched their leaf actions and closed the exact test player.
Strict Apple Silicon cross-target Clippy also passed.

Plugin-managed player creation and the pinned message routing rules remain
open. The native Windows check used an isolated desktop. macOS runtime and
matched interface evidence remain open.

### Plugin playlist context menus 2026-11-23

The headless player test sent a right click to a playlist row and checked
its stable token and window coordinates. The plugin host test checked
zero-based selections, nested items, separators, checked and disabled states,
player and global entry boundaries, and action delivery. It exercised rejection
of stale requests and replayed commands in the same test session.

On an isolated Windows desktop, UI Automation opened the playlist, and a
right click on the file row delivered `[0]` to the builder. The native popup
contained the contributed title. Posted input on the inactive desktop did
not select that item, so native selection still needs interactive verification.
The test player exited normally. The Windows executable SHA-256 was
`0cb70e2b89e00fdfefe22507423ffab5044e022dae1b5e818acc05cc82bf8ff4`.

### Plugin-managed players 2026-11-23

The grouped host regression covers denied media access, numeric IDs, messages
queued during creation, ordinary player labels, managed broadcasts, child
labels, selective plugin loading and controller retirement. It exercises
reload and update suspension on the same host fixture.
The headless player callback group also checks that an idle managed window
appears during the first load and stays hidden after a later explicit hide.

On a private Windows desktop, the controller opened a paused video and created
an idle player in the same process. Both children received numeric messages
queued during startup. The ordinary player received a string-addressed reply.
The idle child opened through its own core API, and both children received a
broadcast. Closing one child preserved the other. Reload retired the surviving
child and left the ordinary player open. Closing that player exited the exact
process with no replacement. The same executable passed ten private-desktop
playback, menu and fullscreen checks in 9.225 s. Its SHA-256 was
`b58586716de1802933ae5aa40a4179969f648fb9f93b68c362e3be247680c9d2`.

An isolated Linux X11 run verified all three message routes, paused playback
in both children, and independent close behavior. The locked Linux application
build, strict Clippy, 512 app tests and 53 interaction tests passed. Windows
passed the locked workspace build, strict Clippy, 919 Rust tests and 78 Python
tests. The pinned null-output engine passed all 58 native tests in 11.20 s.
Strict Apple Silicon cross-target Clippy passed. Native macOS and managed-player
Wayland interaction remain unverified.

### Shared plugin overlay presentation 2026-11-23

Both Windows renderers loaded a confined test page and switched to Simple
Mode on a private desktop. The WebViews reported the expected opacity, content
and styles. Both documents emitted their load events and returned messages to
the plugin. Each player exited normally. The same binary passed the ten
playback, fullscreen, menu and shutdown checks in 9.215 s. The Windows SHA-256
was `5f5a9e818ae71ffa1e6033c207e25d1ac70dbc59c0de36fb5ae0a3e1ce8c6933`.

The headless tests cover stale document completion, stale replies after plugin
reload and hit-test invalidation after a cursor move. The Windows source gate
passed strict Clippy, 922 Rust tests and 78 Python tests. The failed menu callback
from the first run reproduced in isolation. The callback passed with a 512 KiB
QuickJS stack budget, which includes native host frames. The private desktop
rejected full-screen capture, so pixel and physical pointer evidence remains
open.

Strict Apple Silicon cross-target Clippy passed. The locked Linux build,
513 app tests and 53 interaction tests passed. On X11 and Wayland, the shared
page bridge preserved independent player and global standalone WebViews,
including close, reopen and Simple Mode while paused. Native macOS overlay
interaction and Linux overlay presentation remain open.

### Linux plugin overlay composition 2026-11-23

The Windows workspace build and source gate passed 922 Rust tests, including
the headless interaction and pixel checks, and 78 Python tests. The pinned
null-output engine passed all 58 native tests in 11.51 s. Both Windows renderers
loaded the packaged and Simple Mode overlay documents on a private desktop.
Each document reported its expected content and delivered messages to the
plugin. Both test players exited normally, leaving no Riina process. The
Windows binary SHA-256 was
`b02d9f317c7e0f13e29b91311935da6a3218d53904c73e20f7a4b471f99d5e3e`.
The private desktop denied full-screen capture.

The same locked Linux x64 build passed on a private Xvfb display and a native
Wayland Sway compositor under Ubuntu 26.04 in WSL. Both runs captured the
1280 × 800 desktop with the isolated video paused and verified the overlay's
translucency, color changes after a trusted click, resizing and Simple Mode.
Clicking the non-clickable Simple Mode content resumed playback. Both runs
closed the exact player normally and left no Riina process. The application
and libmpv SHA-256 values were
`be2889db0d76679fa7bb97e73eadad1456ec9729d1820a848e3ddf4a7a896517`
and `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`.

Linux strict workspace Clippy, 516 app tests and 53 headless interaction tests
passed. The Cairo conversion test checks transparent pixels, alpha, row padding
and unchanged-frame detection without starting a display. Strict Apple Silicon
cross-target Clippy passed. Native macOS overlay interaction remains unverified.

The locked Windows workspace build, strict Clippy, 883 Rust tests and 78 Python
tests passed. Linux x64 passed the locked application build, strict workspace
Clippy and 844 Rust tests. Strict Apple Silicon workspace Clippy passed from
Windows. The native popup presentation is implemented for Windows playlist
sidebar rows. Blank-space menus, Music Mode and popup presentation on Linux
and macOS remain open.

The follow-up reused the same headless widget and engine-recording tests to
cover clicks on rows and empty space in the sidebar and Music Mode, and the
Add URL action. Invalid URLs produced no engine command. A valid URL sent
an append command to the current session without replacing playback. A
player-realm builder also received an empty index list in the host test.

Three isolated Windows sessions opened the sidebar below its first row,
a Music Mode row, and the blank space below that row. The plugin received
`[]`, `[0]`, and `[]` respectively. Each native popup contained the contributed
item and Add URL. The player exited normally after the menu was dismissed
in each session. The Windows executable SHA-256 was
`1da782a2840e0302c18562f43bcb97900366f550acf4dcc14902454a02e8be1a`.

The locked Windows workspace build, strict Clippy, 883 Rust tests and 78
Python tests passed. Linux x64 passed the locked application build, strict
workspace Clippy and 844 Rust tests. Strict Apple Silicon workspace Clippy
covered the new AppKit adapter from Windows. AppKit linking and desktop
verification, interactive Windows popup selection, and plugin popup
presentation on Linux remain open.

### X11 plugin playlist menus 2026-11-23

The shared playlist request identifies the current surface and row tokens,
then asks player entries to build their menus. X11 presents the resulting tree
with GTK on the owning Winit window. Selection returns to the shared action
handler and rechecks the request generation, surface and playlist order.
Closing the player invalidates pending requests and dismisses its menu.

An isolated X11 run opened the sidebar's blank-space menu, hovered through two
submenus, and clicked a checked plugin item. It received an empty selection
array. A row right-click sent index `0`, and Escape dismissed its menu. In
Music Mode, the row and blank-space menus sent `[0]` and `[]` respectively.
These checks passed with the default video-surface right-click action and with
that action disabled. The player exited normally with the last menu open.

The Windows workspace checks passed before the close-time invalidation change.
The final code passed the focused Windows app gate, which included 54 widget
tests, and the locked workspace build. Strict Apple Silicon cross-target
Clippy passed. Linux passed the locked application build, strict workspace
Clippy and 121 plugin tests. The tested executable SHA-256 was
`dc69fefeb46d3676285c1510e8f9e54d27c3e4a7886f069e84a83f96de3d1c80`
with engine SHA-256
`84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`.
Wayland plugin menus and native macOS desktop verification remain open.

### Wayland plugin playlist menus 2026-11-23

Wayland uses the shared playlist request and action validation. The player
hosts the contributed menu, with checked and disabled items and a separate
panel for each open submenu. Menu bounds are clamped to the client and
keyboard navigation skips separators and disabled items. The menu can
overlap its parent in a narrow Music Mode window, where Left Arrow returns
to the parent.

The native test exposed a conflict between submenu navigation and the
player's seek shortcuts. Slint processes menu-bar shortcuts before the
focused widget. The player suspends those shortcuts while the playlist popup
is open. The regression verifies that the arrow keys navigate the popup
and resume seeking after it closes.

The Windows workspace checks and locked build passed, including the headless
narrow-window keyboard regression, the 54 existing widget tests and the
software-rendered pixel comparison. A separate Windows playback run loaded both
plugin scopes and exited normally. Its executable SHA-256 was
`c510644debf4f54c3debf056546bd039979cc7fda54b8e7e4a9dc5fbb2ddfb31`.
Strict Apple Silicon cross-target Clippy passed.

The final Linux application passed the locked build and strict workspace
Clippy. On an isolated Sway compositor with XWayland disabled, pointer and
keyboard input both selected a checked item two submenus deep. The sidebar
and Music Mode each delivered `[0]` for a row and `[]` for blank space. Escape
and an outside click dismissed open menus. The player exited normally with
its last menu still open. The same executable also passed the native X11
sidebar and Music Mode popup sweep.

The tested Linux executable SHA-256 was
`e1c720707fc67baf6ec7c19c7546c2f68afe356f04ad135c36ea5c8cab5373cf`
with engine SHA-256
`84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`.

### Plugin mpv property reads 2026-11-23

The existing native observation test also reads the engine version before
loading media, then reads the paused flag, duration, path and track list.
It checks that a missing property returns no value. The plugin host test
covers the four JavaScript defaults and the absence of the player API from
global entries.

In isolated Windows and Linux X11 sessions, a player plugin read the real
mpv version during entry evaluation. During playback of the five-second AVI
it read pause, duration, source path, and the video and audio tracks. A missing
property returned `false`, `0`, `null`, and `null` through the four getters.
Both application processes exited normally. The Windows executable SHA-256
was `67f10fd6208d7c8f6e2c0e437712928810896dd61c6ee5f8f5a79593ae3b9513`
with libmpv SHA-256
`a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`.
The Linux executable SHA-256 was
`9fb5e6262425a3f268b29fb30cba6ee980c5cbafa2f624531d70175d3be0bbcb`.
It used the rebuilt libmpv recorded in the Linux tone-mapping verification.

Windows passed the locked workspace build, strict Clippy, 883 Rust tests,
78 Python tests and all 57 null-output native tests. Linux x64 passed the
locked application build, strict workspace Clippy, 844 Rust tests and all 57
null-output native tests with the provisioned runtime libraries. Strict
Apple Silicon workspace Clippy passed from Windows. macOS playback and the
remaining mpv plugin methods still need verification.

### Plugin mpv property writes 2026-11-23

The native observation test now writes a number, flag, string and language
list and reads each result back. It also checks reply errors and volume
writes while boost is enabled and after it is switched off. The policy test
checks that network options require a separate grant and that scripts and
the IPC server remain under application control.

Isolated Windows and Linux X11 sessions used a plugin with a reviewed
filesystem grant. It changed speed, mute, looping and the language list,
and added a local subtitle path to the external file list. The same plugin
was denied script settings, direct volume-range changes, unreviewed
network options and a remote subtitle URL. An initial Linux run exposed
that its libmpv accepted a direct 150% write with a 100% maximum. After the
worker enforced the active preference range on writes, both platforms
reported 100% and exited normally.

The Windows executable SHA-256 was
`81bfba3385859d5ced16c170f9bf687fd1b1a14fd880683436249e909de95d42`.
The Linux executable SHA-256 was
`de82e8a2ea1c4b2b0b2f47926680fc7b2ca6866f3b06526878a114369666e192`.
Both sessions used the native libraries recorded in the property-read
verification above.

Windows passed the locked workspace build, strict Clippy, 884 Rust tests,
78 Python tests and all 57 null-output native tests. Linux x64 passed the
locked application build, strict workspace Clippy, 845 Rust tests and all
57 null-output native tests. The Linux native batch reused its harness
from the workspace tests with the same Cargo target. Strict Apple Silicon
workspace Clippy passed from Windows.

### Plugin mpv commands 2026-11-23

The command-plan test and native suite cover literal arguments, acknowledged
playback commands and the active volume range. The plugin policy test rejects
unreviewed media loads, process execution, NUL arguments and unsupported
Quit forms.

On isolated Windows and Linux X11 desktops, a plugin appended a second AVI
using its reviewed filesystem grant and read the updated native playlist
immediately. It sought to two seconds and selected the second file. Both
players were paused when the plugin received the second file's loaded
notification. The test also checked that punctuation in a text argument did
not alter playback and that a 150% volume request remained at 100% with
boost off. Both players exited normally after the plugin called
`iina.mpv.command("quit")`.

The Windows executable SHA-256 was
`f99f22fad657a56671f291bccb27861823e590b2d2a3a58f970be97d04fda045`.
The Linux executable SHA-256 was
`d8aabe3b0660db8b5705e9600a5c91b686db4dc5a68546c9d0aa315aca640709`.
Both used the native libraries recorded in the property-read verification.

Windows passed the locked workspace build, strict Clippy, 885 Rust tests,
78 Python tests and all 57 null-output native tests. Linux x64 passed the
locked application build, strict workspace Clippy, 846 Rust tests and all
57 null-output native tests. Strict Apple Silicon workspace Clippy passed
from Windows. macOS playback, native rendering interactions and the
remaining plugin command forms still need verification.

### Plugin mpv hooks 2026-11-23

The native hook regression covers priority and registration order, live property
reads during an active hook, repeated continuation and release on plugin unload.
The plugin-host regression queues the initial media callback before the host starts
and checks that both configured entries have initialized when it runs. A pending
hook has a 15-second playback deadline.

On isolated Windows and Linux X11 desktops, a deliberately delayed entry still
received the first file's `on_load` hook. A second AVI exercised synchronous return,
async continuation, a thrown exception and a rejected promise in native priority
order. `mpv.hook` reached the event listener, and `iina.file-loaded` arrived after
the complete hook sequence. Both players exited normally through the plugin Quit
command.

The Windows executable SHA-256 was
`bcac0960dac38cca91821b6eb45cc444b08ceda3f208a534182bae9e970b6591`.
The Linux executable SHA-256 was
`82e380c35d63195cb54b10a5f2edc1d8f97c27d72d8be4e13042c7f01cf8fd07`.
Both used the pinned native libraries recorded in the property-read verification.

Windows passed the locked workspace build, strict Clippy, 885 Rust tests,
78 Python tests and all 58 null-output native tests. Linux x64 passed the locked
application build, strict workspace Clippy, 846 Rust tests and all 58 null-output
native tests. Strict Apple Silicon workspace Clippy passed from Windows. Native
macOS playback still needs verification.

### Plugin WebSocket servers 2026-11-23

The realm regression uses real TCP sockets and WebSocket clients. It covers
separate permission review, bind failures, duplicate startup, UTF-8 and
non-UTF-8 messages, typed arrays, ping/pong, send completion, unknown connection
IDs, connection callback order, same-port replacement and realm shutdown.

On isolated Windows and Linux X11 desktops, the player loaded a test plugin
with a reviewed listener grant. A localhost client completed a WebSocket
handshake, sent UTF-8 text and received the echoed bytes in a binary frame.
The plugin saw connection readiness and send completion. Both player processes
exited normally through the plugin Quit command.

The Windows executable SHA-256 was
`27b94580c6feafe35b53d5fecf6eea7c4c8a1b547c67c3361d6461871e16aa9b`.
The Linux executable SHA-256 was
`8cf741d5b84af55a88e9cc30c794901db58fa2debad692788abadb8e16969ce7`.
Both used the pinned native libraries recorded in the property-read verification.

Windows passed a locked workspace build, strict Clippy, 888 Rust tests,
78 Python tests and 58 native playback tests. Linux x64 passed a locked app
build, strict workspace Clippy, 849 Rust tests and 58 native playback tests.
Strict Apple Silicon workspace Clippy passed from Windows. Windows packaging
generated the notices for all 431 Rust dependencies. Native macOS playback
and Network.framework-specific event timing remain unverified.

### Plugin subtitle providers 2026-11-23

Manifest tests cover provider names, stable IDs and malformed declarations.
The QuickJS test exercises async search and download, keeps each result's opaque
data in its owning realm, and checks the custom-provider sentinel, aging results,
and denied file paths. The playback plan test checks that a download for a prior
media generation issues no subtitle command.

Isolated Windows and Linux X11 players loaded a test provider from the saved
provider preference. Using a 120-second local AVI derived from the checked-in
frame fixture, each player automatically searched once and attached the single
returned subtitle. In a second run per platform, the provider returned two
items. Selecting the first row and activating Download attached that item's
subtitle. The Windows chooser was exercised at 300% display scaling. The
test plugin verified the attached track through libmpv and closed the exact
player process normally.

The Windows executable SHA-256 was
`4dc4fa912c21aeaa23a856701f78ad49bc8b68910b2ef16a0735393501e5f834`.
The Linux executable SHA-256 was
`f80c265a7f5b7120f65872fc46d275bb7d72e21260e9a59bedb2ae4ec7867fac`.
Both used the pinned native libraries recorded in the property-read verification.

Windows passed the locked workspace build, strict Clippy, 891 Rust tests,
78 Python tests and 58 native playback tests. A follow-up batch of 48 subtitle
tests also passed, including the chooser's row selection and download actions.
Linux x64 passed the locked app build, strict workspace Clippy, 852 Rust tests
and 58 native playback tests. Strict Apple Silicon workspace Clippy passed from
Windows. Native macOS playback, matched chooser pixels and third-party provider
conformance remain open.

### Plugin input 2026-11-23

QuickJS tests cover listener priority, consumption, replacement, removal and
player isolation. They also exercise reading the updated key profile from an
active realm. Headless player tests exercise synchronous profile publication
before the event loop, key delivery around the normal binding, dialog protection,
handled mouse-up and the normal click path.

An isolated Windows player used the saved pause-on-open and click-to-pause
settings. A plugin received F9 press and release as well as left-button press,
drag and release. It consumed mouse-up and confirmed playback stayed paused.
The test player closed normally on the private desktop. Executable SHA-256
`16c76af3ee8646c69c531ecf9e71b871ce1a214ba55074515230a17f0868cdf1`.
The isolated Linux X11 player passed the same workflow and confirmed that its
plugin could read the Space binding during entry initialization. It also
closed normally. Executable SHA-256
`355693ed9394b8559b6814773f6f5cb299e904dfe05eaa3229455e853d3f6d59`.
Both tests used the pinned libmpv libraries recorded under the property-read checks.

The first batch passed 857 workspace Rust tests on Linux x64 and 58 native
playback tests on each platform. After the startup profile fix, Windows passed
the locked workspace build, strict Clippy, 896 workspace Rust tests and 78
Python tests. Linux x64 passed the locked application build, strict workspace
Clippy and 109 plugin-related tests. Strict Apple Silicon workspace Clippy
passed from Windows. Native macOS input, pressure-sensitive devices, native-menu
shortcuts and key sequences remain open.

### Plugin utilities 2026-11-23

The utility regressions check grant review, private path containment, credential
namespacing, picker selection and cancellation, independent stdout and stderr
delivery, nonzero exit status, missing helpers, cancellation and the output cap.
The launcher runs native programs under the user's account with a limited
environment and a direct argument vector.

An isolated Windows player ran a reviewed helper, compared streamed output to
the resolved result and returned cancellation from its native file picker. An
isolated Linux X11 player passed spaces, an ampersand and Unicode as literal
arguments, confirmed a nonzero exit and received the numeric missing-binary
error. Both players stayed paused and closed normally. The Windows executable
SHA-256 was `1198520cbb3e2322ebbcae53284dbd9e78c1a52867f2939dc93b28fda9672cb3`.
The Linux executable SHA-256 was
`c67bc52bad4757f4006f49dc64198def97c840b1c94396ffdb28b82794a41957`.
Both tests used the pinned libmpv libraries recorded under the property-read checks.

Windows passed the locked workspace build, strict Clippy, 901 Rust tests and 78
Python tests. Linux x64 passed the locked application build, strict workspace
Clippy and 114 plugin-related tests. Both platforms passed 58 native playback
tests. Strict Apple Silicon workspace Clippy passed from Windows. Native
credential-store round trips, live file selection and native macOS utility
behavior remain open.

### Plugin confirmations and text prompts 2026-11-23

QuickJS tests exercised startup confirmations, empty and Unicode answers,
cancellation, and dropping a realm with a pending prompt. A host shutdown test
closed the plugin host during startup while the interaction was still pending.
The headless Slint test used the actual prompt controls to enter text, submit
with Enter and cancel with Escape.

The isolated Windows player left a plugin's startup confirmation open beyond
the three-second initialization watchdog. The plugin accepted the confirmation,
read typed text, received cancellation from a second prompt and observed the
paused clip load after its entry resumed. Windows executable SHA-256
`6eb3040d5a40bcef8c99757904ec4d27201f641ec0e4e64ac1e056a39e23f6b8`.
An isolated Linux X11 player passed the same confirmation, typed response and
cancellation sequence. Linux executable SHA-256
`9cfb95eff6be9f209bce2d5d0554d234dbe342d38e57f796e6af81d067932541`.
Both players closed normally and used the pinned libmpv libraries.

A Windows test stored a synthetic credential through the utility API, read it
from a new realm of the same plugin, verified isolation from a second plugin
and deleted the entry. Windows passed its locked workspace build, strict Clippy,
905 Rust tests and 78 Python tests. Linux x64 passed its locked application build,
strict workspace Clippy and 117 plugin-related tests. Both platforms passed 58
native playback tests. Apple Silicon strict workspace Clippy passed from Windows.
Native macOS interaction and Linux and macOS credential-store round trips remain
open.

### Standalone plugin windows 2026-11-23

QuickJS tests exercised distinct player and global documents, window properties,
confined paths, stale message rejection, close state and realm retirement. The
protocol tests checked that each window uses a distinct origin and that similar
hostnames cannot pass the origin check.

On an isolated Windows desktop, two native WebViews in the same Riina process
exchanged messages with their respective player and global entries. Closing
and reopening the player entry's window preserved the page's counter. The
player then switched to Simple Mode, and the page reported the applied CSS and
markup. The clip remained paused throughout the test. Quit closed the exact
Riina process normally while both plugin windows were open.

The Windows application SHA-256 was
`0f06701a54b257c972fc2ea752da151d39ffed20d8f30ded977309f707a9abd1`.
The pinned libmpv DLL SHA-256 was
`a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`.

The isolated Linux X11 check passed the same two-window messaging, native
close and reopen, Simple Mode styling and normal shutdown sequence. The Linux
application SHA-256 was
`5dbb4a5dabc80b03e62ef86dd9c8693c1d3f8ae38c662556f13b77f78245c3a8`.
The pinned libmpv shared library SHA-256 was
`84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`.

The same Linux build passed the standalone lifecycle on Sway with XWayland
disabled. The isolated mount namespace hid WSL's detection markers because
Slint selects X11 when it detects WSL. The compositor sent a native close to
the player plugin window. Reopening kept its page counter, the new Simple Mode
document reported its style and markup, and Riina quit normally.

Windows passed its locked workspace build, strict workspace Clippy, 907 Rust
tests, 78 Python tests and 58 native null-output playback cases. Linux x64
passed strict workspace Clippy, 119 plugin-related tests and its locked
application build, and 58 native null-output playback cases. Strict Apple
Silicon workspace Clippy passed from Windows. Native macOS WebView behavior and
matched window geometry remain open.

### Plugin WebView navigation and close shortcuts 2026-11-23

On a private Windows desktop, the overlay and sidebar each sent a message from
their first package page. The executable from `fefc415` received no messages
after loading the second pages. The updated executable received messages from
both pages in each WebView and exited normally. It also passed the separate
standalone reopen and Simple Mode sequence. The updated executable SHA-256 was
`c05cabdd4b9f6b9da7d49feda7599a5b6b8afb0f7bba2a1c131af08ee3b7894e`.

On isolated Linux X11 and native Wayland, Ctrl+W closed the focused standalone
window. Reopening kept the page counter, Simple Mode applied the requested style
and markup, and the fixture stayed paused. Riina quit normally. The Linux
executable SHA-256 was
`94c860d49e7c60a3716aa349a21a92a3ce8cb24f95b5b4f18ec21ede20ab6497`.
Both platforms used the same pinned engines as the preceding test batch.

Windows passed its locked workspace build, strict workspace Clippy, 907 Rust
tests and 78 Python tests. Linux passed its locked application build, strict
workspace Clippy and 119 plugin tests. Strict Apple Silicon workspace Clippy
passed from Windows.

### Plugin sidebar manifest discovery 2026-10-14

Manifest tests cover bounded localized labels. Runtime tests cover confined
documents, messages, visibility, late activation and lifecycle delivery. A Windows
smoke displayed a package-owned sidebar page.

On the private Linux X11 display, the `63dfb72` executable displayed a local
plugin page using WebKitGTK. The page sent a readiness token to the plugin, and
the plugin's reply changed the page background. The full desktop capture
contained 16,321 pixels of the reply color. A window-manager close request
ended the player and its WebKit helpers. GTK logged frame-clock warnings.
Wayland embedding and native macOS plugin behavior remain unverified.

At `55fe191`, the startup-enabled X11 sidebar again completed the message
round trip and the window-manager close ended the player. In a separate
920-pixel-wide player, enabling the same package through Reload All Plugins
opened the sidebar during playback, but its WebKit surface stayed black.
The same late-load trial at `63dfb72` showed the black surface too. Both
players closed normally.

The X11 window tree isolated the late-load failure at `55fe191`. In the
920-pixel-wide player, the sidebar container was 336 × 417 and its WebKit child
stayed at 1 × 1. Resizing the player allocated the child and revealed the page.
A sidebar loaded at startup resized correctly to the same wide player size.
Forcing WebKitGTK's shared-memory transport did not alter the late-load result.
The control at `0ba7173` reproduced the black WebKit child.

Starting the child with the current viewport at `7b0ce17` restored rendering
while playback continued. At `a6c1062`, replaying the allocation after GTK
shows the child matched the 336 × 417 sidebar on both sides of the player.
Both sessions opened a plugin during playback, completed its bidirectional
message exchange, removed the native child on disable, and reopened it
without resizing the player. Both players and their WebKit helpers exited.
The Linux executable SHA-256 was
`a9e8939d6a47e17fbf541d0959e6c57840cb9c734525b23ad322ed74c396863c`.

With focus in a plugin text field, `a6c1062` accepted typing but Ctrl+Q left
the player open. At `0dc5222`, the field still accepted typing, Ctrl+Shift+Q
left the player open, and Ctrl+Q exited normally through the application's
Quit action. A separate run opened two players in one process. The second
player's plugin text field accepted a bare Q key, and Ctrl+Q closed both
players and exited normally. The Linux executable SHA-256 was
`f6ed04be4c8bec1404f21ff8695e84a1b24836a5165cf0249d8a16bbf8aa1997`.

### X11 sidebar menu shortcuts 2026-11-23

With two players open at `98993b8`, the focused plugin field accepted typing
and Ctrl+A, and Ctrl+Q exited normally. Ctrl+, did not open Settings, and
Ctrl+H left both players visible.

The Linux adapter now forwards fixed menu shortcuts to the player that owns
the WebView. The WebView retains text editing shortcuts. Jump To and Open URL
temporarily hide the native WebView while the player displays its entry dialog.
In the first shortcut build, Ctrl+W left the player's window visible. Slint
holds an extra reference until `hide()` runs. An intermediate build hid the
window and exposed an X11 `BadWindow` error during GTK child teardown. The
close paths now notify the plugin, release its sidebar before the parent
window, and clear the borrowed GL frame before asking Slint to hide the player.
The X11 presenter waits for GTK's queued child requests to finish before
releasing the parent. [GDK display synchronization](https://docs.gtk.org/gdk3/method.Display.sync.html)

The verified X11 build opened Settings, Playback History and both filter
windows from a focused plugin field. Ctrl+M minimized only the owning player,
and Ctrl+H minimized both players and Settings. Jump To and Open URL accepted
Escape and restored the plugin page and its text. The native new-player picker
canceled and reopened, then loaded another fixture. Both Ctrl+W and a window
manager close left the other player running. Ctrl+Q then exited normally.
The plugin recorded one `iina.window-will-close` event per player. The Linux
executable SHA-256 was
`53751550012453736175b5b6a379e41588826d32ca46303d2eff0c28a36f1cf1`.

Strict workspace Clippy, the locked Linux application build and all 831 Linux
workspace tests passed. The Windows `cargo xtask check` passed 868 workspace
tests and 78 Python tests. The pinned Windows `cargo xtask native` run passed
all 55 engine cases with null audio output.

### Wayland plugin sidebar 2026-11-23

The locked Linux build passed strict workspace Clippy, 517 app tests and 54
interaction tests. The shared headless regression checks that Space reaches
the focused sidebar and resumes its usual playback action after focus returns
to the player. The corresponding Windows gate passed 550 app tests, 57
interaction tests, the headless pixel check and 78 Python tests. Strict Apple
Silicon cross-target Clippy passed.

On isolated Ubuntu 26.04 displays, X11 and native Wayland each loaded a plugin
sidebar beside paused video and exchanged messages with its player entry.
Both received trusted clicks and typed text, moved between browser fields with
Tab, and kept Space in the focused text field. Resizing changed the browser
viewport. Hiding and reopening retained the fields. On Wayland, Ctrl+, opened
Settings from browser focus and Settings closed separately. Both players
closed normally. On X11, the browser received the wheel and scrolled. In the
Wayland run, GTK received the routed wheel event but the page did not scroll
or report a wheel event. Native input methods and accessibility remain open.
The same build passed the Wayland video overlay checks for blending, live style
changes, resizing, Simple Mode, click-through and normal shutdown.
The Linux executable SHA-256 was
`d4893b0c458e0bc88a2476e798e7a3792e13b27eb6f6afe6331cea6f770530d3`.

The locked Windows workspace build opened its native sidebar and returned
the plugin message on a private desktop. The exact player exited normally.
Its SHA-256 was
`de8f0647a9480a89f2eb4443adf10fdb0cd5196cbade0a012808151cbed27262`.
Native macOS interaction remains unverified.

### Plugin sidebar document lifecycle 2026-11-23

The host regression verified that a retired sidebar cannot send a delayed
reply to a reloaded plugin, and that the current sidebar can still reply.
The combined Windows gate passed 926 workspace tests and 78 Python tests,
including the Windows headless pixel comparison. Linux passed strict
workspace Clippy and the app and widget test groups.
On isolated Ubuntu 26.04 X11 and Wayland displays, the sidebar exchanged
messages with the player, retained form text when hidden and reopened, and
loaded a replacement package document. The replacement received its own
queued message while the original video stayed open and paused. The X11
scenario also verified wheel scrolling. Both players closed normally.
The tested Linux executable SHA-256 was
`c06ab0f391506d47a759e1a4135715ceb88847cebd38adee8be2d00235999f6f`.

On a private Windows desktop, the sidebar opened and returned the player's
message before the exact player process exited normally. The tested Windows
executable SHA-256 was
`043dda75907de0fde0a862d8d1d61a21fe0a6e979681a5dac26b24cab801d6ec`.
Wayland wheel scrolling and native macOS interaction remain open.

### IINA plugin preferences import 2026-11-23

The Windows workspace gate passed 927 Rust tests and 78 Python tests.
The regression covered XML values in QuickJS player and global entries,
binary values, case-sensitive package identity, and merge-on-save.
Apple Silicon cross-target Clippy passed. The Windows dependency inventory
included the original MIT notice from the locked parser crate.
On a private Windows desktop, the native Settings form displayed the
imported value, applied changes, and switched panes. The saved JSON
retained an untouched setting and the original property list remained
byte-for-byte intact. The test process exited normally. Its SHA-256 was
`74a9f32f00d26c02568c45db5cb8ada7dfbc1ec652c9c43544f5bbc43054de57`.
Linux passed a locked application build, strict workspace Clippy, and
889 workspace tests. The native
Settings form check also passed on isolated X11 and Wayland desktops.
Both used test executable SHA-256
`d884156f56dadab4ec15fb54b78d5506f876cc34611e83c3d19cfe3a05bbfb85`.
Native macOS Settings remains unverified.

### Plugin package removal and development links 2026-11-23

The Windows app and UI gate passed 553 app tests, 57 widget tests, the
software pixel comparison, and the Settings navigation checks. Linux passed
strict workspace Clippy, a locked application build, 520 app tests and 54
widget tests. The lifecycle regression starts installed and development
packages together, retires the installed package, and checks the other
package, grants, enabled state and retained data. Strict Apple Silicon
cross-target Clippy passed.

On isolated X11, the Settings actions menu opened during paused playback.
A controlled confirmation helper returned Cancel and then Yes in two visits.
The first visit preserved the installed folder. The second removed it, stopped
its global and player entries, kept its data, and left the linked development
project running. The exact player exited normally. The executable SHA-256 was
`fb2f0399fc78a493de28c00cca7c080ad590f1da618a2e97ef5b8c3ef77203d6`.
An isolated Windows launch verified the installed and linked development
entries could start together. Native confirmation and menu input on Windows
and macOS remain open.

### Plugin information and help 2026-11-23

The full Windows workspace gate passed, including strict Clippy, 553 app
tests, 271 core tests, 57 widget tests, the software pixel comparison and
Settings navigation. The manifest regression covers bundled documentation,
missing and out-of-package paths, and validated web URLs. All 58 pinned
libmpv null-output tests and strict Apple Silicon cross-target Clippy passed.

Linux also passed strict workspace Clippy, 520 app tests, 259 core tests and
54 widget tests. The isolated X11 session opened plugin information during
paused playback. Help rendered bundled CSS, followed a relative link, and
loaded an HTTP document from a loopback server. Neither page exposed
`window.iina`. Closing Settings also closed its Help window, and the player
exited normally. The executable SHA-256 was
`53c30d8468b39bec0876e67add1ca4ecbaa1c58178fc44726f1d4c5802e590c0`.
Native Windows and macOS Help windows and live third-party repository updates
remain unverified.

### Playlist row reordering 2026-11-23

The pinned IINA MiniPlayer hosts the main window's playlist view. Riina's
sidebar and Music Mode now share row drag handling, insertion geometry and
edge scrolling while retaining their own visual presentations.

The full Windows workspace gate passed, including 553 app tests, 59 widget
tests and the software pixel comparison. The two drag regressions exercise
scrolling in both directions with a simulated clock, then verify release and
pointer cancellation. All 58 pinned libmpv null-output tests passed.

The locked Linux build opened 70 sibling video entries on isolated X11 and
kept playback paused. A stationary edge drag moved the first sidebar entry
to row 33 and the first Music Mode entry to row 16. Both moves occurred on
release and preserved the other entries' order, as observed through the
player's local mpv socket. The test instance exited normally. Its executable
SHA-256 was `75830f3b97c0e19ad52b53928821c5258f71f8f88bdbee68e68fdb8916c63ebb`.
Native Windows, Wayland and macOS drag interaction remain unverified.

### Desktop file drops into a scrolling playlist 2026-11-23

The Windows workspace gate passed, including 553 app tests, 61 widget tests
and the software pixel comparison. Both drop-hover regressions advance a
simulated clock, check forward and reverse scrolling, and verify the cue clears
on exit. They also check the insertion slot for an empty playlist. All 58
pinned libmpv null-output tests passed.

The locked Linux build started with 70 entries and paused playback. A native
Xdnd source inserted two files into the scrolled sidebar at row 30 and one
into Music Mode at row 16. An edge dwell revealed the closed sidebar, which
accepted another file at row 31. Cancelling a separate Xdnd drag preserved
the playlist. A GTK Wayland drag source inserted two files at sidebar row 27
and one at Music Mode row 16. Each drop preserved the relative order of the
existing entries, and each isolated player exited normally. The executable
SHA-256 was `6a8f1cabd837410e102bb729b14d561721d50a402328deb57973461007655767`.
Live Windows and macOS desktop drop interaction for this change remains open.

### Playlist navigation resume timing 2026-11-23

The full Windows workspace gate and all 58 pinned libmpv null-output tests
passed. Worker tests cover outdated replies and restarts, redirects and failed
navigation. The native playback test checks the restart event in the first
running snapshot for both video and audio-only entries. It also verifies that
the pause-on-open preference keeps a manually selected entry paused.

The locked Linux build ran on isolated X11 with shared OpenGL output. A
playlist double-click reloaded a video after seeking into it. The playlist
shortcut then advanced to another video and an audio-only file. For all three,
the mpv socket reported start, file loaded, playback restart, and unpause in
that order. The player exited normally. The executable SHA-256 was
`213827ee4a7510916f7e98819fd70cbd66d8df9ca29e128029219a4bf8977003`.
Live Windows, Wayland and macOS presentation timing remains unverified.

### Folder and playlist window routing 2026-11-23

The full Windows workspace gate and all 58 pinned libmpv null-output tests
passed. The application tests cover folder and standalone playlist routing,
duplicate preferences, and grouped opens that retain a single playable entry.

The locked Linux application build ran in isolated X11 with libmpv and shared
OpenGL output. One desktop drop containing a folder and an M3U opened two
additional players in the existing process. The folder's playlist showed both
clips in natural filename order. The M3U player showed its two relative entries
in playlist order. After grouping was enabled, dropping two clips opened one
additional player with both clips. The original player retained its media.
Closing all four windows ended the exact test process normally. The executable
SHA-256 was `a84f7cffae341a8f0ed87aea74379f7b560de2ebc84753a9dfc70b292caf53f0`.
Live multi-window routing on Windows, Wayland and macOS remains unverified.

### Playback History destinations 2026-11-23

The full Windows workspace gate and all 58 pinned libmpv null-output tests
passed. The application tests cover selection preparation for both History
actions. Native tests confirm that the selected playlist survives a pending
folder scan, an append and manual navigation, and that opening regular media
restores the normal discovery policy.

On isolated Linux X11, Return played a selected row in the existing window.
Ctrl+A selected both history rows and Ctrl+Return opened them in one additional
window, even with grouping and duplicate players disabled. The two playlists
contained only the selected entries with same-folder discovery enabled. Closing
History and both players in sequence ended the test process normally. The X11
executable SHA-256 was
`c9e63fbb02184f50a099c76622d9f773a216bfe092cb0f5d321b511da6e7e414`.
The queued-scan guard was verified separately in the Windows native suite.
Live Windows, Wayland and macOS History input remains unverified.

### Playback History outline and file actions 2026-11-24

The Windows workspace gate and all 59 pinned libmpv null-output tests passed.
The grouped regressions cover older history archives, metadata that arrives after
the first playback snapshot, path search on decoded file URLs, and keyboard
range selection across reordered groups. The locked Apple Silicon workspace
passed strict Clippy from Windows.

In isolated Linux X11, two clips with the same Unicode metadata title each
saved a titled entry. End and Shift+Up selected an available file and a
missing file on the same date. Ctrl+Backspace moved the available file to
the isolated desktop Trash, retained both records and refreshed the row's
availability. Searching for a literal space in the decoded path reopened
the matching group. Delete then removed only the retained record. The test
used a scripted confirmation response and verified normal shutdown. The
Linux executable SHA-256 was
`44865439c19b35a9af2de5147691b92cbe1346df3e2d265e77188995e448aefc`.
Live Windows, Wayland and macOS History interaction remains open.

### Application-owned Playback History 2026-11-24

The Windows workspace gate and all 59 pinned libmpv null-output tests passed
for the window-ownership change. The headless test covers one window with
retained search state, focus routing, removing closed player routes, plugin
history updates and starting playback without a registered player. The final
Windows app and UI group passed strict Clippy, the application tests, the
61 widget interaction checks and the player and Settings pixel comparison.

On a private Windows desktop, the File menu from two native players opened
the same History window. Play loaded the selected media into the second
frame. After both frames closed, History opened a new player in the same
process. Closing the replacement player and then History exited normally
with the quit-on-last-window preference both enabled and disabled in separate
candidate runs. Native Windows exits when its final window closes. The final
executable SHA-256 was
`bcd422d61411a19e0d997e50215863632e86a782e1d8583ecaa00154f77d49d9`.

On isolated Linux X11, Ctrl+Shift+H opened History from Welcome. Play showed
the startup player and loaded the selected clip. Two players shared the same
History window and retained its search. Play followed a change of player
focus, fell back to the remaining player after one closed, and created a
fresh player after both closed. Closing the fresh player and then History
exited normally with quit-on-last-window enabled. The Linux executable
SHA-256 was
`ccf8687ccfac0359772e319df03a88bb65a767f3f38e7253b5e1e70e9c04508e`.
The Windows Welcome handoff and live Wayland and macOS behavior remain open.

### Playlist selection actions 2026-11-23

The full Windows workspace gate and all 58 pinned libmpv null-output tests
passed. The playlist engine scenario inserted a row, queued selected rows
around the playing entry, then moved a selection to an observed destination.
It checked stable IDs, the current entry, and preservation of playback
generation and pause. The portable ordering tests verify that only selected
rows move across all insertion slots of small playlists.

In isolated Linux X11, Play Next moved two rows from opposite sides of the
current entry to immediately after it, preserving order and pause. Dragging
both rows moved them together without changing the playing entry. Play in New
Window created one player whose playlist contained only the two selected clips,
with same-folder discovery enabled. The original player kept its playlist and
both windows closed normally. The Linux executable SHA-256 was
`58d4a765c664c2ed2cd4883c92efb1e3aefc3047ec4e7d04a7b34b5d50b2355d`.
The Linux run preceded the final internal command-dispatch refactor. Live
interaction on Windows, Wayland and macOS and the plugin menu path remain open.

### Playlist file actions 2026-11-23

The full Windows workspace gate and all 58 pinned libmpv null-output tests
passed. The file-action test covers repeated references and partial Trash
failure. The command snapshot checks that the engine skips vanished or changed
entries and removes the current entry after the other completed entries. The
Finder adapter passed source checking and strict Clippy for Apple Silicon.

On isolated Linux X11, Show in File Manager sent both selected file URLs in
one request to a test FileManager1 service. Move to Trash moved two disposable
media files into the test profile's Trash and removed their three selected
playlist entries. The unavailable selected file and the unselected file kept
their rows. Playback advanced to the remaining playable clip and the test
process exited normally. The executable SHA-256 was
`d502cf1a48cbf31be72c678be0317c89920c1a7002246fc9b7a5d3d505215332`.
Live verification remains open for Explorer, Finder, Wayland, Linux desktop
file-manager presentation and the plugin-contributed menu path.

### Playlist durations 2026-11-23

The Windows workspace gate and all 59 pinned libmpv null-output tests passed.
The new native test reads duration from the video and WAV fixtures and checks
cancellation and an unavailable path. The application test covers selected
duration, the complete total, unknown durations and IINA's whole-second rounding.
The default-on prefetch preference is part of the existing settings schema.

The locked Linux build on isolated X11 displayed `0:05`, `0:05` and `0:02`
for two video clips and one WAV. The sidebar showed `0:12 in total` and
changed to `0:07 of 0:12 selected` after selecting two rows. An unavailable
fourth entry left existing row durations visible and cleared the total. Music
Mode showed the durations and selection total again after removing that entry.
The first clip stayed paused at 1.5 seconds throughout, and the test process
exited normally. The executable SHA-256 was
`9af4a3fb6097391c35875f98e145aa6212771c0addc9f1ea768b81ec5c4d1191`.
Live Windows, Wayland and macOS duration presentation remains open.

### Playlist subtitle associations 2026-11-23

The Windows workspace gate and all 59 pinned libmpv null-output tests passed.
The grouped regressions cover multi-row clearing, duplicate manual paths,
deferred loading for the current entry, expired scans and picker responses
after Stop, and playback-generation checks. The discovery fixtures also cover
exact sidecars in the media directory with the default child-directory glob.

After the compact-menu change, the grouped Windows check passed for the app,
core and UI crates, including the widget and pixel tests. The locked Apple
Silicon workspace also passed strict Clippy from Windows.

In isolated Linux X11, a test XDG FileChooser service received the player's
parent window, the media directory and the multiple-file filter. It returned
two external subtitles, including a Unicode filename. Both loaded when the
queued clip started and the first was selected. Wrong Subtitle cleared the
two selected entries' assignments for their next playback. That covered
manual files and an automatically matched sidecar. The compact menu kept
Clear Playlist accessible, and the test player exited normally. The Linux
executable SHA-256 was
`f56b8e03f17db30c03fc4b935fd2ea8441cc5b34fc085336aef385d37d88ed1b`.
Desktop file chooser interaction on Windows, Wayland and macOS remains open.
At the time of this check, automatic matching ran as each entry started.

### Playlist resume positions 2026-11-24

The Windows workspace gate and all 59 pinned libmpv null-output tests passed.
The row projection and saved-position regressions cover media identity across
reordering, changed durations, missing duration, and the engine's
filename-hashing policy.
The pinned Windows DLL and the Linux test engine both wrote a native
watch-later file with an uppercase hash for a Unicode media path.
The locked Apple Silicon workspace passed strict Clippy from Windows.

In isolated Linux X11, libmpv saved a 2.5-second position for a five-second
clip. Its 3-pixel marker covered 168 pixels of the 336-pixel sidebar row and
followed the clip when it moved below an unwatched entry. Music Mode showed
the same fraction across 150 pixels of a 300-pixel row. The first clip
stayed paused while the playlist changed. Returning to the player and
selecting the saved clip restored exactly 2.5 seconds, and the process
exited normally. The Linux executable SHA-256 was
`a3657921080736d098e4319df65fa5c909c68a546e8a36b65fa319fdc6a6d220`.
Live Windows, Wayland and macOS presentation remains open.

### Mixed playlist context menus 2026-11-24

The Windows workspace gate passed with the mixed-selection, stale-target
and partial browser-launch regressions. The widget checks also exercise
row and blank-area requests in the main playlist and Music Mode.
The locked Apple Silicon workspace passed strict Clippy from Windows.

In isolated Linux X11, right-clicking a selected local file offered URL
and local-file actions for a mixed selection of two files and three
streams. Copy URLs returned all three URLs, including RTSP, in playlist
order. The test desktop launcher received both web URLs. The GTK menu
and the in-window fallback both passed these checks. At 300 × 365, the
fallback scrolled a full row menu in Music Mode to reach Clear Playlist.
The action kept the playing local entry and its paused state. Both test
players exited normally. The Linux executable SHA-256 was
`7314d6c419a0926973e0c68d5807fef253e9283703736f2e47c1db22e7be331b`.
Live Win32, Wayland and macOS menu interaction remains open for this batch.

### Playlist series titles 2026-11-24

The Windows workspace gate and all 59 pinned libmpv null-output tests passed.
The workspace tests cover Unicode sibling detection, discovery with folder
auto-add off, and persistence of expanded titles through row projection.
The locked Apple Silicon workspace passed strict Clippy from Windows.

In isolated Linux X11, opening the second of three `Café Stories` episodes
added the neighboring episodes and an unrelated clip in filename order.
The unrelated clip kept its full name. The first episode expanded in the
main sidebar, kept its expanded title after reordering, and folded again in
Music Mode. The second episode stayed paused while the playlist changed.
The test player exited normally. The Linux executable SHA-256 was
`12dcc777127863bfab11afff755e6c9a105b7db6ac5e9b709b32235c328ed216`.
Live Windows, Wayland and macOS interaction remains open for this batch.

### Queued subtitle matches 2026-11-24

The Windows workspace gate passed, including Unicode matching across a folder,
playlist replacement, and clearing manual and automatic matches for selected
rows. All seven tests in the pinned libmpv subtitle group passed. The locked
Apple Silicon workspace passed strict Clippy from Windows.

In isolated Linux X11, opening one clip added three neighbors. Two queued
clips each showed one matched subtitle before playing. The engine held only
the active clip's external track. Moving to the third clip used its cached
match and skipped another sidecar written after the first scan. Wrong Subtitle
cleared the second clip's match before it played and preserved the paused
playlist. The test player exited normally. The Linux executable SHA-256 was
`cf1852260188fff1a6ba7ef0612e1a999f02925445d7246a121ffe4648dcfd26`.
Live Windows, Wayland and macOS interaction remains open for this batch.

### macOS application menu composition 2026-10-14

The AppKit adapter consolidates application actions in one native menu while
retaining Services and visibility actions. A focused Apple-target probe compiled
all eleven macOS platform modules with strict Clippy. The full application check
from Windows stopped in a native dependency before application compilation.
Native menu interaction, exact order/shortcuts and Mac playback remain unverified.

### macOS Picture-in-Picture session 2026-11-21

An isolated `aarch64-apple-darwin` source check and strict Clippy run compiled
the AVKit adapter, frame bridge, and platform session against the pinned
Objective-C dependencies. The probe used stub Slint controls. Windows passed
the full workspace gate after moving the portable PiP bindings. Full macOS
application linking and AVKit playback still need native verification.

The Settings color-panel binding also passed strict Clippy for
`aarch64-apple-darwin` against the real AppKit color panel and stub Slint
controls. Native interaction with the panel remains unverified.

The separated GL rendering notifier passed strict Clippy for
`aarch64-apple-darwin` against the real mpv surface and AVKit renderer. This
probe supplied stub Slint and display-profile access. Render timing and active
display color behavior still need native macOS verification.

### macOS Now Playing session 2026-11-21

Strict Clippy passed for `aarch64-apple-darwin` on the session's playback and
focus policy alongside the real MediaPlayer adapter. The probe supplied stub
Slint controls and focus access. Native media-key interaction and playback
resume across application focus changes still need macOS verification.

At `d8421ed`, strict `aarch64-apple-darwin` Clippy covered the actual generated
Slint UI, the real AVKit and MediaPlayer adapters and both media sessions. The
isolated crate supplied minimal focus, key-binding and presentation adapters.
It verified that the session code type-checks against the current UI and engine
error types. The full application check from Windows still stopped in
`objc2-exception-helper` because the Apple Objective-C compiler was unavailable.
Full application linking and runtime verification require macOS.

### macOS input and locale adapters 2026-11-23

At `63c10a6`, strict `aarch64-apple-darwin` Clippy passed in an isolated
crate with the current wheel and locale adapters, the generated player UI,
AppKit gesture and scroll adapters, and Foundation locale formatting. The
probe supplied default control preferences. In a separate full-workspace
source check, the Objective-C exception helper's documentation mode let
compilation continue to `ring`, which still required an Apple-target C
compiler. Full native application and playback verification remain open.

At `a6e9da3`, the entire locked workspace passed
`cargo clippy --workspace --all-targets --locked --offline --target aarch64-apple-darwin -- -D warnings`
from Windows. The cross-check used the verified Zig 0.15.2 release for C
dependencies, with `CC_aarch64_apple_darwin` pointing to
`zig cc -target aarch64-macos.11.0` and `AR_aarch64_apple_darwin` to
`zig ar`. `CRATE_CC_NO_DEFAULTS=1` avoided cc-rs passing a second target,
and `CFLAGS_aarch64_apple_darwin="-O1 -fPIC"` supplied the C build flags.
The check used the normal Objective-C exception helper and compiled the
application and test sources. Apple framework linking, test execution and
desktop playback still need a native macOS verification run.

At `4f7ee4e`, strict workspace Clippy passed with both
`aarch64-apple-darwin` and `x86_64-apple-darwin` using Rust 1.99.0
and the same Zig toolchain. For Intel, the C compiler target was
`x86_64-macos.10.12` and the C compiler and archive-tool settings used the
`x86_64_apple_darwin` suffix. Both source checks include the platform startup
adapter and application menu observer. Both still require execution on macOS.

### Playback keyboard workflows and History search 2026-11-24

History search folds case and accents and searches decoded local or network paths
in Full Path mode. It stores a bounded list of submitted searches per profile.
Both playlist views share keyboard navigation, range selection and row reveal.
Audio, Video and Subtitles reveal focused controls in their own scroll areas.
Playlist export publishes the completed UTF-8 file using atomic replacement.

The Windows workspace gate passed 951 tests, including 63 headless interaction
tests and the software-rendered pixel comparison. The headless checks cover
playlist range selection and paging in both surfaces and focus-driven Audio
scrolling across a pane round trip. All 78 Python tests and 59 native engine
tests passed.
The Windows private-desktop export probe stopped at the Save As confirmation.
End-to-end export through the Windows dialog remains open.

The full Linux application built against the locked dependencies. An isolated
X11 session submitted and saved searches, recalled a recent search, opened an
accented filename, cleared recents and closed the exact application process.
The complete workspace passed strict Apple Silicon cross-Clippy. Native macOS
execution and matched reference pixels for the updated History toolbar remain open.

### Native playlist export and Music Mode handoff 2026-11-24

Windows Save Current Playlist uses the same window owner and modal lifecycle as
Open File. The export worker reads one ordered playlist snapshot after the
destination is confirmed. It writes the UTF-8 file atomically and serializes
exports from the same player. The Windows private desktop verified cancellation,
reopening and export of a local Unicode media path. A workspace test exercised
replacement of an existing file and rejection of an invalid entry.

A manual Music Mode request checks whether the native owner can leave fullscreen.
It waits for an asynchronous fullscreen exit before resizing the compact player.
The compact window records the normal player's placement and reads the live
window size after exiting fullscreen. The open-in-fullscreen preference accounts
for the current Music Mode state. Headless tests cover rejected and accepted
fullscreen transitions and subsequent automatic switching.

The final Windows private-desktop session switched a paused video from an
880 × 500 frame into 1280 × 720 fullscreen, entered Music Mode with the player
shortcut and returned to its original bounds and paused state. It then
cancelled and completed playlist export and closed the tested process.

The Windows workspace gate passed 954 tests and all 78 Python tests. The pinned
native engine suite passed 59 tests. An isolated Linux X11 session returned from
Music Mode to the same player, media and window size, then exited cleanly.
Apple Silicon workspace cross-Clippy passed. Native macOS testing is still open.

### Confirmed fullscreen transitions 2026-11-24

The Windows workspace gate passed 955 tests and all 78 Python tests. All 59
pinned native engine tests passed. On the Windows private desktop, a paused
video switched from an 880 × 500 frame to 1280 × 720 fullscreen, entered Music
Mode and returned to its original bounds with playback paused. The tested
process closed normally. Apple Silicon workspace cross-Clippy passed.

The locked Linux x64 application built and ran on Ubuntu 26.04.1 LTS under
WSL2 kernel 6.18.58.1. In separate Openbox/X11 and Sway/Wayland sessions,
desktop-initiated fullscreen resumed a paused fixture, Escape left fullscreen
and paused playback, and the player returned from Music Mode with the same
paused media. The Wayland test also checked full-output video samples during
the transitions and confirmed that closing its window exited the process.
The Wayland Music Mode test used a remapped key. The default modifier chord
and toolbar activation still need verification on a native Wayland desktop.
Native macOS fullscreen and the full application remain unverified.

- Linux application SHA-256 `225b58817a071391654aa0555cd277ccd612775c0ce7ffb911fe4b7190ee9254`
- X11 libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`

### Console launcher and input detection 2026-11-24

The companion `riina-cli` executable displays help and argument errors
in the terminal. It returns after a normal launch and supervises the player
for `--keep-running` or standard input. The player and launcher recognize
buffered pipes and unread redirected files. Explicit input switches override
that probe, and the launcher forwards its decision to the GUI process.

A locked Linux application build ran seven isolated Wayland scenarios.
The direct player and the launcher each decoded a redirected audio file.
The launcher decoded an already buffered pipe and an explicitly selected
pipe fed after startup. `--no-stdin` opened a relative video filename and
left the redirected file offset unchanged. With an empty input pipe the
launcher exited while the paused video remained visible. Interrupting
`--keep-running` returned a failure status and stopped its own player.
The ordinary close path returned success and every exact test player exited.

The same seven cases passed with the staged Windows binaries on an
isolated desktop. The interruption case sent a Windows Ctrl+C event to the
test launcher’s console and verified that its own player exited. PE headers
confirmed a console launcher and a windowed player. The Windows workspace
gate, locked build and all 60 native engine tests passed. Strict Apple
Silicon cross-Clippy and the bundle layout regression passed. Native macOS
launcher behavior remains unverified.

- Windows application SHA-256 `3b8677b4fa2adda74a76109997b02a45b346409ba5fd619e33e92d0fee47f7b6`
- Windows launcher SHA-256 `cc6885b86872aec0e91b87eb4a065c9b11109db7d5cff434ea6b74937d253400`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`
- Wayland application SHA-256 `d9161421dd69adf3f5a99492bdbf78fbe64143713d106c3f2022d0035ab76a1b`
- Wayland launcher SHA-256 `9459a4bacf333b4b685ffff61dfc81b02bffcdff3c400b6edd1a6285e6c2020a`
- Wayland libmpv SHA-256 `cdce854e1811942602587310026306d92d30c0f593f2be676f07659ae43061d8`
- Wayland libmpv SHA-256 `cdce854e1811942602587310026306d92d30c0f593f2be676f07659ae43061d8`

### Plugin presentation events and Wayland Music Mode 2026-11-24

The Windows workspace gate passed 955 tests, including the player and Settings
pixel comparison, and all 78 Python tests. The 59 pinned native-engine tests
passed. The plugin host regression exercises repeated and rapid mode changes,
the matching state read from each callback's Promise jobs, and pending event
delivery before window close.

The Windows private desktop verified the ordered fullscreen, Music Mode and
Picture-in-Picture entry and exit events against a real player plugin. The
player returned to its paused video and original bounds after Music Mode.
The test closed the separate PiP window and observed the exit event. After
the player menu returned, it cancelled and completed a Unicode playlist
export before closing the exact process. PiP video pixels were outside this
check.

An isolated 1280 × 800 Sway session on Ubuntu 26.04.1 verified fullscreen
playback preferences, the default Ctrl+Alt+M shortcut, the fullscreen Music
Mode toolbar action, and the compact Return to player control. Both routes
restored the same paused media and player bounds. Full-output fixtures
confirmed video pixels, and a plugin recorded all ten expected transitions
with matching state. Compositor close exited the tested process.

Strict Apple Silicon workspace cross-Clippy passed. The AVKit delegate's
started, stopped, and failed-start paths still require native macOS execution.

- Windows application SHA-256 `14a37312bf8d1d259f3a3f880d81155312ae330ac493319980c64ed66bf11c29`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`
- Linux application SHA-256 `1a285aed3aed5db8645eb0b37b9b3fa6cc237c0ce10f578ceb66ab340e9e3453`
- Wayland libmpv SHA-256 `cdce854e1811942602587310026306d92d30c0f593f2be676f07659ae43061d8`

### macOS Picture-in-Picture window lifecycle 2026-11-24

The macOS source applies the configured window action and pause preference
after AVKit confirms PiP entry. It binds AppKit minimize and restore events to
the owning player. The rendering timer borrows the same libmpv surface while
the window is occluded, hidden, or minimized. It uses the context created for
the player renderer and restores the previous context after drawing. AVKit
availability, frame delivery, and window lifecycle on an actual Mac remain open.

Strict Apple Silicon workspace cross-Clippy passed, including compilation of
the new macOS frame-source regression test. That test still needs execution
on macOS. The Windows workspace gate passed 955 tests and 78 Python tests.
The pinned native engine suite passed all 59 tests.

The locked Linux x64 build passed a private Sway playback sweep with pixel
samples, fullscreen preferences, Music Mode shortcut and toolbar input, and
ordered plugin callbacks. The process exited when its compositor window
closed. On the Windows private desktop, the native player returned from
fullscreen and Music Mode, emitted the PiP start and stop events, and exited
normally. The Windows Save As probe timed out immediately after a PiP close
in one run. A subsequent run with a 500 ms settle interval cancelled and
completed Unicode playlist export. Immediate dialog availability remains open.

- Windows application SHA-256 `cb38029878851b0836334d51349c9ecb728c5445dc86c31d968d26538dde1175`
- Linux application SHA-256 `b7cf8f3b1dc8eff0d14c9bf5232a2b487608ad7154da47843ba8147d32164af8`

### Windows Picture-in-Picture and native picker ownership 2026-11-24

A private-desktop baseline intermittently failed on the first Save As action
after Picture-in-Picture. The candidate used the framed player as the native
owner of both Save As and Open File. With the default Hide policy and with
the main window left open, the driver opened Save As immediately after
returning from PiP, cancelled and reopened the picker, exported a Unicode
playlist, cancelled Open File, and repeated four Save As cancellations.

The player kept new window bounds when moved during Picture-in-Picture and
restored entry bounds under the Hide policy. A native minimize opened PiP.
Closing the preview while the player stayed minimized left it closed.
Another minimize after restore opened a new preview, and restoring the
player closed it. These checks covered the window lifecycle and plugin
events. Video motion in the minimized preview remains open. The test
also closed the exact application process while the default Hide policy
had the player parked and the preview open.

The Windows workspace gate passed 955 tests and the 78 Python tests.
All 59 pinned engine tests passed. The locked Linux application built and
passed the private Sway playback, fullscreen, and Music Mode sweep.
Apple Silicon workspace cross-Clippy passed.

- Windows application SHA-256 `a25a9e1b5de0dd3f883ac4742d26cf6d2f856a1c56c73b3a4fc5f6e4f83d41fc`
- Linux application SHA-256 `a17d7b50b2a9db59e2163be874453a063b3628186238f72c1369bfe9d74465c1`

### Linux Picture-in-Picture preview 2026-11-24

The locked x64 application built on Ubuntu 26.04.1. Strict Linux workspace
Clippy and all 538 application unit tests passed. The preview preserves video
orientation and uses the existing playback engine. In a private X11 session
with Xvfb, Openbox and Mesa llvmpipe, the default shortcut opened a preview
above other windows and hid the main player. The test captured moving frames
while the main window was hidden. The preview controls paused, resumed,
paused via Stop, and sought back while remaining paused. Closing the preview
restored the original window with the same media and pause state. With
automatic PiP enabled, a native minimize opened the preview and a native
restore closed it, with no repeat transition. A test plugin observed the
confirmed state in all four callbacks.

A private 1280 × 800 Sway session with no XWayland used compositor-managed
floating windows. The preview showed moving frames, paused and resumed
playback, paused via Stop, and updated after a paused seek. The original
player stayed open and kept its media, bounds, and pause state when the
preview closed. The plugin received one confirmed entry and one confirmed
exit in order. Both backends also reopened PiP and shut down cleanly when
the main window received a native close request with the preview active.
The X11 main window was hidden during that shutdown check.

Adding the shared preview image property preserved the Windows native window
workflow in a private desktop. The player restored its bounds after the
preview closed, opened its native Save As and Open File dialogs immediately,
exported a Unicode playlist, and exited while PiP was active with the main
window hidden. The Windows workspace gate passed 955 Rust and 78 Python tests.
The pinned engine suite passed 59 tests, and the Apple Silicon workspace
passed strict cross-Clippy.

- Windows application SHA-256 `41fee9a1d1eb869a683a586c4369108b83e91581c1dafb79cc725cb25c90be16`
- Linux application SHA-256 `f29ff3c69845b50ba1c8731efccd69a4a4bbbacebaf81b37ab17e14f9c1eade5`
- X11 libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`
- Wayland libmpv SHA-256 `cdce854e1811942602587310026306d92d30c0f593f2be676f07659ae43061d8`

### Compact playback controls and idle feedback 2026-11-25

The compact controller keeps its title and timeline when the main controller's
idle timer runs. Its transport appears when the pointer is over artwork or the
controls, and keyboard focus can reveal the corresponding actions. Wheel input
on the control strip and catalog stays in those surfaces. On Linux the compact
menu button and F10 open the player menu without reserving a separate strip.

The supplied REF-22 resting frame is 600 × 518 pixels. The timeline center is
about 73 points below the artwork at the inferred 2× scale. The Windows software
rendering comparisons cover resting, transport and Chapters in light and dark
themes. The 80 grouped widget tests include idle timeline placement, first-click
transport and wheel routing. The application tests exercise the compact menu
with F10 and pointer activation.

The full Windows gate passed formatting, strict Clippy, 1,078 Rust tests,
79 Python tests and four browser-extension tests. The workspace gate left 78
Rust tests ignored, including the native integration suite. All 65 native
tests passed separately with the pinned null-output libmpv.

The locked Windows workspace build opened the REF-22 GIF on a private Windows
11 desktop. It decoded 960 × 540 video, settled at a 300 × 259-point client and
kept the playlist closed. The first pointer click after 3.5 seconds at idle
resumed playback. A second click paused it, and the exact test process exited
normally. The desktop capture returned an invalid handle, leaving Windows
compositor pixels unverified for this run.

The locked Linux x64 build preserved paused playback through the X11 resize,
catalog and Music Mode handoffs. The original 850 × 580 client returned after
the second exit from Music Mode, and the exact X11 process exited normally.

In a private 1280 × 800 Sway session, the 300 × 410 compact client kept its
title and clocks visible with the catalog open. Native pointer events revealed
transport over the control strip and restored the title over the catalog.
Both the ellipsis and F10 opened the player menu, and Escape closed it. The
main client returned to 850 × 580 with the same paused media. The exact
Wayland process exited normally.

A second 1280 × 800 Sway capture used the REF-22 GIF at 1.25 seconds in a
300 × 259 client. The GIF artwork and title were visible with the catalog
closed and the pointer outside the window. The timeline center was 72.5 points
below the artwork, compared with 72.25 at the inferred reference scale.
The Wayland title spans about 171 pixels at 1×, compared with about 153
points in the reference. Window corners and strip materials also differ.
The two end clocks display different durations, so the comparison establishes
placement and leaves time-display parity open. The test process exited normally.

Strict Apple Silicon cross-Clippy passed for the application and test targets.
Native macOS playback and window behavior remain open.

- Windows application SHA-256 `c3913742a36c3ac0b3781079c36d8b6dbe59dcc293f4012dfa728c9e0164150a`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`
- Linux application SHA-256 `dcd99112d356f0eed9d994456a860d9c97deb2bcfaa44bc8a61c8364b5276d3d`
- X11 libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`
- Wayland libmpv SHA-256 `cdce854e1811942602587310026306d92d30c0f593f2be676f07659ae43061d8`

### Quick Settings timing controls 2026-11-24

The IINA v1.5.0 Subtitle pane previews the delay on its indicator and input
field during a drag. It uses half-second ticks and applies the selected lane
on release. The pinned capture shows the indicator and ticks at zero delay
with the reset action hidden. [Reference REF-25](visual-reference-audit.md)

In the X11 baseline, libmpv reported a subtitle delay of −0.645 seconds while
the pointer was held. The candidate kept the native delay at zero through
each held drag. Both subtitle lanes showed their draft and committed 2 seconds
on release. Audio committed 2.1 seconds using its existing 50 ms increments.
The primary delay stayed unchanged while editing the secondary delay. Playback
kept the isolated fixture open and paused, and the exact process exited.

The same binary and fixture passed the three held-drag checks on native
Wayland in a private Sway session with XWayland disabled. That run resized
the window before entering fullscreen. On a private
Windows desktop, UI Automation applied values to both subtitle lanes and the
audio slider. UI Automation also populated the custom speed field. Focus and
Return submitted the edit, and libmpv reported 1.25×. The exact Windows
process exited normally.

The Windows full gate passed, including the grouped widget tests for both
subtitle lanes, drag cancellation, keyboard focus and accessibility edits.
Only the two Subtitles player frames changed in the headless pixel baselines.
The light and dark differences were limited to the delay controls. All 78
Python and 59 pinned engine tests passed. Apple Silicon cross-Clippy passed.
Linux strict workspace Clippy, 538 application tests and 63 widget tests passed.

- Windows application SHA-256 `d93a3e69ded787a69b09ee73a6ceec9be38afa5cbc64c90c215210f639c5c292`
- Linux application SHA-256 `63b676b4d41506861b7a20abad3935200d118d046acb773d2b73b36f46bd0d10`
- X11 libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`
- Wayland libmpv SHA-256 `cdce854e1811942602587310026306d92d30c0f593f2be676f07659ae43061d8`

### Paused fullscreen rendering and startup resize on Wayland 2026-11-24

In the unmodified `dce6ff4` run, an external fullscreen request arrived
during the first resize of a paused video. Sway announced a 1280 × 800
fullscreen surface, then announced 320 × 180 while keeping it fullscreen.
The captured video still occupied the small surface. Sway 1.11's floating
toplevel handling uses committed client geometry, and another fullscreen
request arranges the output again. [Sway surface handling](https://github.com/swaywm/sway/blob/1.11/sway/desktop/xdg_shell.c#L278-L374)

The first recovery attempt requested fullscreen before the undersized frame
reached the compositor. That frame arrived afterward and shrank the surface
again. The final change waits for the redraw before requesting fullscreen once
for the current output mode. A confirmed fullscreen transition also redraws a
paused frame and closes an open crop or delogo editor before applying the
configured play/pause preference. A shared test checks this ordering and
ensures the confirmed-window event fires once.

In a private Sway 1.11 session on Ubuntu 26.04.1 x64, a test-only compositor
fixture replaced the first fullscreen configure with 320 × 180. It left the
baseline at that size. The candidate committed the small frame first, then
sent one additional fullscreen request. The next configure restored 1280 × 800,
and a full-output capture showed the paused frame at the restored width. The
fixture ran in the compositor process, and the app and engine were unmodified
binaries. With and without the fixture, app and desktop fullscreen entry and
exit preserved the same media and paused position. Each run reported four
confirmed fullscreen events across two complete transitions and exited cleanly.
The same binary also passed the entry, exit, paused frame and shutdown checks
in a private X11 session. Separate X11 runs checked native folder and playlist
drops, grouping several opens into one player, and reopening a multi-selection
from Playback History in the current and a new player. All test processes
exited. The locked Linux application build and strict workspace Clippy passed.
The application harness passed 540 tests and the widget harness passed 63.

The Windows workspace gate and its 78 Python tests passed. The pinned
Windows engine suite passed 59 tests against the same engine used for a
private-desktop smoke test of playback, controls and shutdown. Strict Apple
Silicon cross-Clippy passed. Native macOS playback remains unverified.

- Windows application SHA-256 `3c9639fc04f93e08ccf73866057f258b32c26796908cd1bf6e6887971b6136e9`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`
- Linux application SHA-256 `19b0a509b917d152f582f2eb53c2a28eeab8a237ee80dd59be5ca5144b11fee1`
- X11 libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`
- Wayland libmpv SHA-256 `cdce854e1811942602587310026306d92d30c0f593f2be676f07659ae43061d8`

### Playback History confirmations 2026-11-24

IINA presents clear, remove and trash confirmation from its History window.
Riina's confirmation appears inside the same window. It keeps one pending
action, starts with Cancel focused and discards the action on Escape or
window close. A headless regression exercises Return on Cancel, Tab to the
confirmation button, and delivery of a second answer after the action completes.

On a private Windows desktop, UI Automation found the confirmation inside
Playback History. Cancel and closing the window both preserved the isolated
history. Reopening reused the window. Confirming Clear updated the saved
archive while playback stayed paused. The exact test process exited normally.
The grouped Windows application and UI suite, including the pixel baselines,
passed.

Private X11 and native Wayland sessions exercised all three actions with
temporary data. Cancellation and closing the window preserved the saved
entries and the file. A confirmed trash operation moved the temporary file
while preserving its entry. Remove affected only the selected entry, and
Clear emptied the saved archive. The Linux application harness passed 541
tests and the widget harness passed 63. Apple Silicon cross-Clippy passed
before the confirmation panel spacing adjustment. The final native Wayland
run repeated all three actions. Its full-output capture confirmed both
buttons stayed inside the panel.

- Windows application SHA-256 `9cc4e7c1a58d953873bab17563d2c1e01943756cfbc09bc725fdfdd4e5fe7ad7`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`
- Linux application SHA-256 for the X11 run `d9c56747d7a063f23cbeaef07406c7ec354fa7454a7ee78a4f2e5717edeeaec3`
- Linux application SHA-256 for the final Wayland run `3023b1b96fea6b3a1eadd5ffea39d8048c47c3f1a850894a7186763d19d3cf86`

### Windows system media session 2026-11-24

Windows uses the player's top-level window to register a system media
session. The session sends title and timeline state and routes play, pause,
stop, playlist navigation, skip and position requests through the player.
Button requests use the active input profile when it provides a mapping.
[Microsoft's top-level ownership contract](https://learn.microsoft.com/en-us/windows/win32/api/systemmediatransportcontrolsinterop/nf-systemmediatransportcontrolsinterop-isystemmediatransportcontrolsinterop-getforwindow)

On a private Windows desktop, the native registration test used the
application's Windows adapter with an owned HWND. WinRT returned the expected
title and paused state, then disabled and cleared the registration on drop.
The Riina session and an independently registered WinRT fixture were both
absent from the private desktop's global media-session list. Shell and
hardware media-key interaction remain open. The combined Windows workspace
gate and private-desktop playback, controls and shutdown smoke test passed.

- Windows application SHA-256 `7c49f5de7a226ef291d1f5ffabe65d03f8d5bf10799cc34348d10eb70f8696bd`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`

### Linux system media controls 2026-11-24

Private X11 and native Wayland sessions used the actual MPRIS interface and
pinned libmpv. The runs checked title and track identity, play, pause, seek,
rejection of a stale track ID, the default 100% volume limit, and the native
position and volume after a request. The bus monitor observed playback
property changes and the `Seeked` signal. With multiple windows disabled,
`OpenUri` switched media in the current player. With multiple windows enabled,
Wayland opened the new player and published its track even on a headless seat
without a keyboard. Quit withdrew the service and closed every test window
and exact player process.

The Linux candidate passed strict workspace Clippy, 542 application tests and
63 widget tests before the focus fallback adjustment. The locked application
build and the three desktop runs passed after that adjustment. The Windows
workspace also passed Apple Silicon cross-Clippy. Native macOS runtime
verification remains open.

- Linux application SHA-256 `4cbf64d69a6afd0ea49b5c8710cb2e9e3a31de3bcf2d988d2b14db36f1b600b5`
- X11 libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`
- Wayland libmpv SHA-256 `cdce854e1811942602587310026306d92d30c0f593f2be676f07659ae43061d8`

### Manual software update check 2026-11-24

The Windows native and shared application menus open one Software Update
window. The check reads the project's public stable GitHub release on a
background thread and compares it with the installed version using SemVer.
An available release offers the official release page. Automatic checks,
prerelease selection and installation remain open.

The private Windows desktop used the live release feed. Check for Updates
and Check Again both reported installed and published version 4.2.1.
Repeated invocation reused the window. Close dismissed it, and reopening
reused it. Playback stayed paused, and closing the player exited the exact
test process with Software Update open. The full Windows workspace gate
passed, including the pixel baselines and tests for available, current and
failed results.

The locked Linux application build passed. On an isolated X11 desktop, the
shared menu opened Software Update. It showed the request timeout and offered
Check Again. Escape dismissed the window, and closing the player exited the
exact test process with the update window open. Strict Apple Silicon
cross-Clippy passed. Native macOS activation and playback remain open.

- Windows application SHA-256 `a2f86d45336797da3cb886d9d17f0bb9da8d5f834bf48d6eede1a6761b7ba41`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`
- Linux application SHA-256 `0c04ef702447707cf45296a71a556825d9a27d2bd2fc95ace29b87c2ec00d67f`
- X11 libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`

### Shared test cache and worktree fixtures 2026-11-24

A reused native harness failed while looking for the checkout where its test
helper was compiled. Media fixtures and Windows pixel baselines now resolve
from the running test's workspace. The same native executable reported a
missing clip from an alternate checkout directory, then passed once that
directory contained the fixture. It used the same pinned DLL throughout.

The grouped Windows check passed strict Clippy, 576 application tests,
66 widget tests and the headless pixel comparison. The separate native
engine suite passed all 59 tests.

### Automatic update settings 2026-11-24

General settings now store automatic update permission, an hourly, daily,
weekly or monthly interval, and an optional prerelease channel. New profiles
start with automatic checks off, a daily interval and stable releases. An
undecided profile is asked from the second launch. The application records
attempts per channel under the profile lock so a restart can resume the
schedule and another process can detect a check already claimed.

The private Windows desktop ran three launches on one isolated profile. The
first launch stayed quiet. The second asked permission and saved the decline.
General settings then enabled automatic checks, selected a weekly interval
and enabled beta releases. A due check ran without opening Software Update.
The manual menu compared the published release feed and reported version
4.2.1. The third launch retained the settings and the last attempt. The
fixture stayed paused and each exact test process exited normally.

The locked Linux application build passed. On X11 a profile seeded with the
first-launch record showed the permission prompt. Escape persisted the
decline, and the manual menu remained available. An initial synthetic close
left the player running. The repeat returned focus to the player, closed it
during a check and confirmed that the exact process exited. The Wayland
fixture failed during Winit initialization for both this candidate and the
earlier verified History build. Strict Apple Silicon cross-Clippy passed.
Native macOS verification remains open.

The full Windows workspace gate passed. Headless tests exercised release
selection, restart and interval calculation, synchronization between Settings
windows, and silent scheduled failures and current-version results. The
standalone software render matched the updated General and search popup
baselines. All 59 pinned-engine tests passed separately.

The published Windows 4.2.1 installer matched the release API's byte count
and SHA-256. Windows reported the installer as unsigned. In-app package
download and installation remain open.
[Riina 4.2.1 release](https://github.com/deminden/riina/releases/tag/v4.2.1)

- Windows application SHA-256 `1c1f4bb3d95802717493f20e0ef963c929bbc8da49ea47120df92a736640bb78`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`
- Linux application SHA-256 `cef017208017faef1c98ff506e34ed1b703f399fae99f5bc222136feee3e8780`
- X11 libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`

### Verified Windows installer download 2026-11-24

Software Update can offer the exact x64 Setup asset to a registered Windows
installation. It streams the bounded download from the project's release,
checks the published byte count and SHA-256, and marks it as an Internet
download. Download and Open Setup are separate actions. The setup worker
rechecks the registered destination and verifies the file while excluding
writes before handing the installer to the Windows shell. Portable copies
and releases without a matching asset retain the release page action.

The Windows workspace gate verified malformed and oversized downloads,
checksum changes, cancellation, and stale completion handling. The live
test fetched the published 4.2.1 installer, verified it and acquired the
launch-time read lock. An isolated Windows test opened a stand-in through
the same shell adapter while its executable was read-locked. It received
the exact Unicode destination and returned its known exit code. The pinned
Inno Setup compiler accepted the setup mutex and built an installer from
the archived 4.2.1 portable payload.

The three-launch Windows check passed against the live release feed. The
candidate retained update preferences and paused playback, and all three
test processes closed normally. The separate pinned-engine suite passed
all 59 tests. Strict Apple Silicon cross-Clippy and the locked Linux
application source check passed.

An installed version-to-version upgrade and the Windows reputation prompt
remain unverified. The live download test exercises the current unsigned
asset against its published digest.

- Windows application SHA-256 `9fad427141e0c08fe565eca7d133244cb69293d5eca43e7ca65720b598f4c46b`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`

### Native Wayland update consent retest 2026-11-24

The earlier fixture failure came from Slint 1.18.1 selecting X11 when it
detected WSL. A Winit-only probe connected to the Wayland compositor, while
the Slint backend reported that it could not connect to an X server.
The repeat hid the WSL markers inside a private mount namespace.

The previous updater build and the current locked Linux build both opened
Software Update on native Wayland. The current run checked the active
filename, decoded video and pause state through libmpv. A full 1280 × 800
desktop capture showed the paused fixture and the consent prompt. Escape
saved the decline without recording a release check. The video stayed
paused and closing the player exited the exact test process normally.

- Linux application SHA-256 `cd6afdb3c13936494a2e11139a07cc844d8d496de96db5562a4f99b17d0e17c8`
- Wayland libmpv SHA-256 `cdce854e1811942602587310026306d92d30c0f593f2be676f07659ae43061d8`

### Playback idle requests 2026-11-24

General settings now default to preventing display idle during playback.
The audio exception keeps the system awake while allowing display idle for
audio files and Music Mode. One application-owned request follows the first
playing window in creation order. Pausing or closing that window transfers
the request to the next player if playback continues there.

On Windows 11, the isolated player opened paused without requesting power.
Starting and pausing video acquired and released the display request. Both
General settings were operated through the Settings window while video
played. With the audio exception enabled, switching to Music Mode released
the display request and retained system idle prevention. A second process
loaded an audio file with the saved settings and requested only system idle
prevention. Each test process exited normally and cleared its requests.

The locked Linux application build played video and audio in two native
Wayland windows. Private D-Bus test services recorded the logind idle lock,
portal Idle flag and request disposal. Pause released both requests,
resuming reacquired both, and audio requested only the logind lock. The
fixture verified no inherited mpv screen saver request. Both test processes
closed normally and their requests were released.

The Windows workspace gate, Settings pixel comparisons, all 59 pinned
libmpv native tests and strict Apple Silicon cross-Clippy passed. Physical
sleep, legacy Windows screen saver behavior, a real Linux desktop power
service and native macOS runtime remain unverified.

- Windows application SHA-256 `6571abb938c2a19f8cc38cabe9faff500d6a0936e29cb0eaa3891504add4bbc5`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`
- Linux application SHA-256 `e8b67e5e88534dcb08f7713921eed81588763d2a56af5e1b100c2e429fc4a0d0`
- Wayland libmpv SHA-256 `cdce854e1811942602587310026306d92d30c0f593f2be676f07659ae43061d8`

### Command-line playback 2026-11-24

The executable accepts separate-window, stdin, Music Mode, PiP and fullscreen
requests, and launch-scoped mpv options. Separate windows share one application
and each owns its engine. Music Mode and PiP target the last player. The
initial playlist shuffle fills the native list before starting its first item.
The presentation request follows the first ready observation, including an
initial seeking observation, and a newer media selection supersedes it.

An isolated Windows 11 run opened two paused videos in one process and entered
Music Mode in the second window. Closing the first left the second available.
Separate launches verified two grouped entries, decoded video, pause, initial
volume and the raw mpv speed override, then decoded PCM from redirected
standard input. The profile preserved its stored pause policy and advanced
options. All three exact test processes exited normally.

A locked Linux build ran five native Wayland scenarios. The first verified
both windows and the final window's Music Mode geometry. The second verified
a shuffled two-file playlist, pause, decoded video, volume and speed. The
compositor confirmed fullscreen and a separate PiP surface in the next two
runs. The last decoded redirected PCM and reported the standard input path.
Each scenario closed the exact process through its window.

The full Windows workspace gate, all 60 pinned engine tests and strict
Apple Silicon cross-Clippy passed. Native macOS runtime, automatic discovery
of piped input, and browser URL activation remain separate work.

- Windows application SHA-256 `475225620481a75154da1abe9467aa0e3e2199df24dd878c7967d6f23461d358`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`
- Linux application SHA-256 `f0c4418cef0c497ecec35d3ea219e3babe02a8eb8b16089cd49f3b9c3f2545ad`
- Wayland libmpv SHA-256 `cdce854e1811942602587310026306d92d30c0f593f2be676f07659ae43061d8`

### Browser-link activation 2026-11-24

The application accepts the `riina://open` and `riina://weblink` actions
through a dedicated launch argument or a native URL event. Typed playback
options reach the player without entering the ordinary mpv argument parser.
The local handoff tests reject invalid packets and allow a new application
to take ownership after the previous listener closes.

An isolated Windows 11 session decoded the cold-start HTTP fixture and
confirmed pause, volume and speed. Three separate handoff processes appended
to the active playlist, replaced its selection and opened a second native
frame in the original process. The handoff processes exited successfully.
Both frames closed in order and the original test process exited normally.

The locked Linux application passed cold activation and real desktop
dispatch in an isolated native Wayland session. The registered desktop entry
delivered enqueue, replace, new-window and fullscreen requests. The signed
test query stayed out of the window title. The `weblink` action decoded
a local file URI in the existing player after the fullscreen window closed.

The Windows workspace gate, Settings pixel comparisons and all 60 pinned
engine tests passed. The screenshot regression now waits for mpv's initial
playback restart before testing capture. The pinned Inno Setup compiler
built the opt-in handler against the archived portable
payload. The macOS packager verified that URL registration appears only in
opt-in bundle metadata. Strict Apple Silicon cross-Clippy passed. Installing
the Windows handler on a deployed profile and native macOS URL delivery
remain unverified.

- Windows application SHA-256 `0fe2cb15c58ee78be94f82ca2821ddc9b44c256ebd7e63c807f134df5ee528fa`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`
- Linux application SHA-256 `9885e6aacad3c7041400bd3a0e8a1d9bd2fa4737b90d95abf7f427990c9e26b5`
- Wayland libmpv SHA-256 `cdce854e1811942602587310026306d92d30c0f593f2be676f07659ae43061d8`

### Open in Riina extension 2026-11-24

The extension has a direct toolbar action, a configurable playback menu and
separate page, link, video and audio context actions. Settings → Utilities
opens the bundled browser setup guides. The extension stores the toolbar
preferences and requests access to the active tab when invoked.

In an isolated Linux X11 session, Chrome for Testing 158.0.8127.88 loaded
the extension unpacked. A toolbar click reached the browser's external-link
prompt, then opened Riina and decoded the selected WAV after confirmation.
The link context menu replaced the current media. After enabling the playback
menu in extension settings, **Add to Playlist** appended a third fixture
without changing the current item. The player received the exact signed
query parameters in all three cases. Desktop dispatch used the opt-in
GIO registration and the player from the browser-link activation batch.
The test player closed through its Quit shortcut.

Firefox 160.0.1 loaded the temporary add-on and opened a signed media link
from the context menu through its native external-link confirmation. The
player decoded the selected WAV and closed through its Quit shortcut. The
new Linux build repeated that handoff and both Utilities buttons opened
the correct packaged HTML files through the desktop's registered handler.

The Windows workspace gate and all 60 native engine tests passed, with
updated Utilities and Default Application dialog pixel baselines. The
JavaScript suite exercises all five action encodings, four context targets,
permission scope and rejected URLs. The Windows ZIP assembly and macOS bundle
checks included all 11 extension files, and the setup guides remain available
with URL registration disabled. Strict Apple Silicon cross-Clippy passed.
The grouped fast gate ran the browser and packaging suites with application
and UI tests. Separate Edge and Safari runtime checks and signed extension
packages remain open.

### Shared recent activity and history clearing 2026-11-24

Player menus, Welcome and plugin snapshots share the profile’s Open Recent
list. Welcome resolves the Resume card from the saved local file and uses
the document list for its other rows. The standalone History window and
the File menu keep their independent clear actions. Settings Utilities
clears the archive, Open Recent and the saved Resume entry.

An isolated Linux X11 playback session opened two paused video windows and
a shared History window. Both clips appeared in Open Recent and Playback
History. Escape and closing Settings with the confirmation
open preserved both profile files byte for byte. Reopening Settings left
the previous confirmation closed. Tab and Return accepted the new sheet
and the History window refreshed to its empty state while both videos
remained paused. Closing the first player left the second open. Quitting
the second preserved the empty lists and exited the isolated process.

The headless player regression exercises shared menu and Welcome updates,
independent Open Recent clearing, stale shutdown saves and a new open whose
playlist entry arrives after the ready phase. A separate persistence test
verifies that a writer holding the old revision cannot restore the Resume
entry. The core serialization fixture covers the original preference bytes
before any clear.

The full Windows workspace gate passed with the confirmation changes. The
grouped gate passed with the delayed-playlist follow-up, including the
Windows player and Settings pixel baselines. All 60 pinned native engine
tests and strict Apple Silicon cross-Clippy passed. Native Windows and macOS
dialog interaction remain unverified for this batch.

- Linux application SHA-256 `c5b460b936ab701a6e054234070d20eae9b3b8e0dcc3268401e978c2b5bdc5e2`
- X11 libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`

### Settings Utilities confirmations 2026-11-24

Settings owns the confirmation sheets for restoring alerts and clearing saved
playback progress, history and thumbnails. Restore Alerts shows an acknowledgement,
consistent with IINA v1.5.0 retaining the action without any suppressible alerts.
Progress and history also report the result on the owning Settings window.

An isolated Linux X11 session ran two paused video fixtures. Escape preserved
seeded progress and thumbnail files. Keyboard confirmation cleared progress,
history and thumbnails independently. The thumbnail cache shrank from 17 B
to 0 B, and a symlink and its out-of-cache target were preserved. A regular file
at the progress directory path stayed intact and produced a visible error.
Closing Settings during the thumbnail confirmation preserved a new 18 B entry.
Reopening Settings showed the new size and dismissed the earlier confirmation.
The Quit shortcut exited the isolated player.

The grouped headless regression covers all four actions, including safe initial
focus, Escape, a single callback on confirmation and closing a pending sheet.
The full Windows workspace build and gate, player and Settings pixel baselines,
all 60 pinned native engine tests and strict Apple Silicon cross-Clippy passed.
Native Windows and macOS dialog interaction and matched IINA pixels remain open.

- Linux application SHA-256 `99228dfdf77772fe95bca36ef5b06e4ad6d5082489539eedc58a055c33959aa7`
- X11 libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`

### Shared Log Viewer 2026-11-24

The Window menu and Advanced Settings share one Log Viewer. It reads the
application diagnostic and optional mpv logs, combines subsystem, severity and
case-insensitive message filters, and saves the complete retained log or the
filtered records. Copy preserves the display order of the selected records.
Unchanged polls leave the rows alone and closing the window stops its timer.

The Windows private desktop exercised the native Window menu from two paused
player windows. Both opened the same viewer. Closing and reopening reused that
window, and Quit shut down the isolated process. In Linux X11, the Window
shortcut also reused a single viewer, preserving filters and picking up a log
append on reopen. Control-click and the system clipboard verified two selected
records. A test FileChooser portal exercised full export, filtered export,
cancellation and rejecting a destination that would replace an active log.
The expanded run selected two subsystems, cleared the selections, and appended
76 records to exercise the scrolling list. Follow Latest fully revealed the
newest row, paused while reading older rows and resumed on request. The test
player exited normally.

The two grouped regressions cover record identity across appends, filters,
replacement and partial lines, distinct export snapshots and visibility-bound
polling. The Windows workspace gate and all 60 pinned native engine tests passed
before the viewport adjustment. The final Windows build, grouped app and UI
gate, headless pixel baselines and strict Apple Silicon cross-Clippy passed.
Native macOS interaction, window geometry persistence and matched IINA pixels
remain open. The export run used a test portal. Desktop picker appearance and
interaction remain separate acceptance work.

- Windows application SHA-256 `d3fda4b23129969a3ba31a21810edcb2b84b3b4f9e162b5ffe8893b79c47be1c`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`
- Linux application SHA-256 `f5ebaba6f99e3445b1870859d7ea012cb1c2a7b8d463b59a41c98f57703a2b52`
- X11 libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`

### Media Inspector 2026-11-24

Window → Inspector opens one application window for the focused player. The
General, Tracks, File and Status tabs combine playback observations with
read-only libmpv requests. The profile stores watched properties. Closing the
Inspector stops its timer. Switching the player, file or tab discards an
outstanding sample before it can replace newer values.

The grouped regressions cover incomplete requests across tab changes, file
loads and closing the window. Watch edits preserve unrelated settings and
reject a removal if another process changed the list first.

An isolated Linux X11 session started a multilingual Matroska clip and the
moving-square AVI in separate player windows. The Inspector showed the video
details and the English subtitle track without selecting it for playback.
Copying the File tab's path before and after a focus change confirmed that the
same Inspector followed both files. The Status tab's `pause` watch changed from
`yes` to `no` when playback started. Cancelling a watch prompt left the list
intact. Adding and removing another watch, closing and reopening the Inspector,
and quitting preserved the remaining watch in the profile. The isolated player
then exited normally.

A private Windows desktop opened Inspector from the native Window menu of two
players. Both commands reused one window. Closing and reopening it reused the
same window, and Quit exited the test process. The full Windows workspace gate
passed before the final tab and track-label adjustments. The final grouped
app, UI and engine checks, Windows workspace build, all 60 pinned engine tests
and strict Apple Silicon cross-Clippy passed. Native macOS interaction, matched
IINA appearance and keyboard interaction within watch rows remain open.

- Windows application SHA-256 `926354d58e69e4377dac69103756018b3457daef23e9b0813a031181520867fe`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`
- Linux application SHA-256 `7f236bd39d9d7cbb5aafa222ba64e9b08e0bb19773bd2c31f0f4454a73d826d8`
- X11 libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`

### Release Highlights and About 2026-11-24

Help opens one Release Highlights window with the bundled notes for the build's
version. On the first release launch the app records that version after the
window opens. The profile regression covers concurrent claims, a dropped claim
and opening the guide for a newer version. About also has one window for the
application and keeps its tab when reopened from another player.

The full Windows workspace gate, the 60 pinned libmpv integration tests and
strict Apple Silicon cross-Clippy passed. An isolated Linux X11 development
build showed the paused moving-square fixture and confirmed the guide stayed
closed at startup. Both player menus reopened the same Release Highlights and
About windows. The guide displayed the bundled 4.2.1 notes, Escape dismissed it,
and About displayed the running libmpv and FFmpeg versions. Quit exited the
test process normally.

A private Windows desktop used a workspace build with debug assertions disabled
for Riina to exercise the release startup policy. On a fresh profile the guide
opened and recorded version 4.2.1. Relaunching with the same profile kept the
guide closed until the Help action. Both players reused the guide and About.
About stayed open when its originating player closed. Both application launches
exited normally.

The bundled notes render inline link labels as text and provide a separate
button for the release page. Matched IINA pixels, native macOS window interaction
and the final Windows release package remain open.

- Windows application SHA-256 `a0cb8cea7e89942fe189d0a71b16711ff301b0850030fe830edd46a82df4228f`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`
- Linux application SHA-256 `d93393c0747a72ccdab4c379a4e7dd3c3ae2209aff9c2ddd28eb6e591b8da09b`
- X11 libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`

### Open URL 2026-11-24

The File menu and Welcome share one Open URL window. A typed host and port
receive the default `http://` scheme. A failed load leaves its address ready for
editing. Stop Loading cancels the attempt, and the alternate File action inverts
the preference for using a new player window.

The grouped regressions cover validation, Welcome and menu ownership, and retry
after a cancelled launch. The native engine test uses localhost for an HTTP 401
response, authenticated playback, cancellation and a subsequent load. It checks
that the playlist and both readable transport properties use an address without
credentials. The full Windows workspace gate and all 61 pinned libmpv tests passed.

On a private Windows desktop, two existing players reused the same Open URL
window. Setting the address and invoking Open through Windows accessibility
loaded the localhost clip into the initiating player. The alternate menu action
opened another stream in a third player and kept both existing players open.
Each successful load dismissed the dialog. Quit exited normally.

In an isolated Linux X11 session, a local file URL was rejected before
submission. The keyboard flow showed the implicit scheme, retained the address
after a 404 response, and stopped a request after the server received it. The
next attempt passed an HTTP Basic challenge and played the moving-square
fixture. Reopening from the second player reused the dialog. Escape dismissed
it, and Quit exited normally. The final grouped Windows checks and strict Apple
Silicon cross-Clippy passed after the theme and accessible-input adjustment.

Remembered credentials across application restarts, native macOS interaction,
and matched IINA pixels remain open. The following record covers external
stream loading.

- Windows application SHA-256 `7b22f1ee987287144fee23fcefca0787aa72ce279fb89cd6ff049fa535f92613`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`
- Linux application SHA-256 `3987fbfac3b0a001324f20d49a1c5ce292cb3c081ec09be7cd0765185afe58a2`
- X11 libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`

### External stream loading and Debug Info 2026-11-24

A single URL supplied at startup and an immediately opened browser URL use the
same loading window. A private Windows desktop opened a delayed localhost
stream from the command line. Stop Loading was invoked through Windows
accessibility after the server received the request. The retry and a later
browser activation used the same player. Closing it exited normally.

An isolated Linux X11 session repeated the command-line cancellation and
retried a browser URL after a 404 response. The stream retained the requested
1.5-second start, paused state, 1.25× speed and volume 31. Enqueue added a
playlist item without opening the loading window or requesting the URL. The
new-window browser action opened a second player in the same process.

Help → Dump Debug Info saved both player snapshots in creation order. After
the first player closed, the next report contained only the remaining player.
The session exited normally. The report confirmation dialog was unavailable in
the isolated desktop. The grouped regressions also cover credential removal
from standalone URLs and a closed session whose observations are still retained.

The full Windows workspace gate and all 61 pinned libmpv tests passed. The final
grouped Windows gate and strict Apple Silicon cross-Clippy passed after the
report ownership correction. Native macOS interaction and loading presentation
for mixed or grouped command-line inputs remain open.

- Windows application SHA-256 `e3993a0d1ba67338df7e5b25fda6beeca88104e09566f934f026cdeaf4e0edcf`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`
- Linux application SHA-256 `ec1bfaff4873606cca03de4428e100a13841eb894b40cfcf321a60c27096da3d`
- X11 libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`

### Window switching 2026-11-24

The application-owned Window menu follows each player's visible surface, title
and lifetime. The headless regression covers a hidden registration, its
Welcome-to-player transition, stable selection on rename and retirement while
the underlying component is still alive.

A private Windows desktop opened two players in one process. Both native Window
menus listed the players and checked their own entry. A filename containing an
ampersand retained the literal character. Selecting a minimized player restored
its frame and preserved the initiating menu's checkmark. Closing the second
player removed its entry, and closing the first exited normally.

An isolated Linux X11 desktop exercised the actual menu with pointer input.
Selecting the second player restored it and gave it focus. Bring All to Front
also restored it and left the initiating player focused. The desktop captures
showed the second player's checkmark after switching and only the first player's
entry after closing the second. Quit exited normally.

The full Windows workspace gate, all 61 pinned libmpv tests and strict Apple
Silicon cross-Clippy passed. macOS Window and Help menu interaction, Wayland
window activation and comparison with matched IINA pixels still need verification.

- Windows application SHA-256 `b0eab5d5f5629c23730d45536c2cc0f1e1935ea96bdd5e45247adfaeea98984b`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`
- Linux application SHA-256 `0ad79bb9d2a8ea18be74ea136832f678b491f3c5452809e204061764e98a37dc`
- X11 libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`

### Grouped startup streams 2026-11-24

The grouped regression checks that a prepared network-first startup selection
retains local filename order and subsequent URLs. The existing native HTTP
test also opens an explicit three-item playlist and verifies the paused first
stream, local file identity and trailing URL.

The private Windows desktop and isolated Linux X11 session each launched a
group containing a delayed HTTP stream, a local clip and a second HTTP stream.
Stop Loading cancelled the first request after it reached the server. Submitting
a replacement address in the same dialog opened the replacement and dismissed
the dialog. The Windows Playlist menu showed all three items in order. Native
Next Media on Windows and the playlist shortcut on Linux advanced through both
remaining items in the same player. The second HTTP stream was first requested
after navigation. Both sessions exited normally.

The Windows workspace gate, all 61 pinned libmpv tests and strict Apple Silicon
cross-Clippy passed. Native macOS interaction remains open. Shuffled startup
continues through the regular playlist path.

- Windows application SHA-256 `69ea4c12840467736759a8b07bf34ddfe99663f3d0ee9197c4b0a9c87cbce1d3`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`
- Linux application SHA-256 `6c97ff773b546a6650cedc8d80e9eb11731460992e426e32c90e2b7dcbd33ed7`
- X11 libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`

### Wayland window switching 2026-11-24

An isolated Sway desktop with XWayland disabled opened two tiled players.
The 1280 × 800 desktop capture showed both video fixtures paused. Pointer
selection in the first player's Window menu requested activation for the
second. The protocol trace recorded the compositor's input serial, token
reply and the activation request for the second surface. Sway reported the
second player focused. Closing it removed its menu entry. Closing the final
player exited normally.

The same Linux build passed the isolated X11 checks for minimized-player
restore, Bring All to Front, closed-entry removal and normal shutdown.
The locked Linux application build, strict Linux application Clippy and full
Windows workspace gate passed. The initial floating-window setup did not
produce a stable playback capture. Floating-window transitions and other
Wayland compositors remain open.

- Linux application SHA-256 `6c537f88a3c36ca1989a0da1034933493d2cdd9b2657d1555d7d177a5fa33471`
- Linux libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`

### macOS launch routing and stdin playback 2026-11-24

The macOS port registers Finder documents, Dock reopen, Quit and browser URLs
alongside Winit's delegate. The deferred initial action lets a cold Finder open
take precedence over Welcome or the Open panel. A folder and a file can be
chosen together in the AppKit Open panel. The windowless menu retains Open
File and Quit. The document-list regression covers order, spaces and Unicode.
It compiled in the Apple Silicon check and still needs a native test run.

The full Windows workspace gate and locked build passed. The windowless
selection regression exercised separate players, grouped playlists and rejected
subtitle-only selections without a GUI. Strict Apple Silicon cross-Clippy and
the locked Linux application build and strict application Clippy passed.

A private Windows desktop fed the fixture through an inherited pipe. An
isolated Linux X11 desktop redirected the same fixture to standard input. Each
application opened one player, decoded the 320 × 180 video, advanced playback
and exited normally. The Finder, Dock and native Open panel workflows still
need a packaged macOS desktop run.

- Windows application SHA-256 `7ed94c9e449e65e1734973198b14fb8339e896e0e0771fab547151051c1276fe`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`
- Linux application SHA-256 `22bca8261c8e20af969156c9adc76554332af360d4d8a328994197638e2eaf3c`
- Linux libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`

### Opt-in New Window 2026-11-24

New Window follows IINA v1.5.0's default-off shortcut and opens an independent
Welcome window. The profile regression covers the unset default, the IINA
preference alias and an enabled setting round trip. Existing default profiles
retain their serialized bytes.

A private Windows desktop verified that File hides the command by default.
With the opt-in enabled, two successive File menu activations each opened a
Welcome window and added it to the Window menu. Closing each Welcome removed
its entry while preserving the original playback frame. The saved Open Panel
launch preference did not intercept either New Window command.

An isolated Linux X11 desktop verified that Ctrl+N did nothing by default.
With the opt-in enabled, the same shortcut opened and closed two separate
Welcome windows in succession while keeping the original paused player open.
Both test sessions shut down normally on Windows and Linux.

The full Windows workspace gate, locked Windows and Linux builds, all 61
pinned Windows libmpv tests and strict Apple Silicon cross-Clippy passed.
The native macOS menu and Cmd+N interaction still need a packaged desktop run.

- Windows application SHA-256 `70c28b225201770d021e54134ede8c041bf130226495940ec9f42f3e350094d0`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`
- Linux application SHA-256 `fce31d52c0a962f6e096fe643125058c3b980dffa3e6dc74408193d3adf9cc26`
- Linux libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`

### File and URL open destinations 2026-11-24

The primary File commands follow the saved new-window preference. The modifier
commands invert it, and each label reflects its destination. The windowless
macOS menu opens the shared URL dialog without constructing a player before
submission. Headless regressions cover that dialog route, cancellation, and
the grouping and ordering of files opened without an existing player.

A private Windows desktop exercised both menu entries with the preference on
and off. The four cases each selected a local clip through the native picker
and submitted a loopback stream through Open URL. Each selection played in the
existing frame or opened a second frame as indicated. Changing the preference
in General Settings refreshed both active players’ File and URL labels. Each
test session exited normally.

The isolated Linux X11 desktop passed all four Open URL shortcut combinations
with Ctrl+Shift+O and Ctrl+Alt+Shift+O. Reuse kept the same player and a separate
open kept the original clip available. The four sessions exited normally.

The full Windows workspace gate, all 61 pinned Windows libmpv tests, locked
Windows and Linux application builds, 576 Linux application unit tests and
strict Apple Silicon cross-Clippy passed.
The AppKit Option menu replacements, windowless Open URL on a native Mac, and
the Linux native file picker still need desktop verification.

- Windows application SHA-256 `73c1822ec875147911d32902f0666386145f5d753b1d6a72d40cce692a4dafd3`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`
- Linux application SHA-256 `3e8c1d227f2d38e615f1136a908dcc9e3c1f7fad671e1e08f20cda139c80095f`
- Linux libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`

### History and recent documents without a player 2026-11-24

The macOS File menu can show the shared Playback History archive and open or
clear recent documents when no player is active. Opening through the idle
picker records the selection in one profile update. Finder opens follow the
track-all-played-files preference. Restoring the idle menu keeps commands
available when History outlives the final player.

The regressions open History before attaching players and confirm the same
window retains its search. They also check the persistent archive shared with
later players, Unicode document identity, most-recent ordering and preservation
of resume entries when the document list is cleared.

A private Windows desktop opened History from two players and reused one
window. Clearing Open Recent refreshed both File menus and preserved the
playable History row. History survived closing both players, opened a fresh
player in the same process, and exited normally after the final window closed.
Isolated Linux X11 exercised History from Welcome, focus-based playback
routing, keeping History across player closure and reopening a player from
History. The test process exited normally.

The full Windows workspace gate, locked Windows and Linux builds, 578 Linux
application unit tests, all 61 pinned Windows libmpv tests and strict Apple
Silicon cross-Clippy passed. Finder tracking and the idle AppKit actions and
quit behavior still need a native macOS desktop run.

- Windows application SHA-256 `d506a569e6dd23c6f52b485e97e63eee5072153112f8c42569d6b658f5a5c5c6`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`
- Linux application SHA-256 `efdcb0888956ec6b6d4dd84989f4184d1e20ff9d0ef90077a55e0022eefe6639`
- Linux libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`

### About and Log Viewer without a player 2026-11-24

The idle macOS menus open the application-owned About and Log Viewer windows.
About reads the installed libmpv and FFmpeg versions from a short-lived client
with null output. The AppKit Window menu remains registered while About,
Log Viewer, Playback History or Open URL outlives the final player. Closing
the last visible tool then applies the saved quit policy when appropriate.

Headless tests opened About and Log Viewer before attaching player menus and
verified window reuse. The About test also verified that its close notification
runs after the window hides. The pinned native suite compared the versions
returned by the short-lived client with the values reported by a normal
player before media opened.

The private Windows desktop and isolated Linux X11 session opened both tools
from two players. Both tools remained visible after the opening player closed,
reopened from the remaining player, and closed normally. Both processes exited
after their final player closed. The Linux desktop showed the engine version
labels and log rows while the second clip remained paused.

The Windows workspace gate, all 61 pinned Windows libmpv tests, locked Windows
and Linux application builds, 579 Linux application unit tests and strict Apple
Silicon cross-Clippy passed. The idle menu actions, Window menu registration,
and deferred quit with several tools still need a native macOS desktop run.

- Windows application SHA-256 `7f723d64f8ca9dfaeb2729a37495158aa35a894be3cda1bce0e919bc1f248b0f`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`
- Linux application SHA-256 `a33f23f6a59799359eca8082a64945512a9b47577e997deb9cff478698e4c7f1`
- Linux libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`

### Software Update and Help without a player 2026-11-24

The macOS application initializes the per-profile update schedule before opening
a player. The idle menu can open the shared update window and offers the Help
links, Release Highlights and Debug Info. Software Update and Release Highlights
participate in the deferred quit workflow when they outlive the last player.

The headless update regression covers the first and second launches, declining
automatic checks, and a manual check that joins an in-flight request. It asserts
that player and application commands use the same window and preserve the saved
consent choice. Its update result is supplied locally. The release guide regression
opens before a player connects. Debug Info can produce a zero-player report.

A private Windows desktop checked the live release service, retried from the
same window and preserved paused playback. Release Highlights reopened the
same guide from Help. The test process exited normally. An isolated Linux X11
desktop exercised update consent, saved the Escape dismissal as disabled and
opened the manual update window afterwards. A separate isolated Linux run
saved debug reports showing two players and then only the remaining player.
The report confirmation dialog was unavailable in that desktop.

The Windows workspace gate, all 61 pinned Windows libmpv tests, locked Windows
and Linux application builds, 580 Linux application unit tests and strict Apple
Silicon cross-Clippy passed. The idle AppKit actions, Help search, and shutdown
with these windows open still need native macOS verification.

- Windows application SHA-256 `b915def17e721fb123e918a52738b3a9c518ab7868bedc244058a135a71bb323`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`
- Linux application SHA-256 `adab5ac9c2cda6b41414f31ddf392c2648f63e0cb3281baca133c41606df5fa6`
- Linux libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`

### Profile-backed Settings initialization 2026-11-24

Settings now projects fields from the saved profile before installing
player-specific actions. The application updater and playback power services
can attach to the form with no player. The main Settings regression tests use
the same form factory as the application. Headless coverage includes a saved
profile and Utilities callback without a player, update and power edits
across two forms, an invalid launch choice alongside a decoder edit, and
screenshot edits that preserve a saved AVIF format and custom filename
template outside the form.

A private Windows desktop opened and reopened the same Settings window during
paused playback. Changing the opening policy updated the native File menu
and persisted across restart. Both test processes exited normally. On isolated
Linux X11, the Utilities workflow exercised defaults, progress, history and
thumbnail actions, cancellation and reopening. The test process exited normally.

The Windows workspace gate, including the software-rendered player and
Settings comparison, passed. All 61 pinned Windows libmpv tests, locked Windows
and Linux application builds, 583 Linux application unit tests and strict
Apple Silicon cross-Clippy passed.
The app-owned Settings window and its AppKit menu entry remain to be connected
and need native macOS verification.

- Windows application SHA-256 `2a2647ddc1413cbe2ff407984179d1748334f3da7bd9494aed131952242560c4`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`
- Linux application SHA-256 `5f36224470649363a8e8bd1007b1fb04f1ff12d1cc5992f9ec87ba7b5cd7baaa`
- Linux libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`

### macOS Settings before playback 2026-11-24

The idle macOS application menu registers Settings with Command-comma. The
application-owned form loads and edits the profile without a player. It uses
the application updater, on-disk history and plugin host, along with the
folder, key binding and subtitle controls shared with player Settings.
Closing the form releases its tools and notifies the application's window
lifecycle. When Welcome already has a configured player, the menu can use
that player's Settings actions.

Headless tests open and reuse the form, edit settings across sections, and
verify that a concurrent save keeps unrelated nested fields. Other cases
cover validation before a profile is written, visible error feedback,
preserving a credential when its keyring write fails, and keeping
credentials out of the profile.

A private Windows desktop opened Settings during paused playback. It
cancelled the screenshot folder picker, reopened it, selected a folder and
verified the saved path. A General setting changed the native File menu,
survived restart and left playback paused. Both processes exited normally.
The isolated Linux X11 Utilities run covered confirmations, cancellation,
history, saved progress and thumbnails, then quit normally. The Windows
workspace gate and software-rendered player and Settings comparisons passed.
Locked Windows and Linux application builds, 586 Linux application tests,
all 61 pinned Windows libmpv tests and strict Apple Silicon cross-Clippy passed.

Native macOS verification still needs to cover the menu shortcut, folder and
subtitle pickers, focus and windowless shutdown. An idle form left open when
playback starts continues to edit defaults for subsequent players.

- Windows application SHA-256 `02c65b8cca07a9acade491635cc94804a7ec86b9e6be7f22b04b4bcd30378655`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`
- Linux application SHA-256 `d614aadcb5ddb148d33247ce9597c440246753f554846efcd9e4987582b06d11`
- Linux libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`

### Finder plugin packages and installer review 2026-11-24

The macOS bundle declares the IINA plugin archive type as an alternate
handler. The first archive in a Finder document group opens the Plugins
pane and follows the same review as a package selected in Settings.
Local and GitHub installs validate the package on a worker. The owning
Settings window asks before a new installation or a replacement. An
approved compatible package can start after its grants are saved.

Regression tests cover mixed Finder document groups, invalid archives,
discarded reviews, enabling approved packages, stale replacement
reviews and rollback after a grant write fails. A second writer cannot
reserve the package directory while an earlier placement is rolling back.
Bundle tests validate the file extension and ZIP type declaration.

On a private Windows desktop, cancelling an initial package left it
uninstalled. After approval, the package was installed and enabled.
Cancelling a replacement kept the previous version and grants, and later
approval installed the replacement with its reviewed grants. Playback
stayed paused and the test process exited normally. The Windows workspace
gate, locked Windows and Linux builds, strict Apple Silicon cross-Clippy
and all 61 pinned Windows libmpv tests passed.
On isolated Linux X11, the Utilities confirmations, reopening Settings and
normal shutdown passed. The Linux application suite passed 592 tests.

Native macOS verification still needs Finder delivery for cold and warm
launches, Settings focus, the review sheet and closing it during shutdown.

- Windows application SHA-256 `338fd4b5da0cbe592b8c75357c5956f8de6356c4c566585abad7ab11a3b71c83`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`
- Linux application SHA-256 `8f1b90fe164e9381e60535a79d4ef4691a01dc74bb8d47ce2f871881e85a887d`
- Linux libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`

### macOS global plugins before playback 2026-11-24

macOS startup loads enabled global plugin entries before playback. Global
requests can wake the application to open a standalone tool, create a
player or present a dialog. Changes to plugin tools and application dialog
closure participate in the application's quit policy. The application
checks WebView frames while a standalone tool is visible and uses host
wake notifications while idle.

A host test loads two installed packages with only one enabled and no
player registered. It observes the enabled package opening a standalone
surface and reserving a player, then disables the package and checks that
both resources retire and the application receives a wake.

Private Windows and isolated Linux X11 desktop runs answered a plugin's
startup confirmation, opened separate global and player tools, and closed
the player tool independently. The Windows Plugin menu reopened the same
global window during paused playback. Both test processes exited normally.
The Windows workspace gate, locked Windows and Linux builds, 593 Linux
application tests, 61 pinned Windows libmpv tests and strict Apple Silicon
cross-Clippy passed.

The windowless AppKit routes, native dialog focus and deferred quit still
need a native macOS desktop run.

- Windows application SHA-256 `78aa1a537f35173d80a503b31404551933c51253a48be57cab45e3bad5788a05`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`
- Linux application SHA-256 `39b98b751459870e3f0106b0cba9342da13bd6e9ce9d43bae8ff9ec039a342e2`
- Linux libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`

### macOS Plugin menu before playback 2026-11-24

With no player open, the application menu offers Manage Plugins, global
actions, the global Developer Tool and Reload All Plugins. Manage Plugins
selects the Plugins pane in application Settings. Menu sections use the
profile's package order, with nested items and overflow under More. Menu
updates published during tracking appear after the current menu closes.
Global consoles remain attached to their own realm and close when it stops.

The windowless host regression covers a selected nested action, a disabled
parent, action-driven checkmark changes, and reload with a reused action ID.
An action from the old realm is discarded. Disabling the replacement clears
its menu, player request and standalone window.

The Windows workspace gate and locked Windows and Linux builds passed, with
629 Windows application tests, 593 Linux application tests and 61 tests against
the pinned Windows libmpv. Both isolated desktops answered the startup prompt,
opened separate standalone tools and shut down normally. On Windows the native
menu also opened both developer consoles, which closed independently. The
first immediate standalone-window reopen timed out. After a short interval
following close, the same named action reopened the original window and
playback stayed paused. Apple Silicon cross-Clippy passed. Native macOS menu
tracking, key equivalents, Settings focus, console behavior and shutdown still
need a Mac desktop run.

- Windows application SHA-256 `0880e01b64bc9129de291176ca1a423ac4a06372998340b48919c41252d7fbbf`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`
- Linux application SHA-256 `581bb56b6016a0fbf7c9681daedf17aef694a6757d6ef8382fcf4d708e9ee7ec`
- Linux libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`

### Standalone plugin close, reopen and resize 2026-11-24

Standalone window updates retain explicit open requests and the frame axes
supplied by the plugin. The core test covers coalesced updates, repeated
values and invalid frame edits. The host test covers the same document across
close and reopen, pending visibility, and rejected failure notifications
from an older presentation.

On a private Windows desktop, the plugin restored a previously requested
width after a native move and resize, preserving the current X, Y and height.
The global window reopened six times immediately after the native close.
It retained the same window handle. The developer consoles remained
independent, playback stayed paused, and the exact process exited normally.

The Windows workspace gate and locked build passed. The pinned Windows
libmpv suite passed all 61 cases with two test threads after four timeouts
during the run that overlapped the desktop test and Linux compilation.

On isolated Linux X11 and native Sway Wayland with XWayland disabled, the
global WebView kept the same window while the first player closed. Its Settings
and Quit shortcuts routed through the surviving player, and the exact process
exited normally. The locked Linux build and 593 application tests passed.
Apple Silicon cross-Clippy passed. Native AppKit activation, frame geometry
across displays and the pending-window quit policy still need a Mac run.

- Windows application SHA-256 `0611a8f8dabca303b11766ce5b6f25d2bc8ef7ade774b217084863702f86c379`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`
- Linux application SHA-256 `84ca70f2738e510d4fb3a701a0a2aa68795d25b36c0ada2ff4ae714952e829f2`
- Linux libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`

### macOS application Edit and Services menus 2026-11-24

The idle and playback menu bars share Edit, route Close to the key window and
register their Services submenu with AppKit. The idle application menu adds
Hide Others and Show All. The New Window row and shortcut follow changes to the
saved preference while the application is idle.

The headless editor check exercises two real Slint text items. It confirms that
edits require focus, read-only text can be selected and copied, Unicode clipboard
text survives paste and undo/redo, and a retired window cannot accept a command.
The Windows workspace gate and locked build passed, as did 594 Linux application
tests and Apple Silicon cross-Clippy. Native macOS checks still need to cover
shortcut and click delivery to Slint and WebKit, Services across menu changes,
the selected window for Close, and menu enablement when no edit is available.

### macOS playback menus without an open player 2026-11-24

The macOS application bar retains Playback, Video, Audio and Subtitles before
playback and after the last player closes. It reads the same catalog as the
player. Media-dependent commands are unavailable, and shortcuts and submenu
labels follow the saved profile. Playback offers Go to Screenshot Folder,
which uses the same directory opener as the player. The File menu shows
its current-media commands disabled until a player can handle them.

The headless catalog test covers section order, nested choices, saved filter
names and updating shortcut hints. The Windows workspace gate and locked
build passed, along with 595 Linux application tests and 61 pinned Windows
libmpv tests. On a private Windows desktop, the active player retained all
four native menu sections, kept the fixture paused through a plugin workflow,
and exited normally. Apple Silicon cross-Clippy passed. A native Mac run
still needs to check menu navigation, inactive shortcuts and Finder opening.
That batch left Video and Audio Filters tied to playback sessions.

- Windows application SHA-256 `c4b6dc4983d14f669ccddbfe1fb461575a7bca89acb7d60bf2d7f01a6f319a93`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`

### Application-owned audio and video filters 2026-11-24

The Video and Audio Filters commands open one editor of each kind for the
profile. The idle macOS menu can open them before playback. Saving presets and
editing their shortcuts work with no open player. The active filter chain follows
the last active playback window. Closing a player leaves a visible filter editor
available for preset editing.

The Windows workspace gate passed, including the software-rendered player and
Settings baselines. Headless tests saved a preset before playback, routed commands
between two players, rejected an edit after the media generation changed, and
kept the same editor after both player routes closed. Profile-store tests merged
edits from independent profile views and rejected a stale removal. The locked
Linux application build, 597 Linux application tests and strict Apple Silicon
cross-Clippy passed. Native macOS checks still need to cover opening from the
idle menu, adopting a Finder-opened player, the LUT picker and final-window
shutdown.

On a private Windows desktop, the native Video and Audio menus opened separate
editors. Reopening reused the same video window, and a saved video filter became
active while the fixture stayed paused. Both editors stayed open after the
player closed. Closing the final editor shut down the isolated process normally.
The separate pinned libmpv suite passed all 61 tests.

- Windows application SHA-256 `99d96d622d03f36cf5edff582c9f8d4fa82e5da81ac6efd9906d08655c1de567`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`

### Bundled interface languages 2026-11-24

The Russian and Arabic catalogs cover interface labels in Welcome, Open URL,
Playback History, Log Viewer, About, and the audio and video filter windows.
English is the fallback. The catalogs also localize basic URL validation, the
About version status and log counts. Main menus, Settings, player controls,
technical filter labels and some runtime messages still need translations.

The Windows workspace gate passed, including the English player and Settings
software pixel baselines. One headless workflow switched between English,
Russian and Arabic, activated a Russian Welcome button, checked the long
authentication caption against the bounds of a narrow dialog, and verified
plural forms and Arabic text with an existing playback timestamp. The fast
gate selects the same widget and visual group for catalog-only changes.
All 65 Linux widget tests and Apple Silicon cross-Clippy passed. Native
macOS rendering still needs a desktop check.

On a private Windows desktop, the native menus opened Open URL, Playback
History, Log Viewer, About, and both filter windows under their English
titles. The fixture stayed paused through all six windows and Riina exited
normally after closing them and the player.

In isolated Linux Wayland sessions, the Russian and Arabic Welcome windows
opened their localized Open URL dialogs at 380 × 250. The Russian
authentication caption wrapped onto two lines with the complete text visible.
The Arabic caption and footer controls fit on the same client size. Both
processes exited normally after closing the dialog and Welcome.

- Windows application SHA-256 `6a0e8fc8a7dd994449719ce8afa7e8fefbc53fd7e77683e4777d5b18c694459f`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`
- Linux application SHA-256 `929f1cbffb02d26aa3b52ebd7de3981e6a2f0bdc9016e84a33b580222c103be2`
- Linux libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`

### Localized playback menus 2026-11-24

The Playback, Video, Audio and Subtitles menus use the bundled Russian and
Arabic messages on the shared player and the framed Windows owner. The idle
macOS catalog supplies the same titles and shortcuts. Its screenshot folder
and filter editor actions use keys from the catalog. Saved preset names,
device names, and provider names remain values supplied by the respective models.

The Windows workspace gate passed, including the player and Settings
software pixel baselines. The catalog test compared menu nesting, shortcut
bindings, separators and idle action identities in English, Russian and
Arabic. It also checked that a preset with the same name as an idle command
remains a preset. The Win32 menu test refreshed English, Russian and Arabic
readouts in place, including a value before its caption, while the menu
handles and video client bounds stayed stable.
The Linux application suite passed 596 tests and the widget suite passed 65.
Apple Silicon cross-Clippy passed. The live AppKit menu still needs a native
macOS check across languages.

On a private Windows desktop, native menu actions on a paused fixture
updated speed to 2×, volume to muted, and both delays to 0.5 seconds.
The player kept its pause state and frame bounds. Video Filters opened
and closed through its native menu action, and the test process exited normally.

In isolated Wayland sessions at 1070 × 640, the four menus displayed Russian
and Arabic titles and shortcuts. The Video Filters shortcut opened its
localized editor in both sessions, and both processes exited normally.
A 320 × 180 Russian capture showed that the in-window menu bar clips the
families at the right edge. The following batch addresses narrow menu access.

- Windows application SHA-256 `2db0858b5a9f480226413914d1d006b6fb26e019e30c233ab4caca0214b8c414`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`
- Linux application SHA-256 `e3192bfec9cceb23b4a0528d4c364106c2b29efadb275159d81da00035c3501d`
- Linux libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`

### Compact Linux player menus 2026-11-24

The compact menu regression clicks the Menu button at the top of a 320 × 180
client. It checks all ten families in the scrollable root, a bounded Video
submenu, the Back action, shortcut text and the Video Filters action. Resizing
to 1080 × 660 closes the popup and restores the regular bar.

The Windows locked build and workspace gate passed, including the headless
player and Settings pixel baselines. The null-output engine suite passed 61
tests with the pinned Windows library. Apple Silicon cross-Clippy passed.
The Linux application suite passed 597 tests and the widget suite passed 65.

Private Sway Wayland sessions with XWayland disabled exercised a paused
fixture at 320 × 180 in Russian and Arabic. The compact bar stayed above
the menu. Keyboard traversal revealed Help at the end of the root, and the
pointer Back row reopened the root from Video. Both sessions opened Video
Filters from the scrolled submenu and from the direct shortcut, restored the
wide bar at 1070 × 640 and exited normally.

A private X11 session with Openbox exercised the same Russian compact menu,
opened Video Filters by keyboard, restored the wide bar and exited normally.
Both Linux desktop checks used the same application and libmpv build.

- Linux application SHA-256 `dec7aade65b40d91e95c32a55dd6eb6619daccaa99c3c9ed73d93394018a6305`
- Linux libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`

### Localized application menus 2026-11-24

The application, File, Edit, Plugin, Window and Help menus use the same
English, Russian and Arabic messages on the player surfaces. AppKit uses
the same catalog for its idle application commands. Recent document titles
and plugin-contributed labels come from their own models.

The shared regression checked all ten menu families, translated File
alternates, shortcut hints and command delivery in the three languages.
It opened a localized plugin overflow menu and routed its sixth action
with the original token. The Win32 test sent popup notifications through
each localized Plugin submenu and checked that the adjacent File submenu
did not trigger a plugin refresh. The full Windows workspace gate and
locked build passed, including the player and Settings pixel baselines.
The Linux application suite passed 598 tests and the widget suite passed 65.

On a private Windows desktop, the English native menus opened and closed
About, Playback History, Plugin Settings, Log Viewer and Release Highlights.
The fixture stayed paused, the player frame remained stable and the process
exited normally.

In private Sway Wayland sessions without XWayland, the Russian and Arabic
menus at 320 × 180 opened and closed About, Playback History, Plugin Settings
and Log Viewer. Both restored the localized wide bar at 1070 × 640 and exited
normally.

Apple Silicon cross-Clippy passed. The idle-to-player menu handoff, Services
registration and Edit responder forwarding still need native macOS tests.

- Windows application SHA-256 `05aa59a23b56cb544f0f018035269162ef2e017ebc5154328adf55cc84ed4176`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`
- Linux application SHA-256 `167e7aab1add62ac141d2059391d09bb101e87ebe0bdc94025d1fd98cf32a079`
- Linux libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`

### Scrollable regular Linux menus 2026-11-24

A baseline capture from `d46ef93` at 1070 × 180 showed only the first few
Video rows. The popup covered part of the menu bar, and End left the same
rows visible. The candidate presents both the regular bar and the compact
Menu button through the in-client popup.

The widget regressions exercise the compact root at 320 × 180 and the
regular Video menu at 1070 × 180. They check panel bounds, repeated End
requests, a selected saved preset in the adjacent submenu, the Video Filters
action, hover switching to File and dismissal by click or Escape.
The Windows locked build and workspace gate passed, including the
software-rendered player and Settings baselines. Apple Silicon cross-Clippy
passed. The Linux application suite passed 599 tests and the widget suite
passed 65.

In private Sway Wayland sessions, Russian and Arabic exercised Video Filters
and the saved preset submenu at 1070 × 180 and 320 × 180. The direct shortcut
opened Video Filters again after resizing to 1070 × 640. The horizontal
centers of all ten Russian captions matched the baseline capture. A private
X11 session reached the last Video rows with the mouse wheel, switched to
File on hover, and opened Video Filters using the menu and the shortcut.
All test players exited normally.

- Linux application SHA-256 `9fee2299b686a9b0da21c400a1178620ac5dc37b761f18b2defc8bf57e9e6d11`
- Linux libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`

### Localized Settings navigation and General search 2026-11-24

Russian and Arabic cover the Settings title, sidebar, page and section
headings, search labels and the General controls. Search keeps a stable
target for each displayed result and includes General titles alongside the
existing English aliases. Selecting a control inside a General disclosure
opens that disclosure and reads the live scroll target.

The regression used a 600 × 480 Settings client. It selected the localized
History result by keyboard, then activated the localized Music Mode artwork
result found with an English phrase. Both languages reached the expected
controls. The full Windows workspace gate passed, including the English
player and Settings software pixel baselines. Apple Silicon cross-Clippy
passed.

The locked Windows build also passed a private desktop test. The native
accessibility controls selected the History search result and changed
recording off and back on. Both edits persisted, the physical player frame
and pause state stayed stable, and the player exited normally.

The Linux application suite passed 600 tests and the widget suite passed
65. In private Sway Wayland sessions with a 600 × 480 Settings client,
Russian and Arabic search reached History, the Music Mode artwork
disclosure and the screenshot clipboard setting. History changes persisted
in both languages, and the test players exited normally. Follow-up captures
confirmed the shortened captions fit the visible controls.

- Windows application SHA-256 `d20ac9c8e7c081d54608c5bd355133d433106422f681c86cbff71d3e8759ac72`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`
- Linux application SHA-256 `50f8015774dd9c8eac35213f68e4c4487af21280f08ca5370c722d6498e7ac16`
- Linux libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`

### Localized Interface, Video and Audio Settings 2026-11-24

Russian and Arabic captions cover the Interface, Video and Audio controls and
their Settings search results. The regression covers translated results,
disclosure expansion, runtime Live Text availability, and the tone-mapping
scroll range at 600 × 480. Choice tests cover tone-mapping order, the
ReplayGain and gapless enums, and preservation of a disconnected audio
device's recorded name and description.

The Windows workspace gate passed before the narrow-window follow-up. The
final focused gate passed with strict Clippy, 639 application tests, the
widget tests and all 51 software-rendered pixel baselines. The longer
Interface card shifted six pixels at the bottom of one scrollbar thumb.
The other 50 baseline files matched exactly.

The locked Windows build passed a private desktop test. Selecting the
English search results through native accessibility reached OSD seek
suppression, HDR target-peak override and the Audio device selector. Both
edited switches saved in each direction. The player retained the same
physical frame and paused state and exited normally. All 61 null-output
libmpv tests passed with the pinned Windows DLL. Strict Apple Silicon
cross-target Clippy passed.

The Linux suite passed 602 application and 65 widget tests. In private
Wayland sessions at 600 × 480, Russian and Arabic searches reached OSD
suppression, the expanded target-peak setting and gapless playback. Both
languages saved and restored the OSD and target-peak switches and displayed
the localized gapless choices. The players kept their original frames and
exited normally. Captures confirmed the fullscreen time-and-battery
caption fits on its own row. The Russian OSD sidebar label and two Audio
captions still truncate at this width. The Arabic target-peak range needs
left-to-right isolation for its digits.

- Windows application SHA-256 `a5f29b44b87648999894850dfcd21633fc8cc6bb0c3cd77cefbee50a434a86af`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`
- Linux application SHA-256 `141f1d3be93caa94f86c47381d67ea2cddd0d35e81352ff572b7b9ee798d2d5f`
- Linux libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`

### Controller clocks and center snapping 2026-11-24

The controller displays either duration or remaining time. The elapsed and
right labels share the saved precision. Settings can scale remaining time
by playback speed and disable snapping within five points of center. The
older profile defaults retain the duration display and center snapping.
Tests cover profile migration, precision rounding, paused multi-player
updates and preservation of unrelated saved preferences.

All 62 native Windows null-output tests passed. The added engine regression
changed playback speed to 2× while paused, compared both remaining clocks,
and verified that both became unavailable on stop.

The Windows workspace gate passed with 640 application tests and all 51
software-rendered visual comparisons. The Interface settings baseline changed
only in the scrollbar thumb after adding the controller rows. The Linux
application and widget suites passed 603 and 65 tests during the batch.

The locked Windows executable passed the private desktop scenario. The right
label switched to remaining time, and the native menu listed all four
precisions and checked the saved 10 ms option. At 2× speed, changing Settings
reduced the paused countdown from `-00:05.00` to `-00:02.50`. The center
snap setting and clock options persisted together. The player retained its
original physical frame and exited normally.

The final locked Linux executable opened the same controller options under
Wayland. A pointer click toggled remaining time, and the precision menu
saved the 10 ms option. The paused countdown displayed `-00:02.50` after
enabling speed scaling at 2×. A three-point horizontal drag stayed centered
with snapping on and saved the offset with snapping off. The player retained
its frame and exited normally.

At 600 × 480 under Wayland, the Russian OSD sidebar label and the two wrapped
Audio captions fit. The Russian Key Bindings sidebar label still clips at
that width. In Arabic, the brightness range read `10–10,000` and the saved
400-nit value remained visible.

Strict Apple Silicon cross-target Clippy passed with the AppKit alignment
feedback connected. Physical trackpad feedback and native macOS interaction
remain to be verified.

- Windows application SHA-256 `f8c2fc6accc0547cd99af3afd7e626cda71100163bdd6ad6e3e8d777ff4cd4af`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`
- Linux application SHA-256 `d26234beae74eca1b00800de9206c2da2199d01415f5659b80dd79e3290b02fa`
- Linux libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`

### Subtitles, Network and Control Settings localization 2026-11-24

The Russian and Arabic catalogs cover the three pages and their search
results. Searching for the subtitle paths opens the Advanced disclosure.
Search takes the Network feedback results to the switches near the bottom of
Cache and Control results to their input group. The subtitle auto-load and
encoding selectors preserve the same item order and engine values across
languages. The Russian Key Bindings navigation label fits the narrow sidebar.

The Windows workspace gate passed strict Clippy, 641 application tests,
79 Python checks and all 51 software-rendered visual comparisons. Review
updated two baselines. The Subtitles image shows the default auto-load choice,
and the Network image shows the startup buffering switch centered in its row.
Strict Clippy passed for the Apple Silicon application target. Native macOS
rendering and interaction remain to be verified.

The locked Windows workspace built successfully. On a private Windows
desktop, native accessibility search reached the subtitle paths and encoding,
the seeking indicator, and the right-click action. The Windows test saved and
restored the seeking indicator, kept the player paused at the same frame
geometry, and exited normally.

The locked Linux application ran both languages in an isolated Wayland
session at 600 × 480. Search opened the relevant controls. Editing the
subtitle path, choosing an encoding, toggling the seeking indicator and
changing the right-click action all saved the expected values. Both player
windows retained their frames and exited normally. The Arabic capture shows
the encoding identifier and extractor syntax in left-to-right order. The
Picture-in-Picture mouse choice still truncates in both narrow menus.

- Windows application SHA-256 `079725caf9ed9106c42d3489cfe586248bb2235beb98f119c6d2ba6c12d90c3d`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`
- Linux application SHA-256 `6ad6d14242db08d34868ab168cab5b9911cdee845f4bb9e2991da6d959739d13`
- Linux libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`

### Key Bindings, Advanced, Plugins and Utilities localization 2026-11-24

The four pages expose localized search results. Activation reveals the
settings group and leaves the choice or command to the user. The Plugins
page scrolls the installed list, and plugin information and permission
reviews translate the application text while preserving package metadata.
The Utilities confirmations keep the current cache size in the message.

The software-rendered comparison reports differing baselines together.
An isolated validation withheld the Advanced and Utilities baselines.
A single run failed with both filenames and saved both actual images.

The final Windows workspace gate passed strict Clippy, 643 application
tests, 79 Python checks and all 51 software-rendered comparisons.
Strict Clippy passed for the Apple Silicon application target. Native macOS
interaction remains to be verified.

The locked Windows workspace built successfully. A private desktop test
used accessibility to find controls on all four pages and saved Display
raw values. A local fixture requested the Show alerts permission. Declining
the native review left the profile without a grant. Both private test players
kept their paused frames and exited normally.

At 600 × 480 on an isolated Wayland desktop, both language runs created and
saved an assigned keyboard profile, changed and saved a log level, scrolled
to the eighth local plugin, and read its translated permission. Both opened
and cancelled the thumbnail confirmation and exited normally. The shorter
Picture-in-Picture labels fit in both menus, and the Arabic confirmation
showed the size as `343.0 KiB`. Some built-in key-binding action descriptions
and runtime messages still appear in English.

- Windows application SHA-256 `70e40aaa8af78aac91466357384941d885b8fcdd73071539ffad2c4040642897`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`
- Linux application SHA-256 `6275bca273aa11d0c9cde6345f61f4dafa4c6665e98a5d2a69ad171e07ba5924`
- Linux libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`

### macOS Touch Bar playback controls 2026-11-24

Each player window owns its own AppKit bar, with playback and time displays,
a timeline using the player's sampled thumbnails, and optional transport and
volume controls. The second clock defaults to remaining time and shares its
saved mode across windows using the same profile. The existing clock precision
and speed scaling apply to both places. The fullscreen Escape replacement
follows the active keyboard profile.

The full Windows workspace gate passed strict Clippy, 643 application tests,
51 software-rendered comparisons, and 79 Python checks. The locked Windows
workspace and Linux x64 application builds passed. The full Apple Silicon
application source passed strict Clippy across all targets in the app crate.

The Windows native engine suite passed all 63 cases. The new null-output
playback test accepted an exact seek on the current file, then retained the
replacement file's position and paused state when the previous file's seek and
resume commands arrived.

Physical Touch Bar layout, customization, gesture delivery, and the handoff
between active macOS windows still need a native desktop session. Rewind and
Fast Forward currently use the app's existing speed controls. The configurable
IINA on-screen arrow actions remain open.

### Configurable controller arrows 2026-11-24

Interface Settings offers playback speed, playlist navigation and 10-second
seeks. The on-screen arrows and Touch Bar share the selected action. The Music
Mode arrows move between playlist entries. The speed tests cover
direction changes, short and long presses, and changes in pressure during a
press. The application test covers all three command routes and keeps the
controller visible while an arrow is held.

The full Windows workspace gate, all 63 native playback tests, the locked
Windows and Linux builds, and the strict Apple Silicon source check passed.
The new Settings row changed eight pixels on the Interface scrollbar in the
software-rendered suite. The updated comparison preserves the 750 × 670 capture
size. Physical macOS Force Touch and Touch Bar delivery remain open.

On a maximized player in a private Windows desktop, a short pointer press
retained 2× speed. A long press on the opposite arrow restored 1× after release.
Settings search reached the arrow selector. The native selection saved Seek
and Playlist modes. Pointer presses in Seek mode moved the paused clock from
0 to 10 seconds and back. Playlist mode moved to the next video and returned.
The Settings and player windows closed, and the test process exited normally.

An isolated Wayland desktop used the saved playlist mode to switch from the
first paused video to the second and back. The 900 × 540 frame and window
identity stayed the same. The tested process exited normally.

- Linux application SHA-256 `a58c129e843434dbb3d98bc5ca4e7233170ac0311c518244206ded752e18a697`
- Linux libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`
- Windows application SHA-256 `31fbfa27eec0f47c869847f31d61bd39a4e2b7a951d04e52265886ca82ef51a3`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`

### Retained final thumbnail 2026-11-24

The pinned Windows engine reproduced a thumbnail extraction failure on a
35-second synthetic clip. The decoder had delivered 100 samples through
34.650 seconds when the final capture hit the Live Text pause check. The same
clip completed all 101 samples through 34.999 seconds using the dedicated
thumbnail capture request.

The native thumbnail test now extends the existing fixture into that 35-second
clip and checks the final timestamp. All 63 native tests passed against the
pinned Windows engine. The scoped Windows source gate, both locked application
builds and the strict Apple Silicon source check passed.

An isolated Wayland application run wrote 101 JPEGs and a manifest for the
same clip in its own cache. The manifest covered the final sample at 34.999
seconds. The test window closed and the application exited normally.

- Linux application SHA-256 `3211a854a947d88f051faaeaa57916b334cbef7e6ae0a4af5b984d08f40e32d1`
- Linux libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`

### Touch Bar and controller platform boundary 2026-11-24

The adapter owns Touch Bar registration, AppKit pressure and alignment
callbacks, and the macOS Settings form connection. The shared timeline cache
supplies both consumers. The cache regression verifies that a snapshot-bound
lookup rejects a replacement file or a file still loading, and that the
timeline still finds the preceding sample.

The full Windows workspace gate and the locked Windows and Linux builds
passed. In a private Windows desktop, Settings saved both arrow modes, the
player sought forward 10 seconds and back, and the playlist returned to the
first video. The isolated process exited normally. In an isolated Wayland
desktop, the cache stored all 101 JPEGs through 34.999 seconds and the player
exited normally. The Apple Silicon and Intel Mac targets passed strict Clippy.
Native macOS playback, Touch Bar delivery, and physical pressure feedback
remain open.

### Now Playing timeline artwork 2026-11-24

Now Playing uses the completed thumbnails of the current media when neither
cover art nor Quick Look is available. The shared cache regression covers
incomplete and superseded media, timestamps between samples, exact timestamp
boundaries, and the first frame. Artwork requests can be cancelled without
joining the extraction worker on the UI thread.

The full Windows workspace gate passed, including the headless player and
Settings pixel comparisons. Strict application Clippy passed for Apple Silicon
and Intel Mac. The locked Linux application build ran in an isolated Wayland
session, persisted all 101 JPEGs through 34.999 seconds, and exited normally.
Native macOS still needs a Control Center check for artwork priority, media
replacement during Quick Look, and window shutdown while extraction is pending.

- Linux application SHA-256 `54abb5f4af62b4cd7e06bf352024417277e988837b4d19516620b7df7123d08d`
- Linux libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`

### macOS Now Playing window ownership 2026-11-24

The application registers one MediaPlayer endpoint. Switching players changes
the playback state and artwork source together. The focus-selection regression
covers two players, focus moving outside the player windows, and falling back
to the previously focused player after the owner closes. The remote-command
identity regression covers both a window change and a new file in the same
window. Sleep notifications come from the workspace notification center.

Both regressions passed in the full Windows workspace gate, alongside the
headless player and Settings pixel comparisons. Strict application Clippy
passed for Apple Silicon and Intel Mac from Windows.

Native macOS verification still needs two players playing different files,
media keys and Control Center controls during a focus switch and file change,
owner shutdown, app deactivation and reactivation, and a sleep/wake cycle.

### Single-file folder discovery 2026-11-24

The 64 pinned Windows engine tests passed. The folder regression covers one
open versus a grouped open, a manual append that overlaps discovery, a stale
scan after navigation, and an item removed from the same folder. The worker
compares ordinary and verbatim Windows path spellings when excluding duplicates.

An isolated Windows desktop launched the middle of three clips. The native
playlist showed both neighbors in natural order. The test opened the sidebar,
selected the first clip, removed it with Delete, and played the last clip
with Return. The playlist retained the removal and playback remained paused.
The exact test process exited normally.

- Windows application SHA-256 `7e40ca5ccf92a3e18cad580116f76448e86fb1eb8f8a7f3eef23aee97ecf954a`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`

### Playlist keyboard and clipboard 2026-11-24

The main sidebar and Music Mode expose focused Cut, Copy, Paste, Select All
and Delete actions. Paste resolves a destination from the observed playlist,
expands local folder references off the UI thread and inserts before the
selected row. Inserts retain the focused row as its index changes. After Cut
or Delete, keyboard focus can move to an adjacent surviving row. AppKit's
Edit menu checks for a focused text input before routing an edit to the playlist.

The Windows workspace gate passed. Its 69 widget checks include editing keys
in both playlist surfaces and Up Arrow after an insertion shifts the focused
row's index. The headless player and Settings pixel comparisons passed.
Apple Silicon and Intel Mac application source checks passed for the Edit menu
integration before the final shared focus and menu shortcut changes. Native
AppKit clipboard and menu validation still need a macOS session.

The final shared-menu change passed strict Clippy, all 70 widget checks,
and the headless pixel comparisons in the scoped Windows gate. The new
widget check covers a menu accelerator that also serves as a playlist key.
In an isolated Linux X11 desktop, a paused player copied and pasted a row,
inserted a native file-list folder in natural order, cut and restored a row,
and removed the two occurrences of a sibling before advancing. Keyboard focus
stayed usable between edits. Playback stayed paused at the original volume.
The removed sibling stayed out of the playlist and the process exited normally.
The same binary passed an isolated Sway Wayland session with XWayland
disabled. A native GTK window offered a file-list folder. Riina inserted its
clips in natural order before the chosen row. The test also exercised Copy,
Cut, Paste, Delete and Return in the sidebar, kept the original volume and
paused state, and observed normal exit of the exact player process.

- Linux application SHA-256 `4acfc31f3f6ffde736fa4f8f7e502d16653574a128fcd4bf3a5d0077a377fc89`
- Linux libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`

### Native playlist clipboard formats 2026-11-24

Local file selections now publish both a file list and plain text on Wayland
with data-control support. The file list uses CRLF framing and percent-encoded
file URLs. Paste decodes native Wayland data before handing paths to the
playlist worker. Windows publishes `CF_HDROP` alongside `CF_UNICODETEXT` in
one clipboard open. Both send plain text for selections containing network
references. X11 retains plain-text Copy and accepts file-list Paste.

The Windows workspace gate passed, including strict Clippy, 70 widget tests
and the headless pixel comparison. The Apple Silicon application passed a
source cross-check. The host denied creation of a separate Windows window
station for the clipboard test. Live Windows and AppKit clipboard verification
remain open.

In isolated Sway with XWayland disabled, a native GTK consumer read the exact
URI list and plain-text representations of a filename containing spaces,
`%` and `#`. Riina pasted the copied row before the selected row, expanded
a GTK native folder offer in natural order, and completed Cut, Paste, Delete
and Return without changing pause or volume. The same binary passed the X11
playlist workflow. Each test player exited normally. Three focused Linux
tests passed, covering CRLF framing, filenames with encoded line breaks and
special characters, unsupported file authorities, and the X11 fallback.

- Linux application SHA-256 `41a9c8dfd26a3834d9eb3e29401bc024835fe04a142c5b0912c26b4e571dd202`
- Linux libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`

### Edit menu focus and routing 2026-11-24

Edit commands on Windows and Linux use the same text editor or playlist
row as keyboard editing. The Linux menu restores the original focused item
and its text selection when it closes. Native Windows menu activation
preserves the playlist target while the menu owner has keyboard focus.
The framed video owner updates the live Edit submenu in place.

The Windows workspace gate passed strict Clippy, 650 application unit tests,
70 widget checks and the headless player and Settings pixel comparison.
New unit tests cover selection retention during native window activation
and an in-client popup, and dispatch to the restored playlist row. The
Apple Silicon application passed a source cross-check. Native AppKit menu
interaction and matched reference pixels remain open.

The same Linux binary passed isolated X11 and native Sway sessions. The
tests selected Copy and Delete using the Edit menu and continued with
keyboard playlist navigation, folder Paste and Cut. GTK read the file list
and text from the Wayland copy. Playback kept its pause state and volume.
The exact test player exited normally in each session.

Isolated Windows sessions on the shared renderer and the default video
renderer verified that Delete was disabled without a focused editing target.
The native Edit menu enabled Delete after row selection, removed the row,
and allowed Return to start a remaining clip while paused. Both test
processes exited normally. The final native popup change passed the
affected application and UI package gate, including both new unit tests,
all 70 widgets and the headless pixel comparison.

- Linux application SHA-256 `139d08e45d6a049d89bf6481517c6e98640f77ba9e095cddc89dd594212329f9`
- Linux libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`
- Windows shared-renderer application SHA-256 `09a1df16164271ae2da364a485c36ee4150ad186dfb0ff1d7c85217a3b225ccc`
- Windows default renderer application SHA-256 `f46c060b3301a16d1a5b15c3b9055e5b3c6628343d537edaf827f4da9e53d780`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`

### X11 playlist clipboard formats 2026-11-25

Linux passed strict Clippy, 616 application unit tests and 67 widget tests.
X11 and Wayland use the same file URL encoding, with regression coverage for
escaped carriage returns, line feeds, Unicode and special punctuation.

In isolated X11, a GTK consumer read `text/uri-list`, Unicode text and the
file-manager format after choosing Copy from the Edit menu. The sampled
filename contained spaces, `%` and `#`. A native `MULTIPLE` request preserved
both supported results and rejected an unsupported target in the same request.
An external GTK folder offer remained usable after Riina owned the selection.
Cut removed the playlist row and kept its file available as Copy for external
consumers. An isolated test clipboard manager retained all three formats
after Riina exited.

The same application binary passed the Sway Wayland playlist workflow with
XWayland disabled. File-list and text export, native GTK folder Paste,
focused row editing and normal shutdown all passed on both displays.

- Linux application SHA-256 `83d39c032ddc366198f187fc2b1bf1dae5801c1895c75300a8fb7a24d86326f3`
- Linux libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`

### Windows multi-file opening 2026-11-25

Two isolated sessions used the default video renderer and the native File
menu and picker. With grouping enabled, selecting two videos created one
new player whose playlist contained both entries. The original player
remained open. With grouping disabled, the picker created two independent
players within the same process. Opening one of those files again with
duplicates disabled kept the existing three windows. Each application
exited normally after its windows closed.

- Windows application SHA-256 `f46c060b3301a16d1a5b15c3b9055e5b3c6628343d537edaf827f4da9e53d780`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`

### Playback History file availability 2026-11-25

The History outline checks visible local files on background workers.
Windows UNC shares use a separate worker so an unreachable share cannot
delay checks on the local drive. Filtering or closing the outline discards
obsolete results. Confirmed missing entries hide saved progress. History
keeps the archived reference for search and selection and opens local media
with a decoded path.

The affected Windows application gate passed strict Clippy, 651 application
unit tests and 70 widget checks. The scheduler regression covers obsolete
results after filtering and closing. The outline regression covers pending
and missing files, saved progress and decoded paths.
The full Apple Silicon application passed a source cross-check. Native
macOS interaction and matched visual checks remain open.

An isolated Windows session used the default video renderer. A present file
with spaces, Unicode and a percent sign displayed its saved progress and
played from History. A missing file hid progress and ignored Return.
Restoring that file and reopening History made its progress and playback
available in the same player. The exact test process exited normally.

The full Linux build passed the History confirmation workflow on isolated
X11. Cancel and closing History preserved the file. Confirmed Trash moved
only the selected temporary file and retained its history row. Remove and
Clear then updated the archive as requested. The test process exited normally.

- Windows application SHA-256 `4cdc2c9075866fda09f6fb49459d35a5d8a1610d8c76ab24afca5ba8ffde36f8`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`
- Linux application SHA-256 `58c0d1bf26c4901b0245341169a6f5cd3ed885eb6dbac48aff9a5e5fe52184c4`
- Linux libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`

### Playback History saved progress 2026-11-25

One worker per profile reads mpv's watch-later files outside the playback
and outline locks. Opening History schedules a fresh read. Before a player
reports its filename-hashing option, History uses the profile's inline
Advanced option or the default. Confirming Clear Saved Playback Progress
discards in-flight results and clears the displayed positions.

The affected Windows gate passed strict Clippy, 652 application unit tests
and 70 widget checks. The progress tests cover reopening without a player,
refreshing changed positions, the observed engine option taking priority
and rejecting a result read before Settings cleared it. The Apple Silicon
application source check passed. Native macOS interaction remains open.

An isolated Windows session opened History with two archived entries.
History displayed the position in the watch-later file and showed its new
value after reopening. Cancel in Settings Utilities preserved the file
and the displayed position. Confirm removed the file and the progress
label from the still-open History window while keeping both archive entries.
The exact test process exited normally.

The full Linux application build passed the existing isolated X11 Utilities
workflow, including cancellation, separate clearing of progress, history
and thumbnails, an unavailable progress directory, reopening Settings and
normal exit.

- Windows application SHA-256 `689eadcfed168f91b61fe77d53df375302269c9911861388a9f8f62c587be55a`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`
- Linux application SHA-256 `2d7fd8f2aa8bd2b4c6ba4fc419a4bacb1e964ea1d41cb441c561f155cb5e861e`
- Linux libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`

### Pause When Open during automatic playlist advancement 2026-11-25

The playback worker applies Pause When Open after a file ends naturally or
fails. It waits for libmpv to acknowledge the pause before releasing the
end-of-file hook. Explicit opens, redirects and manual navigation retain
the pause policy of their own commands.

The affected Windows source gate passed strict Clippy, 652 application
unit tests, 39 engine unit tests and 70 widget checks. All 65 tests in the
pinned Windows libmpv suite passed. The new regression advances from video
to audio, verifies the audio starts paused, then disables the preference
and verifies automatic advancement to video resumes playback. The Apple
Silicon application passed a source cross-check.

Separate Windows and Linux X11 desktop sessions exercised automatic
advancement with the setting enabled and disabled. With it enabled, the
next clip appeared in the same player and stayed paused at 0 seconds.
With it disabled, the next clip continued playing. Each exact test
process exited normally. Native macOS playback remains unverified.

- Windows application SHA-256 `82213355b444362945f46799c8918c75142f252598c2b0ea43f580fc6c52f843`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`
- Linux application SHA-256 `0fdd5b4227601bfdc2f62e3afd197591b133c94309f569d98642594f3cb654df`
- Linux libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`

### Playback History regional dates and times 2026-11-25

Date mode groups visits by local day, shows a medium-format heading, and
displays only the time on each media row. Folder & Website mode shows a
localized short date and time. Stable group keys keep collapse and ordering
independent of the displayed labels. macOS uses Foundation date formatters.
Windows and Linux use ICU4X, with the Windows clock preference and Linux
`LC_TIME` selection applied. The operating system and locale data can
produce different separators and year widths.
[ICU4X date and time formatting](https://docs.rs/icu_datetime/2.3.0/icu_datetime/),
[Windows clock preference](https://learn.microsoft.com/en-us/windows/win32/intl/locale-itime).

The grouped Windows gate passed strict workspace Clippy, the workspace
tests including 654 application unit tests and 70 widget checks, and the
headless player and Settings pixel comparison. Deterministic tests cover
12- and 24-hour formatting and grouping independently of row labels.

An isolated Windows desktop using the Irish regional format and a 24-hour
clock showed `6 Nov 2025` and `04:09` in Date mode, and
`06/11/2025, 04:09` in Folder & Website mode. The test exercised
individual collapse, Collapse All, Expand All and changing the grouping.
The exact test process exited normally.

An isolated X11 session used French date formatting and the
`Pacific/Auckland` time zone. Captures showed `6 nov. 2025` and
`5 nov. 2025` as separate local days, including a visit whose UTC date
was the previous day. They showed the expected rows after individual
collapse, Expand All, Collapse All and switching to Folder & Website.
The exact Linux test process exited normally.

The Apple Silicon application and test targets passed source cross-checks.
Native macOS date formatting and matched reference pixels remain open.

- Windows application SHA-256 `6fdc94b007725f0648ec7b98816b67a8ca762430a47707cb01dbe793876af80e`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`
- Linux application SHA-256 `272cb652d9bd3db9fe6f443b22702937bff0e0b49addf366ee1f19ca666def08`
- Linux libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`

### Quick Settings keyboard navigation and presets 2026-11-25

Theme is one Tab stop, following the selected segment. Left and Right move
between themes. Home and End select the first and last choices. Keyboard
focus reveals controls in both dimensions of the Layout viewport, including
the actionable end of a switch row that is wider than the pane.

Video's aspect, crop and rotation groups support one Tab stop each. Arrow
keys, Home and End move the focus and reveal it along the preset rail.
The Custom Crop choice opens its editor on explicit activation. Custom
aspect entry shares the other sidebar fields' draft and cancellation behavior.
Subtitle font size and outline width present the observed value separately from
the list of presets. Dismissing a preset menu leaves the displayed value intact.

The shared player gives visible Quick Settings controls first access to plain
navigation and activation keys. Keys a control declines still reach the
player's active binding profile. Modified menu shortcuts keep their existing
dispatch, and the Linux menu popup still lists the configured shortcuts.

The grouped Windows gate passed formatting, strict Clippy and 735 app and UI
tests, including the headless pixel comparison. Its 74 widget tests include theme
navigation with active playback menu bindings, both narrow Layout placements,
Video keyboard traversal while engine observations lag, and Subtitle size
menu dismissal, activation and focus reveal.
The pinned Windows libmpv suite passed all 65 tests with null audio output.

Two isolated Windows 11 sessions exercised the leading and trailing Layout
panes in 285 × 310 logical-pixel clients at 300% scaling. Theme changed through
arrow keys, Home and End. Tab moved from Theme to the first switch and
continued to the lower Plugins placement control, which responded to Right
Arrow. The fixture stayed paused and both test processes exited normally.

A full Linux x64 application build ran the same keyboard sequence in a
551 × 310 X11 client. The saved Theme values followed the keys. Captures
show the lower Plugins control revealed, focused and changed by Right
Arrow. The test process exited normally.

The Apple Silicon application and test targets passed source cross-checks.
Native Video and Subtitle control checks, macOS runtime and matched IINA
pixels remain open.

- Windows application SHA-256 `086763836e98b2b470bb360643bf5f5b59f40dd76ee541ed81ed3a46804d5161`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`
- Linux application SHA-256 `47f2fea0590f73371b52c5fd9f8beefe0349ae2b3d1f8ca977c984e115385842`
- Linux libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`

### Subtitle color controls and sidebar shortcuts 2026-11-25

Quick Settings and Settings use AppKit's shared color panel on macOS. The
active well owns updates to text, outline or background. Selecting a new
well detaches the old target before setting the initial color. Closing an
owner dismisses its panel, while closing an earlier owner leaves a newer
owner's panel available.

The portable color editor starts keyboard focus on Red. Tab and Shift+Tab
move between channels, and Escape dismisses it with live edits retained.
While editing a numeric field, unhandled Control or Command shortcuts
reach the player with the numeric draft intact. Alt and AltGr input stays
with the editor.

The grouped Windows gate passed formatting, strict Clippy, and 738 app
and UI tests. The widget group ran 77 tests, including portable color
editing and typed requests for the AppKit panel. The headless pixel
comparison and all seven pinned libmpv subtitle tests passed.

A full Linux x64 build accepted keyboard changes in a 551 × 700 X11
client. The engine reported a 4:3 preset, a custom 2:1 aspect, a managed
crop filter, and 90-degree rotation. The video fixture stayed paused and
the test process exited normally. A second X11 session exercised the
subtitle styles. The color editor started on Red and Tab advanced to
Green. Escape kept both channel changes, preserving the outline color.
The preset menus applied font size 60 and outline width 5. The second
fixture stayed paused and its process exited normally.

Private Windows 11 sessions confirmed that Right Arrow on the aspect rail
applied 4:3. The subtitle editor opened on Red, and Left Arrow changed the
engine color from `#FFFFFFFF` to `#FFFEFFFF` while preserving the outline
color. Escape kept the edit. Keyboard selection in the preset menus applied
font size 25 and outline width 0.25. The exact test processes exited normally.

The Apple Silicon application and test targets passed source checks.
AppKit target switching and panel dismissal still need native macOS checks.

- Windows application SHA-256 `e4e6de4cdad4f60bdafffb2e08d31534ecd1733ef6c7a89c754bcc35047adb54`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`
- Linux application SHA-256 `3e239bc9e5f2ff0896c0b9013381c8f6d1c9ab333936581067cf4a406b3564d2`
- Linux libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`

### Playlist and Chapters in Music Mode 2026-11-25

The compact player uses the same Playlist and Chapters tabs as the main
window. The playback menu opens the requested tab, closes it when it is
already active, and switches between tabs without resizing the window. The
toolbar reopens the last active tab. Each list keeps its scroll position
across presentations, including when Music Mode shows taller metadata rows.
Chapter rows show a title and time range, with a
separate marker for the playing chapter. The keyboard moves the selection
through the list, and Enter or a double-click seeks and resumes playback.

The full Windows workspace gate passed formatting, strict Clippy, 1,073
Rust tests, 79 Python tests and four browser-extension tests. The application
tests cover menu routing, the row-height change between surfaces, and
keyboard selection and activation. The software pixel comparison covers
Playlist and Chapters in both themes. The pinned null-output libmpv suite
passed all 65 tests.

On a private Windows 11 desktop, the native Playback menu displayed Hide
Chapters Panel while Chapters was visible. In Music Mode, both a pointer
click and the toolbar accessibility action reopened the remembered tab.
Down Arrow moved the selection with playback paused. Enter sought to the
second chapter and resumed
playback. The tab buttons switched between Playlist and Chapters at the same
frame size. The toolbar hid and restored the active tab. Returning to the
main player kept the same loaded media and the newly paused playback state.
The exact test process exited normally.

The locked Linux x64 application ran the same two-chapter fixture in a
private X11 session. The keyboard switched between Playlist and Chapters
at 300 × 390 and closed the panel to 300 × 90. Enter sought and resumed
playback after Down Arrow moved the selection. Returning to the main player
restored its 850 × 580 frame and retained the paused fixture. The exact
process exited normally.

The Apple Silicon application and test targets passed source checks.
Native macOS playback and matched IINA visuals remain open.

- Windows application SHA-256 `a0a73866801029d204333bbf728f1f4b386025a06c5a6ce8c0b763fc6a6eca7d`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`
- Linux application SHA-256 `674410f854581c9776b9bd9de9ab210b6b231df5b45facc84dbdf502483ac631`
- Linux libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`

### Compact player resizing and profile choices 2026-11-25

A resize shows the Playlist or Chapters panel in the space below Music
Mode's artwork and controls. On release, less than 200 points of available
space rolls up the panel. At or above the threshold the panel keeps the
dragged height. Toggling artwork retains that height, and an explicit
reopen uses the 300-point default. The Windows adapter fixes the client
width during the native gesture. The shared adapter restores that width
when settling the window.

Toolbar and Playback menu choices save their visibility preference
immediately. A resize changes the current window's visibility and leaves
that profile value available for the next player.

The full Windows gate passed formatting, strict Clippy, 1,076 Rust tests,
79 Python tests and four browser-extension tests. The grouped widget and
pixel checks passed, and the pinned null-output libmpv suite passed all
65 tests.

On a private Windows 11 desktop the native sizing messages kept the
client 300 points wide, rendered the tabs during the resize and accepted
the panel at exactly 200 points. The panel retained its custom height
when switching tabs or hiding artwork. Showing the video artwork added
169 points. The toolbar saved the choices immediately, and a drag closed
the panel without rewriting the saved value. Returning to the main player
restored the measured 720 × 499 client and retained the paused media.

The locked Linux x64 application passed the isolated X11 workflow. The
configured 289-point client rolled up to 90 points. At 290 points the
panel stayed open, and a larger 440-point client survived a return to
the main window and reentry into Music Mode. An explicit reopen used the
390-point client. The saved visibility survived a subsequent resize, and
the original 850 × 580 main client and paused media returned.

In a private Sway session the same application used native xdg-shell
surfaces. The test kept the floating window on the output and used a
mapped F8 shortcut to switch modes. It confirmed the same threshold, a
300 × 410 custom client and restoration of the 850 × 580 main client.
A full-output capture showed the restored paused video. The compact
capture showed an empty area between the in-client menu and the tabs.
Compact controls and metadata still need a matched Wayland layout check.
The isolated Windows, X11 and Wayland processes exited normally.

Strict Apple Silicon cross-Clippy passed for the application and test
targets. Native macOS behavior remains open.

- Windows application SHA-256 `7e828ed8e8fe75f6d1ce60df458b1da96cf6e7858b25e8796615e3e4e01823d9`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`
- Linux application SHA-256 `0c6fb77ea21d2e454200dee74e41a1bde310767c86386366ce502ae1a5237c30`
- X11 libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`
- Wayland libmpv SHA-256 `cdce854e1811942602587310026306d92d30c0f593f2be676f07659ae43061d8`

### Compact volume and audio-only subtitles 2026-11-25

The pinned MiniPlayer expands a slider and numeric readout from the mute
button. Scrolling the artwork opens that control when the configured axis
changes volume. Coarse wheel input uses the OSD timeout, and AppKit supplies
trackpad begin and end phases. Riina keeps these paths in the player's wheel
dispatcher and also uses a timeout when a Linux or Windows input stream
has no distinct gesture phases.

The pinned player sends its on-screen messages through the main window.
Music Mode dismisses active messages and clears queued feedback while
retaining the profile's on-screen display setting.

The pinned player permits subtitle-only frames after its libmpv render
context exists. Riina's shared presenter sends the readiness signal through
the playback queue. D3D11 uses the embedded Windows output at media open.
Audio remains classified by its real tracks, and both paths honor an
explicit force-window option.

The full Windows gate passed formatting, strict Clippy, 1,081 Rust tests,
79 Python tests and four browser-extension tests. The two new software
rendering frames cover the open volume panel in light and dark themes.
All 65 pinned null-output engine tests passed separately. Apple Silicon
cross-Clippy passed for the application and test targets. Native macOS
desktop verification remains open.

The isolated Windows 11 application used the embedded D3D11 output with
no video track and observed both selected cues on the paused audio item.
The artwork control rolled the 300 × 390 client down to 300 × 90 and
restored it with both captions still selected. Next Media returned to
paused video without inheriting the audio item's subtitle tracks. The
first posted pointer click expanded the volume panel. The second posted
click did not mute on the private desktop, so the test verified mute and
unmute through the button's accessibility action. The full Windows
desktop capture returned an invalid handle. Physical Windows pointer
input and rendered caption pixels remain open.

The full-output X11 and native Wayland captures show both caption lanes
on the same paused audio fixture. On X11, hiding both lanes restored the
placeholder. A wheel step raised volume from 72 to 76 and showed the
numeric readout without a separate on-screen message. The readout closed
after the configured timeout. The same pointer button muted after opening
the panel. Artwork toggles kept the caption assignments, and advancing
restored the normal 960 × 600 main client with paused video. The isolated
Windows, X11 and Wayland processes all exited.

- Windows application SHA-256 `2db965cce9b3053e142b80583da0e3ee5d417d63ce379d5205d9813d43938e99`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`
- Linux application SHA-256 `ea536e2e2ef9de504a358f9a970a502b58938c61fa53d8ece0b0eb6647fbefae`
- X11 libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`
- Wayland libmpv SHA-256 `cdce854e1811942602587310026306d92d30c0f593f2be676f07659ae43061d8`

### Automatic Music Mode and video window sizing 2026-11-25

Automatic switching uses a single deadline for a pending track catalog
and cancels it when the catalog changes. A replacement file gets its own
deadline even when both files are audio. On return to the main player,
the window owner restores the frame before applying the pending video's
resize policy. Playback targets used solely to draw subtitles on audio
files do not count as decoded video dimensions.

The previous X11 binary did not reach the video's natural client size with
Always resize enabled. The candidate entered a 300 × 390 compact
client on paused audio, retained both caption lanes, and returned to the
first video at 320 × 180. After enlarging the normal window to 720 × 460,
a second audio/video sequence again used a 320 × 180 video client. Native
Wayland kept the same xdg-shell surface and reached the same client size.
The isolated 1280 × 800 X11 and Wayland captures showed visible video on
return, and the player processes exited.

The Windows 11 private desktop run verified both selected captions, the
300 × 90 artwork roll-up and 300 × 390 restoration, and Next Media to the
first paused video at 320 × 180 logical points. The isolated application
closed cleanly. The existing Windows safeguard keeps established D3D11
viewports stable across later automatic playlist changes. A full Windows
desktop capture returned an invalid handle, so Windows composited pixels
and physical-display output remain open.

The locked Windows workspace and Linux x64 application builds passed.
The full Windows gate passed strict Clippy, 1,082 Rust tests including the
headless pixel comparison, 79 Python tests and four browser-extension tests.
All 65 pinned null-output engine tests passed separately. Strict Apple
Silicon cross-Clippy passed for the application and test targets. Native
macOS desktop verification remains open.

- Windows application SHA-256 `af3637d89a32025aedb5caef4eb592a8e1dfa7aa0aed7ea33d9abd5070697637`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`
- Linux application SHA-256 `a5457d575f5c3901d48b737b8d8fe31b1010c78cc1e3c4989a640ea9937bb01b`
- X11 libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`
- Wayland libmpv SHA-256 `cdce854e1811942602587310026306d92d30c0f593f2be676f07659ae43061d8`

### macOS media polling and desktop drop routing 2026-11-25

Now Playing schedules its existing 50 ms refresh while the selected player
publishes an item. Focus changes, playback readiness, the system media
preference and native events wake the service when it has no current item.
PiP starts polling after a presentation request and stops when AVKit has
settled and its last sample has been retired. The 100 ms on-screen and 33 ms
offscreen intervals are preserved. Headless tests exercise the timer cadence,
retirement and later restart, as well as the Slint playback and preference
signals.

The player and Welcome window now use the same desktop drop registration
without choosing a backend. Windows attaches its framed owner and controls.
AppKit attaches the content view, Wayland owns a protocol route, and X11
receives drop events through the window loop. The portable player also binds
system media and the fullscreen event observer through common signatures.

The full Windows gate passed formatting, strict Clippy, 1,084 Rust tests
including the software pixel comparison, 79 Python tests and four browser
tests. Windows tests exercised OLE file and URL data objects, registration
revocation and a drag crossing from the video owner to the controls. The
locked workspace build and all 65 pinned null-output tests passed separately.
The isolated Windows application passed ten playback, controls, fullscreen,
minimize and restore checks, then exited. A live Windows drag and physical
display output remain separate checks.

Strict Apple Silicon cross-Clippy passed for the full application and test
targets. AVKit presentation, Control Center integration and macOS shutdown
still need a native desktop run.

The locked Linux application accepted Xdnd file drops into the visible and
temporarily revealed sidebar, and into the Music Mode playlist. Cancelling
an Xdnd drag left the playlist intact. A GTK source on native Wayland inserted
files into the sidebar and Music Mode. Both runs preserved paused playback
and existing row order, then closed the exact player processes. The desktop
probes used the visible playlist rows above the compact footer.

- Windows application SHA-256 `084d41cfa30f2258a546acc0630e15281ea6f8f516e023f71d62b75232583fa1`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`
- Linux application SHA-256 `311b9bbccbb9eb4da47a9431f0bafc85ae0464fc5d0da0ec906d99a518274902`
- Linux libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`

### Application plugin platform boundary 2026-11-25

The application coordinator routes actions from both player and windowless
menus. Idle actions still carry the global entry generation and are discarded
after reload. The AppKit presenter opts into the menu snapshot and the
standalone-window change counter. The player-only route uses its existing
menu publications and window state. Native shortcut ownership, close
notifications and windowless alert delivery belong to the platform adapter.

The existing host scenario runs with the windowless presenter disabled and
verifies both player and global actions reach their owning realms. The
windowless host scenario explicitly enables the presenter and checks startup,
menu updates, stale actions after reload and resource retirement. The
standalone-window test enables change tracking to cover closure after a
presentation failure.

The Windows workspace gate passed 1,084 Rust tests including the headless
pixel comparison, 79 Python tests and four browser tests. Strict Clippy, the
locked Windows and Linux builds, and all 65 Windows null-output engine tests
passed. Strict Apple Silicon cross-Clippy passed for the application and test
targets. The application menu, ownerless alerts and quit policy still need a
native macOS desktop run.

On an isolated Windows desktop, the startup prompt and both developer
consoles worked during paused playback. The global window restored its
requested width, retained its other frame axes and reopened six times from
the native Plugin menu. Closing the player tool or console left the global
one open. Closing the player then retired the exact process. On native
Wayland the same global WebView survived closing the first player, opened
Settings through the remaining player and accepted its Quit shortcut.
An X11 rerun waited for native focus before answering the startup prompt.
It kept the same global tool across the first player's close, opened Settings
and quit through the surviving player. Both Linux test processes exited.

- Windows application SHA-256 `121784d5fc9759d44d999d7628106f33cfde50486308a71963f6dae506ab4a9d`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`
- Linux application SHA-256 `85ffbf60799cc978b70496c678c49b0dca9337abad0cc999869d64c35d08482d`
- Linux libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`

### Application open and last-window lifecycle 2026-11-25

The platform registration owns AppKit launch events and the native idle
event loop. The shared player manager now defers automatic quit while a
Finder batch, application picker or background file preparation is pending.
Opening a replacement player clears the deferred close. The macOS policy
preserves the idle menu when a hidden plugin player or a tool closes without
a prior visible player close.

The preceding X11 application briefly mapped a replacement Welcome window
with automatic quit disabled, then retired it before it could receive focus.
The shared event loop now waits for the window manager to resolve the close.
The manager explicitly quits after the last Welcome closes and retains the
native idle menu on macOS.

The Windows gate passed strict Clippy, 1,088 Rust tests including the headless
pixel comparison, 79 Python tests and four browser tests. The portable state
tests cover multiple pending opens, picker cancellation, idle tools, hidden
players and late replacement players. The locked Windows and Linux builds
passed. All 65 pinned Windows null-output engine tests passed separately.
The isolated Windows binary passed playback, fullscreen, minimize, restore and
shutdown checks. Apple Silicon cross-Clippy passed. Finder delivery, the
native application picker and quit integration still need a Mac desktop run.

On X11 and native Wayland, the last player closed while Playback History
stayed visible. Closing History then exited with automatic quit enabled.
With automatic quit disabled, the same fixture reopened Welcome and stayed
open until Welcome was closed. On Wayland the probe opened History through
the File menu. All four isolated Linux player processes exited normally.

- Windows application SHA-256 `68a98100618cc9727a7799ce7d51b6c472742b4a09c929fd8ee81bba69c80644`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`
- Linux application SHA-256 `e40cf4b3657444a41d443431e7a893f90d2e255e7947949c0e118759310ec68e`
- Linux libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`

### Fullscreen and compact window platform boundary 2026-11-25

The platform adapters now own fullscreen requests, desktop acknowledgements,
Wayland geometry repair and native Music Mode resize observation. The shared
controller still waits for confirmation before changing playback or entering
Music Mode. The existing Winit filter delivers observations for both Linux
window systems.

The fast check now retains the UI pixel suite when a widget edit shares a
batch with another crate. Edits to the parity ledgers select ledger validation
directly. The workspace check passed strict Clippy, 1,090 Rust tests, 79
Python tests and four browser tests. All 65 Windows null-output engine tests
passed during the batch.

The locked Windows build passed the isolated playback and fullscreen smoke
test. That run checked native surface retention, keyboard enter and exit,
minimize, restore and shutdown. Apple Silicon cross-Clippy passed. Native
AppKit fullscreen and Music Mode interactions still need a Mac desktop run.

In the first Wayland run, a second windowed configure arrived after Music
Mode requested its compact size. The 320 × 180 client stayed windowed-sized
while the mode was active. The Linux resize adapter now checks each configure
against the compact target even if the previous event carried the same size.

The locked Linux build passed desktop- and app-initiated fullscreen changes
in private X11 and native Wayland sessions. Both restored the paused video
through a fullscreen-to-Music Mode handoff, expanded and collapsed the
catalog, and returned to the original client dimensions with the same media
and position. The mode events confirmed fullscreen exit before Music Mode
entry. Both processes exited normally.

A separate Sway probe that deliberately replaced the first fullscreen
configure with 320 × 180 left the video small despite one native retry. It
also reproduced on the prior application build. The 1280 × 800 desktop
capture confirmed a small centered surface. Recovery in that case remains
open.

- Windows application SHA-256 `4b36eff79709734108fce1dac650cd13fa1a52a4015334da31da6f918a868cd6`
- Windows libmpv SHA-256 `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`
- Linux application SHA-256 `0b15425f6d66a6eee9a073f06676f74844bf1c892db4c044def4e45c47cd4db2`
- X11 libmpv SHA-256 `84813585bde6083eed5b6ffd03934cf3537eca4852cabd4f37436a885a93b1cb`
- Wayland libmpv SHA-256 `cdce854e1811942602587310026306d92d30c0f593f2be676f07659ae43061d8`
