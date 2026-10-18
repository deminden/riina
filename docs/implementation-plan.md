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
   Microsoft's material guidance defines Mica as an opaque wallpaper-tinted base
   material and Desktop Acrylic as a translucent backdrop that samples content
   behind the app; in-app Acrylic blurs content inside the app instead
   ([materials overview](https://learn.microsoft.com/en-us/windows/apps/develop/ui/materials),
   [Acrylic guidance](https://learn.microsoft.com/en-us/windows/apps/design/style/acrylic)).
   Riina therefore uses Mica for the persistent window foundation, Desktop
   Acrylic for selected auxiliary/transient windows, and video-backed in-app
   Acrylic for playback overlays. Desktop Acrylic cannot reveal or blur Riina's
   video surface behind its own child controls, so the measured video-sample
   path is the correct Windows material equivalent there; it is not claimed to
   be literal DWM Acrylic.
   Its Winit client is now alpha-capable; the Slint host stays solid until DWM accepts
   the requested type and returns it on readback, then exposes transparent client
   pixels. This confirms the attribute only; Windows may show its opaque fallback
   under system policy. Unsupported systems keep the previous solid host surface.
   The Winit-backed Welcome and Preferences windows now request the same persistent
   backdrop; Welcome layers its dark WinUI Mica fill over that backdrop. About and
   selected auxiliary windows request Desktop Acrylic. About exposes the accepted
   backdrop through transparent content panes; other utility windows expose it
   through transparent roots and WinUI-tinted toolbar, header and unselected-list
   surfaces while preserving solid data-entry and selected-item contrast fills.
   The Windows PiP viewport remains black behind its DWM video thumbnail while
   its controls use a tinted Acrylic fill. Solid platform colors remain when DWM
   rejects the request. The persistent player host refreshes its Mica attribute when the
   cached transparency, Battery Saver or High Contrast policy changes, and retries
   failed operations with increasing delays up to 30 seconds while keeping its solid
   client fallback. Disabling effects switches the client to opaque immediately even
   if Winit temporarily cannot provide the native window. A full-desktop live capture
   now shows video beneath the translucent native-player Quick Settings material.
   Persistent Mica and auxiliary Desktop Acrylic still need visible-pixel
   confirmation. The player
   host, Welcome and Preferences Mica, and
   registered auxiliary Acrylic windows refresh their DWM backdrop when transparency,
   Battery Saver or High Contrast policy changes. The Preferences page and navigation
   surfaces and Preferences card containers use the WinUI light Mica content-layer
   fill only while the backdrop is accepted, including the nested Online Media page;
   edit fields and selected rows retain solid fills.
   The auxiliary policy watcher retries a backdrop operation when the visible client
   state does not match the current system policy, even if that policy has not changed
   since a prior DWM request failed. Retries back off to 30 seconds and repeated
   identical DWM errors are logged once until recovery or a policy change.
   Registered auxiliary windows now synchronize their Winit native theme with the
   Slint palette (fixed light or dark, or live dark-theme state for About and Video
   Filters), so Windows caption and Acrylic treatment follow client appearance.
   The native player's Muda menu bar and caption now receive app-driven Light/Dark
   changes; the caption is reapplied after Muda's non-client redraw. Full-desktop
   captures verify both transitions while the window stays open. Muda's
   theme API covers the menu bar only, so dark-app dropdowns still inherit the Windows
   system popup theme and remain a tracked platform difference.
   The reusable Font Picker now follows the resolved player theme in content and
   its native frame (Winit/AppKit appearance), with DWM Acrylic on Windows.
   Light/dark WinUI layer fills and matching list, selection and preview colors
   preserve contrast across theme changes.
   Verify live transitions and pixels on Windows.
   The native player-controls overlay remains a separate transparent surface. Its
   enabled materials now receive throttled, downsampled SDR video samples and
   render an in-app video blur; HDR continues to use its tint fallback. Whole-desktop
   captures confirm transparency over playback. A five-frame 24fps moving-pattern
   sequence shows the material changing with the video, while earlier 4fps captures
   still show a one-frame offset. The native player now owns an optional Windows
   Graphics Capture session for its host HWND. It receives free-threaded D3D11
   frames, crops the client area into a CPU-readable staging texture, then bounds
   samples to 320×180. The Slint material sampler prefers these window-scoped frames and
   falls back to mpv's `screenshot-raw` when capture is unavailable or stale; HDR
   still uses its tint fallback. An isolated full-desktop playback check showed no
   stationary black patch after moving the player; the black letterbox bands moved
   with the viewport. A matched animated-pattern run confirms that Windows Graphics
   Capture supplies changing video frames to the translucent control surface. The
   rebuilt controller follows the 4 fps color cycle; its center sample remains within
   two 8-bit color levels of the captured video after stable grain is applied. See
   [Microsoft's screen capture guide](https://learn.microsoft.com/en-us/windows/apps/develop/media-authoring-processing/screen-capture), [`CreateFreeThreaded`](https://learn.microsoft.com/en-us/uwp/api/windows.graphics.capture.direct3d11captureframepool.createfreethreaded), and [`CreateForWindow`](https://learn.microsoft.com/en-us/windows/win32/api/windows.graphics.capture.interop/nf-windows-graphics-capture-interop-igraphicscaptureiteminterop-createforwindow).
   Its Quick Settings
   cards now use WinUI's dark/light Acrylic layer fill only while sampled-video
   material is active, and the `Fit Window to Video` action uses theme-specific
   WinUI control fills in the same state. The native controls popup uses DWM
   per-pixel alpha composition; after Slint realization it clears incompatible
   layered-window styles and reapplies the DWM alpha contract. The legacy
   `DwmEnableBlurBehindWindow` API does not blur on Windows 8 and later, but its
   documented alpha behavior is still needed for this transparent top-level
   surface. Video blur comes from sampled video frames, not this legacy API. A live
   native-video capture now shows the video through the controls overlay and
   tinted Quick Settings surface while the overlay follows host movement. Dark
   flat-pattern luminance is calibrated to the pinned IINA Quick Settings and
   controller samples, and both default sidebar and controller widths measure
   within a few points of source. The Windows material transform now uses a
   precomputed-coordinate bilinear enlargement after quarter-size blur; debug
   transform time fell from 26–30 ms to 17–19 ms. Captured sample age at display is
   39–66 ms. The separate native-video transform now precomputes its clamped
   horizontal and vertical blur taps and sums the fixed kernel corrections
   directly, preserving its existing integer weights. Workspace build and format
   checks pass. A 105-sample debug trace measured 5–7 ms for WGC readback,
   17–20 ms for transform, and 30–63 ms from compositor capture to display;
   the earlier transform trace was 28–31 ms. This confirms a shorter transform in
   this run, but not lower end-to-end latency or acceptable system cost. Nonuniform
   moving-video blur strength, other OSD materials, light appearance, system-policy
   transitions, exact glyph contours, and macOS runtime remain open. The floating
   controller now uses a 78-point spacious panel and
   71-point compact panel: IINA's 67-point value is its position-clamp constant,
   while the visible timeline row is 24 points after the seek slider's 6-point
   vertical padding is included. The 300%-scale capture shows the timeline and all
   eight actions without clipping. Controller offset still needs a matched-video-size
   comparison.
   A window backdrop alone does not prove feature parity.
   macOS 27.0.1 Liquid Glass
   rendering remains open.
3. Build macOS and Linux application ports alongside Windows feature and visual work.
   Windows 11 x64 is the only supported application target until each port has a full
   app build, visible playback, input, dialogs, window lifecycle, and shutdown evidence.
4. Deliver Windows binaries from the pinned native build. The MIT source and GPLv3
   combined-package terms are settled. A release still requires exact runtime contents,
   notices, public source access through the tagged application source and pinned
   upstream revisions, rebuild evidence, and playback tests from the final package.

## Current verification gates

Every Windows package candidate must run the portable packaging regression suite,
including candidates that reuse native-engine artifacts. After extraction, run
`scripts/smoke-windows-package.ps1` against the extracted executable and an isolated
fixture/profile. Its report must identify that executable and process, include
all required checks, and be followed by clean process exit and persisted-setting
verification. Keep native engine tests too; a headless DLL test alone cannot
verify the packaged application. Physical-display, audible-output and HDR checks
remain separate.

The current Windows regression sweep must include continuous volume changes and
fullscreen entry/exit, with the native host and mpv child remaining visible
through menu rebuilds. Use a bright isolated video fixture and full-desktop
captures; menu-row state alone cannot prove that video remains presented.
The Windows fullscreen controls popup deliberately leaves one physical scanline
to native video to avoid monitor-sized OpenGL alpha failure. Verify against that
documented extent, while the video host and mpv must retain full monitor bounds.
On exit, all client bounds must match exactly again. The user confirmed visible
video during controller reveal with this adjustment; HDR and other drivers remain
separate checks.

Current focused sweep: small portrait/landscape video controls, restoring hidden
groups after enlargement, native menu shortcuts and settings-panel titles, and
wide/standard/portrait/audio playlist transitions. Preserve the window center and
usable bounds while respecting the configured natural-size timing. Include an
audio-to-video transition; successful video-only navigation does not cover it.
Windows native playback retains an outgoing frame across playlist handoffs and
covers the full video surface, including aspect-fit matte bands. A 32-second
synthetic autoplay audit after this change covered wide, standard, portrait,
return-video, and audio clips with zero host/child size mismatches, zero off-screen
samples, one dark initial-startup sample, and normal shutdown. Captures no longer
show the incoming frame leaking through the retained image's matte bands. The user's
media and flashes shorter than the capture cadence remain unverified. A compact
Windows controller now shortens its height and timeline at small viewport sizes;
hover/focus interaction and matched macOS appearance remain unverified.
The embedded Windows mpv output now has `auto-window-resize=no` because Riina owns
resize policy and synchronizes its own host and child geometry. A 25-second mixed-size autoplay run completed with no fully dark samples, host/child mismatches, or off-screen samples. The user's affected media and flashes shorter than capture intervals still need direct verification.
The Windows alignment loop now validates the mpv child against both the host
client's screen origin and its size; size-only checks could miss a detached,
same-sized child. An isolated full-desktop recheck moved the player by 120×100
physical pixels and found the host client, controls popup, and mpv child at the
same resulting origin and size; no screen-fixed black patch appeared. Rapid clip
transitions and the user's affected media still need direct verification.
A later isolated 0.3.8 GUI recheck with moving-square captured the full desktop before and after moving the window about 100 logical points right; video and controls followed with no stationary black patch. This does not rule out the user media or playlist handoffs. If the symptom recurs, capture the visible player and its host, mpv child and controls-surface bounds together before changing alignment behavior.

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

Automatic Music Mode now requires 300 ms of stable audio-only track metadata before
it changes layout. A Windows mixed-media autoplay run preserved the video window
across four clips and entered Music Mode for the audio item; user-media and macOS
runtime checks remain open.
