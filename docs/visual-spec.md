# Source-derived visual specification

Current OS target: macOS 27.0.1, Apple's latest release as of 2026-10-12.
Existing accepted IINA 1.5.0 captures in this document are from macOS 26.6.2 and
remain evidence for that captured state only. A matched 27.0.1 reference capture
and Riina comparison are required before claiming latest-version visual parity.

## Platform materials

Use the platform's material vocabulary and implementation. On macOS, preserve
IINA's Liquid Glass controls and render them with the macOS material APIs. On
Windows 11, use Mica for long-lived window backdrops and Acrylic for transient
surfaces; use in-app Acrylic when available or renderer-specific video composition
for overlays whose background is the playing video. Do not label Windows surfaces
Liquid Glass or let a desktop backdrop replace the video behind playback controls.
Mica/Acrylic effects should follow Windows transparency and power-saving fallbacks.
Mica is an opaque, wallpaper-reactive base material; it does not reveal windows
behind the app. Use Acrylic where actual translucency is intended. Playback overlays
must blur and tint the video without softening the rest of the picture. Microsoft
documents `DWMSBT_TRANSIENTWINDOW` as drawing behind the full window bounds. A live
Windows 11 trial of a compact owned popup stacked over mpv's video child rendered a
solid gray block over the picture instead of revealing the video. Use the
video-backed composition for Riina's playback surfaces unless a Windows compositor
path is proven to sample the native video output correctly.
Linux uses its native backdrop material when the compositor exposes one and
otherwise uses the documented in-app fallback.

