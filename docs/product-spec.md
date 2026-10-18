# Product and parity contract

Riina is an independent Rust reimplementation of **IINA v1.5.0**. Exact feature,
behavior and interface parity are acceptance goals. Windows 11 x64 is supported;
macOS/Linux application ports are active work and require native verification before
support is claimed. Read [status](status.md) for current evidence and [the
implementation plan](implementation-plan.md) for work order.

## Reference baseline

The pinned commit is `fbcff4df7183b335880396af0655f6bb89d46442` from the official IINA
v1.5.0 repository. Its identity is recorded in
[`reference/upstream-lock.json`](../reference/upstream-lock.json). Use
`scripts/fetch-reference.ps1`; retain the excluded checkout at
`reference/iina-upstream/`. Verify its remote, commit/tag and working tree before reuse.
Do not silently upgrade the baseline or discard reference changes.

Inspect ownership, lifecycle and subsystem boundaries for each feature. Reproduce
observable behavior through an independent implementation and documented platform
equivalents. Keep source-derived facts distinct from live observations and estimates. Do
not execute reference build scripts without inspecting them.

## Product scope

- Use Rust application logic, libmpv playback and a source-faithful Slint interface.
  Declarative UI, shaders, resources and build scripts support that implementation.
- Preserve IINA's defaults, transitions, settings, menus, shortcuts, auxiliary
  windows and plugin behavior. Document OS equivalents and unavoidable differences.
- Keep the elegant reference interface. Branding and platform adaptation must not
  become a redesign or reduce feature scope.
- Keep volume at 0-100% by default; the requested opt-in boost extends it to 200%.
  Do not add Vim controls, advertising, accounts, cloud synchronization, AI features
  or telemetry.
- JavaScript/WebViews are confined to IINA plugin compatibility, not the main UI.
  Do not substitute an Electron, Tauri/React, C# or Qt/QML application.
- Additional Windows versions/architectures and application ports require actual
  builds and playback/lifecycle evidence before support is claimed.

## Licensing and provenance

Original Riina contributions are MIT; the combined application package uses GPLv3 terms.
Follow [licensing](licensing.md) and retain dependency notices and exact [source
access](SOURCE-ACCESS.md). These scopes are settled.

Use the pinned reference for compatibility facts, not translated implementation or
copied protected assets. Audit fonts, artwork, plugins and native dependencies. Use
original or appropriately licensed assets, record hashes/provenance, and keep Apple
fonts, SF Symbols and restricted artwork out of the package. The GitHub contributor
record represents project contributions; follow [AGENTS.md](../AGENTS.md) for
attribution and publishing rules.

## Traceability and visual evidence

[parity.csv](parity.csv) records atomic requirements with source commit/file/symbol,
defaults, behavior, platform applicability, approach, implementation, dependencies,
tests, implementation/verification states, evidence and remaining differences. Classify
active, internal, obsolete and unreachable preferences. Add discoveries; do not drop
requirements or mark unverified work complete to improve a score.

[The upstream map](upstream-audit.md) guides source inspection. The [interface
checklist](interface-parity-plan.md), [visual specification](visual-spec.md) and
[capture ledger](visual-reference-audit.csv) cover the complete interface. Use actual
pinned captures, match media/frame, client size, scale, theme and panels, and compare
regions with explicit tolerances. Missing hardware/captures remain gaps. Do not invent
references or treat Windows regression goldens as proof of IINA fidelity.

## Architecture and rendering

