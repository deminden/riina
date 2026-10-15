# IINA visual reference audit

This audit tracks the imported visual references against Riina without reducing
the work to a parity percentage. Individual observations and SHA-256 hashes are
in [visual-reference-audit.csv](visual-reference-audit.csv).

## Reference set

- Baseline: IINA 1.5.0 on macOS 27.0.1, captured on an M2 MacBook Air (confirmed
  by the user).
- Display scale was not recorded. Do not infer pixel-to-point dimensions as
  settled until a matched capture is available.
- Preference references use the light appearance. Playback references use the
  dark translucent playback surface; several use a green test-pattern video.
- Files are kept outside Cargo's generated `target` directory at
  `reference/captures/iina-reference-set-2026-10-12/`. The imported README lists
  `00-start-window.png`, but that file was not included in the set.
- Captures 09a/10a/15b are near-duplicates of earlier captures; 13b is a failed
  capture. They remain in the set and are marked in the ledger.
- `reference/new/` contains 20 full-screen captures: 18 menu states and two
  contextual windows. REF-26–REF-45 index them in capture order and record the
  visible state and SHA-256. Keep the originals local; the directory is ignored
  by Git. These screenshots also contain unrelated desktop content, including
  chat history, download names, device names, notifications, and Dock icons.
  Do not copy the raw images into published evidence. Crop to the relevant app
  surface and remove unrelated content before sharing a comparison.
- Use the menu sequence by family: REF-26–34 cover application, File, Edit,
  Playback, and Video geometry; REF-35–39 cover Audio and Subtitles; REF-40
  covers Plugins; REF-41–42 show macOS Window tiling; REF-43 shows Help. REF-44
  is a native Open panel with a floating PiP controller and other windows behind
  it. REF-45 is the Welcome window over the same unrelated desktop context.
  Treat the last two as behavioral/context evidence only, not clean pixel
  references.
- The selected audio devices, subtitle providers, installed plugin actions,
  checked rows, current track names, and recent files are runtime state. Treat
  these captures as examples of dynamic menu population, not a complete fixed
  inventory. The encoding submenu is scrollable; the capture shows only part of
  its list. Edit and Window tiling items are macOS-owned surfaces and should be
  handled through platform APIs.
- All 20 files are 2940 × 1912 pixels. Display scale was not recorded; do not
  treat pixels as logical points. The menu screenshots place translucent menus
  over a vivid green test pattern, so use them to compare labels, ordering,
  hierarchy, checks, and menu geometry. Compare material color and translucency
  over representative video content instead.

### Source map for REF-26–REF-45

The menu skeleton and command labels are in
`reference/iina-upstream/iina/Base.lproj/MainMenu.xib`. Dynamic menu contents,
selection marks, and shortcut/state updates are managed in
`MenuController.swift`; player command dispatch is in `MainMenuActions.swift`.
The File menu's `NSOpenPanel` and recent-document registration are in
`AppDelegate.swift`; the Welcome window is implemented by
`InitialWindowController.swift`, which builds its window programmatically and
uses the localized `InitialWindowController.strings` table for text. Plugin-specific
menu contributions and developer actions are in `MenuController.swift`,
`PluginViewController.swift`, and `JavascriptDevTool.swift`. macOS Edit and
Window tiling items include OS-owned behavior; follow AppKit semantics rather
than treating every visible row as app-owned functionality.

REF-30's Video menu shows Half Size, Normal Size, Double Size, Fit to Screen,
Bigger Size, and Smaller Size. The pinned `MainMenu.xib` confirms those labels.
It does not show “Fit Window to Video”: that is a Layout sidebar action in
`SidebarLayoutPane.swift`, so the current Riina Video-menu entry is an extra
menu placement rather than evidence for IINA menu parity. Keep that distinction
when closing the Video-menu gap. REF-41–42 should likewise guide platform
delegation, not custom menu styling in Riina.

The File menu's “Show Current File in Finder” action is confirmed by the pinned
`MainMenu.xib`. Riina now connects the equivalent platform-named action to its
existing playback snapshot and platform reveal adapter on both shared and
Windows-native menu surfaces. It follows IINA's local-versus-network enablement
rule. File > Close now dispatches through the existing player lifecycle and
last-window policy. “Save Current Playlist…” now exports the ordered playlist
as UTF-8 M3U8 through the platform save dialog. “Delete Current File” now moves
the local current item to the OS Trash/Recycle Bin and removes its matching
playlist identity after a successful move. Live delete, file-manager selection,
save, and close behavior remain to be checked against a running player. File >
Playback History now opens a reusable window with date or folder grouping,
search, progress, play/reveal, history removal and file-to-bin actions. Its
dimensions and source-derived columns follow `HistoryWindowController.swift`;
the toolbar, native row icons, multiselection, expand/collapse controls, exact
date formatting and matched pixels remain open. Live history-window behavior
also remains to be checked against a running player.

## Progress log

### 2026-10-12

- Inspected references 09 through 25 individually and recorded visible controls,
  capture defects, duplicate status, current implementation evidence, and next
  comparison action per image.
- Adjusted General page title/card insets and launch row alignment against
  `09-preferences-general.png`; renamed the visible multiple-window control to
  IINA's wording while preserving its existing setting.
- Repositioned Key Bindings “Display raw values” from a header icon into a
  labeled footer switch separated from the action rows, following the captured
  IINA layout. The action is still available in the custom profile menu.
- Changed General preferences from subsection-only views to a continuous scroll
  page, moved the main category navigation back to IINA's positions, and routed
  General search results to section scroll offsets. The reference ledger keeps
  the missing History and Playlist controls open until their behavior is wired.
- Added the playback controller's gear action to open Quick Settings on the
  remembered tab; Video remains the initial tab, matching IINA's default. The
  gear shows active feedback while Layout, Video, Audio, or Subtitles is open.
  On 2026-10-12, the Windows debug app loaded and played the reference test GIF
  (960×540 at 4 fps). Quick Settings opened to Video; the live Layout pane was
  compared with REF-21 and lacked Theme and the three Liquid Glass switches.
- Added Dark/Light/Auto choices and the On-Screen Display, On-Screen Controller,
  and Sidebar material switches to Layout. These use existing theme and surface
  preferences and save through the Layout settings path. The material heading
  is now platform-aware: Liquid Glass on macOS and Translucent Surfaces on
  Windows/Linux. The Windows controls currently change app-rendered tint/opacity;
  native Mica/Acrylic integration is now tracked in the implementation plan. The 0.3.3 debug app
  built, and a live Windows capture shows all controls in the scrollable Layout
  pane while the test clip is paused. Formatting, app check, and strict workspace
  Clippy passed. REF-21 remains open for interaction/persistence checks and a
  matched comparison of panel sizing, material, spacing, and the remaining tabs.
