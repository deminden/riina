# Native D3D11 Windows presentation

Status: accepted default Windows presentation path. The dated evidence below
explains the selection; it is not the latest verification report. See
[Verification](../verification.md) for current results and
[ADR 0003](0003-native-presentation-boundary.md) for later menu/alpha/fullscreen
invariants, including the one-scanline controls adjustment.

The Windows application uses this route by default and accepts `--shared-gl` to
select GL composition for diagnostics. `--native-video` remains an explicit alias.
The 2026-10-11 black-overlay regression was caused by Slint 1.18.1 registering
the shared renderer's conditional MenuBar even when `native-video` hides it.
Its Windows backend then overrides the transparent window attribute. Restore DWM
alpha on the realized chrome HWND with Winit's empty blur region; the visible menu
continues to belong to the framed host. Keep the native-mode background transparent
through loaded/stopped transitions. This is compositor setup, not a video decode fix.

The native owner has one event router: aspect filtering, control-window alignment
and focus restoration share it. Slint replaces rather than stacks window event
hooks, so installing the aspect filter separately removed alignment/fullscreen
focus handling. Property-driven aspect and window-level changes are queued after
the current UI read: synchronous resizing during an AccessKit tree update caused
reentrant `WM_GETOBJECT` and a RefCell panic on isolated startup.

This route uses the shared
player UI, not the probe toolbar. `player_ui.rs` configures the same controls,
catalogs, command senders and coalesced state updates for both renderers.
`native_window.rs` owns the host/popup relationship and fullscreen focus routing.
In windowed mode the popup fills the host client area; its transparent video area accepts the
player's mouse/keyboard interactions, so invisible regions need no separate
hit-test region. Host resize/move/DPI events align both physical size and position.
Startup awaits both HWNDs before sizing the popup. UI callbacks retain senders,
not the Player owner; closing either window joins the native worker before the
host HWND disappears.

During a 2026-10-11 Windows HDR desktop check, playback advanced through the
five-second fixture and the mpv GL framebuffer contained decoded pixels, while
the captured shared-GL application window showed a blank video area. The native
D3D11 route displayed the moving fixture on that same desktop. Windows now selects
the native route by default. Shared GL remains available and remains the default
for macOS/Linux while its Windows HDR composition issue is investigated.

Actual shared-UI evidence: `tests/visual/native-shared-evidence.json`. On the user's
HDR desktop, PQ/HLG presentation, source-derived sidebar placement, fullscreen,
Escape, Tab/Enter toolbar navigation, Stop, Ctrl+O after Stop, SDR recreation and
focused-overlay shutdown were exercised. Initial fullscreen focus loss, sizing
before popup realization and shortcut loss after removing a focused OSC were
found and corrected. Narrow-window dragging did not produce an observed resize;
that check remained pending in that run. Translucency was observed; later
material implementation and current gaps are described in
[platform differences](../platform-differences.md). Calibrated color, broad
DPI/ownership coverage and exact macOS matching remain open. This renderer selection does not imply full interface or feature parity.

The pinned mpv render API supplies OpenGL/software, not a D3D11 texture API.
Its `display-swapchain` property is available in composition mode; native HWND
output does not expose that pointer. Borrowing an unretained foreign COM pointer
across GUI/VO threads would require a proven lifetime contract. This route
instead supplies a live host HWND to mpv and retains the player until before host
closure. mpv owns its child video window and native D3D11 presentation.

An owned, frameless Slint window overlays the video. Ownership is established
before constructing that component because Slint captures winit attributes at
adapter creation. Startup awaits the actual winit window; `show()` or a zero-time
timer alone is insufficient. Host move/resize/scale events reposition the controls.
The ordinary application uses the native D3D11 host on Windows. Non-Windows
application ports continue using shared OpenGL behind the platform boundary.

`d3d11-probe` requests `vo=gpu-next`, D3D11 API/context, `rgba16f`, and automatic
color-space hints. The pinned manual calls rgba16f a best-effort hint for gpu-next,
so requested options alone are not evidence. Actual runtime logs report initial
and hinted swap-chain configuration `R16G16B16A16_FLOAT`,
`RGB_FULL_G10_NONE_P709`. The output query reports DISPLAY1 at 3840×2160,
10 bits, `RGB_FULL_G22_NONE_P709`: an SDR desktop during this test.

The original desktop check exercised native video, seek and play controls, Unicode text input,
fullscreen/restoration, maximize/resize and shutdown. Saved captures and selected
engine log lines are in `tests/visual/d3d11-probe-evidence.json`. Captures are
ordinary desktop images, not HDR measurements or upstream visual goldens.
The accessibility tree exposes both the host and owned controls, with duplicate
related-window entries in the capture service. Indexed button targeting was
inconsistent; coordinate targeting verified the actual overlay controls.

The final optimized probe reports scale 3 and a 2880×1800 physical client for
its 960×600 logical host. The capture service supplied normalized logical images. Earlier
capture-manifest values of scale 1 were not measured DPI; those values are now
unknown with an explanatory note. This one measured scale does not pass the
required DPI/mixed-monitor matrix.

Original PQ/HLG fixtures now decode as 10-bit BT.2020. A later test, after the
user enabled HDR in Windows, queries DISPLAY1 as RGB_FULL_G2084_NONE_P2020.
The observed native target is rgba16hf, full-range RGB, BT.709, scRGB, with engine
target peak 1499 nits; the swap chain remains 16-bit float linear Rec.709.
PQ→HLG→stop→SDR works on both desktop modes. This establishes an engine-reported
HDR presentation route rather than an RGBA8 SDR-only texture path. It does not
establish calibrated luminance, color accuracy or complete production integration.
See `tests/visual/hdr-lifecycle-evidence.json` for actual source/target transitions.

Observing video-target-params exposed non-finite aspect/par values in color-only
native parameters. They now decode to unknown JSON values without inventing a
finite number or failing playback. A unit regression covers that case. Source
parameters can also initially contain only container dimensions; missing color
metadata stays optional until an effective decoded frame is reported.

Gates recorded by the original experiment: color transforms, calibration and consistent SDR UI luminance;
compressed HDR/hardware decoder evidence; calibrated HDR display evidence;
mixed-monitor DPI/ownership/minimize/activation, clipping at narrow widths,
sidebars/menus, keyboard routing outside controls, taskbar/Alt-Tab behavior,
load-failure/rapid-replacement lifecycle, performance and hardware-decoder interop.
The user enabled Windows HDR for the second run. Alt+F4 initially hid only the focused overlay; the probe now closes
the complete player from either window. Native scRGB is the active Windows
presentation route while source-matched controls, sidebars, clipping, output
calibration and broader display coverage are validated. The general HDR parity
requirement remains open.

Later fixes and current gaps are tracked in [status](../status.md),
[platform differences](../platform-differences.md) and
[verification](../verification.md); the original experiment is not a current
task checklist.
