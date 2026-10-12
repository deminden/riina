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