- `cargo check -p riina --locked` completed successfully after the Key Bindings
  layout change. No matched Riina screenshot was available, so the visual change
  remains pending live capture confirmation.
- Removed the custom player title row on every platform to match REF-20/21, where
  playback has no second row above the video and settings remain reachable through
  the OSC gear. Its video inset and Quick Settings pane offset collapse with the
  row. A live hover report found the row was still visible in the Windows debug
  player; the row is now explicitly disabled in the shared UI. The rebuilt
  Windows player confirms the row stays absent with controls visible, the current
  playlist filename appears in the window title, and the bottom gear opens Quick
  Settings. macOS runtime confirmation remains open.
- Traced the REF-20 playback controller to IINA 1.5.0's
  `MainWindowController.swift` and `Preference.swift`. The central OSC has
  previous/play/next and no square Stop control; its default right toolbar is
  Plugins, PiP, Playlist and Chapters, then Quick Settings. The reference ledger
  records this source-derived controller structure. Riina's standard Windows
  playback row now has no square Stop or Fullscreen action and exposes those four
  toolbar controls in the same order. A live Windows accessibility capture at
  1026x627 showed the four labels and center transport controls together. The
  Plugins action is wired to a leading sidebar placeholder; rendering plugin-provided
  content is still missing. Clicking PiP opened the compact playback window in
  the Windows debug build; the player was relaunched afterward. Playlist and
  Plugins interaction behavior still needs direct verification, and a macOS
  runtime capture remains open.
- Measured the floating controller against IINA 1.5.0's `OSCFloatingView.swift`:
  460 pt preferred width, 200 pt minimum, 14/8 pt spacious top/bottom padding,
  10/5 pt compact padding, 12 pt top-row side insets, 8 pt timeline side insets,
  and an 8 pt gap between the top row and timeline. Riina already matches the
  spacious values; compact mode used a 59 pt height and 6 pt inter-row gap. It now
  uses the source-derived 60 pt height and 8 pt gap. Small-video compression stays
  a platform adaptation so transport controls remain inside a constrained video
  viewport. The workspace build and formatting check pass. The full-desktop capture
  path currently returns a uniform black image, so post-change rendered bounds and
  sampled Acrylic pixels remain open.
- Used REF-29's Playback menu and nested Playlist entry as implementation
  evidence. The Playback menus now name the action “Stop and Clear Playlists”
  (the pinned mpv `stop` command clears the playlist), and expose live Playlist
  and Chapters submenus with checked current entries. Playlist activation uses
  the observed playlist order; chapter activation uses the shared seek-and-
  resume command. The workspace compile passes after the Video-menu correction.
  Live menu interaction and exact menu styling remain open.
- Implemented REF-09b's two Open Recent controls with IINA's default-on values,
  dependent setting, a persistent recent-document list, and a File > Open Recent
  submenu with reopen and clear actions. Manual media-picker selections are
  recorded when enabled; successful loads are added when all-played tracking is
  on. The Windows workspace compile passes. AppKit recent-document integration, live Windows
  interaction, restart behavior, and matched History-page geometry remain open.
- The first REF-30 pass led to Video-menu wiring for track and window controls.
  The source recheck below corrects its scope: “Fit Window to Video” is a Layout
  sidebar action, while the six IINA size commands, Delogo, Video Filters, and
  Saved Video Filters remain missing from Riina's Video menu. The Windows
  workspace compile passes; live interaction and matched menu captures remain
  unverified.
- Re-reviewed the 20 `reference/new/` captures against their indexed files;
  all 20 ledger paths exist and their SHA-256 hashes match. Organized the set as
  18 menu states plus two contaminated contextual windows, called out volatile
  menu data and privacy limits, and separated AppKit-owned menus from app-owned
  commands. REF-30 and `MainMenu.xib` show that the Video menu has six explicit
  size commands; “Fit Window to Video” is a Layout sidebar action and its
  placement in Riina's Video menu is an extra. The six size commands are now
  wired through the portable window-resize policy; delogo and filter parity
  remain open. The existing menu work for panel/tracks, PiP,
  fullscreen, Float on Top, aspect lock, Live Text, and Music Mode remains
  compile-checked, with live menu interaction and matched captures open.
  The top-level menu sequence matches IINA. File, Playback, Video, Audio and
  Subtitles now follow the source grouping/separator order for implemented
  entries, with alternate playback-speed, volume and delay steps represented.
  Slint's menu item layout puts assigned shortcut equivalents in the trailing
  column, using the platform primary modifier. Supported Audio, Subtitle, and
  Plugin actions are wired; unavailable commands remain disabled. File > Open
  URL now opens a focused, centered URL-entry dialog from either player menu
  surface, with IINA's Shift+Command+O shortcut and the Windows Control equivalent.
  Open in New Window is source-wired to the native multi-file picker and separate-
  player path for a prepared media batch; live selection/startup remains unverified.
  Exact Video sizing interaction, Audio device/filter controls, full Subtitle controls,
  working Edit/Window/Help actions, user-customized shortcut synchronization,
  and matched pixel captures remain open.
  A live Windows playback window confirms all ten headings at maximized width
  and Ctrl+O/Ctrl+Shift+H in the File shortcut column. At 320 px mini-player
  width the embedded bar wraps into two rows, which remains a compact-window
  layout gap.

### 2026-10-13

- Opened the Windows 0.3.3 debug app and played the short workspace fixture. The
  playback title included the active filename; the visible controller had no
  square Stop action and showed Plugins, PiP, Playlist and Chapters, and Quick
  Settings at the right. The top menu row appeared once in IINA's configured
  order. The clip advanced through its short playlist during inspection, so the
  menu and Quick Settings popovers were not verified in this pass. This is a
  grouped live smoke check, not a matched-scale reference comparison. Closed the
  tested app and confirmed there was no remaining Riina window or process.

### 2026-10-14

- The first REF-21 review conflated the selected Layout category with the selected
  Dark appearance segment. Pixel reinspection shows a neutral glass capsule behind
  Layout and a blue fill behind Dark. REF-24 shows the same neutral capsule behind
  Video. Riina's Windows category tab had used the system accent, unlike the pinned
  reference; it now uses the neutral theme capsule while actual preset and appearance
  choices retain their accent selection. The live Windows capture is still needed
  to verify the changed pixels.
- Audited both Windows video-material paths. Native D3D11 blurs a 320-pixel frame
  sample with sigma 8, while shared GL used taps spaced three reduced-texture
  pixels apart; at 1080p that was roughly four times sharper. Windows GL tap
  spacing now follows the 320×180 native sample bounds and source aspect ratio,
  matching sigma 8 independently on both axes. macOS keeps its prior GL spacing.
  Available Windows captures predate this change, so rendered blur-radius
  comparison remains open.
