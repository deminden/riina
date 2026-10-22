# Verification

Results apply to the named revision, environment and test type. A unit test,
a native decode test and a desktop playback check establish different things.
The latest records below are from Windows 11 x64; dates and exact scope are recorded per batch.

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

## Running checks

### Portable and workspace checks

```powershell
cargo fmt --all -- --check
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
smoke displayed a package-owned sidebar page. Linux X11 is source-wired; Wayland
embedding and native macOS/Linux application behavior remain open.

### macOS application menu composition 2026-10-14

The AppKit adapter consolidates application actions in one native menu while
retaining Services and visibility actions. A focused Apple-target probe compiled
all eleven macOS platform modules with strict Clippy. The full application check
from Windows stopped in a native dependency before application compilation.
Native menu interaction, exact order/shortcuts and Mac playback remain unverified.
