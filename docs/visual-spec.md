# Visual specification

The target is IINA v1.5.0. This page records design rules and reusable reference
metrics. [Status](status.md) owns implementation state; the
[comparison ledger](visual-reference-audit.csv) owns per-reference evidence.

## How to read measurements

- **Source-derived:** values and semantics inspected in the pinned IINA tree.
- **Observed:** pixels or states in a named capture, valid for that environment.
- **Estimated:** inferred scale, geometry or styling that still needs measurement.

The supplied reference machine is user-reported macOS 27.0.1 on an M2 MacBook Air.
Automated captures use macOS 26.6.2. Supplied-image display scale was not recorded;
2x conversions below remain estimates. See [reference provenance](visual-reference-audit.md).
A matching source dimension or Windows golden does not prove macOS visual parity.

## Platform materials and artwork

| Platform/surface | Design target |
|---|---|
| macOS | IINA's Liquid Glass controls and native semantic materials; AppKit owns native titlebar/popover treatment |
| Windows persistent windows | Mica with appropriate content layers |
| Windows auxiliary/transient windows | Acrylic where supported, with solid fallback |
| Playback overlays | Video-backed in-app blur/tint; never substitute desktop wallpaper for video |
| Linux | Available compositor material or a documented in-app fallback |

Use platform-specific labels: Windows calls the setting **Translucent Surfaces**.
Honor transparency, High Contrast, Battery Saver and reduced-motion policy.
Keep the material visible over representative moving video, not only a flat test
pattern. Runtime appearance and policy changes require separate comparisons.
Implementation boundaries are in [platform differences](platform-differences.md).

Use host typography and original licensed vector icons. Apple fonts, SF Symbols
and protected artwork are not bundled. Preserve geometry, visual weight and
meaning; record unavoidable glyph/rasterization differences. Branding follows
the [brand guide](../assets/branding/BRAND_GUIDE.md) and must not redesign playback
controls. Icon provenance is in `assets/provenance.json`.

## Shared geometry

Values are logical points unless a capture explicitly says pixels. `LayoutValue`
and named classes below refer to `reference/iina-upstream/iina/`.

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
| floating transport button frames | 24 × 24 points; speed up/down and play/pause glyphs fill the frame | MainWindowController setup of `oscPlayControlMiddleView` |
| sidebar width | 360 | Preference.defaultPreference |
| sidebar width limit | 80% window width | SidebarController.sidebarMaxWidth |
| sidebar side | playlist/chapters leading; settings trailing | Preference.defaultPreference |
| sidebar content margins | 16 spacious / 14 compact | `LayoutValue.sidebarMargin` |
| sidebar card spacing | 18 spacious / 16 compact | `LayoutValue.sidebarStackViewSpacing` |
| sidebar card padding | 12 spacious / 10 compact | `LayoutValue.sidebarContainerPadding` |
| sidebar row spacing | 12 spacious / 8 compact | `LayoutValue.sidebarItemSpacing` |
| Aspect/Crop/Rotation card content height | 204 spacious / 180 compact (six rows, five gaps, two insets) | `SidebarVideoPane.createSettingsView()` and `LayoutValue` |
| track rows | 26 points | SidebarTrackSelector.tableView |
| subtitle lane header | 16-point icon/label/switch row; 12/8-point group gap | `SidebarSubtitlesPane` and `LayoutValue.sidebarItemSpacing` |
| playlist rows | 30 spacious / 26 compact | SidebarPlaylistPane LayoutValue |
| chapter rows | 40 points | SidebarChaptersPane.tableView |
| auto-hide | 2.5 s | Preference.defaultPreference |

### Controller position and compression

The vertical fraction measures the distance from the video-view bottom to the
controller's bottom edge, not its top edge or the outer frame. The default is
0.1. IINA's default-position clamp uses `video height - 67 - 25`; the 67-point
constant is not the complete visible controller height. Drag bounds preserve
25 points below the video's top and allow the bottom edge to reach its bottom.

Measure against the video container. With edge-to-edge video disabled, that
container excludes sidebars; normalize the horizontal fraction to its actual
width. A 460-point floating bar compresses centered when its full width and
insets cannot fit. Hide mute/volume when no primary audio track is selected,
retaining the leading spacer so the transport remains centered. Equal leading
and trailing reservations must also survive toolbar customization; centering the
whole unequal row shifts the transport. The play and pause hit frames share the
same center; assess glyph optical balance separately.