- Rechecked the pinned IINA `SideBar.swift` against REF-21. The sidebar dismiss
  control is a 36 pt circular glass-style button (28 pt in compact mode), using
  a backward chevron on a leading sidebar and a forward chevron on a trailing
  sidebar. It sits after the tab group on the leading side and before it on the
  trailing side. The tab group and dismiss button use equal spacing across the
  header width. Riina used an X at the far right in both cases. Replaced that
  control with original Riina chevrons, matching source ordering, equal spacing,
  and size, and applied WinUI control-layer fills and a subtle edge only over the active
  Windows video-backed Acrylic surface. This is source/layout-derived; no fresh
  Windows render is available, so the visual close remains open for live review.
- Measured the Layout cards in REF-21 against its approximately 720-pixel sidebar.
  The screenshot scale is not recorded; 2 px per point is inferred from IINA's
  360-point default sidebar width. Screenshot-derived Theme, Compact Interface,
  and two-row Video card heights are approximately 150, 42, and 74 points, with
  roughly 20-point gaps. The pinned `LayoutValue.swift` and
  `SidebarLayoutPane.swift` define the exact source rhythm: outer spacing 18/16
  points, side margins 16/14 points, inner padding 12/10 points, item spacing
  12/8 points, and per-surface switch spacing 6/5 points. The Windows pane now uses
  152/138-point Theme/material cards, 44-point Compact Interface cards, and a
  76-point two-row Video card; full-desktop captures show both spacious and compact
  layouts without clipping. Its mini switches use 36×16-point tracks. The captures
  confirm visual fit, not pixel-exact edges or the inferred 2 px/point scale.
  REF-21's native-resolution crop measures the selected Layout capsule at about
  191–192 points wide (2 px/point inferred from the 360-point sidebar); REF-24's
  shorter selected Video label gives an approximately 186-point capsule. Riina's
  segment widths now follow the selected label's measured text width plus its
  icon, gap, and insets, with 36-point icon-only tabs. This replaces the fixed
  192-point capsule and adapts to each category name. The earlier 123-point
  estimate from a resized preview was incorrect. The 10-point source header
  margins remain separate from the explicit 16/14-point Layout
  card insets. Replaced the eight most mismatched setting symbols with
  purpose-drawn Riina vectors for appearance, Windows material, OSD, OSC, sidebar,
  compact interface, edge-to-edge video, and aspect lock. Original assets are
  independently drawn and hash-recorded; a fresh post-change build and capture are
  required before closing rendered bounds and shape comparison.
- Measured REF-22 at native image resolution. The player frame spans 600×518 px
  at x=112..711, y=76..593; at the inferred 2 px/point scale this is 300×259 pt.
  The artwork/control transition is at y=413, dividing the window into about
  168.5 pt of artwork and 90.5 pt below. Riina's 300×259 pt size uses 169 pt for
  artwork and 90 pt for the compact controller, matching the total frame within
  edge rounding. The Music Mode timeline now follows IINA's 32-point minimum time
  labels, right/left alignment, and 6-point edge insets/gaps. The workspace build
  succeeds; a live capture must verify this geometry, plus Windows corners, title
  placement, and video-backed Acrylic.
- The Audio Quick Settings panel now has a full-desktop Windows capture, but no
  matched-size comparison. Corrected
  its light-theme foregrounds and editable-control fills so they remain readable
  over the WinUI-style translucent card layer; the shared and opaque appearance
  keeps its normal theme-specific fills. At the inferred 2 px/point scale from
  the 720 px sidebar and IINA's 360 pt default, the captured track, Load, Delay,
  and Equalizer cards measure about 98, 48, 92, and 246 points. Riina uses those
  heights and the selected track marker follows the platform accent. The cards
  now retain the header's separate 10-point inset and use the source Audio stack's
  16/14-point side margins, yielding 328/332-point widths in the default 360-point
  sidebar. The earlier
  10-point explicit spacer was added to the parent layout's 8/6-point spacing on
  both sides, making the actual card gaps 26/22 points. The spacer is now 2/4
  points to produce IINA's source-defined 18/16-point gaps in spacious/compact
  mode. The Audio cards also now use IINA's source-defined 8-point corner radius.
  The pinned `SidebarAudioPane.swift` Equalizer stacks its 12-point bold heading,
  full-width profile selector, 120-point fader bank, and separate 10-point
  frequency labels. Riina previously put the selector beside the heading and
  reduced the faders to make room for labels. The rebuilt 300%-scale capture now
  shows the source row order, 12-point content inset, full fader height, and
  labels after scrolling. Video-backed Acrylic remains visible over the animated
  pattern. The source image scale is not recorded and the comparison is not
  pixel-scored; light appearance, material timing/cost, and a matched-size card
  comparison remain open.
- Reviewed REF-25 against the saved Subtitles-panel capture. The shared subtitle
  panel now has separate scrollable primary and secondary track cards, the paired
  Load/Search Online actions, a selected-lane accent, and separate timing/position
  and styling cards. Search Online is wired to the existing online-subtitle window.
  Light-theme foregrounds and Windows Acrylic fills are applied to the new card
  controls. At the inferred 2 px/point scale, the source cards measure about 98 pt
  for each track list, 46–48 pt for Load/Search, 188 pt for timing/position, and
  198 pt for styling, with about 20 pt between cards. Riina uses 98, 48, 190, and
  200 pt. The outer stack and horizontal margins now follow the exact source
  tokens (18/16 pt gaps and 16/14 pt margins); the two track groups use source
  item spacing (12/8 pt). Sidebar containers and track selectors use the source
  8-point corner radius. These are source-scale estimates; keep REF-25 open until
  a live Windows capture confirms geometry, selection colors, and remaining styling.
