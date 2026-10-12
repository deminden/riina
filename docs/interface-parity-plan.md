# Feature gaps, then interface parity

Priority revised by the user on 2026-10-11: implement the currently unaddressed
IINA behaviors first, then close visual differences against the pinned IINA
1.5.0 reference step by step. Preserve independent original Rust/Slint code and
artwork. Keep subsequent development local until a substantial verified public
milestone.

The current OS-version target is macOS 27.0.1, Apple's latest release as of
2026-10-12. The captured IINA 1.5.0 reference surfaces below were taken on 26.6.2;
they remain pinned-app evidence, not 27.0.1 visual goldens. Capture matching IINA
and Riina surfaces on 27.0.1 before accepting current visual parity.

Windows capture now succeeds through the computer-use tool. Earlier captures did
not distinguish capture failure from native-overlay opacity. The 2026-10-11 live
repair identifies an opaque Slint chrome HWND over correctly decoded native video;
restoring DWM alpha reveals the complete fixture. The owner event router now keeps
aspect filtering, alignment and fullscreen focus handling together. Retained
windowed/fullscreen captures verify the local correction, not macOS pixel parity.
The prior monitor-capture failure is no longer a blanket blocker.

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

The pinned `InitialWindowController.swift` constructs the welcome surface at
560 x 440 points and omits the resizable style. A fresh Riina release capture
showed only 560 x 328, despite earlier source requests intended to reach 560 x
440 on Windows. The welcome component now constrains its preferred, minimum and
maximum logical size to the source dimensions. In the rebuilt native Windows
app, the complete welcome contents render at 562 x 469 outer pixels (560 x 440
logical client points) and sit centered. The retained black video host is hidden
without covering Welcome; opening and cancelling the native file picker returns
to the rendered welcome surface. This verifies dimensions and the welcome/picker
lifecycle only; a matched screenshot comparison and remaining welcome details
are still open.

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
and seek OSD, but not matching-state controls or menus. A separate research-only
capture now records every IINA 1.5.0 Settings page on macOS 26.6.2: a 608 x 500
outer window at 1x scale, with a 200-point source-list sidebar, Search field,
24-point icon slots with 18-point symbols, selected-row treatment, General
section links and scrollable 400-point content. All eleven page screenshots were
reviewed and indexed in `tests/visual/reference-captures.json`; they are not
accepted comparison goldens. Riina now has original page symbols, selected-row
styling and a grouped live-search popover for currently implemented controls.
Search supports arrow-key navigation and scrolls toward mapped item positions,
with collapsed Subtitle sections opened when needed. Exact row highlighting, the
full IINA search index, General section links, the 600 x 480 client geometry and a
matched Windows screenshot remain open. Source-derived dimensions alone do not
prove rendered parity.

| Surface | Current gap and required comparison |
|---|---|
| Main window | Native title/menu strip, in-player title/buttons, rounded window shape, aspect fit, startup, separate welcome window and empty state |
| Floating controls | macOS 27.0.1 corner behavior, complete button groups, glyph weight, sliders, spacing, video-sampled material, compact/spacious layouts, hide animation |
| Other control layouts | Top/bottom layouts and their exact allocation, visibility and resize behavior |
| Video settings | Pixel-accurate tab, card and scroll layout; decoder threads, hardware decoder choices and nested tone-mapping controls; ICC/HDR switches and a matched macOS comparison |
| Audio/subtitles | Shared reference tab organization, row styles, selections, editable fields, warnings and secondary subtitle controls |
| Playlist/chapters | Width, resize, row metrics, tabs, text treatment, selections, footer actions and drag/drop |
| Menus | Complete structure, text, checks, enabled state, shortcut display and window ownership |
| Preferences | Source-list search/navigation and all eleven categories; complete General/UI/Video/Audio/Subtitles/Network/Control/Key Bindings/Advanced/Plugin/Utilities panes; matched labels, alignment, enabled state, scrolling and sizing |
| Auxiliary surfaces | Crop editor, font chooser, screenshot preview, transient OSD, dialogs and their focus/lifecycle |
| Window modes | Fullscreen, music mode, PiP, ontop, multiple windows, theme and DPI behavior |

