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

The earlier empty-player capture was a separate 962 x 649 window state: Riina's
name and subtitle sat in the center, while Open File remained at the left edge.
The pinned reference supplies a 560 x 440 welcome surface with Open File/Open URL,
resume-last-file and recent documents. A live Windows capture confirms a 558 x 391
client request produces the 560 x 440 outer window, and Open URL opens a focused
input. An isolated seeded-preferences capture confirms Resume and recent rows
render. The current code now lays out Open File/Open URL side by side, keeps the
resume item out of the recent-document table, and maps Up/Down/Return to the
reference selection model. A regression test verifies that split. File selection
and validated HTTP/HTTPS/RTSP/RTSPS input are wired, and a native start option
opens the selected item at its saved position. The latest layout and navigation
changes still need a live capture. The Resume row now preserves the reference's
separate history icon, action label, filename and right-aligned position. A
dedicated welcome window replaces window reuse in the source; its show/hide and
cancel-recovery lifecycle still needs live verification. An ignored native test
now verifies engine watch-later save/reopen when enabled and no save or restore
when disabled. App-level saved-settings restart, live Windows preference
interaction and matching pixels remain unverified; material is incomplete, and
no pixel comparison against a matching reference has passed.

A fresh screenshot of the release build showed a 560 x 328 outer welcome window,
so the earlier preferred-height request was not reaching the actual window. The
welcome component now sets concrete 558 x 391 bounds instead of only preferred
dimensions. The capture helper displayed the window image but returned a null
state object on both attempts; recapture the rebuilt app before accepting these
bounds or comparing welcome pixels.

General > History now exposes the source's default-enabled Resume last position
setting. The persisted policy configures libmpv's save-on-quit and resume options
at startup and on live changes, and it controls whether the welcome window shows
the separate Resume row or treats the newest item as an ordinary recent document.
Unit tests cover defaults, persistence, both engine options and the row mapping.
An explicit native test also verifies actual position save and restore against
the provisioned DLL. App-level saved-settings restart, live Windows interaction
and matched pixels remain open; this is functional coverage, not completion of
General Preferences parity.

The existing accepted macOS reference captures establish a small paused player
and seek OSD. They do not establish visible floating controls, sidebars, menus or
Preferences. Obtain and inspect those reference states before declaring a visual
comparison passed; source dimensions are useful inputs, not matching pixels.

| Surface | Current gap and required comparison |
|---|---|
| Main window | Native title/menu strip, in-player title/buttons, rounded window shape, aspect fit, startup, separate welcome window and empty state |
| Floating controls | 12pt macOS 26 corners, complete button groups, glyph weight, sliders, spacing, video-sampled material, compact/spacious layouts, hide animation |
| Other control layouts | Top/bottom layouts and their exact allocation, visibility and resize behavior |
| Video settings | Tab organization, aspect/crop/rotation controls, equalizer and hardware-decode controls |
| Audio/subtitles | Shared reference tab organization, row styles, selections, editable fields, warnings and secondary subtitle controls |
| Playlist/chapters | Width, resize, row metrics, tabs, text treatment, selections, footer actions and drag/drop |
| Menus | Complete structure, text, checks, enabled state, shortcut display and window ownership |
| Preferences | Full source-list navigation and all nine categories; complete General/UI/Subtitle/Network/Control/Keybindings/Video-Audio/Advanced/Utilities panes; matched labels, alignment, enabled state, scrolling and sizing |
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

Player settings has independently implemented Layout/Video/Audio/Subtitles
navigation; playlist and chapters share a second tab group. Actual Windows
shared-renderer captures show all six implemented panes, selected tabs and keyboard focus.
Tab/Enter/Space navigation and Escape dismissal were exercised with catalog.mkv
paused at 0:00 in the isolated capture profile; the owned player exited 0.
The images show floating controls overlapping the sidebar. The reference default
edgeToEdgeVideo=true keeps the video container across the window, so this is not
alone evidence of a placement defect. Reference-matched tab glyphs/materials,
resizing and non-edge-to-edge control allocation remain open. The separate
Preferences window now has a source-sized 820 x 480 shell, selectable navigation
in the pinned nine-category order (General, Interface, Video & Audio, Subtitle,
Network, Control, Key Bindings, Advanced and Utilities), the existing General
controls, and a functional partial Interface pane for OSC position, compact mode,
docked controls and OSD visibility/speed/seek suppression. OSD settings update
live overlay filters and persist through the shared settings path. Video & Audio
now includes the source-sized hardware-decoder selector for Disabled, Auto and
Auto (Copy), with the selected mode applied to libmpv and persisted. Advanced
includes an opt-in volume boost control, retaining the 100% default and allowing
up to 200% when enabled. Subtitle now includes the source's independent Primary
and Secondary ASS override selectors with the five-level slider, current level
and explanation, all persisted with the libmpv policy. The Text Subtitles group
now has the editable font size, font chooser, bold and italic toggles,
text/background colors, border size/color, shadow offset/color, blur and spacing
fields. Changes map to shared typed libmpv commands and persist through
preferences, layout changes, sidebar changes and shutdown. Its source-sized
section stack scrolls within the pane; the Advanced disclosure starts folded and
reveals Shadow and Other Styles. The other four Subtitle sections remain open.
Network, Control, Key Bindings and Utilities currently show their title only.
Plugin settings are conditional
upstream and omitted until the plugin system exists. Source-list geometry, full
pane content and a fresh live capture remain open. These changes follow the
source's category names/order; they do not establish pixel parity.

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
trailing and Playlist leading. Edge-to-edge video now uses the shared viewport
geometry and native mpv margin commands; turning it off places open sidebars
outside the video viewport, and enables the reference Dock control bar and
titlebar choice. The native viewport regression passes. Live GUI geometry and
matched macOS pixels remain unverified. Window aspect locking, fit-to-video,
theme/material, plugin placement and port runtime remain open.
