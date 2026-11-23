# Verification

Results apply to the named revision, environment and test type. A unit test,
a native decode test and a desktop playback check establish different things.
The latest records below are from Windows 11 x64; dates and exact scope are recorded per batch.

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

### Plugin sidebar manifest discovery 2026-10-14

Manifest tests cover bounded localized labels. Runtime tests cover confined
documents, messages, visibility, late activation and lifecycle delivery. A Windows
smoke displayed a package-owned sidebar page.

On the private Linux X11 display, the `63dfb72` executable displayed a local
plugin page using WebKitGTK. The page sent a readiness token to the plugin, and
the plugin's reply changed the page background. The full desktop capture
contained 16,321 pixels of the reply color. A window-manager close request
ended the player and its WebKit helpers. GTK logged frame-clock warnings.
Quit from a focused WebView, Wayland embedding and native macOS plugin
behavior remain unverified.

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