HDR switch acceptance is source- and display-dependent. IINA restores the saved
`enableHdrSupport` default (true) for each newly opened player, then shows the Video
sidebar switch only when the source is recognized as HDR and the active display can
present extended range. Enabling requests EDR and matches the renderer target to the
source; disabling restores the display ICC/SDR path. Riina now detects the active
HDR10/PQ output for the player window and connects the saved Video-pane switch to
the source-guarded color policy. Load plans clear source-specific target overrides
before replacing media. The native renderer still needs live transition evidence,
and macOS EDR capability discovery remains open. Decoding an HDR fixture or
enabling HDR in Windows alone does not verify this behavior.

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
resizing and non-edge-to-edge control allocation remain open. All eleven pinned
Preferences pages are now captured from IINA v1.5.0: General, Interface, Video,
Audio, Subtitles, Network, Control, Key Bindings, Advanced, Plugins and Utilities.
The observed window is 608 x 500 including chrome; the source-derived content area
is 600 x 480 with a 200-point sidebar. Riina's current Settings component now uses
the same 600 x 480 client bounds and 200-point sidebar. Its outer Windows chrome,
card layout, scrolling, and matched rendering remain unverified. The sidebar now has original icons and
grouped search over implemented controls. It has the existing General
controls, and a functional partial Interface pane for OSC position, compact mode,
docked controls and OSD visibility/speed/seek suppression. OSD settings update
live overlay filters and persist through the shared settings path. The optional
fullscreen information row now refreshes local time and battery percentage while
fullscreen controls are visible, and stops its timer when hidden. Its border,
glass, clock glyph, and matched-state geometry still require a fresh Windows and
macOS reference comparison. Video & Audio
now includes the source-sized hardware-decoder selector for Disabled, Auto and
Auto (Copy), with the selected mode applied to libmpv and persisted. Advanced
Audio > Volume now contains the opt-in 200% cap extension, retaining the
user-requested 100% default; moving it here leaves Advanced for source settings.
Subtitle now includes the source's independent Primary
and Secondary ASS override selectors with the five-level slider, current level
and explanation, all persisted with the libmpv policy. The Text Subtitles group
now has the editable font size, font chooser, bold and italic toggles,
text/background colors, border size/color, shadow offset/color, blur and spacing
fields. Changes map to shared typed libmpv commands and persist through
preferences, layout changes, sidebar changes and shutdown. Its source-sized
section stack scrolls within the pane; the Advanced disclosure starts folded and
reveals Shadow and Other Styles. Subtitle > Other now contains a preferred-language
field and the pinned 44-entry encoding menu; changing encoding updates `sub-codepage`
and reloads each current subtitle track. The plain-text language editor, exact group
geometry, live save/restart and matched rendering remain open. Auto Load controls
persist and configure mpv's disabled/fuzzy/exact sidecar modes on startup and live
preference saves. Literal directory lists are passed to mpv; IINA's default `./*`
glob is retained but not misinterpreted as a literal directory. Priority keyword
selection now promotes the best matching external track after native discovery.
IINA's broader series matching and full directory/glob matching remain open.
Online Subtitles now supports a user-keyed OpenSubtitles title/hash search, language
filtering, selection, download and loading into the active media. Automatic search
is opt-in, one attempt per playback generation, and requires local media above the
configured duration threshold with no subtitle tracks after discovery settles. The API key uses
the operating system's secure credential store and migrates out of earlier plain
settings JSON. Provider response
edge cases, live playback and source-matched window geometry remain unverified.
Network's Cache section now follows the captured reference layout and uses the
pinned defaults for live and startup mpv configuration. The Network subsection
now exposes the source's user-agent, HTTP-proxy and RTSP-transport controls, persists
them and supplies libmpv startup options; those options keep the source's restart-
required behavior. Its visual measurements are source-derived from the pinned
SettingsPageNetwork and legacy XIB because no capture of this subsection exists.
Online Videos (yt-dlp) now has source-ordered extractor enablement, search-folder
and raw-options controls, persisted preferences, and startup/live mpv options.
The custom search folder takes effect on the next app launch, following the
reference behavior. Online Media plug-in installation and site playback remain
open. The app has not been captured in this state, so rendered content, dimensions
and matched pixels remain unverified. Audio, Control, Key Bindings, Plugin and
Utilities are otherwise incomplete. Source-list selection behavior, geometry,
full pane content and a fresh live capture remain open. The category order is
source-checked; it does not establish pixel parity.

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