- Reviewed REF-24 against the saved Video-panel capture. Replaced the unframed
  controls with separate rounded cards for tracks, aspect/crop/rotation, speed,
  hardware decoding and image adjustments; aligned their estimated heights and
  18-point gaps to the inferred 2 px/point capture scale. Selected presets now
  use the live Windows system accent on both Windows renderers and the macOS blue
  accent on macOS. The saved Windows image
  `tests/visual/quick-settings-gear-windows.png` is 1280×672 with Video selected,
  but it is file-stamped 2026-10-13 and shows the prior ungrouped controls; it
  cannot validate the current cards. A full-desktop Windows capture on 2026-10-14
  shows the previous Video-panel build over moving video. At 300% Windows display
  scale, its earlier 92-point speed card and roughly 18-point stack gap align
  with the approximately 92-point card and 18-point gap in REF-24 at the measured
  2× capture scale. The source-derived current Speed card is 92/88 points; its
  rebuilt appearance still needs a fresh capture. The panel now also uses source-defined 18/16-point outer gaps
  and 16/14-point horizontal margins in spacious/compact mode, yielding 328/332
  point card widths in the default 360-point sidebar. Its card corners now use
  IINA's 8-point sidebar-container radius. Video is visible through
  the Windows panel and card fills. The
  player-window sizes differ, so the lower controls, total panel bounds, exact blur,
  and matching-state visual close remain open. REF-24's neutral selected Video
  capsule exposed a Windows accent-fill mismatch; the rebuilt Windows capture now
  confirms a neutral capsule while selected presets keep the system accent. The
  cropped current capture is recorded in `tests/visual/windows-captures.json` and
  ignored reference-capture storage. The bottom Equalizer card uses IINA's
  spacious/compact 12/8 pt row spacing and 12/10 pt container padding, plus its
  2 pt inner vertical inset. Reducing the five slider/reset rows from 20 to 14 pt
  yields 146/126 pt card heights and 26/22 pt row pitches. The reset control uses
  an original, provenance-recorded vector icon. A rebuilt 3840×2160 full-desktop
  captures at 300% scale measure the spacious card at about 146 pt and the compact
  card at about 126 pt; row pitches are about 26/22 pt. Both show all five rows,
  and UI Automation reports all sliders and reset buttons. The Light/Compact
  full-desktop capture confirms video shows through the Windows material. Exact
  light-theme material scoring against a matching source and reset-glyph pixel
  matching remain open. Capture hashes are in `tests/visual/windows-captures.json`.
- REF-24's Crop row clipped the leading `N` because Riina placed the rail flush
  against the horizontal viewport. Widening the first segment to 54 and then
  72 logical pixels, and centering the text, did not resolve it; the 72-point
  trial also trimmed `Custom`. Restoring the 43-point option width and placing
  the rail inside a leading-padded measured layout now keeps `None` visible and
  includes the inset in the ScrollView content width. A rebuilt 3840×2160,
  300%-scale full-desktop capture shows the playing fixture beneath Windows
  video-backed Acrylic; UI Automation exposes the Crop “Scroll to more choices”
  button, and the capture visibly shows its chevron beside the clipped trailing
  Custom option. The resulting rail geometry and label are recorded in
  `tests/visual/windows-captures.json` and
  `docs/visual-reference-audit.csv`. Lower image controls, total panel bounds,
  exact blur phase, light-theme contrast, policy fallbacks, and exact reset-glyph
  pixels remain open.
- A pixel remeasurement of REF-24 supersedes the temporary 43-point equal-width
  rail and 8-point leading inset. The selected Aspect `Default` segment is 68
  points; Crop `None` is 56.5 points. The ratio labels measure about 45/50/56/49/45
  points. Riina now uses these measured widths (68/56.5/45/50/56/49/45), starts the
  content at the viewport edge, and reserves the 76-point `Custom` segment beyond
  the first six choices. At the 360-point spacious sidebar, the 290-point viewport
  shows the same partial `5:4` segment at its trailing edge while keeping `Custom`
  behind the chevron. Formatting and the fractional-width workspace build pass.
  UI Automation at 300% reports Aspect `Default` at 204×72 px and Crop `None` at
  169×72 px. Native controls align with and hit-test above the video client, but a
  valid full-desktop capture omitted the panel; rendered pixel comparison remains
  pending.
- The pinned REF-24 crop rail shows its six standard presets before the trailing
  chevron; `Custom` is available after horizontal scrolling. The prior Riina
  capture showed part of `Custom` beside the chevron because six 43-point choices
  plus the 8-point leading inset left about 24 points of that segment visible.
  A temporary 47/48-point crop-segment layout was superseded by source pixel
  measurements: `None` is 56.5 points, followed by 45/50/56/49/45-point ratios.
  The 360-point spacious sidebar leaves a 290-point scroll viewport after the
  card insets, arrow, and gap. Current widths preserve the partial trailing `5:4`
  and keep `Custom` behind the scroll cue; a fresh Windows capture is still needed
  to verify fractional layout at runtime.
- Rechecked IINA's `HorizontalScrollViewWithIndicator`: its Aspect and Crop
  preset rails scroll to the opposite edge over 300 ms, hide the chevron while
  moving, then show the reverse chevron at the destination. Riina now animates
  both offsets for the same duration, honors Disable Animations, and uses the
  same edge-only chevron visibility while retaining the source's indicator slot.
  ScrollView continues to own direct wheel/trackpad offsets; only chevron moves
  use the programmatic animation. The locked workspace build and strict
  formatting check pass; rendered Windows interaction and timing still need a
  live capture. The pinned `HorizontalScrollViewWithIndicator.scrollWheel`
  also translates vertical-only mouse-wheel deltas into horizontal movement.
  Riina now applies that mapping over overflowing Aspect/Crop segments, while
  allowing vertical events through when no horizontal overflow exists and
  preserving native horizontal trackpad deltas; a fresh interaction check is
  pending. The final workspace build and format check pass, but the Windows
  Computer Use capture service returned
  `IGraphicsCaptureItemInterop.CreateForMonitor failed (0x80070057)` twice, so
  the wheel behavior was not exercised live.
- Revisited REF-20 and REF-24 for controller and panel proportions. IINA's
  OSCFloatingView.updatePosition() uses a 67-point value for bottom-position
  clamping; this is not the rendered panel height. The visible stack includes
  14/8-point spacious or 10/5-point compact insets, a 24-point control row, an
  8-point gap, and a 24-point timeline after its 6-point slider padding. Its
  visible panel is therefore about 78 points spacious and 71 compact. Riina's
  67/60-point intermediate layout was too short and has been corrected to
  78/71. IINA centers at horizontal fraction 0.5, keeps the panel's bottom edge
  10% of the video-view height above its bottom, and uses a 25-point drag bound.
  The rebuilt 300%-scale capture measures a 458×78-point visible panel and a
  58-point bottom gap in the 619-point client. The Video sidebar measures 360
  logical points wide, matching IINA's 720 source pixels at the inferred 2× scale.
  Both controller positions must be measured from video client areas, excluding
  platform-owned frames and menus.
- The same source audit found the narrow-width fallback had drifted: Riina cut
  24 points from the floating controller at all window sizes and placed it at
  the leading edge once the video client was under 460 points. IINA centers the
  compressed controller until its full 460-point width fits. Riina now preserves
  the full width when possible, compresses with a one-point inset, and stays
  centered until then. Rebuilt GUI capture around the width threshold remains
  necessary to confirm the layout and ensure every action stays inside the video
  surface.
- REF-20's synthetic GIF has no audio. IINA's `PlayerCore.aidChanged()` hides
  its mute and volume slider when there is no selected audio track. Riina now
  matches this behavior: a full-desktop capture of the same GIF hides those
  controls while a selected stereo track keeps them visible. The transport stays
  centered by retaining its leading layout spacer. The captures are indexed in
  `tests/visual/windows-captures.json`. The sampled Windows material is visibly
  out of phase in the animated capture (gold controller over blue video), so
  material synchronization remains the next controller comparison gap.
