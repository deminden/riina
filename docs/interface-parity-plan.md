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