Subtitle Position now has source-default X/Y alignment, editable X/Y margins,
vertical position, letterbox display and window-scaling controls. The values use
validated shared preferences and typed mpv commands, including the source's
two-billion maximum margin and finite-number checks. The controls are implemented
and covered by unit tests. A live Windows capture shows the complete group and all
source defaults in the scroll view, including bordered numeric inputs. Editing,
save/restart behavior and a matching Windows/macOS visual comparison remain open.
Subtitle Other now persists the preferred-language string and selected encoding,
applies both options at startup, and reloads existing subtitle tracks after an
encoding change. Its 44-choice menu follows the pinned source order and passes a
menu round-trip test. The language field is not yet the source token-field control;
runtime save/restart and matched pixels remain open.
Subtitle Auto Load now precedes ASS in the section stack. It exposes the three
source-ordered modes and Advanced priority-keyword/search-path fields with the
pinned defaults. Startup and live preference saves configure mpv's `sub-auto`
property, translating Disabled/filename/IINA modes to no/fuzzy/exact sidecar
matching. Literal directories map to `sub-file-paths`; the default `./*` glob is
retained without being passed as an invalid directory. IINA keyword ranking and its complete directory/glob matcher remain unimplemented.
Playlist shuffle/unshuffle and filename/path sorting now have native command plans,
sidebar controls, and focused core coverage. Native playback confirmation, exact
sort ordering against the live engine, drag and multi-select actions, and matched
Windows/macOS pixels remain open.
Audio now has the captured Hardware, Volume and Other section routes. Typed
preferences configure startup audio threads, driver, device ID, S/PDIF, initial
volume, ReplayGain, gapless playback and preferred languages. The independent
200% volume extension is visible in Volume and remains disabled by default.
Device enumeration, immediate reconfiguration, native language selection and
matched surface pixels remain open. The remaining Network pages, Control, Key
Bindings, Plugin and Utilities remain unfinished. Full surface comparison is
part of the complete parity goal.

Window-aspect lock and Fit Window to Video now appear in the Layout sidebar with
the reference default and visibility conditions. Shared sizing logic uses display
dimensions, rotation and explicit aspect overrides; resize events preserve the
dominant edge, and fitting grows the short dimension within the current monitor.
This is only a portable policy and application compile check. Live window drags,
fit placement, fullscreen/maximize transitions, saved restart, native-window
rendering and matched macOS/Windows captures must still verify the actual behavior.
The aspect callbacks now target the separate native-video host window as well as
the shared chrome window; this wiring still needs playback and resize verification.

The Control Preferences page was a heading-only stub. It now follows the captured
Control page order and exposes Trackpad, Mouse, Scroll and Scrolling Sensitivity
groups. IINA's action enums, available popup choices and defaults are represented
by a validated shared Rust model and saved with the existing preferences file.
The source screenshot shows the Trackpad card at the top, five Mouse rows, then
the vertically and horizontally assigned scroll actions. Riina's page scrolls to
keep the seek-type and three sensitivity sliders reachable in the 480-point client.
The Settings controls and persisted values are now represented; actual gesture
dispatch, Force Touch/pinch platform mapping, a live Windows capture and a matched
macOS comparison remain open. See PREFERENCES-CONTROL-POINTER-001 and
PREFERENCES-CONTROL-SCROLL-001 in the atomic inventory.