The current original play vector has an ink centroid at x=9.167 in its 24-point
view box. Its centered image box therefore looks left-heavy. The paused glyph
uses a 2.5-point optical adjustment (scaled for Music Mode), leaving the button
and symmetric pause bars fixed. Widget checks cover both the hit frame and ink
centroid; this correction is not a matched macOS pixel acceptance result.

Music Mode has separate geometry. Pinned `MiniPlayerWindowController` facts are
28-point transport frames, a play-frame center two points right of window center,
and 18-point volume/playlist/artwork controls. Utility buttons must not push the
transport around. Riina expresses these constraints in its own small shared UI
component; the floating video controller retains its separate sizing policy.
Close/Return belong at the window content's top-left (8-point leading, 6-point
upper inset), rather than inside the bottom transport row. Their frame sizes are
13 points with artwork and 12 without it; vertical gaps are 5/4 points.

Quick Settings keeps scroll memory per pane and per sidebar, including while
Layout replaces the scrolling body. A different pane must not inherit the old
pane's offset. Playlist rows take their full available width. The compact footer
uses loop/shuffle/sort and add/remove/clear groups with 8-point gaps; less frequent
editing actions remain in each row's context menu. The reference's total-duration
label and complete file/subtitle/plugin context actions remain comparison work.

Windows has documented narrow-window and sidebar-avoidance adaptations. Compare
them against the reference before accepting their geometry; see
[platform differences](platform-differences.md#windows-interaction-adaptations).

## Playback controller

The default center group is **previous, play/pause, next**. There is no square
Stop button in the default controller; Stop remains a command elsewhere.
The default right-hand actions, in order, are:

1. Show Plugins Panel.
2. Picture in Picture.
3. Playlist and Chapters.
4. Quick Settings.

The gear opens Layout/Video/Audio/Subtitles Quick Settings. Do not add a duplicate
app-drawn action row over the video. The current media filename belongs in the
window title; macOS local files also use the represented document URL.

Source anchors: `Preference.swift` defines toolbar types/defaults;
`MainWindowController.swift` builds and dispatches both button groups;
`OSCToolbarButton.swift` defines toolbar geometry.

| Detail | Reference measurement or source allocation |
|---|---|
| Spacious controller | 460 x 78: 14 top + 24 controls + 8 gap + 24 timeline + 8 bottom |
| Compact controller | 71 high with 10/5 top/bottom insets |
| Transport spacing | 24 floating; 16 docked |
| Toolbar button frame | 24 x 24 with no inter-button gap |
| Timeline | 46 minimum time-label width, 2 label/slider gap, 8 outer inset |

The 78-point visible height agrees with REF-20 at its inferred 2x scale; exclude
shadow pixels. Compare glyph silhouettes, material motion, focus and hide/reveal
behavior separately.

Fullscreen Additional Info derives from `AdditionalInfoView.swift`: title above
an 18-point bold monospaced clock, 2 x 16 separator and 13-point bold battery
percentage. Time formatting must follow the host's user settings.

## Quick Settings

The tab order is **Layout, Video, Audio, Subtitles**. `SideBar.swift` uses 14-point
bold symbols; selected symbol and text share a visual centerline. Source symbols
are paintbrush, photo, waveform and caption bubble; use independently drawn
equivalents. The selected capsule accommodates its label, while icon-only tabs
use their own measured width.

### Layout metrics (REF-21)

| Element | Source allocation |
|---|---|
| Theme selector rail | Observed approximately 24 points high at inferred 2x scale in REF-21; Windows uses 2-point internal insets and 20-point segments |
| Theme, compact-interface and video-setting symbols | 20 points |
| Material heading and mini-row symbols | 16 points |
| Section captions | Bold 12-point system text |
| Controller choice tiles | 66 x 74, gaps 8, total width 214 |
| Tile corner/outline | Radius 8; selected 2, inactive 1 |
| Tile preview | 40 x 28 viewport around inset 32 x 25 frame |
| Tile label/insets | Label gap 8; item padding 12; bottom inset 8 before section gap 18 |
| Sidebar Position rows | 24-point controls, 10/6 spacious/compact stack gaps |
| Sidebar Position card | 12/10 padding; derived heights 118/106 including border; trailing inset 8 |

Dark, Light and Auto selected fills, text and focus outlines stay inside the
selector rail when appearance changes. Its segments retain equal bounds and
alignment across hover, keyboard focus and selection. AppKit owns the original
`NSSegmentedControl`; the Windows equivalent uses the host accent and player
palette. Do not change the 30-point Playlist/Chapters tabs to size this small rail.

Center each row symbol, label and switch vertically, including Compact Interface
and sidebar-position rows. Section rules run through the caption centerline.
Center the three controller choice tiles on the scroll page, and each preview
inside its tile. Narrow Layout panes retain the reference tile frames on their
scrollable content rather than squeezing the drawings or clipping their labels.

Keep both spacious and compact states scrollable, with every setting reachable.
Do not use old capture-tool failures as a current blanket blocker.

## Preferences

The automated Settings set records all eleven pages in light appearance at 1x.
Its outer frame is 608 x 500; source client geometry is 600 x 480 with a 200-point
navigation split. OS-owned frames differ, so compare client content separately.
Windows initially requests 600 x 620 to show more navigation on high-DPI desktops,
retaining 600 x 480 as the minimum. Work-area fitting clamps auxiliary dimensions
independently; it must not narrow settings just because the screen is short.
The current Preferences surface and standard widgets consistently use light
appearance; broader theme parity remains open.

The sidebar contains search, page symbols, selected rows, section links and
scrolling. Categories are General, Interface, Video, Audio, Subtitles, Network,
Control, Key Bindings, Advanced, Plugins and Utilities. The complete scope is in
the [interface checklist](interface-parity-plan.md#all-eleven-preferences-categories).

| Reference detail | Measurement/state |
|---|---|
| General cards | 40-point row rhythm, 10-point corners; disclosure and expanded states matter |
| Interface Appearance | Three-row list, about 122 high, starting near y=81 in the captured client |
| Interface Window | Initial geometry switches in an 82-high default list near y=248; resize choices in a separate 118-high list near y=346 |
| Control | Trackpad, Mouse, scroll assignment, then seek type/sensitivities; lower content scrolls |
| Key Bindings | System-media row, profile card, search/raw controls, scrolling action table and right-aligned keycaps |
| Advanced | Enablement, Logging, mpv Settings, then additional-options editor below the initial viewport |
| Utilities | Default Application, Restore Alerts, Clear Cache, Browser Extensions; trailing buttons and size labels |

Do not treat captured scroll positions as absolute layout constants. Disclosures,
long translations and different OS fonts change content extent. Network connection
subsection geometry is source-derived rather than captured.

### Legacy subtitle and font-picker measurements

These XIB measurements explain source behavior and older allocation. They do not
automatically define the new Settings page's layout.

| Group | Pinned source facts |
|---|---|
| Font picker | `FontPickerWindowController.xib`: width 458, outer margins 12, filter 24, table block 268 with 23-high headers, editable preview 36 |
| ASS | `PrefSubViewController.xib`: section 99 high; lane selector, five-level override (`no`, `yes`, `scale`, `force`, `strip`), explanation |
| Text Subtitles | 480 x 319 section; Font box 366 x 105, Border box 366 x 56; folded Advanced reveals Shadow and Other Styles |
| Position | 480 x 208 section; X centered, Y bottom, vertical position 100, margins 25/22, letterbox and window scaling enabled |
| Auto Load | 480 x 193 section; Disabled / filename / IINA matching; advanced priority/search paths; defaults IINA mode, no terms, `./*` |

XIB Y coordinates use AppKit's bottom origin. A 19-point font-table pitch was
inferred from a scroll increment, not an observed native row height; keep it
classified as an estimate.

## Music Mode

Music Mode has its own compact layout, art/metadata, timeline, volume popover,
playlist/artwork toggles and return-to-player action. The supplied REF-22 frame
measures 600 x 518 pixels. At the inferred 2x scale this suggests 300 x 259 points,
with about 169 points for artwork and 90 below; scale confirmation remains open.
Artwork height also depends on media dimensions and the artwork-visibility setting.

Windows hides its in-window menu in Music Mode to avoid wrapping over the compact
controls and restores it on return. macOS retains its external application menu.
Check title/art transitions, hidden controls, focus, playlist and native video
continuity rather than comparing only the static compact frame.

## Auxiliary materials

Auxiliary acrylic cards use shared neutral white fills (`#ffffff40` in light
appearance and `#ffffff09` in dark appearance). Slint color literals use RGBA;
Windows ARGB integers must be converted explicitly rather than pasted as literals.

## Acceptance

Use the full [interface checklist](interface-parity-plan.md). Match capture state,
record scale/dimensions, define region-specific tolerances, and inspect actual
images. Include both themes, compact/spacious and narrow layouts, selected and
disabled states, keyboard focus, scrolling and animation. Preserve defects and
unknowns in the ledger; do not regenerate the upstream reference to match Riina.