- Pixel samples from REF-20's flat green frame and the saved Windows controller
  capture give panel-to-video linear-sRGB luminance ratios of 0.69 for IINA
  and 0.34 for Riina. These captures use different flat-color videos and are
  not a matched comparison, but they show the saved Windows dark controller is
  substantially too opaque. The Windows OSC tint was reduced from 60% alpha in
  that saved capture to 40%, then calibrated to 33% alpha (`#15151555`) from the
  measured ratio; its 58% blurred-frame layer is unchanged. The current full-
  desktop capture helper returns a uniform-black image, so a post-adjustment
  visual capture and light-appearance comparison remain open.
- REF-23 uses an animated 4 fps color pattern. On the 2026-10-14 Windows Audio
  panel capture, the video changed every 250 ms while the Acrylic sample updated
  only twice per second. A 20 Hz mpv screenshot experiment did not remove the
  color lag and coincided with a 10 fps / 18% CPU desktop performance readout;
  this single reading is not a controlled benchmark. That more expensive fallback
  remains capped at 10 Hz. The separate WGC path reads a bounded 320×180 native
  compositor sample and now permits updates at 30 Hz (33 ms) while material is
  visible. Its free-threaded callback orders published samples by Microsoft's QPC
  compositor timestamp and invalidates readbacks still in flight when material
  visibility or media generation changes. The faster WGC cadence should reduce
  capture scheduling delay; measured visual synchronization and playback cost
  remain open.
- The Windows video-backed material now uses the REF-21-calibrated sampled blur
  opacity and translucent fills in both shared-GL and native-video paths. Selected
  Quick Settings states use `UISettings.GetColorValue(UIColorType::Accent)`;
  Layout switches, placement selectors, playback progress, and selected playlist
  markers now use that accent too. Selected text and side-selector glyphs use a
  luminance-aware foreground. The accent and contrasting foreground refresh
  while playback is open. The 2026-10-15 whole-desktop capture shows the video
  filling the playback viewport and its blurred gold square through the dark
  translucent Quick Settings/Video surface. This confirms the sampled-video
  material is composited over the video; its final-frame state does not verify
  sample refresh during motion. At 300% Windows display scale, the sidebar spans
  about 360 logical points, matching REF-21's default width; the player window is
  smaller than that fullscreen reference, so its overall screen proportions are
  not comparable. A separate five-frame, 24fps full-desktop sequence captured at
  roughly 650ms intervals shows the
  blurred gold sample changing as the square moves across and away from the
  sidebar's video region. This confirms qualitative motion response, not exact
  frame synchronization or whether WGC rather than the mpv fallback supplied the
  sample. Dark video-backed material strength was pixel-sampled against REF-21:
  its outside/sidebar pair retains 53.7% linear luminance; the Windows Video-panel
  capture retains 52.6% at the same 360-point sidebar width. This closes the dark
  material-strength calibration to a 1.1-point difference. Exact color/grain,
  accent contrast, frame timing, light theme, and system-policy fallback remain
  open.
- Measured the REF-21 Layout card edges in the pinned 2940×1898 image. At the
  inferred 2× scale, each card is about 328 points wide with 16-point side
  insets inside the 360-point sidebar. Riina previously produced 324-point cards;
  the Layout stack now centers 328-point cards within its existing 12-point
  content padding. The Windows live comparison remains open.