The Windows Slint controls are captioned “Translucent Surfaces”. Renderer-backed
video surfaces composite a sampled blur, subtle Windows-only grain, and tint in-app
to avoid replacing video with a desktop backdrop; this is an Acrylic approximation,
not DWM Acrylic. The REF-20 Windows OSC currently uses a 33%-alpha dark fill
(`#15151555`) over the 58%-opacity sampled-blur layer. This is calibrated against
the panel-to-video luminance ratio in saved IINA and Riina captures with different
flat-color fixtures; it is a measured starting point, not a same-frame pixel match.
Welcome and Preferences use separate native Mica paths. Welcome and About are
alpha-capable from creation;
full-size Slint content preserves the opaque fallback until DWM accepts the material,
then uses an 85%-opaque tint. Preferences keeps its existing opaque page surfaces
over Mica. The regular Windows Winit backend creates alpha-capable native
surfaces for windows without a native menu; Slint keeps native-menu windows opaque.
The dedicated native-video backend keeps an alpha-capable native host but paints
the video client opaque black; only its separate controls surface is per-pixel
transparent and composites sampled-video Acrylic over playback. About, Video
Filters, Playback History, Log Viewer, Online Subtitles and Font Picker use a
DWM transient Acrylic path with solid fallback.
Other non-video auxiliary surfaces still use tinted surfaces without a system
material. When accepted, the About window now lets DWM's Desktop Acrylic base
show through its content panes; a rejected backdrop keeps the existing solid
appearance. For enabled playback materials, Windows Graphics Capture samples the
native video surface at up to 30 Hz and directly downsamples its mapped D3D11
surface to at most 320×180. The mpv RGBA screenshot path is a fallback, throttled
to 100 ms, and also downsamples before storing the sample. A 500 ms startup grace
lets available native capture supply the first image. Source-aware identities
prevent native and fallback frame counters from colliding; samples older than
750 ms are discarded even while paused. This keeps the material responsive to
4 fps content without matching the decoder's display rate. Asynchronous transform
replies are polled at the UI cadence (16 ms), and the quarter-resolution blur
runs on a bounded worker. The UI supplies a blurred, lightly grained SDR image to
Slint. Paused native capture refreshes the still compositor image; paused mpv
fallback samples once per position. This avoids a second visible renderer and
prevents a full decoded frame from lingering in the material state. This is an
in-app video-backed
Acrylic approximation; HDR overlays retain the existing tint because the sample
is SDR. Playback
performance and pixels still need live verification. [DWM system backdrops](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/ne-dwmapi-dwm_systembackdrop_type)
cover the full bounds of their window, so putting Desktop Acrylic on the full-size
controls overlay would obscure playback; the older
[`DwmEnableBlurBehindWindow`](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/nf-dwmapi-dwmenableblurbehindwindow)
API no longer draws blur on Windows 8 and later. The
macOS control keeps its
“Liquid Glass” label and must be verified on macOS 27.0.1.
The native-video host still requests DWM's main-window Mica attribute, but paints
its video client opaque black on every policy path to prevent desktop content from
showing through playback. The Settings window requests Mica and uses the WinUI
light content-layer fill (`#ffffff80`). Welcome requests Mica and uses WinUI's dark
`LayerFillColorDefault` (`#4C3A3A3A` ARGB, `#3A3A3A4C` in Slint). About requests
transient Acrylic. Video Filters, Playback History, Log
Viewer, Online Subtitles and Font Picker now request transient Acrylic and switch
their root tint to alpha only after DWM accepts it; opaque inner content preserves
the original readable surfaces. Their visible pixels still need a live capture.
Native-video controls use sampled-video SDR blur rather than a full-window DWM
backdrop. Registered transient windows refresh their backdrop when Windows
transparency, Battery Saver, or High Contrast policy changes. Windows suppresses
Acrylic during Battery Saver but keeps Mica; both materials yield to disabled
Transparency effects and High Contrast. The native-video and shared-GL player
paths stop custom video blur and WinUI Acrylic card treatment during Battery Saver,
while their Mica host backdrop remains. Verify live pixels and fallback readability
per surface.
Platform references: [Apple Materials](https://developer.apple.com/design/human-interface-guidelines/materials),
[Windows Mica](https://learn.microsoft.com/en-us/windows/apps/design/style/mica),
[Windows Acrylic](https://learn.microsoft.com/en-us/windows/apps/design/style/acrylic),
and [WinUI Acrylic theme colors](https://github.com/microsoft/microsoft-ui-xaml/blob/main/controls/dev/CommonStyles/Common_themeresources_any.xaml).

For the macOS implementation, use AppKit's semantic materials by surface role:
`NSGlassEffectView` for Liquid Glass control groups and
`NSVisualEffectView.Material.sidebar` for sidebar backgrounds. Let AppKit own
titlebar and popover materials where it creates those views itself. Its contract
for `NSGlassEffectView` only guarantees the assigned `contentView` sits inside
the glass; it does not define arbitrary sibling-view z-order. Do not layer a glass
view blindly over or under the single Slint-rendered player view. A per-surface
bridge must receive the real Slint bounds and prove that controls, hit-testing,
video sampling, and accessibility remain in the right order on macOS 27.0.1.
The current pinned Winit 0.30.13 AppKit backend uses the window's `contentView`
as its raw AppKit view handle; Slint does not expose each player control group as
an independent `NSView`. Any native glass integration therefore needs an explicit
per-surface composition boundary instead of treating the Winit handle as a control
container.
See [NSGlassEffectView](https://developer.apple.com/documentation/appkit/nsglasseffectview)
and [NSVisualEffectView](https://developer.apple.com/documentation/appkit/nsvisualeffectview).

The macOS 27 Quick Settings reference `REF-21` contains a solid green test-pattern
region outside the dark sidebar (`#65C466` at source pixel 2000,400) and a blank
sidebar region (`#459546` at 2300,800). The current video-backed Slint fallback
uses a 32% sampled-blur layer; its dark sidebar base is calibrated to 42% opacity
on macOS (`#1515156B`) to approach those measured pixels. Windows keeps its
separate fallback tint. Windows now uses an 80% sampled-blur layer for the
Quick Settings sidebar to reduce sharp high-contrast video detail; the controller
and other video-backed surfaces remain at 58%. macOS native-video sidebar layers
remain at 53%, and shared-renderer layers at 32%. The Windows shared-GL kernel now
matches the native path's sigma-10 blur at the
320×180 material-sample scale, deriving horizontal and vertical tap spacing from
the source aspect ratio instead of using the much tighter legacy kernel. macOS
keeps its prior GL blur spacing. In the default 360-point sidebar, REF-21 measures
the Layout cards at 328 points wide with 16-point side insets; keep these cards
centered in the pane's 16/14-point outer margins. The pinned source sets outer
Layout-pane spacing to 18 points in spacious mode and 16 in compact mode; these
source values take precedence over the screenshot's rough 20-point gap estimate
until a matched-scale capture resolves the difference. Its 16/14-point side
margins yield 328/332-point cards at a 360-point sidebar, respectively. The first
Layout card also uses source-defined 12/10-point inner padding, 12/8-point group
spacing, and 6/5-point spacing between the three compact material switches.

The Windows Layout pane sizes the first card to 152/138 points and the other two
cards to 44 and 76 points, following IINA's source-defined inner rhythm. Full-
desktop captures verify that both spacious and compact settings fit without
clipping; the inferred screenshot scale and exact card-edge alignment still need
pixel scoring. The Quick Settings tab capsule now derives its width from the
selected label: three 36-point icon tabs plus a label-sized selected tab, with
the 3-point inter-tab gaps and 6-point capsule inset. The Layout reference is
about 191–192 points wide; the shorter selected Video label yields a narrower
capsule, matching REF-24's approximately 186-point crop. The Windows header row
retains its 10-point side insets. 300%-scale UIA showed the old Subtitles target
18 pixels outside the player; the corrected geometry places the capsule edge 30
pixels (10 points) inside it. The UIA evidence is recorded in
`tests/visual/riina-osc-sidebar-geometry-evidence.json`; fresh rendered bounds
and pixels remain unverified.
The REF-21 on-screen-controller selector's equal tiles measure 66×74 points,
with 8-point gaps and a 214-point total row in the 2× reference crop. This agrees
with the pinned 40×28-point preview artwork, 12-point item padding, and default
8-point `UIHelper.hStack` gap. Riina now uses the measured 66×74-point tiles and
8-point gap; post-build UI Automation should confirm its 214-point row.
The sidebar's Windows dark tint is 29% opaque (`#1515154A`); light remains
78%. Rounded Windows sidebar corners retain a subtle one-pixel edge. Pixel
samples from REF-24 give `#65C466` video and `#449345` sidebar, about a 52.7%
linear-sRGB luminance ratio. Matching live Windows captures of the same flat
green test pattern measured 39.7% with 40% tint (`#417455`). Same-size full-
desktop captures at 28% and 32% gave 54.2% (`#4D8660`) and 49.1% (`#49805C`)
in the un-carded trailing strip; card-fill pixels are excluded from the material
sample. The 29% tint capture measured 52.5% (`#4C845F`), within 0.4 percentage
points of the exact sampled REF-24 pair at 52.1%. The full-desktop comparison
uses a 360-point sidebar at 300% Windows scale versus 360 points at 2× in REF-24.
This remains a cross-platform material approximation, with sampled-video phase,
panel geometry, and light-theme behavior tracked separately.
An independent flat-green REF-21 check gives the same result: its outside/sidebar
pair is `#65C466` at `(2000,400)` and `#459546` at `(2300,800)`, retaining 53.7%
linear luminance. The 2026-10-15 Windows Video-panel capture measures `#62B27D`
at `(2150,850)` and `#4C845F` in an un-carded sidebar strip at `(2250,850)`,
retaining 52.6%. This is a 1.1-point ratio difference at the same 360-point
sidebar width. It validates the dark video-backed tint level across both captured
reference states; it does not establish exact cross-platform color, card-fill,
motion-phase, or light-theme parity.
macOS retains its separate 32% sample layer and calibrated tint. Windows
light-theme overlays use the same
78%-opaque base tint and dark OSD fills across renderer paths so changing the
renderer does not make the material more opaque.
The Windows dark floating controller and in-app title strip use a separate
15.7%-opaque `#151515` tint, keeping the playback controls lighter than the
sidebar. In REF-20, the controller background retains 71.8% of the green test
frame's linear luminance; the previous 33% Windows tint retained 47.4%. A full-
desktop capture of the rebuilt 15.7% tint measured 72.0%, 0.2 percentage points
from the reference sample. The visible bar spans about 458 points on Windows
versus 461 in the 2× IINA capture; its source-defined width remains 460 points.
This verifies the dark flat-frame tint and width only; moving-video blur phase,
controller vertical bounds, text contrast and light appearance remain open.
When the Windows video-backed sidebar material is active, its cards use the WinUI
dark/light `LayerOnAcrylicFillColorDefault` fills (`#09FFFFFF` / `#40FFFFFF`);
other platforms and opaque fallback states retain their prior card colors. This
preserves video visibility in the light Windows theme while keeping card hierarchy.
Quick Settings audio controls keep editable values legible over those cards:
their delay input, profile selector, and external-track action use theme-aware
WinUI-style translucent control fills on Windows Acrylic, and audio headings,
range labels, and disabled actions follow the active foreground theme. The shared
and opaque paths retain ordinary light/dark control fills. A live dark Windows
capture confirms the sampled Acrylic remains visible over playback. The Audio
Equalizer follows IINA's vertical sequence: an 18-point heading, a full-width
23-point profile selector, a 120-point fader bank, and separate 10-point frequency
labels. Spacious/compact card padding follows the source's 12/10-point values and
its row spacing follows 12/8 points. The complete fader labels are visible after
scrolling at the current 619-point player height. The capture is not pixel-scored;
light appearance and material timing remain open. The selected
track marker, Layout switches and selectors use the user's Windows system accent
color on both Windows renderer paths (with `#0078d4` as its unavailable-system-color
fallback), as do playback progress and selected playlist markers; macOS retains
its existing blue palette. The Windows value comes from
[UISettings.GetColorValue](https://learn.microsoft.com/en-us/uwp/api/windows.ui.viewmanagement.uisettings.getcolorvalue)
and refreshes during playback. Selected labels use the higher-contrast black or
white foreground for that accent. Subtitle lane
segments, delay input, and the subtitle-load action follow the same platform-aware
control fill and foreground rules; selected lane semantics remain distinct from
the translucent hover state. Primary and secondary subtitle tracks, the load/search
actions, timing controls, and style rows use separate reference-matched rounded
cards. `Search Online…` opens Riina's existing subtitle-search window. The Video pane keeps the track selector,
aspect/crop/rotation controls, speed, hardware decoding, and image adjustments in
separate rounded cards. Its selected presets use the platform accent; unselected
controls and text inputs use active-theme WinUI Acrylic fills on Windows, with
readable light-theme foregrounds. These video card and control tokens still need
a matched Windows capture. Aspect-ratio and crop preset rails retain an 8-point
chevron slot with a 4-point gap; the chevron appears only at either scroll edge,
edge-to-edge scrolling animates over 300 ms unless Disable Animations is on, and
vertical-only mouse-wheel motion over overflowing rails maps to horizontal scroll.
Video-equalizer reset controls use an original filled circular reset-arrow vector
to follow IINA's `arrow.counterclockwise.circle.fill` role without bundling Apple
artwork. At the native REF-24 capture scale of 2×, the equalizer's label origin
to slider origin measures about 65 points. A matched 3× Windows capture measures
about 76 points; the fixed 65-point label column caused most of that shift. Riina
now uses a 56-point label column with IINA's 8-point row gap, and a 16-point
reset-action slot. After the adjustment, UI Automation at 300% confirms a
56-point label, 64-point label-to-slider offset, 14-point row bounds at a
26-point pitch, and a 16-point reset action. The reset center is about 21 points
from the card edge, matching the reference measurement. A usable screenshot is
still needed to verify visible glyphs, material, and rendered alignment.
The REF-24 Hardware Decoding row uses a 16-point CPU symbol, an 8-point gap,
and a default 13-point system label; these put the label 36 points inside the
card. Its switch track spans about 44×20 points at 2×, with a 26×16-point
selected thumb. Riina's previous row omitted the symbol, used 12-point text,
and had a 30×18-point capsule. The row now uses an original chip symbol and the
measured label, track, thumb, and heights. The optional HDR row uses an original
16-point sun symbol, an 8-point gap, a default 13-point label, and the source's
small NSSwitch size: 36×16 points with a 21×12-point selected thumb, measured
from the pinned 11d HDR preference capture. REF-24 contains no HDR row, so a
compatible playback capture is still needed. The Speed card's REF-24 slider track
spans about 246 points at 2×. The rebuilt
300%-scale UIA bounds showed 248 points, so the spacious and compact horizontal
insets each increased by one point. The new 246-point control bound matches the
reference track; a valid post-change screenshot is still needed for its visible
thumb and value-label alignment.
The `Fit Window to Video` action uses Windows Acrylic control fills only on that
same surface: dark mode uses WinUI default/secondary fills (`#0FFFFFFF` /
`#15FFFFFF`); light mode uses tertiary/secondary neutral fills
(`#4DF9F9F9` / `#80F9F9F9`) and dark foregrounds. Its shared/macOS and
opaque-fallback appearance is unchanged.
The Windows Graphics Capture path bilinearly downsamples the mapped BGRA client
surface directly into a sample no larger than 320×180. Its CPU material path
reduces that sample to quarter dimensions, applies the renderer-compatible 5×5
weighted kernel with proportionally reduced tap spacing, restores 320×180, and
adds stable grain before scaling to video bounds. This keeps the broad
sigma-10-equivalent blur while avoiding an intermediate full-resolution RGBA
copy. Material conversion runs on a bounded background worker; Slint image
creation and presentation stay on the UI thread. It does not verify exact
Acrylic pixels, other video colors, AppKit Liquid Glass, or live macOS rendering.

Subtitle color wells open the shared AppKit `NSColorPanel` on macOS, with alpha
selection enabled. Color-panel changes are converted through sRGB and applied to
the current Preferences window as they change; other platforms retain the in-app
RGBA editor. Native panel placement, picker-mode behavior, cancel/save lifecycle,
and rendered subtitle output still require matched macOS verification.

Use AppKit's system font family for macOS text when a component has no explicit
font family, as IINA does with system fonts. Riina obtains that family from
`NSFont.systemFont` and supplies it through a shared Slint typography global to its
player, Settings and auxiliary windows; preserve explicit monospace and user-selected
subtitle/font-picker faces. Compare resolved font, weight mapping, line height and
wrapping against live macOS captures before adjusting recorded point sizes.

For the macOS player window, set `NSWindow.representedURL` to the current local
media URL and use its filename as the window title. This gives AppKit the document
icon and titlebar path menu IINA exposes. Clear the represented URL for network media
and use the stream title. Windows keeps the `filename — riina` title format. Native
titlebar rendering and its document icon still require a live Mac comparison.

The user prioritizes full interface parity first. Windows captures now work again;
an actual General Preferences before/after review exposed a blank 24px folder
button and touching Save to/path text. The original chooser now draws its own
ellipsis/focus ring and keeps keyboard activation; the path has additional spacing.
The floating panel now uses the recorded 12pt macOS 26 corners. These captures
verify those riina corrections, not complete macOS pixel equivalence. Follow the
complete surface inventory in docs/interface-parity-plan.md.

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

The floating controller's default `controlBarPositionVertical` value is the
distance from the bottom edge of the video view to the controller's bottom edge,
normalized by the video-view height. Its default is 0.1; it is not a top offset
or an offset from the outer window frame. Clamp the stored bottom inset to the
source's `video height - 67 - 25` limit for the default position. During a drag,
the source's AppKit bottom-origin rectangle converts to a top-origin range that
keeps the bar at least 25 points below the video-view top; its bottom edge may
reach the video-view bottom. Horizontally, IINA's drag rectangle clamps the
left edge to 10 points and permits the right edge to reach the video boundary.
On Windows, measure against the realized native
host client area after its frame and in-window menu are excluded.
Unless Edge-to-Edge Video is enabled, IINA's `videoViewContainer` spans the
content between the leading and trailing sidebars; the floating and docked
controller bounds therefore follow that reduced viewport. In edge-to-edge mode,
the video container and controller retain full-window bounds under the sidebar.
Normalize saved horizontal drag position against whichever video-container width
is active, not the outer window width.
The full-width floating bar is 460 points wide. Below that viewport width, keep
it horizontally centered while it compresses; apply the saved horizontal
fraction only when the 460-point bar plus its edge insets fit. The Windows client
leaves a one-point edge inset when compressing so the bar cannot be clipped by
the native video bounds.
Hide the floating bar's mute and volume slider when no primary audio track is
selected, matching IINA's `aid == 0` behavior. Preserve the leading spacer when
those controls are hidden so the center transport remains centered under the
trailing toolbar.

## Default macOS playback controller

IINA 1.5.0's standard on-screen controller has previous, play/pause, and next
in its center. Its toolbar button type has no Stop entry, so there is no square
Stop button in the default controller (Stop remains available as a playback
command elsewhere). By default, four configurable toolbar buttons sit to the
right, in this order: Show Plugins Panel, toggle Picture in Picture, Playlist
and Chapters, and Quick Settings. The defaults are
`Preference.ToolBarButton.plugins`, `.pip`, `.playlist`, and `.settings` in
`Preference.swift`. `MainWindowController.swift` builds the center trio and the
toolbar independently; the toolbar actions open the Plugins or Settings
sidebar, open Playlist and Chapters, or toggle PiP. The toolbar can be
customized in preferences, but these four are the pinned default shown by the
macOS reference. Source anchors:
`reference/iina-upstream/iina/MainWindowController.swift:572` builds the
three-button center group; `Preference.swift:864` defines the available toolbar
button types and `Preference.swift:1072` defines their default order;
`MainWindowController.swift:913` builds the right-side controls and
`MainWindowController.swift:2968` routes their actions;
`Base.lproj/Localizable.strings:428-435` provides the button names.

Riina exposes independent visibility switches for all nine IINA toolbar action
types under Interface > On-Screen Controller and persists their order with the
control-bar policy. The default four match IINA's source order; users can move
actions up and down in the list. Like IINA, Live Text only appears when its feature
preference is enabled and the platform reports it available. IINA's drag/drop
editor and matched visual review remain open.

Riina's standard playback controller follows the same center and toolbar action
set. Windows live accessibility inspection confirms the controls are present and
the Stop/Fullscreen substitutions are gone. Its Plugins sidebar is currently a
placeholder; plugin-provided sidebar rendering and same-state macOS capture
comparison remain open. The Windows PiP button opened the compact playback
window in the debug build; the app was relaunched afterward.

Fullscreen Additional Info is source-derived from `AdditionalInfoView.swift` and
`MainWindowController.swift`: the title sits above an 18-point bold monospaced
clock, a 2-by-16-point separator and a 13-point bold battery percentage. The
horizontal stack uses 4-point spacing; the card uses 16-point liquid-glass or
10-point visual-effect corners with 16/8-point content padding. It is anchored
below the title bar, beside the trailing sidebar, and fades with fullscreen UI.
The current original Slint card follows those text sizes, alignment and visibility
rules, but its rendered glass, system clock symbol, exact card height and battery
glyph still need a same-state capture comparison. Riina currently shows local
24-hour time; matching each platform's user-selected short-time format remains open.

The spacious floating controller now uses the REF-20 measured 460×78-point
interior, formed by 14-point top padding, 24-point controls, an 8-point row gap,
a 24-point timeline allocation and 8-point bottom padding. Compact UI uses the
same control and timeline sizes with the upstream 10/5-point vertical insets,
for a 71-point interior. The dark material bounds in REF-20 measure about
460×78 points at the inferred 2× capture scale; round-corner shadow pixels are
excluded. The center transport group follows upstream spacing: 24 points between
24-point-wide controls while floating, and 16 points while docked. The Windows
toolbar now follows `OSCToolbarButton`'s 24×24-point frames with zero
inter-button spacing; compact Windows rows keep the same hit width and constrain
their height to the row. Exact icon silhouettes, sampled blur phase, light
appearance, animations, window controls, full sidebar layout and focus geometry
remain open.
The timeline follows `MainWindowController`'s 46-point minimum centered time
labels, 2-point label-to-slider gaps and 8-point outer inset. Windows compresses
the label minimum to 34 points in its constrained small-video mode.
For constrained Windows viewports, Riina uses a platform adaptation: below
402 points wide or 210 points high, the controller shrinks to 50 points, its
transport row to 14 points and timeline to 16 points. It collapses the volume
slider first, preserves configured toolbar actions at 24-point widths when they
fit, and hides previous/next below 330 points. This keeps playback controls within
a short native video client, but its compact hit targets and relationship to the
pinned macOS controller still need a same-state capture.
The controller now clamps its floating position to the available video viewport.
Below 48 points of height it omits the timeline row; below 330 points of width it
keeps Play/Pause and suppresses the two neighbor actions. A 980-by-330-pixel
Windows resize kept the video visible and the native child aligned, but the test
capture did not expose the controller, so visible button placement at that size
remains unverified.
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

The video controller has a responsive compact state: below 330 logical points it
keeps Play/Pause and the timeline while hiding previous/next; configured volume
and toolbar groups remain hidden until their full hit targets fit. The floating
panel is bounded by the video surface during resize. This is a Windows-specific
usability adaptation awaiting a live check against the reported small-window
case; macOS geometry remains pinned to the IINA baseline.

Music Mode hides the standard title controls and uses a full-width 300-point
compact controller. REF-22's 824×742 screenshot has a 600×518-pixel player frame
at x=112..711, y=76..593. The inferred 2× scale yields 300×259 points. Its
artwork/control boundary at screenshot row 413 divides the frame into about 168.5
points of artwork and 90.5 points below. Riina uses 169 points for artwork and 90
for the controller, matching the total frame within edge rounding. Its previous/
next actions and playlist/artwork toggles are wired to shared playback and
preference state; an original artwork icon is used. Live Windows corner, title,
timeline, and video-backed Acrylic alignment remain unverified.
The hover toolbar has close-player and return-to-player actions.
The playback surface does not add an app-drawn button row above the video.
Playback settings and tracks remain reachable from the on-screen controller's
gear; the top inset and settings-pane placement collapse with that row on all
platforms. macOS identifies local media using its filename and native document
titlebar; Windows identifies it with the filename alongside Riina.
The Music Mode playlist keeps single-click selection separate from double-click
playback.
The compact volume control opens a slider popover, with mute available inside it.
Music Mode artwork area height follows embedded-art dimensions for audio and
display aspect ratio for video, defaulting to square when unavailable. The art
toggle hides or reveals that reserved upper viewport. Audio without
embedded art or visible subtitles receives an original Riina placeholder. When
the controls hide, the title and artist/album move into the lower panel while
the timeline stays visible with a 200 ms cross-fade. Source materials,
hover/focus states, live transition and popover behavior, native-video viewport
behavior, and a matched macOS mini-player capture remain open.
On Windows, the top-level app menu is inside the player window. Hide it only in
Music Mode, where the 300-point width makes it wrap over the controls; the compact
toolbar keeps playlist, artwork, Quick Settings and exit actions available. Restore
the full menu when normal playback resumes. macOS keeps its menu bar outside the
player window and retains the source compact height.

The supplied Riina identity is used for the welcome-window mark, README wordmark
and Windows executable icon. See assets/branding/BRAND_GUIDE.md for capitalization
and asset rules. Branding does not recolor or reshape playback controls, whose
appearance continues to follow the pinned IINA reference. Windows shell rendering
of the embedded icon has not yet been checked on a built application.

The shared seek panel now uses estimated geometry from the reviewed paused-seek
capture: an upper-left rounded dark panel, bright elapsed clock, subdued total
clock and blue progress indicator. Values come from observed playback, triggered
by a native seek completion counter rather than the requested destination. The
one-second timeout follows the pinned Preference.swift osdAutoHideTimeout default
(line 1088). Seek, video, and screenshot overlays fade out over the source's
0.5-second OSDAnimationDuration; disabling animation or global OSD uses an
immediate transition. The clock layout explicitly reserves the panel's 16-point
left/right padding. Typeface, material, exact metrics, and event-relative timing
remain open comparison gates. No reference pixels or protected assets are shipped
in the UI.

Font chooser geometry is source-derived from the pinned
Base.lproj/FontPickerWindowController.xib: 458pt width, 12pt outer margins,
24pt filter, a 268pt two-table block with 23pt headers, 36pt editable preview
before the custom font field, 24pt custom input, 80pt buttons separated by 12pt.
Labels are Choose a font:, Family, Typeface, Preview and Or enter the font name:.
The tables meet at a shared seam upstream; riina now removes its previous 12pt
gap but has not reproduced the native border overlap. The preview caption sits
inside the field near its upper-left edge. Search/table and table/preview gaps
are 8pt; preview/custom label gap is 12pt; custom/button minimum gap is 16pt.
The XIB records contentRect height 524 but contentView height 496; matching live
reference execution must resolve the actual client height. Riina retains 524
and flexible space before buttons, rather than claiming that discrepancy closed.
These are compiled source allocations, not measured rendered equivalence.
System colors, row height, selection material, table headers, font metrics,
focus rings, theme behavior and screenshots remain open visual gates.

Font table rows now use an original shared renderer with a 19pt pitch suggested
by the XIB's verticalLineScroll value. That value is a scroll increment, so this
pitch remains an estimate until native row measurements are available. Labels
use provisional 13px system text, flat dark background, blue selection and a
keyboard cursor outline. These replace the default toolkit rows to support
native double-click callbacks; they do not close table-material or pixel gates.

Subtitle Preferences / ASS group (source-derived from the pinned
`PrefSubViewController.xib`): the ASS section is 99 points high. Its source
container begins 117 points from the section's leading edge and is 89 points
high. In the container, the Primary/Secondary segmented control is 304 × 21 at
(45, 53), the five-tick override slider is 124 points wide at (133, 28), and the
level and explanatory labels share the slider row and lower inset. XIB frame y
coordinates use AppKit's bottom origin. Riina now uses the source's visual order
(lane selector, override level, explanation) and the same slider ordering
(`no`, `yes`, `scale`, `force`, `strip`) with a 99-point section. Windows
rendered metrics, materials, interaction and matching macOS pixels are still
unverified; these allocations are not a pixel-parity claim.

Subtitle Preferences / Text Subtitles (source-derived from the pinned
`PrefSubViewController.xib`): the section is a 480 × 319 point view. Its Font box
is 366 × 105 at (117, 206); Border is 366 × 56 at (117, 146). Shadow and Other
Styles sit below the folded Advanced disclosure, then reveal on demand. Riina
keeps the 366-point box width inside a 480-point section and scrolls the expanded
content. The font-size, border-size, shadow-offset, blur and spacing fields use
the source's three-fractional-digit number format. Current Windows rendering and
matched macOS pixels remain unverified.

Subtitle Preferences / Position (source-derived from the pinned
`PrefSubViewController.xib`): the section is 480 × 208 points. Its Align box is
366 × 56 at (117, 144) in the XIB's bottom-origin coordinates. The saved defaults
come from `Preference.swift`: centered X alignment, bottom Y alignment, vertical
position 100, margins X 25 and Y 22, with letterbox display and window scaling
enabled. A Windows capture shows the complete controls with bordered numeric
fields and these defaults. The capture verifies the current rendering only;
matched IINA metrics, system control materials, numeric formatter edge cases and
matching Windows/macOS pixels remain unverified.

Subtitle Preferences / Auto Load (source-derived from the pinned
`PrefSubViewController.xib`): the section is 480 × 193 points. The mode popup is
255 × 25 at (117, 161) in bottom-origin coordinates; its options are Disabled,
Subtitles containing media filename, and Detect intelligently by IINA. The
Advanced disclosure sits above a 360 × 132 stack with the priority-keyword field,
helper text, and search-path field. Defaults are IINA matching, no priority terms,
and `./*`. Riina keeps those controls at the corresponding source order and uses
the same principal allocations; advanced field typography and expanded-state pixels,
live behavior, and matching macOS captures remain open.

## Captured IINA Settings reference

The pinned IINA v1.5.0 Settings window has eleven captured pages at 1x in Light
appearance on macOS 26.6.2. Its outer frame is 608 x 500 points; source-derived
client geometry is 600 x 480, with a 200-point navigation split. The sidebar
shows a rounded Search field, page symbols, selected-row backgrounds, indented
section links and a vertical scroll indicator. Page labels are General,
Interface, Video, Audio, Subtitles, Network, Control, Key Bindings, Advanced,
Plugins and Utilities. General shows section headings above rounded bordered
cards, with 40-point rows and 10-point card corners. These captures are research
references from the official disk image, not matched Windows screenshots or
accepted comparison goldens. Riina now declares a 600 x 480 Settings client area
with a 200-point sidebar, matching the measured reference geometry. That is a
source-level dimension match; no fresh Windows Settings capture has verified its
rendered outer frame or content alignment. The sidebar has original geometric
page symbols, selected-row styling and a live Search field with grouped results
for currently implemented controls. Search supports arrow-key selection, Return
activation and mapped item-position scrolling, but does not cover the full IINA
settings index or highlight an exact row. The Network connection section is
source-derived from the pinned SettingsPageNetwork and legacy XIB, not visually
captured. These icon shapes are authored independently and recorded in
`assets/provenance.json`; their rendered scale and spacing still need matched
comparison. Pixel samples from the Plugins capture show the navigation surface at
approximately `#f6f6f6` and the content surface at `#ffffff`; inset lists use a
slightly shaded fill with a thin gray outline. Riina now uses those base surface
colors while retaining its rounded, bordered cards. This source-pixel correction
has not been validated in a live Windows render. See
`IINA-1.5.0-SETTINGS-SURFACES-001` in the reference-capture manifest before
changing those layouts.

The pinned General capture shows compact disclosure rows between At launch and the
Opening media card. Riina's General Behavior page now uses the same 40-point row
rhythm; When media is opened and Pause/resume when collapse by default, and the
Opening media card uses right-aligned 36 × 18-point switches in the captured row
order. Fullscreen entry/exit preferences now apply at the fullscreen action. The
retained Windows `interface-after-preferences-0.jpg` predates the current 600 × 480
Settings window, and this host has no desktop app surface for a fresh capture. These
source-measured changes remain unverified against a current Windows render; the
608 × 500 pinned macOS capture remains the reference.

Interface > Window uses four separate rounded lists after Appearance: expandable
Initial window size and Initial window position switches; resize mode and its
three timing choices; physical-resolution behavior; and two float-on-top options.
The size detail is one horizontal Width/Height, value, and unit row. Position
details follow the source's four-row X/Y offset and screen-edge arrangement.
Riina now represents these controls and derives later card positions and scroll
anchors from their expanded heights. This is a layout implementation step, not
evidence of pixel parity: live expanded-state capture, exact card and row metrics,
keyboard focus appearance, and matched Windows/macOS pixels remain open.

The 2026-10-11 macOS capture places the Appearance heading above a three-row,
122-point rounded list starting at y=81 in the captured client. The Window heading
follows, with its two expandable switches in an 82-point default list starting at
y=248. Resize mode and its three radio choices occupy a separate 118-point list
starting at y=346; physical resolution is its own single-row list. Riina now follows
that default heading/card hierarchy and the measured first-card positions and
heights, and recomputes later positions when geometry details expand. The captured
macOS Liquid Glass row is represented visually and exposes its three per-surface
switches. Native material rendering and matched macOS 27.0.1 comparison remain open.
Windows uses the platform-specific “Translucent Surfaces” title, with Mica/Acrylic
system backdrops on selected windows and sampled-video fallback materials during
playback. The Windows overlay checks `UISettings.AdvancedEffectsEnabled`, Battery
Saver and High Contrast and uses opaque fills while system materials are disabled;
cached policy refreshes during playback. Sampled Acrylic grain stays fixed in
surface coordinates instead of changing on every video frame. The persistent
player host clears and restores Mica when that policy changes. Registered
auxiliary windows now share a weak-handle policy monitor and clear/restore their
Mica or Acrylic DWM attribute as system policy changes. The Windows Settings
page and navigation bases use a 50% light content fill over Mica only while DWM
accepts Mica; cards retain their solid fills. This follows the [Windows Mica
layering guidance](https://learn.microsoft.com/en-us/windows/apps/design/style/mica).
Runtime rendering, policy transitions and per-surface comparison remain open: the
latest Windows desktop capture returned a black frame.

Windows selected controls and keyboard-focus outlines use the current system
accent, including player tools, Quick Settings presets, sidebar tabs, delay inputs,
and track rows. Quick Settings category tabs keep IINA's neutral glass capsule on
both platforms; Dark/Light/Auto and actual presets use platform accent selection
with contrast-aware labels and icons. Focus outlines retain their lighter IINA
blue on macOS. The Windows fallback is Windows blue when the system color is
unavailable.
Confirm these states against a matched Windows capture, especially with light
themes and high-contrast accent colors.

Network > Online Videos (yt-dlp) is implemented as the source's two stacked
rounded lists: the Online Media recommendation and the extractor switch with
custom search-folder and raw-options details. The pinned source identifies the
switch as enabled by default and requires a restart for the custom folder. The
Windows surface has not been captured in this state; row metrics, plugin-state
copy, disabled styling and matched reference pixels remain open.

The captured Control page (Settings surfaces capture, page 07) begins with a
Trackpad card containing Pinch to and Force Touch to, followed by a Mouse card
with single click, double click, right click, middle click and first-click-focus
options. The next card assigns vertical and horizontal scrolling; the final card
contains Seek type and seek, speed and volume sensitivity. At the captured 608 x
500 outer size the latter controls continue below the initial 480-point client and
use the page's vertical scroll view. These source-visible order and defaults guide
the new shared Control page; its rendered Windows metrics, state changes and
matched macOS pixels have not yet been reviewed.

The pinned macOS Settings capture for Key Bindings (page 08) shows a 600 × 480-point
client area. The page has a rounded system-media-control row, then a rounded profile
card with a profile selector, delete and keyboard buttons, a search field, a raw or
lock control, and a vertically scrolling action/shortcut table. Rows use a text
label on the left and a compact outlined key-cap on the right. Riina now implements
that one-column hierarchy and the first seven visible default mappings. Its complete
Windows render, selected-row behavior, source-specific keycap metrics, full maps and
matched pixels remain unverified. The legacy XIB retains profile lifecycle and
table-edit behavior not fully visible in the captured state.

The pinned Utilities capture (page 11) uses the same 600 × 480-point client and
200-point sidebar. Its scrollable content orders Default Application, Restore
Alerts, Clear Cache and Browser Extensions as separate gray section labels above
rounded bordered lists. The cache list groups saved playback progress, playback
history and thumbnail cache rows; long descriptions wrap beneath the row title,
while compact trailing action buttons and cache size labels align to the right.
Riina now follows this section and row hierarchy, and the recent-media history
row has a confirmed clear action. Thumbnail clearing remains disabled because
there is no application-owned thumbnail cache yet. Native action surfaces,
lower-page scrolling and Windows pixel comparison remain to verify.
At the captured 1x scale, the three cards begin at y≈81, 168 and 290 points from
the image top. The Clear Cache card is about 202 points tall, with row heights near
87, 74 and 41 points; Browser Extensions begins below the first viewport at y≈514.
Trailing action pills are about 40 × 26 points with a 10-point card inset. These
are screenshot measurements, not toolkit layout constants from upstream.

The pinned Advanced capture (page 09) uses the same 600 × 480-point client and
200-point sidebar. A single advanced-settings card sits first with the flask glyph,
help affordance, switch and two-line restart note. Logging has a two-row card and
a separate log-window action row. mpv Settings follows with compact OSD and
configuration-directory cards; the latter keeps its path chooser and switch on
one line. Additional mpv options continues below the initial viewport in a
120-point editor with a two-column table and add/remove controls. Riina now follows
that order and compact row hierarchy with original vector symbols. The current
render, in-app log window, editor drag/paste/focus behavior, scroll and matched
macOS pixels still require direct comparison.

## REF-21 Layout icon metrics

The pinned `SidebarLayoutPane.swift` uses 20-point symbols for the Theme,
Compact Interface and video-setting rows; the Liquid Glass heading and its three
mini rows use 16-point symbols. Riina's `SidebarPositionToggle` previously fixed
every icon at 16 points while the Theme and Liquid Glass headings both used 18.
It now supports an explicit icon size: the Theme symbol is 20, the Liquid Glass
heading is 16, and Compact Interface plus the video-setting rows are 20. Mini
material rows and unrelated sidebar controls retain 16. REF-21's Theme, material,
OSD, OSC, sidebar, compact-interface, edge-to-edge and aspect-lock symbols now each
have a purpose-drawn Riina vector at the source-defined 16/20-point size. Their
earlier generic window, gear, message, mouse, list and filmstrip symbols did not
match the source meaning. Fresh rendered bounds are recorded in the capture
manifest; exact path contours remain a platform-specific approximation because
Apple artwork is not bundled.

## REF-21 Quick Settings tab icons

`QuickSettingViewController.swift` assigns paintbrush.fill, photo, waveform,
and captions.bubble.fill to Layout, Video, Audio, and Subtitles. `SideBar.swift`
renders those symbols at 14 points with bold weight. Riina uses independently
drawn equivalents at 14 points: paintbrush, landscape photo, waveform, and caption
bubble. The SVGs carry MIT provenance; no Apple artwork is bundled.
IINA's selected Layout label measures 78×24 px in the pinned 2× crop; Riina's
12-point label measures 76×22 px in the same-scale rebuilt capture. The selected
segment now derives its width from the text's preferred width plus icon, gap,
and padding, while icon-only segments use 36 points. This adapts to the shorter
Video label; live selected-state pixels still need a matched capture.
The REF-24 crop shows the selected symbol and label sharing a visual centerline.
In the Windows REF-21 capture the symbol sat above the label's visual center
inside the selected pill. The tab's inner horizontal layout now centers both
items on its cross axis. A rebuilt playback window opens, but Quick Settings
could not be activated through the available desktop-control bridge; fresh
rendered alignment remains unverified.

The Layout pane's three On Screen Controller choices use 66×74-point items with
8-point horizontal gaps (214 points total). Each item has an 8-point corner
radius, a 2-point selected outline and a 1-point inactive outline. Its preview
uses a 40×28-point viewport around an inset 32×25-point visible frame, followed
by an 8-point label gap and 12-point item padding. Windows keeps the inactive
stroke neutral and theme-aware while the selected outline follows the system
accent; post-change pixels remain pending.
The tile row also retains IINA's 8-point bottom inset before the next
18-point section gap, placing the “Sidebar Position” heading 26 points below
the tiles. Post-change pixels remain pending.
The Sidebar Position container has an additional 8-point trailing content inset
after its material card. Its three rows now use 24-point controls, the source's
10/6-point spacious/compact stack spacing, and 12/10-point spacious/compact
container padding. Derived card heights are 118/106 points, including its
one-point outline on each edge. This follows `SidebarLayoutPane.swift`'s
`sidebarSettingsSpacing` and `sidebarContainerPadding`; live pixel alignment
remains open.
The two section captions use IINA's bold 12-point system label. Riina's prior
11-point text measured about 16 px high in the matched-scale crop versus about
19 px in IINA; the shared caption style now uses 12 points. The rebuilt glyph
and separator alignment remain to verify.
