# Source-derived visual specification

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
one-second timeout follows the pinned Preference.swift osdAutoHideTimeout default
(line 1088). The capture proves a visible state, not its duration. The clock
layout now explicitly reserves the panel's 16-point left/right padding.
Typeface, material, exact metrics, animation and event-relative timing remain open
comparison gates. No reference pixels or protected assets are shipped in the UI.

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
