# Development

Run commands from the repository root. Read [AGENTS.md](../AGENTS.md) before
changing behavior; it contains the working, verification and publishing rules.

## Windows setup

Requirements: Windows 11 x64, Rust 1.99.0 (edition 2024), MSVC C++ tools,
Windows SDK and 7-Zip. Versions are pinned in `rust-toolchain.toml`,
`Cargo.toml` and `Cargo.lock`.

```powershell
./scripts/fetch-reference.ps1
./scripts/fetch-mpv.ps1
cargo build --workspace --locked
cargo run -p riina --locked -- "C:\media\movie.mkv"
```

The reference fetch verifies IINA v1.5.0 against
[`reference/upstream-lock.json`](../reference/upstream-lock.json).
The mpv fetch supplies a pinned **development** DLL under `tools/mpv/`;
release packages use the separately identified release engine.

If this checkout uses workspace-local Rust/MSVC tools, use the development
wrapper. Pass an explicit argument array so PowerShell preserves `--`:

```powershell
./scripts/dev.ps1 -CargoArguments @('build', '--workspace', '--locked')
./scripts/dev.ps1 -CargoArguments @('run', '-p', 'riina', '--locked', '--', 'C:\media\movie.mkv')
```

Windows defaults to native D3D11 video. Add `--shared-gl` after `--` to inspect
the shared SDR renderer; `--native-video` explicitly selects the default route.
For macOS/Linux setup, see [port development](port-development.md).

## Repository map

| Path | Responsibility |
|---|---|
| `crates/core/` | Portable commands, state, preferences, catalogs and policies |
| `crates/mpv/` | libmpv FFI, worker, observations and rendering integration |
| `crates/app/src/` | Application orchestration and UI adapters |
| `crates/app/src/preferences/` | Preference loading, persistence, section adapters and settings window |
| `crates/app/src/plugins/` | Plugin installation, execution, permissions and event handling |
| `crates/app/src/subtitles/` | Subtitle discovery, provider adapters and credentials |
| `crates/app/src/player/` | Player composition, input, playback actions and presentation state |
| `crates/app/src/windows/` | Auxiliary windows, including About, history, filters and pickers |
| `crates/app/src/platform/` | OS services, native windows and presentation |
| `crates/ui/ui/` | Slint components and design tokens |
| `crates/test-support/` | Test-only temporary directories, fixture paths and snapshot waits |
| `crates/xtask/` | Local verification, focused native tests and coverage reports |
| `scripts/` | Dependency setup, checks, packaging and capture tools |
| `tests/fixtures/` | Original synthetic media and provenance |
| `tests/visual/` | Capture manifests and presentation evidence |
| `reference/` | Pinned source locks and ignored local reference material |
| `tools/`, `target/` | Local dependencies and generated build output |

The core crate keeps its public types re-exported from `lib.rs`. Their implementations
live in `command/` (intent, argument validation and plans), `playback.rs`,
`media_catalog.rs`, `captured_frame.rs`, `loop_mode.rs` and `preferences.rs`.
Cross-feature regressions are grouped under `tests/` by commands, playback and
preferences. `crates/mpv/src/lib.rs` owns player handles and command senders.
`worker.rs` owns the native core. `properties.rs` and `auto_seek.rs` handle
native observations and per-file Auto seek timing.
The `player_ui/` modules own playback observations, media catalog models, native
material sampling, plugin-window integration and audio/video/subtitle controls.
The root still owns session resources and their teardown order. Plugin callbacks
retain command senders rather than playback owners.
Player input/focus, lifecycle and system-media adapters, OSD/controller feedback,
LiveText capture, media discovery and GL rendering now have their own modules.
Both presentation paths construct `UiSession` in one place. Automatic resume is
scoped to the generation the window paused; LiveText serializes frame captures
and validates frame position, seek revision and capture revision before applying
recognition. Discovery readiness uses per-request identities, so a superseded scan
cannot replace a newer scan's completion state.
Native frame capture acknowledges rejected and failed requests separately from
successful pixels, allowing the serialized LiveText capture flow to retry.
The playlist action module owns row selection, reordering and context actions.
Window sizing keeps the media generation in Rust and Slint track callbacks
send typed selection categories with string IDs.

`key_binding_preferences.rs` exposes the shared profile interface. Its modules own
captured defaults/loading, key identity conversion, the compiled effective index,
action dispatch, menu accelerators and editor operations. Native Slint key scalars
are converted at the event boundary. Duplicate keys retain their first menu order
and last action; malformed modifiers do not become bare-key overrides.
`plugin_preferences.rs` owns the window coordinator; inventory, update/recovery
and permission persistence are separate. All grant mutations use the same stable
sidecar file lock before reading and merging the latest JSON, including rollback.

