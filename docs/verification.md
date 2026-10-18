# Verification record · native Windows · 2026-10-09

## Regression test improvements · 2026-10-18

The tests now check interacting operations against explicit expected behavior:

- Four independent child processes start behind a shared gate and perform 48
  contended read/modify/write updates. The parent enforces a 15-second deadline
  and owns cleanup of each child. This checks the file lock across processes,
  rather than merely checking atomic replacement from two threads.
- Three fixed seeds drive 288 stale-writer saves. An oracle assigns individual
  typed fields and compares the entire saved preference object after each save.
  Coverage includes nested siblings, no-op saves, returning to earlier values,
  optional theme removal, history clearing, cancelled/invalid edits, and retrying
  the identical candidate after Windows rejects an atomic replacement.
- Nine command sequences cover 36,864 operations across queue capacities 1, 2
  and 64. A FIFO model checks admission, explicit rejection, command order,
  wake coalescing/rearming and final disconnection. A five-second outer deadline
  also detects an accidentally blocking sender.
- Thirty loopback HTTP scenarios cover five redirect statuses, three methods
  and both request/download transports. The recorder checks actual methods,
  paths, encoded bodies, credentials and framing headers. Additional cases cover
  origin reentry after credential stripping, exactly ten redirects, non-redirect
  statuses with a Location header, and invalid/oversized options before I/O.
- The release smoke validator rejects each missing field/check, malformed roots,
  contradictory success/error, invalid durations, misleading fixture-name
  substrings, mismatched process identity, and unknown/duplicate checks. Tests
  also run its actual command-line entry point with bounded subprocesses.

The new merge cases exposed conflation of absent fields with explicit nulls and
insertion of null for an unchanged field removed by another writer. The merge
now distinguishes those cases. Regression tests live beside their owning modules
in separate test files; no test dependency was added.

An isolated Rust harness using the actual merge/queue functions and their tests
passes normally and fails for each of four deliberate faults: whole-snapshot
replacement, the old absent/null handling, silently accepting queue overflow,
and leaving wake notifications armed after rejection. This is targeted test
sensitivity evidence, not exhaustive mutation coverage. Harness inputs and logs
remain untracked under `target/smarter-tests-sensitivity`.

The final locked workspace run passes 596 tests. Its 56 ignored entries are 53
native checks, two clipboard checks, and the child-process helper that the parent
test explicitly runs four times. All 31 Python packaging/report tests pass.
The cross-process scenario also passes eight consecutive repetitions. The locked
workspace build, formatting, strict all-target Clippy and parity traceability
check pass. The stricter smoke validator accepts the previously recorded real
application report; no new GUI run is claimed here.
The GUI runner and native engine behavior were not changed; this work does not
replace the separately recorded native/GUI or physical-display verification.
Check logs are under `target/smarter-tests-*`.

## Local baseline review fixes · 2026-10-18

Baseline `d1ba5ae` is retained locally. The six review findings have implementations
and regression checks:

- Preference updates merge changed fields under a shared file lock. Layout and
  sidebar writes are scoped; interleaved/concurrent writers, reset-to-initial and
  corrupt-file retry are covered. Session persistence is a separate child module.
- Plugin request/download/XML-RPC transports share redirect handling. Tests use
  loopback servers for port-change credential removal, same-origin retention,
  redirect exhaustion, blocked targets and POST rewrite headers, plus explicit
  full-origin and HTTPS-downgrade policy cases.
- Pause actions use the same bounded FIFO as other accepted commands. A full queue
  returns a busy error instead of creating unordered sender threads. The test
  enforces a deadline and checks order, overflow rejection, retry and disconnect.
- Windows Audio-track selection and external-audio loading are connected. The
  extracted application's realized native menu is checked for enabled actions and
  a populated track submenu.
- Packaging tests run independently on PRs and in the selected-source package job,
  including native-artifact reuse. Workflow contracts have regression checks.
- `smoke-windows-package.ps1` launches the actual extracted executable with a
  disposable profile and a single isolated fixture. It validates executable/PID,
  active media, native playback progress, menu identity and surface geometry on
  volume changes, deferred double-click fullscreen, controller hide/reveal,
  windowed restoration, setting preservation and normal process exit. Partial,
  stale-process and falsely typed reports are rejected by tested validation.

The local test archive contains the debug executable and the reused pinned DLL;
it is development evidence, not a verified distributable release. The extracted
application smoke completed successfully in 6034 ms. The runner separately checked
that decoder threads (7), ICC (disabled), audio threads (3), network user-agent and
Hable tone-mapping choices remained saved after the application exited. Process
12316 exited normally. Before/fullscreen/after captures were valid 3840×2160
images with non-black probes and were inspected; the revealed controller appears
over video, and the prior desktop returns after exit. Private captures remain
untracked. This does not recertify the physical display or HDR.

The locked workspace build, formatting and strict all-target Clippy pass. All 51
native integration tests pass (38.29 seconds), plus both ignored native library
tests (4.47 seconds), against `tools/mpv/libmpv-2.dll` with null audio. The native
fixture root is shared across all 51 tests; their scenario assertions remain
separate. The final workspace rerun passes (586 passed; 55 explicitly ignored in
that default invocation). Including the 53 native checks run separately gives
639 Rust tests passed; the two clipboard checks were not run. All 25 portable
packaging/workflow/report tests pass. The changed GitHub workflow has not been
executed remotely. Logs and GUI evidence are retained under `target/review-fixes-*`.
No native compilation or public release was run. `target/debug` measured 43.46 GB,
below the 50 GB cleanup threshold.

## Windows volume/menu and fullscreen regression · 2026-10-18

The user confirmed that guarding `WM_SETREDRAW` removed the volume blackout,
but the remaining whole-menu rebuild still made video jump. The revised native
host keeps volume observations out of that menu model and updates the disabled
Audio information row with `SetMenuItemInfoW` when its popup opens. Native tests
verify the label, stable root/submenu handles, unchanged client bounds, repeated
menu detach/restore, visible host/child preservation, and hidden-host safety.

Fullscreen/Music Mode retain the Slint menu tree and detach the Muda-owned root
instead. Successful surface alignment keeps the controls HWND visible; failed
alignment still hides it until verified recovery, which requests a native redraw.
Double-click actions defer native geometry changes until pointer release unwinds.
Startup centering now skips fullscreen/maximized windows. An isolated fullscreen
startup measured host, controls and mpv at (0,0), 3840×2160, with no native menu
attached. The prior startup candidate had been incorrectly shrunk to work-area
bounds by delayed centering.

Locked workspace build, formatting, strict all-target Clippy and workspace tests
pass (577 passed, 55 explicitly ignored). The ignored native/HDR tests are not
reported as passed. Logs are in `target/volume-fullscreen-final-*.log` and the
native geometry record is `target/volume-fullscreen-final-windows.json`.
The user observed black fullscreen video while captures showed decoded video;
physical-display confirmation of the final candidate remains required. Raw
desktop captures contain unrelated content and are not publication evidence.

The user subsequently confirmed the volume issue is fixed. In the temporary
bitblt presentation check, video was visible until mouse movement revealed the
controller, so changing video presentation alone did not resolve the defect.
The controls alpha setup now uses a real empty region with
`DWM_BB_BLURREGION`, rather than the null whole-client region, and explicitly
disables maximized colorization. This follows Winit 0.30.13's transparent HWND
setup and the [DWM region contract](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/ns-dwmapi-dwm_blurbehind).
Normal flip presentation is restored. The user reported the same controller-reveal
blackout with the empty-region change and with a temporary Slint creation-time
transparency patch. The toolkit override was removed after the failed check.
The pinned native engine is reused. Fullscreen remains unresolved.

An opt-in `RIINA_TRACE_NATIVE_ALPHA` probe reads three canvas pixels before
presentation (at most 120 samples, at least 500 ms apart). With the controller
visible and hidden, all three were `[0,0,0,0]`, the default framebuffer was bound,
and the framebuffer reported eight alpha bits at 3840×2160. This rules out an
opaque canvas at those sample locations; it does not certify physical display
composition. Full-desktop captures before launch and after exit were valid
3840×2160 images without a persistent Riina surface. The exact test process
(7548) was closed. Private captures and the trace remain under `target`.