### Key Bindings page

The pinned macOS Settings capture (page 08) is the visual reference: a rounded
system-media-control row, then a profile selector and action list with outlined
key-caps aligned at the right. A search field and raw-value control sit above the
list. The legacy `PrefKeyBindingViewController.xib` is evidence for profile and
mapping operations, not the new Settings page geometry. Riina now follows the
captured one-column hierarchy, includes the first seven visible IINA defaults, and
filters rows by readable labels or raw values. Its profile store supports list,
create, duplicate, import, save, rename and delete, with reveal/delete wired to the
page. Active-profile dispatch routes playlist/chapter panels, fit-to-video, Music
Mode and Picture-in-Picture to their existing app callbacks; the IINA Default
profile binds these application actions. The reveal-file action uses the active
playlist entry and delegates local-file selection to the native file manager.
The Live Text binding toggles and persists the existing availability-gated
recognition preference. The profile-set menu now creates an empty set, duplicates
the selected built-in or user set, imports an input.conf file and reveals the
active profile. The native import chooser is asynchronous. Profile rename, delete
confirmation, and the remaining full-map/action coverage remain open.
Unsupported custom actions are consumed with a status message. Windows
`WM_APPCOMMAND` is wired through the active mapping and system-control preference;
hardware input remains unverified. Action-builder
parity, full mapping coverage for the five built-in profiles, key recording,
selection-aware removal, macOS remote-command integration and matched pixels
remain incomplete. Each selectable built-in profile now has an original starter
map, and supported mpv actions route through a bounded argument parser. Runtime
GUI interaction and the Windows page have not yet been captured against the
macOS reference.

### Utilities page

The pinned page 11 capture shows the four section hierarchy and a scrollable
content column. Riina now renders those sections and their action rows. Confirmed
Clear Saved Playback Progress deletes only files inside Riina's watch-later
directory and leaves that directory ready for future progress writes. The default
application action opens Windows Default Apps; Restore Alerts reports the current
source-equivalent empty state. Clear Playback History confirms and removes Riina's
persisted recent-media entries, refreshes the live welcome list, and prevents the
current item from being re-added at shutdown until another media item opens. Riina
does not yet have IINA's separate playback-history database. Thumbnail previews now
have source-matched disk entries and budget eviction under the Riina cache directory.
Browser extensions remain absent. The latest
Windows render, focus treatment and matched pixel comparison remain unverified.

The Plugins page now uses the captured overview card, GitHub/local-package actions,
update status, refresh affordance and 60-point installed-plugin rows. Its row model
is data-driven, so an empty installation is represented honestly instead of showing
the reference machine's installed plugins as if they were present. GitHub installs
now resolve the latest release package or the repository's main-branch source archive,
validate it through the shared package staging path and review requested permissions
before install. Refresh now checks installed GitHub plugins' `ghVersion` metadata in
the background. Updates can be confirmed from the plugin information action and
replace the package while preserving enabled state. Live update behavior, exact store
behavior, permission and update-state appearance, and a matched Windows capture
remain open; this layout increment does not close plugin parity.
The row columns now follow the capture: a bounded plugin-name field, version and
overflow action grouped together, with the information control aligned at the far
edge. Disabled switches also reject Space/Enter, matching their disabled pointer
behavior. These are source-layout and interaction corrections only; the rendered
Windows page still needs a matched capture.

The pinned General page exposes a four-item section list: Behavior, History,
Playlist and Screenshots. Riina now follows that navigation pattern and separates
the existing pause/fullscreen, resume-position and screenshot controls into their
corresponding sections. Settings search also opens the matching General subsection.
The additional IINA window-lifecycle and playlist policies, source-matched row
details, and macOS/Windows pixel comparison remain open.
The settings base now uses the captured white content surface and light-gray
sidebar; the settings cards retain their inset fill and border.
Pixel samples from the Plugins image also set its overview/list fill to `#fafafa`
and the row separators to `#e1e1e1` across the complete card width.
