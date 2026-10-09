# Source-derived visual specification

Baseline v1.5.0 only. No macOS reference captures were available. No official images
have been downloaded or measured. Every color and glyph metric is currently estimated.

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

Current spike uses 78 logical pixels for practical slider layout, a native Windows
titlebar, estimated typography, substitute Unicode glyphs and a tinted panel. This
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