The workspace uses pinned Rust, Slint and libmpv. Versions and features are recorded in
`Cargo.toml`/`Cargo.lock`; keep compiler/runtime dependencies compatible. Follow the
[repository map](development.md#repository-map) rather than adding empty crates. Keep
modules small, ownership explicit and unsafe invariants documented.

Windows defaults to native D3D11 presentation ([ADR
0002](adr/0002-native-d3d11-experiment.md)). The shared OpenGL route is SDR and remains
the provisional macOS/Linux application path ([ADR
0004](adr/0004-shared-gl-application-ports.md)). Native presentation ownership and
teardown follow [ADR 0003](adr/0003-native-presentation-boundary.md). The earlier OpenGL
feasibility experiment is historical, not a pending renderer choice.

Respect the pinned `client.h`, `render.h` and `render_gl.h` contracts. mpv's native
video outputs are distinct from its render API; do not invent a D3D/Vulkan/wgpu
render-context interface. Borrowed RGBA textures alone do not establish HDR output.
Verify source/target color, presentation format, display capability and SDR UI luminance
end to end. Change the integration with a documented working experiment if it
demonstrably prevents acceptance.

Keep domain tests independent of GUI/GPU/native-library setup. Test actual full
applications and native services separately. Never exclude the application from all
checks and describe library-only success as a supported port.

## Required behavior

The following sections define acceptance, not a claim of present completion. [The
requirement inventory](parity.csv) remains incomplete and must grow as the pinned source
reveals additional behavior.

## Playback ownership, events, and synchronization

Give each independent player session its own explicitly owned libmpv context. Separate
command/event processing from graphics work, following the actual render API's threading
contract. UI callbacks should enqueue intent; they should not perform synchronous
decoder or network work.

Observe playback state through libmpv properties and events. Do not invent a second
playback clock in the UI. Represent loading, playing, paused, buffering, seeking, ended,
stopped, and failed states distinctly, including unknown duration and non-seekable
sources.

Use generation identifiers to prevent stale events or thumbnail requests from an old
file updating a new session. Correlate asynchronous command results. Handle end-of-file
reasons, load failures, unavailable tracks, and renderer loss explicitly.

Use bounded queues with deliberate backpressure. Coalesce superseded seek/hover updates,
but do not silently discard stop, load, shutdown, or other consequential commands. Avoid
tight polling loops and redundant high-frequency UI updates.

Respect FFI string encodings, nullability, node ownership, buffer lifetimes, and
matching free functions. Treat foreign event storage as borrowed unless copied into
owned Rust values. Validate numeric conversions and non-finite values. Query the loaded
client API version and reject incompatible binaries with a helpful error.

Render and wakeup callbacks must obey the headers' restrictions. Keep them short, avoid
unsafe reentrancy, and marshal notifications to the owning event loop. Preserve the
graphics state expected by the GUI renderer. Clean up callbacks, GPU resources, render
contexts, and player handles in a tested order.

Do not read video frames back to the CPU on every playback frame. Thumbnails, OCR, and
screenshots are separate, bounded workflows. Hardware-decoding fallback must be
observable in diagnostics rather than hidden behind a misleading “hardware acceleration
enabled” preference.

## Playback and media parity

Treat the following as audit categories, not a substitute for the source-derived
inventory. Implement every confirmed upstream capability and add anything this list
misses.

### Opening and controlling media

Reproduce file and URL opening, multi-file opening, drag-and-drop, opening into an
existing or new player, recent items, and command-line startup behavior. Handle spaces,
Unicode, long paths, network shares, unavailable files, and URLs without lossy path
conversion.

Implement play/pause, stop, exact and relative seeking, source-configured seek
increments, frame stepping where supported, playback speed, mute, volume, chapter
navigation, file/playlist repeat, and A–B loops. Match what happens at boundaries, after
seeking while paused, and at the end of a file or playlist.

Support the media formats available through the selected libmpv build. Discover its
actual capabilities; do not promise every codec independently of how the binary was
built. Preserve external audio tracks, track selection, metadata, album art, and
audio-only behavior.

### Video settings

Port source-exposed aspect ratio controls, locked/unlocked window proportions, crop
presets and interactive crop behavior, rotation, flip/mirror, deinterlacing, video
adjustments, filters, and relevant advanced options. Route them through the appropriate
backend rather than visually faking changes in a disconnected UI.

Match screenshot semantics, including frame versus window content, subtitle inclusion,
clipboard/file destinations, naming, format, feedback, and preview behavior. Test color
and orientation, not just file creation.

Provide accurate playback diagnostics: source resolution, frame rate, decoder, selected
tracks, dropped frames, audio/video synchronization, and color/output information where
available. Unsupported properties must appear as unavailable, not as fabricated values.

### Audio settings

Reproduce audio-device enumeration and selection, language preferences, audio delay,
channel controls, equalization, volume limits, replay gain, gapless settings, and
pass-through options where present upstream and supported by the selected Windows
backend.

Handle device removal, Bluetooth changes, default-device changes, and recovery without
unexpected playback or misleading UI state. Preserve the separation between a user's
preferred device and the device currently available.

### Subtitles

Support embedded and external subtitles, source-defined automatic matching/search paths,
primary and secondary tracks, visibility, track switching, delays, and text encodings.
Test Unicode paths and subtitle text.

Use the playback engine's appropriate subtitle renderer for ASS/SSA styling and timing.
Do not replace rich subtitle rendering with a plain text label. Preserve applicable font
attachments, positioning, scaling, outlines, shadows, colors, style override levels,
margins, and letterbox behavior.

Reproduce online subtitle search and provider integration where the pinned release
exposes them, including any plugin-hosted providers. Handle authentication, limits,
failed searches, download selection, and unavailable services honestly. Do not fabricate
successful results or bundle someone else's API credentials.

## Playlist, history, thumbnails, and secondary modes

Implement playlist insertion, removal, reordering, sorting, shuffle and restore-order
behavior, next-item transitions, source-defined automatic neighboring-file discovery,
metadata display, and playlist import/export where supported.

Separate stable item identity from display index. Preserve ordering while asynchronous
metadata arrives. Keep large playlists responsive and cancel obsolete scanning work.
Match selection, keyboard navigation, context menus, and drag behavior.

Implement chapter lists and timeline markers with correct names and positions. Port
hover previews, time displays, thumbnail sizing, caching options, and remote-media
policies. A hover preview must not seek the main playback session or stall audio.

Run thumbnail generation with bounded concurrency and cancellation. Define cache keys,
maximum size, eviction, and invalidation. Avoid unlimited decoding jobs when the mouse
moves quickly across the timeline.

Reproduce history, recent items, resume positions, clearing history, and the upstream
privacy controls. Use transactional or atomic persistence and versioned migrations. Do
not block startup on unreachable recent network paths. Do not persist sensitive URL
tokens unnecessarily.

Reproduce music mode as its own faithful layout, including album art, metadata, compact
controls, optional playlist visibility, automatic switching, and return to the main
player. Do not substitute a generic mini-player with fewer controls.

Implement picture-in-picture as the closest Windows equivalent, preserving playback
continuity and source-defined parent-window behavior. Moving between main, mini-player,
fullscreen, and picture-in-picture must not create duplicate audio, reset position, or
accidentally replace the player session.

## Windows behavior, input, and accessibility

Provide proper native dragging, resizing, minimum sizes, maximize/minimize/restore,
snapping, fullscreen, always-on-top, focus handling, and window placement persistence.
Recreate the appearance while implementing genuine window behavior; do not merely paint
decorative titlebar buttons.

Test mixed-DPI monitors, monitor removal, display changes, sleep/wake, lock/unlock, task
switching, and fullscreen transitions. Keep windows recoverable when their previous
monitor is absent. Do not globally disable display or power settings.

Map macOS integrations deliberately: use Windows media controls for playback metadata
and media keys, Windows shell integration for opening and revealing files, appropriate
power requests for playback, and a Windows credential mechanism for secrets. Verify the
APIs available to a desktop Rust application rather than copying an API intended for a
different application model.

Preserve source shortcut semantics with documented Windows modifier substitutions. Do
not globally translate every Command key to Control without checking conflicts and
text-entry behavior. Port the keybinding editor, presets, conflict handling, and
mpv/IINA action mapping where present.

Route keyboard, mouse, wheel, touchpad gestures, menus, and system media keys through
the same command model. Match click, double-click, scrolling, seeking, and volume
behavior. Prevent shortcuts from firing while editing text or when a dialog owns focus.
Suppress duplicate media-key handling.

Treat Force Touch, Touch Bar, Apple Remote, and other hardware/platform-specific
features individually. Preserve the underlying actions through appropriate controls or
bindings. Document genuinely unavailable hardware interactions without claiming
identical support or deleting the requirement from the audit.

Implement keyboard focus order, visible focus, accessible names, roles, values, toggle
states, and live updates for custom controls. Test what Windows UI Automation and
Narrator actually expose. A polished canvas with inaccessible buttons is not complete.

Respect high contrast, reduced motion, and reduced transparency where applicable.
Implement usable text fields, selection, clipboard behavior, and IME input. Keep custom
UI legible at 100%, 125%, 150%, and 200% scale, including mixed-scale monitor
transitions.

## Settings, menus, localization, and advanced behavior

Port every active preference and its default, permissible values, dependencies,
persistence rules, and restart requirements. Reproduce the pinned release's Settings
organization and quick-settings layout rather than arranging controls into a new design.

Model preferences with typed values, validation, schema versions, migration tests, and
atomic updates. Keep live backend state synchronized with settings where appropriate.
Distinguish temporary per-file changes from persistent defaults.

Create a central action registry so menu items, keyboard shortcuts, buttons, plugins,
and accessibility actions invoke the same behavior and share enabled/checked state.
Preserve menu grouping, labels, contextual availability, and source ordering as closely
as the platform allows.

Cover the inspector, log viewer, history window, welcome/initial window, About view,
keybinding editor, plugin management, and other auxiliary surfaces discovered in source.
Do not reduce parity to the main playback window.

Support the upstream advanced mpv configuration model, custom options, user
configuration directories, filters, shader settings, logs, and scripting entry points
where present. Specify precedence between application-managed settings and user
configuration. Do not blindly load configuration from arbitrary media directories or
silently overwrite incompatible user settings.

Investigate Live Text behavior in the pinned release and implement the closest practical
Windows text-recognition interaction if confirmed. Keep recognition local by default,
cancellable, and outside playback/render threads. Preserve frame-coordinate mapping and
selection behavior. Record differences in recognition quality or OS services rather than
claiming Apple's implementation was ported.

Externalize user-facing strings. Preserve reusable, appropriately licensed upstream
translations with attribution where practical. Test English and Russian early, plus a
right-to-left and a long-string layout fixture. Maintain placeholders and plural rules.
Track missing translations rather than filling production UI with untranslated
identifiers.

## IINA plugin compatibility is part of parity

Read the pinned JavaScript plugin host, API implementations, manifests, bundled plugins,
and official type definitions. Pin any separately obtained definitions to an identified
revision and reconcile them with the v1.5.0 source; current documentation may describe a
different API version.

Build a plugin compatibility matrix covering package structure, installation, lifecycle,
permissions, global and per-player contexts, module loading, events, callbacks,
promises, logging, preferences, commands, mpv hooks, menu contributions, playlist
extensions, subtitles, file/network operations, utility execution, UI views, and
multiple players.

Preserve the observed public API names and semantics instead of inventing an unrelated
Rust extension API and calling that compatible. Internal implementation may differ;
externally visible behavior must be tested.

Choose an embeddable JavaScript engine only after checking the syntax and runtime
features required by actual plugins. A Rust binding to an existing engine is acceptable.
Do not assume that replacing JavaScriptCore with QuickJS or another engine automatically
preserves module behavior, event ordering, or host API semantics.

Where upstream plugins supply webview-based overlays, sidebar content, settings, or
standalone windows, implement the appropriate Windows webview integration, such as
WebView2 if suitable. Keep it confined to compatibility surfaces and load it only when
needed. Its composition with video must be included in renderer feasibility work.

Handle JavaScript failures, runaway work, plugin reloads, and shutdown safely. Use
explicit capability checks for filesystem, networking, launching executables, and
opening external URLs. Separate plugins' persistent data and credentials. Reject archive
traversal, unsafe extraction, and ambiguous package identifiers.

Do not claim that a worker process alone constitutes a complete security sandbox.
Document the trust model and enforce the permissions you advertise. Never auto-install
or execute an untrusted plugin as part of ordinary playback.

Write conformance fixtures for portable plugins and test representative upstream plugins
against both hosts where reference execution is available. Record passed API tests and
specific incompatibilities. Support for one trivial plugin is not evidence of complete
plugin parity.

Keep ordinary mpv scripts conceptually separate from IINA plugins. Audit the bundled
libmpv scripting capabilities and replicate the source's exposure of each rather than
conflating the two extension systems.

## Network, helper processes, and security

Reproduce URL playback, caching/buffering controls, proxies, user-agent behavior, and
online-media helper integration where present upstream. Do not equate support for direct
media URLs with support for every website.

Discover and pin helper executables through a documented source and integrity policy.
Launch helpers with argument arrays rather than shell-interpolated strings. Handle
cancellation, timeouts, output size limits, version mismatches, and useful error
reporting. Redact authorization headers, signed URL parameters, and secrets from logs.

Treat media, subtitles, playlists, plugin packages, and imported configuration as
untrusted inputs. Test malformed data and unexpected encodings. Disable implicit
execution where the upstream feature does not require it.

Load native libraries from explicit trusted installation paths using an appropriate
Windows DLL-search policy. Do not search the current media directory for DLLs or
automatically load an arbitrary binary from the working directory. Record every packaged
binary's source, build configuration, architecture, version, and hash.

Use user-consented installation for file associations and custom URI schemes. Do not
hijack the original `iina://` scheme or force default-app changes. Protect IPC and
single-instance handoff from malformed or unauthorized requests. Imported mpv options
and URI parameters must not become an unrestricted command-execution channel.

Keep local playback usable offline. Network services, update checks, and optional plugin
discovery must not block application startup.

## Testing and objective parity evidence

Build a fixture corpus from synthetic or appropriately licensed media. Include short
audio/video files, multiple audio tracks, chapters, external and embedded subtitles,
styled ASS, rotated video, unusual aspect ratios, variable frame rate, Unicode
filenames, malformed files, and long playlists. Record fixture provenance and generation
commands.

Add SDR and HDR fixtures for the relevant tests. Native HDR, pass-through audio, and
hardware-specific behavior require appropriate hardware validation; an ordinary
screenshot or headless CI job does not prove them.

Write unit tests for commands, state transitions, preferences, migration, playlist
order, resume logic, keybinding translation, URL/path validation, and parser behavior.
Use deterministic clocks and controlled events where useful.

Write real libmpv integration tests for opening, playback progression, pause, seek,
track changes, end-of-file, invalid input, and clean shutdown. Mark tests that require
the native library clearly. Missing DLLs should produce an explicit prerequisite failure
or reported skip, not a fake successful playback result.

Add Windows GUI smoke tests for opening a fixture, clicking real controls, entering
fullscreen, resizing, toggling sidebars, changing a setting, reopening the app, and
verifying persistence. Assert observable state through accessible controls or a tightly
scoped test-only interface.

Use controlled local HTTP fixtures for network failures, redirects, slow responses, and
interrupted streams. Do not make routine CI depend on a commercial streaming website
remaining available.

Implement reproducible visual captures for the initial window, playing/paused states,
visible/hidden controls, each major sidebar, light/dark mode, music mode,
picture-in-picture, settings, menus, and representative auxiliary windows. Use the same
media frame and explicitly matched layout state.

Store actual capture dimensions and scale. Compare normalized client areas using image
differences, region-specific metrics, and human review. Define tolerances before
accepting changes. Mask only documented unavoidable differences; never mask the entire
area that fails.

Separate three kinds of evidence: measured upstream references, Windows regression
goldens, and manually estimated designs. Passing against a Windows golden proves
stability, not fidelity to upstream. Do not regenerate reference images to make a
failing implementation pass.

Maintain a traceability check that verifies each completed parity row has real
implementation and verification evidence. Detect missing or orphaned requirements.
Report portable exact matches, adaptations, unverified work, and gaps separately; avoid
one misleading universal percentage.

## Performance and reliability targets

Make playback smooth without unnecessary UI work. Measure startup, local-media
time-to-first-frame, seek responsiveness, frame pacing, dropped frames, CPU/GPU
utilization, working set, and idle wakeups on a documented machine.

Use the same libmpv build and equivalent playback settings as a control when
investigating overhead. Compare like-for-like resolution, codec, decoder, display
refresh, power mode, and output path. Do not publish invented performance numbers.

As provisional engineering goals, target responsive first interaction, smooth 60 Hz UI
animations on suitable hardware, uninterrupted supported 1080p/4K playback, and
near-idle UI scheduling when paused or hidden. Set concrete regression thresholds once a
measured baseline exists, and record them with the benchmark configuration.

Do not run continuous expensive blur, thumbnail generation, media scanning, or UI
redraws when their results cannot be seen. Bound caches and background work. Preserve
playback responsiveness during large playlist operations and network failures.

Stress repeated file changes, seeks, fullscreen transitions, window
creation/destruction, minimize/restore, audio-device changes, and application shutdown.
Check for steadily increasing native handles, GPU resources, memory, and leftover helper
processes. Fix lifecycle defects rather than compensating with periodic restarts.

## Build and distribution acceptance

Provide pinned, repeatable Windows MSVC setup and portable packages with required native
libraries, version metadata, dependency inventory, notices and source/build identity.
Add installer/upgrade coverage without requiring administrator rights for ordinary
playback. Keep application state separate from installed files.

Respect installation consent and system policy. Pin tools/actions to verified
identities, document download authenticity limits, and load DLLs only through trusted
paths. Do not imply signing or a secure updater without a real verified release channel
and signing identity.

Every candidate must verify the exact package, run native engine checks and test the
extracted application, including normal shutdown and persisted settings. Native
artifacts may be reused only with matching source revisions, patches, scripts,
architecture and provenance. Do not duplicate native/vendor source trees in release
assets; use immutable source access as described in the release policy.

See [Development](development.md) for commands and packaging steps,
[Verification](verification.md) for scoped evidence, and [AGENTS.md](../AGENTS.md) for
local work, cache, test-session and publishing rules.

## Completion criteria

- Every active pinned requirement has implementation and appropriate verification
  evidence, with documented platform adaptations and hardware gaps.
- The full player, menus, sidebars, Preferences, plugin surfaces and auxiliary
  windows have matched-state visual and interaction comparisons.
- Native playback, accessibility, localization, performance, stress/lifecycle,
  packaging and hardware checks meet recorded acceptance thresholds.
- Release claims identify the exact revision, environment and remaining gaps.
  A polished candidate with known gaps remains a candidate with known gaps.

Foundation/playback feasibility is already established. Continue from current code and
evidence rather than restarting those milestones. The [implementation
plan](implementation-plan.md) owns the next tasks.
