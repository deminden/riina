# Interface parity first

Priority set by the user on 2026-10-10: complete interface parity before adding
further features. Target the pinned IINA 1.5.0 release, preserving independent
original Rust/Slint code and artwork. Subsequent development stays local until a
substantial verified public milestone.

Windows capture now succeeds through the computer-use tool. The default shared
renderer capture shows the original video fixture and complete floating panel.
Native renderer captures expose chrome but show a black video region; that alone
does not establish that the visible native playback surface is black. The prior
monitor-capture failure is no longer a blanket blocker.

The existing accepted macOS reference captures establish a small paused player
and seek OSD. They do not establish visible floating controls, sidebars, menus or
Preferences. Obtain and inspect those reference states before declaring a visual
comparison passed; source dimensions are useful inputs, not matching pixels.

| Surface | Current gap and required comparison |
|---|---|
| Main window | Native title/menu strip, in-player title/buttons, rounded window shape, aspect fit, startup and empty state |
| Floating controls | 12pt macOS 26 corners, complete button groups, glyph weight, sliders, spacing, video-sampled material, compact/spacious layouts, hide animation |
| Other control layouts | Top/bottom layouts and their exact allocation, visibility and resize behavior |
| Video settings | Tab organization, aspect/crop/rotation controls, equalizer and hardware-decode controls |
| Audio/subtitles | Shared reference tab organization, row styles, selections, editable fields, warnings and secondary subtitle controls |
| Playlist/chapters | Width, resize, row metrics, tabs, text treatment, selections, footer actions and drag/drop |
| Menus | Complete structure, text, checks, enabled state, shortcut display and window ownership |
| Preferences | Full toolbar/tab navigation and every pane; custom small controls; labels, alignment, enabled state, scrolling and sizing |
| Auxiliary surfaces | Crop editor, font chooser, screenshot preview, transient OSD, dialogs and their focus/lifecycle |
| Window modes | Fullscreen, music mode, PiP, ontop, multiple windows, theme and DPI behavior |

For each surface, record fixture/position, client bounds, scale, theme, selected
panel and reference release hash. Inspect the actual before/after images. Compare
geometry, type, original glyphs, colors, materials, stacking, focus and animation
at matching states. Preserve explicit differences and failed observations.

First captured corrections: the General folder chooser's toolkit padding hid its
ellipsis; explicit original geometry keeps the glyph and keyboard activation.
The folder label needs a gap after Save to. Floating corners now use the recorded
macOS 26 value. These changes do not close the larger interface acceptance target.

Settings now has independently implemented Video/Audio/Subtitles navigation;
playlist and chapters share a second tab group. The pinned settings tab order
also includes Layout, which remains unimplemented. Actual Windows shared-renderer
captures show all five implemented panes, selected tabs and keyboard focus.
Tab/Enter/Space navigation and Escape dismissal were exercised with catalog.mkv
paused at 0:00 in the isolated capture profile; the owned player exited 0.
The images show floating controls overlapping the sidebar. The reference default
edgeToEdgeVideo=true keeps the video container across the window, so this is not
alone evidence of a placement defect. Reference-matched tab glyphs/materials,
Layout, resizing and non-edge-to-edge control allocation
remain open. These captures are inspection evidence, not a passed pixel comparison.

Both sides now retain their own selected pane. Captured Windows playback shows
Playlist plus Audio simultaneously; switching to Chapters preserves Audio, and
closing Audio preserves Chapters. Crop entry closes both panes. Side-specific
close actions restore player focus before hiding the focused control. This does
not yet cover RTL placement, side preferences or animation fidelity.

Independent four-point resize handles now retain side-specific widths, with
300–500-point drag bounds and the reference window-width cap. Opening clamps the
visible panel without rewriting the saved width. A completed resize persists the
live preferences; a failed write restores both last-saved widths and reports the
error. Actual Windows drag/restart/save-failure and click-outside-dismissal checks
passed in an isolated profile. Narrow-window cap, native renderer interaction,
RTL and other-platform runtime remain unverified. Exact reference pixels remain
open; the retained restored-width capture is private regression evidence.

The Layout tab now exposes persisted Floating, Top and Bottom controller modes,
Compact interface, and the leading/trailing assignments for Settings and
Playlist/Chapters. Top/Bottom reserve their own space alongside an open sidebar.
The position switches move the selected pane, preserve one pane per physical
side, and replace content when both categories share an edge. Windows capture
confirmed the Settings move and same-side replacement; defaults remain Settings
trailing and Playlist leading. Theme/material, edge-to-edge/window sizing, dock
behavior, video fit, plugin placement and matched macOS pixel comparisons remain
open.