`preferences_window.rs` owns the auxiliary window and its platform dialogs/timers.
Its private modules handle edit validation/commit, key binding actions, live decoder
updates and refresh. `screenshot_preferences.rs` holds screenshot intent only.
Music Mode preferences are persisted before their live callbacks run. OSD queue
ownership stays in `video_osd.rs`; native change detection lives in `observations.rs`.
Disabling a feedback category removes that category's queued messages.

`plugin_files.rs` owns file capabilities and containment. Its `context` module owns
player snapshots/event diffs; `encoding` owns strict text decoding. Ordinary storage
identifiers retain their directory names. Long, uppercase, reserved and otherwise
nonportable names use a SHA-256 key prefixed with `~`, preventing truncation,
replacement and case aliases from merging private storage. Existing ambiguous
legacy directories are not migrated: their ownership cannot be determined safely.

`plugin_runtime.rs` owns session startup, command admission and cancellation. Its
private modules separate the worker loop, execution budget, package loading,
timers, permission review, bounded output, messages and individual host APIs.
Every JavaScript turn includes its Promise jobs under the same execution budget.
Shutdown cancels executing JavaScript before waking and joining the worker;
network workers retain their concurrency slot until result delivery completes.
Ready timers and host commands alternate when both are pending, so continuous
player events and zero-delay timer chains cannot starve one another.

`playback_history.rs` owns shared history and persistence decisions independently
of plugins. `plugin_snapshot.rs` converts playback and owned history entries into
the plugin API view. History and recent-media locks are never held together;
history actions release their lock before publishing a snapshot. Selection removal
persists one batch before changing the in-memory rows and revision.

## Checks

Use `cargo xtask check --fast` for a focused batch. It checks formatting and
source line lengths, then lints and tests the crates with staged, unstaged or
untracked changes. Slint and Settings search changes check navigation. UI
source changes alone run the player interaction and visual tests together.
Linting uses the same packages and includes their libraries and those tests.
When UI and other Rust code change in one batch, the selected crates also
include the application. Edits to visual tests or their baselines stay scoped
to the UI crate. Script and workflow changes run the Python suite. Shared build
inputs, test helpers and media fixtures select workspace-wide linting and tests.
A clean checkout runs the portable core tests. Documentation-only batches stop
after the source checks.
Use `cargo xtask check --fast --base <revision>` to include a batch of local
commits in the same check. Pass the commit immediately before the batch. The
check also includes staged, unstaged and untracked changes.
Use `cargo xtask check` at integration points for the full local gate, including
the Python, parity, fixture and asset checks. Run `cargo xtask native` for the native
engine suite with null audio output. It selects the native test target and reuses
the harness built by the workspace gate. Tests in `crates/mpv/tests/native/` are
grouped by feature. Run `cargo xtask native subtitles::` for the subtitle cases
or substitute another module prefix. On Windows it uses the pinned development
DLL from the setup above unless `RIINA_TEST_MPV_LIBRARY` points to another
provisioned build. Windows runs up to four native tests at a time and other
platforms run them serially by default. `RUST_TEST_THREADS=1` forces a serial
run anywhere. Failed tests show their captured output.

Native capture tests use disposable directories by default. To keep files for
the pixel and format checkers, set `RIINA_TEST_ARTIFACT_DIR` to an existing
absolute directory and run the relevant native group. Each capture test creates
a dedicated subdirectory and refuses to reuse an earlier capture directory.
Use a fresh parent directory for another run.

With the checkout-local Windows toolchain, run
`./scripts/dev.ps1 -CargoArguments @('xtask', 'check', '--fast')`. Replace the
argument array with `@('xtask', 'check')` for the full gate or
`@('xtask', 'native')` for native tests. Pass a third argument such as
`'subtitles::'` to select one native group.

Run `cargo xtask ui` or `./scripts/dev.ps1 -CargoArguments @('xtask', 'ui')`
for Slint changes. It checks Settings navigation and runs the headless widget
tests and Windows pixel comparisons in one Cargo invocation.
For a palette or layout adjustment, the Windows-only
`cargo test -p riina-ui --test headless_visual --locked` compares the committed
pixels without compiling the player application or provisioning libmpv.

`cargo xtask coverage` is optional. If `cargo-llvm-cov` and the pinned
toolchain's `llvm-tools` component are installed, it runs the workspace tests
and reports coverage by crate plus the ten largest uncovered function bodies.
It writes the full JSON report under `target/riina-coverage/` (or
`$CARGO_TARGET_DIR/riina-coverage/` when configured).

