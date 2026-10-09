# Source-derived visual specification

Verified reference state IINA-1.5.0-PAUSED-SEEK-001 is now available from run
38005949498: 320 x 180 pixels, scale 1, macOS 26.6.2, Light theme. Actual engine
observations bracket the image at paused 2.0 seconds; frame 048 agrees. A transient
seek OSD reads 00:02 / 00:05, with a progress indicator; main controls remain hidden.
This is accepted reference evidence, not a matched-size riina comparison golden.
The persistent playback phase/speed diagnostic line has been removed from riina;
faithful transient OSD and visible-control/sidebar captures are still required.

Baseline v1.5.0 only. Official macOS reference execution now exists: the initial
capture is a 320 x 180 player window on macOS 26.6.2 at scale 1, Light theme,
with the original catalog fixture visible and controls hidden. Its requested
two-second position was not established, so it is excluded from matching-state
golden comparisons. See tests/visual/reference-captures.json and
docs/reference-capture.md. Control colors, material and glyph metrics remain
estimated until actual visible-control captures are reviewed and measured.

| Token | Value | Evidence |
|---|---|---|
| floating width | 460 logical points; min 200 upstream | OSCFloatingView.width and minimum constraint |
| bottom offset | video height × 0.1 | Preference.controlBarPositionVertical |
| center | video width × 0.5 | Preference.controlBarPositionHorizontal |
| corner | 6 pre-macOS 26; 12 on macOS 26 | OSCFloatingView.init |
| top padding | 14 spacious / 10 compact | OSCFloatingView LayoutValue extension |
| bottom padding | 8 spacious / 5 compact | OSCFloatingView LayoutValue extension |
| panel horizontal padding | 12 top / 8 bottom | OSCFloatingView.init |
| row separation | 8 | OSCFloatingView.init |
| sidebar width | 360 | Preference.defaultPreference |
| sidebar width limit | 80% window width | SidebarController.sidebarMaxWidth |
| sidebar side | playlist/chapters leading; settings trailing | Preference.defaultPreference |
| track rows | 26 points | SidebarTrackSelector.tableView |
| playlist rows | 30 spacious / 26 compact | SidebarPlaylistPane LayoutValue |
| chapter rows | 40 points | SidebarChaptersPane.tableView |
| auto-hide | 2.5 s | Preference.defaultPreference |

The floating controls now use the recorded 67-point outer height, 460-point width,
14/8 vertical insets and 8-point row separation. The 17-point button row is an
independent layout allocation pending reference capture measurements. A native
Windows titlebar, estimated typography, original vector playback icons and tint remain. This
is not pixel parity or video-sampled blur. Replace toolkit sliders and glyphs with
measured custom geometry after live baseline captures. Light/compact/docked layouts,
window controls, full sidebar layout, animations and focus measurements remain incomplete.
Initial catalog sidebars now use source width/side defaults and row heights; source
tab groups, simultaneous leading/trailing panels, drag resize and language localization
remain pending. Materials, spacing and typography still require reference captures.
The client video dimensions are converted to physical pixels with Slint's DPI scale.

Capture plan: initial, playing, paused, hidden OSC, both themes, compact/spacious,
each sidebar, music/PiP, menus and settings. Record OS, IINA version, theme/layout,
client dimensions, scale, fixture hash/position and panels. Normalize client areas;
define per-region tolerance before goldens. Windows captures prove regression only.

Pixel-perfect interface parity is a primary acceptance goal alongside features.
Default riina surfaces must match IINA v1.5.0; extensions remain opt-in. Measure
layout, typography, icons, material, focus and animation by region at matched
client sizes and scale. Golden comparison requires actual reference captures;
source geometry and Windows-only captures cannot prove pixel-perfect completion.

Player controls use original MIT vector icons recorded in assets/provenance.json.
The shared component renders a consistent 17-point box and enabled/disabled tint,
independent of system symbol-font coverage. Remaining repeat/secondary controls
use provisional glyphs. Vector shapes are not yet measured against macOS goldens.

The shared seek panel now uses estimated geometry from the reviewed paused-seek
capture: an upper-left rounded dark panel, bright elapsed clock, subdued total
clock and blue progress indicator. Values come from observed playback, triggered
by a native seek completion counter rather than the requested destination. The
2.5-second timeout is provisional: the capture proves a visible state, not its
duration. Typeface, material, exact metrics, animation and timing remain open
comparison gates. No reference pixels or protected assets are shipped in the UI.
