# Implementation plan

Riina is an established, functioning media-player application with real Windows
playback and broad player and preferences workflows. The remaining work is closing
IINA v1.5.0 behavior and interface-parity requirements; this plan describes current
priorities, not a project timeline. The traceable requirement inventory is
`docs/parity.csv` and remains incomplete.

## Priority order

1. Close fully missing IINA behaviors in the requirement inventory. Choose work by
   user impact and dependency order; carry shared state and behavior through portable
   layers, then connect each platform adapter. Keep the opt-in 200% volume extension
   available while preserving the 0–100% default.
2. Measure and close visual gaps against actual pinned macOS captures. Compare complete
   player, menu, sidebar, preferences, dialog, and auxiliary-window surfaces at matched
   states and sizes. Preserve IINA's visual language; do not replace it with a redesign.
   Implement platform-native materials as a tracked part of this work:
   - macOS keeps IINA's Liquid Glass setting and uses the current macOS material API.
   - Windows 11 uses Mica for the persistent window backdrop and Acrylic for transient
     surfaces; playback overlays must compose against the playing video, using in-app
     Acrylic when the renderer supports it or a measured video-backed fallback.
   - Keep labels and fallback behavior platform-aware. Honor Windows transparency and
     battery-saver fallbacks; do not call Windows materials Liquid Glass.
   - Linux uses the compositor's native backdrop when available and a documented
     in-app fallback otherwise.
   Current state: the Windows native player host requests and reads back
   `DWMSBT_MAINWINDOW`, which Windows 11 maps to Mica on build 22621 and later
   ([Microsoft DWM backdrop types](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/ne-dwmapi-dwm_systembackdrop_type)).
   Unsupported systems keep the renderer surface. Debug playback works. Fresh
   per-window captures show Welcome and the empty player, but the helper still
   loses the active playback window, so visible Mica remains unverified. Slint
   surfaces remain opaque, transient Acrylic and video-backed overlays are open,
   and the shared switches still select RGBA tint/opacity. macOS 27.0.1 Liquid Glass
   rendering remains open. Inspect Slint/Winit transparency and per-surface compositor
   boundaries before wiring effects; one window-wide backdrop does not prove parity
   for OSD, controller and sidebar.
3. Build macOS and Linux application ports alongside Windows feature and visual work.
   Windows 11 x64 is the only supported application target until each port has a full
   app build, visible playback, input, dialogs, window lifecycle, and shutdown evidence.
4. Deliver Windows binaries from the pinned native build. The MIT source and GPLv3
   combined-package terms are settled. A release still requires exact runtime contents,
   notices, public source access through the tagged application source and pinned
   upstream revisions, rebuild evidence, and playback tests from the final package.

## Current verification gates

Current focused sweep: small portrait/landscape video controls, restoring hidden
groups after enlargement, native menu shortcuts and settings-panel titles, and
wide/standard/portrait/audio playlist transitions. Preserve the window center and
usable bounds while respecting the configured natural-size timing. Include an
audio-to-video transition; successful video-only navigation does not cover it.
Windows playback still flashes during some clip transitions. The pinned mpv
Windows video output creates a child HWND and resizes it after parent-location
events; the app now includes that video child with the host and transparent
controls in the native resize batch. The follow-up 35-second loop found zero
host/child size mismatches in 2,082 stable samples, after one mismatch in the
previous build. Rendered pixels remain unverified, and the user's reported flash
remains open. Capture transition frames before considering this closed.

- Every implemented behavior has typed intent, observed backend state where applicable,
  and tests at the layer that owns the behavior.
- Visual parity claims require Windows captures compared with the pinned macOS reference;
  source inspection and layout dimensions alone do not establish parity.
- Material parity requires separate runtime checks for window backdrops, transient
  surfaces and video-backed overlays, including OS transparency and power-saving fallback.
- Platform support requires full application and playback/lifecycle evidence. Library-only
  CI is insufficient.
- Binary publication requires successful validation of the exact candidate package,
  its notices and public corresponding-source access. The license choice itself is not a gate.
- Run long checks asynchronously and continue independent work while they run. Record
  which revision and components each check covers; do not wait idly or launch duplicates.

## Architecture invariants

Keep playback, preferences, playlist/catalog models, and user-visible policies portable.
Place HWND, display, file-dialog, file-manager, and OS lifecycle operations behind
platform adapters. Keep libmpv ownership explicit and tear down playback before its
presentation surface. Shared feature work must not introduce Windows assumptions into
macOS or Linux code. Record genuine platform differences in the requirement inventory.

Use small Rust modules, typed intent, and comments that explain semantics or invariants.
Keep the IINA v1.5.0 reference outside product source. Implement from observable behavior
and documented APIs; do not translate upstream implementation code or copy protected
assets.