[Verification](verification.md#running-checks) contains the commands, native
prerequisites, GUI procedure and the latest scoped results. Keep implementation
and verification states separate in the requirement inventories.

Clippy enforces the 150-line function and seven-argument limits, documented
unsafe blocks, and explicit error handling in application code. Existing
exceptions are marked on the relevant item with a reason. The generated
module in `riina-ui` expects the unwrap and panic lints from the generator.
The Rust source line-length exception list shrinks as lines are reformatted
or removed.

Quick Settings input, category navigation and narrow-header geometry regressions
in `crates/app/tests/quick_settings_input.rs` run with the workspace tests. They exercise the generated player controls through
Slint's headless test backend, including pointer/key input and focus changes.
The generated UI crate skips its empty unit-test harness, so the workspace suite
compiles the generated widgets only for their consumers.
Debug builds retain the Slint lookup metadata those tests need. Release-profile
widget tests require `SLINT_EMIT_DEBUG_INFO=1`; normal distribution builds omit
that metadata. Native window/input and physical-display checks remain separate.
On Windows, `cargo test -p riina-ui --test headless_visual --locked` compares
decoded RGBA pixels with the PNGs under `tests/visual/headless/windows/`.
The command compiles the generated UI and software renderer without building
the player executable or the libmpv adapter.
The software renderer uses a fixed clock and dimensions. The corpus covers
playing, paused and stopped states, both player themes, all sidebar panes,
compact and docked controls, Music Mode, on-screen displays, crop geometry,
Live Text, all 11 light Settings pages, and their search and modal overlays.
The test runs on Windows to keep system-font rasterization consistent.
For a pixel mismatch, set `RIINA_HEADLESS_MISMATCH_DIR` to an output directory
and rerun the test. It saves the actual PNG there and still fails the comparison.
Set `RIINA_UPDATE_HEADLESS_BASELINES=1` only when intentionally regenerating
the corpus, then rerun without it and inspect the affected frames. These
are Riina rendering regressions. Native composition and matched IINA
captures have separate verification.

The app also runs its real player callbacks with a bounded recording engine
in an isolated Slint test process. Run that headless regression directly with
`cargo test -p riina --lib --locked player_callbacks_and_playback_projection_work_headlessly`.
The development Cargo profile retains source line information for workspace
code and minimizes dependency debug information. A fresh dependency build can
take longer because dependencies are optimized.

Run `cargo test -p riina-mpv --test snapshot_benchmark --locked -- --ignored --nocapture`
to measure snapshot publication and UI retention without launching a player.
The benchmark publishes 10,000 clock updates against 30 tracks, 500 playlist
entries and 256 retained events. On the Windows development profile, replacing
deep copies with shared snapshots and catalogs reduced allocations from
27,000,000 to 20,000 and elapsed time from 1,785.792 ms to 4.769 ms. This measures
the handoff with unchanged collections. Decoder, plugin and rendering work is
measured separately.

Track-list regressions live in `crates/app/tests/quick_settings/track_navigation.rs`
and run through that same generated-player test target. They exercise Audio,
Video and both subtitle lists with delayed observations, model replacement,
keyboard traversal, scrolling, disabled states and narrow language badges.

`quick_settings/equalizer.rs` covers menu traversal, naming validation,
submission/cancellation and focus restoration. `quick_settings/workflows.rs`
checks neutral auxiliary-window acrylic colors, independent page scrolling,
playlist row/footer bounds, quiet paused
Music Mode transitions and first-pointer slider/color input. Transport tests
measure the original triangle's filled-area center as well as its button frame.
Wait for Flickable's overscroll clamp before comparing restored positions; an
intermediate elastic offset is not a stable page position.

The read-only installed-font diagnostic remains available separately from the
player. It reports counts and discovery timing without listing private font paths:

```powershell
cargo run -p riina --bin font-catalog-probe --locked
```

## Repeating timers

The interval is part of the behavior for playback presentation and hover
feedback. Keep it when narrowing a timer's lifetime. The last column records
when each repeating timer or one-shot delay is needed.

Pause on minimize follows the owning window's minimize and restore events.

| Owner | Interval | Work | When needed |
|---|---:|---|---|
| Platform material monitor | 500 ms | Refresh native acrylic policy | While registered auxiliary surfaces are alive |
| Video and audio filter window | 300 ms | Refresh live filter rows | Starts after show, stops on close or when a tick finds the window hidden |
| Playback History window | 500 ms | Refresh visible history and grouping | Starts after show, stops on close or when a tick finds the window hidden |
| Log Viewer | 500 ms | Read and append new log records | Starts after show, stops on close or when a tick finds the window hidden |
| Font Picker | 250 ms | Follow the owner's appearance | While Font Picker is visible. Font discovery delivers results when ready |
| About window | 250 ms | Follow the owner's theme and read pending libmpv and FFmpeg versions | Starts after show, stops on close or when a tick finds the window hidden. Version reads stop when both labels are available |
| Online subtitles | 500 ms | Check the automatic subtitle search trigger | While automatic lookup is enabled and media is loaded. Each tick checks media eligibility |
| Player appearance | 500 ms | Apply palette changes and refresh the system reduced-motion policy | While a player window is owned. Explicit themes still follow the system motion policy |
| Settings motion | 500 ms | Refresh the reduced-motion flag | Starts after show, stops on close or when a tick finds Settings hidden |
| Settings audio devices | 750 ms | Read the observed audio device list | Starts after show, stops on close or when a tick finds Settings hidden |
| Settings recovery notice | 250 ms | Wait for the player to show the recovery notice | Until the first visible player status line |
| Fullscreen clock and battery | 60 s | Update local time and battery capacity | While fullscreen controls and the information card are visible |
| Native HDR availability | 2 s | Refresh the display HDR state | While the Windows framed owner is visible and restored |
| macOS picture-in-picture | 100 ms | Sync AVKit state and update the active indicator | After the player creates a PiP controller, until the session ends |
| Timeline thumbnails | 80 ms | Drain bounded batches of thumbnail frames | One initial pass and a pass on media or settings changes. The timer repeats while extraction runs. Pointer callbacks update hover previews directly |
| Native video transition | 80 ms | Capture outgoing and incoming frames and settle the handoff | While a playlist request waits for its frame, playback approaches a pending outgoing capture, or the incoming surface is settling |
| Windows startup material policy | 500 ms | Reapply native backdrop settings | While the framed owner is visible and restored |
| Windows surface alignment | 16 ms | Align owner, native video and controls | While the framed owner is visible and restored, including mpv child-window handoffs |
| Playlist metadata | 300 ms | Refresh visible playlist rows | One delayed pass after a lookup completes or the display policy changes |
| Automatic Music Mode | 100 ms | Complete the track-catalog debounce | While media is loaded and automatic switching is enabled, including paused playback |
| Subtitle discovery | 120 ms | Discover sibling media and select a matching subtitle track | One delayed pass at setup and after relevant playback or discovery settings change |
| Windows picture-in-picture | 250 ms | Check automatic enter/exit on minimize or restore and refresh active preview | While PiP is active or the automatic minimize policy applies. Stops at shutdown |
| macOS Now Playing | 50 ms | Sync media controls and drain system actions | During the player session. Stops before clearing the closing player's media state |
| Windows package smoke | 100 ms | Advance the package verification steps | For the lifetime of the smoke run |
| Live Text | 300 ms | Request and project paused-frame recognition | While recognition is enabled and paused video is eligible, with one cleanup tick after eligibility ends |
| Windows player material policy | 100 ms | Follow system material policy and accent colors | While the player displays Windows materials or controls |
| Windows video material sampling | 16 ms | Capture and transform a video sample | While a visible material uses active native video, with one cleanup tick to clear retired samples |
| Plugin UI | 25 ms active, 125 ms idle | Drain plugin messages, update presenters and sample window properties | The host wakes the fast tick for new work. With no active sessions, retain the window-property tick for plugins enabled later |

The Windows owner monitors native show-state changes and resumes surface
alignment, material policy, and HDR polling at their existing intervals. Winit
still receives its move and size notifications. If the native monitor cannot
attach, the owner logs the failure and retains polling.
[Win32 window-position notification](https://learn.microsoft.com/windows/win32/winmsg/wm-windowposchanged)

The Slint controls also own short presentation delays. Their bindings start
them for the corresponding interaction and stop them after it settles.

| Control | Delay | Work |
|---|---:|---|
| Equalizer profiles | 16 ms | Show the name dialog after the popup releases focus |
| Font Picker | 16 ms | Reveal selected rows once the visible lists have a layout |
| Font and track lists | 32 ms | Hand accessibility focus to a row after its tree is rebuilt |
| Layout sidebar scrollbar | 850 ms | Hide the scrollbar after scrolling |
| Sidebar frame | 260 ms | End the close transition |
| Quick Settings scroll page | 1 ms | Restore a page's scroll position after switching |
| Video controls | 300 ms | Finish a programmatic scroll |
| Timeline slider | 750 ms | Release the drag value when the native seek reply is late |

## GUI sessions

Use an isolated profile and a single short fixture. Confirm the filename,
playback state and selected panel before capture. After each test, close that
Riina instance and confirm it exited before rebuilding or testing another build.
If necessary, terminate only the exact process started for that test.

For flashing, black patches, clipping or composition reports, capture the full
virtual desktop with `scripts/capture-windows-desktop.ps1`. Check its dimensions
and non-black probes, then inspect the full image. Compare before launch and after
exit for a stationary artifact. Window-only captures can supplement control
inspection. Keep unrelated private desktop content out of published evidence.

## Build storage

Keep incremental compilation enabled. Review `target/debug` during repeated
builds, but clean generated artifacts only above **50 GB**, after confirming no
build, check or application is using them. Prefer the specific stale package or
cache. Preserve logs, release evidence and reference files; do not routinely
erase shared Cargo downloads. Keep references outside `target/`.

## Release candidates

1. Choose the exact source revision and version; use the configured GitHub account.
2. Reuse a successful native artifact only if source revisions, patches, scripts,
   architecture and provenance match the candidate's build identity.
   If its CI artifact has expired, the release workflow's `native_release_tag`
   input can restore the engine from an existing published package. It verifies
   the archive checksum, tag/source identity, unchanged `tools/mpv` tree, runtime
   inventory and native notices. It does not reuse the previous application.
3. Run portable packaging tests, assemble the archive, and verify runtime contents,
   notices, hashes and source/build manifests.
4. Extract the archive and run the actual executable through
   `scripts/smoke-windows-package.ps1`. Keep native engine checks as well.
   Build the installer from that same payload with pinned Inno Setup 6.7.3;
   `smoke-windows-installer.ps1` verifies install/reinstall, all payload hashes,
   Start menu and uninstall registration, installed playback and uninstall.
   The installer test refuses to replace an existing installed Riina.
5. Record physical-display, audible-output and HDR checks separately. Publish only
   the verified candidate when a public milestone or explicit request authorizes it.

Use the release workflow's manual dispatch for a candidate. Set
`build_release_candidate=true` and select one native reuse source. Leave
`publish_release=false` when the archive needs a desktop GUI check first; publish
those exact verified assets afterward. `publish_release=true` publishes after the
same run's package checks pass. Creating the version tag does not start another
binary build.

The installer definition is `packaging/windows/riina.iss`. It installs only for
the current user under `%LOCALAPPDATA%\Programs\Riina`, requires Windows 11 x64,
and uses a stable application ID for subsequent upgrades. The default desktop
shortcut is unchecked. No default file association or recursive profile removal
is performed. Every portable payload file, including notices and source-access
records, is installed unchanged. Inno Setup's own generated uninstaller files
are additional installation metadata. Compiler downloads are checksum and
publisher-signature checked by `prepare-windows-installer.ps1`.

### Retest a saved package without rebuilding

If compilation and native checks succeeded but the GUI environment failed, use
**Verify saved Windows package** with the original `candidate_run_id`. It requires
the original release workflow's build/lint/native gates to have passed, checks the
artifact's repository and source identity, downloads the saved ZIP, and verifies
its checksum, runtime, notices and exact corresponding source before running it.
When an installer is present, verification also requires its original build gate
and rechecks install/reinstall, shortcuts, payload identity, installed playback
and uninstall. Successful verification uploads the original ZIP, installer and
checksums with fresh reports; it does not compile or publish anything.

Hosted Windows runners may expose only the system OpenGL 1.1 implementation.
Slint's FemtoVG interface needs a newer OpenGL context even though mpv presents
video with D3D11. Both release workflows stage checksum-pinned Mesa 26.0.3 x64
`opengl32.dll` and `libgallium_wgl.dll` beside the disposable test executable and
select `llvmpipe` for that smoke step. The release ZIP is unchanged. This is a
software-interface test, not physical GPU, compositor or HDR certification.
Normal desktop verification uses the original extracted package and its driver.

The test-only driver comes from the
[Mesa Windows distribution](https://github.com/pal1000/mesa-dist-win/releases/tag/26.0.3).
Its archive hash is pinned in `scripts/prepare-windows-ci-graphics.py`; no system
driver installation or third-party deployment script is used.

The source and package license scopes are settled; see [licensing](licensing.md).
Use the immutable application tag, Cargo lockfile, native manifest and public
upstream sources for [source access](SOURCE-ACCESS.md). Do not duplicate native
or vendored Rust source trees in release assets.

## Resolved review findings

See the [hardening review status](hardening-status.md) for findings, local
commits, and open verification work.
