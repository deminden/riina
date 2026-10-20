# Verification

Results apply to the named revision, environment and test type. A unit test,
a native decode test and a desktop playback check establish different things.
The latest records below are from Windows 11 x64; dates and exact scope are recorded per batch.

## Latest evidence

| Check | Revision | Result and limits |
|---|---|---|
| Published 4.2.0 exact package | [Release identity](../tests/visual/windows-release-4.2.0-2026-10-20.json) | Locked build, formatting, strict Clippy, workspace/native regressions and imports/notices/source checks passed. Engine reused from 4.1.0; exact archive tested locally (5843 ms) and in CI (6211 ms), inspected on the desktop and published unchanged. Scoped GUI results and remaining artwork/focus/hardware/port gaps are in the record |
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
