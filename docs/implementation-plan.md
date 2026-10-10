# Implementation plan

Crop next: implement the full custom crop interaction (selection, confirmation
and cancel) and enable Custom... in the shared sidebar. Preset commands and
observed filter selection are now connected; current custom control is disabled.
Use the generation-bound crop_session::Session lifecycle for entry and exit;
wait for native pause and cleared crop before presenting the selection surface.
Retire the editor on replacement/Stop without restoring the old file's pause.
Entry clears the old crop; Cancel restores pause but does not restore that crop.
Next geometry work must map source pixels through display aspect/rotation and
letterboxing, then resize the video area on both presentation routes.
Resolve requested-preset identity where small source dimensions yield identical
geometry, and verify actual UI highlight and crop output with rendered evidence.
The portable preset/managed-filter foundation has native Windows coverage; add
mixed foreign-filter, replacement and hardware-decoding conflict
coverage before claiming the full crop feature. Keep original source pixels as
the calculation input and distinguish requested dimensions from chroma-aligned
native output. Verify the same behavior on current macOS/Linux app builds.
Presets under a 1:1 aspect override and all quarter turns now have native coverage,
as do an offset rectangle and whole-source confirmation clearing the managed crop.
UI coordinate conversion, mouse selection, confirmation/cancel and pixel proof
are still required; a backend rectangle command does not satisfy interactive mode.

macOS and Linux ports are immediate development priorities alongside Windows
interface parity. Do not postpone them until Windows reaches full IINA parity.
The next portability milestone is a full application build on both systems,
followed by local-file playback, shared controls, file selection, preferences,
fullscreen and clean shutdown on each. Core-only CI is a prerequisite, not this
milestone's exit condition.

Each new feature must keep shared behavior in the portable layers and isolate
native services behind the platform boundary. Record an explicit port gap when
a native implementation is pending; do not silently introduce Windows assumptions
into shared UI or playback logic. Keep the default 100% volume cap and opt-in
200% extension identical across targets.

1. Milestone 0: pin official source, inventory real files/defaults, establish workspace,
   reproducible dependency selection, license tracking, and executable checks.
2. Milestone 1 (current): prove libmpv OpenGL composition, local-file playback,
   working overlay input, resize/DPI, native accessibility and shutdown. Benchmark
   and assess an end-to-end HDR route before selecting a production renderer.
3. Daily workflows: playlist/chapter/track models, subtitle loading, persistence,
   history/resume, keybinding translation, source-matched light/dark and compact UI.
4. Advanced UI/platform: music/PiP, thumbnails, video/audio filters, screenshots,
   media controls, power/display lifecycle, localization and configuration precedence.
5. Plugins: inventory the pinned public JavaScript APIs, then choose an engine and
   build conformance tests plus narrowly scoped WebView2 compatibility surfaces.
6. Parity closure: full preference/menu denominator, reference captures, accessibility,
   hardware tests, stress/performance baselines, corresponding-source audit and packaging.

Do not claim milestone exits until their runtime evidence exists. Prioritize a bounded
HDR/native-window composition spike next if OpenGL presentation cannot support HDR.

Current daily-use increment: observed catalogs, stable-ID playlist editing, repeat
cycling, chapter activation and external audio/primary subtitles are implemented with
native tests. Do not expand every settings screen before closing the renderer gate.
Native-window/D3D11 feasibility now has an executable and observed scRGB/display
configuration, overlay input, fullscreen and maximize evidence (ADR 0002).
Original PQ/HLG fixtures, decoded metadata, target parameters and SDR/HDR desktop
queries now have evidence. Native replacement/stop/recreation and focused-overlay
keyboard navigation were exercised. Next: source-matched sidebars/controls,
native clipping/blur and narrow-window/DPI behavior on the D3D11 candidate, plus
compressed HDR/hardware decoding and calibrated color/performance evidence.
The existing GL path remains a provisional SDR feasibility renderer.

The native candidate now runs behind the shared player UI via `--native-video`.
Shared command ownership, native focus routing, actual HWND realization, fullscreen,
Stop/reopen and popup-focused shutdown have runtime evidence. Next renderer work:
video blur/composition, narrow-window and mixed-monitor DPI coverage, modal ownership,
compressed HDR/hardware decode and measured color/performance. Keep these gates open
while completing source-matched controls; transparent tint does not reproduce blur.

Current product direction: riina is a Rust IINA reimplementation. Windows 11 x64
is the current target; macOS and Linux ports are near-term work, not supported. Pixel-perfect
interface parity and full behavioral/feature parity with IINA v1.5.0 are primary
acceptance goals. Preserve its elegant interface; do not substitute a redesign.
The requested extension is opt-in volume boost up to 200%; the default volume
range remains 0–100%. Default behavior and appearance target exact IINA parity. No Vim
controls are requested. Historical wiina evidence remains under its original name.

Near-term port gate: compile/test riina-core on Windows, macOS and Linux; audit
libmpv discovery and GL lifecycle on each target; implement native dialog/storage/
window services behind the platform boundary; keep the same shared controls and
behavior tests. Port support requires actual builds and playback evidence.

Portability checks now exercise the shared core on all three desktop systems.
The matrix is being expanded to the portable mpv interface unit tests; native
playback remains a separate gate. The toolchain no longer forces installation
of a Windows standard library on every host. HWND-based engine startup is
Windows-only. Raw native identifiers are now behind an opaque platform
presentation, retained until decoder shutdown. Next: implement native OS
dialog/storage/presentation services and verify each application port.

The macOS/Linux shared GL application now has preference storage, library discovery
and owned asynchronous picker backends. Full application lint/test/build CI is
running for these targets (ADR 0004); obtain actual outcomes before declaring the
build gate closed. Next port work is runtime playback/dialog/lifecycle validation,
native localized filename ordering and target-specific presentation/HDR integration.
