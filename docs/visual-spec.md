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
Linux uses its native backdrop material when the compositor exposes one and
otherwise uses the documented in-app fallback.

The current Windows Slint controls only switch RGBA tint/opacity. They are
captioned “Translucent Surfaces” while native Mica/Acrylic composition is not
implemented. The three per-surface switches remain a visual approximation until
each Windows surface is connected to an appropriate compositor material; the
macOS control keeps its “Liquid Glass” label and must be verified on macOS 27.0.1.
Platform references: [Apple Materials](https://developer.apple.com/design/human-interface-guidelines/materials),
[Windows Mica](https://learn.microsoft.com/en-us/windows/apps/design/style/mica),
and [Windows Acrylic](https://learn.microsoft.com/en-us/windows/apps/design/style/acrylic).

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
| sidebar width | 360 | Preference.defaultPreference |
| sidebar width limit | 80% window width | SidebarController.sidebarMaxWidth |
| sidebar side | playlist/chapters leading; settings trailing | Preference.defaultPreference |
| track rows | 26 points | SidebarTrackSelector.tableView |
| playlist rows | 30 spacious / 26 compact | SidebarPlaylistPane LayoutValue |
| chapter rows | 40 points | SidebarChaptersPane.tableView |
| auto-hide | 2.5 s | Preference.defaultPreference |

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

The floating controls now use the recorded 67-point outer height, 460-point width,
14/8 vertical insets and 8-point row separation. The 17-point button row is an
independent layout allocation pending reference capture measurements. A native
Windows titlebar, estimated typography, original vector playback icons and tint remain. This
is not pixel parity or video-sampled blur. Replace toolkit sliders and glyphs with
measured custom geometry after live baseline captures. Light/compact/docked layouts,
window controls, full sidebar layout, animations and focus measurements remain incomplete.
For constrained Windows viewports, Riina uses a platform adaptation: below
402 points wide or 210 points high, the controller shrinks to 50 points, its
transport row to 14 points and timeline to 16 points; optional side controls stay
collapsed until they fit. This keeps playback controls within a short native video
client, but its hover state and relationship to the pinned macOS controller still
need a same-state capture.
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
compact controller. Its previous/next actions and playlist/artwork toggles are
wired to shared playback and preference state; an original artwork icon is used.
The hover toolbar has close-player and return-to-player actions.
The playback surface does not add an app-drawn button row above the video.
Playback settings and tracks remain reachable from the on-screen controller's
gear; the top inset and settings-pane placement collapse with that row on all
platforms. The native window title identifies the current playlist filename
alongside Riina.
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
Windows uses the platform-specific “Translucent Surfaces” title until Mica/Acrylic
backdrops are wired into the native window and video-backed surfaces.

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