The temporary `RIINA_TRACE_OVERLAY_SCANLINE` check used a fullscreen-only controls
popup one physical scanline shorter than the host; the video host and mpv retained
the complete monitor dimensions. The user confirmed video stayed visible when
revealing the progress bar. The controls measured 3840×2159, while the host and
mpv both measured 3840×2160. The exact test process (2372) was closed afterward.
This adjustment is now the normal fullscreen path, without an environment switch;
unit checks cover the fullscreen size, windowed restoration and minimum height.
Final locked build, formatting, strict all-target Clippy and workspace tests pass:
579 passed, 55 explicitly ignored. Logs are `target/fullscreen-fixed-{build,tests,clippy,fmt}.log`.
The final build ran without diagnostic environment flags. F exited automatic
fullscreen, double-click re-entered it, and Escape exited again. The native menu
was restored and all three windowed clients matched at (546,215), 2747×1711;
fullscreen host/mpv remained 3840×2160 and controls 3840×2159. The inspected
3840×2160 full-desktop capture contained video and the revealed controller with
valid non-black probes. Captures remain supplementary to the user's physical
confirmation, not proof of physical presentation by themselves. The final exact
process (15176) exited; no Riina process remained. HDR and other drivers were not
tested. Geometry records and private captures remain under `target/fullscreen-fixed-*`.
The hypothesis is supported by a [GLFW maintainer's direct Windows 11 experiment](https://discourse.glfw.org/t/black-screen-when-setting-window-to-transparent-and-size-to-1920x1080/2585/5):
monitor-sized undecorated OpenGL windows lost alpha and changing their extent
avoided the driver path. This is supporting evidence, not a Riina test result.

## IINA Default primary input map · 2026-10-14

Compared the seeded profile against the primary section of the pinned
`iina-default-input.conf`. Added its missing Alt+arrow frame steps, Shift+arrow
subtitle seeks, Shift+Meta+L file-loop toggle, audio-delay reset, subtitle-delay
adjustments/reset, and fullscreen Escape/Enter commands. `Meta+D` now routes to
the existing online-subtitle search callback. A focused regression test checks
each added mapping resolves to a supported runtime action; GUI key delivery and
the rendered key-entry behavior remain unverified.

## IINA Default online subtitle shortcut · 2026-10-14

The pinned IINA input map assigns `Meta+D` to `find-online-subs`. The default
profile now seeds that application action and dispatches it to the existing
online-subtitle search callback. The preferences row shows Shift+Command+D on
macOS and Ctrl+Shift+D on Windows/Linux, following the profile key normalization
used by live dispatch. The focused regression test confirms the mapping, action
route, normalized key and display. Physical key delivery and the search window's
live GUI behavior remain unverified.

The source-to-seed comparison confirmed all 58 primary-section entries and all
33 Alternative mpv entries are represented by the seeded profile and system-key
defaults. Riina routes `quit` through its application lifecycle, renders
`show-progress` in its own OSD, and sends the remaining mpv commands through the
embedded engine. Regression tests check each alternative binding and ensure all
IINA/system defaults resolve to a runtime action. Physical keyboard/media-key
delivery and GUI appearance remain unverified.

## IINA Default mute shortcut · 2026-10-14

The pinned IINA input map binds mute to both `m` and `Meta+/`. Riina's IINA
Default profile now seeds both; the existing platform modifier conversion maps
`Meta+/` to Command+/ on macOS and Ctrl+/ on Windows/Linux. The focused regression
test checks the seeded mappings, displayed shortcut, and normalized key on the
current platform. Physical key delivery and native-menu appearance remain GUI
verification items.

## Stop saves the configured resume position · 2026-10-14

Playback Stop now checks the observed `save-position-on-quit` option and active
playback phase. When enabled, one ordered command plan issues
`write-watch-later-config` before `stop`; when disabled or inactive, it issues
only `stop`. Focused core-plan tests cover playing, paused, disabled, and loading
states. A focused MPV property test verifies the save policy is observed and can
become unavailable. The native integration test
`resume_last_position_saves_on_stop_and_restores_only_when_enabled` passed against
`tools/mpv/libmpv-2.dll`: it verified the watch-later record existed before the
player was dropped, then reopened the media and restored the saved position. In
the disabled case, explicit Stop and subsequent player drop both left the
watch-later directory empty. The shared menu's live GUI response still needs
verification.

## Font preview static width classes · 2026-10-14

The font catalog now retains OpenType OS/2 width classes (1–9), and regular
upright face selection uses normal width as a tie-breaker. Because Slint exposes
font family, weight and slant but not font stretch, the 24-point editable preview
uses centered horizontal scaling for a selected condensed/expanded face when its
family also has a normal-width face. Families without a normal-width base retain
the platform font match without extra scaling to avoid double stretching. The
rendered font remains a platform approximation; variable-width axes, actual
selected glyph identity and matched native captures remain open.

## macOS sidebar Reduce Motion behavior · 2026-10-14

The pinned IINA sidebar fades under macOS Reduce Motion, while its explicit
Disable Animations preference sets duration to zero and uses the slide path to
avoid a zero-duration fade glitch. Riina now keeps the system setting separate on
macOS, fades the sidebar at 240 ms when only system Reduce Motion is active, and
keeps explicit Disable Animations instantaneous. Windows/Linux retain their
system-reduced animation policy. The full workspace tests passed after the
title/OSC inset changes; strict all-target Clippy and the Windows application
build are pending.
AppKit build, matched sidebar captures, animation timing, and live preference
changes still require macOS runtime evidence. Riina's ordinary-motion sidebar
slide now runs for 240 ms, and explicit Disable Animations snaps that transition
to its final state. The title row and bottom OSC now animate their leading/trailing
insets with the sidebar on macOS; Reduce Motion leaves those bounds in place.
Matched AppKit spacing, rendered timing, and macOS runtime behavior remain open.

## Coalesced A/B loop feedback · 2026-10-13

The shared OSD reducer now retains both endpoint messages when one native
playback observation contains updates to A and B, preserving A-then-B order. A
snapshot that clears both endpoints emits one `AB-Loop: Cleared` message. The
focused tests `coalesced_ab_loop_endpoints_each_receive_feedback` and
`simultaneous_ab_loop_clear_is_reported_once` passed. This covers reducer
behavior only. `cargo fmt --all -- --check`, strict workspace Clippy,
`cargo build --workspace --locked`, and the parity-inventory traceability check
passed. The complete `cargo test --workspace --locked` suite passed; its 49
native libmpv integration cases remain ignored by default and are covered by the
separate pinned-engine run recorded above.
Playback timing, visible OSD rendering, and macOS/Linux runtime remain
unverified.

## A-B Loop menu shortcut label · 2026-10-14

The A-B Loop command now uses the current profile's shortcut in both the shared
player menu and the Windows native-video menu. This matches the pinned IINA
default profile (`Meta+l`) and its portable mpv input binding (`l`) while keeping
custom-profile labels synchronized with the action dispatcher. The focused
`menu_shortcuts_follow_the_active_profile_for_both_player_surfaces` test passes
with all eight key-binding preference tests, and `cargo fmt --all -- --check`
passes. The full `cargo test --workspace --locked` suite also passes; its 51
native libmpv integration tests are ignored by default. Live menu rendering and
keyboard activation still need GUI verification.

## Windows work-area window placement · 2026-10-13

The Windows player now sizes initial video windows against the monitor's actual
taskbar-free work area and centers their outer frame in those same bounds. This
keeps sizing and placement consistent with side taskbars and docked app bars;
auxiliary windows remain centered over their owner and clamped to its monitor.
`cargo check --workspace --locked`, `cargo build --workspace --locked`,
`cargo test --workspace --locked`, strict all-target Clippy, formatting, and
`git diff --check` passed. The rebuilt app
launched, but the desktop capture helper failed twice with
`CreateForMonitor ... 0x80070057` after refreshing its window selection, so
on-screen placement could not be verified. The exact test process was closed
after normal shutdown did not finish.
The focused Windows placement tests pass for a negative-origin monitor with a
taskbar inset, measured native-frame inset subtraction, and rejection of empty
or inverted work rectangles. The full workspace test suite passed after these
changes, with native libmpv cases run separately against the pinned engine.

## Development workspace and native playback checks · 2026-10-13

`cargo test --workspace --locked` passed across the app and core suites. The
49 ignored libmpv integration cases were then run serially with null audio against
the reused Windows native engine from workflow run 38138622911; all 49 passed.
The tested DLL hash is
`1749e6b5519afa383c88442e97026754e4e169bd46107ecc0ab0b08d205ad0af`, matching
the 0.3.3 release verification record. The broader suite covers playback
controls, track selection, file and playlist operations, filters, lifecycle,
and shutdown/reopen behavior. Physical audio output and GUI interactions were
not exercised by these native tests. The parity inventory, settings-navigation
and fixture-integrity checks also passed.

The current 0.3.4 workspace suite passed after native wakeup coalescing; all 49
applicable pinned-engine integration tests also passed. Earlier full-suite runs
exposed one dropped volume command under queue pressure and one intermittent
localhost HTTP load failure. The event worker previously enqueued duplicate
`Wake` messages despite relying on them to coalesce. An atomic pending-wakeup flag
now permits only one queued wake and is cleared on consumption or when the queue
is already full. Focused coverage verifies repeated wakes occupy one queue slot
and a full intent queue does not leave wakeups permanently armed. The full native
suite then passed serially, including the volume-boost stress and localhost HTTP
playback cases. `cargo test --workspace --locked` also passed after this change.
Strict all-target workspace Clippy and `cargo build --workspace --locked` passed
with the coalescing implementation. Formatting, parity traceability and
`git diff --check` pass as well.

# OpenSubtitles workflow and current macOS target · 2026-10-12

The app now has a manually opened OpenSubtitles search window, user-provided API
key setting, hash-aware local-media search, preferred-language filtering, result
selection, bounded download, cache storage and loading into the active player.
The key is wired to the platform credential store (Windows Credential Manager,
macOS Keychain or Linux Secret Service), is masked in Settings, and is excluded
from preferences JSON. Startup migrates an older saved key out of JSON after the
secure store accepts it. Automatic search is disabled by default and waits for
subtitle discovery before offering a search on eligible local media. The final
`cargo test --workspace --locked` passes the app, core and mpv test suites; 50
native/environment-dependent tests are ignored by default.
Rustfmt and `git diff --check` pass; the final strict all-target Clippy rerun passed.
`cargo build --workspace --locked` could not replace
`target/debug/riina.exe` because a Riina player process already running from that
path held the file open; it was left undisturbed. A uniquely named Windows
Credential Manager entry round-tripped and was deleted by the focused test; the
settings migration was exercised with an isolated in-memory store and save stub.
Migration against a real settings file, provider network calls and GUI playback
have not been verified at runtime.

Subtitle Auto Load resolves relative and absolute paths, home-directory paths, and
IINA's trailing `/*` child-directory pattern after a local file starts. A bounded
background scan filters supported subtitle extensions and media-name matches, then
submits one batch with mpv's non-selecting `auto` flag. The online-subtitle
auto-search timer waits for this scan to finish before its own 500 ms no-track
window. The workspace suite covers path expansion, filtering, matching modes,
preference/generation changes, track-priority selection, episode grouping,
episode-number conflict rejection, reciprocal closest-name matching, the pairing
work bound and batch command planning. Live playback, matching GUI captures and
IINA's full grouping and prefix/suffix distance behavior remain unverified.
Apple's latest-version target update is recorded below.

Apple's current version table lists macOS 27.0.1 as latest on this date, while the
captured IINA 1.5.0 UI surfaces are from 26.6.2. The 27.0.1 target and the need for
matching reference captures are recorded in `docs/status.md` and
`docs/interface-parity-plan.md`. [Apple's macOS version table](https://support.apple.com/en-gb/109033)
and [Tahoe update history](https://support.apple.com/en-us/122868) provide the
current release context. No latest-macOS visual parity or application support is
claimed.

The full application build matrix and native playback integration job now target
macOS 26 and the macOS 27/Xcode 27 preview runner. IINA reference capture is
pinned to that preview. It rejects captures unless `sw_vers` reports exactly
27.0.1, preventing older hosted images from being recorded as current goldens.
GitHub currently documents `xcode-27` as
a public-preview label and its published runner image as macOS 27.0, so the
27.0.1 capture remains unavailable on the documented hosted image at this point.
See [GitHub's runner labels](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)
and [runner image release notes](https://github.com/actions/runner-images/releases).

## Visible player smoke check · 2026-10-12

On the currently running Windows player executable (`target/debug/riina.exe`,
last written 2026-10-11 18:50), a paused media window rendered video, bilingual
subtitles, playback position and status. The Playback menu opened and Music Mode
could be toggled on and back off; the unchecked state was confirmed after restore.
The Video menu opened and exposed Aspect Ratio, Crop, Rotation, Flip and
Deinterlace. An automation click on the Aspect Ratio submenu failed, after which
a fresh capture showed the player still responsive and unchanged. This is a
limited smoke check of that existing binary, not verification of the unbuilt
source changes, full UI parity, dialogs, or the release package.

# macOS app bundle and default file associations · 2026-10-12

The shared IINA v1.5.0 imported-type manifest now drives Windows associations,
macOS `UTImportedTypeDeclarations` and the Launch Services registration path.
`config/macos-open-extensions.csv` retains the wider Finder-open coverage from
the pinned `CFBundleDocumentTypes`. `scripts/package-macos-app.py` assembles a
`.app` bundle from the built executable; the macOS/Linux application workflow now invokes the
packager and `plutil` validation on its macOS runner.

Windows-hosted `cargo check --workspace --locked` and strict all-target Clippy
passed, as did workspace formatting, the parity inventory check and Python
syntax compilation of the packager. The x86_64 Apple-target check stopped in
`ring` before Rust type-checking because this Windows host has no Apple-target C
compiler (`cc`). No tests or GUI were run. Bundle launch, playback, Launch
Services consent/handler behavior and current-macOS rendering still need a Mac
run; this does not establish macOS support.

## Subtitle auto-load priority · 2026-10-13

The priority matcher now counts configured case-sensitive keywords only in the
extensionless subtitle filename stem, matching IINA's `FileInfo.filename` input.
Parent-directory and file-extension text cannot accidentally influence track
selection or candidate order. Focused tests cover filename case, folder/extension
exclusion and discovery ordering; all four passed, and the complete workspace
suite passes. The 49-test native playback suite also passed against the pinned
Windows DLL. Formatting, strict all-target Clippy and the workspace build passed.
Playback timing, GUI capture and matched macOS pixels remain unverified.
Series-level grouping and fuzzy thresholds remain open; path expansion and
candidate loading are covered above.

## macOS PiP frame-bridge API audit · 2026-10-12

Apple's current Core Video documentation marks `CVOpenGLTextureCache` deprecated
and documents IOSurface-backed `CVPixelBuffer` allocation through
`kCVPixelBufferIOSurfacePropertiesKey`. The PiP bridge investigation therefore
selects direct rendering into IOSurface-backed buffers in the existing CGL
context, with a bounded buffer pool, rather than adopting the deprecated texture
cache. `pip_frame.rs` now implements that source path, while the AVKit controller
and Slint rendering notifier produce and enqueue samples from the existing
libmpv context. `cargo check -p riina --target x86_64-apple-darwin --locked`
and `cargo check --workspace --target x86_64-apple-darwin --locked` pass,
including the macOS-gated producer and integration. Strict Apple-target Clippy
for `riina` and strict all-target workspace Clippy on Windows also pass. This is a source
check only: the Windows host cannot link against the Apple SDK or verify CGL/AVKit
runtime behavior, GPU synchronization, color/HDR, pacing, window policy, or GUI
pixels. The BGRA8 producer does not preserve HDR metadata. See Apple's [texture-cache status](https://developer.apple.com/documentation/corevideo/cvopengltexturecache-780)
and [IOSurface pixel-buffer attributes](https://developer.apple.com/documentation/CoreVideo/kCVPixelBufferIOSurfacePropertiesKey).

## Player-window file drops · 2026-10-12

The shared player window now batches adjacent native dropped-file events for 80 ms,
then sends all paths through the existing ordered media-discovery/open path. This
keeps a multi-file drop as one selection instead of starting competing discovery
requests. The behavior follows the pinned IINA main-window drop path in
`DraggingDetect.swift` and `InitialWindowController.swift`. No GUI, native drag,
playlist-row insertion, URL/text, or subtitle-only drop behavior has been verified.
The Windows `cargo check --workspace --locked` and Windows-hosted
`cargo check --workspace --target x86_64-apple-darwin --locked` both pass. The
edited Rust file passes direct Rust 1.99 rustfmt checking, parity traceability
accepts all 195 inventoried rows, and `git diff --check` passes. The Apple target
check type-checks source only; it does not link against an Apple SDK. No tests were
run.

## macOS source cross-check · 2026-10-12

`cargo check --workspace --target x86_64-apple-darwin --locked` passes with the
Rust 1.99.0 Apple standard library and Zig 0.15.2 compiling target C code. This
type-checks the macOS-gated application source, including System Media Controls.
It does not link the app against an Apple SDK or exercise macOS playback, PiP,
rendering, window lifecycle, or UI pixels. Full macOS build and runtime evidence
still require a Mac runner. The check exposed and this increment fixed macOS-only
type and ownership errors, and gated Windows plugin-overlay IPC out of other
platform builds. It also compiles the AVKit PiP content source and playback
delegate, live-duration handling, and playback-state invalidation path. No
An IOSurface/sample-buffer producer is now connected and type-checks, but no Mac
link, PiP display, seeking, pacing, HDR, or window-lifecycle runtime verification
has been done, so PiP usability is not established. The Windows workspace `cargo check --workspace
--locked` also passes. No tests or GUI checks were run for this increment.

## Crop edge pointer anchoring · 2026-10-10

Pinned CropBoxView changes resize boundaries by mouse-position delta. Source
inspection found riina instead assigned the current pointer coordinate to the
boundary, losing the grab offset within the hit strip. Original crop_drag::Drag
now retains the initial pointer and rectangle, computes movement deltas and
preserves the opposite edge. The app uses this portable typed model for all
edges and free selection. Invalid pointer values are rejected.

Unit regressions start 1.5 logical units inside each strip: unchanged pointer
keeps (30,60)-(330,220), and ten-unit movement changes only the grabbed boundary.
Orthogonal one-unit movement leaves the other axis unchanged. Free reverse
dragging keeps its origin and a NaN pointer fails. This reproduces the required
geometry numerically; no runtime pointer jump was observed or claimed.

The UI uses four-point hit strips centered on boundaries, with vertical strips
excluding the two-point top/bottom corners, horizontal/vertical resize cursors,
and a two-point outline. Requirements are from CropBoxView draw/resetCursorRects/
updateCursorRects; Slint enum availability was checked in the pinned runtime.
No protected source was translated or assets copied. Actual pointer hit testing,
very small/crossed rectangles, stroke placement, native candidate and rendered
IINA comparison remain unverified. Workspace unit tests, strict all-target lint
and formatting pass. No GUI or explicit native-suite rerun was performed; native
command planning and crop dispatch are unchanged.
Optimized Windows build passes in 37.48 seconds.

## Continuous crop draft coordinates · 2026-10-10

The pinned CropBoxView keeps its selection geometry continuous and the crop
action converts coordinates/dimensions to integers. Original SourceRect now
separates that editing domain from the typed integer native rectangle. Continuous
selection/project APIs retain edge snapping, aspect/rotation and bounds checks;
legacy integer APIs convert only at their boundary. The app coordinator retains
SourceRect for pointer movement and presets, projects it for drawing and converts
it for labels/confirmation. Preset centering occurs before integer conversion.

A known 960×600 viewport test moves the pointer by 0.15 logical units, changing
continuous source width by 0.05 pixels while native/label width remains 100.
The starting draft retains source x=10.2/y=12.3/width=100.1/height=59.9 and
converts to x=10/y=12/width=100/height=59. Fractional source rectangles round-trip
through two viewport shapes at every quarter turn with native truncation retained.
Invalid NaN and out-of-source drafts are rejected. Existing integer native crop
projection tests continue to pass.

All 28 native tests, workspace unit tests, strict all-target lint and formatting
pass. No GUI or current other OS app test was run for this change; these checks
prove the model and conversion boundary, not pointer rendering or pixel parity.
The optimized Windows build passes in 36.40 seconds.

## Connected interactive crop editor · 2026-10-10

The original shared editor coordinates source sessions and rectangle projection.
Its observed state runs on the UI event loop; no UI handle crosses the native
worker boundary. Mutex ownership ends before property publication or focus
changes. Source replacement retires the editor without restoring stale pause.
Entry waits for native pause and managed-filter removal, with seeking excluded;
Done is disabled while waiting. There is no rendered-frame completion gate yet.
Source rectangles reproject on viewport changes. The control bar supplies the
reference's six interactive presets (including 3:2), label and Done/Cancel.
Free/edge pointer handlers are connected but have no pointer runtime evidence.

Initial UI compile failures were callback declaration syntax and subtraction
spacing, then strict lint found an unnecessary app borrow. These were corrected.
Workspace unit tests, formatting and strict all-target lint pass. The initial
optimized build passes in 35.48 seconds.

GUI source 4e1724e and binary hash are recorded in crop-editor-evidence.json.
Starting paused at position 2/volume 37, Custom opens with (0,0)/(320×180) and
the six presets plus Done/Cancel. Tab and Space on 4:3 changes the label to
(40,0)/(240×180). Reverse Tab returns to editor scope; Return confirms and
restores Play/position 2/volume 37. Reopening Custom returns to whole source;
initially disabled presets/Done become enabled after native reset readiness.
An editor Right key followed by Escape leaves position 2 and volume 37. Close
exits 0 with empty stderr. This does not prove rendered crop pixels, pointer
handles or full keyboard/reference parity. Existing core/native crop tests are
unchanged from the previous 28-test native pass; this UI increment did not rerun
that native suite.

Later correction centralizes focus restoration for all editor exits, including
source invalidation, and validates projection before entering. Its source
replacement GUI path remains untested. Continuous source selection, video-area
padding/shadow and native candidate layout, frame synchronization, crop OSD,
playing-state GUI restoration and current macOS/Linux runtime remain open.
Final workspace unit tests, strict all-target lint, formatting and optimized
Windows build pass (34.09 seconds). The later correction was not GUI-retested.
Standard titlebar button disabling and the exact segmented-control keyboard
behavior remain reference parity gaps as well.

## Crop selection projection · 2026-10-10

Original crop_geometry::Projection separates source pixels, unrotated display
dimensions and a logical viewport. It centers contain-fit content, maps source
rectangles through quarter-turn rotation, and inverts pointer drags back to
source pixels. Four-source-pixel edge snap follows the pinned CropBoxView
requirement. Entirely letterboxed or empty selections are rejected. Missing
display parameters and non-quarter-turn rotation are reported instead of
inventing a coordinate system. The mpv property contract describes dw/dh as
aspect-corrected sizes and rotate as intended clockwise display rotation
(reference/mpv-upstream/DOCS/man/input.rst video-params properties).

Unit checks exercise known 960×600 letterboxing, a 300×160 source selection at
(10,12), reverse drag, rotated top-strip placement, three display aspects and
two viewport shapes at every quarter turn, edge snap, tiny sources, empty drag,
missing metadata and invalid numeric dimensions. Initial round-trip lost x=23
to x=22 due floating-point cancellation; quantization now corrects only integer
proximity bounded by machine epsilon and source extent. Ordinary fractional
coordinates still truncate.

The new native test uses observed source dw/dh/rotate while paused, with Default,
1:1 and 4:3 overrides at 0/90/180/270. Source remains 320×180 and projected
251×137 at (23,17) in a (20,20,920,492) viewport round-trips to the same source
rectangle. This verifies metadata and mathematical mapping, not rendered video
placement, IINA's actual coordinate behavior, pointer interaction or the native
video candidate's viewport. Custom UI, frame synchronization, other filters,
rendered parity and current macOS/Linux runtime remain unverified.
All 28 native tests, workspace unit tests, strict workspace/all-target lint and
formatting pass. Optimized Windows build passes in 35.23 seconds. No GUI or
current other OS application build was run for this geometry increment.

## Interactive crop session lifecycle · 2026-10-10

Pinned InteractiveMode.swift enter/exit/cancel establishes explicit pause during
editing and restoration of the prior pause state. SidebarVideoPane.swift Custom
clears the managed crop before entering. Cancellation leaves it cleared.
Original portable Session validates source generation and pixel dimensions at
worker dispatch. Enter/Exit command plans validate before emitting any native
commands; resume uses explicit pause yes/no rather than a toggle. Readiness
requires observed pause and disappearance of the managed label.

The new native regression runs cancellation and rectangle confirmation from
both playing and paused sources. Every entry removes the old 4:3 crop and
pauses with volume 37/speed 1.5 retained. Confirmation submits 300×160 at (10,12),
cancellation leaves no managed crop, and both restore the original pause state.
A changed speed provides a barrier when an already-paused exit has no new pause
observation. Same-file replacement increments generation; stale confirmation
reports the source-changed error and preserves replacement pause and filters.
Unit checks cover source changes, unavailable video and invalid confirmation
without partially constructing crop/resume plans.

This does not implement the interactive editor or prove its appearance. Shared
selection surface, coordinate conversion under aspect/rotation, pointer handles,
presets (including interactive 3:2), confirmation/cancel controls, native route
layout and current macOS/Linux runtime remain open. No GUI test was run for this
backend-only increment.
All 27 native tests run explicitly pass, as do workspace unit tests, strict
workspace/all-target lint and formatting. Optimized Windows build passes in
34.70 seconds.

## Shared crop preset controls · 2026-10-10

Crop row labels/order and 24px segment height follow the pinned CropView
requirements. Native catalog observations map managed filter dimensions to the
five presets, no managed filter to None and explicit coordinates to Custom.
Unknown catalog/source is distinct from None; named/positional parameters have
unit coverage. The expanded native crop regression checks observed preset
indexes and explicit rectangle Custom selection. Small-source preset dimension
collisions are not resolved by this inference; exact requested identity remains
a parity gap. Custom... is visibly disabled until interactive mode exists.

Initial application compile failed on passing owned Playback instead of a
borrow and using a nonexistent status setter; corrected wiring passes workspace
unit tests, strict all-target lint, formatting and focused native crop test.
Optimized Windows build passes in 33.86 seconds.

GUI at source 5174f0e (binary hash recorded in crop-sidebar-evidence.json) exposes
Aspect/Crop/Rotation order, all six enabled preset/reset buttons and disabled
Custom. Tab enters crop from custom aspect, continues through preset order and
skips disabled Custom to rotation. Right/Left from None and Space from 4:3 leave
Play, position 2 and volume 37. Close exits 0 with empty stderr. The reader does
not expose selected segment state; this does not prove actual GUI crop effect
or visual highlight. Native effect/mapping proof is separate. Rendered geometry,
materials, pointer input, native video candidate and current port runtime remain
unverified for this increment. Interactive custom crop and crop OSD remain open.

## Crop rectangles and display overrides · 2026-10-10

The expanded crop regression applies every preset under an aspect override of
1:1 at rotation 90 / 180 / 270 / 0. Requested dimensions still derive from source
320×180; clearing retains rotation and aspect. It then applies 300×160 at source
pixel x=10 and y=12, checks observed parameters and actual output, pause/volume
and retained aspect. Confirming the whole 320×180 source clears the managed
filter, matching the pinned InteractiveMode.swift crop confirmation requirement.
Rectangle coordinates use the native top-left origin; a future UI must convert
its coordinate system explicitly. The reference's bottom-left selection origin
is converted before native dispatch. No protected implementation was translated.
Unit coverage rejects zero extents, out-of-source and overflow coordinates and
tests whole-source dispatch as removal. Source is revalidated when queued intent
is planned. UI interaction, foreign-filter native chains, decoder conflict policy,
pixel parity and current other OS runtime remain unverified.
All 26 native tests passed with the initial rectangle implementation. After
adding whole-frame confirmation and rejecting empty whole-frame sources, the
expanded focused native crop test passes again, as do workspace unit tests,
strict workspace/all-target lint and formatting. No GUI or current port build
was run for this change.
The final optimized Windows build passes in 32.60 seconds.

## Managed preset crop foundation · 2026-10-10

Original portable crop code inscribes 4:3 / 16:9 / 16:10 / 21:9 / 5:4 inside
source pixel dimensions, truncates the reduced dimension and leaves centering to
the crop filter. Source requirement: pinned SidebarVideoPane.swift CropView and
PlayerCore.swift setCrop; engine contract: local pinned mpv DOCS/man/input.rst
vf command (same-label add replaces; label removal targets one filter).
No application source was translated or assets copied.

The new native test uses catalog.mkv (320×180 YUV420), pauses and sets volume 37.
It checks every preset's requested vf parameters, actual chroma-aligned output,
exactly one riina-crop label, source dimensions staying 320×180, pause/volume and
absence of navigation feedback. Clearing restores 320×180 and repeated clearing
is harmless. Initial test failed at 21:9 because requested height 137 is aligned
to 136 by the engine; the corrected test distinguishes those facts. Five-four
likewise requests width 225 and outputs 224. Unit checks cover portrait, odd,
zero, tiny and maximum dimensions plus reset isolation from a foreign label
and rejection when the native catalog or source dimensions are unavailable.

GUI controls and custom interaction are not implemented by this increment.
Mixed foreign-filter native behavior, decoder policy conflicts, aspect/rotation
combinations, actual IINA crop presentation and current port runtime remain open.
Workspace unit tests, strict workspace/all-target lint, formatting and all 26
ignored native tests run explicitly pass. Optimized Windows build passes in
34.34 seconds. No GUI or current macOS/Linux build was run for this increment.

## Selected subtitle boundary and resumed loops · 2026-10-10

The new native test selects manual-captions.srt, pauses at position 3 in its
last cue and sets A–B to 2.8 / 3.2. Sub-seek forward leaves navigation completion
unchanged until playback resumes; before repair the automatic loop incorrectly
advances it from 1 to 2. Explicit pause/resume now clears only an unstarted
feedback token. Worker tests verify that transition retires an acknowledged
no-op, while an already-started seek still completes normally. Native command
dispatch and raw native completion counting remain unchanged.

All 25 native tests, workspace unit tests, strict lint, formatting and optimized
Windows build pass. This tests a paused final
cue followed by explicit resume, not no-op navigation during an already-running
loop. First-cue boundaries, event interleaving, GUI and actual IINA presentation
comparison and current other OS runtime remain open.

## Subtitle no-op navigation and automatic loops · 2026-10-10

The new native regression loads the catalog with no subtitle selection, pauses,
seeks to 0.3, sets A–B points 0.2 / 0.6, issues sub-seek, waits for volume 37 as
a command-stream observation and resumes. Before repair the first automatic
loop increments navigation completion from 1 to 2, incorrectly labeling it as
application navigation. The worker now requires an observed primary subtitle
selection before arming feedback for sub-seek; native dispatch remains unchanged.
The regression then passes with raw loop completion advancing and navigation
completion unchanged. All 24 native tests pass.

The existing selected-caption test additionally checks forward and backward
cue navigation increment the navigation counter; its focused run passes.
Workspace unit checks, strict lint, formatting and optimized build pass.
This establishes the no-selected-subtitle case, not all possible no-next-cue
cases. Selected-track final/first cue boundaries and rapid selection followed by
navigation remain open, along with GUI and current other OS runtime checks.

## Per-message OSD suppression · 2026-10-10

Pinned Preference.disableOSDSpeedMsg and disableOSDSeekMsg default false;
OSDMessage.isDisabled maps speed and seek independently. Portable preferences
now persist disable_osd_speed and disable_osd_seek, with serde legacy defaults
false. Global enable and timeout remain separate. Feedback has typed message
identity, and speed suppression consumes observations rather than postponing
an old message. Seek suppression is applied after navigation observation, so
raw completions are consumed without presentation.

Unit checks cover legacy defaults, persistence, suppressed speed followed by
equalizer/hardware feedback, no stale replay after enabling, and a later new
speed message. A coalesced speed/decoding check additionally verifies suppression
is applied before message choice, retaining Hardware Decoding: Off rather than
swallowing it behind the suppressed speed candidate. Updated workspace unit
checks, strict lint, formatting and final optimized build pass.

The optimized Windows shared-GL GUI at cbee705 runs with process-local
LOCALAPPDATA pointing to an isolated workspace preference directory; existing
user settings are untouched. With both suppression flags true, global OSD true
and timeout 3 seconds, custom speed 2 updates without Speed text, deliberate
seek to 2 updates without seek-clock text, and a brightness edit displays
Brightness: -27. Play and volume 37 remain; close exits 0 with empty stderr.
Saved JSON preserves both flags and global settings. Evidence:
tests/visual/osd-suppression-evidence.json. The GUI binary hash was not retained
before the subsequent coalesced-case rebuild; the source and build/launch
sequence are recorded. These are immediate AX checks, not rendered pixels.
No GUI retest of the subsequent coalesced-case change is claimed.

Settings-page switches, exact simultaneous-message ordering, locale formatting,
rendered OSD materials and current other OS runtime remain open.

## Navigation versus decoder refresh feedback · 2026-10-10

The prior hardware toggle GUI exposed a seek-clock overlay after decoder
refresh. Raw native SEEK / PLAYBACK_RESTART pairs do not establish user
navigation. The worker now retains that diagnostic counter separately from
completed_navigation_seeks. A submitted navigation command arms feedback;
native SEEK establishes its start and RESTART completes it. Decoder-changing
intent and file replacement clear stale feedback. Failed replies clear it;
successful replies retain it because native testing establishes reply-before-
SEEK ordering. SeekOsd consumes the navigation counter.

Worker unit checks cover internal restart, success, failure, replacement and
successful reply preceding SEEK; presentation checks verify raw decoder completions remain quiet
while navigation completion shows feedback. New native integration verifies
initial load has zero navigation completions, then paused 2 / 2 / 0.5 absolute
seeks each increment navigation and raw completion counters without errors.
The first full suite has 22 passes and the new navigation test times out with
raw completion 1 but navigation completion 0; a focused repeat reproduces it.
Inspection identifies premature clearing on successful command reply. The
repair retains that request, and its unit check covers reply before SEEK.
All 23 native tests then pass, including the repaired navigation test. Workspace
unit tests, formatting, strict lint and optimized Windows build pass.

At source 149f48f, shared-GL GUI toggles expose Hardware Decoding: Off then On;
deliberate timeline seek to 2 displays 00:02. Native-video-candidate GUI also
exposes Off then On, and deliberate Left navigation displays 00:00 at position
0. Both retain Play and volume 37. The native helper rejects set-value despite
the editor's settable label; ordinary Tab navigation reaches the switch before
Space/Return activation. Shared-GL closes with exit 0 and empty stderr; native
closes with exit 0 and only the existing 74-byte chrome-layout diagnostic.
Evidence with source and binary hash: tests/visual/decoding-feedback-evidence.json.
These are accessibility text/state checks, not pixels, HDR or color validation.
No-op navigation followed by
automatic loops and command-reply/event interleaving
under rapid mixed seeks/decoder changes, automatic loop feedback equivalence,
current other OS runtime and rendered visual parity remain open.

## Video setting feedback · 2026-10-10

Initial GUI at 201fb77 shows Brightness: -27 and eventual expiry, but speed
changes to 1.234567 and 2 do not expose their speed messages. Inspection finds
the catalog-only displayed cache is being reused as feedback history; unchanged
catalogs leave old brightness values in that cache. Repair adds an independent
VideoOsd history that consumes every snapshot while retaining only feedback
fields. A regression check verifies brightness once, no repeat on unchanged
observations, then speed once and no repeat on clock-only updates. Initial GUI
close exits 0 with empty stderr.

Repaired GUI at 0b1387a confirms Brightness: -27 followed by Speed: 1.234567x;
both disappear in later snapshots. Exact timeout latency is not measured. Play
and volume 37 remain; normal close exits 0 with empty stderr. Hardware toggle
Space instead exposes the seek clock (00:04), replacing its configured-mode
feedback. That interaction remains failing and requires separating deliberate
navigation from decoder-refresh native seek notifications. Evidence with source
and binary hash: tests/visual/video-feedback-evidence.json. Workspace unit tests,
strict lint, formatting and optimized build pass. No rendered pixel comparison,
progress-bar measurement or current other OS runtime is established.

Pinned OSDMessage.speed / hwdec / brightness / contrast / saturation / gamma /
hue define speed text with up to six decimals, On/Off for configured hardware
decoding and signed equalizer labels with progress mapping -100 → 0, 0 → 0.5,
100 → 1. MPVController sends feedback on confirmed property changes, including
auto → auto-copy even though both display On. Original video_osd.rs derives
matching English messages from known previous/current observations in the same
media generation. Initial/unknown values, replacement and native errors do not
produce success messages. The existing global timeout and enable setting apply.

Unit checks cover unchanged/initial snapshots, confirmed six-decimal speed,
generation/error suppression, signed progress, active-decoder fallback versus
configured policy and auto-copy mode changes. One coalesced snapshot displays
one coherent message with deterministic channel priority; exact reference event
ordering remains open. Final updated build and GUI checks pending. Existing
22 native backend tests passed on the unchanged backend in the previous turn;
they do not establish this new presentation behavior. Per-speed suppression,
locale messages, materials/geometry, progress rendering, timeout lifecycle and
current other OS runtime remain open. No public update.

## Custom speed engine boundary · 2026-10-10

The reference formatter has a minimum but no maximum and PlayerCore.setSpeed
dispatches the finite request. The provisioned engine documents a 0.01–100
option range. riina previously rejected above-100 requests before dispatch;
it now leaves upper-range rejection to the native engine. Focused native test
excessive_custom_speed_reports_native_error_without_changing_playback passes:
101 reports a native error while speed 1.25, pause and volume 37 remain. Core
coverage retains 101 and 1e308 as finite requests; normalization avoids overflow
while scaling very large finite input to decimal precision. This does not prove
IINA's error presentation or recovery. All 22 native tests pass, along with
updated core unit checks, formatting, strict lint and optimized Windows build.

At source 8e85752 the shared-OpenGL GUI retest submits 2, then Tab reaches Reset
playback speed. Return resets the observed input to 1 and slider to 8. The reset
node is absent from AX now; after focusing the input, Tab directly reaches
Hardware Decoding. This establishes the conditional control's keyboard and AX
behavior, not rendered pixels or the exact cause of the earlier lingering
snapshot. Control-A / BackSpace clears the input through ordinary keyboard
editing; Return restores 1 even when the native speed was already 1. Play,
volume 37 and position 5 remain. Normal close exits 0 with empty stderr.
Source and binary hash: tests/visual/speed-reset-focus-evidence.json. Current
macOS/Linux runtime and native error presentation remain open. No public update.

## Video speed controls · 2026-10-10

Pinned SidebarVideoPane.SpeedView defines 25 logarithmic ticks from 0.25x to
16x, octave labels at 0.25 / 1 / 4 / 16 and two-decimal slider results. Custom
entry has six-decimal positive half-down rounding and minimum 0.01; blank means
1. The reset is hidden at 1x. This original implementation parses text into f64
and restores editor display from backend observations; slider movement snaps to
an integer tick. Speed precedes the hardware switch in the shared video panel.

Unit checks cover octave anchors, intermediate tick 9 → 1.19, custom
1.2345674 / 1.2345675 / 1.2345676 rounding, below-slider input 0.01, above-slider
input 32, blank reset and invalid/nonfinite input. All 21 native tests pass:
the new speed test observes 0.25 / 1.19 / 16 / 1.234567 / 0.01 / 32 / 1 while
preserving pause and volume 37 without errors. Workspace unit checks, strict
lint, formatting and optimized Windows build pass.

Optimized shared-OpenGL GUI at 80a8da4 exposes slider position 8 and input 1 with
no reset node initially. Submit 1.2345675 and Return yields observed editor
1.234567 and slider position 9.216021. Tab from the editor reaches Reset playback
speed; Return restores editor 1 and slider position 8. Play, volume 37 and
position 5 remain. The reset node remains in subsequent AX snapshots despite
the conditional hiding rule, so its actual rendered visibility is not proven.
The helper rejects blank set-value for this numerically reported editor; blank
reset is established only by core/native checks. Close exits 0 with empty
stderr. Evidence: tests/visual/video-speed-evidence.json.

Locale grouping/decimal separators and exhaustive decimal ties
are unverified; the existing Command::Speed ceiling of 100 remains a gap for
unbounded reference custom entry. Native tick drag, neutral indicator, reset
artwork, measured layout, OSD and current other OS runtime remain open.

## Video equalizer · 2026-10-10

Requirements: pinned SidebarVideoPane.EqualizerView orders brightness, contrast,
saturation, gamma and hue. Each small slider spans -100–100, passes integer
values, and has a reset-to-zero action; macOS 26 adds neutral zero marking.
PlayerCore.setVideoEqualizer sets the corresponding native option. This original
implementation retains typed channel identity and independent optional observed
values. Missing or invalid native values disable the affected control rather
than treating an unknown channel as neutral. The shared UI uses equal-width
labels, 8px spacing, original slider geometry and a temporary text reset glyph;
native rendered geometry is unverified.

Unit coverage checks all five commands at signed limits and rejects overflow
and out-of-range values. The new native test observes initial zero values,
edits channels sequentially to -100 / 100 / -27 / 13 / 61, then independently
resets each while comparing the complete five-channel state. Every step retains
pause, volume 37 and speed 1.5 without errors. All 20 native tests pass. This
headless test proves configuration, not actual rendered color changes. Workspace
unit checks, formatting, strict lint and optimized Windows build pass.

The optimized shared-OpenGL GUI at 1949507 exposes all five sliders at zero and
their reset buttons. An accessibility edit of brightness to -27 returns the
observed -27 while the other four remain zero. Tab reaches Reset Brightness;
Return restores observed zero. Play, volume 37 and position 5 remain throughout.
Normal close exits 0 with empty stderr. Source and binary hash are in
tests/visual/video-equalizer-evidence.json. GUI activation for the other four
channels, native fractional drag rounding, reset/slider rendered geometry,
actual color output, equalizer OSD and current other OS runtime remain open.

## Hardware decoding policy · 2026-10-10

Pinned requirements come from SidebarVideoPane.HwdecSwitch, PlaybackInfo.hwdecEnabled,
PlayerCore.toggleHardwareDecoding and Preference.HardwareDecoderOption. Configured
hwdec determines switch state independently of hwdec-current. Off sets no;
on restores the preferred Auto / AutoCopy / Disabled policy. The mode preference
defaults to Auto, is preserved by legacy settings and round-trips through JSON;
invalid modes are rejected. The Settings picker now exposes Disabled, Auto and Auto (Copy); its rendered appearance and live selection behavior remain unverified.

The native policy test observes Disabled → AutoCopy → Auto while paused at volume
37 without errors, then verifies AutoCopy survives explicit file replacement.
Headless MJPEG has no active hardware decoder; hwdec-current is unavailable here.
An initial test incorrectly expected the explicit no sentinel and another
incorrectly expected replacement to retain pause; both assumptions were corrected
to the actual property availability and existing replacement-resumes contract.
All 19 native tests then pass. These checks do not prove hardware acceleration,
HDR or equivalent rendered native switch geometry. Workspace unit checks, strict
lint, formatting and the optimized Windows build pass.

The optimized shared-OpenGL GUI at 25478f9 exposes Hardware Decoding, and Tab
from custom aspect traverses 0° / 90° / 180° / 270° then focuses the switch.
Space and Return each retain the Play button and volume 37. The accessibility
helper represents the switch as a button and omits checked state; this check
does not independently prove GUI mode values. At EOF the first decoder refresh
changes the clock from 5 to 4.958 and exposes the existing seek clock OSD; native
IINA equivalence for that transition remains unverified. Close exits 0 with
empty stderr. Source and binary hash are recorded in
tests/visual/hardware-switch-evidence.json. Pixel comparison, hardware codec
acceleration, Settings mode selection, hardware-decoding OSD and current port
runtime remain open. Nothing published.

## Track metadata and Unicode ratio validation · 2026-10-10

The first full native suite at 8d4b1d1 has 17 passes and one failure in the
pre-existing secondary visibility test: its hidden-text assertion fails after
a same-position seek. Five isolated repeats pass, so no reproducible product
regression or exact missing-text value is established. Inspection finds its
cue predicate can accept the previous clock/text before the new seek completes.
The test now waits for completed_seeks to advance and seeking to finish before
checking the refreshed cue and hiding it; the text-retention assertion remains.
Three consecutive full-suite confirmation runs pass all 18 native tests after
the wait repair. The optimized Windows build, formatting and strict lint pass.
This improves the test's synchronization; it does not establish the precise
cause of the original intermittent failure or exhaustive timing stability.

Video track labels now deserialize demux-w/h/fps, display paired dimensions with
the multiplication sign, and round rates to three decimals without trailing zeroes.
Labels retain codec → dimensions → fps → Default order. Audio channel/kHz and
subtitle labels remain kind-specific; an explicitly empty title no longer adds
a leading blank before metadata. Core checks pass for 24/23.976/29.97 fps,
half-away rounding, missing fields, empty title and audio/subtitle isolation.
The original catalog native test verifies 320×180, 24fps and the complete label,
unchanged through all aspect overrides and quarter-turn rotation properties.

Custom ratio classification now recognizes Unicode Nd, including Arabic, Indic
and fullwidth digits. The original text remains the native argument and custom
selection identity; numeric fractions, Roman numerals, superscripts and Arabic
decimal separators are rejected. This uses the existing pinned regex 1.13.1 as
a direct core dependency. Unicode category semantics are documented by
[ICU](https://unicode-org.github.io/icu/userguide/strings/regexp.html) and
[Rust regex](https://docs.rs/regex/1.13.1/regex/#syntax). The BMP-only boundary is
inferred from the pinned reference's character-count NSRegularExpression search
range; supplementary-digit acceptance requires an actual macOS check.

The new explicit native test passes: after a valid 4:3 override, original Arabic
ratio ١٦:٩ is rejected by libmpv and reports an error while the 4:3 override,
pause and volume 37 remain. This proves no silent transliteration/default reset,
not equivalent IINA error presentation or recovery. The optimized shared-OpenGL
GUI at dcc09d4 exposes the complete video label
`No Title (mjpeg, 320×180, 24fps, Default)` and English language badge, at paused
EOF with volume 37. Normal close exits 0 with empty stderr. Evidence is in
tests/visual/track-label-evidence.json. This verifies accessibility text, not
rendered pixels; fractional-rate GUI fixtures and current other OS runtime
remain pending. All work remains local.

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

## A–B timeline marker appearance · 2026-10-14

The pinned `MainSliderLoopKnob.colorset` defines universal sRGB gray `0.410` and
dark appearance gray `0.650`, which map to `#696969` and `#a6a6a6`. The shared
Slint timeline now switches its A/B marker fill between those values and retains
IINA's three-point width, 75% height, and one-point corner radius. The
post-change `cargo build --workspace --locked` and `cargo fmt --all -- --check`
pass, as do `git diff --check` and parity traceability. Full workspace tests and
strict Clippy pass on the unchanged Rust implementation from the immediately
preceding verification. A rendered Windows or matched macOS capture has not been
verified, so visual color accuracy remains source-derived.

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

OSD preferences preserve the enabled and one-second defaults across legacy
settings, round-trip fractional timeouts, and reject negative or unrepresentable
durations. The Interface pane exposes auto-hide time and text size, and the
configured timeout drives transient seek and video feedback. Seek, video, and
screenshot overlays now fade out over the pinned 0.5-second duration; disabling
animations or turning off OSD removes the overlay without a fade. Workspace
compilation verifies the Slint changes. Live timer/fade behavior, reduced-motion
integration, accessibility during the transition, and matched macOS rendering
remain unverified.

The On-Screen Controller preferences now include IINA's enabled-by-default
auto-hide policy and editable 2.5-second timeout. The single-shot deadline is
re-armed by activity, applies while paused, and is cancelled when auto-hide is
disabled. Core tests cover legacy defaults, fractional values, validation and
round-tripping; app tests cover the numeric editor. The pinned source also keeps
the controller visible while hovered or dragged and fades it over 0.25 seconds.
Transparent hover areas now cancel the deadline over the title bar and OSC, then
restart it on exit; input is forwarded to the existing controls. Compilation
verifies this wiring. Controller chrome now fades over the pinned 0.25 seconds,
with a delayed removal that is cancelled if activity returns during the fade.
Live hover/fade interaction remains unverified. Dragging the floating controller
and live Settings interaction remain open.

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
The shared list source now routes face-row double-click through exact face
selection and picker acceptance. Direct pointer acceptance remains unverified;
the earlier GUI attempts could not reliably inject a face-row double-click.

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

## Resume last position · 2026-10-10

An explicit ignored native test against the hash-pinned Windows libmpv DLL opens
the original catalog fixture, waits for playback to pass two seconds, shuts down,
and confirms a watch-later file is written. A second engine restores from that
file. Separate runs with resume disabled create no watch-later file and reopen
the media near zero. `cargo test -p riina-mpv --test native --locked
resume_last_position_saves_and_restores_only_when_enabled -- --ignored --nocapture`
passes. Workspace tests, strict all-target Clippy, formatting and the optimized
workspace build also pass. This tests engine policy, not the app's full settings
restart flow or the Preferences control's rendered pixels; both remain open.

## Windows Picture-in-Picture surface

The Windows controller keeps the existing libmpv renderer and decoder, projects its
native video surface into a compact always-on-top Slint window through DWM, and
syncs title, pause, seek and configured main-window lifecycle behavior. Closing PiP
preserves playback state, while its explicit Stop action pauses, matching IINA's
separate exit and stop semantics. Core lifecycle tests cover restoration without an
implicit pause. Entry checks the pause command and restores its prior pause state if
hiding the main window fails; the renderer is moved only after that fallible window
action succeeds. While active, the Windows host now re-queries the DWM source size
and recomputes the aspect-fit destination at the PiP refresh cadence. This keeps the
projection fitted when the PiP window is resized or autoplay changes source geometry.
Focused tests cover unchanged, resized, and changed-aspect destinations; app strict
Clippy and workspace tests pass. Live DWM rendering, PiP controls, minimized/hidden
restoration, display changes and visual parity remain unverified. The desktop
automation surface currently exposes no controllable Windows app, so no GUI pass is
claimed. macOS PiP remains pending.

## Headless Windows native playback tests

GitHub candidate [38152037885](https://github.com/deminden/riina/actions/runs/38152037885)
passed source packaging and dependency notices but failed its 42-test native playback
gate with a process access violation. The exact native engine artifact from run
38138622911 passes the complete ignored native suite locally: 42 passed, 0 failed.
The test setup selects the bundled OpenAL backend and OpenAL Soft's `null` driver,
which verifies initialization, decoding and player lifecycle without physical
speakers; it does not verify audible output hardware. The difference between hosted
and local execution remains unresolved. The workflow also runs an integration test
against the exact extracted portable package after the native suite passes.

## HTTP URL playback

`http_url_opens_and_plays_a_local_media_fixture` serves the original
`moving-square.avi` fixture over localhost and loads its HTTP URL through the
provisioned libmpv library. The native Windows run passes and verifies advancing
playback, the five-second duration, and the selected audio track. It runs headless
with the OpenAL null output configured for the hosted-style environment; this is
not evidence of physical audio output, public network protocols, GUI URL entry,
or macOS runtime behavior.

Command: `cargo test -p riina-mpv --test native --locked --
http_url_opens_and_plays_a_local_media_fixture --ignored --nocapture`.

## macOS Live Text recognition adapter

Riina now routes the existing opt-in, paused-frame capture through Apple's Vision
text-recognition request on macOS 13 and later. Recognition runs on a worker with
an autorelease pool; the captured RGBA storage remains alive until the synchronous
request completes. Vision's normalized bottom-left boxes are clipped and converted
inside the macOS adapter to top-left pixel coordinates. Recognition results are dropped
if the media generation, paused state, or interactive crop/music mode changes.
Availability now also follows macOS's `AppleLiveTextEnabled` preference: an explicit
system opt-out hides/disables the feature, while an unset key keeps it available, as in
IINA's source availability gate.

The adapter API was source-checked for `aarch64-apple-darwin` using the pinned Rust
toolchain; macOS runtime and GUI tests are deferred until an Apple test environment is
available. A full application cross-check from Windows could not build `rquickjs-sys`
because this host has no Apple-target C compiler or SDK. This is source/API evidence
only: macOS application linking, real frame recognition, and rendering are still
unverified. On macOS, recognized regions now use read-only selectable text so users
can drag-select within each recognized line and copy the selection with the standard
shortcut. This remains a shared-renderer approximation: the reference uses VisionKit's
`ImageAnalysisOverlayView`, whose native selection, context menu, accessibility behavior,
and video-aligned geometry are not yet reproduced.

## macOS system typography inheritance

Each Slint text window now receives AppKit's system font family through its own
shared typography global: Player, Settings, Welcome, About, font picker, video
filters, history, online subtitles, and log viewer. macOS Picture in Picture uses
AVKit's native surface, so it does not use this Slint path. Text with an explicit font
family keeps that choice. On Windows the global receives an empty family, preserving
the normal toolkit fallback. Workspace compilation checks the installer for each root;
an Apple-target check was attempted from Windows but stopped in `ring`'s C build
script because a Darwin C compiler is unavailable here, before compiling Riina's
macOS code. Matched Mac captures have not verified family resolution, weight mapping,
metrics, or line wraps.

## macOS represented media titlebar

The player routes the current local media URL to AppKit's
`NSWindow.representedURL`, and keeps non-local media unrepresented. This is the
document-icon/path-menu surface used by IINA. The exact `window_document.rs` module
cross-compiles against the aarch64-apple-darwin AppKit and Slint Winit APIs in a
minimal probe. The full application target still stops in `ring`'s C build script
because this Windows host lacks a Darwin compiler, before compiling Riina's macOS
application. The titlebar label, icon, path menu and network-title behavior remain
unverified on macOS. On the current Windows tree, workspace check and strict
all-target Clippy pass; workspace tests pass with 312 app tests and 51 provisioned-
native tests ignored.

## macOS display ICC profile delivery · 2026-10-14

The shared libmpv render surface now accepts the documented ICC-profile render
parameter. On macOS, the player resolves its active `NSScreen`, extracts that
screen's ICC bytes through CoreGraphics, and refreshes the render parameter when
the player moves to a different display. The default-on Load ICC profile setting
is persisted and updates the live render policy. Automatic ICC selection is enabled
only after a display profile has been installed; explicit HDR output turns it off,
and a display without ICC data disables the path. The default-on Enable HDR mode
setting is persisted separately from the current player's HDR switch and reapplied
when a new source starts or its HDR metadata becomes available. The native library
symbol is optional so an older compatible libmpv can continue rendering without
this enhancement.

Windows workspace compilation covers the shared render API, settings UI, persistence,
and ABI declarations, but not the macOS-only AppKit/CoreGraphics adapter. Apple-target
compilation, live ICC setting interaction on macOS, profile switching on multiple
displays, visual color validation, and matched preference-card geometry remain
unverified.

## Multi-selection playlist removal

The playlist toolbar's Copy, Cut, and Remove actions now honor the selected rows.
Cut copies the selected paths in visible playlist order, then submits their stable
native IDs in one intent. The planner resolves every ID against the same observed
playlist snapshot, sorts the resulting indices, and adjusts each later removal for
the rows already removed, matching IINA's `IndexSet` behavior. Missing and duplicate
IDs reject the whole intent before native commands are issued. Core planning tests
cover non-adjacent rows, reversed selection order, stale IDs, duplicates, and the
empty selection. The Windows workspace compiles with the updated toolbar dispatcher.
The New action now reaches its independent-player launch handler. Live Windows
selection/cut, new-player launch, and removal of the currently playing row remain
unverified.

## Multiple command-line media paths

Startup now retains every positional media argument, including paths containing spaces, and submits the batch through the same directory expansion and natural ordering used by file selection. HTTP, HTTPS, RTSP, and RTSPS arguments now pass shared URL validation and remain in order among local file selections instead of being interpreted as filesystem paths. The internal renderer switch remains a first-argument control and is not treated as media. Windows and `x86_64-apple-darwin` workspace source checks pass; strict all-target Windows Clippy and strict Apple-target app Clippy pass. No URL playback or GUI run was performed. The wider IINA CLI option grammar remains unimplemented.

## Current binary candidate verification

Candidate run [38174384779](https://github.com/deminden/riina/actions/runs/38174384779)
at source commit `eae179b` has passed its hosted native playback and lifecycle
gate against this DLL, including the queued-frame shutdown regression. The app
build, runtime import audit, and playback/lifecycle test from the exact extracted
ZIP also passed. Its Windows package and complete-source artifacts are available.
GitHub skipped the final cross-archive audit and publisher because this run reused
the native artifact, leaving that dependency intentionally skipped. The workflow
conditions now explicitly gate those jobs on successful package and source jobs;
the corrected candidate still must pass the complete audit before publication.

The exact release DLL from native-engine run 38138622911 has SHA-256
`1749e6b5519afa383c88442e97026754e4e169bd46107ecc0ab0b08d205ad0af`.
Its complete runtime manifest passes locally. All 45 current native playback/lifecycle
tests pass against it with mpv's clocked null audio output, including independent
players and the HTTP range fixture. The preceding 43-test suite also passed with
OpenAL Soft's null device. Workspace tests, formatting and strict Clippy pass locally.
These runs cover the current Windows source changes; they do not certify GUI,
physical audio hardware, HDR display or macOS runtime behavior.

Hosted native checks still crashed during A-B loop shutdown using mpv's
null output. Changing the audio backend and retaining libmpv did not resolve that failure.
The hosted mini dump identifies `libmpv-2.dll+0xf4a510` making a virtual call at
`[rax+0x38]` into the unloaded `MMDevApi.dll` range during audio-device notification
cleanup. This matches the pinned engine's endpoint-notification unregister path.
The Windows playback worker now owns a COM MTA guard before core creation and
releases it only after core destruction on that same thread. The hosted guarded
candidate passes the A-B loop, HTTP and independent-player checks, then hangs while
waiting for async requests on interlaced-media shutdown. The separate wait is now
removed: `mpv_terminate_destroy` starts termination and owns core joining. A new
native regression closes and reopens paused interlaced media with frame-step and
capture commands still queued. All 45 tests pass locally in about 28 seconds;
hosted verification of the complete suite remains required. Native
modules now remain loaded for the application lifetime while each player destroys
its own core; shutdown tracing records accepted shutdown, callback removal, native
destruction and worker joining. The additional native regression closes one of two
players, verifies the survivor can seek while paused, then reopens playback after
both earlier cores have been destroyed. The HTTP fixture server now handles
reopened connections and byte-range requests during decoder probing and seeking.
These locally verified changes still require hosted verification before being reported as the
release fix. The application keeps its normal Windows audio selection. The full
native assertions are retained and run before the slower application checks and
build. The native step has a ten-minute outer limit, including compilation. Its
failure-only reproduction has a 90-second limit and captures a hang dump from its
own child process before stopping that child. Diagnostic results cannot replace
the failing gate. A hosted run must pass the full suite and a separate
playback check against the final extracted ZIP. Publication additionally requires
the exact source hashes, native repository revisions, notice contents, application
commit and final asset checksums to match.

Clipboard file references preserve their authority: Windows network file URLs
become UNC paths, local file URLs decode spaces and Unicode, and POSIX adapters
reject remote authorities without a supported mapping. Three platform-independent
parser regressions cover those cases and reject malformed percent encodings,
embedded NULs, and relative file URLs. Live clipboard interaction is unverified.

## Plugin runtime parity increment · 2026-10-11

Package plugin entries now expose a bounded CommonJS `require()` path for
package-local modules, resolved relative to the active entry/module file. Module
loads enforce canonical package containment, per-file size, total byte, module
count, and nesting limits; the wrapper returns `module.exports` and caches paths.
The same runtime increment adds bounded timers and message relay between paired
global/player plugin sessions. The MIT license notices name Denis Demin as
requested.

`cargo fmt --all -- --check`, `cargo build --workspace --locked`, strict
workspace Clippy, `scripts/check-parity.py`, and `git diff --check` pass on
Windows. The build used the isolated `target/verify-portable` directory. No tests
were run for this increment. CommonJS resolution and caching, timer delivery,
message relay, plugin UI behavior, and playback lifecycle remain unverified at
runtime.

## System media controls · 2026-10-12

The IINA default key map now includes the pinned system media actions for play,
pause, toggle, stop, next/previous, fast-forward/rewind, volume and mute. These
commands enter the same typed playback callbacks as ordinary key bindings.
Windows `WM_APPCOMMAND` events route through the active profile and honor the
Use System Media Controls preference; set-pause actions preserve play/pause
intent, and next/previous use the app's playlist callbacks.

`cargo fmt --all -- --check`, `cargo build --workspace --locked`, and strict
workspace Clippy pass on Windows. The build used `target/verify-portable`.
Parity inventory validation and `git diff --check` are recorded after this
implementation update. No tests or hardware-key GUI checks were run. Hardware
delivery, playlist boundary behavior, macOS remote-command integration and
matched interface behavior remain unverified.

The macOS source path now registers IINA-matched remote transport, custom forward
and backward skips, position and rate changes, and repeat-mode cycling. It queues
MediaPlayer callbacks onto the Slint UI thread, tracks the configured repeat state,
and publishes media type, title, audio artist/album, duration, queue and chapter
positions, elapsed time and playback rate to Now Playing while system controls are
enabled. Key-profile changes refresh the remote targets after IINA's one-second
settling interval. The feature clears its Now Playing entry and removes command
targets when disabled. Its conditional macOS code could not be compiled or
exercised on this Windows host; Control Center, hardware-key delivery and target
lifecycle remain unverified on macOS. Windows compilation does not validate this
code path.

The Windows `cargo fmt --all -- --check`, workspace build, strict workspace Clippy,
and `cargo test --workspace --locked` all passed using `target/verify-portable`.
The app test binary reports 104 passed and two clipboard tests ignored. The 45
libmpv integration tests were ignored because they require the pinned native DLL.
These shared checks do not compile the conditional macOS source or verify macOS
playback and UI behavior.

## Utilities thumbnail cache · 2026-10-12

The Utilities page now displays the size of Riina's app-owned thumbnail cache and
confirms before clearing it. Cleanup only removes regular entries within the
expected `riina/thumb_cache` directory, preserves the directory, and skips
symlinks. The locked Windows workspace build and strict Clippy pass. Both
cache-clear unit tests pass within `cargo test --workspace --locked`; the app test
binary reports 106 passed and two clipboard tests ignored. The 45 libmpv integration
tests remain ignored because they require the pinned DLL. Rendered Settings
interaction remains unverified. The feature does not provide thumbnail generation
or preview rendering.

## Key binding profile and action coverage · 2026-10-12

All five selectable built-in profiles now have original starter maps instead of
empty mapping lists. Generic mpv profile actions are parsed into bounded argument
vectors and sent through libmpv without shell execution; external process-launch
commands are rejected. Quotes, escapes and input flags are handled;
profile-specific section actions are rejected. Semicolon-chained
commands are split with quoted and escaped semicolons preserved, then submitted as
separate libmpv commands. Screenshot, fullscreen, A–B loop and shuffle actions use
Riina's existing typed callbacks. The IINA Default also binds playlist and chapter
panels, fit-to-video, Music Mode and Picture-in-Picture actions. Its reveal-file
action reads the current playlist item and delegates local paths to the platform
file manager. Its maps remain partial and do not reproduce the full upstream
profiles. The `live-text` action checks native availability, persists the toggled
preference and updates the paused-frame recognition overlay. PiP surfaces remain
unimplemented on macOS and Linux.

`cargo fmt --all -- --check`, `cargo build --workspace --locked`, and strict
workspace Clippy pass on Windows. The build used `target/verify-portable`.
Parity traceability and `git diff --check` are recorded after the inventory
update. No tests or GUI checks were run. Full profile coverage, action-builder
editing, input capture, mpv OSD flag behavior, and live shortcut dispatch remain
unverified.

The chained-command parser/planner change is also formatted and compile-checked
with the same workspace build and Clippy run; runtime action submission and GUI
behavior are not verified.

## Key-binding profile lifecycle · 2026-10-12

The Settings profile menu now offers empty-set creation, duplication of the
selected built-in or user set, renaming a selected user set, `.conf` import through
an asynchronous native file chooser, and profile-file reveal. Successful create,
duplicate, rename and import actions select the resulting user profile and persist
it as the active profile. Built-in sets remain read-only, and deleting a user set
requires confirmation. Live Settings interaction and matched page pixels remain
open. Final source passes
`cargo fmt --all -- --check`, `cargo build --workspace --locked`, strict workspace
Clippy, parity traceability, and `git diff --check`. The parity checker reports 192
initial atomic requirements with valid traceability and an incomplete inventory.
No tests or GUI verification were run.

## Windows plugin video overlay · 2026-10-12

The Windows native-video presentation now hosts permission-gated plugin overlays
in Wry/WebView2 child views. Package resources are canonicalized under the plugin
root and served through a network-restricted local protocol. The adapter keeps the
IINA simple-mode document semantics, carries bounded two-way messages, preserves
opacity across document navigation, and passes pointer input through except where
the DOM element has `data-clickable`. Shared-renderer windows report that native
video presentation is required instead of placing the child over the controller.

`cargo fmt --all -- --check`, `cargo build --workspace --locked`, strict
`cargo clippy --workspace --all-targets --locked -- -D warnings`, parity
traceability, and `git diff --check` pass on Windows. `cargo test --workspace
--locked` passes; 45 provisioned-libmpv tests and two clipboard-environment tests
are ignored by default. Build artifacts use `target/verify-portable`. No live
WebView2, media playback, or visual comparison was performed. macOS/Linux overlay
adapters, multiple player instances, complete host APIs, permission failure
feedback, and plugin lifecycle behavior at runtime remain open.

## Playlist network browser action · 2026-10-12

Selected HTTP/HTTPS playlist entries now expose an Open in Browser action. The
shared URL validator checks the original authority form before parsing, accepts
case-insensitive HTTP schemes, and rejects parser-repaired malformed URLs. The
platform adapters dispatch through ShellExecuteW on Windows, `open` on macOS, and
`xdg-open` on Linux.

`cargo fmt --all -- --check`, `cargo build --workspace --locked`, strict workspace
Clippy, `cargo test --workspace --locked`, parity traceability, and `git diff
--check` pass on Windows. Browser delivery and playlist interaction were not
tested in the GUI; macOS/Linux builds and runtime behavior remain unverified.
Selected playlist rows use Slint context menus for Open in Browser and Copy URL.
IINA's network section header and matched native menu appearance remain open parity
work.

## Thumbnail Preview preference controls · 2026-10-12

The Interface > Thumbnail Preview section now exposes enablement, maximum cache
size, remote-volume generation, and width. Defaults match the pinned reference:
enabled, 500 MB, remote volumes disabled, and 120 pt. Values are validated and
stored in the root preferences schema, and older files inherit the defaults.

Windows formatting, locked workspace build, strict Clippy, workspace tests, parity
traceability, settings-navigation check, and diff check pass. The 46
provisioned-libmpv native tests and two clipboard-environment tests remain ignored
by default. No Settings GUI capture was made. The extractor also passes against the
pinned DLL and the player now feeds frames to its seek-bar hover preview. Cache
reuse and eviction and platform volume detection are implemented; cache-hit/live
playback interaction, Settings GUI capture, and matched macOS/Windows rendering
remain open.

## Accessibility motion preference · 2026-10-12

The player's on-screen control, title-bar, compact-mode, and OSD fades now use
zero duration when either Riina's saved Disable Animations preference is enabled
or the Windows/macOS system Reduce Motion preference is active when the player
window is initialized. The system preference remains a launch-time snapshot;
live OS preference notifications, Linux detection, a full animation audit, and
matched GUI captures are still open.

## Isolated thumbnail extraction · 2026-10-12

`riina-mpv` now exposes a cancellable thumbnail worker that opens a second silent,
paused libmpv instance, samples 101 evenly spaced positions, copies RGBA frames,
and downsizes them while preserving aspect ratio. This leaves the visible player's
clock, pause state, and audio path outside the extraction worker. Its native test
passed against the pinned Windows DLL and the moving-square fixture: all 101 frames
arrived at 64×36, timestamps were ordered, and neighboring samples included changed
pixels (4.70 seconds). Pure resize tests cover downscaling, avoiding upscaling, and
the output dimension bound. The player starts this worker for local video sources
and cancels it asynchronously when the source or thumbnail settings change. Cache
read/write and budget eviction plus Windows/macOS/Linux mounted-volume checks are
implemented. Live playback/source-switching, cache-hit and eviction interaction,
macOS/Linux engine tests, and rendered GUI parity remain open.

## Timeline hover preview integration · 2026-10-12

The player polls its latest playback snapshot and starts a worker for local video
sources. The worker performs mounted-volume inspection, cache reads, JPEG encoding,
and decoding on a separate silent libmpv instance; the UI drains a bounded stream of
frames. Hovering the seek bar updates the time label and shows the nearest available
earlier frame. Replacing the source or changing thumbnail preferences cancels the
old worker asynchronously. Network URLs are skipped. Source metadata and preview
width key cached frame sets; older entries are evicted to the configured byte limit.
Windows, macOS and Linux mounted-volume metadata gates generation when remote-file
previews are disabled. Cache reuse now has an app-worker test; visual placement, live
pointer/source-switch interaction and native macOS/Linux execution remain unverified.

`cargo fmt --all -- --check`, `cargo build --workspace --locked`,
`cargo test --workspace --locked`, and strict all-target workspace Clippy pass on
Windows using `target/verify-motion`. The test suite reports 117 app unit tests
passing; 46 provisioned-libmpv native tests and two clipboard-environment tests are
ignored by default. The standalone extractor's native test was run explicitly
against the pinned DLL in the prior verification pass. No GUI or macOS check was
run for this integration.

## GitHub plugin installation and version checks · 2026-10-12

The Plugins preference now opens a repository installer with the three official IINA
plugins and accepts either `owner/repo` or a GitHub repository URL. A background
worker checks the latest release for an `.iinaplgz` asset and falls back to the
repository's `main` source archive if the asset is missing or invalid. Downloaded
archives are size-bounded and use the same path, manifest and package validation as
local imports. Requested permissions are reviewed before installation, and installed
plugins remain disabled until enabled. Refresh compares installed `ghVersion` values
with each repository's current `main/Info.json` metadata. Update prompts can stage a
repository package, require the same plugin identifier, review only added permissions,
preserve existing grants and enablement, and roll back the package if the updated
runtime does not start.

Windows strict all-target Clippy passes with warnings denied; the standalone workspace
check also passed before the update-application path was added. Direct Rust 1.99
rustfmt formatting was applied. The Apple-target source check could not complete
because this Windows host lacks the `cc` cross-compiler required by `rquickjs-sys`.
No tests or GUI install,
update, playback, lifecycle or matched-pixel checks were run. Live update prompts,
active-plugin restart, and native macOS execution remain unverified.

## Online Media plugin and built-in extractor coordination · 2026-10-12

The official `io.iina.ytdl` plugin now disables mpv's built-in extractor during
startup and immediately after live enable/disable. Riina keeps the user's saved
built-in setting intact while showing IINA's disabled control state and guidance
when the plugin is enabled. The parity inventory still marks this requirement
partial: plugin-provided quality selection and downloads, real-site playback,
captured Windows and pinned macOS settings surfaces, and runtime lifecycle remain
open.

The Windows `cargo check --workspace --locked`, direct Rust 1.99 rustfmt check,
parity inventory check and `git diff --check` passed. This toolchain has no Cargo
fmt or Clippy components installed. No tests, GUI, playback, or macOS check were run
for this change.

## Multiple player opening policy · 2026-10-12

General settings now save IINA's allow-multiple-player preference (default on),
simultaneous-open grouping (default off), and duplicate-player policy (default off).
After one media item is loaded, later file, URL, and recent-media requests can launch
separate players; grouped file selections share one player process. Resume requests
carry their saved position into the new player. With multiple players disabled, the
existing player continues to handle the request. Duplicate filtering covers the
current player's playlist and each incoming batch unless the multi-item grouped-open
preference routes the whole batch to one player; grouped batches bypass the filters.

The shared-renderer path now owns secondary `PlayerWindow` and `UiSession` instances
in one process, matching IINA's independent-player lifecycle more closely. The
Windows native-video path still uses the process launcher. Windows `cargo check
--workspace --locked`, direct Rust 1.99 rustfmt, the 197-row parity inventory check,
Settings navigation check, and `git diff --check` passed before this in-process
manager change. No tests, GUI, playback, or macOS checks were run for the manager.
Sharing plugin state and history across players, cross-process duplicate detection and
focus, directory/playlist grouping semantics, the Windows native-video manager, and
matched Settings/player captures remain open.

## Keep window open after playback · 2026-10-12

General settings now expose and persist IINA's `keepOpenOnFileEnd` behavior, enabled
by default. The value is passed into each new libmpv session and changes the active
session through a typed command. The Windows workspace check passed for the source
revision including this setting, as did direct Rust 1.99 formatting, the 198-row
parity check, and `git diff --check`. No tests, GUI, playback, or macOS checks were
run for this preference; EOF replay and playlist-boundary behavior remain
unverified.

## App lifetime after closing the last window · 2026-10-12

General settings now persist IINA's `quitWhenNoOpenedWindow` preference, default
off. The shared-renderer manager restores a Welcome window when the final player is
closed with quitting disabled; closing Welcome leaves the application event loop
available, while enabling the preference quits after the last managed window closes.
Windows native-video still uses its existing process lifecycle, and application
reopen via the macOS Dock or Windows activation is not implemented. The Windows
workspace check passed for this source revision. No tests, GUI, close/reopen, or
macOS runtime checks were run.

## Duplicate media across managed players · 2026-10-12

When duplicate players are disabled, the shared-renderer manager now compares each
incoming item against the playlists of every managed player. Already-open media is
skipped and its existing player is shown for separate opens; repeated entries in a
separate-player batch are also removed. A grouped simultaneous-open request bypasses
both duplicate filters, matching IINA's batch policy. `media_open` regression tests
cover preserving a grouped batch and filtering an ungrouped batch. Cross-process
Windows native-video behavior and live focus/open verification remain unverified.

## Grouped opens and duplicate filtering · 2026-10-12

When simultaneous multi-item opens are grouped into one player, the entire batch
now bypasses duplicate-player filtering, even when the saved allow-duplicates option
is off. This matches IINA's grouped branch, which routes the batch before applying
the separate-window duplicate check. The same grouped-request predicate is used by
selection preparation and the in-process player manager so accepted paths are not
filtered a second time. Separate-player batches continue to skip already-open media
and repeated entries. `cargo test --workspace --locked`, strict all-target Clippy,
Rust formatting, `git diff --check`, and the 214-row parity check passed. The active
Riina process still holds `target/debug/riina.exe`; no replacement build, GUI, or
playback check was performed.

## General settings and fullscreen pause behavior · 2026-10-12

General > Behavior now presents the captured IINA disclosure rows collapsed by
default and right-aligned switches in the Opening media card. Expanding the rows
retains existing media-open options and exposes new fullscreen entry/exit policies.
The saved defaults are both off. Entering fullscreen resumes only when configured
and currently paused; leaving fullscreen pauses only when configured and currently
playing. The 608 × 500 pinned macOS General capture informed the row order, 40-point
disclosure rhythm and 36 × 18-point switch geometry; those pixels are source-measured,
not a verified Windows comparison. The retained Windows Preferences image predates
the current 600 × 480 window, and no desktop UI surface was available for a fresh
capture.

Windows `cargo check --workspace --locked`, direct Rust 1.99 formatting, parity
traceability (199 rows), and `git diff --check` passed. No GUI, fullscreen playback,
or macOS runtime check was run; minimize/inactive/sleep pause behavior remains open.

## Additional confirmed-state OSD messages · 2026-10-12

The shared playback observer now routes confirmed volume and mute changes, audio delay,
primary/secondary subtitle delay and position, subtitle visibility, and selected track
changes to the transient video OSD. Volume progress follows the active 100% or opt-in
200% ceiling. Evidence for this slice is a Windows workspace compile and direct
rustfmt checks of the changed Rust files. No behavior-specific tests were added or run;
message ordering/coalescing, native GUI timing, matched pixels, and macOS/Linux runtime
remain unverified.
## OSD change queue · 2026-10-12

The UI retains multiple distinct feedback changes observed in one coalesced playback
snapshot and presents them one at a time using the configured OSD timeout. Pending
messages are discarded when OSD is disabled or the media generation changes. Windows
workspace compilation and direct Rust formatting checks pass. Behavior tests and live
validation of ordering, timer progression, matching pixels, and macOS/Linux rendering
remain outstanding; the deterministic comparison order is not proven to match native
property callback order.
## Additional confirmed-state OSD feedback · 2026-10-12

The shared observer now reports crop selection, video rotation, subtitle scale, effective
file/playlist loop mode, and A/B loop endpoint changes. It waits for an already-active
media phase before treating observed values as user feedback, avoiding startup-option
changes being presented as interactive actions. The Windows workspace compile and Rust
format check pass. No behavior tests were added or run; endpoint coalescing, exact IINA
wording/order, timer rendering, matched pixels, and macOS/Linux runtime remain unverified.
## Explicit-action OSD feedback · 2026-10-12

Stop now retains its message across the stopped-state update, and accepted chapter
selection, playlist append/paste, and playlist-clear actions show their source-style
transient feedback. File append counts are taken after media discovery; failed or empty
actions produce no success message. Workspace compilation, formatting, and parity
traceability pass. No behavior tests or GUI checks were run; timing, menu/pointer parity,
matched pixels, and other-platform behavior remain unverified.

## Linux user media associations · 2026-10-12

Utilities now installs a user-scoped `org.riina.player.desktop` entry and maps all
66 extensions in the shared media-type manifest to MIME types before applying the
selected defaults with `xdg-mime`. The command work runs off the UI thread and
reports partial failures. Rust formatting, whitespace checks, a locked Windows
workspace build, and strict all-target Windows Clippy passed. Those Windows checks
exclude the Linux-only implementation; this host has only the Windows Rust target
installed. Linux compilation, desktop-session
integration, and opening media from the file manager remain unverified.

## macOS Finder alias selection · 2026-10-12

Shared selection now resolves input paths through Foundation's alias-file URL API
before directory expansion, matching IINA's `Utility.resolveURLs` pass. Network
URLs and malformed paths bypass that file-URL conversion, and resolver errors keep
the original path as IINA does. Rust formatting and the Windows workspace checks
pass for the shared wrapper. The Windows build excludes the Foundation implementation;
Apple-target compilation, alias fixtures, file selection, and playback on current
macOS remain unverified.

## macOS Now Playing asset URL · 2026-10-12

The Mac media-controls snapshot now publishes the current playlist item as a local
file URL or preserves its validated network URL in
`MPNowPlayingInfoPropertyAssetURL`, matching IINA's Now Playing metadata. Rust
formatting and the Windows workspace build/Clippy cover the shared compilation
surface. Apple-target compilation and Control Center/Now Playing runtime behavior
remain unverified; external artwork behavior is recorded below.

## macOS Now Playing cover artwork · 2026-10-12

Now Playing metadata supplies `MPMediaItemPropertyArtwork` for external album-art
files, and an independent paused libmpv decoder captures the selected embedded
album-art video track without seeking the active player. The external image is
decoded by ImageIO directly to an orientation-correct thumbnail with a 768-pixel
maximum; embedded pixels are encoded to a bounded JPEG before delivery to
MediaPlayer. Quick Look/timeline-thumbnail fallbacks and IINA's internal
default-on artwork opt-out are implemented below.
Formatting, the locked Windows workspace check, strict Clippy and the focused
Apple API binding check pass, but the Windows checks exclude macOS-gated code. The
full Apple-target check stops in `ring` before application sources because this
host lacks an Apple-target C compiler. Control Center rendering remains
unverified. No tests or GUI checks were run.

When no album-art track exists and timeline preview is enabled, the macOS system
media controller now captures the timeline frame before IINA's 60-second artwork
position, or before the midpoint for videos shorter than two minutes, using the
independent paused decoder. Embedded track-cover capture requests only its first
frame instead of generating the full 101-frame timeline set.

## macOS Now Playing Quick Look fallback · 2026-10-12

Artwork selection now follows IINA's front-cover, Quick Look, then timeline
thumbnail order. Quick Look uses a 2000-pixel request, scale 1 and the low-quality
representation, retrying up to three times before falling back to the timeline
sample. It runs away from the UI thread, and its returned CGImage is encoded to
JPEG before passing bytes to MediaPlayer. Network resources skip Quick Look and
timeline fallback. The hidden `enableNowPlayingArtwork` key defaults to true and
is checked during playback, so changing it removes or restores the artwork entry.
Rust formatting, the locked Windows workspace check, strict
all-target Clippy, and the workspace build's compilation phase pass. A focused
Apple-target API check also passes. The complete Apple-target workspace check
stops in `ring` before the app because this Windows host lacks an Apple-target C
compiler; the build could not replace `target/debug/riina.exe` because a running
Riina process holds it open. These host checks do not verify macOS playback,
Control Center rendering, or the full app build. No tests or GUI checks were run.

## Pause and resume when inactive · 2026-10-12

The default-off preference is saved and shown on macOS only. AppKit activation
notifications enqueue per-player actions, and Winit focus events maintain the
set of focused Riina player windows. Each player remembers the generation it
paused and resumes only after both the app and its own window are active, if the
preference remains enabled and playback is still paused. Focus moving to a
non-player window such as Preferences does not trigger the policy. A focused
Apple-target API check passed for both AppKit notifications and the Slint/Winit
focus accessor. The full Apple-target app build is blocked before app sources by
`ring` requiring a missing Apple-target C compiler; native focus timing remains
unverified. No tests or GUI checks were run.

## Pause and resume on minimize · 2026-10-12

The General disclosure now includes the default-off minimize preference. A
50 ms UI timer observes Slint's minimized state. It pauses only a currently
playing generation and resumes only that same generation when the window is
restored and that playback remains paused. This preserves user pause state and
does not resume a stopped item or a replacement item. The shared Windows
workspace check and Clippy cover the wiring, but native minimize/restore timing
and GUI appearance have not been verified. No tests or GUI checks were run.

## Playlist clipboard interoperability · 2026-10-12

Playlist copy/cut use every selected row in playlist order. Paste now inserts at
the first selected row, falling back to the clicked row and then the start. The
macOS clipboard writes IINA's native filename property-list representation and
plain text, and also publishes each selection as an NSURL object through AppKit's
`writeObjects` API. Paste reads IINA filename lists first, then all file-URL and
URL items, then plain text. A focused x86_64 Apple-target API compile check passed
with warnings denied. The full app Apple-target check stops in `ring` because no
Apple-target C compiler is installed on this Windows host. The locked Windows
workspace check, strict all-target Clippy, formatting check, parity traceability
check and diff whitespace check pass. Native pasteboard interoperability and GUI
behavior were not tested.

## Live system Reduce Motion · 2026-10-12

The existing 500 ms appearance refresh now also updates the player window from
Windows or macOS system motion preferences while it remains open. A retained
timer applies changes to the reusable Preferences window as well. The stored
Riina preference remains independent and combines with the system setting. The
Windows workspace check and strict all-target Clippy pass; the full Apple-target
check remains blocked in `ring` by the missing Apple-target C compiler. Linux now
reads `org.freedesktop.appearance/reduced-motion` from the XDG Settings portal and
applies later `SettingChanged` signals on the next UI refresh. The Linux portal
module passed a focused x86_64 Apple-target API compile check with warnings denied;
this checks the Unix D-Bus source without proving a Linux application build or
portal runtime. Portal runtime, complete animation inventory, and native GUI
behavior remain open. No tests or GUI checks were run.

## macOS pause on system sleep · 2026-10-12

General preferences now store the default-on IINA `pauseWhenGoesToSleep` policy.
The macOS player observes `NSWorkspaceWillSleepNotification` and sends a queued
pause action to the UI, which pauses only loaded, actively playing media and
honors the saved toggle. Rust formatting, the locked Windows workspace check,
strict all-target Clippy cover the shared property and persistence wiring. A
focused Apple-target binding check passed for the observer and notification
types. The full Apple-target workspace check stops in `ring` because this
Windows host lacks an Apple-target C compiler, and native sleep/wake playback
remains pending. No tests or GUI checks were run.

## macOS Now Playing profile commands · 2026-10-12

MediaPlayer play, pause, toggle, stop, next, previous, skip-forward and
skip-backward actions resolve against the current profile and dispatch its last
matching binding on the UI thread, retaining the reference fallback when no
binding exists. Forward and rewind preferred intervals are loaded from the same
profile snapshot and refreshed at most once per second while system controls are
active, avoiding repeated profile-directory/file reads on the 50 ms UI timer.
The locked Windows workspace check, strict Clippy and formatting pass, but do not
compile the macOS dispatch. The Apple-target workspace check is blocked in
`ring` by the missing Apple-target C compiler; hardware-key and Control Center
behavior remain unverified. No tests or GUI checks were run.

## Incremental Windows interface smoke check · 2026-10-12

Launched the rebuilt 0.3.1 debug application with Cargo's default incremental
compilation enabled. The home screen showed the version, Open File, Open URL,
and recent-media list. Opening the `moving-square.avi` sample reached the player
surface and displayed its synthetic test frame. The playlist then advanced to
`long-silence.wav`; its playback position advanced from 0:08 to 0:24, and the
Pause control changed to Play with a Paused status after input. This confirms
basic recent-media navigation, video rendering, and playback state controls.
Audio output quality, the full control set, and visual parity with the macOS
reference remain unverified. The C: drive had about 197 GB free after the build;
`target\debug` was 19.64 GB, including a 6.41 GB incremental cache.

## Plugin file API and activation gates · 2026-10-12

`cargo test --workspace --locked` passed all 143 application tests and the other
workspace tests, including plugin file API, manifest-domain, private and granted
external move, external overwrite-rejection, and local HTTP transport coverage.
The 46 native libmpv integration tests remain ignored because their pinned runtime
is not provisioned in this test run.
`cargo clippy --workspace --all-targets --locked -- -D warnings`,
`cargo fmt --all -- --check`, `git diff --check`, and the parity traceability
checker passed. The HTTP transport test exercises an allowed localhost JSON
response; the JavaScript Promise bridge, external network behavior and redirect
edge cases remain unverified. The file API currently supports exists, list,
UTF-8 text read/write, delete, and move. Binary handles, trash, track aliases,
additional encodings, cross-volume move behavior, and full IINA path semantics
remain open.

## Live Windows playback-control smoke check · 2026-10-12

The rebuilt 0.3.1 application opened on screen. Play/Pause changed the control
label and status from playing to paused and back to resumed, and the Open File
dialog was visible through the Windows computer-use surface. The first capture
showed `long-silence.wav` after a longer delay than the five-second
`moving-square.avi` fixture. A separate launch showed the next same-folder
`external-tone.wav` fixture, and saved recent-media state recorded
`moving-square.avi` at 4.998 seconds. This confirms that the command-line item
played to its end before later captures; it was not an argument failure. The latest
rebuilt app also displayed the 0.3.1 Welcome surface with Open File, Open URL, and
recent media. This smoke check verifies playback progression and controls, not
captured video pixels. Full IINA visual parity remains unverified.

## Plugin binary file handles · 2026-10-12

Implemented `iina.file.handle(path, mode)` with IINA-compatible `offset`,
`seekTo`, `seekToEnd`, `read`, `readToEnd`, `write`, and `close` methods. Handles
are owned by the plugin runtime and close when explicitly closed or when that
runtime is dropped. Reads and writes are capped at 4 MiB per call, and each plugin
runtime is limited to 64 open handles. Absolute paths still require the declared
file-system permission; private `@data` and `@tmp` paths remain scoped.

`cargo test --workspace --locked` passed, including 145 app tests and the new
JavaScript-facing binary-handle scenario. The 46 native libmpv tests remain ignored
because the native test runtime was not provisioned. `cargo clippy --workspace
--all-targets --locked -- -D warnings`, `cargo fmt --all -- --check`, and
`git diff --check` passed. This verifies the QuickJS bridge and filesystem behavior;
it does not verify a third-party plugin package or macOS runtime behavior.
## Plugin file-manager reveal dispatch · 2026-10-12

Added `iina.file.showInFinder(path)` and routed it through Riina's platform file
manager adapter. Private plugin paths remain available; absolute paths are resolved
only with the reviewed file-system permission. Windows selects the item in Explorer,
macOS uses Finder reveal, and Linux currently opens the containing folder as its
platform adaptation.

`cargo test --workspace --locked` passed all 146 app tests and the remaining
workspace tests; the 46 native libmpv tests were ignored because the runtime was not
provisioned. `cargo fmt --all -- --check`, `git diff --check`,
`cargo clippy --workspace --all-targets --locked -- -D warnings`, and the parity
traceability checker passed. Tests verify path existence, private and granted external
path resolution, and rejection of an ungranted external reveal request. The OS
file-manager launch was not invoked by the tests, so runtime interaction on Windows,
macOS, and Linux remains unverified.

## Plugin text encoding support · 2026-10-12

`iina.file.read(path, { encoding })` now accepts UTF-8, UTF-16, UTF-32, strict
ASCII, Latin-1, and common WHATWG encoding labels, including the IINA names for
Windows code pages, Shift-JIS, EUC-JP, GBK, and GB18030. Decoding is strict: malformed
byte sequences and unknown/unsupported labels return errors instead of silently
substituting text. Foundation encodings absent from or mapped differently by the
portable decoder remain an explicit parity gap.

Added the pinned `encoding_rs` dependency and tested Windows-1252 and Latin-1
byte semantics, BOM-selected UTF-16/32, Shift-JIS, malformed UTF-8, and unknown
labels. `cargo test --workspace --locked` passed all 147 app tests and the remaining
workspace tests; the 46 native libmpv tests were ignored because the native test
runtime was not provisioned. `cargo clippy --workspace --all-targets --locked -- -D
warnings`, formatting, and diff checks passed.

A Rust license packaging smoke run collected `encoding_rs` license and copyright
files, then stopped at the already-locked `webpki-roots` 1.0.9 package because its
`CDLA-Permissive-2.0` data license is not in the collector's reviewed grant list.
That dependency was present in the committed lockfile before this change. I did not
change the license policy; the complete package notice audit remains open for release.
## Plugin-private path normalization · 2026-10-12

Private `@data` and `@tmp` paths now normalize `.` and in-root `..` segments, matching
IINA's standardized path expansion for valid private paths. A parent segment that
would escape the plugin root, rooted paths, and drive-prefix paths are rejected;
existing symlink containment checks remain in force.

`cargo test --workspace --locked` passed all 148 app tests and the remaining workspace
tests; 46 native libmpv tests remain ignored without the pinned native runtime.
`cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D
warnings`, `git diff --check`, and the parity traceability check passed.

## Plugin media and external-track file aliases · 2026-10-12

Player plugin sessions now receive the current media directory and external track
paths from playback snapshots. `@current/<path>` resolves relative to the current
media directory and requires the reviewed file-system grant. `@video/<id>`,
`@audio/<id>`, and `@sub/<id>` resolve to the matching external track filename;
these track aliases follow IINA's direct track resolution before the ordinary
filesystem permission gate. The global plugin session remains independent of player
context, and context updates are retried if the bounded host command queue is busy.

The alias resolver test passes for granted current-media access, permission denial,
external audio-track resolution without the generic filesystem grant, and missing
track handling. Workspace cargo check and app clippy passed; formatting, diff, and
parity traceability checks passed. Native path behavior and live plugin-session
playback changes remain unverified in the GUI.

## Plugin core playback actions · 2026-10-12

`iina.core` now emits typed player commands for open, OSD, pause, resume, stop,
relative seek, absolute seek, speed, and chapter selection. Local paths are resolved
through the permission-aware plugin file resolver; network media URLs require the
network permission and a manifest-allowed domain for HTTP(S) and SMB. Other URL
schemes, including RTSP, follow IINA's filesystem-permission branch. Commands are
delivered to the player's engine through the host queue.

Runtime tests verify command shapes and numeric validation, plugin-private media
opening, and file/network permission boundaries. `cargo test --workspace --locked`
passed all 153 app tests and remaining workspace tests; 46 native libmpv tests were
ignored because the pinned runtime is not provisioned. Workspace cargo check, app
clippy, formatting, diff, and parity traceability checks passed. No GUI playback test
was run for these actions; history/recent/version getters, UI visibility, and other
IINA core proxy APIs remain open.

The read side of the core API now exposes the playback status proxy and chapter
snapshot through player context updates. The proxy covers paused, idle, position,
duration, speed, video dimensions, network-resource state, URL, and title; unknown
status names return undefined, while unavailable known values return null. A runtime
test verifies snapshot shape and chapter serialization; a host adapter test checks playback-to-status mapping. GUI playback updates
and native/macOS behavior remain unverified.


## Plugin audio, subtitle, and video track APIs · 2026-10-12

The core plugin API now reads audio/subtitle/video track snapshots and writes track
selection, audio/subtitle delay, volume, and mute through typed player commands.
External audio, subtitle, and video loads share the media resolver, including network
permission and allowed-domain checks. Serialized tracks include IINA's field names,
forced/default/selected flags, and demux metadata; formatted titles include track ID,
title, codec details, and default status. Language labels use Riina’s platform-aware
display-name adapter to match the active system locale.

Runtime tests cover command mapping, null-selection no-op behavior, value bounds, and
network-load denial. Host tests cover selected track snapshots and serialized metadata.
The Windows workspace test suite passed 154 app tests and remaining workspace tests;
46 native libmpv tests were ignored because the pinned runtime is not provisioned.
Clippy, formatting, and diff checks passed for this worktree. Live player-session command
delivery, macOS/Linux runtime behavior, GUI captures and live locale changes remain unverified.

## Plugin core UI visibility · 2026-10-12

The runtime test verifies that `iina.core.setUIVisibility(true/false)` emits the
matching IINA disable-UI flag, and the host test verifies the flag reaches the player
UI message queue. `cargo fmt --all -- --check`, parity traceability, and
`git diff --check` pass. The focused UI visibility tests passed, followed by a passing
`cargo test --workspace --locked` run (156 application tests; 46 native libmpv tests
ignored because the pinned runtime is not provisioned). Workspace Clippy with
`-D warnings` also passed.
No GUI or macOS/Linux runtime test has been performed for this method.

## Plugin core recent documents · 2026-10-12

`iina.core.getRecentDocuments()` now returns the persisted recent-media entries
as `{name, url}` objects and adds successful current playback items to the live
snapshot. The plugin host receives the saved list on startup and updated snapshots
from the playback callback. Tests verify absolute file URLs, remote URL names and
ordering, host serialization, and the JavaScript getter. The focused mapping and
runtime tests passed. `cargo test --workspace --locked` passed with 157 application
tests; 46 native libmpv tests were ignored because the pinned runtime is not
provisioned. Workspace Clippy with `-D warnings`, formatting, parity traceability,
and `git diff --check` passed. No live GUI or macOS/Linux behavior has been verified.

## Plugin core version · 2026-10-12

`iina.core.getVersion()` now exposes the IINA-compatible `{iina, build, mpv}`
shape using Riina's package version, the release run number (or `development` for
local builds), and the loaded engine's observed `mpv-version` property. The native
manual defines that property as the mpv version/copyright string, which may be a
release version or a Git hash: <https://mpv.io/manual/stable/#property-list>. The
The property-decoding test verifies both the engine string and its transition to
unavailable state. The runtime test verifies all three returned fields. The focused
tests and `cargo test --workspace --locked` passed (157 application tests; 46 native
libmpv tests skipped because the pinned runtime is not provisioned). Workspace
Clippy with `-D warnings`, `cargo fmt --all -- --check`, parity traceability, and
`git diff --check` passed. `cargo build --workspace --locked` passed. No native
GUI version panel or non-Windows app runtime is being claimed for this API.

## Audio equalizer · 2026-10-12

The portable audio pane now exposes ten frequency bands, the pinned built-in
curves, and save/rename/delete for named profiles. Profile gains persist in the
preferences document. The player labels its lavfi filter and removes only that
filter when replacing a curve; libmpv's `af` property feeds the observed gains
back to the controls. The implementation uses the pinned frequencies and width
calculation while keeping the rest of the audio-filter chain intact.

The five focused `riina-core` audio-equalizer tests passed, including persistence,
graph validation, profile operations, and decoding the observed filter. The
ignored native integration `audio_equalizer_applies_updates_and_returns_to_flat`
also passed with `tools/mpv/libmpv-2.dll` and a null audio output. `cargo check
--workspace --locked` and `cargo fmt --all -- --check` passed. The GUI controls
have not been exercised, and there is no matched macOS capture or macOS/Linux
native playback result for this feature.

## Mixed media and subtitle drops · 2026-10-12

The dropped-file route now preserves subtitle files when a batch also contains
playable media. It submits one typed open intent and attaches the subtitle paths
after libmpv reports the replacement media loaded, without selecting the new track.
A later load or stop cancels the deferred attachment. Small `.url` Internet
Shortcut files resolve only when their `[InternetShortcut]` section contains an
allowed, validated network URL. The parser unit test rejects out-of-section
values, unsupported schemes, malformed URLs, and missing targets. The focused
live-libmpv test `opening_dropped_media_with_subtitles_attaches_them_to_the_new_file`
passed with the provisioned Windows runtime. `cargo check --workspace --locked`,
formatting, and strict workspace Clippy passed. The workspace suite passed for the
mixed-subtitle implementation before shortcut parsing was added; the parser test
passed separately. The suite marks 48 provisioned-libmpv integration cases ignored,
with the focused drop/subtitle test run separately. GUI drag-and-drop, raw URL/text
payloads, playlist-row insertion, and macOS/Linux behavior remain unverified.

## Localized delay entry · 2026-10-12

Audio, primary subtitle, and secondary subtitle delay fields now format observed
values with ICU decimal data for the system locale and accept that locale's decimal
and grouping symbols on commit. Display precision matches IINA's three significant
digits; editing and playback commands retain their entered precision. Grouping must
be valid in three-digit groups; malformed grouping and multiple decimal marks are
rejected. Five focused tests passed, including real ICU `de-DE` formatting and an
Arabic-locale round trip for native digits and bidi marks. Workspace check and strict
Clippy passed. Rendered-field geometry, live locale changes, matching macOS
screenshots, and runtime behavior on macOS/Linux remain unverified.
# Manual-open repeat mode

`media_open::tests::manual_open_repeat_is_opt_in` confirms the preference does
not add a loop command by default. `manual_open_repeat_maps_the_selected_loop_mode`
checks playlist and file loop mappings, and
`invalid_manual_repeat_mode_is_rejected_only_when_enabled` ensures malformed
stored selection is ignored while the feature is off and reported when enabled.
The command is applied after explicit manual media-load actions. Native playback
looping and live preference persistence remain separate GUI/native checks.

# Plugin file trash

`plugin_file_api_scopes_private_paths_and_checks_external_access_grants` checks
that `iina.file.trash` is exposed through the JavaScript bridge, requires the
file-system permission for `@current` media paths, and rejects plugin storage
roots. `move_scopes_private_paths_and_requires_grant_for_external_paths` checks
that an absolute external path remains in place when the grant is absent. The
OS Trash/Recycle Bin move itself and macOS/Linux runtime behavior remain
unverified.

# Plugin file encoding names

The pinned IINA encoding-name map includes Foundation aliases beyond browser
encoding labels. The decoder now accepts ISO Latin-5 and Thai, KOI8-R, Big5-E and
Big5-HKSCS-1999, and ISO-2022-JP-1/-2/-3 names where the portable codec has a
compatible base decoder. Focused cases are included in
`text_read_supports_iina_encoding_names_and_rejects_malformed_input`. Big5-HKSCS
and ISO-2022-JP variants currently use the base codec tables and can differ for
extension-specific characters; EBCDIC, CNS/EUC-TW, HZ, UTF-7, and specialized Mac
encodings remain unsupported. Exact decoding parity requires macOS runtime checks.

## macOS and Linux workflow corrections · 2026-10-13

The v0.3.3 application-port workflow exposed strict macOS lint/build failures and
one Linux test that assumed a Windows drive path. Removed the target-only unused
imports, changed the ImageIO thumbnail-options return type to Core Foundation's
retained type, bounded macOS pinch-resize position arithmetic before converting to
native i32 coordinates, and made the local-media path test use a host-native absolute
path while retaining platform-specific Windows-drive assertions. The complete
Windows workspace suite passes after these changes. A Windows-hosted Linux
cross-check stopped at missing pkg-config/sysroot and C compiler tooling; an Apple
cross-check stopped at the missing target C compiler/Apple SDK. The actual macOS and
Linux application builds still need a hosted workflow rerun and remain unverified.

## Windows window placement · 2026-10-13

Initial window geometry now clamps point-based offsets to the selected display,
including displays with negative virtual-desktop origins. Video-driven resizes
retain their centered placement while clamping the result to the current monitor.
The first media resize previously reused the reduced fit-size limit as monitor
positioning bounds, placing fit-height windows too high. It now centers against
the full current display while keeping the independent fit-size limit, accounts
for the decorated outer frame, and uses outer dimensions for initial centering.
Windows now recenters after the shared chrome's first layout and first playback
state, then debounces native move/resize events until the frame settles. It uses
Win32 monitor/frame rectangles to avoid DPI virtualization and placeholder
coordinates. Later user Music Mode toggles preserve the current frame center.
A regression test for a 1152×648 window on a 1280×720 display passes and expects
position `(64, 36)` before the native frame inset is applied. Fresh live placement
verification on the scaled Windows desktop measured outer bounds
`[1452, 441]–[2388, 1718]` on a 3840×2160 monitor (center error: 0 px horizontally,
0.5 px vertically). The exact test instance was closed afterward.
Music Mode entry and exit use the same centered resize path on shared and native
Windows presentation surfaces.
The native Windows host starts centered on the primary display,
so the first player surface no longer inherits an arbitrary Winit placement.
Ten focused `riina-core` window-resize tests passed. The workspace build, strict
workspace Clippy, and full workspace test suite passed (48 native libmpv cases
remain ignored without the provisioned runtime). A fresh player launch measured
outer bounds `[174, 27]–[3667, 2132]` on a 3840×2160 primary monitor, with center
error 0.5 px horizontally and vertically. That exact test process exited normally
and no Riina process remained.
First-show placement now also centers the Welcome, About, Preferences, Playback
History, Filter, Online Subtitle, and Font Picker windows over their owning player,
clamped to the monitor work area. Reused auxiliary windows retain their last user
position. The shared placement policy passes its focused test for a negative-origin
display, owner centering, and an off-screen owner clamped into the work area. The
Windows workspace check, strict Clippy, formatting, and full workspace suite pass;
live positioning of these auxiliary surfaces still needs a grouped GUI check.
The first-show content-to-window conversion now measures the actual offset between
Winit's inner and outer screen positions. This avoids treating the title bar and
bottom resize border as equal halves, which had displaced the client contents on
decorated Windows windows. Focused tests cover asymmetric frame offsets and
negative monitor origins. A fresh desktop capture attempt failed because the
foreground window did not report a process id; this code change is not yet visually
verified.

## Windows package audit · 2026-10-13

The v0.3.3 Actions run built its Windows package and passed package playback,
but its final source audit assumed every vendored Rust notice was copied at its
source-tree path. The reused engine package instead organizes Windows-target
Rust notices by crate and version under `THIRD_PARTY_LICENSES/NATIVE_RUST`.
The audit now verifies each inventoried notice byte-for-byte and maps source
vendor paths to the target inventory, while excluding vendored crates absent
from the Windows dependency set. All 19 release-checker tests pass, and the
exact downloaded v0.3.3 package passes the corrected audit against commit
`8cf9cc222b40936deab0747dc0d8c5d866bb02e3` (101,748 indexed source files and
88 native repositories). The package checksum and three-binary PE import check
also pass. Its packaged player window could not be captured because the UI
capture crop fell outside the monitor, so its visual release check is still open.
The published ZIP was independently downloaded and its SHA-256 matches the
release checksum manifest. Its `SOURCE-COMMIT.txt` names the immutable v0.3.3
tag commit, and its packaged libmpv DLL matches the reused native-engine artifact.
A fresh computer-use window lookup returned a Riina handle that disappeared on
both capture attempts, while the next listing showed only the desktop shell; the
exact release package therefore still needs a live visual check in a working UI
session. No Riina process remained after the attempt.
On a later launch of that same extracted package, two screenshots of the Welcome
window showed a white client area while the accessibility tree exposed its normal
controls. Opening a recent media item was attempted, but the subsequent monitor
capture failed with `0x80070057`; the UI outcome could not be observed. The exact
release process was closed and confirmed absent. This is a visual-check failure,
not a passed screen comparison; reproduce it in a working capture session before
deciding whether it is a release rendering defect or a capture-path failure.

## Current debug UI access · 2026-10-13

The v0.3.4 debug Welcome window and its accessibility tree were discoverable.
Accessibility-only capture returned the Welcome controls, but screenshot capture
again failed with `IGraphicsCaptureItemInterop.CreateForMonitor ... 0x80070057`.
Control activation failed because coordinate geometry was unavailable; keyboard
input did not change the window. No visual comparison or UI interaction is
claimed. The main window was closed, the remaining console-host window was
inspected, and the exact debug process and all Riina windows were confirmed gone.

On a fresh capture attempt, the first main-window handle became stale before it
could be rehydrated; a fresh listing and handle succeeded, but screenshot capture
again failed with `0x80070057`. The accessibility-only tree still worked, while
indexed clicks reported unavailable coordinate geometry and keyboard input did
not activate Welcome actions. Alt+F4 closed the main window but left its exact
debug executable alive; that process alone was stopped, then process and window
lists were both checked empty.

After the latest workspace build, the main `Riina` window and native-video host
window were both returned for the exact debug executable. Screen capture failed
twice with `IGraphicsCaptureItemInterop.CreateForMonitor ... 0x80070057` after a
fresh application/window lookup. Alt+F4 closed the main window; its exact
remaining debug process was then stopped and verified absent. This attempt did
not produce a visual reference or interaction result.

## v0.3.3 published-package revalidation · 2026-10-13

Workflow-dispatch run [38332824004](https://github.com/deminden/riina/actions/runs/38332824004)
reused the verified native-engine artifact (`native-engine` skipped), passed
Windows native playback/lifecycle tests and the corrected exact-package source and
notice audit, then published successfully. The current v0.3.3 release has the
72,613,531-byte Windows ZIP with SHA-256
`2a72ded17b01c928cd0bdc5aec9be31c1ca286ef743bdba5201587b7910d2a74`; its
verification manifest identifies tagged source commit
`8cf9cc222b40936deab0747dc0d8c5d866bb02e3`, 101,748 indexed native source files
and 88 repositories. This closes the prior package-verification failure.

The latest debug visual attempt selected the rebuilt `target/debug/riina.exe`
window and retried after refreshing its handle. Both screenshot attempts failed
with `IGraphicsCaptureItemInterop.CreateForMonitor ... 0x80070057`. Alt+F4 closed
the player window but left the native host process, so the exact test process was
ended and verified absent. No visual comparison is claimed.

## Apple-target compilation from Windows · 2026-10-13

`cargo check --workspace --locked --target aarch64-apple-darwin` reached the
`ring` dependency and stopped because this Windows host has no target C compiler
(`cc`). The shared `riina-core` crate does compile for `aarch64-apple-darwin`.
This is not a full macOS application build; the native macOS CI/build host remains
required for app compilation and runtime evidence.

## Windows startup window placement · 2026-10-13

The newly built debug app was launched twice. With an isolated preference setting
of `action_after_launch=none` and no media, its outer frame was exactly centered
in the 1280x672 monitor work area (0 px horizontal and vertical delta). With
`tests/fixtures/moving-square.avi`, it was within one horizontal pixel of center.
Both exact test processes were closed and verified absent. The GDI desktop
capture returned an all-black image, so this verifies frame placement only, not
rendered appearance. Binary hash, frame bounds, and test scope are recorded in
`tests/visual/windows-window-placement-evidence.json`.

## Revalidation after the release · 2026-10-13

The public latest-release endpoint still identifies v0.3.3 as current, with the
72,613,531-byte Windows x64 ZIP, `SHA256SUMS.txt`, and
`RELEASE-VERIFICATION.json`. The most recent successful Windows release workflow
is [38332824004](https://github.com/deminden/riina/actions/runs/38332824004).
No second v0.3.3 release was created.

On the current dirty v0.3.4 checkout, `cargo build --workspace --locked`,
`cargo clippy --workspace --all-targets --locked -- -D warnings`,
`cargo fmt --all -- --check`, `git diff --check`,
`scripts/check-parity.py`, and `cargo test --workspace --locked` passed. The
workspace command runs 197 application tests. New tests cover toolbar
forward/backward drag reordering, invalid toolbar order data, Windows work-area
margins and frame positioning, audio-device routing, subtitle priority, and
normalized volume OSD progress at the default 100% and opt-in 200% caps. The
separate provisioned native playback suite passed 49/49, including live audio
device selection and HTTP fixture playback. The Python repository tooling suite
passed 19/19. The traceability checker validates 244 initial requirements and
still reports the inventory incomplete.

The public application-ports workflow
[38332341339](https://github.com/deminden/riina/actions/runs/38332341339) has
failed macOS application lint on both macOS runners and Linux application/shared
tests on Ubuntu. Its logs endpoint returned HTTP 403 requiring repository admin
rights, so the exact remote diagnostics are unavailable from this session.
Local cross-target Clippy attempts independently stop before application code:
the Apple target lacks a C compiler, and the Linux target lacks a configured
pkg-config sysroot. This Windows host has no WSL installation, so these failures
do not establish the remote failure causes or substitute for the native CI
hosts. The portable `riina-core` crate passes strict cross-target Clippy for
both `aarch64-apple-darwin` and `x86_64-unknown-linux-gnu`.

The latest live visual check launched the rebuilt debug app, selected its
refreshed `Riina` window, and retried capture after the first stale window ID.
Both capture attempts fail at `IGraphicsCaptureItemInterop.CreateForMonitor`
with `0x80070057`. The player window was closed, its remaining exact debug
process was stopped, and the process list was confirmed empty. No appearance or
interaction result is claimed from this attempt.

## Current feature sweep · 2026-10-13

On the current dirty v0.3.4 checkout, formatting, diff checks, all 203 workspace
application tests, strict workspace Clippy, the workspace build, the parity
traceability checker, and 19 Python tooling tests passed. The provisioned native
libmpv integration suite passed 49/49 tests in 33 seconds. A live Riina window was
enumerated and launched through Windows Computer Use, but screenshot capture failed
twice with `IGraphicsCaptureItemInterop.CreateForMonitor ... 0x80070057`, including
after refreshing the window handle. The exact test process was stopped and verified
absent. This sweep establishes automated, native playback, build and process
lifecycle results; it does not establish live GUI interactions or visual parity.

## Timed OSD behavior regression · 2026-10-13

Six focused `video_osd` regression tests bring the application suite to 203 tests.
They verify delay direction and progress for audio plus both subtitle lanes,
primary/secondary subtitle positioning and visibility, selected-track identity
including `#0 <None>`, rotation, reciprocal subtitle scaling, and playlist-loop
feedback. Stop, chapter selection, playlist append, paste and clear also share
typed action-feedback messages matching IINA's reference text. Eleven related OSD
rows now distinguish unit-tested message semantics
from pending live display timing, native event ordering, matched pixels, and
macOS/Linux runtime. The refreshed live capture attempt again failed with
`CreateForMonitor ... 0x80070057`; its exact Riina process was closed.

After resetting and reinitializing the Computer Use JavaScript session, the
rebuilt app's first window ID again expired before capture. A fresh app/window
selection still reached `CreateForMonitor ... 0x80070057`. Alt+F4 closed the
player; its exact remaining process (PID 6068) was stopped and confirmed absent.
The new toolbar drag affordance therefore has compile and unit-test evidence but
no live interaction or screenshot evidence.

## Windows placement recheck · 2026-10-13

The realized player frame is now centered immediately, before asynchronous player
setup. On this single 1280×720 display, the idle Welcome window measured
`[154, 9, 1126, 663]` within the `1280×672` work area. A clean-profile launch of
the 320×180 moving-square fixture measured `[85, 0, 1196, 672]`: its horizontal
center is within one pixel, and the frame stays inside the taskbar-free work area.
The unexpectedly near-full-work-area playback size persists after changing the
natural-size input to decoded source dimensions; the correct display capture and
the remaining resize cause still need investigation. After adding a DPI-aware
24-point automatic-size inset, the rebuilt playback window measured
`[127, 24, 1153, 648]`, exactly centered with a 24-pixel work-area margin. The
window title showed `track-languages.mkv` although the launch requested another
fixture, so this establishes placement behavior but not fixture-specific sizing.
The process was closed and confirmed absent. These are Win32 geometry measurements,
not a visual-pixel comparison.

## Current workspace and UI retest · 2026-10-13

On the current dirty v0.3.4 worktree, `cargo build -p riina --locked`,
`cargo test --workspace --locked` (209 application tests), strict all-target
workspace Clippy, formatting, `git diff --check`, and the parity traceability
check passed. The normal workspace test command leaves 49 provisioned-engine
integration cases ignored; the separate pinned-engine 49/49 pass is recorded
above and in the release verification evidence.

The debug Welcome window rendered and showed version 0.3.4. Opening the long media
sample produced a filename-bearing playback window in the fresh app listing, but
the desktop capture bridge rejected it as unusable after one re-list and
rehydration. The exact test process was closed and verified absent. This pass
confirms the Welcome screenshot only; playback controls and Quick Settings remain
visually unverified.

The requested v0.3.3 release is already published. GitHub reports successful
release-readiness, Windows packaging, package verification, and publish jobs; the
native-engine job was skipped. The immutable tag points to source commit
`8cf9cc222b40936deab0747dc0d8c5d866bb02e3`, and its release verification record
includes the reused libmpv hash. No second v0.3.3 release was created.

## Follow-up player capture · 2026-10-13

`cargo build -p riina --locked` completed for v0.3.4. The Welcome screenshot
rendered and displayed Version 0.3.4. Opening `tests/fixtures/moving-square.avi`
updated the app listing to `moving-square.avi — riina`; the refreshed playback
window was then rejected by the capture bridge as unusable after one fresh
re-list and rehydration. Process 22040, launched from `target/debug/riina.exe`,
was closed and verified absent. This does not verify playback pixels or live menu
behavior. `cargo check --workspace --locked`, `cargo fmt --all -- --check`, the
209-app-test workspace suite (49 provisioned-libmpv integration cases remain
ignored by the default command), the parity traceability check (245 rows), and the
19 Python tooling tests passed after the shared and Windows native-video Playback/
Audio menu state wiring. Strict workspace Clippy also passed for the current source
revision. A fresh development binary build then completed with those changes.

The rebuilt executable was launched again for visual validation. Welcome capture
again showed version 0.3.4 and the recent-media list. Opening
`tests/fixtures/moving-square.avi` changed the app listing to
`moving-square.avi — riina`, but the refreshed playback HWND remained unusable
after one re-list and rehydration. Process 22384 was closed and verified absent.
No playback pixels or live menu state were visually verified.

## Verification sweep and Windows last-window lifecycle · 2026-10-13

The current v0.3.4 workspace passed `cargo build --workspace --locked`,
`cargo test --workspace --locked` (207 app tests), strict workspace Clippy,
formatting, the parity traceability checker (244 inventoried rows; still incomplete),
and the Python tooling suite (19 tests). The explicit provisioned libmpv suite passed
49/49 native playback tests. Windows placement was rechecked: the shared-renderer
player's native outer frame is exactly centered at `[154, 9]` in the 1280×672 work
area. The isolated Welcome frame was also exactly centered at `[354, 99]`.

With `quit_when_no_opened_window=false`, closing Welcome restored the empty native
player and the process stayed alive; closing that player ended the isolated test
process. With the preference enabled, closing Welcome exited. These process-level
checks cover Windows only. Visual screenshot APIs still fail (`CreateForMonitor`
0x80070057); GDI and PrintWindow returned black client pixels, so no visual
comparison is claimed. The published v0.3.3 package remains unchanged; v0.3.4 has
not been prepared for release.

## IINA menu state and modifier alternates · 2026-10-13

The shared menu and Windows native-video menu now bind Pause/Resume, speed,
volume, mute, and audio delay to playback observations. Pressing Alt/Option switches
the existing menu rows to IINA's alternate frame-step, fine speed, fine volume, and
fine audio-delay actions. Workspace `cargo check`, all 209 app tests, strict Clippy,
formatting, and the 246-row parity traceability check passed. Menu interaction is
still unverified because the playback HWND cannot be captured. A fresh debug binary
build completed with the alternate-row implementation.

## Owner-monitor placement and refreshed desktop capture · 2026-10-13

New player processes now receive the owning player's screen-space center. The
Windows startup host selects the work area containing that point, so playback
opened from a secondary-display player starts on that same display. In-process
shared-renderer children continue to center over the owner. An explicitly
configured initial position remains authoritative. The startup-argument regression
test verifies the internal coordinates are removed from media arguments.

`cargo fmt --all -- --check`, strict workspace Clippy, `cargo test --workspace --locked`
(210 app tests; 49 provisioned-libmpv tests remain ignored by default), and
`cargo build --workspace --locked` pass. The parity traceability checker validates
246 rows and reports that the inventory remains incomplete.

The Windows desktop helper captured the Welcome surface and a player with the menu
bar, playback controls, and Video Quick Settings visible. The fixture was paused at
its final timestamp and the captured video area was black, so rendered video pixels
are not verified. Switching Quick Settings tabs caused the helper to lose the window
handle; test process 8652 was stopped and confirmed absent. Audio-pane interaction
and live multi-monitor placement still need verification.

## Fresh rebuild, pinned native suite, and playback capture retry · 2026-10-13

After cleaning the 53.2 GiB project-local dev cache, `cargo test --workspace --locked`
passed (212 app tests); `cargo build --workspace --locked` passed. The explicit
serial native suite passed 49/49 tests using `tools/mpv/libmpv-2.dll`. Its digest
matched `reference/native-lock.json`: `a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`.

The app opened the `moving-square.avi` fixture and initially reported that filename
in the player title. Same-folder playlist progression changed the title to
`external-tone.wav — riina` before capture. The Windows computer-use helper rejected
the playback window as unusable after a fresh list and rehydration retry. Process
21816, launched for this check, was stopped and verified absent. This attempt does
not verify playback pixels, menu actions, Quick Settings, or live window placement.

## Deterministic standalone Windows placement · 2026-10-13

Standalone player and auxiliary windows now use the primary monitor's work area
when no visible owner supplies a display. Pointer position is no longer used as
an implicit monitor selector; child windows still follow their explicit owner,
and configured initial geometry remains authoritative. The placement fallback
has a focused unit regression for owner-versus-primary selection. `cargo build
-p riina --locked` passed after removing an incomplete Slint shortcut lookup that
prevented the workspace from compiling. A fresh debug Welcome view was captured,
but the UI helper rejected its window handle during shutdown and again after a
fresh selection. Only the exact launched debug process was stopped; a process
check confirmed it was gone. The capture did not expose screen coordinates, so
live centering and multi-monitor placement remain unverified. A subsequent
`cargo test --workspace --locked` passed all 213 app tests, including the
owner-versus-primary placement fallback regression.

## Immediate Windows first-show placement · 2026-10-13

Windows placement now tries to center a new player or auxiliary window as soon
as its Winit handle is available. The existing bounded retries remain for
windows whose native frame is not realized yet. This removes the unconditional
80 ms wait that could show a new window at Winit's temporary default location
before moving it to the owner or primary work area.

The focused placement tests passed (5 tests), and `cargo build -p riina
--locked` passed. On the rebuilt app, a Win32 frame-bounds check measured the
visible Welcome window at `[354, 99, 926, 573]` within `[0, 0, 1280, 672]`,
exactly centered with zero-pixel delta. The screenshot attempt still failed in
Windows Graphics Capture (`CreateForMonitor failed: 0x80070057`), so rendered
visuals and multi-monitor behavior remain unverified. The exact Riina debug
process started for this check was stopped and confirmed absent. The measurement
is in `tests/visual/windows-window-placement-evidence.json`. `cargo test -p
riina --locked` passed 211 tests (2 ignored); strict app Clippy, formatting,
and `git diff --check` passed.

## Profile-driven menu shortcut hints · 2026-10-13

The shared and Windows-native menus now derive shortcut labels for supported
playback, track, window-size, panel, and window actions from the active key-binding
profile. Both surfaces consume the same ordered shortcut model; absent mappings
produce empty hints. IINA's Meta+0/1/2, Meta+-/=, screenshot, playlist, chapter,
speed, audio-delay, and window-level mappings were added to the built-in profile.
Fit to Screen and the source window-scale actions now dispatch through the
corresponding shared resize command.

`cargo test -p riina key_binding_preferences::tests --locked` passed (8 tests),
including active-profile replacement and IINA default shortcut conversion.
`cargo build -p riina --locked`, strict workspace Clippy, and
`cargo test --workspace --locked` passed (213 app tests). The 49 provisioned-libmpv
native integration tests are ignored by default. A fresh Welcome capture was
possible, but the UI helper failed on close and again
after a fresh selection; it did not capture the updated menus. The exact debug
process was stopped and confirmed absent. Live menu visuals and interaction remain
unverified.

## Current release and native playback recheck · 2026-10-13

GitHub currently lists v0.3.3 as a published stable release with the Windows
x86_64 ZIP, `SHA256SUMS.txt`, and release-verification record. The Windows release
workflow and package-verification checks succeeded. The requested 0.3.3 binary
release is therefore already deployed; the local workspace remains v0.3.4 and
has not been released.

The explicit serial suite against the pinned `tools/mpv/libmpv-2.dll` passed all
49 native playback tests. Its first run exposed a Windows-only fixture-server
socket mode error in the HTTP playback case. Accepted sockets are now switched
back to blocking mode before the request worker starts; the isolated HTTP test
and the complete native suite then passed. The full workspace tests passed 213
app tests (49 native tests remain ignored by default), strict workspace Clippy,
formatting, and the v0.3.4 app build passed.

The latest public macOS/Linux application workflow (run 93 at commit
`7e31e85`) failed at Full application lint on both macOS runners and at
Application and shared unit tests on Ubuntu. The Windows workflow for that
commit succeeded. A Windows-host Apple cross-target lint attempt stopped before
project code because the Apple-target C compiler needed by `ring` is unavailable
on this host; macOS runner failures still need their job-log diagnosis. The
saved Video menu capture predates the active-profile shortcut changes, and the
fresh desktop screenshot helper still fails with
`CreateForMonitor failed: 0x80070057`; updated menu visuals remain unverified.

## Video size menu waits for valid geometry · 2026-10-13

The Video menu's size actions depended on dimensions from the first `Playing`
update. If mpv reported source/output dimensions one event later, those actions
stayed disabled. Riina now updates natural video geometry whenever it becomes
available, triggers the pending initial resize once the first valid dimensions
arrive, and clears stale dimensions while media loads, stops, or fails. Later
geometry changes update the size commands without repeatedly resizing the window.

Three focused tests cover display-size preference, source-size fallback,
rotation, and missing/zero geometry. `cargo test --workspace --locked` passed
216 app tests; strict workspace Clippy, formatting, and `cargo build -p riina
--locked` passed. Fresh menu pixels could not be captured in this run, so the
size-item enabled state still needs live visual confirmation.

## Quick Settings menu titles follow the active pane · 2026-10-13

IINA changes the Video, Audio, and Subtitles menu action to “Hide … Panel” while
that settings tab is open, then restores “Show … Panel” when another tab or
sidebar is active. Riina's actions already toggled the correct pane but their
labels were static. The shared and Windows-native menu bindings now track the
active pane on the configured settings side. `cargo check -p riina --locked` and
`cargo fmt --all -- --check` passed. Live menu interaction and matched visuals
remain unverified.

The Plugin menu now changes between “Show Plugins Panel” and “Hide Plugins
Panel” as the plugin sidebar opens and closes, following IINA's `updatePluginMenu`
behavior. The follow-up `cargo check -p riina --locked`, strict workspace
Clippy, and formatting check passed. Live menu interaction and matched visuals
remain unverified.

## Autoplay window-transition flicker · 2026-10-13

Pinned mpv Windows source shows that `wid` creates a child video HWND and
installs a parent-position hook that resizes it asynchronously. Riina now
synchronously batches media-size changes for the video host, this mpv child,
and the transparent controller. Both the media-size and aspect-lock callbacks
now honor the configured resize timing, so automatic playlist changes do not
resize the window when timing is “when opened manually” or “never”. The Windows
native path also enables mpv playlist URL prefetch to reduce load gaps. The audit
records direct child HWND bounds and samples screen pixels in the video area.

The follow-up executable SHA-256 is
`D20D1F74479C667AC5CBC71BDEFEC47F883C62E9AD510B8A9147E338E87F41A6`. A
35.35-second loop through wide, standard, portrait, audio, and return-video
fixtures recorded zero host/child size mismatches across 2,082 stable 10 ms
samples, zero off-screen samples, and normal shutdown (PID 5640, exit code 0).
The previous asynchronous-child build had one mismatch at 5,879 ms, when the
host had changed to 908×454 but the video child was still 960×483. The follow-up
audit also saw eight brief `IsWindowVisible` false samples but no host hide/show
events in the Windows event hook. This verifies synchronized bounds, not rendered
pixels; the user-visible flash remains open pending a live visual check.

The final Windows executable SHA-256 is
`C22E0C3E5FD6D1A26D802D8F3EF659CF33D0B9F5EE80ED70FBBD8F0E69EB3AB4`. A fresh
35-second autoplay loop covered wide, standard, portrait, return-video, and audio
fixtures. It recorded zero host/child size mismatches and zero off-screen
samples, with normal shutdown (PID 8120, exit code 0). During the video-to-video
portion, automatic playlist changes kept the same window frame. Screen-pixel
sampling found no dark handoff sample on the first two video transitions; one
near-black sample remained at 15,950 ms as the higher-resolution return-video
fixture began. This is a short decoder/render handoff, not a window resize. The
visible transition issue is reduced but not fully resolved.

The earlier asynchronous-child build had one mismatch at 5,879 ms, when the host
had changed to 908×454 but the video child was still 960×483. The synchronized
three-surface change removed that mismatch. The 49 pinned-DLL playback/lifecycle
tests passed against `target/ci-native-engine-38138622911/libmpv-2.dll`; the
focused resize-timing regression test also passed. `cargo build --locked`,
`cargo test --workspace --locked` (221 app tests; 49 native tests ignored in the
ordinary workspace run), strict workspace Clippy, formatting, and `git diff
--check` passed after the follow-up.

## Native video handoff frame · 2026-10-13

Windows native playback captures a generation-bound outgoing frame about 0.35 seconds before EOF. It keeps that image through mpv's end-of-file state when autoplay has a successor, while an incoming video buffers, and through its first 0.75 seconds because mpv can coalesce time updates and publish frame metadata before the surface is visible. It clears for an audio-only successor; an observed paused frame may release it earlier. Five focused display-policy tests pass.

A clean 23-second autoplay run set resize timing to Always, disabled resume positions, and covered wide, standard, portrait, return-video, and audio clips in order. It sampled a 320-by-180 center crop 771 times at a 29.6 ms average and 174 ms maximum gap. None of the three video-to-video transitions or the video-to-audio transition had an all-dark sample. The run recorded one host/child size mismatch sample during the portrait-to-wide resize, no off-screen samples, and normal shutdown. A fresh 18-second repeat recorded 563 samples at a 31.8 ms average and 172 ms maximum gap; it had no all-dark video-to-video sample or child-size mismatch. Six all-dark samples occurred during startup/first-frame startup in the longer run. Earlier `IsWindowVisible` samples reported windows hidden even as full screen captures showed the moving video and controls; that flag is not a flash measurement. Audio-to-video handoff, the user's media, and flashes shorter than the sampling gap remain unverified, so keep the issue open until it is reproduced on those files.

`cargo build --workspace --locked`, `cargo test --workspace --locked` (226 app tests passed; 49 native integration tests ignored), strict workspace Clippy, formatting, and `git diff --check` passed after the 0.75-second grace-window adjustment. A mixed-codec audit captured the rendered interlaced pattern after startup; its center-crop darkness was caused by the fixture image, so that crop is not a valid blank-frame detector for this source. Automatic-resize behavior on the user's media and flashes shorter than the 174 ms maximum sampling gap remain unverified. The local change is post-release and is not included in v0.3.4.

## Riina 0.3.4 Windows binary release · 2026-10-13

Tag `v0.3.4` points to application commit `0a4c8b361f31b01882e20f81245963be17ac07a2`. GitHub Actions run `38389488319` completed successfully: the package job reused the successful native-engine artifact, verified its runtime hash, passed native playback/lifecycle and lint/unit checks, assembled the Windows x64 ZIP, and passed exact extracted-package/source verification before publication. The release is public at `https://github.com/deminden/riina/releases/tag/v0.3.4`.

The published ZIP is 72,854,337 bytes. Its downloaded SHA-256 `833a95e8fa74cc13138eb911cc25ce70c84e7851da9fc230552dbf7344f0eaae` matches the public `SHA256SUMS.txt`. `RELEASE-VERIFICATION.json` records source commit `0a4c8b361f31b01882e20f81245963be17ac07a2`, 101,748 indexed native-source files, 88 native-source repositories, and the package's libmpv and Cargo.lock hashes. The exact downloaded package contains its GPLv3 and third-party notices, runtime/source manifests, source commit and source-access instructions. The Windows runtime import checker verified all three packaged PE binaries. A local package smoke run played `moving-square.avi`, advanced to `track-languages.mkv`, maintained aligned/on-screen windows and exited normally (code 0); no Riina process remained.

## Windows transition resize follow-up · 2026-10-13

The prior audit caught one frame where the native host had resized for the incoming portrait clip but mpv's child video HWND still had the outgoing clip's dimensions. Windows now aligns the transparent controller and every mpv child surface during the host resize notification. The outgoing transition-frame request also moves from 0.35 to 1.25 seconds before EOF so mpv has more time to return it before autoplay advances.

The fresh 25-second autoplay audit used the existing wide, standard, portrait, return-video, and audio fixtures with automatic resizing enabled. The executable SHA-256 was `E6190293BDEBCD15D1015DEEE9B5074592617D12A1B8CB30E1D8D917BF6970`. It recorded 825 stable samples at a 27.9 ms average and 214 ms maximum gap, zero host/child size mismatches, zero all-dark samples after startup, zero off-screen samples, and normal exit. Three `IsWindowVisible` samples reported false during the video sequence; full captures still showed the video and controls, so these are not counted as flashes. `cargo build --workspace --locked`, `cargo test --workspace --locked` (226 app tests passed; 49 native tests ignored), strict workspace Clippy, formatting, and `git diff --check` passed. The local fix is not in v0.3.4. The user's affected files and flashes shorter than the audit sampling gap remain unverified.

## Windows transition rendering · 2026-10-13

After the previous resize fix, higher-resolution screen captures at the first video-to-video handoff still showed a narrow incoming-frame strip leaking through above the retained outgoing image. The incoming child surface could resize without a host `WM_SIZE` event, so the Windows native path now checks and aligns the mpv child and transparent controller every 16 ms, issuing a batch resize only when bounds differ. The transition image now fills the overlay client area; an opaque black matte covers aspect-fit bands so the incoming surface cannot show through. The ordinary playback image geometry is unchanged.

`cargo build --workspace --locked`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked` (226 app tests passed; 49 native integration tests ignored), `cargo fmt --all -- --check`, and `git diff --check` passed. The rebuilt executable SHA-256 was `3FC70EE2A5867FB228EF6632A21DC4FC0531FDAECF98A29CDC34C2F1CC01D025`. A 32-second GUI autoplay run covered wide, standard, portrait, return-video, and audio fixtures; it recorded 1,161 samples, zero host/child size mismatches, zero off-screen samples, one dark sample at startup, and normal shutdown. Full captures during the first video-to-video handoff show the retained outgoing frame with clean black bars and no incoming strip. The test process closed normally and no Riina process remained. The user's affected files and sub-cadence flashing remain unverified.

## Windows playlist handoff and resize follow-up · 2026-10-13

The user continued to report flashes during clip switches. The transition cover now waits for an `mpv` raw capture from the incoming video instead of releasing on a fixed 0.75-second guess. It briefly displays that incoming capture while the native surface catches up; if capture fails, playback time provides a three-second release fallback. On Windows, autoplay changes that would reduce the player window area retain the current size to avoid downsizing the D3D11 host during decoder replacement. Manually opened media still follows the configured resize policy.

`cargo build --workspace --locked`, `cargo test --workspace --locked` (228 app tests passed; 49 native integration tests ignored), strict workspace Clippy, `cargo fmt --all -- --check`, and `git diff --check` passed. The final rebuilt executable SHA-256 was `C582808B3176A17C9CF4C2966EB41C0717AEF4013817A91F48BE80D5DAE708B5`. The 27-second GUI autoplay audit covered wide, standard, portrait, return-video, and audio fixtures. Across 822 stable samples after startup, the average capture gap was 30.4 ms and the maximum was 364 ms; it found no all-dark video samples, no off-screen samples, and no host/child size mismatches. The video window kept the same frame size across the three video-to-video transitions. Riina closed normally (exit 0), and no process remained. This synthetic run does not settle flashes shorter than the maximum sample gap or behavior on the user's files.

## Windows constrained controller layout · 2026-10-13

The controller's floating position is clamped to the current viewport. Its height follows the available video area; below 48 points the timeline is omitted, and below 114 points width only Play/Pause remains in the center row. `cargo build --workspace --locked`, `cargo test --workspace --locked` (228 app tests passed; 49 native integration tests ignored), strict workspace Clippy, formatting, and `git diff --check` passed after this change.

A live 980-by-330-pixel resize kept video playback visible, on-screen and aligned between the window host and mpv child, then closed normally. The capture did not show the controller even after moving the pointer into the resized window, so this only verifies video/window stability; visible button placement and interaction at that size remain open.

## Compact controller extreme viewport bounds · 2026-10-13

The compact transport now drops horizontal insets and gaps below 64 logical
points, and limits its row height and vertical spacing to the available height
below 24 points. This targets transiently tiny resizes where fixed controller
chrome could extend beyond its viewport. The workspace build is in flight;
pointer interaction and a visible capture at the affected sizes remain
unverified.

## Windows clip-switch resize ownership · 2026-10-13

The native Windows mpv output now sets `auto-window-resize=no`, leaving video-size
changes to Riina's configured resize policy and synchronized host/child geometry.
`cargo build --workspace --locked`, `cargo test --workspace --locked` (228 app
tests passed; 49 native playback tests ignored in the ordinary run), strict
workspace Clippy, formatting, and `git diff --check` passed.

A 25-second GUI autoplay run on wide, standard, portrait, return-video, and audio
synthetic clips recorded 1,045 stable samples, zero fully dark video samples, zero
host/child size mismatches, and zero off-screen samples. It captured 52 full-window
transition frames and exited normally; no Riina process remained. The screen
sampler checks every 10 ms and transition captures are 40 ms apart, so briefer
flashes and the user's affected media remain unverified. Evidence is in the ignored
local audit directory `tools/window-stability-audit/autoresize-disabled/`.

## Automatic Music Mode handoff stabilization · 2026-10-13

`music_mode::State` now waits 300 ms of stable audio-only track catalog before
switching layout. Regression tests verify that a later video-track update or an
unknown catalog cancels the candidate. `cargo build --workspace --locked`,
`cargo test --workspace --locked` (228 app tests passed; 49 native playback tests
ignored in the ordinary run), strict workspace Clippy, and formatting passed.

A 25-second Windows autoplay run covered wide, standard, portrait, return-video,
and audio fixtures. All four video items kept the same player size; the audio item
entered Music Mode after its title appeared. The run recorded 1,047 stable samples,
zero fully dark video samples, zero host/child mismatches, and zero off-screen
samples, then exited normally with no Riina process left. Evidence is in the ignored
local audit directory `tools/window-stability-audit/music-mode-stability/`. The
user's affected files and macOS runtime remain unverified.

## Windows incoming-frame handoff cover · 2026-10-13

The incoming transition frame remains visible for 750 ms after a successful raw
capture before the native D3D11 child is uncovered. `cargo fmt --all -- --check`,
`cargo test --workspace --locked` (228 app tests passed; 49 native playback tests
ignored in the ordinary run), strict workspace Clippy, and `cargo build --workspace
--locked` passed. A 32-second Windows autoplay run covered wide, standard, portrait,
audio, and return-video fixtures. Across 1,136 stable samples it recorded no dark
video samples after startup, no host/child size mismatches, and no off-screen frames.
Full captures across video handoffs show a retained image followed by the incoming
frame without a black gap. Riina closed normally (exit 0), and no process remained.
Evidence is under the ignored local directory
`tools/window-stability-audit/incoming-hold-075-experiment/`. This synthetic run
does not confirm the user's affected files or flashes shorter than the capture
cadence.

## Incoming-frame capture stabilization · 2026-10-13

Incoming raw-frame capture now waits until playback has advanced 250 ms, scaled
down for very short media. The outgoing frame remains over the native surface
during that settling interval. Eight transition-policy tests pass. The full
workspace suite passes; 49 native playback tests are ignored because the pinned
DLL is not provisioned. Strict workspace Clippy, formatting, and the workspace
build pass.

A 28-second Windows autoplay GUI run covered six mixed-size synthetic clips with
automatic resizing enabled. It recorded 733 samples, zero host/child size
mismatches, and zero off-screen samples, then closed normally with no Riina
process remaining. Full-window captures during the two sampled mpv child-hide
events show the retained frame covering the video surface; captured handoffs show
the incoming image appearing after the outgoing frame without a blank gap. The
mostly-black synthetic fixtures make dark-pixel counts unreliable. Evidence is in
the ignored local files `tools/window-stability-audit/incoming-stability-20261013-retry.json`
and `tools/window-stability-audit/transition-*.png`. The user's affected media
and flashes shorter than the screen-capture cadence remain unverified.

## Windows clip-switch follow-up · 2026-10-13

`cargo fmt --all -- --check` and `cargo build --workspace --locked` passed after
adding a 300 ms stable-host-size gate before incoming-frame capture and capture-
before-navigation for manual Next/Previous (500 ms timeout fallback). The local
26-second Windows autoplay audit used six generated clips and “Always resize.” It
recorded 609 samples, zero host/child size mismatches, zero off-screen samples,
normal shutdown, and no remaining Riina process. The screen sampler found extended
black intervals in the generated interlaced clips; those cannot be attributed to
the player transition from this evidence. A separate Windows smoke check sent
Ctrl+Right and advanced from `01-square.avi` to `02-tracks.mkv`, exiting normally.
It did not capture the manual transition visually. The user's media remains
unverified. Evidence: ignored local directory
`tools/window-stability-audit/switch-surface-stability-20261013/`.

## Advanced preferences and log viewer GUI · 2026-10-13

`cargo fmt --all -- --check` and `cargo build --workspace --locked` passed. A
Windows GUI smoke check started with Advanced disabled, enabled it, confirmed the
saved preference was true, opened the Log Viewer, read four fixture entries, and
observed a fifth entry appended while the viewer remained open. Captures verify
the Search field and toolbar actions fit at the default window size. The test
harness's normal close timed out, so it ended the exact Riina process it launched;
no Riina process remained afterward. Filter interaction and the remaining viewer
actions are not GUI-verified. Evidence: ignored local directory
`tools/window-stability-audit/log-viewer-smoke-20261013/`.

## Windows autoplay surface-size stability · 2026-10-13

An initial GUI audit with “Always resize” played four generated video clips. Across
455 stable samples the host stayed 2747×1711 physical pixels, with zero host/video
child size mismatches and zero off-screen samples. Repeated captures exposed two
gaps: the video aspect-ratio callback bypassed the automatic-resize guard, and a
playback-state update briefly changed the client-area height while the outer frame
stayed steady, consistent with a native-menu refresh. The Windows handoff rule now
covers the aspect callback, and menu updates are coalesced for 80 ms. After
`cargo fmt --all -- --check`, `cargo build --workspace --locked`, and the focused
Windows automatic-playlist regression test passed, a 20-second GUI replay with
“Always resize” covered six generated clips. Across 644 stable samples, the host
and mpv child stayed 2747×1711 physical pixels, with zero size mismatches and zero
off-screen samples. Riina exited normally and no process remained. Generated
interlaced frames still produced dark center crops, so this does not prove the
user's reported flash is resolved on their files. Evidence: ignored local
directories `tools/window-stability-audit/resize-trace-20261013/`,
`tools/window-stability-audit/hold-frame-autoplay-recheck-20261013/`,
`tools/window-stability-audit/resize-stack-trace-20261013/`, and
`tools/window-stability-audit/menu-coalesce-autoplay-20261013/`.

## Refresh playback menu state before native menu paint · 2026-10-13

The Windows native-video menu now receives a synchronous state refresh for pause,
speed, volume/mute and audio delay at `WM_INITMENU`. This closes the gap where the
ordinary 80 ms menu-update coalescer could leave the first displayed menu frame
with stale values. Microsoft documents `WM_INITMENU` as the point when a menu is
about to become active and may be modified before display ([Win32 reference](https://learn.microsoft.com/windows/win32/menurc/wm-initmenu)).
The change is being workspace-checked; live menu-open behavior
still needs GUI verification against a playing video. Localized presentation and
matched IINA pixels also remain open.

## Keep final frame at EOF · 2026-10-13

Extended the ignored native integration test to assert that an EOF snapshot retains
video metadata and the current playlist item before Play is pressed. With the pinned
`tools/mpv/libmpv-2.dll`, `cargo test -p riina-mpv --test native --locked
play_restarts_media_after_end_of_file -- --ignored --nocapture` passed (1 test).
The test also confirms restart from the beginning. It uses a headless mpv output;
visible final-frame pixels and playlist-end setting combinations still need GUI
verification. No GUI player was left open.

## Windows clip-switch frame handoff · 2026-10-13

`cargo fmt --all -- --check`, `cargo build --workspace --locked`, and
`git diff --check` passed. The capture-reply path preserves a transition frame when
its command reply arrives one media generation after the request, while rejecting
older captures. Windows native mode no longer creates a second libmpv OpenGL render
context over the D3D11 child output. The cover waits for the visible, size-matched
mpv child surface and uses opaque captured pixels. A 24-second Windows GUI autoplay
run covered six generated clips and collected 995 screen samples. It recorded zero
host/mpv-child size mismatches and zero off-screen frames; Riina exited normally.
Generated interlaced clips still produced dark center samples. This does not
confirm the user's reported flash is resolved on their media or rule out gaps
shorter than the capture cadence. Evidence: ignored local directory
`tools/window-stability-audit/native-render-separation-20261013/`.

## Playback History group navigation · 2026-10-13

`cargo check --workspace --locked` passes with per-group toggling and Expand All /
Collapse All actions wired to the Playback History view model. Group state is
retained across history refreshes and reset when history is cleared. A bounded
Windows GUI smoke attempt did not open Playback History through its shortcut, so
the new controls were not exercised and no matched macOS/Windows capture was made.
The test process closed normally; no Riina process remains.

## Windows clip-switch surface readiness follow-up · 2026-10-13

The transition cover now treats the visible, size-matched mpv child as the
presented output surface, so a hidden stale child cannot block readiness. The
workspace formatting check and `cargo build --workspace --locked` passed. A
25-second Windows autoplay audit resized the player to a 960×600 physical
outer frame after 8 seconds and sampled 951 frames across six mixed-size clips.
It recorded zero host/child size mismatches and zero off-screen samples; the
test process exited normally and no Riina process remained. A second targeted
14-second capture around clip changes found no hidden host samples. The very
dark center samples belong to the generated interlaced fixtures. This does not
reproduce the user's reported flashes or rule out gaps shorter than the screen
capture cadence; the affected media still needs direct verification. Evidence is
in the ignored local directories `tools/window-stability-audit/transition-user-followup-20261013/`
and `tools/window-stability-audit/transition-switch-frames-20261013/`.

## Windows transition-cover render ordering · 2026-10-13

The Slint transition image is now assigned before enabling its opaque cover,
preventing an intermediate render from showing the cover's black matte before
the retained frame. After incoming-frame capture, the cover now remains visible
for 750 ms of monotonic wall time, regardless of playback speed. The focused
`incoming_frame_grace_uses_wall_clock_at_high_playback_speed` regression passed at
8× playback speed. `cargo fmt --all -- --check`, `cargo build --workspace --locked`,
and `git diff --check` passed. A live six-clip Windows autoplay check captured
normal rendered content on later clips, but the point-in-time screenshots did not
sample quickly enough to prove that brief flashes are gone or to reproduce the
user's files. Ctrl+Q did not close the test player, so the exact Riina process
started for this run was stopped; a subsequent process check found no Riina process.
Keep the reported flash open pending high-rate screen capture and direct media
verification.

## Windows incoming-video readiness · 2026-10-13

Transition-cover fallback expiry now uses elapsed wall time, so a resume position
past three seconds does not uncover the native surface before capture. The cover
also waits briefly when a coalesced playback update has not yet reported the
incoming track list; the capture timer can start from video-track metadata before
full video dimensions arrive. The two focused cover-readiness regressions pass.

`cargo fmt --all -- --check`, `cargo build --workspace --locked`, and
`git diff --check` pass. A 25-second Windows autoplay screen audit traversed six
generated clips with 572 samples, no off-screen frames, and normal process close.
The generated interlaced and low-resolution MPEG clips have dark center content;
the audit also saw one transient host/child-size mismatch. It cannot confirm the
user's files or rule out a flash shorter than the sampling interval. Evidence is
in the ignored directory
`tools/window-stability-audit/transition-late-metadata-20261013/`.

## Windows transition-cover release ordering · 2026-10-13

When a transition ends, the opaque retained image now remains assigned until the
cover rectangle is disabled. Clearing the image first could let Slint render its
black backing for a frame between property updates. `cargo fmt --all -- --check`,
`git diff --check`, and `cargo build --workspace --locked` pass. A 24-second GUI
autoplay capture traversed six clips in 555 screen samples, with zero off-screen
samples, zero host/child size mismatches, and normal shutdown. The captured
handoffs show incoming clip content; black center samples came from the generated
dark interlaced/MPEG fixtures. This does not reproduce the user's affected media,
and the screen capture cannot rule out shorter flashes. Evidence is in the ignored
directory `tools/window-stability-audit/cover-release-order-20261013/`.

## Preferred-language token editor · 2026-10-13

Audio and Subtitle preferences now render preferred languages as ordered removable
chips with an inline entry field. Enter accepts a token; comma-separated entry
pastes multiple values; duplicates are ignored without changing existing order;
removing a chip updates the saved comma-separated value through the existing
settings pipeline. Three focused token parsing/order/removal tests pass; the
workspace app target compiled, and strict Clippy, format, and diff checks pass.
A Windows GUI attempt could not operate the native Open File dialog through the
available window handle, so visual interaction and the matched macOS reference
remain unverified. The exact Riina process started for that attempt was closed
and no Riina process remains.

## Windows incoming black-frame retry · 2026-10-13

Reviewing the high-rate report above showed repeated black screen samples after
clip changes. The first incoming-capture guard rejected fully black images, but a
new 16-second autoplay audit still caught a mostly black capture with a narrow
bright stripe, which passed the initial visibility threshold. Incoming captures
are now checked for picture across both sides of the sampled frame; rejected
captures keep the outgoing cover and retry at a bounded interval while the
existing three-second fallback remains active. Every capture reply has a revision
so a later successful retry refreshes the Slint cover. A focused test now rejects
the narrow-stripe capture. All 14 focused transition tests and strict workspace
Clippy pass, `cargo fmt --all -- --check` and `git diff --check` pass, and
`cargo build --workspace --locked` succeeds. A second 16-second Windows replay
covered four generated clips at up to 100 screen samples per second. It recorded
no host/child size mismatch or off-screen sample and closed Riina normally. Fully
dark center samples on the interlaced clip fell from 41 to 21, but remain present;
this reduces a transition artifact without resolving all reported flashing. The
user's affected media remains unverified. An isolated software-decoding replay
and an experimental deinterlace-on replay did not consistently reduce these dark
samples across both interlaced codecs; production remains at IINA's deinterlace-off
default. Evidence is in the ignored directories
`tools/window-stability-audit/incoming-black-frame-retry2-20261013/`,
`tools/window-stability-audit/incoming-black-frame-software-decode-20261013/`,
and `tools/window-stability-audit/incoming-deinterlace-test-20261013/`.

## Playlist-row transition capture · 2026-10-13

`cargo fmt --all -- --check`, `cargo check --workspace --locked`, strict
workspace Clippy, and `git diff --check` passed. A Windows GUI pass opened the
playlist panel and double-clicked `05-return-video.avi`; playback history shows
it followed `03-portrait.avi` and preceded the following `04-audio.wav` item.
The exact Riina process
closed normally and a process check found no remaining Riina instance. This
confirms the direct playlist-selection route executes, but the screen capture did
not establish that the user's flash is fixed. An autoplay screen audit across
wide, standard, portrait, return-video, and audio fixtures recorded 666 samples,
zero all-dark samples, zero host/child size mismatches, zero off-screen frames,
and normal shutdown. Evidence is in the ignored directory
`tools/window-stability-audit/flash-variable-window-20261013/`; affected user
media and flashes shorter than the capture interval remain unverified.

## Windows incoming-frame stability · 2026-10-13

The cover now requires three usable incoming-frame captures spanning at least
400 ms before its existing 750 ms release grace can begin. `cargo fmt --all --
--check`, `cargo check --workspace --locked`, `cargo build --workspace --locked`,
and `git diff --check` passed. The built executable SHA-256 is
`217908464F431AD7D883B18FA95AA3D2BCE609B445FF312EECC3956DD91F974E`. A six-clip
Windows GUI replay closed normally (exit 0), with 572 geometry samples, no
off-screen samples, and no mpv-child size mismatches. `GetWindowState` showed the
synthetic video rendering in the test player, but the monitor's desktop-capture
images were occluded by Codex, so this run does not verify transition pixels or
the reported flash. No Riina process remains. The user's affected media still
needs a direct check.

## Playback History multi-row action alignment · 2026-10-13

Removed the duplicate bottom action bar, which offered single-row operations
despite outline multi-selection; Play, New Window, Reveal, Remove, and Trash now
use the selection-aware context-action path. The context selection plan includes
all selected visible rows in outline order and adds the clicked row, matching
IINA's pinned `selectedRowIndexes` plus `clickedRow` behavior. Two focused tests
cover row order, clicked-row inclusion, and filtered-out selections.
`cargo fmt --all -- --check`, `cargo check --workspace --locked`,
`cargo test --workspace --locked` (246 app tests passed; 49 native integration
tests ignored because the pinned runtime is not provisioned for this invocation),
strict workspace Clippy, and `git diff --check` pass. Live History window
interaction and matched macOS/Windows captures remain to be verified.

## Windows clip-switch flash follow-up · 2026-10-13

Replayed the six generated video clips with the rebuilt Windows app visible and
captured the handoffs on screen. Transition captures showed video content at the
tested clip changes; the 814-sample run recorded no off-screen frames. It did
record one transient host/child-size mismatch 2 seconds into the final clip,
after its transition, plus black-center samples within the deliberately dark
interlaced fixture. These generated clips still do not reproduce the user's
reported flash on their media, so the issue remains open. Riina exited normally
and no process remained. Evidence is in the ignored directory
`tools/window-stability-audit/user-flash-followup-20261013/`.

## Playback History Windows layout check · 2026-10-13

Opened File → Playback History in the rebuilt Windows app using the isolated six-
entry history fixture. The window displayed date grouping, folder/site grouping,
expand/collapse, search-mode and clear-history controls, plus the media/progress/
played-at columns. The duplicate bottom action strip is absent and the list uses
the freed vertical space. Context-menu selection actions and visual comparison
with a matched macOS capture remain open. Riina was closed normally after the
check and no process remained.

## Windows autoplay cover handoff · 2026-10-13

When a usable full-frame capture arrives for an incoming clip, the transition
cover now releases immediately to live native playback instead of continuing to
display the captured snapshot over the moving surface. Loading, unknown video
metadata, black frames, and incomplete captures still retain the outgoing
cover. All 14 focused transition tests pass; `cargo fmt --all -- --check`,
`cargo build --workspace --locked`, and `git diff --check` pass. A foreground
24-second Windows replay crossed all six generated clips, recorded 807 samples
with zero off-screen samples, and Riina exited normally. The monitor also saw
three hidden samples and one late host/child size mismatch. The transition
captures still do not reproduce the user's media, so this change does not
confirm that the reported flash is fixed. Evidence is in the ignored directory
`tools/window-stability-audit/incoming-cover-release-20261013/`.

## Windows clip-switch stability release gate · 2026-10-13

The player previously hid the transition cover on the first usable incoming-frame
capture, before the existing three-frame/400 ms surface-stability check had
completed. It now keeps the opaque handoff image until that stability check
confirms the incoming surface, while preserving the existing three-second
fallback if capture cannot stabilize. Two focused regression tests cover the
release gate and timeout. All 16 transition-frame tests pass; formatting, strict
Riina Clippy, and the Riina build pass.

A 24-second Windows GUI replay of the final rebuilt executable used the mixed-
size wide, standard, portrait, return-video, and audio fixtures. It produced 657
window samples with zero off-screen frames and zero host/mpv size mismatches. Of
511 video samples, the sole near-black sample was during initial startup; none
occurred at clip changes. Captured handoff screenshots show incoming video
content. Riina closed normally and no Riina process remains. The user's affected
files are still unavailable, so this improves the specific premature-release
race but does not yet verify the reported flash on those files. Evidence is in
the ignored directory
`tools/window-stability-audit/flash-stability-final-20261013/`; executable SHA-
256: `5E7FDF22B15EB888CCF669C1DA5088F1FF801A898365A8F0A83747AEB878E5F7`.

## A/B timeline marker drag behavior · 2026-10-13

The shared playback slider now records where the pointer grabbed each A/B marker
and moves that marker only after an actual drag, preserving its position when the
mouse first goes down. The existing hit-target ordering keeps the playhead ahead
of loop markers and gives B priority where A and B overlap. The pinned reference
behavior was inspected in `PlaySliderLoopKnob.swift`; Riina's implementation is
independent. `cargo check -p riina --locked`, formatting and diff checks pass, and
the focused `dragging_an_ab_loop_point_updates_only_the_selected_endpoint` core
test passes. Native pointer interaction and matched captures remain unverified.

## Windows outgoing-frame capture retry · 2026-10-13

The clip-switch timer previously stopped requesting an outgoing frame as soon as
mpv accepted the asynchronous command, even when its capture reply later failed.
Riina now retries every 240 ms until a frame for that playback generation is
confirmed. The focused `outgoing_capture_retries_until_a_reply_confirms_the_frame`
test, `cargo fmt --all -- --check`, `cargo build -p riina --locked`, and strict
`cargo clippy -p riina --all-targets --locked -- -D warnings` pass.

A 30-second Windows mixed-codec autoplay replay produced 1,260 screen samples at
a 23.5 ms average and 214 ms maximum sampling gap, with zero host/child-size
mismatches, zero off-screen samples, and normal exit. A follow-up captured full
windows at the black-center samples: those images showed the interlaced/MPEG test
pattern itself, while the handoff screenshots showed continuous picture output.
No actual blank was sampled at the transitions in this generated playlist. The
user's reported files remain unverified. Evidence is in the ignored directories
`tools/window-stability-audit/retry-capture-20261013/` and
`tools/window-stability-audit/retry-capture-confirm-20261013/`; executable
SHA-256: `1D1CCEB29FEA4337A539FB26952660C84F713266CC32FB676426DBE5A6C777D8`.

## Player-scoped plugin event API · 2026-10-13

Added bounded `iina.event.on/off` listeners and worker-thread delivery for
`iina.file-loaded` and changes to playback properties already present in the
Riina plugin snapshot. The focused runtime/context tests pass (41 plugin tests),
as do strict Riina Clippy and `cargo build -p riina --locked`. This is partial
IINA event parity: arbitrary `mpv.observe(property:)`, the wider event catalogue,
global event access, and macOS runtime behavior remain unimplemented or
unverified.

## Windows incoming native-surface release gate · 2026-10-13

Moved the visible-child/host-size stability check ahead of transition-cover
release. This prevents stable decoder captures from releasing the cover while
the native output is hidden or reconfiguring. All 18
`transition_frame::tests` pass; formatting, diff checks, strict app Clippy, and
the player build pass.

The visible 25-second Windows synthetic-folder replay recorded 1,011 samples,
zero off-screen samples, zero host/mpv child-size mismatches, and zero sampled
near-black video frames after startup. Captured handoffs show continuous video;
Riina exited normally and no process remained. This does not reproduce the
user's affected files or prove sub-sample flashes are absent. Evidence is in
`tools/window-stability-audit/flash-surface-gate-followup-20261013/`.
Executable SHA-256: `DB748A09F7D386229D8792D77DEE2E504EB2B612CD0FA7BC588CC066D001041F`.

## Dynamic plugin MPV-property events · 2026-10-13

Player-scoped plugin listeners can now observe bounded, deduplicated dynamic
MPV properties. Runtime tests verify subscription requests and event dispatch;
`cargo test --workspace --locked` verifies the full workspace, and the targeted
ignored native integration test
`dynamic_mpv_property_observation_publishes_generic_node_values` passes against
the pinned Windows libmpv DLL using `container-fps`. Strict workspace Clippy and
the workspace build pass. This proves native property observation and portable
state update, but not the complete QuickJS-to-GUI path. The wider IINA event
catalogue, global events, and macOS runtime remain open.

## Windows autoplay cover lead · 2026-10-13

Extended the outgoing-frame cover to 450 ms before an autoplay handoff when a
successor exists. All 19 `transition_frame::tests` pass, `cargo fmt --all -- --check`
and `git diff --check` pass, and `cargo build -p riina --locked` passes.
The built player launched with the synthetic codec folder and advanced through
`05-mpeg2-bff.mkv`, confirming playlist playback. Two refreshed capture attempts
against its stable host window failed with
`IGraphicsCaptureItemInterop.CreateForMonitor` error `0x80070057`, so this run
does not verify transition pixels or the user's flashing. The test process was
closed normally and a process check found no Riina instance.

## Open URL stream protocol coverage · 2026-10-13

The protocol envelope follows mpv's `file|URL` input model and its documented
network backends ([mpv manual](https://mpv.io/manual/stable/),
[FFmpeg protocol reference](https://ffmpeg.org/ffmpeg-protocols.html)).
The shared media URL validator now accepts common mpv stream schemes beyond
HTTP and RTSP, including FTP, RTMP, SRT, UDP, SMB and mpv's `ytdl://` handler.
It preserves the supplied URL spelling and query escaping for libmpv, rejects
unknown schemes and local `file://` URLs in the network-only entry path, and
keeps Windows drive paths classified as files. The three focused
`media_reference::tests` pass as part of `cargo test --workspace --locked`.
Plugin media opens apply network permission and manifest-domain checks to all
recognized stream schemes, including RTSP and RTMP; `ytdl://` checks its nested
source host. Plugin grant tests cover allowed and denied hosts. Live playback of
each external protocol remains unverified; runtime availability depends on the
packaged engine's FFmpeg protocol support.

## Windows playlist phase-gap cover · 2026-10-13

The transition cover now remains enabled if a newer media generation briefly
reports `Ended` or `Stopped` before incoming metadata arrives. This handles a
late end notification crossing the UI update for a playlist handoff; once the
track list confirms an audio-only item, the cover can release immediately. The
two focused phase-gap regressions pass. `cargo test --workspace --locked`, strict
workspace Clippy, `cargo fmt --all -- --check`, `git diff --check`, and
`cargo build --workspace --locked` pass.

A 25-second mixed-codec GUI replay advanced through the playlist and exited
normally. It did not provide usable visual evidence: the desktop-copy samples
were uniformly black, and Windows Graphics Capture returned
`CreateForMonitor` error `0x80070057`. One late sample for `06-catalog.mkv` also
reported a window frame far outside the measured work area, so that geometry
observation needs a separate, foreground-verified reproduction. The user's
switching flash remains open; this change is not claimed as a visual fix.

## Windows clip-switch cover and compact transport follow-up · 2026-10-13

The transition overlay now continues to use the complete outgoing frame while
the incoming decoder capture and native child surface settle. The incoming
capture is only a readiness signal, avoiding display of a partial raw frame.
The compact transport keeps previous/next buttons hidden until at least 132
logical pixels are available for their hit targets and spacing. All 22 focused
transition tests pass, including the partial-incoming-frame regression. The full
workspace test run passes (257 app tests; 50 native integration tests ignored),
as do strict workspace Clippy, formatting, and the workspace build.

A fresh 25-second Windows autoplay replay traversed all six mixed-codec clips,
with zero off-screen samples, zero mpv-child size mismatches, and normal
shutdown. Its 258 stable pixel samples and 45 transition screenshots were
uniformly black, so they do not establish whether any individual handoff flashed.
Direct capture of the Riina window failed twice with Windows Graphics Capture
`CreateForMonitor` error `0x80070057`. The user-reported flash and the compact
controller's appearance therefore remain visually unverified. Riina was closed
and a process check found no instance. Evidence is in the ignored directory
`tools/window-stability-audit/outgoing-frame-hold-20261013/`.

## OpenSubtitles multi-selection · 2026-10-13

The result list supports modifier-click toggling, Shift range selection, Select
All, Return/double-click download, and sequential download of selected results.
The focused `ResultSelection` tests pass within the workspace run. The app build,
strict Clippy and full workspace tests pass. This session did not exercise the
live service/API key or visually verify the dialog; Assrt is now implemented as a
second native provider, and Shooter.cn is implemented as a third. Live service
availability and GUI results remain unverified; plugin-registered providers
remain open.

## Assrt subtitle provider integration · 2026-10-13

Assrt selection now routes manual and eligible automatic searches through a
bounded HTTPS client; selected results are downloaded and loaded using the
shared sequential batch path. Separate credentials stay in the platform
credential store and never enter preferences JSON. Seven fixture tests cover
search and detail response fields, empty result objects, token validation,
provider errors, HTTPS host confinement and playable file extension selection.
The Settings provider/token lookup and search attribution link are compiled and
the subtitle-provider Settings search is covered. The full workspace test suite
passes; 50 native integration tests remain ignored because this environment lacks
the pinned libmpv test provision. Live Assrt requests/playback and dialog capture
remain unverified because no user token or usable GUI capture surface was
available. The official API contract is at https://2.assrt.net/api/doc.
Automatic search submits a provider request when the result window opens, once
for each eligible media generation after the duration, subtitle-track and
sidecar-discovery gates pass.

## Shooter.cn subtitle provider · 2026-10-13

The provider catalog now includes Shooter.cn for manual and eligible automatic
searches. Its fixture coverage checks response filtering, supported subtitle
formats, HTTPS host confinement, four bounded 4 KiB sample hashes, small-file
rejection, and bounded metadata text. `cargo test --workspace --locked` passes
with 271 app tests; 50 native integration tests are ignored because this test
environment lacks the pinned libmpv provision. Strict workspace Clippy,
formatting, diff checks, and the Windows player build pass. The user-facing
disclosure states that Shooter receives the local path and sampled hash. Live
service availability and GUI playback/results remain unverified.

## Windows visible mpv surface alignment · 2026-10-13

The 16 ms native-surface alignment loop now resizes only visible mpv child
windows. mpv may retain hidden child HWNDs while replacing its swap chain; the
previous loop kept resizing those stale windows even though they could not be
seen. A replacement child is included as soon as Windows marks it visible. The
workspace test suite passes (264 app tests; 50 native integration tests ignored
because the pinned libmpv DLL is not provisioned here), as do strict workspace
Clippy, formatting, `git diff --check` and the full workspace build.
No usable live capture was available for this run, so the user's reported flash
is not yet confirmed fixed.

## Windows surface-replacement stability · 2026-10-13

The incoming handoff stability timer now fingerprints the aligned visible mpv
child surfaces in addition to host size and media generation. A new child HWND,
changed child dimensions, or relabeled surface restarts the settling interval.
The user reports that the whole-screen white flash was no longer visible in the
latest playback check (“seems ok”). Automated pixel capture was unusable, so this
is a user-observed improvement rather than a captured visual pass. The synthetic
test launcher shut down normally, and no Riina process remained afterward.

## Windows tiny-controller playback audit · 2026-10-13

The fresh 0.3.5 debug build played the moving-square fixture at a forced 180×72
outer size while advancing to the next playlist item. Across 307 samples there
were zero hidden-window, off-screen, and host/video-child mismatch samples. The
test process exited normally and a process check found no remaining Riina
instance. The small-window screenshot captured the video surface without the
controller, so it verifies placement and playback geometry, not button visibility
or interaction.

A second 420×270 autoplay replay recorded 288 samples, one `IsWindowVisible`
false sample, no off-screen samples or host/video-child mismatches, and normal
shutdown. Its capture also showed only the video surface, so controller visuals
and pointer behavior remain unverified.

## Assrt compressed subtitle archives · 2026-10-13

Assrt results can now return supported subtitle files from ZIP archives without
writing archive contents to disk. Downloads remain capped at 16 MiB; archives
are limited to 128 entries, and traversal paths, symbolic links, empty files,
unsupported formats and expanded entries above 16 MiB are ignored. Unit fixtures
cover valid extraction, traversal and symlink rejection, malformed archives and
archives without subtitles. `cargo test --workspace --locked` passed all 273 app
tests; 50 native integration tests were ignored because the pinned libmpv DLL is
not provisioned locally. Strict workspace Clippy with `-D warnings`, formatting
and `git diff --check` pass. Live Assrt downloads and the result-window GUI remain
unverified.

## Built-in online subtitle source menus · 2026-10-13

The Subtitles menu now shows the configured default provider in its primary
online-search action and lists OpenSubtitles, Assrt and Shooter in a source
submenu on both shared and Windows-native menu surfaces. Picking a source routes
that provider into the existing results picker without changing the saved
default. `cargo check --workspace --locked`, strict workspace Clippy, formatting,
and `cargo test --workspace --locked` pass (273 app tests; 50 native tests
ignored because the pinned libmpv DLL is not provisioned locally). Menu pixels,
live selection and the upstream direct-download behavior remain unverified;
plugin-registered providers are not included yet.

## v0.3.5 published package · 2026-10-13

Tag `v0.3.5` points to source commit `7ad5cec27c4de0a8addb93e0c3d53319971c3178`.
Release workflow [38472242460](https://github.com/deminden/riina/actions/runs/38472242460)
completed successfully: release readiness, native playback/lifecycle, package
verification, and publish passed; `native-engine` was skipped. The package reused
the successful native-engine artifact from run `38138622911`; the release record
identifies the packaged libmpv hash `1749e6b5519afa383c88442e97026754e4e169bd46107ecc0ab0b08d205ad0af`
and Cargo.lock hash `5ceee2866d9afb49677a120b7c88c667277604696bd6b0414c732922094ab1b5`.

The public Windows ZIP is 69.8 MB with SHA-256
`970a2e755449ee00f35acd93a3b2d2ac7f003f9d76672adc41253813743c13b9`, matching the
published `SHA256SUMS.txt`. Its `RELEASE-VERIFICATION.json` records 101,748 indexed
native source files across 88 repositories and the exact tagged source commit.
The [published v0.3.5 release](https://github.com/deminden/riina/releases/tag/v0.3.5)
includes the portable binary, checksums and verification record.

## Native menu first-paint state · 2026-10-13

The Windows native menu callback refreshes playback labels and Video, Audio,
Subtitles and Plugins panel toggle titles synchronously during `WM_INITMENU`.
`cargo check --workspace --locked`, `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets --locked -- -D warnings`, and
`git diff --check` pass. The complete workspace build passed shortly before the
panel-title addition; the post-addition workspace check compiles the changed
code. Live menu interaction, screen capture and matching macOS presentation have
not been verified.

### Windows active Video-panel title · 2026-10-13

On the rebuilt Windows x64 native-video executable
(`F2C5FC93E24EE55B6FA722D91190900E45618F53008A5D2F4698AD1704978D73`), opening
Video > Show Video Panel and reopening Video displayed “Hide Video Panel” with
`Ctrl+Shift+V` as the first command. The file's same-folder autoplay advanced
into an audio item during inspection; the open Video panel remained reflected in
the menu title. Normal close completed and no Riina process or window remained.
This verifies the live Windows first-paint state for the Video-panel title only.
Audio/Subtitles and Plugins toggles, dynamic playback labels, Windows visual
matching and all macOS menu behavior remain unverified. Structured observation:
`tests/visual/native-menu-first-paint-windows-evidence.json`.

## Font family default face · 2026-10-13

The installed-font catalog now orders an upright face nearest weight 400 before
other styles. Selecting a family uses that first face and retains the exact
PostScript name as the subtitle-font draft. The full workspace test run passes
277 app tests; 50 native integration tests are ignored because the pinned DLL
is not provisioned locally. Focused tests verify Regular preference, fallback
ordering and empty families. Pointer selection, rendered face-row order,
native-video and macOS runtime remain unverified.

## Music Mode hides main-window sidebars · 2026-10-13

IINA v1.5.0 hides all main-window sidebars before switching to its compact audio
surface. Riina's Music Mode transition now clears both the leading and trailing
sidebar selections before notifying the platform adapter to resize. The complete
workspace tests pass (277 app tests; 50 native playback tests ignored because the
pinned DLL is not available). `cargo build --workspace --locked` passes with this
change. `cargo fmt --all -- --check` and strict workspace Clippy pass. Live
transition verification remains pending because no local mpv DLL is provisioned;
macOS runtime remains unverified.

## Plugin sidebar manifest discovery · 2026-10-14

`Manifest::parse_with_locale` now retains a non-empty `sidebarTab.name` after
trimming, selects exact/language/English localized labels, rejects labels longer
than 128 bytes or containing control characters, and ignores absent, blank, or
non-string labels. Focused tests cover the label bounds and locale fallbacks.
The package installer, plugin inventory, and runtime loader pass the system
locale into the parser. The player QuickJS session now exposes
`iina.sidebar.loadFile`, `show`, `hide`, `postMessage`, and `onMessage`, emits
bounded typed effects, and delivers registered callbacks on the plugin worker.
The Windows player now populates a plugin selector from enabled localized
manifests and hosts each selected plugin document in a child WebView. A confined
custom scheme serves package files; WebView JSON IPC routes messages back to the
QuickJS worker. The host sends `iina.window-loaded` after the UI event loop starts
and replays that lifecycle transition to player plugins enabled or reloaded later.
A focused host regression test passes for late activation and duplicate
notification handling. The shared confined child-WebView presenter compiles and
its path/serving tests pass on Windows, and a live Windows debug-build smoke test
showed the enabled plugin label and package page. The presenter is now wired on
Linux X11, where GTK is initialized on the UI thread and pumped from the player
timer as Wry requires; Wayland embedding remains open. A Windows-host Linux
cross-check stops in `yeslogic-fontconfig-sys` because no Linux pkg-config sysroot
is configured, before it reaches Riina's code. An Apple-target build, Mac runtime,
native Linux build/runtime and matched IINA panel geometry remain pending. The
workspace suite passed; 50 native libmpv tests are ignored unless the native test
engine is provisioned.

### Plugin sidebar placement · 2026-10-14

The Quick Settings placement preference now controls opening the plugin panel
from the toolbar/menu and when selecting a plugin tab. Sidebar-resize saves retain
the preference. `cargo test -p riina
sidebar_preferences::tests::plugin_panel_opens_on_the_saved_side --locked` passes
for both leading and trailing placement. An Apple Silicon target check was
attempted but stops in the `ring` build script before Riina compilation because
this Windows environment has no C compiler (`cc`). No live panel capture was
available in this session. `cargo test --workspace --locked` passes with 297 app
tests; 51 native libmpv tests remain ignored because the pinned test engine is
not provisioned. Strict workspace Clippy, formatting, Windows workspace build,
and `git diff --check` pass. Sidebar-side segmented controls expose an accessible
label and map left/right arrows to leading/trailing; this was source-compiled but
not exercised through a live UI.

### Windows Quick Settings Video pane · 2026-10-14

On a fresh isolated profile, opening the toolbar gear showed the Video pane
selected by default, with the current video track selector above aspect ratio,
crop, rotation, speed, hardware-decoding, and picture controls. The four tabs
were Layout, Video, Audio, Subtitles in that order. This is a live interaction
check only; no track change or macOS-matched visual comparison was performed.
The `v` subtitle-visibility GUI check was inconclusive because the window changed
size during automation and its coordinates became stale.

## Plugin file-started lifecycle event · 2026-10-14

The pinned IINA event inventory includes `iina.file-started` before `iina.file-loaded`. Riina now derives the start notification from the media-generation transition and dispatches it to subscribed player-plugin sessions before the loaded notification. Focused `player_` tests pass (24), and `cargo test --workspace --locked` passes (282 app tests; 50 provisioned-libmpv tests ignored). This verifies portable event sequencing and QuickJS listener delivery; live media/plugin UI behavior and the remaining IINA event catalogue are still open.
## Plugin window lifecycle events · 2026-10-14

The plugin host now diffs observed player-window properties and sends IINA-compatible player-scoped events for movement, resize, fullscreen changes, Picture in Picture changes, minimization/restoration, and screen changes. Initial geometry is treated as a baseline so opening the player does not produce false movement events. Geometry and mode-diff tests and a QuickJS listener-delivery test pass. Live window/plugin interaction, exact event cadence, and macOS runtime verification remain open.
## Plugin event bridge verification · 2026-10-14

The portable plugin event bridge now dispatches `iina.file-started` on a media-generation change and IINA player-window movement, resize, fullscreen, PiP, minimize/restore and screen-change events from window-property diffs. Focused runtime/context tests passed. `cargo test --workspace --locked` passed with 283 app tests and 50 provisioned-native tests ignored; strict workspace Clippy, workspace build, formatting check, `git diff --check`, and the parity inventory checker passed. Initial window properties establish a baseline. Live plugin events during media/window changes and macOS runtime remain unverified.
## Plugin pre-close lifecycle notification · 2026-10-14

The late-enabled plugin lifecycle test now subscribes to `iina.window-will-close`, closes the host through its shutdown path, and confirms the callback completes and its message reaches the host before the session is destroyed. Focused test passes. Actual window-close interaction and macOS runtime remain open; current workspace gates pass.
## Plugin file/window shutdown event verification · 2026-10-14

Plugins now receive file-started, file-loaded, supported playback-property changes, window movement/resize/fullscreen/PiP/music-mode/minimize/restore/screen changes, and a bounded `iina.window-will-close` callback before teardown. The Windows and shared-player close paths use a one-shot notification; the host flushes callbacks and collects their messages before destroying plugin sessions. The lifecycle and window-diff tests pass. `cargo test --workspace --locked` passes (283 app tests; 50 native tests are ignored by default), as do strict workspace Clippy, `cargo build --workspace --locked`, formatting, `git diff --check`, and the parity traceability checker. Actual live window transitions, close interaction, and macOS/Linux runtime verification remain open.
## Plugin engine-initialized replay · 2026-10-14

The pinned IINA source emits `iina.mpv-initialized` after `mpvInit()` and replays it to plugins reloaded for an existing player. Riina now detects the transition from an unknown engine version to the observed libmpv version and replays the event to late-enabled player plugins. The lifecycle integration test passes, and `cargo test --workspace --locked` passes (283 app tests; 50 native tests ignored by default); strict workspace Clippy, `cargo build --workspace --locked`, formatting, diff, and parity checks also pass. This verifies portable event sequencing and late activation. Full GUI playback integration and macOS/Linux runtime verification remain open.
## Plugin main-window focus event · 2026-10-14

The source emits `iina.window-main.changed` with a Boolean payload on main-window gain and loss. Riina maps this to the focused Winit player window and emits on observed state changes. The window-state test and player-event delivery test pass. The full workspace suite passes (283 app tests; 50 native tests ignored by default), as do strict Clippy, workspace build, formatting, diff and parity traceability. Live focus transitions and sub-snapshot timing are unverified.
## Plugin automatic window-size event · 2026-10-14

`iina.window-size-adjusted` increments when media geometry enters the configured sizing policy, even when the current window frame is retained; the plugin host reports the current frame as event data. The context regression checks both a changed frame and a same-frame policy event. `cargo test --workspace --locked` passes (283 app tests; 50 native tests ignored by default), as does `cargo build --workspace --locked`. Strict Clippy, workspace build, formatting, diff, and parity checks pass.

## Plugin overlay-loaded event · 2026-10-14

A completed plugin sidebar document queues `iina.plugin-overlay-loaded`; the host broadcasts it to subscribed player sessions. The late-enabled plugin lifecycle test passes with this delivery. Windows and macOS WebView load callbacks and GUI behavior remain pending verification.

### Plugin event and engine gate results · 2026-10-14

Final `cargo test --workspace --locked`, `cargo build --workspace --locked`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo fmt --all -- --check`, `git diff --check`, and `scripts/check-parity.py` pass. Workspace tests report 283 app tests and 50 ignored native tests. The separately enabled native smoke test `dynamic_mpv_property_observation_publishes_generic_node_values` passes with the bundled development DLL, confirming libmpv loads, plays the moving-square fixture, and reports an observed property. This smoke test does not verify an installed package.

## Plugin thumbnails-ready event · 2026-10-14

The completion predicate requires a non-empty sample set with every sample present and is covered by a focused unit test. The controller reports successful completion to the plugin host; host lifecycle coverage subscribes and checks delivery. `cargo test --workspace --locked` (284 app tests; 50 native tests ignored by default), `cargo build --workspace --locked`, strict Clippy, formatting, diff, and parity checks pass. GUI playback and plugin WebView delivery remain unverified.

## Plugin manager menu command · 2026-10-14

“Manage Plugins…” now opens Settings > Plugins from the shared player menu and
Windows native-video menu. Regular Settings opening retains its current pane.
“Reload All Plugins” tears down active plugin sessions and their sidebar/overlay
content, reactivates enabled packages, and replays window/engine initialization
events. The focused reload integration check passes. The full workspace suite passes
(285 app tests; 50 libmpv native tests ignored). Formatting, diff, compile, and
parity checks and strict workspace Clippy pass. Menu interaction and matched visual
verification remain pending.
## Cross-target checks from Windows · 2026-10-14

`cargo check --workspace --locked` passes for the current Windows working tree.
Cross-checking `aarch64-apple-darwin` stops in dependency build scripts because
this Windows host has no C/Objective-C compiler (`cc`); setting `DOCS_RS=1`
skips the Objective-C helper but then `ring` also requires the unavailable C
compiler. The Linux target check stops in `yeslogic-fontconfig-sys` because no
Linux sysroot or cross-configured `pkg-config` is available. These are host
toolchain/sysroot limits; they provide no macOS or Linux application-build,
playback, rendering, or lifecycle evidence. Repeat those checks on matching
native build hosts.

## Player-plugin menu actions · 2026-10-14

The runtime exposes the IINA-shaped `iina.menu` item/separator, add/list/remove,
submenu, mutable state, shortcut and refresh operations. Bounded recursive menu
snapshots reach shared and Windows-native Plugin menus, and activation returns to
the owning player session's QuickJS callback. Tests cover item state and
shortcuts, removal and re-addition, tree bounds, and host-to-plugin activation.
Live menu pixels and interaction remain unverified. Nested entries currently
render as flattened breadcrumb rows. Dynamic separators use the Slint menu
separator sentinel. Each plugin now has one display-name heading, with global
actions before player actions; exact source ordering and Developer Tool entries
remain open. Global and player plugin sessions contribute menu items with scoped
callback routing.

## Global and player plugin menu contributions · 2026-10-14

Global plugin entry scripts now receive the bounded `iina.menu` API, publish
menu snapshots separately from player sessions, and route actions back to the
correct session even when both scopes reuse the same item identifier. The host
integration test exercises both entry scopes and passes. The full workspace
suite passes (289 app tests; 50 native libmpv tests remain ignored by default).
The workspace build, strict Clippy, formatting, diff, and parity traceability checks
also pass after the menu-row grouping change. Nested submenu hierarchy, first-paint
behavior, native separator appearance, and GUI interaction remain unverified. This
verifies scoped routing within one player host; sharing one global runtime across
multiple player windows remains open.

Although the 50 native integration tests are ignored by default in the workspace
suite, the development libmpv is provisioned at `tools/mpv/libmpv-2.dll`. The
explicit native smoke test `dynamic_mpv_property_observation_publishes_generic_node_values`
passes against that DLL and plays the moving-square fixture. Thus “unavailable”
describes the default test selection, not the current development runtime.

## Windows plugin-menu open event · 2026-10-14

Opening the native Windows Plugin submenu is identified from its top-level menu
handle and dispatches `iina.menu-update` to subscribed player-plugin sessions.
The plugin-host integration check confirms the QuickJS listener runs; the complete
workspace test suite passes (292 application tests, with 50 provisioned-libmpv
tests ignored). Formatting, diff validation, the 252-row parity CSV check,
`cargo build --workspace --locked`, and strict workspace Clippy pass. GUI menu
opening, first-paint updates after asynchronous plugin callbacks, shared Slint
menus, and macOS runtime remain unverified.

## IINA plugin ordering · 2026-10-14

The pinned implementation persists plugin identifiers in `PluginOrder` and uses
that sequence for the manager and active player menu. Riina now supports row
drag-and-drop in the Plugins settings list, stores the normalized sequence in
`plugin-order.json`, applies it during plugin host startup/reload, and immediately
reorders the live player menu. New installations append in the same deterministic
name order used by the manager when no saved position exists; removed identifiers
are ignored when normalizing. Core order normalization/movement tests, host package
ordering, and both persisted-order and default-name-order menu tests pass. The full
workspace suite passes (292 app tests; 50 native libmpv tests remain ignored by
default). The workspace build, strict Clippy, formatting, diff, and parity checks
pass. Order changes now broadcast to registered live player windows; weak entries
are pruned during the broadcast. Live drag interaction and matched IINA menu
captures remain unverified.

## Plugin submenu action semantics and engine label · 2026-10-14

Submenu container rows in the breadcrumb fallback are disabled, while nested leaf
rows preserve their plugin/scope callback tokens. Replacing one plugin scope keeps
the other scope's submenu rows. The About window labels pending engine version
metadata as “Checking…” rather than wording that can be mistaken for a missing
libmpv engine. When About opens before those properties arrive, a visibility-gated
timer refreshes the labels. The version-label unit tests cover pending and reported
states. The full workspace suite passes
(295 app tests; 50 native libmpv tests ignored by default), as do the workspace
build, strict Clippy, formatting, diff validation, and parity traceability checks.
Native GUI submenu hierarchy remains unverified.

## Native engine version reporting · 2026-10-14

The targeted native playback smoke test passed against the provisioned
`tools/mpv/libmpv-2.dll`. It first waits for non-empty mpv and FFmpeg version
properties while playback is stopped and before any media has been loaded; it
then plays the moving-square fixture and checks dynamic property observation.
This confirms the bundled engine reports its version before media opens, so an
early About placeholder is a brief asynchronous update rather than an unavailable
engine. It does not verify the packaged release or the About window's live GUI
refresh.

The About window's pending labels now say “Checking…” while the stopped player
awaits its first version snapshot, and “Reading…” after playback begins. This
describes the short metadata read without implying that libmpv is absent. Three focused
About-window mapping tests and `cargo fmt --all -- --check` pass; visible GUI text
and packaged-engine failure reporting remain unverified.

## Idle libmpv version check · 2026-10-14

After a report that mpv appeared unavailable, the existing native integration test
`dynamic_mpv_property_observation_publishes_generic_node_values` was run directly
against the bundled Windows DLL. It passed and observed non-empty mpv and FFmpeg
version properties while playback remained stopped and before media was loaded.
The player window starts the engine before media opens, so the About window's
pending label represents an asynchronous property update, not an intentionally
deferred engine start. No live About-window capture was available in this check.

## Video quick-settings overflow controls · 2026-10-14

Riina's Video pane aspect-ratio and crop strips now expose a directional chevron
when their choices overflow, matching IINA's trailing indicator pattern. Clicking
the arrow moves to the opposite end and changes its direction; each strip reserves
the indicator's eight-point width and four-point gap. `cargo check --workspace
--locked`, `cargo build --workspace --locked`, formatting, diff validation, the
252-row parity traceability check, and strict all-target workspace Clippy pass. The
full workspace tests pass (295 application tests; 50 native libmpv integration tests
remain ignored by default). The rebuilt app was launched for a live check and then
closed; Windows.Graphics.Capture failed with
`IGraphicsCaptureItemInterop.CreateForMonitor failed (0x80070057)`, and Riina
windows/processes were confirmed closed. Arrow appearance and interaction therefore
remain GUI-unverified.

## Quick Settings track selector row parity · 2026-10-14

The pinned IINA track table uses a plain table, a narrow selected-row accent, and
horizontal grid lines; labels and language badges use semantic appearance colors.
Riina's Audio, Video, and Subtitle rows now omit the full-row selection fill,
retain the accent marker, draw a subtle horizontal separator, and adapt track,
None, and language-badge colors to the active appearance. `cargo check --workspace
--locked`, `cargo build --workspace --locked`, `cargo test --workspace --locked`
(295 application tests; 50 native libmpv integration tests ignored by default),
formatting, diff validation, and parity traceability pass. Strict all-target
workspace Clippy also passes. Matched table pixels and live track scrolling still
need GUI verification; the latest Windows capture attempt failed with
`IGraphicsCaptureItemInterop.CreateForMonitor failed (0x80070057)`.

## Floating OSC drag position persistence · 2026-10-14

The floating OSC now uses IINA's normalized horizontal-center and bottom-gap
preferences (0.5 and 0.1). Drag coordinates clamp inside the video bounds, center
snap uses a five-point threshold, and release routes through the established
preference writer. Under 460 points the controller recenters, retaining the saved
position for wider windows. Legacy preferences receive the IINA defaults; values
outside the normalized range are rejected. The added preference round-trip and
validation assertions pass. `cargo check --workspace --locked` and the full
`cargo test --workspace --locked` pass (295 application tests; 50 provisioned
libmpv integration tests remain ignored by default). Windows pointer routing,
drag persistence after resize, app relaunch restoration, and matched IINA visual
comparison are not yet verified. The workspace build and strict all-target Clippy
also pass. A GUI check was attempted, but Windows.Graphics.Capture failed twice
with `IGraphicsCaptureItemInterop.CreateForMonitor failed (0x80070057)`, so no
drag or visual result is claimed. The exact test-launched Riina process exited
normally after a close request.

## Control scroll input parity · 2026-10-14

The Control pane retains IINA's four-step Relative Seek Amount and the saved
`seek_sensitivity` key remains readable as a compatibility alias. Shared scroll
effects now receive delta precision and mouse-vs-trackpad class independently.
This preserves IINA's mouse seek map for phase-empty events even when AppKit marks
their deltas precise, while trackpad seek, volume, and speed use the configured
sensitivity curves. Unit checks cover both seek curves and mouse/trackpad volume
behavior; the settings round-trip covers the legacy and current field names. The
full workspace test suite passes (295 application tests; 50 provisioned libmpv
integration tests ignored by default). Strict all-target Clippy and the full
workspace build also pass.
Live wheel input, Auto seek timing, AppKit target build, and matched Control-pane
captures remain unverified.

## Generic mpv plugin events · 2026-10-14

IINA's `MPVController` emits each native mpv event as `mpv.<event-name>` to
player-scoped listeners, separately from observed property notifications. Riina
copies event names from `mpv_event_name` while the player worker owns the native
event and retains the latest 256 in sequence order. Plugin-context diffs deliver
the new names with null payloads once, even if playback snapshots coalesce. The
QuickJS host regression test verifies `mpv.start-file` reaches a subscribed
plugin once; a libmpv integration test with the bundled Windows DLL verifies
native `start-file` and `file-loaded` event names. The full workspace suite passes
(296 app tests; 51 native integration tests are ignored by default), the targeted
native event test passes, and the workspace build, formatting, diff, and parity
traceability checks pass. Strict all-target workspace Clippy passes. Live plugin UI behavior
and macOS/Linux runtime remain unverified.

## macOS plugin-menu event source check · 2026-10-14

Added a scoped AppKit `NSMenuDidBeginTrackingNotification` observer for the
active player's `Plugin` menu. It compares `NSApp.keyWindow` with the player's
AppKit window before notifying its plugin host, and removes its observer token
when the UI session drops. The exact observer source type-checks for
`aarch64-apple-darwin` with a generated Slint player component, and strict Apple-
target Clippy passes in that isolated check. AppKit's notification fires at
tracking start, later than IINA's `menuWillOpen` callback, so first-open menu
refresh timing is a known question. The full app target check stopped in `ring`
because this Windows host has no C compiler for the Apple target, and live menu
interaction still requires macOS hardware. Windows `WM_INITMENUPOPUP` remains
the separately implemented native path. The Windows workspace check, strict
Clippy, and full workspace tests passed after this change (296 app tests; 51
native tests ignored by default).

## Video-backed translucent player surfaces · 2026-10-14

Shared-OpenGL playback now creates a quarter-resolution Gaussian-sampled blur
texture while the player context is current and exposes it beneath the enabled
Liquid Glass sidebar, title row, floating controller, seek/video/plugin OSDs, and
fullscreen information card. Blur rendering runs only while an enabled material
surface is visible. The render pass restores draw/read framebuffer bindings,
texture/program/VAO/buffer bindings, viewport, and relevant GL enable state;
renderer teardown destroys the GL objects before the context is released.
`cargo fmt --all -- --check`, `cargo check --workspace --locked`,
`cargo clippy --workspace --all-targets --locked -- -D warnings`,
`cargo test --workspace --locked` (297 application tests passed; 51 native tests
ignored by default), and `git diff --check` pass. A live desktop capture is
unavailable because the computer-control surface reports no available apps.
Runtime GPU behavior and material pixels remain unverified, and native Windows
D3D playback does not use this GL pass.

## Help menu links · 2026-10-14

The focused `cargo test -p riina --locked help_menu_tests` passed. It checks the four fixed project destinations and rejects unknown actions. `cargo fmt --all -- --check`, `git diff --check`, and the 17-column parity CSV validation passed. The test build compiled both the shared player menu and the Windows-native menu callback. No GUI interaction or matched screenshot was captured, so presentation and actual browser launch remain unverified. The 0.3.6 tag predates this local parity follow-up.

## 0.3.6 release workflow and headless sidebar test · 2026-10-14

GitHub release run [38550745763](https://github.com/deminden/riina/actions/runs/38550745763) reused the previously verified Windows native engine and its native playback/lifecycle step passed. The later Windows lint-and-unit-test step failed one unrelated UI test because it constructed a Slint player window on a runner without an OpenGL 2.0 renderer (298 passed, 1 failed, 2 ignored). The plugin-sidebar test now checks the extracted pure placement decision instead of creating a window. The local full workspace suite passes with 302 application tests; 51 native tests are ignored by default and are exercised separately in the release workflow.

## 0.3.7 corresponding-source size and package scope · 2026-10-14

The release candidate Git tree contains 455 tracked files totaling approximately 17 MB before compression. The large local IINA reference checkout is not tracked in that tree. The release workflow attaches the Windows ZIP, checksum file, and package-verification record; it does not attach a duplicate source tarball or native/vendored source trees. The exact application source is available from the immutable v0.3.7 tag, with native source revisions and rebuild steps identified inside the binary package.

## 0.3.7 local verification · 2026-10-14

For the v0.3.7 application source, `cargo test --workspace --locked` passed with 302 application tests and 51 provisioned-native tests ignored by the local default run. `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo build --workspace --locked`, `cargo fmt --all -- --check`, and `git diff --check` passed. The test that failed on the prior headless runner now uses the pure sidebar-placement decision; hosted native playback and final package checks are tracked by release run 38552722145.

## 0.3.8 Help diagnostics and About metadata wording · 2026-10-14

The source-defined Dump Debug Info action is wired into the shared and Windows-
native Help menus. Its Downloads report includes playback and selected media
metadata, limits each collection to 512 items and each text field to 1,024
characters, and omits captured pixel buffers. The success dialog warns that
filenames and URLs may be private. A focused test verifies output, privacy-bearing
paths, field clipping, and collection bounds. Stopped-state About labels now say
“Checking…” because the bundled engine initializes before media and reports its
version asynchronously. The full workspace suite passes (303 app tests; 51 native
tests ignored by default), as do strict all-target Clippy, `cargo build --workspace
--locked`, formatting, diff, and parity checks. GUI menu interaction and the
source-defined Help row's absence from the reviewed capture remain unverified.

## Native Open Recent integration

The Open Recent menu's persisted app entries follow the record-recent preference.
File-picker selections are noted immediately; successful media loads are added only
when Track all played files is enabled; opening an existing menu entry moves it to
the front. Clearing the menu also clears the platform's app-specific recent list.
On Windows, Riina assigns `org.riina.player`, submits existing local media paths
through [`SHAddToRecentDocs`](https://learn.microsoft.com/en-us/windows/win32/api/shlobj_core/nf-shlobj_core-shaddtorecentdocs),
and clears the app's Recent/Frequent Jump List categories through
[`IApplicationDestinations::RemoveAllDestinations`](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-iapplicationdestinations-removealldestinations).
The Shell API preserves user-pinned destinations. On macOS, entries and Clear use
[`NSDocumentController`](https://developer.apple.com/documentation/appkit/nsdocumentcontroller).
A Windows unit test checks local-file filtering and canonical paths; native Shell
interaction, macOS runtime and Open Recent activation through AppKit remain
unverified. The focused test, full workspace suite (305 app tests), strict workspace
Clippy, workspace build, formatting, and parity traceability check pass. Linux desktop
recent-file integration remains open.
The attempted Apple-target app check stopped while building the `ring` dependency:
this Windows host has no target C compiler (`cc`), so Riina's macOS source was not
type-checked by that command.

## Video track row activation and current binary · 2026-10-14

The Quick Settings Video pane's shared track-row handler now selects video tracks
on a single click; it previously selected them on double-click, unlike the Audio
and Subtitle selectors. `cargo check --workspace --locked`,
`cargo test --workspace --locked` (311 application tests; 51 native tests ignored
by default), strict all-target Clippy, formatting, diff validation, and the parity
traceability check pass. Multi-track click behavior still needs live verification.
The test-launched 0.3.8 window reported `moving-square.avi — riina`, the `mpv`
child surface, and a playback clock; Windows.Graphics.Capture then failed with
`IGraphicsCaptureItemInterop.CreateForMonitor failed (0x80070057)`. The exact test
instance was closed and confirmed absent, so no visible playback or Quick Settings
pixels are claimed.

GitHub's current published release is v0.3.7 with its Windows x64 ZIP,
`SHA256SUMS.txt`, and `RELEASE-VERIFICATION.json`; the Windows package and
verification jobs succeeded while the native-engine build was skipped for reuse.
The v0.3.6 GitHub release was created after v0.3.7 by a historical source-tag
backfill. At the time, the release workflow always passed `--latest`, so that
backfill incorrectly promoted an older release; commit `26060a6` fixes the
workflow to preserve the newer latest release. Keep v0.3.7 latest and do not
rerun the v0.3.6 backfill.

## Secondary subtitle selector appearance · 2026-10-14

The secondary track list now shares the primary list's selected-row accent,
separator, and appearance-aware text and language-badge colors, matching the two
lanes in the pinned Subtitles-pane capture. `cargo test --workspace --locked`
passed (311 application tests; 51 native tests ignored by default), strict
all-target Clippy, formatting, diff validation, and parity traceability passed.
The current Windows capture bridge still fails on the playback window, so matched
live pixels and interaction are not verified.

## Audio Quick Settings card grouping · 2026-10-14

The Audio pane now uses separate rounded surfaces for the track selector, external
audio loading, delay controls, and ten-band equalizer, based on the pinned IINA
capture `23-playback-audio-panel.png`. The existing audio-track viewport and delay
range are unchanged; the opt-in volume boost remains a separate Riina row.
`cargo check --workspace --locked`, the full workspace tests (311 application
tests; 51 native tests ignored by default), strict all-target Clippy, formatting,
diff validation, and parity traceability pass. The playback-window capture bridge
failed, so card geometry/materials and live audio selection remain unverified.

## Audio equalizer scale labels · 2026-10-14

The Audio equalizer shows `+12 dB`, `0 dB`, and `-12 dB` labels at the right of
the ten faders, aligned with the slider range endpoints and midpoint in the pinned
IINA Audio-pane capture. The full workspace suite passed (311 application tests;
51 native tests ignored by default) with the labels present; after their final
position adjustment, `cargo check --workspace --locked`, strict all-target Clippy,
formatting, diff validation, and parity traceability passed. Live spacing and
rendered pixel matching remain unverified because the playback-window capture
bridge is unavailable.

## Audio equalizer profile menu · 2026-10-14

The permanent profile-name field and adjacent Save/Rename/Remove buttons were
replaced with a profile menu ordered like IINA v1.5.0's `SidebarAudioPane`:
Save, Rename, Remove, separator, Manual, separator, built-in presets and user
profiles. Save is available for the custom curve; Rename and Remove are available
only for a saved user profile. The menu separates Manual, built-in presets, and
saved profiles. Save and Rename open a name prompt, and the menu scrolls when its
profiles exceed the visible area. Invalid names leave the prompt open for
correction; Rename rejects the current name as a duplicate, matching IINA's
prompt validator. Profile names now preserve the exact prompt text, including
leading and trailing spaces, as IINA does. Rename begins with an empty field;
the input validator updates an inline error and disables Save until the name is
valid. The validator also checks names against the visible preset and saved-profile
labels, following IINA's duplicate-name list rather than only internal profile
identifiers. Preset, Manual, and user-profile selection now use distinct typed
identities. Legacy saved-profile settings without the new kind flag still infer
their selection from their stored name when that name cannot refer to a preset.
The focused audio-equalizer core tests and complete Windows workspace test suite
pass (51 provisioned-native tests remain ignored in this run). Strict all-target
Clippy, formatting, and the 255-requirement parity CSV traceability check pass.
An initial Windows GUI pass verified panel/menu rendering and
exposed a prompt dismissal bug; the prompt now opens after its menu closes, but a
fresh interaction pass could not reacquire the test window because the Windows
automation bridge stopped reporting its process id. The test instance was closed.
The macOS target check still stops in `ring` before application sources because
this Windows host lacks an Apple-target C compiler. Native macOS menu/prompt
behavior, matched pixels, and GUI interaction remain unverified.

## macOS application menu composition · 2026-10-14

The pinned IINA menu has one application menu containing About, Update, Preferences,
Services, Hide, Hide Others, Show All, and Quit. Slint 1.18.1's Winit/Muda backend
adds its standard macOS application menu before the Slint menu tree. A macOS AppKit
normalizer now moves Riina's About, update, Preferences, and quit entries into that
native menu, retains AppKit Services and visibility actions, removes the duplicate
`riina` top-level menu, and labels the remaining app menu `Riina`. The generated
Slint action items stay intact so their menu identifiers continue to dispatch.
On macOS, the Preferences item is presented as `Settings…` with AppKit's named
general-preferences image, matching the captured macOS 27 menu. `Check for Updates…`
now opens the current Riina release page; a native version comparison/updater is
still absent.
Normalization runs after the player is shown and when a native menu begins tracking,
so a Slint menu rebuild is repaired before the next menu interaction. The user
reference at `reference/new/Screenshot 2026-10-12 at 19.17.33.png` confirms the
single IINA application menu on macOS 27. The Windows workspace build, strict
Clippy, formatting, full tests (51 provisioned-native tests ignored), and
256-requirement parity check pass. A focused Apple-target probe compiles all 11 exact
`platform/macos/` source modules with strict Clippy. The sweep caught and resolved an
unsafe AppKit image-name symbol use, deprecated ICC API usage, and two strict-Clippy
issues in Now Playing artwork handling. A full local Apple-target application check
still stops in `ring` because this Windows host has no C compiler for the Apple
target. Live item order/shortcuts, activation, and titlebar rendering still need the
macOS application workflow and a Mac GUI capture.

## Windows DWM Mica host transparency · 2026-10-14

The native-video host now opts into Winit alpha composition. Its Slint background
stays black until `DwmSetWindowAttribute(DWMWA_SYSTEMBACKDROP_TYPE,
DWMSBT_MAINWINDOW)` succeeds and a readback confirms that type; it then becomes
  transparent so the DWM system backdrop can show through. The readback confirms
  the requested type, not visible Mica pixels; Windows can use an opaque fallback
  under system policy. If the request fails, the host keeps its black surface.
  `cargo clippy --workspace --all-targets
--locked -- -D warnings`, `cargo fmt --all -- --check`, `git diff --check`, and the
256-row `scripts/check-parity.py` traceability check pass. The computer-control
surface returned no available Windows apps, so there is no live screenshot or
visible-Mica claim. Transient Acrylic and the separate video-controls overlay
  still need material and playback comparison. The Welcome surface now uses the
  same request/readback gate and has not yet been visually captured. About uses
  `DWMSBT_TRANSIENTWINDOW` with an opaque theme fallback; its Acrylic rendering
  also awaits a live capture.

## macOS subtitle color panel source · 2026-10-14

The shared subtitle color well selects AppKit's alpha-enabled `NSColorPanel` on
macOS and keeps the existing custom RGBA editor elsewhere. The new native module
passes the isolated aarch64-apple-darwin strict-Clippy probe. The Windows workspace
strict-Clippy build and Slint UI generation pass; the Mac-only Preferences callback
wiring was not part of that Windows build. Full application Apple-target linking,
color-panel runtime, picker-mode behavior, save/cancel behavior and rendered output
remain unverified.

## Windows DWM material surface initialization · 2026-10-14

The regular Windows Winit backend now requests alpha-capable native windows before
creation; the Slint backend still forces windows with native menus opaque. About
and Welcome keep full-client fallback fills and expose translucent content only
after their DWM Acrylic/Mica requests succeed. `cargo build -p riina --locked`,
strict workspace Clippy, formatting, and `git diff --check` pass. The desktop
control surface currently exposes no Windows application windows, so actual DWM
material pixels and playback composition remain unverified.

## Windows Settings Mica surface · 2026-10-14

The Preferences window now requests the DWM persistent-window Mica backdrop after
creation. Its window surface becomes transparent only after DWM accepts the
request. The page/navigation surfaces use the 50% WinUI light Mica content-layer
fill while cards preserve their solid contrast and the no-Mica fallback. A live
Settings window returned DWM backdrop type 2 (`DWMSBT_MAINWINDOW`) with a
successful readback. `cargo build -p riina --locked`, strict workspace Clippy,
formatting, `git diff --check`, and the parity traceability script pass. The
desktop capture returned a black frame, so rendered Mica pixels and matched-window
appearance remain unverified.

## Windows native-video overlay material sampling · 2026-10-14

Native D3D11 playback now requests `screenshot-raw video rgba` only while an
enabled translucent overlay is visible. Sampling is limited to one request per
500 ms during playback and one per position while paused. The mpv worker
bilinearly downsamples directly from the owned RGBA buffer to at most 320×180
before retaining it in playback state. The UI applies blur and low-amplitude
grain, then places the SDR image
beneath Slint's translucent controller, sidebar and OSD tints. HDR output skips
the SDR sample. `cargo fmt --all -- --check`, `git diff --check`,
`cargo clippy --workspace --all-targets --locked -- -D warnings`,
`cargo build --workspace --locked`, and `cargo test --workspace --locked` pass.
Two material-sample unit tests verify the 320×180 output bound, uniform-color
preservation, and rejection of inconsistent RGBA storage. Synchronous as well as
asynchronous optional capture failures are kept out of playback's fatal error state.
The desktop automation currently exposes no Windows applications, so native
playback composition, capture cost, and matched material pixels remain unverified.

## Windows auxiliary Acrylic surfaces · 2026-10-14

The Video Filters, Playback History, Log Viewer, Online Subtitles and Font Picker
windows now request the DWM transient backdrop when shown. Each switches its root
tint to alpha only after DWM accepts the request; existing opaque controls remain
unchanged, and an error leaves the prior solid fill. The About window uses the
same shared request path. `cargo fmt --all -- --check`, `git diff --check`,
`cargo clippy --workspace --all-targets --locked -- -D warnings`,
`cargo build --workspace --locked`, `cargo test --workspace --locked`, and the
256-row parity traceability check pass. The desktop surface has no Windows app
available, so Acrylic pixels, backdrop policy fallback and readability remain
unverified. A later live window probe opened About from the app menu and read
DWM backdrop type 3 (`DWMSBT_TRANSIENTWINDOW`) successfully; it closed the exact
Riina process afterward. That confirms DWM accepted Desktop Acrylic, not that
the expected pixels are visible.

## Windows native video material strength and capture diagnostics · 2026-10-14

Native-video Slint overlays now use a 58% sampled-blur layer rather than the
shared-renderer/macOS 32% layer. The native Windows sidebar uses a 60%-opaque dark
tint or 78%-opaque light tint, 12 px corners, and a subtle one-pixel edge; macOS's calibrated 32% layer and
tint are unchanged. The 320×180 Windows sample now uses an 8 px Gaussian blur
before display scaling. The worker reports mpv's error text for failed material
captures, and distinguishes missing result data, decode failure, and downsample
failure from ordinary screenshots. `cargo build -p riina --locked`,
`cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets --locked -- -D warnings`, and
`cargo test --workspace --locked` pass. `git diff --check` and the 256-row parity
traceability check pass. A brief local fixture playback completed without a
material-capture error. Windows Computer Use exposed the Riina Welcome and a fresh
moving-square playback window, but screenshot capture failed on both at
`CreateForMonitor` with `0x80070057`; GDI returned a black GPU surface.
Therefore the opacity change has not been visually verified. The exact GUI test
process was closed after observation.

## Windows sampled Acrylic blur spread · 2026-10-14

The native Windows material image now uses an 8 px Gaussian sigma on the
320×180 sample before scaling to player-video bounds. The new synthetic-edge test
verifies the blur carries into a wider shoulder; the focused `video_blur` test
group passes (3 passed). `cargo build -p riina --locked`, strict package Clippy,
formatting, diff validation, and parity traceability pass. The material pixels
remain unverified because Windows Graphics Capture still fails with
`CreateForMonitor` (`0x80070057`) on the current desktop.

## Windows system material policy · 2026-10-14

Windows' `UISettings.AdvancedEffectsEnabled` and `GetSystemPowerStatus` now gate
Mica/Acrylic requests and the custom sampled-video overlay. The player polls the
cached policy while open; turning effects off or entering Battery Saver restores
opaque overlay fills, stops video-frame sampling, and clears the persistent host's
DWM Mica attribute. Re-enabling effects reapplies Mica and the configured glass
surfaces. Policy reads are cached for two seconds. Windows compile and strict app
Clippy checks pass. A
Windows GUI comparison with transparency enabled/disabled and Battery Saver
transitions remains required; it is not claimed as visually verified.
The rebuilt player launched against `moving-square.avi` and initially showed that
filename in its title. Folder auto-play advanced to other fixtures before the
accessibility observation; the tree exposed transport controls and Quick
Settings, but the material could not be held on one frame. The desktop screenshot
request still failed at `CreateForMonitor` (`0x80070057`). The exact test process
was closed and the desktop surface confirmed no Riina window remained.

## Windows Acrylic grain stability · 2026-10-14

The native-video sample grain now uses a stable coordinate pattern rather than
the playback frame revision as its seed. This keeps the noise layer anchored to
the material while sampled video changes. The focused `video_blur` test group
passes (4 passed), including a check that two different source frames receive
the same per-pixel noise offset. `cargo build -p riina --locked` and strict app
Clippy pass. Pixel-level live output remains unverified because desktop capture
still fails on this machine.

## Windows persistent-host backdrop policy changes · 2026-10-14

The persistent player host now polls the cached material policy every 500 ms and clears
`DWMSBT_MAINWINDOW` when Windows disables transparency effects or enters Battery
Saver. Its Slint background becomes opaque after DWM accepts the cleared type.
When effects are allowed again, the host requests Mica, checks the applied DWM
type, then restores the transparent client tint. A shared UI-thread monitor now
tracks weak handles for registered Mica and Acrylic windows, clears
their DWM backdrop and uses their solid fill when policy disables materials, and
restores the selected backdrop when policy allows it again. All three platform
policy reads must succeed and High Contrast must be off; otherwise custom
translucency uses the opaque fallback. The cache and
live-window monitors refresh at 500 ms. The focused policy tests pass (2 passed)
and `cargo test --workspace --locked` passes (318 app tests; 51 native libmpv
tests remain ignored because their DLL is not provisioned). The app build, strict
Clippy, formatting and diff checks pass. A live launch returned DWM backdrop
type 2 (`DWMSBT_MAINWINDOW`) with a successful readback for both the player host
and Settings window; by the six-second
observation the player had advanced to `track-languages.mkv`, and no Riina process
remained after shutdown. The Preferences window now uses the 50% light WinUI Mica
layer fill on page and navigation backgrounds only while the DWM backdrop is
accepted, leaving its solid fallback and content cards intact. The desktop capture
API still returned a black 1280×720
frame, so visible Mica/Acrylic pixels, live policy transitions and fallback
readability remain unverified.

## Windows material pixel calibration · 2026-10-14

Sampled the pinned REF-21 PNG directly: the solid green video region at
`(2000, 400)` is `#65C466`; a blank Quick Settings sidebar region at
`(2300, 800)` is `#459546`. A 53% video sample layered over the existing 60%-opaque
`#151515` Windows sidebar tint predicts `#4E934F`, closer to the measured sidebar
than the previous 58% strength. Only the Windows Quick Settings sidebar changes;
other native-video surfaces remain at 58%, shared-renderer and macOS layers at
32%. The nested Online Media Preferences page now reveals the existing Mica layer
through its body background as well. `cargo build -p riina --locked` passes after
these UI changes. No live Windows pixel capture was available; this source-image
calibration does not establish rendered parity.

The Windows Welcome Mica client now layers WinUI's dark default content fill,
`#4C3A3A3A` ARGB (Slint RGBA `#3A3A3A4C`), over its native backdrop. Its solid
fallback remains unchanged. `cargo build -p riina --locked` passes after this UI
adjustment; live Mica pixels remain unverified.

The fresh Windows launch exposed the Welcome controls in the accessibility tree,
but screenshot capture failed twice at `CreateForMonitor` (`0x80070057`), and
control input failed with `coordinate input geometry is unavailable`. No playback
material pixel check was possible. The exact Riina test process was stopped after
normal close failed; process and window inventories then confirmed it was closed.

The Quick Settings Windows card fill now follows WinUI's dark/light Acrylic layer
values (`#09FFFFFF` / `#40FFFFFF`) only while video-backed material is active.
`cargo build -p riina --locked` passes after this adjustment; live pixels
remain unavailable.

The Quick Settings `Fit Window to Video` action now uses WinUI dark/light
control-layer values and theme-appropriate foregrounds while Windows video-backed
sidebar material is active. Shared and opaque fallback styling is unchanged.
`cargo build --workspace --locked` passes; this source-derived adjustment has not
yet been visually verified in a live Windows capture.

For a fresh Windows GUI check, debug Riina launched and exposed its main window,
but Computer Use could not capture it: `IGraphicsCaptureItemInterop.CreateForMonitor`
failed twice with `0x80070057`. Normal close did not retire the test window, so
only the exact debug process started for this check (PID 13992) was ended. The
process and Riina window inventory are now empty; no rendered-pixel result is
claimed.

The native video overlay no longer requests legacy DWM blur; realization now
restores Winit's `WS_EX_LAYERED` alpha if needed and clears the mouse-transparent
style. [Microsoft documents](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/nf-dwmapi-dwmenableblurbehindwindow)
that `DwmEnableBlurBehindWindow` no longer produces blur starting with Windows 8.
The WinUI/DWM materials and the sampled video blur remain separate mechanisms.
This alpha path was later corrected: the player overlay had set `WS_EX_LAYERED`
without a layered-window update surface. See the Windows native overlay alpha
entry below; the legacy API is retained there only for its documented alpha
composition behavior.
After this change, `cargo build --workspace --locked`,
`cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`,
and `git diff --check` pass. No GUI test passed because screen capture remains
unavailable.

The About window's full-size content, sidebar and main pane now use transparent
fills after the shared DWM path accepts Desktop Acrylic; the solid dark/light
fills remain the rejection fallback. This makes the native backdrop visible
throughout its content surface. `cargo build --workspace --locked` passes. Live
contrast and acrylic pixels remain unverified because Windows screen capture is
still unavailable.

The Video Filters, Playback History, Log Viewer, Online Subtitles and Font Picker
windows now expose transparent roots only after the shared DWM path accepts
Desktop Acrylic; solid fallback colors remain. Utility toolbar, header, backing
panel and unselected row fills now use WinUI Acrylic layer values, while data-entry
fields and selected rows retain solid contrast fills. The reusable Font Picker's
live theme-following behavior is recorded below. Workspace build, formatting and
diff verification for this earlier adjustment passed: `cargo build --workspace --locked`,
`cargo fmt --all -- --check`, and `git diff --check`. Live appearance and text
contrast remain unverified because Windows screen capture is still unavailable.

The Windows PiP window now registers for the shared Desktop Acrylic policy monitor
on its first successful show. The client background becomes transparent only after
DWM accepts the backdrop; the video rectangle stays black behind the DWM thumbnail,
and the controls strip receives a dark translucent tint. The workspace build,
formatting and diff checks pass. Live thumbnail/control composition remains
unverified because desktop capture is unavailable.

The 46 outer Preferences cards now switch from their original opaque gray to the
existing 50% light WinUI Mica fill only when the DWM backdrop is accepted. Input
fields and selected rows stay solid. `cargo build --workspace --locked`,
`cargo fmt --all -- --check` and `git diff --check` pass. Live text contrast remains
unverified.

The registered Windows auxiliary windows now synchronize Winit's native theme
with their Slint palette when shown and when About/Video Filters change their
`dark-theme` property. Fixed light utility surfaces request light; Welcome and
Font Picker use the resolved player theme. `cargo build --workspace --locked`,
`cargo fmt --all -- --check` and `git diff --check` pass. Live DWM caption tint,
Acrylic pixels and theme-transition appearance remain unverified because Windows
screen capture is unavailable.

The Font Picker now follows the resolved player theme while visible. Its root tint,
native frame, headings, list text/selection, and preview change between matching
light and dark palettes; preview contrast and opaque selection are retained. Winit's
macOS implementation maps the same theme choice to AppKit `NSAppearance`.
Workspace build, formatting, strict Clippy and diff checks pass. Live theme/material
appearance and text contrast remain unverified without Windows screen capture.

The auxiliary DWM material watcher no longer treats an unchanged policy value as
proof that the backdrop is active. It retries while Riina's root transparency state
differs from that policy, allowing transient DWM failures to recover without a
system-setting change. Retry delays grow to 30 seconds and identical failures are
logged once until recovery or a policy change. Workspace build, strict Clippy,
formatting and diff checks pass; live recovery/pixels remain unverified without
Windows capture.

The persistent Windows player host now applies the same bounded retry and error
deduplication policy to Mica. Its confirmed DWM state is tracked separately from
the current OS policy and client alpha, so rejected operations do not prematurely
mark the backdrop current. The workspace build, strict Clippy, formatting and diff
checks pass. Disabling effects hides client alpha immediately if the Winit callback
cannot provide a native handle, while the DWM clear retries. Live recovery and
visible Mica remain unverified without capture.

About now tracks the live player theme while open; the shared Windows policy watcher
uses the resulting `dark-theme` value to update its native Winit theme. The workspace
build, strict Clippy, formatting and diff checks pass. Theme changes and caption/Acrylic
pixels remain unverified in a live Windows capture.

## Quick Settings sidebar dismiss control · 2026-10-14

The sidebar header now matches IINA's source-derived directional dismiss affordance:
36 pt circular control (28 pt compact), placed beside the tabs according to sidebar
side, with original Riina backward/forward chevrons. The header distributes the
control and tab group across the available width, matching IINA's equal-spacing
layout. The active Windows video-backed Acrylic surface applies a WinUI control-layer
fill and subtle edge to that button, with the secondary control fill on hover. REF-21
pixel scans suggest 150/42/74-point Theme, Compact Interface, and two-row video-layout
card heights with 20-point gaps. The 2 px/point scale is inferred from IINA's 360-point
sidebar default because capture scale was not recorded; inner row sizes were adjusted
to fit the estimated card bounds.
`cargo build --workspace --locked`, strict all-target Clippy, workspace tests
(318 app tests; 51 native tests ignored without the provisioned DLL), formatting,
and `git diff --check` pass. The two new SVG hashes match their manifest entries.
The full checkout asset check stops on the unchanged
`assets/icons/music-mode-placeholder.svg` raw-byte hash; the indexed/canonical-byte
check must be run after staging. A new live Windows render is unavailable, so the
control's final appearance and hit target remain unverified.

## Quick Settings video cards and WinUI controls · 2026-10-14

REF-24's video track list and four control groups now render as rounded cards,
with 18-point inter-card spacing and source-sized height estimates derived from
the inferred 2 px/point capture scale. Selected aspect, crop, and rotation presets
use the Windows blue accent on the native Windows surface and the macOS default
blue on shared rendering. Video labels, custom inputs, switches, and reset controls
follow the active theme and use WinUI-style translucent fills while the Windows
video-backed Acrylic sidebar is active. The same panel edits extend the prior
Audio and Subtitle light-theme controls. `cargo build -p riina --locked`, formatting,
and `git diff --check` pass. No live Windows GUI test was possible because the
screen-control tool exposed no application windows; REF-23, REF-24, and REF-25
remain open for matched capture comparison.

## Quick Settings subtitle cards · 2026-10-14

REF-25 now has separate scrollable primary and secondary track cards, paired
Load Subtitle and Search Online actions, and distinct timing/position and style
cards. Search Online calls the existing subtitle-search flow. The source screenshot
was measured at the inferred 2 px/point scale: track cards about 98 pt, Load/Search
about 46–48 pt, timing/position about 188 pt, and styling about 198 pt, spaced by
about 20 pt. The implementation uses 98/48/190/200 pt cards and 18 pt gaps. These
remain scale-derived estimates until a matched live capture is available.
`cargo build -p riina --locked` passed. A desktop capture could not be made because
the screen-control surface exposed no application windows; visual appearance and
interaction remain unverified against a live Windows build.

## Quick Settings audio card spacing · 2026-10-14

REF-23's saved capture gives estimated card heights of 98/48/92/246 pt for the
track list, external-track action, delay, and equalizer, with roughly 18–20 pt
between cards. Riina now matches those heights and uses 18 pt card gaps. The
selected track marker follows the Windows or macOS accent. Formatting and diff
checks and `cargo build -p riina --locked` pass. Live rendering remains open
because the desktop capture surface exposed no application windows.

## Music Mode compact window geometry · 2026-10-14

The REF-22 capture is 824×742 pixels; its player bounds measure about 600×518
pixels. The existing 300-point window width implies a 2× reference scale, giving
an estimated 300×259-point window with about 168 points of 16:9 artwork and a
90-point lower title/transport region. Riina's compact controller/window height
and the Windows host minimum now use 90 points instead of 72. Existing music-mode
layout test expectations were updated but not run. `cargo build -p riina --locked`,
formatting, and `git diff --check` pass. Matched visual capture checks remain open.

## Windows current-build player launch · 2026-10-14

Launched the current debug build with the reference test-pattern GIF. Windows
reported the title `sample-test-pattern.gif — riina`, confirming the filename is
present in the native title. The desktop bridge exposed no app windows, so no
rendered pixels or pointer interactions were verified. Closed the exact test
instance and confirmed no Riina process remained.

## Windows transport Acrylic tint · 2026-10-14

The native-video transport's dark fill changed from 76% to 60% opacity to match
the calibrated Windows title-bar and Quick Settings sidebar material over the
sampled video blur. `cargo fmt --all -- --check`, `git diff --check`, and
`cargo build -p riina --locked` pass. The desktop bridge exposed no app windows,
and the saved controller screenshot predates this change; rendered pixels and
text contrast remain unverified.

## Windows hardware-rendered material capture · 2026-10-14

Launched the current player with `tests/fixtures/moving-square.avi`, but its
native title reported `track-languages.mkv — riina`, so the requested fixture
was not confirmed as the displayed media. A foreground `CopyFromScreen` capture
returned an all-black frame for the hardware-rendered window and could not verify
material pixels. Closed the exact test process and confirmed it exited.

## Windows Music Mode menu visibility · 2026-10-14

A PrintWindow capture of the previous 300-point Music Mode window showed the
application menu wrapping into multiple rows and leaving the compact player
covered. A 60-point height reserve did not fix the wrapped menu. The current
Windows build hides that menu row only while Music Mode is active; the capture of
the native host confirms the headings are gone. The separately composited alpha
controls window returned black through per-window PrintWindow, so transport pixels
remain unverified. The exact test process exited, with no Riina process remaining.

## Windows shared-renderer Acrylic policy · 2026-10-14

The optional shared-GL Windows player now initializes and refreshes the system
material policy, gates sampled blur on that policy, and uses the same Windows
Acrylic card/control treatment as native-video playback. When effects are disabled,
it clears the cached blur and returns to solid surfaces. `cargo fmt --all -- --check`,
`git diff --check`, and `cargo build -p riina --locked` pass. No live shared-GL
material pixel comparison was available.

## Windows material transparency across renderer paths · 2026-10-14

The Windows shared-GL path now uses the same 53% Quick Settings and 58% other
video-backed sample strengths as native-video playback. Light-theme title,
sidebar and controller fills, plus dark OSD fills, use Windows translucency
values regardless of renderer. macOS retains its existing sample strength and
tint. `cargo build -p riina --locked`, `cargo fmt --all -- --check`, and
`git diff --check` pass. Matched Windows pixels and text contrast remain
unverified because the desktop surface capture is unavailable.

Selected track markers and Video/Subtitle preset controls now follow
`UISettings.GetColorValue(UIColorType::Accent)` on both Windows renderer paths,
with a `#0078d4` fallback; macOS keeps `#0a84ff`. Live system-color changes
refresh during playback, and selected labels switch between black and white
based on the accent's relative luminance. Layout toggles, progress bars, and
selected playlist markers use the same Windows accent. macOS colors are unchanged.
`cargo build -p riina --locked`, `cargo fmt --all -- --check`, and
`git diff --check` pass. The desktop capture tool currently exposes no Windows
app surface for matched pixel review.

## Windows native video overlay bounds and full-display capture · 2026-10-14

Alignment now reads the host HWND's live Win32 client rectangle and converts its
client origin to screen coordinates before positioning the controls overlay and
mpv child. `cargo fmt --all -- --check`, `git diff --check`, and
`cargo build --workspace --locked` passed. A DPI-aware 3840×2160 full-desktop
capture showed a separate debug console obscuring the player when the debug build
was launched visibly; hiding that console exposed the app window. Runtime window
inspection measured the overlay and video child on the host client bounds. The
media state changed to an audio-only Music Mode window during capture, so this
confirms native geometry but not video pixels or visual parity. The exact test
process was closed and no Riina process remained.

## Windows native player capture scope · 2026-10-14

The Windows computer-use helper returned a 918×619 host-window screenshot and a
separate 916×570 transparent-controls screenshot for `moving-square.avi`. The
playback accessibility state advanced from 0 to 0.125 seconds with Pause shown,
but neither window capture contained the rendered video. These are per-window
images, not a whole-desktop composite; the missing GPU child pixels make the
black viewport inconclusive and cannot show whether the video is confined to the
transport bar. A full-screen, DWM-composited capture is still required for that
question and for material pixel checks. The test window was closed, and a fresh
application inventory returned no Riina window.

## Windows player menu appearance · 2026-10-14

The app's saved explicit theme is Dark while Windows AppsUseLightTheme is enabled.
The native player host now receives the app's explicit Winit theme in its initial
window attributes, before Slint creates the Muda menu. `cargo build --workspace
--locked` and formatting passed. A live Computer Use capture showed a dark title
bar and menu strip on first player paint; opening Playback showed the native
dropdown still rendered in the OS light style. This improves initial frame/menu
strip alignment but leaves the menu popup's appearance open. The native window
capture still omits the GPU video child. Riina was closed after the check and a
fresh app inventory returned no Riina process.

## Windows native video overlay follows moved host · 2026-10-14

The controls overlay could stay at its former desktop position when the native
video host moved, leaving a black rectangle over the playback area. Alignment
now checks the realized overlay HWND bounds after the native move and reapplies
Winit's physical client geometry when Windows has not moved it yet. A live
`moving-square.avi` check moved the host by 300×300 screen pixels; the controls
surface moved by the same amount and retained its relative offset. The strict
workspace build and `git diff --check` passed. Per-window capture still omits
the GPU video child, so this confirms surface movement rather than the
composited video pixels. The test process was closed and no Riina window or
process remained.

## Windows DWM alpha and video-backed Acrylic · 2026-10-14

The native controls HWND no longer adds `WS_EX_LAYERED` without an
`UpdateLayeredWindow` backing surface. After Slint realization, Riina clears the
layered/mouse-transparent styles and re-applies DWM's per-pixel alpha contract;
the legacy blur-behind API is used for alpha handling, not visual blur. The
locked workspace build, `cargo fmt --all -- --check`, and `git diff --check`
passed. A live `moving-square.avi` capture showed the moving video visible
through both the native controls overlay and the tinted Windows Quick Settings
material. Moving the host by 300×150 screen pixels moved the overlay by the
same amount while preserving its offset. This verifies video visibility and
tracking on the native-video path; matched IINA pixels, OS-policy transitions,
and other display configurations remain open. Riina was closed after testing.

## Windows debug console no longer leaves a black desktop window · 2026-10-14

The Windows debug executable previously used the console subsystem while the
release executable used the GUI subsystem. Launching the debug player therefore
created a second `conhost.exe` window titled with the executable path. A live
window inventory showed that black console at screen origin (87, 72), separate
from the player at (543, 72), matching the reported fixed black region when the
player moved. The Windows player entry point now uses the GUI subsystem in debug
and release builds; standalone diagnostic binaries retain their console behavior.
`cargo build --workspace --locked`, `cargo fmt --all -- --check`, and
`git diff --check` passed. After rebuilding, a live playback launch exposed only
one Riina window and its green video test pattern filled the player viewport; no
separate console window appeared. The test instance was closed and a fresh
inventory returned no Riina window or process.

## Whole-desktop Windows capture · 2026-10-14

`scripts/capture-windows-desktop.ps1` sets per-monitor DPI awareness before
reading the virtual desktop bounds and captures every display at physical pixel
resolution. On this 3840×2160 desktop it captured the complete desktop; the
earlier DPI-unaware attempt returned only a scaled corner and was not used for
visual conclusions. The full capture showed the moving-square clip in Riina's
playback viewport and no separate black patch over it. This sample was at the
end of the five-second clip and the window was stationary, so it does not verify
the earlier screen-fixed patch report or a clip transition. A separate full
capture with Quick Settings open at 0:01 showed moving video visible through the
tinted sidebar and its translucent card fills, confirming video-backed Acrylic
composition; matched IINA pixels and Windows policy fallbacks remain unverified.
NVIDIA's performance readout remained anchored to the desktop edge. Riina was
closed normally after the inspection, with no process remaining.

## Windows Quick Settings category material · 2026-10-14

REF-24 shows a neutral glass capsule behind the selected Video category, unlike
the blue accent reserved for selected aspect/crop presets. The shared UI now uses
the neutral themed capsule on Windows as well; preset accents, focus rings, and
system-accent controls remain unchanged. `cargo build -p riina --locked` passed.
A post-build test used an isolated copy of `moving-square.avi` so autoplay could
not enter neighboring fixtures. UI Automation opened Quick Settings and selected
Video. The full 3840×2160 desktop capture and its player-window crop show the
neutral Video capsule, blue selected presets, and the paused video frame visible
through the tinted Acrylic panel. The crop is 2748×1856 physical pixels at 300%
scale; it is recorded at `reference/captures/riina-live-2026-10-14/` and described
in `tests/visual/windows-captures.json`. The frame was paused at the five-second
clip's end, so motion during this post-change capture was not verified. The
player size differs from fullscreen REF-24; total panel bounds, lower controls,
light-theme contrast, exact blur and system-policy fallbacks remain open. The
test instance was closed normally and no Riina process remains.

## Windows video-backed Acrylic sampling cadence · 2026-10-14

UI Automation opened Quick Settings/Audio during playback of a fresh isolated
copy of the reference 4 fps GIF. DPI-aware full-desktop captures show video under
the Audio panel and controller, confirming that the native overlay and sampled
material are visible together. Pixel probes across six captures at about 220 ms
intervals showed the panel color out of phase with the video at the 20 Hz
sampling experiment. The desktop performance overlay showed 10 fps and 18% CPU
in one capture; this was not a controlled benchmark. Raising capture cadence did
not resolve the phase gap, so the code was restored to its 10 Hz limit. The
restored `cargo build -p riina --locked` passed. Full-desktop and player-window
captures are recorded under `reference/captures/riina-live-2026-10-14/` and
indexed in `tests/visual/windows-captures.json`. The test instance was closed
normally and no Riina process remains. REF-23 remains open until material
sampling follows the presented frame with acceptable readback cost.

## Windows native overlay alignment retry · 2026-10-15

The alignment error path no longer writes possibly stale Winit outer-window
coordinates over the overlay's Win32 client-derived placement; the existing
16 ms alignment loop retries from the host HWND. `cargo build -p riina --locked`
passed. A DPI-aware move reproduced the stale overlay: the host client moved to
(686, 305), while its popup remained at (546, 215). A direct `SetWindowPos`
corrected it, while the old deferred batch reported success without moving the
owned popup. The 16 ms alignment retry now positions the controls popup directly
and batches only mpv child sizing; the separate aspect-ratio resize path retains
its existing batch. After rebuilding, the same move left the popup at exactly
(686, 305), size 2747×1711, matching the host client. The 3840×2160 desktop
capture shows the video and controls together without the old-position gray
strip. The test instance was closed and no Riina process remained.

## Windows controller volume visibility and live proportions · 2026-10-15

The REF-20 sample is the no-audio `sample-test-pattern.gif`. Pinned IINA's
`PlayerCore.aidChanged()` hides both mute and volume when the selected audio id
is zero. Riina now derives `has-active-audio-track` from the selected primary
audio track and hides that controller group when none is active, retaining its
leading spacer so transport remains centered. `cargo build -p riina --locked`,
`cargo fmt --all -- --check`, and targeted `git diff --check` passed. A full
3840×2160 desktop capture of the same GIF shows no mute/volume group; a fresh
isolated `moving-square.avi` capture with selected stereo audio keeps both
controls visible. Both GUI instances were closed and no Riina process remains.
The live 300%-scale controller measures about 460×66 points, centered with its
bottom 57 points above the 570-point host client bottom, consistent with IINA's
460×67-point controller and 10% vertical fraction. The Windows material remains
visibly out of phase in the animated GIF capture: a gold sampled controller
surface sits over a blue video frame. Material frame synchronization and exact
Windows Acrylic appearance remain open.

## Windows playback material scope and reply latency · 2026-10-15

A live Windows 11 trial applied `DWMSBT_TRANSIENTWINDOW` to the native full-size
controls popup; DWM accepted the attribute. A second trial placed a compact,
rounded, owned Win32 popup between mpv's video child and the controls window.
That popup rendered as a solid gray block over the moving-square video rather
than showing the video through Acrylic. The trial surface was destroyed and the
player closed. Its full-desktop capture is indexed in
`tests/visual/windows-captures.json`. The DWM backdrop path is not a valid
video-backed control material on this presentation path without a compositor
bridge that can sample the native video output.

Native material capture requests remain limited to one per 100 ms. The Slint
poller now checks for asynchronous replies every 16 ms, reducing the worst-case
wait between a capture reply and its display without increasing screenshot-raw
readback frequency. `cargo build -p riina --locked` passed. A fresh isolated GIF
playback and twenty 100 ms full-desktop pixel probes still showed different
source and controller colors; the inspected frame was about one GIF frame apart.
This rules out the UI reply-poll interval as the sole cause. The capture sequence
is indexed in `tests/visual/windows-captures.json`. The test instance was closed
and no Riina process remains. Frame synchronization and exact Acrylic appearance
remain open.

The pinned mpv source at `reference/mpv-upstream` documents
`--screenshot-sw=no` as rendering screenshots through the active VO; that path
may reinitialize its renderer and interrupt playback. Riina's normal screenshot
settings deliberately keep that path for output fidelity, and the material
sampler uses the same `screenshot-raw` command. This is a plausible readback-latency
and performance contributor, not yet a proven cause of the one-frame offset.
Do not change the global screenshot mode without preserving native screenshot
semantics and HDR behavior.

## Windows playback viewport and moved-window capture · 2026-10-15

Launched the current debug executable with an isolated copy of
`moving-square.avi` so folder autoplay could not change media during the check.
A 3840×2160 full-desktop capture showed the video across the playback viewport;
the black top/bottom bands were inside the player and moved with the video
window. Moving the native host by 100×70 screen pixels kept the video and
controls aligned, with no stationary black patch in the capture. A previous
non-isolated attempt had advanced through neighboring audio fixtures and
produced an ambiguous capture, so it is not used as evidence. The isolated
player was closed and no Riina process remains. The previously reported fixed
black debug console is already prevented by the Windows GUI-subsystem entry
point in `crates/app/src/main.rs`.

A separate desktop-fixed black banner appeared during the playback-window visual check. Its live capture showed the NVIDIA eye icon and the text “Press Alt+Z to open NVIDIA Overlay”; it belongs to the installed NVIDIA Overlay, not Riina's native video or controls windows. The banner remained screen-positioned while Riina was behind another window. Do not adjust player geometry to compensate for this external overlay.

## Windows presented-frame capture research · 2026-10-15

The native-player material sampler remained one displayed frame behind in the
4 fps reference clip after reducing its UI reply-poll delay. Checked Microsoft's
Windows Graphics Capture contract against the pinned `windows` 0.62.2 bindings:
`IGraphicsCaptureItemInterop::CreateForWindow` can target a native HWND;
`Direct3D11CaptureFramePool::CreateFreeThreaded` raises `FrameArrived` off the
UI dispatcher; and each `Direct3D11CaptureFrame` exposes a D3D surface plus
QPC-based `SystemRelativeTime`. These APIs provide a concrete path to compare
presented-frame timing with mpv's `screenshot-raw` route. Sources: [Microsoft screen capture guide](https://learn.microsoft.com/en-us/windows/apps/develop/media-authoring-processing/screen-capture),
[CreateFreeThreaded](https://learn.microsoft.com/en-us/uwp/api/windows.graphics.capture.direct3d11captureframepool.createfreethreaded),
[CreateForWindow](https://learn.microsoft.com/en-us/windows/win32/api/windows.graphics.capture.interop/nf-windows-graphics-capture-interop-igraphicscaptureiteminterop-createforwindow).

## Windows window-scoped material capture implementation · 2026-10-15

Added an optional Windows Graphics Capture session owned by the native player
presentation. Its free-threaded callback reads the host window's D3D11 surface,
uses a D3D11 staging crop for the client area, converts the captured BGRA pixels,
and retains only a 320×180-or-smaller sample. Material UI updates prefer fresh
window-capture frames and retain the existing mpv `screenshot-raw` request as a
fallback; capture is disabled and its last sample cleared when the material is
hidden, playback ends, or the video generation changes. HDR keeps its tint path.
`cargo check -p riina --locked` and `cargo build -p riina --locked` passed after
binding and thread-ownership fixes. A full-desktop check with an isolated
`moving-square.avi` showed the video viewport and its black letterbox bands move
with the player; no desktop-fixed black square remained at the old position. The
image is indexed in `tests/visual/windows-captures.json`. Because mpv sampling
remains enabled as a fallback, this screenshot does not establish that WGC supplied
the displayed material sample. Exact sampler-source delivery, timing, transitions,
and shutdown during a capture-active session remain unverified. Closed the exact
test instance and confirmed no Riina process remains. `cargo fmt --all -- --check`
and `cargo clippy -p riina --all-targets --locked -- -D warnings` passed.

## Windows native video position guard and screen-fixed patch recheck · 2026-10-15

The 16 ms native-surface alignment loop previously considered an mpv child
aligned when its client size matched the host, without checking its screen
origin. It now checks both the host client origin and size, and retries its
existing child-window placement when either differs. `cargo check -p riina
--locked`, `cargo build -p riina --locked`, and `cargo fmt --all -- --check`
passed. A full-desktop playback check with an isolated `moving-square.avi`
fixture moved the host from (528,72) to (648,172). The host client origin,
controls popup, and mpv child all measured (666,315) at 2747×1711; a full-desktop
capture showed the square and controls inside the player with no screen-fixed
black patch. The capture is indexed in `tests/visual/windows-captures.json`.
This sample was near the clip's final frame, so transition flashes and the
user's affected media remain unverified. Riina was closed after the check.

## Quick Settings Layout geometry · 2026-10-15

The Layout pane uses IINA source-defined outer spacing (18 points spacious,
16 compact), card padding (12/10), group spacing (12/8), and material-switch
row spacing (6/5). On the rebuilt Windows 11 app, opened Quick Settings >
Layout and inspected both Spacious and Compact Interface states over an
isolated moving-square fixture. Both full-desktop captures are indexed in
`tests/visual/windows-captures.json`; controls fit without clipping. The
spacious card remains about 4 points taller than the previous screenshot
estimate. This was a visual review, not a pixel-scored comparison. The final
`cargo build -p riina --locked` passed (42.99 seconds), and
`cargo fmt --all -- --check` passed. The exact test process was closed and no
Riina process remains. Workspace Clippy and tests were not run for this visual
adjustment.


## OSC follows video viewport around sidebars · 2026-10-15

The pinned `MainWindowController.setupVideoContainerConstraints()` bounds the
video container between open sidebars unless Edge-to-Edge Video is enabled;
`OSCFloatingView` sizes and centers its controller inside that container. Riina's
floating and docked controller now use the same conditional horizontal viewport
for width, position, center snapping, and normalized drag persistence, and animate
viewport changes over the sidebar's 240 ms transition. `cargo build -p riina
--locked` passed (59.27 seconds); `cargo fmt --all -- --check` passed. A rebuilt
Windows GUI check opened Quick Settings/Layout, disabled Edge-to-Edge Video, and
confirmed the centered floating controller ends before the trailing sidebar. The
pre-test preference was restored, a full-desktop capture is indexed, and the exact
test process was closed. Drag persistence and custom-position behavior were not
exercised; no test suite or Clippy run was requested for this visual adjustment.

## Full-desktop playback patch check · 2026-10-15

Captured the full 3840×2160 desktop while `sample-test-pattern.gif` played in the
0.3.8 debug build, then moved the player host by 120×90 physical pixels and
captured again. In both frames the video is visible across the playback viewport.
The only black regions are horizontal aspect-fit matte bands inside the player;
they move with the video window. The vacated screen area shows the browser, with
no stationary black square. Both captures are indexed in
`tests/visual/windows-captures.json`. This does not reproduce the report on the
user's media or verify rapid playlist transitions. Closed the exact test process
and confirmed no Riina process remains.

## Windows Quick Settings Video material calibration · 2026-10-15

Matched the pinned REF-24 Video panel against the same green test-pattern fixture
on a full 3840×2160 Windows desktop. With card fills excluded, the un-carded
trailing strip measured 54.2% relative linear-sRGB luminance at 28% tint and
49.1% at 32%; the sampled IINA panel/video pair measures 52.1%. Set the Windows
dark sidebar tint to 29% (`#1515154A`); the rebuilt capture measured 52.5%, a
0.4-point difference from that reference sample. The live capture is indexed in
`tests/visual/windows-captures.json`. Sidebar width is 360 points on both surfaces
(Windows at 300% scale and REF-24 at 2×). This verifies tint and width only; blur
timing, the remaining panel geometry, light appearance, and policy fallbacks remain
open. The build and formatting check passed, and no Riina process remains.

## Windows floating controller material calibration · 2026-10-15

Compared the floating controller against pinned REF-20 over the green test
pattern. REF-20's background sample retains 71.8% relative linear-sRGB luminance;
the previous Windows 33% tint retained 47.4%. The Windows dark controller and
in-app title strip now use 15.7% tint (`#15151528`). The rebuilt full-desktop
capture measured 72.0% in the same controller region, a 0.2-point difference.
The capture is indexed in `tests/visual/windows-captures.json`. Pixel scans put
the visible bar width at about 458 points on Windows and 461 in REF-20; the
source-defined width is 460 points. Moving-video blur phase, vertical silhouette,
text contrast, light appearance and Mac runtime remain open. The workspace build
and formatting check passed, and the test process was closed with no Riina
process remaining.

## Windows sampled Acrylic transform cost · 2026-10-15

Rebuilt after moving the native CPU blur to half-sized samples, then restoring
the 320×180 material image before grain. Launched an isolated copy of
`sample-test-pattern.gif` and captured the full 3840×2160 desktop; the complete
player window and video viewport are visible in
`reference/captures/riina-live-2026-10-15/windows-material-halfsample-full-desktop.png`.
The trace confirmed Windows Graphics Capture supplied 43 displayed samples.
Readback took 84–93 ms, the UI-thread material transform took 50–56 ms, and
sample age at display was 137–160 ms. The previous full-size transform measured
123–130 ms with 220–344 ms sample age, so the half-size transform reduced CPU
work and improved freshness. The transform still blocks the UI thread and the
capture readback remains costly; asynchronous or GPU-side processing and matched
pixel comparison against Windows Acrylic remain open. `cargo build -p riina
--locked`, `cargo fmt --all -- --check`, and `git diff --check` passed. The test
instance was closed normally and no Riina process remains.

## Windows mapped-frame downsampling cost · 2026-10-15

The WGC readback now bilinearly samples the mapped BGRA staging surface directly
into the bounded 320×180 RGBA image. This removes a full client-size RGBA
allocation and conversion followed by a second CPU downsample, while retaining
the same pixel-center bilinear mapping as `CapturedFrame::downsample_to_fit`.
An isolated animated GIF ran in the rebuilt 0.3.8 debug app. Its full 3840×2160
desktop capture is indexed at
`reference/captures/riina-live-2026-10-15/windows-material-direct-downsample-full-desktop.png`.
WGC tracing measured 5–7 ms readback, 51–58 ms worker transform, and 75–117 ms
capture-to-display age, compared with 95–105 ms readback and 169–181 ms age
before direct downsampling. The controller shows sampled video through its
translucent surface in the full-desktop image. Exact material matching, rapid
clip transitions, and sub-100 ms worst-case age remain open. The app was closed
and no Riina process remains.

## Windows sampled Acrylic work moved off the UI thread · 2026-10-15

The Windows material transform now runs on one bounded worker thread. Its input
queue holds at most one waiting sample and drains to the newest frame before
processing, avoiding an ever-growing queue of stale images. Pixel buffers return
to the UI thread; only Slint image creation and property updates remain there.
The UI discards results from old playback generations and expired samples.

Built and played the isolated animated test pattern with material tracing. A
3840×2160 full-desktop capture at
`reference/captures/riina-live-2026-10-15/windows-material-worker-full-desktop-1.png`
shows the video across the playback viewport and the translucent controller
over it. Three WGC samples were transformed in 51–57 ms each; readback measured
95–105 ms and sample age at display measured 169–181 ms. Moving the transform
off the UI thread addresses the 50 ms synchronous UI stall, but does not reduce
capture-to-display age; the GPU readback remains the dominant delay. Rapid clip
changes and quantitative UI responsiveness still need a dedicated check. Build,
format, and capture-manifest validation passed. Closed the test instance and
confirmed no Riina process remains.

## REF-20 right-toolbar frame geometry on Windows · 2026-10-15

IINA's `OSCToolbarButton.swift` assigns 24×24-point frames; its playback
`NSStackView` uses zero spacing. Riina's four default right-side actions
previously occupied 132 points as 30×17-point controls with 4-point gaps. They
now occupy the source-derived 96-point cluster, with 24-point frames and zero
spacing; compact rows use the same width while constraining button height to the
available row. `cargo build -p riina --locked` passed in 1m46s and
`cargo fmt --all -- --check` passed. The rebuilt 918×619 target-window capture
shows the four action glyphs in place over the green test pattern; it is indexed
in `tests/visual/windows-captures.json`. This verifies bounds and visibility,
not glyph matching or full-desktop pixels. Closed the exact test instance and
confirmed no Riina process remains.

## Windows quarter-resolution material blur and controller check · 2026-10-15

The sampled material blur now reduces the 320×180 frame to quarter dimensions
before applying the same wide 5×5 kernel and restoring the texture for stable
grain. This preserves the full-frame blur radius while cutting worker pixels by
four compared with the half-size pass. On the isolated paused GIF, a full
3840×2160 desktop capture shows the complete player and controller, with the
controller over the playback image. WGC readback measured 5–6 ms, transforms
28–31 ms, and capture-to-display age 39–54 ms, improving from 51–58 ms and
75–117 ms respectively. The live controller remains about 460×66 points,
centered with its default 10% bottom offset. This does not establish exact
Windows Acrylic pixel parity or rapid transition behavior. The build and format
check passed; the app was closed and no Riina process remains.

## Windows material source identity and fixed-screen artifact check · 2026-10-15

Material transform keys now include their source as well as playback generation
and frame revision, so WGC sequence 1 cannot be mistaken for mpv material frame
revision 1. Paused frames now expire under the same 750 ms freshness limit as
playing frames. When native capture is available, mpv fallback waits 500 ms for
its initial WGC sample. The rebuilt debug app displayed five traced WGC samples;
readback took 5–7 ms, transforms 31–41 ms, and capture-to-display age 57–73 ms.
No stale mpv fallback result was displayed. The full 3840×2160 desktop capture is
indexed at `reference/captures/riina-live-2026-10-15/windows-material-source-identity-full-desktop.png`.

A separate movement check on the previous build moved the player from its
initial desktop position; the playback surface and controls moved with it, and
no black patch remained at the old coordinates in the full-desktop capture. The
desktop does show a fixed NVIDIA performance overlay, so an external screen
overlay remains a plausible explanation if the reported black square appears
again. This run did not reproduce that square. Build, format and diff checks
passed, and the exact test instance was closed with no Riina process remaining.

## Windows transport spacing and full-desktop playback check · 2026-10-15

IINA's `MainWindowController.swift` creates three 24-point playback buttons and
sets their floating stack spacing to 24 points (16 docked). Riina now uses
24-point-wide transport controls with the matching floating/docked spacing. The
rebuilt debug app opened the recent `sample-test-pattern.gif`; the video remained
visible across the playback viewport and the controller showed Previous, Play,
Next, then the four configured right-side actions. Visual inspection confirmed
the wider center spacing. The captured screen is the full 3840×2160 desktop; the
smaller 1280×720 CUA image is the target window's logical screenshot at the
300%-scaled display, not a desktop crop. Capture and hash are recorded in
`tests/visual/windows-captures.json`. `cargo fmt --all -- --check` and
`cargo build -p riina --locked` passed. Closed the test instance and confirmed no
Riina process remains. Matched icon shape, exact bar height, material blur and
pixel-scored comparison remain open.

## REF-20 controller proportions on Windows · 2026-10-15

The original 2940×1898 REF-20 image measures about 920 pixels wide and 156
pixels across the controller's material fill, or approximately 460×78 points at
the inferred 2× capture scale. Its source layout accounts for that height:
14-point top inset, 24-point controls, 8-point gap, 24-point timeline and
8-point bottom inset. Riina previously allocated only 67 points and rendered a
roughly 64-point material fill. The rebuilt Windows layout now uses 78 points
spacious and 71 compact; small-window compression remains unchanged. On the
300%-scaled Windows display, the full-screen 1280×720 logical capture measures
the Riina controller at about 459×77 points, and the green video fills the
entire display behind it. This verifies outer proportions and placement, not
pixel-level tint or blur phase. Capture and hash are in
`tests/visual/windows-captures.json`. `cargo fmt --all -- --check` and
`cargo build -p riina --locked` passed. Closed the exact test instance and
confirmed no Riina process remains.

## REF-20 playlist and PiP toolbar symbols on Windows · 2026-10-15

The pinned IINA toolbar uses `list.bullet.rectangle` for Playlist and `pip.enter`
for PiP. Riina's previous icons were a list/play arrow and a monitor stand. The
original MIT SVGs now show a bulleted list in a rounded rectangle and a nested
picture window, respectively. `cargo build -p riina --locked` passed in 39.67s;
`cargo fmt --all -- --check` passed. The rebuilt 918×619 target-window capture
shows both icons at playback size with the green video visible through the
translucent controller. The capture and SHA-256 are indexed in
`tests/visual/windows-captures.json`. This is a qualitative shape comparison,
not pixel-level matching. The full local asset checker stopped at the unrelated
`music-mode-placeholder.svg` checkout-byte hash; the two edited icon hashes
match their provenance entries. Riina was closed and no process remains.

## REF-20 timeline label spacing on Windows · 2026-10-15

IINA's `MainWindowController.swift` gives elapsed and duration labels a
46-point minimum width, centers their text, separates them from the slider by
2 points, and keeps an 8-point outer inset. Riina had content-sized left-aligned
labels with an 8-point gap. The Windows timeline now uses those source values;
small-video mode reduces the minimum label width to 34 points. `cargo build
-p riina --locked` passed in 1m32s and `cargo fmt --all -- --check` passed.
The rebuilt 918×619 target-window capture shows centered labels, the shortened
seek track, and the video visible behind the 78-point controller. It is indexed
in `tests/visual/windows-captures.json`. This is a source-geometry and visibility
check, not a pixel-scored comparison; exact digit typography remains open. No
Riina process remains.

## Isolated full-desktop black-patch movement recheck · 2026-10-15

A controlled 0.3.8 debug playback check opened a temporary isolated copy of
`tests/fixtures/moving-square.avi`. Full-desktop captures before and after moving
the native host 160×100 physical pixels show the moving square and horizontal
black aspect-fit bands following the player, with no stationary black patch at
the previous coordinates. NVIDIA's performance metrics stayed fixed at the
upper-right. No Riina code change was made because this fixture does not reproduce
the user's media or playlist handoff; those cases remain open. Both full-desktop
images and their SHA-256 hashes are indexed in `tests/visual/windows-captures.json`.
The exact GUI test process was closed and no Riina process remains. No build or
automated tests were run for this visual-only recheck.

## REF-21 Layout icon sizing and full-desktop capture · 2026-10-15

`cargo fmt --all -- --check`, `cargo check -p riina --locked`,
`cargo build --workspace --locked`, and
`cargo clippy --workspace --all-targets --locked -- -D warnings` passed after the
16/20-point icon-size correction. The full-desktop script initially failed at its
C# DPI helper; its block-bodied replacement ran successfully and returned
3840×2160 with non-black probes. The isolated-video capture shows the video across
the player surface, the controller at the bottom, and Quick Settings/Layout at the
right. Capture hash and state are indexed. GUI behavior was inspected at the final
frame of the five-second fixture, so transitions are not verified. `cargo test
--workspace --locked` passed in 1m35s. Its 51 native libmpv integration cases are
ignored by default; the provisioned `tools/mpv/libmpv-2.dll` still matches the
SHA-256 in `reference/native-lock.json`, but no ignored native test was run. The
GUI test instance was closed and no Riina process remains.

After all builds, checks, and GUI testing ended, `target/debug` measured 50.55 GB.
The package-scoped `cargo clean -p riina` removed 39.7 GiB of generated Riina
outputs. `target/debug` is now 10.89 GB, C: has 128.76 GB free, and the shared
Cargo registry was not touched. Cargo incremental compilation remains enabled.

## Screen-fixed overlay triage · 2026-10-15

Rebuilt and opened the current debug player with an isolated copy of
`moving-square.avi`. The five-second fixture had reached its final frame by the
time the window was moved, so motion during the drag was not verified. The video
image, title bar, menu, and controls remained together in the moved player. A
large stationary desktop panel was also visible before launch and remained after
Riina had exited; its lower strip reads “Default IME” and “MSCTFIME UI”. The
Windows `TextInputHost` process was present, while the panel's owning HWND was not
identified. NVIDIA performance metrics were also visible at the fixed upper-right
desktop position. This capture does not reproduce a Riina-owned black patch; the
user's affected media remains unverified. No source change was made. The exact
debug player was closed and no Riina process remains.

## Native video surface opacity and Audio delay ticks · 2026-10-15

The first full-desktop capture reproduced desktop content showing through the
native playback surface. `NativeVideoHost` made its client background transparent
to expose Mica, which is unsuitable for the HWND receiving native video. Its
background is now opaque black; the independent controls HWND retains its
translucent Windows material. `cargo build -p riina --locked` passed in 1m12s.
The rebuilt app played an isolated `moving-square.avi`. A 3840×2160 full-desktop
capture shows no browser content through the video, while Quick Settings remains
translucent over it. Quick Settings/Audio also shows 21 delay tick positions and
the centered `0s` value label. Both captures and hashes are indexed in
`tests/visual/windows-captures.json`. This was visual inspection, not a
pixel-scored REF-24 comparison; the unrelated stationary IME surface remains in
the desktop, and the user's original media/black-patch report is not reproduced.
The exact debug process was closed and no Riina window or process remains.

## REF-23 Audio pane details on Windows · 2026-10-15

Compared the delay control to the pinned IINA Audio panel and
`SidebarSliderView.swift`. Riina now uses 2-point round low-contrast marks at
the 21 delay positions, matching IINA's small native tick dots; its neutral
indicator remains centered and the live `0s` value sits above the slider.
Added original 16-point clock-delay and three-band Equalizer symbols, and matched
the sidebar action label to `Load External Audio Track…`. The exact assets and
MIT provenance hashes are recorded in `assets/provenance.json`. `cargo build -p
riina --locked` passed in 1m44s; a follow-up build passed in 54s after matching
the external-audio button's 24-point height and 12-point side insets. The rebuilt
Audio pane was inspected during isolated-fixture playback on the full 3840×2160
desktop. The playback image is opaque, while the Acrylic Quick Settings cards
reveal the video beneath them.
Capture hash and state are indexed in `tests/visual/windows-captures.json`.
This is a source-based visual review, not a pixel-scored comparison. Audio
card-bound measurements, light appearance, and WGC material lag/cost remain open.
The exact test window was closed and no Riina process remains.

## REF-20 Windows transport glyph proportions · 2026-10-15

The pinned `MainWindowController` gives its speed-down, play/pause, and
speed-up buttons 24×24-point frames; the speed controls are double chevrons by
default. Riina's `Tool` component previously limited their artwork to 17 points,
making all three center glyphs visibly undersized. The icon-size limit is now
configurable: the floating and docked transport controls use 24 points, while
other tools retain their existing 17-point default. Added independently drawn
double-chevron speed icons and large play/pause symbols. `cargo fmt --all --
--check` and `cargo build -p riina --locked` passed. The rebuilt Windows debug
player opened the isolated 4-fps pattern; a live target-window capture confirms
the larger center icons, double-chevron speed arrows, video behind the
translucent controller, and four right-side actions. Separate paused and playing
captures confirm the large play triangle and pause bars. During active playback,
WGC samples followed the changing pattern colors; readback measured 5–6 ms,
transform 30–32 ms, and capture-to-display age 42–68 ms. This check is not
pixel-scored and did not inspect the compact layout. The exact test process was
closed and no Riina window or process remains.

## Timeline preview and screen-fixed black-patch recheck · 2026-10-15

During a live check of `sample-test-pattern.gif`, a dark rectangle appeared only
while the pointer hovered over the seek bar. It contained the `0:00` preview
label and disappeared as soon as the pointer moved into the video. The Slint
timeline intentionally renders its thumbnail-preview card at the pointer, which
explains this transient rectangle; it was not over the video surface. The video
and its aspect-fit bands remained together when the player window moved, and a
full-desktop capture showed no black patch fixed at the former player position.
After closing the exact debug process, a pale desktop panel remained with the
labels “Default IME” and “MSCTFIME UI”; its owner is still unconfirmed and it is
outside Riina. The user's reported media and playlist transition still need a
reproduction. WGC diagnostics for this run measured 5–6 ms readback, 31–41 ms
transform, and 57–76 ms sample age. No code change or build was made. No Riina
window or process remains.

## REF-21 keyboard tab focus and black-patch recheck · 2026-10-15

`cargo build -p riina --locked` passed in 1m37s and `cargo fmt --all -- --check`
passed. On the rebuilt debug player, mouse-selecting Layout leaves the Quick
Settings tab without a focus outline; pressing Space shows the keyboard focus
ring. With `sample-test-pattern.gif` playing, I moved the Riina window twice and
checked full-desktop captures near the right display edge. The separate mpv
video HWND followed the main window by the same offsets, and the test pattern
stayed visible. Its narrow black bands remained aligned with the player and are
aspect-fit letterboxing. The fixed dark strip was at the far desktop edge,
outside the playback image; no Riina-owned black patch was reproduced. The
strip's owner is still unidentified, and the user's original media remains
unverified. The exact debug player was closed; no Riina window or process
remains.

## Windows sampled-video material and controller alignment · 2026-10-15

The Windows sampled-video material filter evaluates the existing 5×5 weight
matrix as two separable passes with five exact center-line corrections. Its final
quarter-size-to-display enlargement now precomputes bilinear sample coordinates
instead of invoking the generic image resizer for every output frame. The focused
`native_material_enlargement_matches_triangle_resampling` test passed, with every
channel within one level of the previous Triangle resizer. `cargo fmt --all --
--check` and `cargo build -p riina --locked` passed. In the debug build,
WGC readback measured 5–6 ms, material transform 17–19 ms, and capture-to-display
sample age 39–66 ms; the previous transform trace measured 26–30 ms. A live
isolated `sample-test-pattern.gif` check showed the WGC material following the
animated colors beneath the translucent controls. Center RGB samples stayed within
two levels per channel of the captured uniform color after stable grain. The
3840×2160 full-desktop capture and player-window crop are indexed in
`docs/visual-reference-audit.csv`; the crop SHA-256 is
`56B4212A7B2580ACC897CE9E83E936F5C1E7E52D35C637669432162305FB0BE3` and the
full-desktop SHA-256 is
`33E96E805B4E488B25D7D29A3C2E604DCFB337E127A984C603B33FC1F5377E0D`. The live
window measures 918×619 logical points; the controller fill spans approximately
458 points against IINA's 460-point source width. Its video image and narrow black
aspect-fit bands move with the player; no stationary black patch was reproduced.
The isolated GUI instance was closed and no Riina window or process remains.
Nonuniform video blur, exact controller vertical bounds and glyph contours, light
appearance, system-policy transitions, and native macOS rendering remain open.

## REF-20 Windows controller geometry correction · 2026-10-15

Source review found that IINA's `OSCFloatingView.swift` uses 67 points only for
vertical position clamping. `MainWindowController.swift` wraps the small native
seek slider in 6-point top and bottom padding, making the visible timeline row
24 points. With its 14/8-point spacious and 10/5-point compact insets, the visible
panel measures about 78 points spacious and 71 compact. Riina's intervening
67/60-point layout omitted that padding and was too short. The Slint layout now
uses 78/71 points and a 24-point timeline row. `cargo fmt --all -- --check` and
`cargo build -p riina --locked` passed. The 3840×2160 full-desktop capture at
300% scale and player crop show the full controls, a 458×78-point visible panel,
and a 58-point gap in the 619-point client (9.4%). IINA's matched 2× capture
measures about 460×80 visible points with a 9.7% bottom gap; the remaining panel
bounds differ by about two points. Capture paths and hashes are recorded in
`docs/visual-reference-audit.csv`. A separate moving-square playback capture
shows successive video frames; WGC transform time was 18–19 ms and displayed
sample age 27–54 ms. The exact test instance was closed and no Riina process
remained. The controller still needs a moving, high-contrast shape directly
beneath it to verify nonuniform blur strength and sample phase.

## REF-23 Audio Equalizer layout correction · 2026-10-15

Compared Riina's Quick Settings Equalizer with IINA v1.5.0's `SidebarAudioPane.swift`.
IINA places the heading, full-width preset selector, 120-point vertical fader bank,
and 10-point frequency labels in separate rows. Riina previously placed the
selector beside the heading and shortened each fader column to fit labels below.
The Windows card now follows the source order, with 12/10-point spacious/compact
padding and 12/8-point row spacing. Existing card bounds and sampled-video Acrylic
remain in place.

`cargo fmt --all -- --check` and `cargo build -p riina --locked` passed. The rebuilt
player was inspected at 3840×2160 / 300% scale over an isolated 4 fps pattern. The
full-desktop capture shows the separate heading, full-width selector, full-height
faders, and video through the dark material. A scrolled capture shows all ten
faders, the dB scale, and frequency labels. Both hashes and states are indexed in
`tests/visual/windows-captures.json`. The player was closed after inspection.
Pixel-scored card comparison, light appearance, and live material timing/cost remain
open.

## REF-21 Layout symbol correction on Windows · 2026-10-15

Compared the live Quick Settings/Layout pane with the pinned `SidebarLayoutPane.swift`
and REF-21 capture. The source assigns 20-point symbols to Theme, Compact Interface,
and video rows; its material subheading and mini rows use 16-point symbols. Riina
previously showed a generic window, gear, chat bubble, mouse, list, and filmstrip in
these settings. Replaced those with eight independently drawn symbols for appearance,
Windows material, OSD, OSC, sidebar, compact interface, edge-to-edge video, and
aspect lock. The new original SVGs are MIT and have matching SHA-256 provenance
records.

`cargo fmt --all -- --check` and `cargo build -p riina --locked` passed. The rebuilt
0.3.8 player was opened with an isolated copy of the 4 fps pattern. The full
3840×2160 capture shows all eight symbols at their source-sized frames and video
through the Windows sampled Acrylic. The capture and hash are in
`tests/visual/windows-captures.json`; no pixel-scored glyph/card comparison or light
theme check was done. The exact test PID exited gracefully and no Riina process
remains.

## REF-21 Windows theme segment focus behavior · 2026-10-15

Compared the theme selector with IINA's native `NSSegmentedControl` in
`SidebarLayoutPane.swift`. `SidebarTab` now suppresses its focus border after
pointer selection and shows it after keyboard navigation or activation, matching
the platform's pointer-versus-keyboard focus behavior. `cargo fmt --all -- --check`
and `cargo build -p riina --locked` passed. On Windows 11 at 300% scale, a live
`sample-test-pattern.gif` session showed the selected Light segment without a
focus border after a mouse click; pressing Space showed the keyboard focus ring.
The full 3840×2160 captures are indexed in `tests/visual/windows-captures.json`.
Light Acrylic remained visible over playback. Card/tab pixel scoring and exact
material comparison with REF-21 remain open. The test instance was closed and no
Riina process remains.

## REF-21 Quick Settings category icon parity · 2026-10-15

The pinned `QuickSettingViewController.swift` assigns paintbrush.fill, photo,
waveform, and captions.bubble.fill to Layout, Video, Audio, and Subtitles;
`SideBar.swift` renders them at 14 points with bold weight. Riina's tab row now
uses original MIT equivalents for those four meanings at 14 points. The rebuilt
3840×2160 Windows capture and a same-scale header crop show the new glyphs in
place over video-backed Acrylic. The selected label was increased from 11 to 12
points after the first matched crop measured 69×20 px versus IINA's 78×24 px. The
final 12-point capture measures 76×22 px; the label fits its segment. Build,
formatting, provenance hash, JSON and CSV checks passed. The final full capture is
indexed; exact glyph contours and card/tab pixel scoring stay open. The test
instance was closed and no Riina process remains.

## Windows transition-cover black-square guard · 2026-10-15

The rendering boundary now independently requires an outgoing transition image
before enabling its opaque black matte. This closes the empty-image path even if
the display predicate changes later. `cargo build --workspace --locked` and
`cargo fmt --all -- --check` passed. A fresh full-desktop 3840×2160 check moved
the player during `moving-square.avi` playback, then allowed the local fixture
playlist to advance to another item. Video followed the host; no screen-fixed
black square appeared. The user's original media and triggering transition
remain unverified. Closed the exact test process and confirmed no Riina process
remains.

## User report: black square stays fixed while player moves · 2026-10-15

The user reports that the black square remains at the same desktop location
when the playback window moves. This points to a separate desktop-anchored
surface, but does not identify its owner. A current window inventory had no
Riina process and showed NVIDIA GeForce Overlay windows; the full-desktop GDI
capture returned uniformly black and is invalid for visual attribution. Do not
adjust player geometry until the square is reproduced with the player and its
native host, video child, and controls HWND bounds captured together.

## REF-24 fractional crop rail and native controls geometry · 2026-10-15

The selected Crop `None` segment now uses the measured 56.5-point width. The
workspace build and formatting pass. UI Automation on an isolated moving-square
playback at 300% scale reports Aspect `Default` at 204×72 physical pixels and
Crop `None` at 169×72 pixels; the latter reflects integer screen-bound rounding
from 56.5 points. The ratio buttons measure 135/150/168/147/135 px, matching
45/50/56/49/45 points. The native controls HWND aligns to the host client bounds
(546,215,2747,1711) and receives hit tests over playback. A 3840×2160 desktop
capture passed non-black validation but omitted the controls/sidebar despite the
accessible controls being onscreen. Treat geometry as verified and the rendered
surface/pixel comparison as unresolved. The test instance was closed and no
Riina process remains.

## REF-20 Windows floating controller position conversion · 2026-10-15

The Windows floating bar now converts IINA's bottom-origin vertical preference
and AppKit drag rectangle into top-origin layout coordinates. The default
bottom inset uses IINA's 67-point controller clamp plus its 25-point top margin;
the drag range keeps the bar 25 points from the video area's top while permitting
it to touch the bottom. Horizontal dragging matches IINA's 10-point left inset
and permits the right edge to reach the video boundary. Formatting passed. The
full workspace build passed. No GUI positioning
test was run: Windows Graphics Capture is still failing to create a monitor
capture item (`0x80070057`), and the available prior screenshots do not show the
new build's position.

## Saved-filter menu shortcut alignment · 2026-10-15

Saved Video and Audio Filter entries now set Slint's typed `shortcut` property
from their stored key and modifier bits on both menu implementations. Labels no
longer contain manual spaces. `cargo fmt --all -- --check` passed; the workspace
build passed. No live native-menu capture was taken, so right-column layout
and accelerator dispatch remain unverified.

## Fixed-screen black patch move recheck · 2026-10-15

Launched the rebuilt `riina.exe` against an isolated copy of `moving-square.avi`
so folder autoplay could not replace the fixture. A native host move of 120×80
screen pixels moved the visible controls window by exactly 120×80 pixels; the
host-to-controls offset remained 18×143 pixels. The other 36×36 Riina-owned
window at `(0,0)` is Winit's documented layered, transparent event-target
helper. Windows Graphics Capture again failed with `CreateForMonitor`
(`0x80070057`), and the full-desktop GDI capture was uniformly black, so the
black patch itself could not be visually located or attributed. NVIDIA Overlay
processes were running, which is only a lead and not proof of cause. Closed the
exact test process and confirmed no Riina process remained. No renderer change
was made without a usable capture of the reported patch.

## REF-24 image-adjustment slider geometry · 2026-10-15

Native-resolution crops of the pinned 2× reference and the saved 3× Windows
capture measure the equalizer cards at approximately 327.5 and 328 points wide.
The label origins differ by less than one point, while the first slider begins
about 65 points after its label origin in IINA and 76 points after it in Riina.
Changed the Riina label column to 56 points (plus the source 8-point row gap) and
reduced the reset-action slot from 20 to 16 points. This source-derived change
moves the slider start about nine points left and the reset center about two
points outward. `cargo fmt --all -- --check` and `cargo build --workspace
--locked` pass. The rebuilt UI Automation tree exposed the Video pane's upper
controls, but not the lower image-adjustment rows while they were below the
viewport. Windows Graphics Capture failed with `CreateForMonitor` (`0x80070057`)
and the GDI desktop capture was uniformly black, so rendered post-change bounds
remain unverified. The exact Riina test process was closed.

## REF-24 Speed and image-adjustment bounds · 2026-10-15

The rebuilt native-video player was opened with an isolated moving-square clip.
At 300% scale, UI Automation measured the Speed slider at 738×60 pixels
(246×20 pt), matching the 246-point reference track. Its input text region
remained 42 points inside the 50-point input slot. The lower Video pane measured
the image-adjustment label column at 56 pt, label-to-slider offset at 64 pt,
slider/reset row bounds at 14 pt and pitch at 26 pt, and each reset hit target at
16 pt. The exact test process was closed and no Riina process remained. `cargo
fmt --all -- --check` and `cargo build --workspace --locked` pass. Windows
Graphics Capture failed with `CreateForMonitor` (`0x80070057`) and full-desktop
GDI capture returned uniform black; no visible-pixel or material claim is made.

## REF-21 Layout scrollbar policy · 2026-10-15

The pinned Layout reference has no scrollbar at rest, while the saved Riina
capture shows a persistent right-edge thumb. The Layout-specific `ScrollView`
was missing `vertical-scrollbar-policy: always-off`; added it to match IINA's
autohiding reference while keeping wheel scrolling available. Formatting and
`cargo build -p riina --locked` pass. The rebuilt isolated-pattern app opened
Quick Settings > Layout in UI Automation; Layout controls remained available
and wheel input exposed the lower Settings control. The remaining Sidebar
Position rows were not confirmed through the current UIA tree. Both full-desktop
GDI capture and `PrintWindow` of the controls HWND produced black output, and
Windows Graphics Capture is unavailable (`CreateForMonitor`, `0x80070057`), so
the idle scrollbar appearance and complete bottom-of-pane reach remain unverified.
The exact test process was closed and no Riina process remains.

## REF-21 Layout content-derived scroll range · 2026-10-15

Replaced the hard-coded 760-point Layout `ScrollView` extent with
`max(visible-height, layout-content.preferred-height)`, so its viewport follows
the rendered card and section heights. `cargo fmt --all -- --check` and
`cargo build -p riina --locked` pass. The attempted GUI check was invalidated by
a topmost `ScreenClippingHost` window owned by Windows (`3840×2160`, process
672) that covered the player and desktop capture. No visual or scrolling claim
is made from that run. Closed the exact test Riina process; left the Windows
snipping interface untouched.

To preserve IINA's autohide behavior while using the Windows native scrollbar,
the Layout scroll view now exposes its `as-needed` indicator after a scroll event
and hides it 850 ms after the last event; repeated events restart the timer. The
sidebar pane change also clears the temporary visibility state. The locked
application build passes. The indicator's appearance and timing remain
unverified because the full-screen Windows snipping overlay is still present.

## REF-21 Fit Window to Video state · 2026-10-15

Compared the Layout row predicate with IINA's pinned `SidebarLayoutPane`:
`removeBlackBarBtn` is visible only when window-aspect ratio is unlocked. Riina
now uses that preference for the Fit Window to Video action; the Docked one-row
card is 44 points, Edge-to-Edge's two-row card is 76 points, and the unlocked
32-point action row increases either card accordingly. `cargo fmt --all --
--check` and `cargo build -p riina --locked` pass. Runtime verification of the
Edge-to-Edge, Docked, locked, and unlocked states remains pending because the
full-screen Snipping overlay blocks clean visual inspection.

## REF-21 OSC bounds around open sidebars · 2026-10-15

Source review of the saved 3840×2160 Layout capture found the floating OSC
underlapping the open trailing sidebar. In Edge-to-Edge mode, the OSC now uses
the video width after subtracting visible sidebars; the underlying video keeps
its full edge-to-edge dimensions. `cargo fmt --all -- --check` and
`cargo build --workspace --locked` pass. Live control-bar placement and
sidebar bounds are verified by UI Automation; rendered alignment and pointer
hit-testing need a clean desktop capture with the Quick Settings panel open.
The saved 3840×2160/300% capture estimates a 915-point player window and
360-point trailing sidebar, leaving a 555-point OSC span. A centered 460-point
bar moves from roughly x=228 to x=48, removing an estimated 132-point overlap;
an isolated-fixture UI Automation run on the rebuilt binary confirms the last
OSC action ends at x=2033 px and the first Video-panel row begins at x=2264 px,
leaving 231 physical pixels (77 points) clear. The exact test process closed
normally and no Riina process remained. `tests/visual/riina-osc-sidebar-geometry-evidence.json`
records the binary, fixture, bounds, and limitations. This verifies control
geometry only; further UIA states verified a leading Layout sidebar, Playlist
leading with Quick Settings trailing, and Plugins leading with Quick Settings
trailing. With both sides occupied, optional toolbar actions hide instead of
overlapping the panel while center transport buttons remain in the video span.
Both test processes exited normally. The topmost snipping overlay still blocks
visual/material capture and drag-fraction verification.
The same test measured Quick Settings tab hit rectangles: Subtitles ends at
x=3311 while the top-level window ends at x=3293, but UIA reports it on-screen
and its Invoke action opens the Subtitles panel. The older saved full-desktop
image shows the glyph inside the frame. This is an unresolved UIA/rendered-edge
coordinate discrepancy, not proof of clipping; keep the measured 192-point
capsule pending a fresh pixel capture rather than shrinking it speculatively.

## REF-21 On Screen Controller tile geometry · 2026-10-15

The selected tile in the native-resolution pinned screenshot measures 132×148
px at 2×, or 66×74 points; adjacent tiles are 16 px (8 points) apart, so the
row spans 214 points. `SidebarLayoutPane.swift` uses the default 8-point gap from
`UIHelper.hStack`. Riina's prior row was 216 points with 65.33-point tiles and
10-point gaps. It now uses 214/8/66-point geometry. The incremental `cargo build
--workspace --locked` passed. The old screenshot's overlapped OSC predates the
controller/sidebar bounds fix and must not be used to assess current overlap.
The source also uses a 40×28-point image view, an 8-point vertical stack gap,
and 12-point item padding. In the pinned crop, the visible preview frame is about
32.5×25 points. Riina now reserves that 40×28 viewport and draws an inset 32×25
frame with the source-defined 8-point label gap. `cargo fmt --all -- --check`
passed. The rebuilt app started and restored a prior `moving-square.avi` session,
but Computer Use failed twice during window capture with
`IGraphicsCaptureItemInterop.CreateForMonitor` (`0x80070057`); no screenshot or
post-change UIA measurements were obtained. The exact Riina process was closed
and confirmed absent. Tile hitbox geometry and the stationary-black-patch playback
observation remain visually unverified by this run. A 3840×2160 full-desktop
baseline captured after exit also returned black at all 25 probe points, so it
cannot establish whether the reported patch persists without Riina.

`OSCLayoutSelector.Item` also specifies an 8-point corner radius, a 2-point
selected outline, and a 1-point outline for inactive tiles. Riina's inactive
tiles previously had no outline and used 7-point corners. The component now uses
the measured radii and widths with a low-contrast theme-aware inactive stroke;
`cargo fmt --all -- --check` and the incremental workspace build passed. A fresh
capture is pending.

`createOSCSettingsView()` adds 8 points below the OSC tiles, then the outer
sidebar stack adds its 18-point section spacing before the Sidebar Position
heading. Riina previously applied only the 18-point gap. The Layout pane now
reserves the additional 8 points without changing the other section gaps.
Formatting and the locked workspace build passed. The desktop capture path still
returns an all-black baseline, so the rebuilt 26-point rendered gap is not yet
visually confirmed.

The same matched-scale crops measured the section-title glyphs at about 19 px in
IINA and 16 px in the prior Riina capture. The pinned `createSectionTitle` uses
a bold 12-point system font; Riina used 11 points. Its `SidebarSectionTitle`
caption now uses 12 points. The locked workspace build and formatting check
passed with the final layout changes; rendered glyph bounds are pending.
The source also pads the Sidebar Position container 8 points below its card.
Riina keeps that inset in a transparent wrapper below the source-derived
spacious/compact material card heights, extending the scroll range by 8 points.
The scroll-end pixels remain unverified.

## REF-21 Sidebar Position row geometry · 2026-10-15

The source uses `SidebarPosSwitch` rows in a vertical stack with 10/6-point
spacious/compact spacing and 12/10-point container padding. Riina now uses
24-point rows and source-derived card heights of 118/106 points, replacing its
30-point rows, 2-point gaps, 8-point padding, and fixed 114-point height.
`cargo fmt --all -- --check` and `cargo build --workspace --locked` pass. The
GUI launch did not use the documented `-- <media>` argument separator, and the
resulting window title reported `track-languages.mkv`; fixture playback was not
verified. Computer Use then failed twice with `CreateForMonitor` (`0x80070057`),
and the full-desktop fallback returned black at all 25 probes. `ScreenClippingHost`
PID 672 was present; it was left untouched. No rendered geometry is claimed. The
exact test process closed normally and no Riina process remained.