- Split Windows material policy by backdrop type. Battery Saver now disables
  Desktop Acrylic and sampled-video Acrylic while leaving Mica active; disabled
  Transparency effects and High Contrast still suppress both. This matches the
  current [Windows materials guidance](https://learn.microsoft.com/en-us/windows/apps/develop/ui/materials).
  Formatting and whitespace checks passed. The new full-desktop capture confirms
  video-backed translucency in dark mode; matched pixel scoring, light theme, and
  system-policy fallbacks remain unverified.
- Corrected the selected category-tab fill from blue to neutral glass after
  comparing REF-21 and REF-24. The separate Dark/Light/Auto choice and selected
  video presets retain their platform accent. Windows now follows the reference
  category treatment while keeping Windows Acrylic behind the panel.
- An earlier per-window capture measured a 928×624 player host on the 1280×720
  display; the host, controls and mpv child bounds matched, but that capture and
  `PrintWindow` omitted the GPU video surface. The DPI-aware whole-desktop capture
  added on 2026-10-14 does include the video and overlay; the Audio/Video captures
  now show the sampled frame through the translucent panel. Keep per-window
  captures out of pixel conclusions and use the full-desktop source plus a window
  crop. Matching viewport pixels, live material timing and system-policy fallbacks
  remain open.

- The pinned `MainWindowController.setupVideoContainerConstraints()` places the
  video container between the leading/trailing sidebar bounds unless Edge-to-Edge
  Video is enabled; `OSCFloatingView.updatePosition()` sizes, centers, clamps, and
  persists the floating controls relative to that container. Riina previously
  positioned floating and top/bottom controller bars against the full window, so
  the 460-point floating bar could run beneath a 360-point Quick Settings sidebar
  when Edge-to-Edge Video is disabled. The current REF-21 comparison has
  Edge-to-Edge Video enabled, so its full-window controller bounds must stay
  unchanged. Controller width, horizontal origin, drag constraints, and normalized
  saved position now use the video viewport when that setting is disabled. Capture
  both edge-to-edge states and verify dragging/snapping; macOS runtime verification
  remains open.

- A full-desktop Dark → Light → Dark transition on the rebuilt Windows app showed
  the app theme updating its native title bar and Muda menu strip together without
  recreating the player window. Dark File-menu capture also shows the popup remains
  Windows-Light while the system app theme is Light; Muda's menu theme controls its
  bar only. Captures and hashes are recorded in `tests/visual/windows-captures.json`.
  Exact popup appearance and matched IINA menu geometry remain open.

- REF-20 source inspection found the floating transport controls are 24 points
  wide with 24-point stack spacing; docked controls keep 24-point width and use
  16-point spacing. Riina previously rendered 30-point-wide controls only 4 points
  apart. The layout now matches those horizontal dimensions and keeps its narrow
  viewport fit checks aligned to the new transport footprint. The rebuilt 0.3.8
  debug app shows the sample video across the viewport and the transport controls
  at the increased spacing in a windowed playback capture. The full 3840×2160
  desktop capture is indexed in `tests/visual/windows-captures.json`; build and
  format checks passed. At that stage its 17-point control row was still below
  IINA's 24-point button frame; the follow-up source/image remeasurement below
  corrected the overall controller geometry.

- Re-measuring REF-20's original 2940×1898 image corrected that 67-point estimate:
  the controller material interior spans about 156 pixels vertically (78 points
  at the inferred 2× scale), while the width spans about 920 pixels (460 points).
  This agrees with the source layout's 14-point top inset, 24-point control row,
  8-point row gap, 24-point timeline allocation and 8-point bottom inset. Riina's
  previous live controller fill was about 64 points high at 300% display scale.
  The spacious 78-point and compact 71-point layouts now follow those measured
  totals. The rebuilt full-screen capture measures about 459×77 points at the
  Windows 300% scale and is indexed in `tests/visual/windows-captures.json`.
  Build and format checks passed; material color and dynamic blur phase remain
  open.

### REF-20 right-toolbar bounds

IINA's `OSCToolbarButton` assigns each playback action a 24×24-point frame, and
`MainWindowController` places those frames in a zero-spacing stack. Riina used
30×17-point frames with 4-point gaps, spreading the four default actions across
132 points. The Windows toolbar now uses 24×24-point frames with zero gaps, for
the source-derived 96-point cluster. Compact rows preserve the 24-point action
width while limiting height to the available row. The rebuilt 918×619 target-
window capture shows the four actions inside the green-video controller. The
playlist symbol is now an original bullet list inside a rounded rectangle, and
the PiP symbol is a nested picture window, following IINA's `list.bullet.rectangle`
and `pip.enter` semantics. The rebuilt target-window image is indexed. Exact SF
Symbol contours, hover/focus fill, and matched full-desktop pixels remain open;
Riina bundles no Apple artwork or SF Symbols.

### REF-20 timeline label spacing

`MainWindowController.swift` gives both elapsed and duration labels a 46-point
minimum width, centers their text, places them 2 points from the playback
slider, and pads the control by 8 points at the outer edges. Riina previously
used content-sized left-aligned labels and an 8-point gap, making the slider
too long and shifting its endpoints. The Windows timeline now follows the
46/2/8-point source geometry; small-video mode reduces the label minimum to
34 points so the slider remains usable. The rebuilt 918×619 target-window capture
shows centered elapsed/duration labels and the shortened slider inside the same
78-point controller. The capture is indexed; pixel-scored comparison and the
time labels' exact typography remain open.

### 2026-10-15

- Corrected the full-desktop capture helper's DPI declaration after the installed
  PowerShell C# compiler rejected its expression-bodied method. The script now
  captures the full 3840x2160 desktop and validates non-black probes. A first
  playback launch auto-advanced from the short video to a neighboring audio
  fixture; an isolated copy of the video kept the comparison state stable. The
  indexed capture shows the full video inside the player window, the bottom
  controller, and Quick Settings/Layout together. Aligned REF-21 setting icons
  to the pinned 16/20-point measurements. The capture is inspected at full scale,
  but is not pixel-scored; transition behavior and exact card/tab differences
  remain open. Closed the GUI test instance and confirmed no Riina process
  remains.

- A subsequent matched `sample-test-pattern.gif` playback confirms that the WGC
  sample tracks its 4 fps color cycle beneath the translucent controller. Center
  RGB values remain within two levels per channel of the captured flat frame after
  stable grain. Replacing the generic full-size Triangle enlargement with a
  precomputed-coordinate bilinear pass reduced debug material-transform time from
  26–30 ms to 17–19 ms; the focused pixel-equivalence test passes within one color
  level per channel. WGC readback was 5–6 ms and displayed samples were 39–66 ms
  old. The player crop measures 918×619 logical points with a 458-point controller
  fill, close to IINA's 460-point source width. Full-desktop and player-only captures
  are indexed in the REF-20 ledger. The player and its letterbox bands move together;
  a stationary black patch was not reproduced. A follow-up rebuild reduces the
  spacious tray from 78 to IINA's measured 67 points and the compact tray to 60; the
  13-point timeline, labels, playhead, and buttons remain visible in a full-desktop
  capture. Re-reading `OSCFloatingView.swift` with `MainWindowController.swift`
  shows that the source's 67-point value is only its position-clamp constant;
  `playSlider.padding(.vertical(6))` makes the visible timeline row 24 points.
  Riina's 67/60-point panel was therefore too short and is corrected to 78/71.
  Its default bottom offset is about 10% of the video client, matching the source
  fraction. Exact offset at a matched video-client size, blur over nonuniform
  motion, light appearance, and native macOS rendering remain open.

## Highest-priority open work

1. The new continuous General layout needs a matching Riina capture. Verify
   Open Recent visibility and tracking behavior across app restart; AppKit's
   native recent-document integration remains open. Playlist is missing
   track-name display and the music-mode restriction; auto-play-next and
   manual-open repeat still need live behavior verification.
2. Compare the updated Windows Layout capture with REF-21 at a matched client
   size and record remaining spacing, material, and panel-bound differences.
   Then compare Audio, Video, and Subtitle at the same state and size. Maintain
   the source appearance and do not treat shared settings semantics as pixel
   parity.
3. Capture each corresponding Riina preferences page and playback surface at
   matched window size/theme, then replace qualitative “compare” actions in the
   ledger with measured visual gaps and verified closes.
4. Close Delogo and video-filter behavior from REF-30, then verify the six wired
   size commands through live playback and matched captures. Fit Window to Video
   remains in the Layout sidebar and has been removed from the Video menu.
5. Recover `00-start-window.png` from the original capture source or remove its
   stale README index entry after confirming it cannot be recovered.

## Update rules

- Review each image individually and preserve its original bytes and hash.
- Keep missing references, capture defects, and duplicates explicit.
- Mark a visual change complete only after comparing an actual Riina capture at
  the matching state and scale. Compilation alone is not visual verification.
- Update this progress log and the per-reference ledger whenever a gap is
  implemented or a matched capture changes its status.

## REF-21 Windows theme focus and Light appearance · 2026-10-15

The three Theme segments now use the same pointer-versus-keyboard focus treatment
as the Quick Settings category tabs. A rebuilt 300%-scale Light capture shows no
focus outline after clicking Light and shows the outline after pressing Space.
Both full-desktop states are indexed. Light Acrylic remains translucent over the
green playback image; exact card/tab and material pixel scoring against REF-21 is
still open.

## REF-21 Windows Quick Settings tab icons · 2026-10-15

The four category symbols now follow the roles in `QuickSettingViewController.swift`:
paintbrush, photo, waveform, and caption bubble. They render at IINA's 14-point
bold-symbol size using original Riina vectors. A rebuilt Windows capture and
same-scale crop confirm the icon row's meaning and proportions over Acrylic. The
selected Layout text now uses 12 points and measures 76×22 px against IINA's
78×24 px while remaining inside the pill. Remaining card/tab edge pixel scoring
and exact glyph contours remain open.

The 300%-scale UIA record placed the trailing Subtitles tab's hit bounds 18
physical pixels beyond the player edge. The capsule remains source-sized at 192
points, but the header row used the full sidebar width after its 10-point insets,
so `space-between` pushed the capsule past the content edge. The row now uses
the inset content width. Rebuilt UI Automation moves the tab 60 physical pixels
left and places the inferred 192-point capsule edge 30 pixels (10 points) inside
the window, matching the source inset. Primary and secondary lane headers also
measure 16 points high. The workspace build and formatting check passed; a
rendered-edge capture remains pending because the full-desktop capture returned
uniform black.

## REF-24 Windows Video switch-card grouping · 2026-10-15

The pinned Video-pane screenshot shows Hardware Decoding within the same
radius-8 card used by the pane's settings and Equalizer containers. Source
code also places optional HDR in that same container with 12/8-point row
spacing and 12/10-point spacious/compact padding. Riina had separate controls
cards with a 10-point radius, and its Aspect/Crop/Rotation card also used
10 points. The current implementation uses one shared Acrylic-backed card,
source-sized row spacing/padding, and 8-point radii on both Video cards.
`cargo build --workspace --locked` passed; rebuilt spacious/compact window
captures show the card corners over live video-backed Acrylic without clipping.
These window-bounded captures are not pixel-scored. The reference capture has
no HDR row, so the two-row state needs a compatible live source.

## REF-24 Aspect/Crop/Rotation card proportions · 2026-10-15

`SidebarVideoPane.swift` uses the shared 12/10-point spacious/compact container
padding and 12/8-point item spacing for its Aspect, Crop, and Rotation rows.
Riina's card still used 8-point padding, 12-point spacing, and a fixed 200-point
height in both modes. The six 16/24-point rows plus five source-sized gaps and
container padding yield 204 points spacious and 180 compact; Riina now uses
those mode-specific dimensions. A rebuilt matched-size capture is still needed
to verify the rendered card bounds and its fit with the neighboring Video cards.

### REF-24 reset-glyph follow-up

The pinned `SidebarVideoPane.swift` uses a filled circular counterclockwise
reset symbol for every equalizer row. Riina's existing vector was only an open
arc, so it missed the source's filled silhouette. The independent MIT vector
now cuts a counterclockwise arrow through a filled circle; its provenance hash
is updated and `cargo build --workspace --locked` passed. UI Automation exposed
the Video panel's reset actions, but the full-desktop capture returned uniform
black after opening the panel and is invalid visual evidence. The rendered
shape and pixel match remain unverified.

## Windows screen-fixed transition-cover follow-up · 2026-10-15

A native move event now drops any retained opaque transition cover before aligning
the transparent controls HWND. This closes the path where a cover could remain at
its old desktop coordinates while the player moves. Formatting and the workspace
build passed. The desktop capture helper produced an all-black image
with Riina closed, so this implementation change awaits live visual verification.
The previous moving-square capture shows the fixture's gold square moving with the
video and does not reproduce the reported black patch.

## REF-24 Windows Acrylic blur strength · 2026-10-15

The saved full-desktop Video-panel capture shows the moving gold test square
remaining too distinct through the Quick Settings sidebar. Increased the Windows
sidebar's sampled-blur opacity from 53% to 80%; its independently calibrated
tint and source-measured card/sidebar geometry are unchanged. The current
full-desktop capture helper returns black, so this material adjustment is not yet
visually verified on the rebuilt app. Review high-contrast moving content and
preserve the REF-24 flat-frame luminance target.

## REF-24 Windows sampled-video blur follow-up · 2026-10-15

The saved 300%-scale moving-square capture still shows a recognizable hard edge
through the sidebar material. Increased the shared/native Windows sample blur
from sigma 8 to 10. At the measured 915-point player width, the 320-pixel sample
scale yields an approximately 29-point blur. The calibrated tint and 80% sidebar
sample opacity are unchanged. The rebuilt Windows capture and material latency
check remain open; the previous UI-control attempt failed before the Quick
Settings pane could be targeted.

## REF-24 Speed card source geometry · 2026-10-15

`SidebarSliderView.swift` specifies a 16-point title icon, 12-point bold title,
50-point value field, indicator centered on the slider thumb, an 18-point gap
above the slider, and 16 points before the tick-label row. Riina's card omitted
the title icon and thumb indicator, used a 76-point field, placed Reset beside
the field, and spread tick labels evenly across the whole card. The Video card
now uses an original MIT playback-speed glyph, puts Reset in the title row,
shows the current speed above the thumb, uses a 50-point field, and positions
the four labels at IINA's tick fractions across the slider track. Its 92/88
point spacious/compact heights follow those row and container-padding
constraints. IINA measures the 16-point slider bottom inset to the card bottom;
the 2-point gap and 14-point tick-label row occupy that inset. The tick row now
remains inside the Slint vertical stack; it had accidentally become a sibling of
the stack and therefore was not laid out after the slider. This corrects the
rendered geometry; formatting and `cargo build --workspace --locked` now pass.
Live geometry capture remains pending.
The pinned screenshot's card border spans 184 pixels (92 points at 2×); the
sidebar width independently measures 720 pixels for its 360-point source width.
The 20-point slider/input row uses a 10×18-point thumb measured from the
reference's 20×36-pixel knob. The value label sits 2 points above that row, and
the thumb leaves 2 points before the 14-point tick row. This yields 68 points of
stacked content plus the spacious/compact 12/10-point vertical insets and
1-point borders, for 92/88-point cards. The prior 98/94 estimate used an
oversized slider row and default 6×8-point thumb. Track and marker coordinates
now follow the configured thumb centers; default sliders retain their previous
6-point track inset. A rebuilt
Windows capture is still needed to compare the new rendering.
The previous GUI capture attempt failed with `CreateForMonitor (0x80070057)`;
the latest build has not been visually captured. No Riina test process remains open.

## REF-24 Speed card UI Automation bounds · 2026-10-15

The rebuilt Windows app exposes the source-sized 246-point slider, 50-point
custom-speed field, thumb-positioned live value, and source-fraction tick labels
at 300% scaling. The tick row is 14 points high beneath the 20-point slider row.
The next Hardware Decoding card appears below the spacious Speed card. These
measurements establish control bounds and order, not text rendering, thumb shape,
or Acrylic appearance. A trustworthy screenshot still needs to include the
sidebar. The exact test instance was closed and no Riina process remains.

## REF-21 Sidebar scrollbar visibility · 2026-10-15

The pinned `SidebarPane.swift` enables `autohidesScrollers`, and its saved
Quick Settings reference has no vertical indicator at rest. The saved Windows
REF-21 full-desktop capture shows a persistent scrollbar along the right edge
of the same pane. Riina now keeps that sidebar content scrollable while hiding
the idle scrollbar. Wheel/trackpad behavior and the rebuilt at-rest appearance need a fresh
capture. Formatting, workspace check, and application build passed; the current
Windows Graphics Capture service returned `CreateForMonitor (0x80070057)` during
the attempted UI check.

## REF-21 On Screen Controller choice geometry · 2026-10-15

The pinned native-resolution crop measures the selected choice at 66×74 points
with 8-point horizontal gaps. Its source view uses a 40×28-point preview,
8-point vertical spacing, and 12-point item padding; the visible frame is about
32.5×25 points. Riina now reserves that preview viewport, draws the inset frame,
and uses the measured 214-point row. Source `NSBox` choices retain 8-point
corners, a 2-point selected outline and a 1-point inactive outline; the Riina
component now follows those metrics with a low-contrast themed inactive stroke.
The workspace build and formatting check for the outline correction passed. Live
measurements remain open: Computer Use failed at `CreateForMonitor (0x80070057)`, and the
full-desktop capture helper returned an all-black baseline with Riina closed.
Neither failure is visual evidence for the rebuilt UI.
The reference OSC selector adds 8 points below the tile row before the outer
18-point section gap. Riina now reserves that 8-point inset, bringing the next
section title's intended offset to 26 points below the tiles. Formatting and the
workspace build pass, but rendered spacing still needs verification.
The matched-scale crop also showed the section captions at about 16 px high in
Riina and 19 px in IINA. `createSectionTitle` uses a bold 12-point font; the
Riina caption is now 12 points instead of 11. The locked build and formatting
check pass; fresh glyph-bound verification remains pending.
The source gives its Sidebar Position container an 8-point bottom inset. Riina
reserves that transparent scroll-tail below the material card. Its three rows
now use 24-point controls, source-defined 10/6-point spacious/compact gaps and
12/10-point container padding, giving derived card heights of 118/106 points.
This replaces a fixed 114-point card with rows spaced 2 points apart. The
rebuilt geometry still needs matched-scale pixel verification.

## REF-25 Subtitle lane header geometry · 2026-10-15

`SidebarSubtitlesPane.swift` uses a 16-point `1.square.fill`/`2.square.fill`
icon, 12-point bold label, and mini `NSSwitch`, followed by the source's
12/8-point spacious/compact group gap and 98-point track list. Riina used a
28-point text row with circular Tool buttons, adding 12 points per lane and
missing both numbered badges and switch tracks. The new Windows row uses a
16-point numbered badge, 12-point label, and 36×16-point toggle with the system
accent, restoring the source row height while preserving native Windows control
color. The click target covers the full row without adding a layout column, and
keyboard/assistive-technology checkbox actions use the same toggle state. The
corrected workspace build passed; rendered capture verification remains pending.

## REF-24 Speed tick-label thumb centers · 2026-10-15

The previous 300%-scale UIA layout put the interior `1x` and `4x` labels at
83 and 163 points from the slider origin. `SidebarSliderView.xOffset` places
those two interior labels at the one-third and two-thirds positions across
thumb travel, with half of the 10-point thumb width at each endpoint. Riina's
custom speed slider uses that 10-point thumb, but its labels still compensated
for a 6-point track inset. Updated both formulas to the active thumb geometry;
the expected correction is 0.67 point (2 physical pixels) per label. Rebuild
and fresh UIA bounds check are pending; this small alignment change does not
establish pixel-level parity.

## REF-24 Hardware Decoding switch dimensions · 2026-10-15

The pinned 2× reference crop measures the enabled switch track at 88×40 pixels
(44×20 points) and its thumb at about 26×16 points. The rebuilt Windows control
had measured 30×18 points through UI Automation. Riina now uses the reference
track and thumb proportions; its Hardware Decoding row and card use the measured
20-point control height, with the spacious/compact padding and optional HDR gap
included in the outer card calculation. Pixel bounds also show a 16-point CPU
glyph, an 8-point gap, and a 13-point system label beginning 36 points inside
the card; Riina's previous row omitted the glyph and used 12-point text. It now
uses an original chip icon and 13-point label. The optional HDR row now also has
an original 16-point sun symbol, an 8-point gap, a 13-point label, and measured
small-switch geometry. The pinned source creates an `NSSwitch` and uses its
small control size on macOS 26 and later. The 2× REF-11d HDR preferences capture
measures the crisp selected system switch at 72×32 pixels (36×16 points), with
a 42×24-pixel (21×12-point) thumb. Riina now uses those dimensions; formatting
and the locked workspace build pass. HDR-row UIA bounds, a compatible playback
capture, and rendered pixels remain open.

## REF-20 Windows live playback framing · 2026-10-15

An isolated `moving-square.avi` run produced a player-window image and a valid
3840×2160 full-desktop capture. The 320×180 moving-square clip reached frame
119; its video and letterbox bars stayed attached to the player, with no
screen-fixed black patch over playback. The floating controller appears
consistent with IINA's 460-point width and default lower placement. Windows
rejected the Quick Settings input at its foreground-window guard, so the
material and current Hardware Decoding row were not captured. The exact Riina
test process was closed. This is a live visual check, not a matched pixel score.

## REF-24 Windows sampled-video material while paused · 2026-10-16

Opened the Video Quick Settings panel on the rebuilt Windows app with the
sample-pattern fixture. During playback, the Acrylic approximation visibly
sampled the changing video frame. At a paused frame, the earlier implementation
let that sample age out and switched the sidebar to its solid fallback. The
sampler now retains the last displayed sample while the playback position stays
within 0.25 seconds of that frame; a seek, media-generation change, stale moving
frame, or disabled transparency still clears it. After rebuilding, a live
capture showed the blue paused video tint remaining under the Video sidebar
after 1.2 seconds, past the 750 ms refresh timeout. The 360-point sidebar width
matches IINA's pinned 360-point default. No desktop details leaked through. The
capture was inspected but not saved; exact color/blur comparison against
`reference/captures/iina-reference-set-2026-10-12/24-playback-video-panel.png`,
light appearance, and pixel scoring remain open. Formatting and the locked
workspace build pass, and the exact Riina test process was closed.

## REF-21 Quick Settings selected-tab label alignment · 2026-10-15

The pinned REF-24 crop shows the selected symbol and label sharing a horizontal
centerline. Riina's saved REF-21 Layout capture showed the icon above the label's
visual center inside the selected pill. The selected tab now uses explicit
cross-axis centering for both contents. Its selected width follows the rendered
label width, while each icon-only segment is 36 points. REF-21 estimates the
Layout capsule at 191–192 points; REF-24 estimates Video at about 186 points.
The 0.3.8 app opened and played `moving-square.avi`, and the full-desktop capture
confirmed the media title and four right-side tools. The Windows desktop-control
bridge exposed no native app for pointer input, so Quick Settings could not be
opened for visual verification. Fresh tab pixels and panel-row fit remain open.
