# Current project status

## Application state

The pinned IINA Default input profile includes both `m` and `Meta+/` for mute.
Riina now seeds both bindings; the primary modifier displays as Command on macOS
and Control on Windows/Linux. Runtime shortcut interaction remains platform-GUI
verification work.

The same profile now seeds IINA's primary-section bindings for frame stepping,
subtitle seeking, file-loop toggle, audio/subtitle delay adjustments and reset,
fullscreen Escape/Enter, and online subtitle search. The shortcut labels use
platform primary-modifier conventions. Live shortcut delivery and appearance
still need GUI verification.

The remaining Alternative mpv bindings are now represented too, including app
quit, explicit progress OSD, long seeks, subtitle scaling and secondary subtitle
visibility. The full input-file map and its additional system-media keys each
resolve to a player or application action; physical keyboard and remote-key
interaction remain unverified.

The macOS shared libmpv renderer now receives ICC data from the active window's
display and refreshes it when the player moves between displays. Riina enables
automatic ICC selection after that profile is installed and disables it during
explicit HDR output. The default-on Load ICC profile and Enable HDR mode settings
now appear as separate Video preference cards; the ICC switch persists and updates
the live renderer. Apple-target compilation, profile-change notifications, color
validation, and matched card geometry remain open.

Player, Settings, Welcome, About and the auxiliary Slint windows now receive an
AppKit system-font family default on macOS, while preserving explicit subtitle,
monospace and font-picker choices. macOS Picture in Picture uses native AVKit, not
this Slint font path. Windows receives an empty family and keeps its normal fallback.
Mac font resolution, weight mapping, line wraps and matched visual captures remain
unverified.

The Stop intent now writes MPV's watch-later position before stopping active
playback when `save-position-on-quit` is enabled, matching IINA's explicit
`savePlaybackPosition()` call. It skips the write when resume saving is disabled
or media is not active. This is covered by core command-plan checks and native
property decoding. The focused Windows libmpv test confirms the record is written
before the player is dropped and that reopening restores the saved position; the
shared menu's live GUI interaction remains unverified.

The font picker now reads OpenType static width classes and uses a centered
horizontal preview scale when the selected condensed/expanded face has a normal-
width sibling. It prefers a normal-width face when family and weight choices are
otherwise equal. This compensates for Slint's lack of a font-stretch property;
variable-width axes and exact native glyph matching remain open. Unit coverage is
in place; rendered font-picker comparison is still required.

On macOS, system Reduce Motion now selects the IINA-style sidebar fade while the
explicit Riina Disable Animations preference still makes that transition
instantaneous. Reduce Motion remains a global animation-disable signal on Windows
and Linux, where that is the platform adaptation. macOS source and runtime
verification are still pending. Ordinary motion now slides the panel from its
edge over 240 ms and shifts the title row and bottom controls by the sidebar
width. Reduce Motion keeps those bounds in place during its fade; exact AppKit
spacing and live geometry still need runtime comparison.

### Current window and controller work

The Audio Quick Settings equalizer now puts profile management in the profile
selector menu, matching IINA's Save, Rename, Remove, separator, Manual and preset
ordering, with a second separator before saved profiles. Save and rename ask for a
profile name in a small popover; the permanent
name field and adjacent action buttons are gone. Profile actions are enabled for
the current profile type, and saved profiles remain selectable from the scrollable
menu. A rejected Save/Rename keeps the prompt open so the entered name can be
corrected, including when Rename repeats the current name. Profile names retain
the exact entered text, including surrounding spaces. The shared UI compiles on
Windows. Rename opens with an empty field, and live validation appears inside the
prompt while disabling Save for an invalid or duplicate name. Duplicate checks
use the visible preset and user-profile labels. Profile identity is now tagged as
Manual, built-in, or user-defined, allowing a saved `rock` profile alongside the
built-in Rock preset without selection or persistence ambiguity. GUI verification,
a native macOS alert, and matched macOS pixels remain unverified.

On macOS, the player now merges its Slint About, update, Preferences, and Quit
actions into the native application menu, retaining AppKit Services and window
visibility commands. The menu is normalized once after the player appears and
again when a native menu begins tracking, covering Slint's later menu rebuilds.
The Windows build passes; an Apple-target check is blocked before Riina code by the
missing cross-target C compiler for `ring`. Menu activation and matched Mac pixels
still need verification on the Mac reference machine.

Small video windows keep the transport centered and the timeline visible. The
volume and toolbar groups disappear together when there is insufficient space,
following the pinned IINA floating-controller behavior. Live Windows captures
confirmed the portrait layout, a native border drag to a small landscape window,
and restoration of the full controller on maximize. The gear opens Quick Settings;
the native Video menu then displays “Hide Video Panel” and its shortcut.

The controller now compresses to a 50-point bar with 14-point transport buttons
and a 16-point timeline when the viewport is under 402 points wide or 210 points
high. Compact mode hides the volume slider before configured toolbar actions,
reduces those action buttons to 24 points, and suppresses previous/next below
330 points. Hover/focus behavior in the compact state and a same-state IINA
comparison still need capture.

At narrower widths, the compact bar now keeps only Play/Pause and the timeline
below 330 logical points; previous/next return once the bar has enough room for
comfortable hit targets. Its floating width is also clamped to the video surface,
including transient sizes below the normal minimum, and Play/Pause yields its
minimum width rather than overflowing the row. A live visual check is still
needed to confirm behavior at the user's reported small video sizes.

The last extreme-size pass removes the compact row's horizontal insets and gaps
below 64 points, and clamps its row height and vertical spacing to the available
height below 24 points. A fresh 180×72 Windows autoplay audit stayed on-screen
with aligned host and video surfaces through a clip change and shut down normally.
The captured viewport showed video but not the controller, so tiny-state control
pixels and interaction remain unverified.

The Windows native-video transport now shares the title bar and sidebar's 60%-opaque
dark Acrylic tint over the sampled blur; its former darker fill masked more of the
material. The stored quick-settings capture predates this change, and live UI capture
is unavailable in the current desktop session, so transport transparency and contrast
remain visually unverified.

The floating controller now stores normalized horizontal center and bottom-gap
positions, following IINA's 0.5/0.1 defaults. Drag updates are bounded to the video
surface, snap to the center within five logical points, and save at release; the
bar recenters when the viewport is narrower than its 460-point minimum. Position
values are validated and carried through player, layout, sidebar, and screenshot
preference saves. Pointer routing during child-control interaction, post-resize
placement, and GUI comparison remain unverified.

The floating bar now keeps its 460-point width until the video client is narrower,
then compresses with a one-point inset and remains centered until the full-width
bar fits again. The previous layout reserved 24 points at every width and left
the bar on the leading edge when it compressed, unlike IINA's centered narrow
state. The Windows build and a matched narrow-window capture are pending.

Windows aspect constraints now apply to the native user resize gesture rather
than every `WM_SIZE` notification. Unchanged menu row models remain stable across
playback clock updates, preventing repeated native menu rebuilds. Media sizing
uses generation identity, including loads whose intermediate Loading snapshot
was coalesced, and runs after automatic Music Mode transitions. A bounded native
resize measures wrapped menu rows and moves/sizes around one saved center.

During mixed-aspect autoplay, the framed video host, mpv video child, and
transparent controller are now submitted together for a Windows resize. A
35-second loop recorded zero video-child/host size mismatches across 2,082 stable
samples, zero off-screen samples, and normal shutdown; the previous build had one
child-size mismatch during a clip switch. A follow-up found the aspect-lock path
could still resize during autoplay when the configured timing was manual-only.
Both media and aspect changes now honor that timing, and Windows native playback
prefetches the next playlist URL. A fresh 35-second pixel audit found no window
resize during video-to-video autoplay and no host/child size mismatch. One brief
black frame remains as a higher-resolution clip begins, so transition rendering
is still open. Exact build and audit details are in `docs/verification.md`.

The current display is physically 3840×2160 at 300% DPI, with a 3840×2016 work
area. Older 1280×720/1280×672 measurements below were DPI-virtualized coordinates;
they are not a second physical display configuration.
Matched macOS pixels, multiple monitors, unusual aspect extremes, auxiliary-window
coverage and macOS/Linux runtime remain open.

A follow-up now keeps the captured outgoing frame on top for the entire Windows
handoff; incoming raw captures serve only as a release signal after the native
surface settles. The compact floating transport hides its previous/next buttons
until there is room for their full hit targets. The user reports that the whole-
screen white flash was no longer visible in the latest playback check (“seems
ok”). Automated pixel capture was unusable, so this is a user-observed improvement
rather than a captured visual pass; affected media and rapid clip switching remain
open for follow-up.

OpenSubtitles results now support modifier-click toggling, Shift range selection,
Select All and sequential batch downloads. OpenSubtitles and Assrt are selectable
providers with separate secure credentials. Assrt fixture tests cover token
validation, response parsing including its empty-object no-results form, status
errors, direct file types and secure download host checks. Shooter.cn is now the
third selectable provider; its bounded protocol fixtures pass in the full
workspace suite. The Subtitles menu now labels the configured default source and
offers an explicit OpenSubtitles, Assrt or Shooter submenu on shared and
Windows-native menu surfaces. Choosing a source opens the existing result picker
for that service without changing the saved default. Plugin provider
registration, IINA's direct-result download behavior, live service playback and
native dialog visuals remain open.

Riina is an established Rust media-player application, beyond its initial feasibility
and playback milestones. The current development version is 0.3.8. It opens and plays local media through libmpv on Windows,
with native D3D11 video presentation, working transport controls, and broad player
and preferences workflows. Regression tests cover shared state, playback, settings,
file handling, and lifecycle behavior. The pinned Windows libmpv also passes a
native HTTP playback check that serves the repository's moving-square fixture over
localhost and verifies advancing playback, duration, and the selected audio track.
This does not verify physical audio hardware, public network streams, or macOS playback.
The libmpv event wakeup now coalesces repeated native notifications into one
bounded-queue message, preserving room for playback commands during event bursts;
focused queue-pressure tests cover coalescing and recovery after a full queue.
The stored Disable Animations preference combines with live Windows, macOS, and
Linux desktop Reduce Motion settings while the player or reusable Preferences
window is open. Linux reads the standardized XDG Settings value and follows its
change signal; native portal behavior and the complete animation inventory
remain open.

Interface > On-Screen Controller now persists independent visibility choices for
all nine IINA toolbar action types, including Full Screen, Music Mode, Subtitle
Track, Screenshot, and Live Text. Each action can be moved up or down in the saved
order or repositioned with a drag handle; the default remains Plugins, PiP,
Playlist, Quick Settings. Unit tests cover forward/backward reordering and reject
invalid order data. Matching IINA's customization presentation and live drag
interaction review remain open.
Live Text is removed from the toolbar unless enabled and available, matching its
source preference gate. The preference list can restore IINA defaults.
Command-line HTTP(S), RTSP(S), FTP(S), RTMP, SRT, UDP, SMB and `ytdl://` media
references share URL validation with the Open URL workflow and retain their
position among local paths. Common stream-scheme classification and validation
are unit-tested; live non-HTTP URL playback and the wider IINA command-line
option grammar remain open.
Shared selection resolves macOS Finder aliases before folder expansion; the native
alias API is source-implemented but awaits an Apple-target build and Mac runtime
verification. Windows Shell `.lnk` shortcuts now resolve on the file-discovery
worker before directory expansion; the Windows workspace compiles this path, but
native shortcut interaction remains unverified.
The General settings now preserve IINA's allow-multiple-player, simultaneous-open
grouping, and duplicate-player defaults. Grouped batches bypass duplicate filtering
as in IINA; separate-player batches still filter already-open and repeated media.
The Settings > General surface now stacks Behavior, History, Playlist, and
Screenshots in one continuous scroll page, matching IINA's page model; search jumps
to the matching section. History now has IINA's default-on Open Recent and Track all
played files switches, persists them independently of playback history, and provides
an app-owned File > Open Recent menu with reopen and clear actions on the shared and
Windows native menu surfaces. The menu records file-picker selections and optionally
records successfully loaded media. File > Open URL now has a focused URL-entry dialog
and the IINA Shift+Command+O equivalent on macOS (Shift+Control+O on Windows), routed
through the shared URL validation/open path. Workspace compilation passes; live menu
and URL-dialog interaction,
restart behavior, matched visuals, and AppKit's system recent-document integration
remain unverified. Playlist metadata preferences now default on with metadata shown
only in Music Mode by default; supported local audio tags are loaded asynchronously
and rendered as title plus artist in playlist rows. Broader format coverage,
playlist-settings interaction, and matched macOS row geometry remain unverified.
Playlist's manual-open repeat preference is persisted and dispatched as file or playlist loop
intent; live interaction and matched rendering remain unverified.
The Windows native Video menu exposes video-panel and track selection, PiP,
fullscreen, Float on Top, aspect lock, Live Text, and Music Mode through the
existing player actions. The new macOS reference set confirms IINA's Video menu
also includes Half, Normal, and Double Size, Fit to Screen, Bigger Size, and
Smaller Size, plus delogo and video-filter menus. Half/Normal/Double, Fit to
Screen, Bigger, and Smaller now route to the shared window-size policy for both
the shared player and Windows native-video host. The size policy uses the current
video display bounds, rotation, physical-resolution setting, monitor bounds, and
IINA's 25-point Bigger/Smaller step. Fit Window to Video belongs in IINA's Layout
sidebar and has been removed from Riina's Video menu. Fit to Screen centers a
shrunk window in the current monitor's usable bounds; the other size commands
preserve video aspect while resizing around the window center. Video-size
commands now enable when the first valid natural dimensions arrive, even if
libmpv reports them after the initial loaded state; replacement/loading and
stopped media clear the previous dimensions. Focused tests cover source and
display-size selection, rotation, and invalid geometry. A fresh visual menu
check is still needed. A moving-square fixture debug run started, but the current
desktop-control session exposed no inspectable application windows, so live menu
behavior remains visually unverified.
Delogo reuses the source-space interactive selector and playback-pause lifecycle,
then applies a labeled `lavfi` delogo filter; the checked menu item removes only
Riina's Delogo filter. Shared and Windows native menus are wired. The Video
Filters window lists active filters, accepts custom mpv filter strings, and
offers all eleven IINA presets with parameter forms for crop, expand, sharpen,
blur, delogo, 3D LUT, and custom mpv/FFmpeg filters. It removes active entries,
saves and edits named filters persistently, and toggles or deletes saved entries
from the window and Video menu. Preset controls use the reference's descriptive
parameter labels. Active filter strings can be edited in place and applied while
preserving the rest of the observed chain; the pinned IINA filter window has no
active-chain reorder control, so the observed mpv order is retained.
Saved filters can record, persist, display, and dispatch modifier-aware shortcuts
from the player window and menu, with platform-specific modifier labels. The 3D
LUT form now has a native file picker to populate its path without manual entry.
The window follows the app's
live light/dark appearance. Workspace formatting check, `cargo check`, and strict
Clippy pass. Live filter rendering, window interaction, cancellation,
and matched visuals remain unverified for grouped GUI review.
See REF-30 in
`docs/visual-reference-audit.csv`.
The top-level menus now follow IINA's app, File, Edit, Playback, Video, Audio,
Subtitles, Plugin, Window, Help order on both shared and Windows-native surfaces.
Video, Audio, and Subtitles menu commands now switch between “Show … Panel” and
“Hide … Panel” based on whether that exact settings tab is open on the configured
settings side. The Plugin menu also follows the plugin sidebar's visibility.
These match IINA's dynamic panel commands; live menu interaction remains visually
unverified.
File, Playback, Video, Audio and Subtitles sections now follow the pinned
`MainMenu.xib` order and separators for implemented entries, including nested
audio-track choices and the reference's alternate speed, volume and delay steps.
Shortcut metadata is configured for the menu's right-hand column using Command
on macOS and Control on Windows for application commands. Profile-backed player
actions now derive their shortcut hints from the active key-binding profile in
both shared and Windows-native menus; unsupported or absent actions leave the
hint blank. Broader mapping coverage and live profile-switch verification remain
open. Windows
Playback and Audio menus in shared-renderer and Windows native-video modes now
show Resume/Pause with Space, live speed, live volume with the muted suffix, a
checked Mute item, and current audio delay. The Windows menu bridge copies these
values from the shared playback snapshot. Speed adjustment entries use IINA's
“by” wording, matching their multiplicative behavior. These labels are
source-wired. Frame-step, fine speed, fine volume, and fine audio-delay alternates
now replace their primary rows while Alt/Option is held, matching IINA's menu
alternate behavior in the shared menu and Windows native-video menu. Live modifier
interaction and matched visual verification remain open.
The Windows native-video menu also refreshes pause, speed, volume/mute and audio
delay synchronously on `WM_INITMENU`, before the first menu rows paint; live menu
opening and matched platform visuals still need verification.
Audio, Subtitles and Plugin menus route their
implemented actions to existing player commands. The File menu's Open in New
Window action is source-wired to the multi-file picker and separate-player
workflow while preserving the current session; live selection and startup remain
unverified. Commands without an equivalent yet remain disabled, including
Edit, Window and Help links and the online-subtitle-provider submenu. The
Subtitles menu selects primary and secondary tracks,
toggles primary and secondary visibility, changes subtitle encoding, adjusts or
resets scale and delay, and opens the reusable font picker through existing
command paths. The Windows native-video menu forwards the same observed track
catalogs and actions to the shared player. “Save Downloaded Subtitle…” is enabled
only for a selected primary subtitle in Riina's online-subtitle cache and copies
it through the platform save dialog. The provider selector, localization, dynamic keybinding equivalents, native
Macintosh menus, and live menu interaction remain open for implementation or
grouped GUI review.
About Riina now opens a reusable 640×400 window from the shared and Windows-native
menus. It shows the application, libmpv, and FFmpeg versions, and has License,
Contributors, and Credits sections with project links. Build/date metadata,
contributor collection and credits content, native macOS layout/materials, and
live window behavior remain open.
Quit Riina now routes from both menu surfaces. The shared-renderer manager saves
and tears down every player session before exiting; Windows native-video invokes
its existing full player shutdown. Live quit/restart behavior and macOS lifecycle
verification remain open.
Window > Minimize and Zoom route to the active shared or Windows native-video
window's platform minimize and zoom/maximize operations. Bring All to Front now
restores and requests focus for every player and welcome window managed by the
shared-renderer application path. The Windows native-video path still runs players
in separate processes, so it cannot yet activate all of those windows together. Live
operating-system behavior and matched macOS menu captures remain open. Workspace
`cargo check`, formatting, and strict Clippy pass for this change; GUI activation
remains deferred to grouped visual and interaction review.
“Show Current File in Finder/Explorer” now invokes the existing
playback-aware local-file reveal action on both the shared and Windows-native
menu surfaces. The label names Finder, Explorer, or the generic file manager for
the current platform. It follows IINA's local-versus-network enablement rule.
File > Close now dispatches through the same player lifecycle used by the window
close control, including session save, player teardown, and last-window policy.
“Save Current Playlist…” writes the ordered current playlist as UTF-8 M3U8 using
the platform save dialog. File > Delete Current File now moves a local file to
the operating system Trash/Recycle Bin and removes the matching playlist entry
after the move succeeds. Live delete, save, reveal, and close-menu behavior remain
unverified. File > Playback History now opens a reusable 800×600 history window
with Date or Folder & Website grouping, filename/full-path search, progress and
played-time columns, reopen/reveal/remove actions, file-to-bin, and confirmed
clear-history behavior. It refreshes when the recorded history changes and keeps
selection attached to the media path as newest-played ordering changes. Live
interaction and matched history-window visuals remain unverified. Native menu interaction and matched
captures are still incomplete. A live Windows playback-window check confirmed
all ten headings fit in one row at maximized width and the File menu shows
Ctrl+O and Ctrl+Shift+H in the trailing shortcut column. At 320 px mini-player
width the embedded menu bar wraps into two rows; exact macOS rendering and
matched captures for other menus remain open.
General > Playlist now saves IINA's default-on Add files in the same folder
automatically option. A worker scans direct playable siblings, sorts videos and
audio in separate native-natural order groups, and inserts only missing files on
the correct side of the current playlist entry. The Windows workspace compiles the
path; live playlist ordering and the macOS capture remain unverified.
Later local, URL, and recent-media opens use in-process player windows on the
shared-renderer path, including macOS. The Windows native-video path still launches
separate processes. Playback history state is shared by in-process player windows,
and open history views refresh from the shared revision. When duplicate playback is
disabled, an in-process request for media already open now restores and focuses the
matching player instead of reporting a failed open. The selection policy has focused
unit coverage; live focus behavior, plugin state, cross-process history/duplicate
synchronization, and native-video lifecycle verification remain open.
General settings now persist IINA's quit-after-all-windows-close preference
(default off) and keep-window-open-after-playback preference (default on). On the
shared-renderer path, closing the last player restores Welcome when quitting is off;
Windows native-video now consults the quit preference on window close and the Quit
command always exits. Its setting policy is unit-tested; live OS close/reopen behavior
and a visible reopen path while the native process stays alive remain open. The
keep-open setting reaches new and active libmpv sessions; EOF replay and playlist
boundaries still need native playback verification. General > Pause/resume when now
stores fullscreen entry/exit settings and applies their pause intent when fullscreen
changes; the default-off minimize policy now pauses only active playback and
resumes only the same media generation when restored. Its 50 ms state sampling
still needs native lifecycle verification. The default-off macOS inactive policy
now tracks registered player windows, pauses only when focus moves away from
player playback, and restores only the media generation it paused; its native
timing still needs verification. The default-on macOS sleep policy is implemented
but still needs a native build and playback verification. Timed
OSD now also reports observed volume/mute, audio delay,
subtitle timing/visibility and selected track changes; exact rendering and
remaining OSD event families remain open; state-based rotation, crop, subtitle
scale, loop mode and A/B loop feedback plus stop/chapter/playlist-action messages
are implemented. Focused regression tests now cover delay direction/progress,
separate primary and secondary subtitle delay/position/visibility, selected-track
identity and the disabled `#0 <None>` choice, rotation, reciprocal subtitle scale,
and playlist-loop feedback. Track-selection feedback includes the selected track
ID and localized language name. When one native observation contains both updated
A/B loop endpoints, Riina queues feedback for both in A-then-B order; a simultaneous
removal of both points produces one clear message. The A-B Loop menu label now follows the active profile binding in both player surfaces. Matched OSD rendering, live
locale changes, and native timing remain open. Stop, chapter selection, playlist
append, paste, and clear confirmations now share typed action-feedback messages
with regression coverage for the IINA reference text; action acceptance timing and
the stopped-state hold remain GUI-verification gaps.
Wheel Auto seek now starts with exact seeking and measures the first native
seek-to-playback-restart interval for each media file; later Auto seeks use exact
mode below 50 ms and keyframe mode otherwise. The render path reports composed
frames to libmpv immediately before Slint presents them. Scroll gestures now keep
the first non-zero native axis through diagonal motion. A retained AppKit event
monitor supplies native macOS phase, precision, natural-direction, and momentum
values to the shared scroll action. Trackpad seeking now pauses at gesture start
and resumes only the same media generation if it was playing before the gesture;
abrupt momentum after the pointer leaves is filtered. By default the timeline
forces seek and the volume sliders force volume, independent of the configured
scroll action. The two IINA slider-scroll opt-outs now persist under Control
preferences and appear in Interface > On Screen Controller; when enabled they
swallow wheel input over their slider while preserving click and drag. AppKit
integration, native event timing, paused-seek progress, live wheel behavior, and
matched settings visuals remain unverified.
Scroll action math now keeps native delta precision separate from the gesture
class: phase-empty precise mouse wheels use IINA's mouse seek/volume curves, while
trackpad gestures use the configured trackpad sensitivities. The Control pane's
four-step Relative Seek Amount maps to IINA's independent mouse and trackpad seek
tables. Existing local `seek_sensitivity` values remain readable as an alias.
macOS pinch gestures now route through each player's Winit event filter: the
default action resizes around the window center within monitor and minimum-size
bounds, while the alternate action toggles fullscreen in the pinch direction.
Second-stage trackpad pressure dispatches the saved Force Touch action once per
press. Target compilation, real hardware interaction, and matched captures remain
unverified; Pinch and Force Touch are macOS-specific input capabilities.
The saved Accept First Mouse preference now configures Winit's macOS window
attribute at app startup. Winit does not expose a runtime setter, so changes apply
after restarting Riina; the setting currently affects the whole Slint surface,
while IINA scopes it to video and loop-slider views. Dynamic, view-scoped behavior,
macOS compilation, and first-click interaction remain open.

Subtitle Auto Load applies comma-separated priority keywords to external tracks
and now expands the default `./*` and configured subtitle search directories on a
background worker for each local playback generation. It filters IINA-supported
subtitle extensions, matches media filename prefixes or containment according to
the selected mode, and loads matches with mpv's non-stealing automatic-selection
flag. In intelligent mode it recognizes numbered episode groups from three or more
sibling media files, recursively groups differing filename prefixes, matches reciprocal
nearest prefixes, and compares the episode name within each group. These group matches
run alongside the token-based series matcher, which handles multiple varying numeric
identifiers such as season and episode numbers. A bounded reciprocal nearest-name
fallback remains available when those matches find nothing; explicit episode-number
conflicts and subtitles matched to neighboring media are excluded from that fallback.
Configured priority keywords then match the extensionless subtitle filename stem
case-sensitively, excluding directory names and extensions, to choose a preferred
external track and order discovered candidates consistently.
Riina supports manual OpenSubtitles and Assrt search and download. Each provider's
credential is stored in the platform credential store and excluded from settings
JSON. Assrt search is title-based, caps results and response sizes, accepts only
secure service download URLs, and supports direct subtitle files plus bounded
in-memory ZIP archives;
downloaded files are added to the active player. The results identify Assrt's
upload date and include the service attribution link required by its API terms.
ZIP results are unpacked in memory with bounds on entry count, file paths,
symlinks, and expanded subtitle size.
Automatic search uses the selected provider for eligible local media above the
configured duration threshold when no subtitle tracks are found after discovery
settles; it now submits the search when opening the result window and runs at most
once per eligible media generation. OpenSubtitles still uses preferred languages
and the available media hash. Shooter.cn uses four sampled-content hashes with
an explicit Settings and search-dialog disclosure that it receives the local
path and hash. Plugin-registered providers remain open.
Live service requests and playback, Settings/search window visuals, provider
error handling,
and macOS behavior remain unverified. API behavior was derived from
https://2.assrt.net/api/doc and the pinned provider catalog in
`reference/iina-upstream/iina/OnlineSubtitle.swift` and
`reference/iina-upstream/iina/AssrtSubtitle.swift`.
The Auto Load matcher uses bounded prefix and suffix distances for
numeric-pattern groups and now performs bounded recursive filename-series grouping,
reciprocal nearest-prefix assignment, and per-episode name matching across differing
media and subtitle prefixes. Subtitle groups of any size participate in nearest-prefix
selection, while media groups require at least three siblings. These results run
alongside direct and token-based matches. Deterministic tie handling, end-to-end
playback/captured UI comparison, and matched macOS visual behavior remain open.

Windows 11 x64 is the supported application target. macOS and Linux application
ports are in development and are not supported yet. Full application builds, visible
playback, input, dialogs, window lifecycle, and shutdown evidence are required for
support.

## Parity state

The pinned baseline is official IINA v1.5.0. The parity inventory currently traces
244 atomic requirements, and the inventory is still incomplete. Many requirements
remain partial or unverified. There is no defensible overall parity percentage.
Full behavioral coverage and measured, full-surface visual parity remain primary
acceptance goals. Use `docs/parity.csv`, `docs/interface-parity-plan.md`, and the
captured reference surfaces to choose and verify work.
As of 2026-10-12, the current macOS target is Golden Gate 27.0.1 ([Apple's release
list](https://support.apple.com/en-us/100100)). All accepted IINA captures currently
on file were taken on macOS 26.6.2 and need a 27.0.1 recapture before they can serve
as latest-version visual references.

The audio sidebar now has ten adjustable equalizer bands, all pinned built-in
curves, and persisted named user profiles. Its portable command manages only
Riina's labeled FFmpeg filter, preserving unrelated audio filters, and observes
the active gain curve from libmpv. The first implementation is in place; native
audio output, profile lifecycle interaction, and visual comparison still need
verification.

The separate Audio Filters window follows IINA's custom mpv and FFmpeg preset
choices. It lists the observed audio chain, adds, edits and removes active
filters, stores saved definitions separately from video filters, and exposes
them through the Audio menu. The `af` observation now retains full filter
objects while still deriving Riina's equalizer state. Workspace compilation
and strict Clippy pass; grouped audio playback/menu review remains pending.

Known substantial gaps include live verification of the new Windows
Picture-in-Picture surface and lifecycle, the macOS PiP surface, unfinished
preferences and plugin behavior, live validation of controller fade timing, and
source-matched menus, sidebars, materials, window
geometry, and complete interaction captures. Plugin enable/disable now persists
state; a background host discovers enabled packages during player startup and owns
bounded runtime sessions until disable or shutdown. Player and optional global
entry files run in separate bounded sessions. Granted log, transient OSD and alert
events reach the UI. Bounded JavaScript timers run on each isolated runtime. The
host continuously drains plugin events and relays bounded JSON messages between
paired global and player contexts. Package plugins can load bounded CommonJS
modules with require(), resolved relative to the active package file and
confined to that package. GitHub plugin installation now accepts a repository URL,
prefers the latest `.iinaplgz` release asset, falls back to the main-branch source
archive, validates the package, and reviews requested permissions before install.
GitHub latest-version checks now compare each installed repository's `ghVersion`
against its `main/Info.json` metadata on a worker. Available updates can be confirmed
from the plugin information action; updates validate the repository identity, review
only additional permissions, preserve earlier grants and replace the package with
rollback while restarting enabled sessions through the plugin host. Live update and
restart behavior remains unverified. Plugin-created player instances, complete host
APIs, capability-failure feedback in Settings, macOS/Linux overlay presentation and
live GUI validation remain open. When the official `io.iina.ytdl` plugin is enabled,
Riina now disables mpv's built-in extractor at startup and on live plugin toggles,
greys out its Network settings, and preserves the saved built-in-extractor choice.
The plugin's quality-selection and download flows, public-site playback, and matched
Network-settings captures remain open. Windows overlays now use a
package-confined WebView2 child over the native video host with plugin messaging
and marked-element hit testing; runtime behavior and matched pixels remain
unverified. Thumbnail extraction and timeline hover preview are now wired into the
player for local videos; disk cache reuse/eviction and mounted-volume policy are
implemented, while live interaction and matched captures remain open. Windows Playback and Video menu clicks have been confirmed by the
user.
The menu is now removed when the native host enters fullscreen and restored in
windowed mode; that visibility change still needs a live check. Other playback
interaction remains under verification.
Windows HDR and native-window work has runtime evidence, but calibrated end-to-end
HDR parity and broad hardware coverage remain unverified. Consult the parity
inventory for the exact state of each requirement.

The PiP lifecycle now follows IINA's distinction between closing and stopping:
closing restores the main-window state without changing playback, while the
explicit Stop control pauses playback. The macOS source owns a public AVKit
sample-buffer controller and playback delegate, with an IOSurface-backed
three-buffer sample producer fed by the existing libmpv render context. The
previous Windows-hosted Apple-target check covered the macOS-gated source, but
the latest workspace check stops in `ring` before Rust type-checking because no
Apple-target C compiler is installed on this host. The app has not been linked or
run on a Mac, so PiP behavior and support are not established.
The BGRA8 producer does not preserve HDR metadata. Window policy/restoration,
live display, seek and pacing behavior, color handling, and matched visual
verification remain open. The Windows projection and interaction still need
live GUI verification. The Mac rendering constraint and remaining surface
experiment are recorded in
`docs/adr/0005-macos-picture-in-picture.md`.

macOS system media controls now have source implementations for remote transport,
custom skip intervals, seeking, playback rate and repeat mode. Now Playing
publishes media, audio metadata, queue/chapter position, current asset URL and
timing while the preference is active. External artwork tracks are published as
MediaPlayer artwork, and embedded covers are captured from the selected album-art
track using a separate paused decoder. When no cover track exists and timeline
previews are enabled, Now Playing captures the timeline frame immediately before
IINA's 60-second artwork position, or before the midpoint for media shorter than
two minutes. The independent decoder does not seek the active player. Missing or
invalid cover art now falls back to Quick Look, with up to three attempts, followed
by the timeline frame when enabled; network resources skip those fallbacks.
The hidden `enableNowPlayingArtwork` macOS default mirrors IINA's default-on
opt-out and is observed during playback so disabling it removes current artwork.
macOS play, pause, toggle, stop, next, previous, forward and rewind commands use
the active key profile, and skip-button intervals are reloaded at most once per
second while controls are active. Native command delivery remains unverified.
External cover files are decoded through ImageIO's orientation-correct thumbnail
path at a 768-pixel maximum, reducing memory use for large source images.
The Windows workspace build and strict Clippy pass. The focused Apple API binding
check passes, but a full x86_64-apple-darwin workspace check stops in `ring` because
this Windows host has no Apple-target C compiler. The macOS target build, Control
Center and hardware-key behavior still require a Mac runner and live verification;
these sources do not establish macOS support.

The Utilities page now reports cache size from Riina's OS cache directory and can
clear its thumbnail cache after confirmation. Its default-application action now
offers IINA's Video, Audio, and Playlist category picker. Windows registers
selected types in the current user's application capabilities and opens Windows
Default Apps, where Windows 11 retains control of the final choice. macOS packages
the app's document declarations and registers selected Launch Services handlers.
Linux writes the current user's `org.riina.player.desktop` entry and applies the
selected MIME associations with `xdg-mime` on a worker so the settings window
stays responsive. macOS consent behavior, Linux desktop
runtime, bundled playback/lifecycle, and live Windows dialog/association
verification remain open. Restore Alerts now follows IINA's confirm-then-acknowledge
flow; as in IINA v1.5.0, there are no active suppressible alerts to reset. Thumbnail Preview settings now
expose the source defaults and save their enablement, cache limit, remote-volume
policy, and width. A separate paused, silent libmpv worker can extract and scale
the 101 evenly spaced frames used by the pinned reference without seeking the
audible player. Its lifecycle now feeds a timeline hover preview, persists
source-and-size-matched JPEG sets under Riina's cache directory, evicts older sets
to the configured limit, and applies the remote-mounted-volume preference through
platform volume metadata. Live player interaction, macOS native verification and
matched GUI comparison remain open; browser extensions remain open.

Player-window file drops now coalesce adjacent Winit drop events into one ordered
media-selection request, matching IINA's batch-open behavior for files dropped on
the main window. Supported dropped subtitle files are retained and added as external
tracks without changing subtitle selection, whether or not playable media is included.
Mixed batches attach the dropped subtitles after libmpv reports the new media loaded,
and a subsequent open cancels a deferred attachment. Small Windows Internet Shortcut
`.url` files resolve validated supported stream targets before media selection. Raw
URL/text drops, dropping onto playlist rows, and live
macOS/Windows/Linux drag-and-drop behavior remain open.

The playlist sidebar now supports additive and range selection, stable-order
multi-row drag reordering, and multi-entry Copy, Cut, and Remove actions. Removal
resolves stable native IDs against one playlist snapshot and adjusts indices as
rows are removed; core tests cover order and stale selections. The Windows app
builds successfully; live selection/removal still need verification, as do drag
behavior, external file drops, edge autoscroll, and macOS interaction comparison.
Selected HTTP and HTTPS playlist entries now have Open in Browser and Copy URL
context actions. URL validation and platform dispatch are implemented; native
browser delivery and the playlist interaction still need runtime verification.

Playlist copy and cut preserve all selected references in playlist order, and
paste inserts before the first selected row. macOS writes individual file URL
objects for current applications and retains IINA's filename-list and plain-text
representations. Paste reads native filename lists and every file-URL or URL
pasteboard item before its text fallback. Native Finder interoperability and full
GUI comparison remain unverified.

Music Mode now replaces its normal speed-adjustment controls with previous/next
playlist navigation while compact mode is active, with direct playlist and
album-art visibility toggles. The compact controller spans the 300-point window
and hides the normal title controls; standard playback retains speed, stop, and
fullscreen actions. Music Mode has hover-revealed close-player and return-to-player
buttons. The volume slider now opens from the compact volume control
and closes when the pointer leaves the controller. Its playlist selects on a
single click and starts the selected entry on a double click. When controls hide, the
transport row gives way to centered title and artist/album metadata while the
timeline remains visible. Embedded art uses a reserved upper viewport, the
visibility toggle covers or reveals that surface, and an original Riina
placeholder appears for audio without embedded art or visible subtitles. Music
Mode artwork height follows embedded artwork dimensions for audio and the
display aspect ratio for video, falling back to square artwork when dimensions
are unknown. The new navigation and artwork graphics are original Riina SVGs.
In Windows Music Mode, the top application menu is hidden so its wrapped headings
cannot cover the compact 90-point transport. The existing music-mode toolbar
provides playlist, artwork, Quick Settings and exit actions; the full menu returns
when normal playback resumes. macOS keeps its native menu bar outside the player
window. A Windows PrintWindow capture confirms the headings disappear in the new
build, but its alpha controls surface is captured separately as black, so transport
pixels and interaction remain unverified.
Artwork sizing has unit coverage for portrait, wide, square and unknown
dimensions, along with optional artwork and playlist height combinations.
Source-matched materials, live transition and popover behavior, native-video
viewport behavior, and macOS capture comparison remain open. The metadata and
transport cross-fade uses IINA's 200 ms interval; its live appearance remains
unverified.

## Licensing and binary distribution

Riina's original source is MIT. The combined application package is GPL-3.0-only.
The v0.3.0 package includes the exact third-party notices and corresponding
sources for its pinned native engine and Cargo dependencies.

The latest published GitHub release is Riina 0.3.5 and includes a Windows x86_64
portable application, checksums, and a release verification record. Its successful
workflow reused a previously verified native engine, passed native playback and
shutdown checks, and audited the exact package against its tagged application source
and native source manifest. The [v0.3.5 release](https://github.com/deminden/riina/releases/tag/v0.3.5)
and [successful release workflow](https://github.com/deminden/riina/actions/runs/38472242460)
are available online.

The v0.3.0 source archive measured 1.60 GB compressed and 2.57 GB expanded.
Its native-source snapshot included 1.01 GB of Git object metadata across 5,568
entries. The v0.3.0 archive remains unchanged. The v0.3.3 release does not attach
duplicate native or vendored Rust source trees: the exact application source is
served from the release tag, Cargo.lock pins Rust sources by checksum, and the
package's `SOURCE-MANIFEST.tsv` identifies each public native repository, revision
and source-file hash. `SOURCE-ACCESS.md` provides retrieval and rebuild directions.
The audit also found a 166 MB Opus DNN model archive among the native build inputs;
the pinned builder recipe retrieves it from its upstream source.

Current release assets:

- [Windows portable ZIP](https://github.com/deminden/riina/releases/download/v0.3.5/riina-0.3.5-windows-x86_64.zip)
- [Tagged source code at v0.3.5](https://github.com/deminden/riina/archive/refs/tags/v0.3.5.tar.gz)
- [SHA-256 checksums](https://github.com/deminden/riina/releases/download/v0.3.5/SHA256SUMS.txt)
- [Verification record](https://github.com/deminden/riina/releases/download/v0.3.5/RELEASE-VERIFICATION.json)

The published v0.3.5 Windows ZIP is 69.8 MB and has SHA-256
`970a2e755449ee00f35acd93a3b2d2ac7f003f9d76672adc41253813743c13b9`.
Its verification record identifies immutable source commit
`7ad5cec27c4de0a8addb93e0c3d53319971c3178`, 101,748 indexed native source files
across 88 repositories, libmpv SHA-256
`1749e6b5519afa383c88442e97026754e4e169bd46107ecc0ab0b08d205ad0af`, and Cargo.lock
SHA-256 `5ceee2866d9afb49677a120b7c88c667277604696bd6b0414c732922094ab1b5`.
The exact package and notices were verified; native playback and lifecycle checks
passed. Workflow `38472242460` skipped rebuilding the unchanged native engine and
reused its successful artifact from run `38138622911`.

Earlier v0.3.3 shipped its Windows portable binary. Its exact 72.6 MB package
matches SHA-256 `2a72ded17b01c928cd0bdc5aec9be31c1ca286ef743bdba5201587b7910d2a74`,
identifies immutable source commit `8cf9cc2`, contains no duplicate native or
vendored Rust source trees, and passes the package audit with 101,748 native
source-file records across 88 repositories. The first tag-triggered workflow's
older strict notice matcher failed; corrected main-branch workflow-dispatch run
`38332824004` then passed native playback/lifecycle tests, exact package
verification, and publication. It reused the verified native engine instead of
running the multi-hour engine build again. Earlier native shutdown failures and
their resolution remain in `docs/native-distribution-audit.json` as
release-engineering evidence.
The x264 encoder is excluded from the release engine; FFmpeg's built-in H.264
decoder remains enabled. The development DLL is not the release runtime. License
terms and package provenance are recorded in `docs/licensing.md` and
`docs/native-distribution-audit.json`.

## Verification state

The parity inventory contains 244 currently inventoried requirements,
while explicitly reporting that the inventory is incomplete. The next release
candidate must run the complete package, corresponding-source, and playback gates. The Unix presentation marker
derives `Clone` to match the Windows adapter's shared timer ownership. The full
macOS/Linux application workflow passed Clippy, workspace tests, and application
builds for both matrix hosts at commit `007e410` in
[run 38167050195](https://github.com/deminden/riina/actions/runs/38167050195).
This establishes successful linking on those runners, not native media playback,
PiP, display behavior, or supported-platform status. Windows compilation/domain
and portable playback workflows are separate checks; consult their linked Actions
runs for live conclusions.

Local verification evidence and exact limitations are recorded in
`docs/verification.md` and the requirement rows in `docs/parity.csv`. GUI, HDR,
platform, or release-package behavior is not considered verified unless its evidence
is recorded for that exact surface or artifact.

Plugin HTTP requests now have a permission-gated QuickJS promise bridge for GET, POST, PUT, PATCH and DELETE, plus `download(url, destination, options)` and `xmlrpc(location).call(method, args)`. Downloads recheck redirect domains, share the active-request limit and timeout, cap binary data at 64 MiB, and write only through the plugin file-permission path. XML-RPC calls encode and parse the standard scalar, array, struct, date, and base64 value families. Exact and wildcard manifest-domain checks run before sending. Redirects are followed only after rechecking each target against the manifest; request count, timeout, payload, and response limits are enforced. The JavaScript bridge, XML-RPC type mapping, download path permissions, and external-network behavior remain unverified at runtime.
The plugin core API now exposes audio, subtitle, and video track snapshots, serialized current tracks, typed track selection, audio/subtitle delay, volume, mute, permission-checked external track loading, `setUIVisibility`, `getRecentDocuments`, `getVersion`, and `getHistory`. Its `core.window` proxy now snapshots loaded state, frame, fullscreen, PiP, always-on-top, visible, sidebar, screens, and minimized state; typed writes route frame, fullscreen, PiP, always-on-top, sidebar, and minimized actions through the UI host. Playback history is distinct from the bounded recent-media/resume list, has a default-on recording preference, is returned newest first with IINA's field names and JavaScript `Date` objects, and can be cleared without deleting recent documents. A versioned `playback-history.json` archive is atomically updated when a media generation is first observed, with migration from the prior preferences field; unsupported or invalid archives are preserved rather than overwritten. Restored entries read their saved progress from libmpv's watch-later files using the observed `ignore-path-in-watch-later-config` setting and mpv-compatible filename hashes. New history entries remain without progress until mpv writes a watch-later record. A Windows debug run visibly played the repository's moving-square fixture and exercised the History-page recording checkbox; the stored value was true after restoring its original state. Windows workspace tests and strict Clippy pass with the window proxy changes; 46 native integration tests remain ignored because they require the provisioned libmpv runtime. This is a focused interaction check, not a comparison against IINA's rendered surface. History is shared by in-process player sessions; the history window remains visually and interactively unverified, and full plugin runtime/window parity remains open.

The plugin host now starts plugins requesting the network permission used by the
HTTP API; the previous capability check rejected them before JavaScript execution.
The permission gate and a localhost HTTP JSON response are covered by tests;
JavaScript Promise and external-network behavior remain unverified. The file API
supports exists, list, named text encodings for reads, UTF-8 writes, delete, move,
and binary handles with seek/read/write/close operations bounded to 4 MiB each.
UTF-8/16/32, strict ASCII, Latin-1, and common WHATWG labels are decoded. Additional
Foundation names now cover ISO Latin-5/Thai, KOI8-R, and accepted Big5-HKSCS and
ISO-2022-JP variant names. The latter variants use the portable decoder's base
Big5 or ISO-2022-JP tables, so encoding-specific extension mappings can differ from
Foundation. Other Foundation-only encodings such as EBCDIC, CNS/EUC-TW, HZ, UTF-7,
and specialized Mac character sets remain unsupported.
Move accepts the reviewed absolute-path permission for either endpoint;
overwriting external files through text `write` is denied to match the plugin API
behavior.
`@data` and `@tmp` are plugin-scoped; `.` and in-root `..` segments are normalized,
root escapes and storage symlinks are rejected, and absolute paths require the
reviewed file-system permission. `@current/<path>` now expands against the current
media directory and requires that grant; `@video/<id>`, `@audio/<id>`, and `@sub/<id>`
resolve the current external track paths from playback context. `file.trash` now
moves a resolved path to the OS Trash/Recycle Bin, following IINA's distinction
between trashing a selected path and permanently deleting private `@data`/`@tmp`
files. Its file-system permission gate has a runtime bridge test; native file-manager
effects and macOS/Linux behavior remain unverified. Unsupported Foundation encodings,
cross-volume move behavior, and exact IINA path semantics remain open.
`showInFinder` now resolves a plugin path and delegates to
the platform file-manager adapter; its external path requires the same explicit
file-system permission. Workspace tests cover its permission gate, but not the OS
file-manager interaction. Handle tests cover binary reads and writes, seeking,
offsets, closing, and per-call limits; separate path tests cover private and granted
external locations. The Rust notice collector still stops at the pre-existing
`webpki-roots` CDLA data license, so the full release notice audit needs review.

`iina.core` now exposes permission-aware media opening, OSD, pause/resume/stop,
relative and absolute seek, playback speed, chapter selection, a playback-status
proxy, chapter snapshots, recent-document and version snapshots, and the
`setUIVisibility` disable-UI state. Calls become
typed events sent through the player's host; local paths require filesystem
permission and supported stream URLs require network permission plus a
manifest-allowed domain, regardless of whether the stream is HTTP, RTSP, SMB, or
another supported protocol. The playback-history getter is implemented, and
the `core.window` proxy is source-wired; plugin command GUI execution and native
window semantics remain pending live verification.

Windows startup placement now centers the realized native frame immediately,
before the player UI/session finishes asynchronous initialization. Playback-driven
resizing still recenters after the final initial size is known. Natural window size
now uses decoded source dimensions, preventing the VO's current output target from
feeding back into the next window-size request. Display aspect calculations retain
post-filter dimensions. A Windows 11 run with the 320×180 moving-square fixture
still ended at nearly the full 1280×672 work area. Windows now reserves a 24-point
DPI-aware inset on each side when calculating automatic video size, so a capped
playback window remains visibly separated from the work-area edges. The rebuilt
app measured at `[127, 24, 1153, 648]` on the 1280×672 work area, exactly centered
with a 24-pixel edge margin. The title showed `track-languages.mkv` despite a
different requested fixture argument, so this confirms playback-window placement
but not the fixture-specific sizing path. Startup sizing and centering use the
current monitor's taskbar-free
work area together. The client-size limit subtracts the measured native frame
insets before the video is sized, so the outer frame still fits. The native outer frame is recentered after Slint's first
layout settles and after command-line media finishes loading; configured
initial-position offsets remain clamped to their selected display. Video-driven
and Music Mode resizes preserve the frame center. Core geometry tests cover
negative-origin secondary displays and oversized offsets. A prior Win32 check
measured full-display centering on a 3840×2160 primary monitor, before this
work-area correction. The rebuilt current app passed workspace compilation,
tests, and strict Clippy, but the desktop capture helper failed with
`CreateForMonitor ... 0x80070057`, so the corrected work-area placement still
needs a visual/runtime measurement in a working capture session. The latest
accessibility-only observation exposed the Welcome controls, but screenshots
still fail with `0x80070057` and UIA click geometry is unavailable.
The client-to-outer-window conversion now uses measured screen positions instead
of splitting the full frame thickness in half; this preserves the actual top
caption inset when it differs from the bottom border. Regression tests cover an
asymmetric caption frame and negative virtual-desktop coordinates. The current
desktop capture session again failed to expose a stable Riina window, so the live
visual confirmation remains open.
Windows now centers the player after its visible surface is created, so idle
launches and cancelled file dialogs do not retain Winit's default placement. The
existing post-media center accounts for playback sizing, and resize or scale changes
settle while a startup center is pending; moving the window does not schedule another
center. An explicit initial-position preference bypasses automatic centering.
The shared-renderer Windows entry point and in-process player windows now use the
same work-area centering policy when no explicit initial position is configured;
the shared-renderer path applies it as soon as its native window exists, with a
short realization fallback. These alternate paths are compile-checked; their live
frame bounds still need measurement because the current desktop capture API fails.
The transparent controls HWND now receives the native host's bounds before its first
show, preventing the separate overlay from briefly appearing at Winit's default
screen position. A Windows playback launch measured the controls outer frame exactly
over the host client rect, and the host retained a 24-pixel margin from the 1280×672
work area. The live screen capture remains unavailable, so rendered pixels are still
unverified.
Auxiliary windows with a hidden or unrealized owner center in the monitor work area
instead of inheriting the owner HWND's default upper-left rectangle. The latest
startup-placement change was measured on the current 1280x672 work area: an idle
launch was centered exactly and a loaded moving-square fixture was within one
horizontal pixel. Both test processes were closed. The desktop screenshot API
returned an all-black frame, so rendered pixels remain unverified; measurements
are recorded in `tests/visual/windows-window-placement-evidence.json`.

The Windows native player host now requests DWM's `DWMSBT_MAINWINDOW` backdrop
and reads back the applied type, with a renderer-surface fallback when DWM rejects
the attribute. Microsoft maps this value to Mica on Windows 11 22H2 and newer.
The debug app reached playback with this call in place, but capture still fails, so
visible Mica, other persistent surfaces, Acrylic dialogs, and video-backed overlays
remain unverified.

## macOS plugin-menu update event · 2026-10-14

The macOS player now observes AppKit menu tracking and sends IINA's
`iina.menu-update` event when the active player's Plugin menu begins tracking.
Because Slint installs the active player's menu on the process menu bar, the
observer filters by both menu title and key-window identity so inactive player
sessions do not receive duplicate menu-open events. The observer is retained and
removed with its player UI session. AppKit exposes this as a did-begin-tracking
notification, slightly later than IINA's `menuWillOpen` delegate callback; live
macOS interaction must verify whether changed items appear on the first open.
The observer source passed an isolated Apple-target compile and strict Clippy
check against a generated Slint player component. A full app Apple-target build
and macOS runtime remain unverified.

IINA's Audio > Audio Device menu was still a disabled placeholder. Riina now builds
that submenu from libmpv's observed device IDs and descriptions, filters devices by
the active output driver, marks the observed selection, and sends the selected native
ID to libmpv's writable `audio-device` property during playback. The shared menu and
Windows native menu share the same typed action. If the selected endpoint disappears
during active playback, Riina requests automatic output. Command validation, selected
row mapping, driver filtering, and one-shot fallback conditions have regression tests.
The provisioned Windows libmpv check exposed three endpoints and verified that
selecting an alternate changed the observed property. Physical hot-plug and GUI
capture remain unverified.

The Quick Settings Audio pane now follows IINA's control order: a bounded audio
track selector first, then external-audio loading, delay, and the equalizer.
The optional volume-boost switch is hidden from Quick Settings at its default-off
state, preserving the reference's direct transition from Delay to Equalizer. It
appears in the Audio pane after the extension is enabled from Audio preferences, so
the user can turn it back off there. Backend track switching is covered by the full
native playback suite. A fresh matched live capture remains unverified.

Audio Delay pointer input now keeps the scrubbed value local while dragging,
snaps it to 50 ms increments, and sends one native update on release, following
IINA's `AudioDelayView.sliderAction()`. Keyboard and accessibility adjustments
still apply through the immediate change path. The adjacent numeric field now
previews the draft using the active locale's delay formatter. Release timing,
native property synchronization, and matched GUI behavior remain unverified.

## Verification sweep and Windows last-window lifecycle · 2026-10-13

The current v0.3.4 workspace passed `cargo build --workspace --locked`,
`cargo test --workspace --locked` (207 app tests), strict workspace Clippy,
formatting, the parity traceability checker (244 inventoried rows; still incomplete),
and the Python tooling suite (19 tests). The explicit provisioned libmpv suite passed
49/49 native playback tests. Windows placement was rechecked: the shared-renderer
player's native outer frame is exactly centered at `[154, 9]` in the 1280×672 work
area. The isolated Welcome frame was also exactly centered at `[354, 99]`.

With `quit_when_no_opened_window=false`, closing Welcome restored the empty native
player and the process stayed alive; closing that player ended the isolated test
process. With the preference enabled, closing Welcome exited. These process-level
checks cover Windows only. Earlier monitor-wide capture calls had failed
(`CreateForMonitor` 0x80070057), and GDI/PrintWindow returned black client pixels.
Fresh per-window capture displayed the Welcome and empty-player surfaces, though not
the video playback surface. The published v0.3.3 package remains unchanged;
v0.3.4 has not been prepared for release.

Windows first-show placement now retries until a native HWND accepts its
position, and shrinks an oversized outer frame to the work area while preserving
its ratio. A runtime check found the old native host at `[0, 0, 1280, 720]` on a
`[0, 0, 1280, 672]` work area. After closing Welcome in the rebuilt v0.3.4 app,
the measured empty-player outer frame was `[176, 24, 1104, 648]`: centered with
24-pixel top and bottom margins. A fresh screenshot showed the title bar, menus,
and player surface. The command-line playback test briefly showed
`moving-square.avi — riina`, confirming filename title wiring; the capture helper
then lost the playback window, and the exact test process was closed. Shared
video pixels and matched IINA comparison remain open.

First-show placement now selects the taskbar-free work area under the pointer
for new windows without a visible owner, then falls back to the primary monitor.
An existing visible owner still takes priority, so auxiliary windows and child
players remain beside the window that opened them. This avoids sending a launch
to the primary display when the user is working on another monitor, and avoids
using Winit's temporary cascade coordinates. Earlier single-monitor captures
measured the Welcome window at `[354, 99, 926, 573]` and playback host at
`[127, 24, 1153, 648]` on a 1280×672 work area. Those measurements remain the
single-monitor baseline; multi-monitor pointer selection still needs runtime
verification. The Windows selection-priority test passes for owner, pointer,
and primary fallback. A playback capture on the pre-change build confirmed live
video and controls, but did not exercise multi-monitor placement. A post-change
Windows launch displayed Welcome and the empty-player surface; the test process
was closed afterward. The active desktop exposes no second monitor for runtime
verification.

The Windows native-video Window menu now enables Bring All to Front. Its handler
matches the exact running executable, restores and asynchronously raises visible
windows from sibling player processes, then requests focus for the player that
received the command. The executable-path unit test and strict workspace Clippy pass; live multi-process
activation remains unverified because the desktop capture helper stopped accepting
the playback window after a Quick Settings click.

Crop OSD now follows the observed managed-filter state: preset names and crop
clearing are reported only after the native filter catalog confirms the change.
Interactive custom crops include their source-pixel origin and dimensions, matching
IINA's readable crop label; moving a custom rectangle while remaining in the Custom
category produces fresh feedback. Unknown filter state is kept distinct from a
confirmed uncropped state. Two focused unit tests pass. A live Windows 11 run
selected Video > Crop > 4:3 and captured the visible `Crop: 4:3` OSD plus the
cropped playback surface; evidence is in `tests/visual/crop-osd-windows-evidence.json`
and its screenshot. Live custom-rectangle selection, exact OSD timing, and matched
macOS comparison remain open.

The IINA 1.5.0 Video Quick Settings pane uses the system aspect-ratio, crop, and
rotation symbols beside those section labels. Riina now uses original 16 px
vector glyphs for the same three headings. Its fixed component height now includes
all three headings, control rows, and layout spacing. The application build passes;
the symbol shapes and spacing still need a live comparison capture before visual
acceptance.

## Windows UI retest · 2026-10-13

The rebuilt 0.3.4 Welcome window rendered through the desktop capture bridge and
reported its version and recent-file controls. Selecting the long playback sample
opened a player titled with the filename, but the bridge then reported that the
player window was not usable for capture after one fresh re-list and rehydration.
The exact test process was closed and no Riina process remains. This confirms the
Welcome capture path only; playback controls and Quick Settings visuals still need
a successful live capture.

On a fresh follow-up, Welcome capture succeeded after selecting a newly listed
window handle. Double-clicking `moving-square.avi` changed the app's listed title
to the playback filename, but its refreshed window handle disappeared before the
capture request could use it. Test process 13952 was stopped and confirmed absent;
no video pixels or Quick Settings interaction were verified.

The rebuilt v0.3.4 Welcome window captured successfully and showed Version 0.3.4
and the recent-media list. Opening `moving-square.avi` changed the app listing to
`moving-square.avi — riina`, but `get_window` rejected the refreshed playback HWND
as unusable after one re-list and rehydration. Test process 22040, launched from
`target/debug/riina.exe`, was stopped and verified absent. Playback rendering,
dynamic menu state, and Quick Settings remain unverified through this capture path.

A second fresh launch used the development binary rebuilt after the Windows
native-video menu-state bridge changes. Welcome capture succeeded again. Opening
`moving-square.avi` changed the listed player title to
`moving-square.avi — riina`, but capture failed as soon as the player replaced the
Welcome surface; a fresh list and rehydration produced the same unusable-window
error. Process 22384 was stopped and verified absent. The native menu labels compile
and pass workspace tests, but live Playback/Audio menus and video pixels still
cannot be visually checked through this bridge.

New player windows opened from an existing player now stay on that player's
monitor. The shared-renderer path centers the child over its owner; separately
launched Windows players inherit the owner's monitor as a startup hint. Previously
both paths could move a new window to the primary monitor. A configured initial
position still takes precedence. Multi-monitor runtime placement remains to be
verified.

Standalone Windows windows now choose the primary display when there is no visible
owner. Their placement no longer follows the pointer, which could be left on a
secondary display or at a remote-desktop edge. A fresh Welcome capture showed the
surface, but the computer-use helper failed while closing its HWND; the exact
debug process was then stopped and confirmed absent. Screen-coordinate placement
and multi-monitor behavior were not visually verified in this run.

The desktop helper again captured the Welcome window and the playback window.
The playback capture exposed the shared playback controls, menu bar, and Video
Quick Settings; the fixture was paused at its final timestamp and the video area
appeared black, so this did not verify video pixels. Quick Settings tab switching
then lost its window handle. Test process 8652 was stopped and confirmed absent.

## Current workspace validation · 2026-10-13

`cargo test --workspace --locked` passed. All 49 ignored native playback integration
tests also passed serially against `tools/mpv/libmpv-2.dll`; its SHA-256 matched the
pin in `reference/native-lock.json` (`a3708b0be3d3b5d8f1fb4c2e2a43346356471a78060e598dab774d00e8cc05f3`).
`cargo build --workspace --locked` completed successfully. The current development
binary launched with `moving-square.avi` and reported that filename in its player
title. The fixture advanced to the following same-folder playlist item during
capture. The Windows capture helper rejected the playback window with `window is
not a usable app window` after a fresh window-list and rehydration retry. Test
process 21816 was stopped; a process check confirmed no Riina process remains.
This leaves live playback pixels, menu interaction, Quick Settings, and multi-
window positioning unverified in this run.

## Windows video handoff follow-up · 2026-10-13

The 0.3.4 release is public with its tested portable x64 package; see the release and checksum evidence in `docs/verification.md`. The local playback handoff fix remains post-release work. A generation-bound frame is retained across autoplay's `Ended` to next-file transition and through a bounded 0.75-second incoming-video window; paused incoming media with observed frame info can release it earlier. Five focused policy tests pass. A clean 23-second synthetic run with resize timing set to Always covered wide, standard, portrait, return-video, and audio clips. Its optimized 320-by-180 center crop recorded 771 samples at a 29.6 ms average and 174 ms maximum gap. There were no all-dark samples at the three video-to-video transitions or the video-to-audio transition. One host/child size mismatch sample occurred during the portrait-to-wide resize; a fresh 18-second repeat had no mismatch samples. Both runs had no off-screen samples and Riina exited normally. Six dark samples appeared during startup/first-frame startup in the longer run. Earlier hidden-window readings were misleading: full screenshots from those intervals still showed video and controls. A mixed-codec run's interlaced center crop was mostly black by fixture content, while a full-window capture showed the rendered interlaced pattern; do not use the crop alone as evidence of blank video. Audio-to-video handoff, the user's media, and flashes shorter than the capture gap remain unverified, so automatic-resize flicker stays open pending a reproduction on those files.

The latest investigation found that mpv's child video HWND could retain the previous clip's dimensions for one sampled frame after the host resized. Windows now realigns every mpv child surface during the host resize notification, and the outgoing frame capture is requested 1.25 seconds before EOF to give mpv more time to return it. A fresh 25-second autoplay run recorded 825 stable samples (27.9 ms average, 214 ms maximum gap), zero host/child size mismatches, zero all-dark samples after startup, zero off-screen samples, and normal shutdown. Full captures at three `IsWindowVisible` false samples still showed the moving video and controls. The reported flash is not confirmed fixed on the user's media; this audit does not resolve flashes shorter than its sampling gap.

## Windows transition rendering follow-up · 2026-10-13

Frame-by-frame captures exposed two gaps the earlier center-crop audit missed: mpv can resize its child HWND during a playlist handoff without a host resize event, and the retained transition image left transparent letterbox/title-row bands where the incoming video showed through. A 16 ms native-surface alignment timer now corrects child and controller bounds during playback. While an outgoing frame is retained, the UI now covers the complete video surface with that frame and an opaque black matte behind its aspect-fit image. The regular video layout is unchanged.

The rebuilt Windows app completed a 32-second autoplay run through wide, standard, portrait, return-video, and audio fixtures. It recorded zero host/child size mismatches, zero off-screen samples, one dark sample during initial startup, and normal shutdown. Transition captures after the first video-to-video switch show a single retained frame with clean matte bars rather than the previous incoming-frame strip. This synthetic fixture still does not verify the user's files or flashes shorter than the capture cadence; keep the issue open for those cases.

## Windows playlist handoff follow-up · 2026-10-13

The user still observed flashing during video switches. The remaining weak points were the fixed 0.75-second release timer and automatic host-window downsizing during playlist autoplay. Riina now requests an incoming raw frame and holds a transition image until that capture succeeds, briefly showing the captured incoming frame while the native surface catches up. A three-second playback-time fallback prevents a failed capture from freezing the cover. On Windows, automatic playlist changes that would reduce the window area keep the current player size; manually opened media still follows the configured resize policy.

A fresh 27-second GUI autoplay run covered wide, standard, portrait, return-video, and audio fixtures. Across 822 stable samples, average capture spacing was 30.4 ms and the maximum gap was 364 ms. The video clips kept one window size across all three switches, the sampled video surface had no all-dark samples after startup, and the run recorded zero off-screen samples, zero host/child size mismatches, and normal shutdown. This verifies the synthetic playlist path, not the user's affected media or flashes shorter than the maximum gap; keep the user's report open until directly checked.

## Windows constrained controller layout · 2026-10-13



The player controller now clamps its floating position to the video viewport. It shrinks to the available height, hides the timeline below 48 points, and retains Play/Pause while removing the two neighbor actions below 114 points of width. A 980-by-330-pixel GUI resize kept playback visible, on-screen and aligned with the native video child. Its screenshot did not show the controller, so button placement and interaction in the constrained viewport remain unverified; no visual-parity claim is made.

## Windows clip-switch flash follow-up · 2026-10-13

The user still sees flashes while switching between videos. The native mpv output
now disables `auto-window-resize`; Riina already owns resize timing and changes the
host and child surface together, so mpv must not independently resize the embedded
window during a playlist handoff. The synthetic run is complete; direct confirmation on the user's affected media remains open.
Synthetic follow-up run with `auto-window-resize=no` completed normally across wide,
standard, portrait, return-video, and audio items. It recorded 1,045 stable samples,
zero fully dark video samples, zero host/child size mismatches, and zero off-screen
samples. This does not establish a fix for the user's media or flashes shorter than
the capture interval.

Online subtitle providers now include Shooter.cn as the third IINA provider.
The opt-in lookup hashes four bounded samples from a local video, clearly
discloses that it sends the file path and sample hash, confines HTTPS downloads
to Shooter.cn domains, and routes selected files through the existing cache and
batch-load flow. The full workspace suite passes 271 app tests, including its
protocol fixtures; strict workspace Clippy and the Windows player build pass.
The live API and native result dialog remain unverified.

The user still reports flashes on video-to-video changes. Investigation found
that mpv can retain a hidden child HWND during swap-chain replacement, while the
16 ms alignment timer was resizing every matching child, including hidden stale
surfaces. Alignment and readiness now consider visible mpv children only; a new
child is aligned as soon as it becomes visible. Confirmation on the affected
media is still pending.

## Automatic Music Mode handoff stabilization · 2026-10-13

Automatic Music Mode now waits for an audio-only track catalog to remain stable for
300 ms before changing layout. This avoids treating mpv's initial audio-track update
as proof that the incoming playlist item has no video. Unit tests cover transient
and stable catalogs. A 25-second Windows mixed-media autoplay run retained the same
window size for four video items, then switched to Music Mode for the audio item;
it recorded no fully dark video samples or host/child mismatches and exited normally.
The user's affected media and macOS runtime remain unverified.

The pinned v1.5.0 mini-player transition hides all main-window sidebars before
the compact surface is resized. A live Windows check exposed that Riina retained
the active Video Quick Settings pane while same-folder autoplay entered Music
Mode, leaving the pane clipped over the compact player. The Slint Music Mode
transition now clears both sidebars before resizing. Workspace tests and build
pass; strict Clippy and GUI verification of the corrected transition are pending.
The affected media and macOS runtime remain unverified.

## Windows video-switch flash follow-up · 2026-10-13

The transition cover now stays up for 750 ms after the first incoming raw-frame
capture, giving the native D3D11 surface more time to begin presenting before the
cover is removed. Focused transition-policy tests, the full workspace tests, strict
Clippy, formatting, and the workspace build passed. A 32-second mixed-size autoplay
GUI run recorded 1,136 stable samples with no dark video samples after startup, no
host/child size mismatches, and no off-screen frames; captured handoffs show the
cover spanning the surface replacement. The test closed normally and no Riina
process remains. The user's affected files and sub-sample flashes remain open.

## Incoming-frame capture stabilization · 2026-10-13

Incoming-frame capture now waits for up to 250 ms of playback before replacing
the retained outgoing image, with a proportionally shorter wait for very short
clips. A six-clip autoplay run with automatic resizing enabled produced no host/
child-size mismatches or off-screen samples. Full-window captures showed the
retained image during sampled native-child visibility changes and no blank gap
in the captured handoffs. The user's affected media and flashes shorter than the
capture cadence remain unverified.

## Windows clip-switch follow-up · 2026-10-13

Incoming-frame capture now waits until the Windows host size has stayed unchanged
for 300 ms, then the retained frame stays over the native output for the existing
750 ms handoff window. Manual Next/Previous also captures the outgoing frame before
issuing navigation, with a 500 ms fallback if capture does not complete. The
workspace build and formatting passed. A 26-second six-clip autoplay run with
automatic resizing enabled exited normally with no host/child size mismatch or
off-screen samples. Screen samples include long dark intervals in generated
interlaced fixtures, so this run cannot distinguish encoded black content from a
brief render gap. A separate Windows smoke check sent Ctrl+Right, advanced from
`01-square.avi` to `02-tracks.mkv`, and exited normally. The user's reported flash
is still not confirmed fixed on their media; manual-transition visuals and
sub-sample flashes remain unverified.

## Advanced preferences and log viewer GUI · 2026-10-13

The Windows Advanced Settings callback now feeds the preferences save path; the
first GUI pass exposed that changes were otherwise only reflected on screen. The
Advanced toggle persisted from disabled to enabled. The log viewer opened with
four fixture rows and showed a fifth row appended while the window was open. Its
captured toolbar now keeps Search and all actions visible at the default size.
The workspace build and format check passed. The harness had to end its exact
Riina test process after normal window shutdown timed out. Filter interaction,
copy/save/follow actions, pinned IINA comparison, and macOS rendering remain open.

## Windows autoplay surface-size stability · 2026-10-13

Windows automatic resize and aspect-lock paths now preserve the existing frame
during playlist handoffs. A further screen audit caught one brief client-area
height change with the outer frame steady, consistent with a native menu refresh
during clip metadata updates. Menu synchronization is now coalesced for 80 ms
during playback-state bursts. A 20-second GUI replay with “Always resize” covered
six generated clips: 644 stable samples kept the host and mpv child at 2747×1711
physical pixels, with zero size mismatches or off-screen samples. Riina closed
normally and no process remained. Interlaced test clips produced dark center crops,
so the user's media still needs checking. Initial sizing and manually opened media
continue to follow the selected resize preference.

## Keep final frame at EOF · 2026-10-13

The pinned-Windows-libmpv integration check now asserts that EOF leaves the current
video metadata and playlist entry loaded, then verifies Play restarts the file from
the beginning. The test passes. Visible final-frame pixels, the other keep-open and
autoplay policy combinations, and full Windows/macOS/Linux presentation remain open.

## Windows clip-switch frame handoff · 2026-10-13

An outgoing-frame capture could finish just after mpv began the next clip and be
discarded as stale, leaving no frame for the transition cover. Capture replies now
retain the generation they were requested for and are accepted for the immediately
following clip only; older replies remain rejected. Further inspection found that
Windows native D3D11 playback also created a second libmpv OpenGL render context,
which drew a second video feed over the native child window. Native playback now
skips that render context and starts its media from the native setup callback. The
transition timer also waits for the mpv child window itself to be visible and match
the host client size continuously before capturing/releasing the cover. Captured
cover pixels are made opaque.

`cargo build --workspace --locked`, format check, and `git diff --check` pass. A
24-second Windows GUI autoplay run covered six mixed-format clips with 995 screen
samples: zero host/mpv size mismatches and zero off-screen samples; Riina closed
normally. Generated interlaced clips still produce dark center samples. The replay
cannot prove the user's reported flash is resolved on their media. Evidence is in
the ignored directory `tools/window-stability-audit/native-render-separation-20261013/`.

## Playback History group navigation · 2026-10-13

Playback History now lets users collapse or expand individual date/folder groups
and provides Expand All and Collapse All actions. The collapsed state survives
history refreshes and clears with the history. `cargo check --workspace --locked`
passes; live interaction and matched IINA rendering remain unverified. History
Multi-selection and multi-row context-action routing are implemented from the
pinned outline selection model and have focused selection-order coverage. The
duplicate bottom action bar is removed so Play, New Window, Reveal, Remove and
Trash use the selection-aware context-action path. Live interaction and native
row artwork remain open for matched GUI verification.

Windows handoff follow-up: the transition image is now assigned before its cover
becomes visible, closing a one-render black-backing race. After a successful
incoming-frame capture, the cover now holds for 750 ms of wall time rather than
media time, so playback speed cannot shorten the settling interval. Its 8× speed
regression passes. A live six-clip screen check showed normal rendering in later
clips, but did not sample fast enough to confirm the reported flash is gone. Keep
the user-media and short-flash verification open.

## Windows incoming-video readiness · 2026-10-13

The handoff cover could still drop early when the incoming clip resumed beyond
three seconds, because its safety window used media position, or when the UI saw
the `Playing` state before mpv had reported whether the clip had video tracks.
The fallback now uses monotonic wall time, keeps the cover while track metadata is
unknown, and begins incoming-frame capture as soon as the track list confirms a
video stream rather than waiting for full dimensions.

Two focused regressions pass, along with `cargo fmt --all -- --check`,
`cargo build --workspace --locked`, and `git diff --check`. A 25-second Windows
autoplay run crossed six generated clips with 572 screen samples, no off-screen
frames, and normal shutdown. The center crop remained dark in the generated
interlaced and low-resolution MPEG clips because of their content; one transient
host/child-size mismatch occurred late in the run. This does not establish that
the user's affected media or flashes shorter than the sampling interval are fixed.
Evidence is in the ignored directory
`tools/window-stability-audit/transition-late-metadata-20261013/`.

## Playlist-row video handoff · 2026-10-13

Double-clicking a playlist row previously bypassed the capture-first transition
used by Next and Previous. Direct row selection now captures the outgoing image,
waits for that specific capture reply, then selects the entry by stable playlist
ID. Each outgoing capture increments a revision so a refreshed frame replaces an
older cached image. The built Windows player completed a mixed-folder UI run in
which the selected `05-return-video.avi` played before autoplay reached the audio
item; the app closed normally. `cargo check --workspace --locked` and formatting
passed. The user's reported flash remains open for verification on their media.

## Windows incoming-frame stability · 2026-10-13

One usable incoming raw frame no longer starts the transition-cover release timer.
The handoff now asks mpv for multiple usable frames spanning at least 400 ms;
after the final capture, the existing 750 ms cover grace still applies. This is
intended to avoid releasing over an output that has only produced a transient
partial frame. Formatting, workspace check, and workspace build pass. A six-clip
GUI replay completed with no host/child size mismatch or off-screen sample and
closed normally. Direct screen capture confirmed the generated video was being
rendered, but the high-rate desktop screenshots were covered by the foreground
Codex window, so this run does not verify the transition pixels or prove the
user's flash is fixed. The affected media still needs a direct check.

Windows clip-switch follow-up: an outgoing frame capture was treated as complete
when mpv accepted the command, even if the asynchronous capture reply failed.
Riina now retries every 240 ms until a captured frame is confirmed. The focused
retry regression, player build, and Windows mixed-codec replay pass. The replay
showed no host/child-size mismatch or off-screen samples; direct full-window
captures during handoffs showed continuous video. The user's media still needs a
direct check, so the reported flash remains open.

## Player-scoped plugin events · 2026-10-13

Plugins can register and remove bounded `iina.event` listeners on their QuickJS
worker. Riina routes `iina.file-loaded` and changes to playback properties already
present in its plugin player snapshot to subscribed player sessions. Focused
runtime/context tests pass. Dynamic `mpv.observe(property:)` is now implemented
with a player-scoped, deduplicated libmpv observer for generic node values; a
pinned-libmpv test confirms a real property update reaches portable playback
state. The wider event catalogue, global event access, full QuickJS-to-GUI
integration, and macOS runtime behavior remain open.

## Windows native-surface handoff gate · 2026-10-13

The transition cover now remains until both incoming captures are stable and the
visible mpv child surface has remained visible at the host's current size. The
previous timer could release on healthy decoder captures before checking whether
the native swap chain was still reconfiguring. All 18 transition-policy tests
pass. The rebuilt Windows app replayed the five-item synthetic folder for 25
seconds: 1,011 screen samples, zero host/child-size mismatches, zero off-screen
samples, no sampled near-black frames after startup, and normal shutdown. These
fixtures do not reproduce the user's reported flash or validate sub-sample
effects. Evidence is in
`tools/window-stability-audit/flash-surface-gate-followup-20261013/`.

## Dynamic plugin mpv property events · 2026-10-13

Player plugins can subscribe to arbitrary single-segment `mpv.<property>.changed`
events. The QuickJS worker requests a bounded, deduplicated observation; the mpv
worker observes it as a generic node, copies the value into portable playback
state, and the host dispatches changes back on the plugin worker. Existing core
snapshot properties avoid duplicate observers. A limit of 128 distinct dynamic
properties bounds player state. Workspace tests, strict Clippy, and a real
pinned-libmpv property-observation test pass. Full GUI/plugin integration, the
wider IINA event catalogue, global event access, and macOS runtime behavior
remain open.

## Windows autoplay video handoff lead · 2026-10-13

For an autoplay playlist item with a successor, the retained outgoing frame now
covers the final 450 ms before the handoff. Previously the cover appeared only
within the final 120 ms or after mpv reported the playlist transition, leaving a
short interval where Windows could expose a reconfiguring native video surface.
The earlier cover is limited to items with an autoplay successor so a final
playlist item does not freeze early. A focused policy regression was added. This
reduces the handoff race window but does not establish that the user's flashes
are fixed; the affected media still needs a direct GUI check.

The cover also survives a late `Ended` or transient `Stopped` snapshot after a
new file generation has begun. mpv's playlist lifecycle notifications can cross
the UI snapshot boundary during a handoff; treating that short phase dip as a
completed stop could disable and then re-enable the cover for one render. Two
focused regressions cover this ordering and its expiry/metadata release. A fresh
GUI replay is still needed to confirm the user's report.

## Open URL protocol coverage · 2026-10-13

The shared URL validator now recognizes common mpv stream schemes including
FTP(S), RTMP variants, RTP, SRT(P), TCP, UDP, MMS, SMB, and `ytdl://`, in
addition to HTTP(S) and RTSP(S). It preserves the user's URL spelling and
rejects unsupported schemes, local file URLs, malformed authorities, and
Windows drive paths as network inputs. All three focused tests pass in the full
workspace test run. This proves shared classification and validation only;
actual playback still depends on the packaged engine protocol set, and non-HTTP
native stream playback has not been verified. Plugin media opens now route all
recognized stream schemes through network permission and manifest-domain checks;
`ytdl://` authorizes against its nested source URL.

## Windows clip-switch stability identity · 2026-10-13

The native handoff gate now restarts its stability interval when the visible
mpv child HWND set, child dimensions, or window title changes. This catches a
same-size renderer replacement that host-size-only tracking could miss. The
Windows player build and strict app Clippy pass. Playback on the user's affected
clips remains unverified. A later user check reports no current flashing; repeated
transitions across the affected clips remain unverified.

## Native menu first-paint state · 2026-10-13

The Windows `WM_INITMENU` callback now synchronously refreshes both playback
labels and the Video, Audio, Subtitles and Plugins panel show/hide labels from
the shared window state. Menus therefore do not depend on the coalesced
80-millisecond menu-state update to display current toggle titles. Workspace
check, formatting and strict Clippy pass. A live Windows native-video check
confirmed the first Video-menu item changes to “Hide Video Panel” after opening
the panel. Audio/Subtitles, Plugins, playback labels and macOS menu behavior
remain unverified; see `docs/verification.md`.

## Font family default face · 2026-10-13

Font catalog rows are now ordered with an upright face nearest regular weight
first, so selecting a family defaults to Regular rather than the lightest
installed face. Focused tests cover regular preference, deterministic fallback
and empty families; the complete workspace test run passes (277 app tests, 50
native integration tests ignored because the pinned DLL is unavailable here).
Rendered row order, pointer routing, native-video and macOS font enumeration
remain unverified.

## Plugin sidebar manifest discovery · 2026-10-14

The pinned plugin contract lets `Info.json` declare a sidebar tab name. Riina's
manifest loader previously discarded that field, blocking discovery of plugins
that contribute sidebar content. The shared manifest now retains a bounded,
validated label, chooses localized labels using the system locale, and ignores
absent or non-string labels. The installer, enabled-plugin inventory and runtime
loader all use that locale. A portable sidebar state machine now validates
package-relative documents, gates visibility and messages on document loading,
and resets listeners when a document changes. The persistent QuickJS player
session now exposes `iina.sidebar.loadFile`, `show`, `hide`, `postMessage`, and
`onMessage`; its typed effects pass through the bounded plugin event queue, and
registered callbacks receive host messages on the plugin worker. Focused tests
cover confined paths, visibility ordering, effect emission, and callback delivery.
Windows now builds a selector from enabled plugins' localized labels, opens the
selected plugin's package page in a child WebView, serves package-relative files
through a confined custom scheme, and returns JSON messages to the plugin worker.
The player sends `iina.window-loaded` after its UI event loop starts so plugins
can initialize the view at the expected lifecycle point. The plugin host now
remembers this transition and sends it to player plugins enabled or reloaded
after the window has loaded; a focused host test exercises that late-activation
path and idempotent startup notification. A fresh Windows debug-build smoke test
showed an enabled plugin tab and its package page in the child WebView. The
confined child-WebView adapter now also builds on the macOS target path; an
Apple-target build and live Mac run are still required. Linux player sidebars now
use the same presenter on X11, with GTK initialized on the player thread and its
event queue pumped by the UI timer. Wayland child embedding remains unsupported
by this adapter. The Windows-host Linux cross-check is blocked before Riina
compilation because no Linux pkg-config sysroot is configured. A native Linux
build/runtime and matched IINA sidebar geometry remain open.

## Quick Settings Video pane live check · 2026-10-14

A fresh Windows debug build and isolated profile opened the Quick Settings pane
from its toolbar gear while `track-languages.mkv` was paused. The Video pane was
selected by default. Its selector displayed the active video track before the
aspect-ratio, crop, rotation, speed, hardware-decoding, and picture controls; the
tab sequence was Layout, Video, Audio, Subtitles. This confirms the Windows
interaction path and default pane choice. Track switching, other tab behavior,
matched IINA pixels, and macOS/Linux runtime remain open. A separate attempt to
verify the `v` subtitle-visibility shortcut was inconclusive after the live
window changed size and the computer-use coordinates no longer matched.

Aspect-ratio and crop preset strips now reserve IINA's trailing scroll indicator
space and show a directional chevron when the choices overflow. Activating it
moves to the opposite edge with IINA's 300 ms animation; the chevron hides while
the rail is between its ends and returns with the opposite direction at the
destination. The animation follows Riina's Disable Animations preference.
Chevron commands leave the scroll offset owned by the scroll view so direct
wheel/trackpad scrolling remains independent. Keyboard navigation between
segments remains available. Workspace compilation passes; live chevron timing
still needs a Windows interaction capture.

The Audio, Video, and Subtitle track tables now follow IINA's plain-table
selection treatment: the chosen row keeps its narrow accent marker without a full
row fill, and rows have subtle horizontal grid lines. Active row sizing and
language badges remain unchanged. Track, None, and language-badge colors now follow
the active Light/Dark appearance. Native selection behavior is covered elsewhere;
matched pixels and live track scrolling still need GUI verification.

## Plugin file-started lifecycle event · 2026-10-14

Player plugins now receive `iina.file-started` when Riina begins a new media generation, followed by `iina.file-loaded` once media metadata is ready. The new event is emitted on the generation transition even if the UI coalesces the short Loading snapshot. It uses IINA's event spelling and player-session scope. Focused context/runtime tests and the complete workspace test suite pass (282 app tests; 50 native libmpv tests remain ignored by default). The broader IINA event catalogue, global controller events, and live plugin interaction remain open.
## Plugin window lifecycle events · 2026-10-14

The plugin host now diffs observed player-window properties and sends IINA-compatible player-scoped events for movement, resize, fullscreen changes, Picture in Picture and music-mode changes, minimization/restoration, and screen changes. Initial geometry is treated as a baseline so opening the player does not produce false movement events. Geometry and mode-diff tests and a QuickJS listener-delivery test pass. Live window/plugin interaction, exact event cadence, and macOS runtime verification remain open.
## Plugin pre-close lifecycle notification · 2026-10-14

Player-window shutdown now sends subscribed plugins `iina.window-will-close` before stopping playback or tearing down the plugin host. The host waits for each bounded QuickJS callback barrier and drains its resulting messages before plugin sessions are destroyed. This covers shared-player and Windows native-video close paths through one guarded player callback. The focused host lifecycle test and full workspace checks pass; live close behavior remains unverified.
## Plugin engine-initialized replay · 2026-10-14

Riina now dispatches `iina.mpv-initialized` when the observed libmpv version first enters the plugin player context. Plugins enabled after engine initialization receive the event during activation, alongside the late `window-loaded` replay. This follows IINA's `mpvInit()` event and AppDelegate reload behavior. A host integration test verifies late-event delivery exactly once, including delivery before the pre-close lifecycle callback. All 283 workspace app tests, strict Clippy, workspace build, formatting, parity traceability and diff checks pass. macOS/Linux runtime behavior and the broader IINA event catalog remain open.
## Plugin main-window focus event · 2026-10-14

IINA's `iina.window-main.changed` event now follows the focused native player window. Riina samples Winit focus alongside its existing window snapshot; the initial state is false so the first focus gain is delivered. Context and QuickJS listener tests pass. Workspace tests (283 app tests), strict Clippy, workspace build, formatting, diff and parity checks pass. Live focus switching and rapid focus changes that occur between snapshots remain unverified; the wider event catalogue remains open.
## Plugin automatic window-size event · 2026-10-14

Riina now queues `iina.window-size-adjusted` when a media geometry update reaches the configured window-size policy. It reports the sampled player frame and still fires when the policy retains the current frame, matching IINA's event distinction from an ordinary user-driven resize. The frame-retained regression assertion and full workspace suite pass (283 app tests; 50 native tests ignored by default), as does the final workspace build. Strict Clippy, workspace build, formatting, diff, and parity checks pass. GUI plugin behavior and macOS runtime remain unverified.

## Plugin overlay-loaded event · 2026-10-14

Riina now reports `iina.plugin-overlay-loaded` to player-plugin sessions when a confined plugin sidebar document finishes loading. This follows the pinned source's sidebar and overlay load callbacks. The late-plugin host test passes with exactly the event delivery path covered; Windows and macOS WebView runtime behavior remains unverified.

### Plugin event gate results · 2026-10-14

The full workspace suite passes (283 app tests; 50 provisioned-native tests ignored), strict Clippy, workspace build, formatting, diff check, and parity traceability. The ignored libmpv native smoke test also passes with the bundled Windows engine: it decodes the local moving-square fixture and observes `container-fps`. This proves the development DLL works; it does not diagnose a different installed or already-running instance.

## Plugin thumbnails-ready event · 2026-10-14

The timeline preview controller now marks thumbnails ready only after a successful complete sample set, including a complete cache hit; partial sets and failed/cancelled tasks do not notify plugins. Completion is routed to player-plugin sessions as `iina.thumbnails-ready`. The full workspace test suite passes (284 app tests; 50 native tests ignored by default), as do the workspace build, strict Clippy, formatting, diff, and parity checks. GUI playback and plugin WebView delivery remain unverified.

## Plugin manager menu entry · 2026-10-14

The Plugin menu now includes “Manage Plugins…” on both the shared player and
Windows native-video surfaces. It opens Settings directly to the existing Plugins
page; normal Settings opening preserves its current page. The IINA menu audit now
records this implemented command. “Reload All Plugins” now shuts down current
plugin sessions, removes their sidebar/overlay content, and reactivates enabled
packages with their window/engine initialization events replayed. A host integration
test proves the plugin entry executes again. Menu actions contributed by both the
global and player entries are now shown and return to their owning runtime.
Per-plugin Developer Tool actions and matched visual verification remain open. See
`PLUGIN-MENU-001` and `REF-40`.

## Plugin-contributed menu actions · 2026-10-14

Player plugins can now create, remove, update, and force-refresh menu items using
the IINA-shaped `iina.menu` API. Bounded recursive snapshots carry titles,
enabled/selected state and key bindings through the host to the shared and
Windows-native Plugin menus; activating an item calls its retained JavaScript
callback on the owning plugin worker. Nested items remain flattened as breadcrumb
rows; plugin separators use the Slint 1.18.1 dynamic-menu separator sentinel.
Each plugin now has one display-name heading, with global actions before player
actions. The Plugins settings list now supports row drag-and-drop and persists
plugin identifiers in `plugin-order.json`; plugin startup, reload, and the live
player menu use that order. Reordering updates the invoking player's menu; order
changes are broadcast to all live player windows, pruning closed windows through
weak registrations. Global actions still route to their owning runtime within one
player host; sharing the global runtime across multiple player windows remains
open. Nested submenu hierarchy, Developer Tool actions, live drag/drop interaction
and matched visual comparison remain open. Windows native Plugin opening sends
`iina.menu-update` to subscribed player sessions; shared Slint and macOS menu-open
event delivery remain unverified.

## About version labels · 2026-10-14

The About window says “Checking…” while the stopped player awaits its first
engine-version snapshot, and “Reading…” after playback begins if metadata
has not arrived yet. It then displays the observed libmpv and FFmpeg versions.
These are metadata states, not an indication that the playback engine is
unavailable. If About opens before the properties arrive, a visibility-gated
timer refreshes the labels while the window remains open.
A bundled-libmpv integration check verifies both version properties reach the
stopped player snapshot before any media opens, then remain available during
playback. This separates version-reporting delays from library load failures.

## Plugin submenu action routing · 2026-10-14

Flattened plugin-menu breadcrumb rows for items with children are now disabled,
matching IINA's behavior where submenu containers navigate and only leaf items
invoke callbacks. Nested leaf rows retain their plugin/scope/item tokens. A player
UI regression test covers this routing rule; visual submenu hierarchy remains open.

## Generic mpv plugin event delivery · 2026-10-14

Plugin `event.on("mpv.<event>", callback)` subscriptions now receive generic
libmpv event notifications in addition to `mpv.<property>.changed`. The player
worker copies each event name before the next native wait call and keeps a bounded
256-event sequence in playback state; the plugin host emits every retained event
after its last delivered sequence, so coalesced UI snapshots do not drop bursts.
The native libmpv integration test covers start-file and file-loaded delivery,
and a bridge unit test covers ordered exactly-once dispatch. Full plugin GUI use,
event behavior under more than 256 undrained events, and macOS/Linux runtime
remain unverified.

## A–B timeline marker colors · 2026-10-14

The A and B loop markers now use the pinned IINA `MainSliderLoopKnob` asset's
appearance values: `#696969` in light appearance and `#a6a6a6` in dark appearance.
The marker size, 75% height, and rounded shape continue to follow
`PlaySliderLoopKnob`. Source-derived colors are wired, but live marker rendering,
native drag interaction, and matched Windows/macOS captures remain unverified.

## Plugin sidebar placement · 2026-10-14

Quick Settings now includes the IINA-compatible Plugins leading-side preference.
It defaults to the leading side, persists with the other layout settings, and
relocates an already open plugin panel when changed. The toolbar/menu toggle now
opens it on the saved side, and sidebar-resize persistence retains this preference.
The focused app test verifies both saved-side choices. Playback behavior and the
matched panel presentation still need GUI verification.

The Layout pane now groups Theme/material options, Compact Interface, and Video
settings into separate rounded cards, matching the reference's hierarchy. Sidebar
placement rows now show icon-led Leading/Trailing choices for Settings, Playlist
and Chapters, and Plugins. Slint compiles, but the current debug build has not yet
been captured; card proportions, translucency, tab geometry, and native-material
rendering remain open for matched visual review. The selected Quick Settings tab
now pairs an icon with its label while the other settings tabs use compact icons,
matching the captured IINA tab strip at the source-layout level. Sidebar-side
choices expose their leading-side meaning to accessibility and support arrow-key
selection.

## Video-backed translucent surfaces · 2026-10-14

Shared-OpenGL playback now renders a quarter-resolution, sampled blur of the
current video frame and composites it beneath the enabled translucent sidebar,
title-row, floating-controller, seek/video/plugin OSD, and fullscreen-information
tints. It only renders while at least one enabled material surface is visible. The
pass preserves framebuffer, texture, viewport, and relevant GL enable state, and
its objects are destroyed while the player context is current. It is optional:
shader/setup/render failure does not stop playback. Workspace compilation,
strict lint, and tests pass (297 application tests; 51 native tests ignored by
default). Native Windows D3D composition, GPU cost, side-by-side visual
comparison, and macOS runtime/material appearance still need live verification;
Windows D3D does not use this GL pass.

## Help menu destinations · 2026-10-14

Riina Help opens the Status section of the project README; Release Highlights
opens the latest release; GitHub opens the source repository; Website opens the
project README.
The Help menu marks “Use New Settings Window” checked because the new settings
window is the only settings interface currently implemented; selecting it opens
Settings without changing the active mode.
The pinned XIB's “Dump Debug Info” action is now wired on the shared player and
Windows-native Help menus. It writes a bounded current-player snapshot to Downloads
and warns that playlist filenames and URLs may be sensitive. The verified capture
omits this source-defined item, so matched menu captures must resolve that visual
difference. Native Help-menu Search remains macOS-specific.

### Headless sidebar placement check

Plugin-panel placement now has a pure decision helper for leading/trailing side
selection, so its unit test does not create a renderer-backed player window.
The player still applies that placement to the same leading/trailing sidebar
properties; this keeps the Windows CI test independent of a desktop OpenGL
driver.

### PiP projection sizing

The Windows DWM PiP projection now re-queries the source video dimensions and
recomputes its aspect-fit rectangle while the PiP surface is active. Resizing the
PiP window and autoplaying a clip with a different aspect ratio update the projected
video without re-entering PiP. Focused geometry tests, the full workspace suite,
formatting, and strict application Clippy pass. Live DWM rendering and interaction
remain unverified because the current computer-control surface exposes no Windows
application for GUI testing.

### Native Open Recent destinations

The shared Recent list now also updates the platform-native recent destinations.
Windows sets a stable AppUserModelID at startup, records existing local media with
the Shell, and clears only this app's Recent and Frequent Jump List categories.
macOS records and clears through NSDocumentController. Manual file selection,
successful playback, and opening an existing Recent item follow their separate
preference gates and refresh recency. All 305 workspace app tests, strict workspace
Clippy, workspace build, formatting, and parity traceability pass. Windows taskbar
behavior, macOS menu activation/runtime, and Linux desktop recent
file integration remain open.
The Apple-target source check stops in the `ring` build because this Windows host
does not provide the target C compiler (`cc`); no macOS source-check result is
claimed.

## Quick Settings video track selection · 2026-10-14

Clicking a video-track row in Quick Settings now selects it immediately, matching
IINA's track selector and Riina's existing accessibility action. The shared row
handler previously selected video tracks only on double-click, even though Audio
and Subtitle rows selected on a single click. A live multi-track switch still needs
verification; the current desktop capture bridge failed during the playback-window
capture, so no click-through result is claimed.

## Secondary subtitle track rows · 2026-10-14

The Secondary subtitle selector now matches the Primary selector: selected rows use
the narrow IINA accent marker without a full-row fill, with horizontal separators
and appearance-aware track/language colors. The reference shows both lanes with the
same table treatment. Workspace tests and strict Clippy pass; matched live captures
remain unavailable through the current Windows capture bridge.

## Audio Quick Settings card hierarchy · 2026-10-14

The Audio pane now frames track selection, external-audio loading, delay, and the
equalizer in separate rounded cards, following the captured IINA Audio pane. The
track selector retains its fixed 98-point viewport and single-click selection; the
delay and equalizer controls keep their existing ranges and actions. The opt-in
volume-boost row now appears only while enabled, keeping the default panel's card
order aligned with IINA. Slint compilation and workspace tests pass; matched live
pixels and audio interaction remain unverified.

The Audio equalizer now shows IINA's `+12 dB`, `0 dB`, and `-12 dB` scale beside
the ten vertical gain sliders; slider columns were narrowed to make room for the
scale in the same card. Workspace compilation and strict Clippy, formatting, and
parity traceability pass; the full workspace test suite passed before the final
coordinate alignment. Matched spacing, typography, and rendered pixels still need
a live capture.

## macOS native media titlebar identity · 2026-10-14

Player updates the macOS window's represented URL for local media, allowing AppKit
to supply the file document icon and titlebar path menu. The macOS title is the file
display name; network media clears the represented URL and uses the stream title.
Windows keeps its existing `filename — riina` title. The exact AppKit titlebar helper
cross-compiles against the aarch64 Apple target and Slint Winit API in a minimal
probe. Current Windows `cargo check --workspace --locked`, strict all-target Clippy,
and workspace tests pass (312 app tests; 51 provisioned-native tests were ignored).
The full Apple-target application build is still blocked in `ring`'s C build script
because this host has no Darwin compiler. Native titlebar rendering, document icon,
path-menu interaction and stream titles still need an M2 Mac capture.

## macOS AppKit module compile sweep · 2026-10-14

A focused aarch64-apple-darwin probe now checks all 11 source modules in
`platform/macos/` with strict Clippy, including the AppKit application menu, system
font, display ICC, PiP, plugin-menu, scroll, system-media and represented-titlebar
surfaces. The sweep caught an unsafe AppKit image-name symbol use, deprecated ICC
API usage, and two strict-Clippy issues in Now Playing artwork handling; all four
were corrected. This validates the native modules against Apple-target APIs but
does not replace the full application build or M2 GUI/runtime verification. The
full application target still stops in `ring` before Riina application code because
this host lacks a Darwin C compiler.

## macOS Quick Settings sidebar tint calibration · 2026-10-14

Measured the pinned Quick Settings reference against its green test pattern: a
blank region outside the dark sidebar is `#65C466`, while the blank sidebar sample
is `#459546`. With the existing 32% video-blur layer, the former 69%-opaque macOS
sidebar base predicts a visibly darker composite. The macOS dark Liquid Glass
fallback now uses `#1515156B` (42% alpha), which predicts approximately
`#4E924F` under that solid-color sample. Windows retains its own tint. This is a
capture-specific fallback calibration; AppKit Glass rendering and matched Mac
runtime pixels remain unverified.

## Windows native host Mica transparency · 2026-10-14

The persistent native player host now creates an alpha-capable Winit client and
keeps its original black Slint surface until DWM accepts and reads back
`DWMSBT_MAINWINDOW`. The readback confirms the requested type, not the actual
pixels rendered under transparency, power, or hardware policy. After that
confirmation, the host client becomes transparent; Windows may show Mica or its
opaque system fallback. If DWM rejects the request, the host keeps its solid
fallback. Strict workspace Clippy, formatting, diff validation and the
256-row parity traceability check pass. The desktop capture surface currently
reports no available Windows apps, so visible Mica pixels and playback-window
composition remain unverified. Transient Acrylic and material on the separate
transparent controls overlay remain open.

The Windows Welcome window now requests the same persistent Mica backdrop as the
main host and reveals it through an 85%-opaque dark tint only after the Winit window
is realized and DWM accepts the backdrop type. Rejection keeps the original opaque
dark surface. Source compilation passes; the current desktop exposes no app windows,
so its visible material is not yet verified.

The transient Windows About window now requests `DWMSBT_TRANSIENTWINDOW` and
reveals Desktop Acrylic through 85%-opaque light/dark content panes after DWM
accepts the backdrop type. A rejected request preserves its existing opaque theme.
Welcome and About now declare transparent native window backgrounds from creation,
while full-size content rectangles keep their original opaque fallback until DWM
accepts the material. The regular Windows Winit backend also creates alpha-capable
surfaces for windows without native menus; Slint retains opaque native-menu windows.
This makes transparency available before the asynchronous DWM response. The
Windows app build, strict Clippy, formatting and diff checks pass; visible Acrylic
pixels and titlebar/client composition still require a Windows capture.

The long-lived Preferences window now requests the same DWM Mica backdrop as the
player host. Its window background is transparent only after DWM accepts the
request; the existing sidebar and page surfaces remain opaque, preserving the
current layout while allowing Mica to show through the uncovered page gaps. The
Windows app build, strict Clippy, formatting, diff checks and parity traceability
pass. A live material capture remains unavailable.

## Windows material policy and Settings layering · 2026-10-14

Windows Mica and Acrylic follow separate policies: Transparency effects and High
Contrast gate both, while Battery Saver gates Acrylic and sampled-video blur but
not Mica. Policy reads fail closed to solid surfaces, and the player host plus
registered Mica/Acrylic windows refresh their backdrop at 500 ms intervals. The
Windows policy monitor also covers the shared-GL player path: disabling the
relevant Acrylic policy removes video blur and restores solid surfaces there too;
enabling it restores WinUI Acrylic card and control styling. The Preferences page,
navigation backgrounds, and card
containers use the light 50% WinUI Mica content-layer fill while DWM accepts Mica;
input fields and selected rows stay solid. The no-material fallback keeps the
original opaque colors. A live
window-attribute probe opened Settings with Ctrl+, and read DWM backdrop type 2
(`DWMSBT_MAINWINDOW`) for Settings and the player host. A second probe opened
About from the app menu and read type 3 (`DWMSBT_TRANSIENTWINDOW`). Both probes
shut down their exact test process and left no Riina instance running. Build, strict Clippy and
workspace tests pass (318 app tests; 51 native libmpv tests remain ignored
without the DLL). DWM attribute readback is not pixel evidence: desktop capture
still returns a black frame, so rendered Mica/Acrylic, live policy transitions
and contrast/readability remain to verify.

## macOS subtitle color panel source path · 2026-10-14

Subtitle color wells now call AppKit's shared alpha-enabled `NSColorPanel` on
macOS, convert live changes through sRGB, and route them through Preferences'
existing save/apply path. Other platforms keep the in-app RGBA editor. The new
native module passes the isolated aarch64-apple-darwin strict-Clippy probe; the
full app target and native Mac UI remain unverified. Picker-mode behavior,
window ownership, save/cancel flow and rendered subtitle output need a live Mac
comparison.

## Windows native-video translucent materials · 2026-10-14

When a translucent controller, sidebar or OSD is visible over native D3D11
playback, the player now requests a raw RGBA mpv frame no more often than every
500 ms. The player worker bilinearly downsamples each owned frame to at most
320×180 before retaining it in playback state. The UI then blurs and lightly
grains the SDR sample for its material layer; paused playback samples once per
position, and HDR retains the existing tint. This avoids a second visible
renderer and avoids putting a desktop backdrop over the video. Formatting,
workspace build, strict Clippy and
workspace tests pass. Live Windows pixel/performance comparison remains pending;
the desktop control surface did not expose Windows applications during that run.

## Windows video-backed Acrylic visibility adjustment · 2026-10-14

Native-video Slint material layers now expose more of the sampled video: 58%
sample opacity and an 8 px Gaussian blur at the 320×180 sample size, compared with
the 32% shared-renderer/macOS layer. The Windows sidebar dark tint uses 60% alpha,
the light tint 78%, and the surface has rounded
12 px corners with a subtle one-pixel border. macOS's measured Liquid Glass tint
and sample strength remain unchanged. The player built and completed a brief
moving-square playback without a material-capture error. Computer Use now lists
the player and returns its accessibility tree, but `get_window_state` fails at
`CreateForMonitor` (`0x80070057`), and GDI returns black pixels for the GPU
surface. These checks do not verify material appearance or performance.

The Windows Video Filters, Playback History, Log Viewer, Online Subtitles, and
Font Picker windows now request DWM transient Acrylic. Their root tint becomes
translucent only after DWM accepts the request, while inner rows and panels keep
their existing opaque colors. DWM rejection preserves each prior solid window
fill. Rendering and readability still need live Windows comparison.

## Windows video-backed material reference calibration · 2026-10-14

Calibrated the native Windows Quick Settings sidebar sampled-frame opacity at 53%, while other native-video surfaces retain 58%. The pinned
IINA Quick Settings capture records `#65C466` in the video and `#459546` in a
dark sidebar region. With the existing 60%-opaque `#151515` Windows tint,
53% sidebar sample opacity predicts `#4E934F` at that point. This brings the Windows
in-app Acrylic approximation close to the measured reference while leaving the
macOS and shared-renderer material strengths unchanged. Windows live pixels and
other video content still need capture-based verification.

A whole-desktop capture of the Audio panel now confirms that the sampled-video
material and controls compose over the native mpv child. A sequence with a unique
copy of the 4 fps reference GIF showed the panel color out of phase with the video
at 10 Hz. A 20 Hz experiment did not remove the mismatch and coincided with one
10 fps / 18% CPU desktop performance-overlay reading; that reading is not a
controlled benchmark. The higher cadence was reverted and the current 10 Hz cap
remains until sampling can be tied to presented frames without repeated full-frame
readbacks. REF-23 stays open for temporal, geometry and cost comparison.

The Audio pane spacing now accounts for both levels of Slint parent layout spacing.
The earlier 10-point spacer was not the complete gap: the surrounding layout adds
8 points above and below in spacious mode, producing 26 points total. The Audio
cards now use 2/4-point spacers to reach IINA's 18/16-point spacious/compact stack
spacing exactly. The cards now center at IINA's 16/14-point side margins while
the tab header keeps its separate 10-point inset, and their corners use IINA's
8-point radius. The workspace build succeeded with all three geometry edits. A
follow-up GUI capture did not bring a player window to the foreground, so updated
dimensions remain source-derived until a clean Audio-pane capture confirms them
against REF-23.

The nested Online Media page in Preferences now also exposes the active Mica
content layer instead of painting a full opaque white rectangle over it.

The Windows Welcome window now uses WinUI's dark Mica layer fill (`#4C3A3A3A`
ARGB) rather than an 85%-opaque dark panel, allowing the accepted system backdrop
to show through while keeping its existing opaque fallback.

Quick Settings cards now use WinUI's dark/light Acrylic layer fills
(`#09FFFFFF` dark / `#40FFFFFF` light) only when the Windows sampled-video sidebar material is
active. This preserves the existing macOS and opaque Windows card treatments and
reduces the previous over-opaque light-mode card tint.

The `Fit Window to Video` action now uses WinUI control-layer fills while that
sampled-video material is active: dark mode uses default/secondary fills, and
light mode uses tertiary/secondary neutral fills with dark foregrounds. Shared
macOS and opaque fallback styling remains unchanged. The locked workspace build
passes, but the new action colors still need a fresh rendered Windows capture.

The native Windows controls popup uses DWM per-pixel alpha composition. Its
realized HWND clears incompatible layered and mouse-transparent styles before
reapplying DWM's alpha contract; video-backed sampling remains the overlay's
blur mechanism.

The About window now removes its app-tinted content panes only while DWM confirms
Desktop Acrylic, letting the native material show through its content area. A
rejected request still uses the original solid panes. Live Acrylic pixels and
contrast remain to verify.

Other Acrylic-enabled utility windows now expose a transparent root after DWM
acceptance. Their toolbar, table-header, backing panels and unselected list rows
use WinUI Acrylic layer fills; data-entry fields and selected rows retain solid
contrast fills. Live material pixels and text contrast remain to verify.

The Windows Font Picker follows the resolved player light/dark theme while visible.
Its native frame and DWM Acrylic layer, list text and selection, headings, and
preview switch together; Winit maps the native theme to AppKit appearance on macOS.
The preview remains locally tinted and selection remains opaque. Live contrast still
requires a Windows capture.

The Windows PiP window now requests transient Acrylic on first show. Its DWM video
thumbnail keeps the black viewport behind it, while the lower controls strip uses
a dark translucent layer over the system material; DWM policy monitoring switches
the strip back to its solid fill when effects are disabled. Live video/control
composition remains to verify.

Preferences card containers now use the established 50% WinUI light Mica content
layer when DWM accepts the backdrop; input fields and selected rows remain solid.
Disabling system materials restores the prior gray card fills. Live Mica contrast
remains to verify.

The Windows auxiliary-window material monitor now retries when its current root
transparency state disagrees with the system policy, even if the policy itself has
not changed since an unsuccessful DWM request. This keeps an initially rejected
transient request from remaining solid indefinitely if DWM later becomes available.
Retries back off to 30 seconds and repeated identical errors are logged once until
recovery or a policy change.

The persistent player host now tracks confirmed DWM Mica state separately from the
current Windows policy and visible client fill. Rejected operations retry with the
same bounded backoff; disabling effects keeps the client opaque until DWM confirms
the cleared backdrop, and immediately hides client alpha if the Winit handle is
temporarily unavailable.

The About window now follows live player appearance changes while visible, keeping
its Slint palette and Windows native frame/Acrylic theme synchronized without
reopening the window.

Quick Settings sidebar headers now use IINA's circular directional dismiss control:
the leading sidebar places a backward chevron after its tabs, and the trailing
sidebar places a forward chevron before them. The tabs and button distribute across
the available header width, as in IINA. The control uses a WinUI layer fill and edge
while Windows video-backed Acrylic is active. This source-derived update
is not a live visual verification; desktop capture remains unavailable.

REF-21's screenshot pixels also suggest 150/42/74-point heights for the Theme,
Compact Interface, and two-row video-layout cards, with 20-point vertical gaps.
The scale is inferred from the reference sidebar width; the Quick Settings Layout
cards now use these dimensions while retaining the optional third action row.
Matched Windows pixel verification remains open.

Windows material opacity now stays consistent across native-video and shared-GL
renderers. Both use the calibrated 53% sampled layer for Quick Settings and 58%
for other video-backed surfaces, with matching translucent light-theme base and
OSD fills. The shared-GL Windows blur kernel now matches the native renderer's
sigma-8 blur at the 320×180 sample scale with aspect-aware horizontal and
vertical tap spacing; macOS keeps its prior blur spacing and 32% sample opacity.
Windows renderers continue to honor
Transparency Effects, Battery Saver and High Contrast; actual matched-scale
Windows material pixels and text contrast remain to be verified.

Windows selected track markers, Quick Settings presets, enabled Layout switches,
sidebar-side selectors and controller-placement choices now read the user's system
accent via `UISettings.GetColorValue(UIColorType::Accent)` in both shared-GL and
native-video modes. It refreshes while playback is open and falls back to Windows
blue (`#0078d4`) if the system color cannot be read. This avoids the shared
renderer using macOS blue for selected states. Selected labels and side-selector
glyphs choose black or white according to the accent's relative luminance rather
than assuming white will contrast with every user accent. Playback progress,
playlist selection, and the Volume Boost enabled state follow the same accent on
Windows; macOS colors are unchanged. Focus rings and selected playback tools,
sidebar tabs, delay editors, track rows, and Quick Settings focus now follow the
Windows accent as well. Quick Settings category tabs use a neutral themed glass
capsule on Windows and macOS; Dark/Light/Auto and actual presets keep their
platform-accent selection, with contrast-aware foregrounds. Appearance still
needs matched-scale Windows capture review, especially with light themes and
unusual user-selected accent colors.

## Windows material policy and REF-21 card geometry · 2026-10-14

Mica and Acrylic now use separate Windows policy checks. Battery Saver disables
Desktop Acrylic and sampled-video Acrylic, while Mica remains available; both
materials fall back when Transparency effects are off or High Contrast is active.
Persistent hosts monitor Mica policy and transient surfaces monitor Acrylic policy.
The pinned REF-21 screenshot measures the Layout cards at 328 points in the
default 360-point sidebar with 16-point insets. Riina now centers the cards at that
width. Per-window GDI and `PrintWindow` still do not reliably include the native
video/controls composite; the DPI-aware full-desktop capture helper does. Policy
transitions and matched-scale rendering still need live GUI verification.

REF-21 and REF-24 show neutral glass capsules for selected Quick Settings category
tabs; blue is reserved for appearance choices and selected presets. Riina now uses
the neutral themed material on Windows category tabs too, while retaining the
Windows accent for actual selected values. A post-build full-desktop 2026-10-14
capture confirms the neutral Video capsule and shows the video sample through the
Windows Acrylic panel. The capture is smaller than REF-24's fullscreen player;
matched panel bounds, lower controls and exact material strength remain open.

An additional Windows playback launch on 2026-10-14 measured the host window at
928x624 on a 1280x720 display, with the 915x570 host client, transparent controls
window, and mpv child surface aligned at the same client origin. This launch did
not show a host/child geometry offset. GDI desktop capture rendered the native
video region black, while `PrintWindow` captures of the layered/GPU surfaces did
not provide a trustworthy composite; treat them as capture limitations, not proof
of a black playback surface or visual parity. The test instance was closed and no
Riina process remained. Matched Windows video/material pixels remain unverified.

The native controls overlay now derives its bounds directly from the host HWND's
Win32 client rectangle and client-to-screen origin, avoiding stale or virtualized
Winit geometry during movement and media-size changes. A rebuilt 300%-DPI session
measured the overlay and visible mpv child at the host client origin and size. A
full-display capture also showed that a visibly launched debug build's separate
console can cover the player; hide the debug console when capturing or exercising
the player. A stable video-frame capture after this alignment change remains open.

When native alignment is still settling, the error path no longer writes Winit's
possibly stale outer-window position back to the controls popup. The 16 ms native
alignment retry remains authoritative. The rebuilt player showed the video in a
full-desktop capture and the overlay's measured bounds matched the host client
after a move; a transient old-position tint appeared in the immediate capture and
was gone one second later. Because playback had advanced to an audio-only item by
then, a persistent video-cover artifact has not been reproduced or ruled out.

The Windows computer-use capture can now target the live native Riina window. A
918×619 host capture and a separate 916×570 transparent-controls capture were
returned for the 5-second moving-square clip. At an observed 0.125 seconds, the
host image still omitted the expected video pixels; it does not composite the
separate layered controls and mpv GPU child into a whole-desktop screenshot. The
black viewport in that capture therefore does not establish that playback is
black or confined to the controls. Use a real desktop-composited capture for
video and material pixel verification. The test instance was closed.

The Windows player now gives Winit the saved explicit Light or Dark theme before
creating the native menu, while System continues to inherit the OS theme. This
keeps the title bar and menu strip aligned on first paint; the live dark-theme
capture shows both dark. The Playback dropdown still uses the OS light popup
surface on this machine, whose Windows app theme is Light. That popup appearance
remains an open platform-theme difference. The test instance was closed.

The player chrome had been forcing `WS_EX_LAYERED` after Slint initialization,
without providing the layered-window update surface that Windows requires. The
overlay now returns to DWM's per-pixel alpha composition and reapplies the DWM
alpha contract after realization. Windows' legacy blur-behind API is used only
for its documented alpha handling; sampled video remains the panel blur source.
A strict build passes. The new live video capture shows the test pattern through
the controls layer and through the tinted Quick Settings material. Moving the
player by 300×150 screen pixels moved the controls surface by the same amount.
Matched reference pixels, system-policy transitions, and multi-display behavior
remain open.

The 2026-10-15 3840×2160 full-desktop capture showed video in the main playback
area and exposed a moved-host controls popup left at its former position. A
DPI-aware move measured host-client origin (686, 305) and stale popup origin
(546, 215). Direct `SetWindowPos` corrected the popup, where the prior deferred
batch did not. The 16 ms alignment retry now moves that popup directly; the
rebuilt app placed it at the exact client origin and size 2747×1711 after the
same move. The full-desktop capture shows the controls over the playback image
with the stale gray strip gone. The test instance was closed and no Riina process
remains. Multi-display and repeated playlist-transition alignment remain open.

REF-20's no-audio GIF also exposed an OSC mismatch: IINA hides mute and volume
when no primary audio track is selected. Riina now follows that rule while
preserving the centered transport; live captures hide the controls for the GIF
and retain them for selected stereo audio. The previous capture measured the rendered Windows controller at about
460×66 points because its layout omitted the source slider's vertical padding.
Source review shows IINA's 67-point value is only for position clamping; its
visible stack is about 78 points spacious and 71 compact. Riina now uses those
heights and a 24-point timeline row. The previous full-desktop captures are
indexed in `tests/visual/windows-captures.json`; a rebuilt capture measures
458×78 points with a 58-point gap in the 619-point client. Windows Graphics
Capture follows the 4 fps pattern beneath the translucent controller. A dark
flat-green comparison measured 72.0% relative luminance, against IINA's 71.8%,
with visible width within about 3 points. Nonuniform-motion blur, exact
matched-client vertical bounds, light appearance, and text contrast remain open.

The 2026-10-15 3840×2160 whole-desktop capture of Quick Settings/Video shows the
test clip filling the playback viewport and its blurred gold square through the
dark translucent sidebar. This verifies the full desktop composition and video-
backed material visibility; the clip had reached frame 119, so live material
refresh during motion and matched opacity remain open. The screenshot is indexed
in `tests/visual/windows-captures.json`; its raw image stays in the ignored local
reference-capture folder. At 300% display scale, the approximately 1,080-pixel
pane is 360 logical points wide, matching REF-21's default sidebar width. The
player window is smaller than IINA's fullscreen capture, so whole-screen ratios
are not a valid geometry comparison.

The native `DWMSBT_TRANSIENTWINDOW` backdrop was tested on the full-size controls
popup and on a compact owned Win32 popup between the video and controls. DWM
accepted the compact popup but rendered it as a solid gray block over playback,
so it cannot serve as the video-backed control material. Sampling requests
remain at 10 Hz; the UI now polls asynchronous replies every 16 ms. A fresh
isolated GIF run with twenty timed pixel probes
still showed the controller material about one frame behind the live image, so
the remaining delay is upstream of that UI poll. The capture is indexed in
`tests/visual/windows-captures.json`; a frame-synchronized or surface-scoped
material path is still needed. The pinned mpv documentation says
`screenshot-sw=no` renders through the active VO and may reinitialize its
renderer, making it a plausible readback-latency contributor. This is not yet a
proven cause; changing that global option could alter user screenshots and HDR.

## Windows Graphics Capture sampler and black-patch recheck · 2026-10-15

The native presentation now owns an optional window-scoped Windows Graphics
Capture session for its material sampler. It reads free-threaded D3D11 frames,
crops the client surface to a staging texture, and bounds samples to 320×180;
fresh samples take priority, with the existing mpv screenshot path retained as a
fallback. HDR stays on its tint path. The debug build, strict Clippy, and formatting
passed. A
full-desktop check using an isolated moving-square fixture showed the viewport's
black letterbox bands moving with the player and no black square fixed at the old
screen location. This visible check does not distinguish the active sampler from
the fallback, so WGC frame delivery and timing still require direct verification.
The capture is indexed in `tests/visual/windows-captures.json`. The exact GUI test
instance was closed; no Riina process remains.

## Quick Settings Layout source geometry · 2026-10-15

Compared the Layout pane with the pinned `LayoutValue.swift` and
`SidebarLayoutPane.swift`. The current spacious/compact stack spacing is 18/16
points, card side insets are 16/14 points, and internal padding, item spacing,
and mini-switch spacing follow the source values (12/10, 12/8, and 6/5 points).
Mini material switches use 36×16-point Windows tracks. Current card sizes are
152/138 points for Theme/material, 44 points for Compact Interface, and 76 points
for the two-row Video card. Rebuilt Windows full-desktop captures show both
spacious and compact states without clipping; their hashes are indexed in
`tests/visual/windows-captures.json`. Geometry is not pixel-scored. The settings
header now uses a 192-point capsule after measuring the reference at about 191-192 points in a native-resolution crop. A live capture of the updated header remains pending.

## Floating controller follows sidebar bounds · 2026-10-15

The pinned `MainWindowController.setupVideoContainerConstraints()` sizes the
video container between open sidebars (unless Edge-to-Edge Video is enabled),
and `OSCFloatingView` centers/clamps its controller within that container. Riina
was sizing and centering its floating, top, and bottom controller against the
full window even when Edge-to-Edge Video was disabled, allowing controls to
overlap a trailing Quick Settings pane on a narrow player. Controller width,
horizontal position, center snap, drag bounds, and stored normalized position now
follow the video viewport when that setting is disabled; edge-to-edge mode retains
full-window bounds. The 0.3.8 Windows build passed, and the full-desktop GUI capture confirms the
centered controller stays inside the video bounds with Edge-to-Edge Video off.
Drag persistence and macOS runtime verification remain open.

## Windows video-backed material motion capture · 2026-10-15

The full-desktop motion sequence at
`reference/captures/riina-live-2026-10-15/windows-material-motion-01.png`
through `windows-material-motion-05.png` shows Quick Settings/Video over active
24fps playback. Across frames 116, 12, 27, 43, and 58, the moving square crosses
the video viewport and its blurred gold sample changes beneath the translucent
sidebar. This qualitatively verifies the video-backed Acrylic approximation
responds while playback moves. Five captures spaced about 650ms apart cannot
measure frame-accurate lag or identify whether WGC or the mpv fallback supplied
each sample. The 360-point sidebar width still matches REF-21. File Loop was
enabled only to keep the short test clip moving; the test instance was closed and
no Riina process remains. Capture hashes and state are in
`tests/visual/windows-captures.json`.

## Windows native player menu appearance · 2026-10-15

Changing the app's Layout theme already restyled the native title bar and client,
but the Muda menu bar retained its launch-time color scheme. The native-video
theme synchronizer now updates the attached Muda menu when the app changes between
Light and Dark, using the pinned Muda 0.19.3 message path. It applies the caption
theme after Muda's non-client redraw and tracks the app theme directly because
Winit's reported theme remains the cached system scheme for explicit overrides.
A rebuilt 0.3.8 app was captured across Dark → Light → Dark at 3840×2160 and 300%
desktop scale; both caption and menu strip followed each change without recreating
the window. The File dropdown remains light when the app is
dark and Windows' app theme is light: Muda documents its theme setting as applying
to the menu bar, not popup menus. Captures and hashes are in
`tests/visual/windows-captures.json`. The exact popup-theme adaptation and matched
IINA menu geometry remain open.

## Reported stationary black playback patch · 2026-10-15

The user reports a black square covering playback that remains fixed when the
player moves. A fresh Riina 0.3.8 GUI check used the isolated moving-square fixture
and full-desktop 3840×2160 captures before and after moving the native window about
100 logical points to the right. The video, aspect-fit black bands and controls
followed the player; the former screen area showed the browser, with no stationary
black patch. Both captures and hashes are recorded in
`tests/visual/windows-captures.json`. This does not rule out the user's media,
playlist handoffs, or another display/DPI state. If it recurs, capture the visible
player and its host, mpv child and controls-surface bounds together before changing
alignment behavior.

Follow-up: the user now reports that a black patch can stay at its desktop
coordinates while the player moves. Source review found that the incoming-frame
stability fallback could enable the opaque Slint cover when the outgoing image had
already been cleared, leaving its black backing over playback. The fallback now
requires the outgoing image. A full-desktop capture during this follow-up showed
that Riina was closed, so the new report still needs a live reproduction; native
surface alignment remains a separate open possibility if a patch persists with a
valid retained frame.

The Windows native alignment path now clears the retained transition image and
opaque cover immediately when it cannot verify the transparent controls surface
against the native video client's current bounds. This prevents an unaligned popup
from leaving its transition cover painted at an old desktop position. The move,
resize, and periodic alignment paths all use this guard. Build and live playback
verification are pending; the user's specific black patch has not yet been
captured in a live repro.

A debug GUI check launched `moving-square.avi`, but the saved playlist advanced
to `long-silence.wav` before a usable video comparison. The black viewport moved
with the player during a forced 180-pixel window move; that only confirms the
ordinary audio-only viewport follows its host and does not reproduce a patch over
active video. The exact test process was closed and no Riina process remains.
## REF-21 Quick Settings Layout proportions · 2026-10-15

Compared the Layout pane against the pinned IINA 1.5.0 source and REF-21. At the
inferred 2 px/point reference scale, the Windows mini material switch track was
narrow; it now measures 36×16 points. Updated card heights to 152/138 points for
the Theme/material card (spacious/compact), 44 points for Compact Interface, and
76 points for the two-row Video card. The workspace build completed successfully.
Rebuilt Riina 0.3.8 was inspected in both spacious and compact modes using full
3840×2160 desktop captures; neither state clipped the Layout controls. The
compact preference was restored and the test app was closed. Capture evidence and
SHA-256 hashes are in `tests/visual/windows-captures.json`. Exact geometry,
selected-tab focus appearance, Windows Acrylic material matching, and real-media
settings behavior remain open.

## REF-21 Quick Settings tab proportions · 2026-10-15

A native-resolution crop of REF-21 measures the four-segment header capsule at
about 191–192 points wide (the 2 px/point scale is inferred from its 360-point
sidebar). The earlier 123-point estimate came from a resized full-image preview
and was incorrect. Riina now uses a 192-point capsule. Its 86-point selected
segment, three 32-point icon segments, three 1-point gaps, and 6 points of inset
total 191 points inside the capsule. This is within about 1% of the reference at
the inferred scale. `cargo build --workspace --locked` completed successfully. A live capture is
still needed to confirm the selected label fit and header position because the
current computer-use surface exposes no native desktop apps.

## REF-22 Music Mode timeline geometry · 2026-10-15

Measured the IINA 1.5.0 capture at native resolution: the 600×518-pixel player
frame is x=112..711, y=76..593; the artwork/control boundary is row 413. At the
inferred 2 px/point scale, this is a 300×259-point frame with about 168.5 points
of artwork and 90.5 points below. Riina's layout allocates 169+90 points and
matches the total. The Music Mode timeline now follows IINA's 32-point minimum
for both time labels, right/left alignment around the seek bar, and 6-point
insets/gaps (Riina used natural label widths and an 8-point gap previously). The
`cargo build --workspace --locked` succeeded. A matched Windows capture is still
needed for rounded corners, title/timeline position, and video-backed Acrylic
appearance; the current computer-use surface exposes no native desktop apps.

## REF-24/25 playback panel spacing · 2026-10-15

Extended the IINA `LayoutValue.swift` measurements into the Windows Video and
Subtitle panes. Both now use the source's 16/14-point horizontal margins and
18/16-point outer card gaps in spacious/compact mode. Subtitle track groups now
use the source's 12/8-point item spacing. Layout, Video and Subtitle card corners
now use IINA's 8-point sidebar-container radius; the remaining Layout position
card also uses the source side inset. These adjustments preserve the video-backed
Acrylic surfaces and make the default 360-point sidebar cards 328/332 points wide.
`cargo build --workspace --locked` and `cargo fmt --all -- --check` now pass.
Live panel geometry and light/dark material comparison remain unverified.

An attempted rebuilt-app Audio-panel capture launched the moving-square fixture,
but the desktop capture returned a uniform black 3840×2160 image (36 KB). It was
discarded as invalid evidence; the exact Riina test process was closed and no
Riina process remains. This does not indicate a playback-rendering failure, since
the capture path itself returned a black desktop.

## REF-20 floating controller compact geometry · 2026-10-15

Applied IINA's `OSCFloatingView.swift` compact measurements to the Windows
floating controller: its standard compact height is now 60 pt, the top-row to
timeline gap is 8 pt in both layouts, and bottom padding is 5 pt. IINA's other
source values remain 460/200 pt preferred/minimum width, 14/8 pt spacious and
10/5 pt compact top/bottom padding, and 12/8 pt top-row/timeline side insets.
`cargo build --workspace --locked` and `cargo fmt --all -- --check` pass. Live
rendering comparison is still pending. Small-video compression remains the
current platform adaptation; the broken full-desktop capture did not verify its
rendered bounds or the Windows material pixels.

## Windows native overlay composition recheck · 2026-10-15

The user reports an opaque black patch covering playback that stays at the same
desktop position when the player window moves. That spatial behavior points to a
stale/opaque controls-overlay surface, rather than a decoder frame. The native
controls are a separate transparent HWND over mpv's video HWND. Reinspection found
that post-realization setup was clearing Winit's `WS_EX_LAYERED` composition style
along with its unwanted click-through flag. It now preserves layered alpha when
present and clears only `WS_EX_TRANSPARENT`, while retaining the DWM alpha setup.
This matches Winit's transparent-window contract and Microsoft's description of
per-pixel alpha composition. `cargo build --workspace --locked` and
`cargo fmt --all -- --check` pass; a visual move/repaint check is still required
to confirm this fixes the user's report.

The rebuilt test instance's native window geometry was measured before and after a
120×90-pixel host move: the controls popup moved from (546,215) to (666,305),
matching the host's (528,72) to (648,162) move plus its 18×143-pixel client inset.
This confirms the app popup itself followed the host in that sample, but the
realized popup style was `0x110` and did not contain `WS_EX_LAYERED`, so the test
does not confirm per-pixel alpha composition. The
full-desktop capture helper again returned a uniform-black 36 KB image and cannot
confirm whether the reported visible patch remained. A separate earlier capture
identified a desktop-fixed black NVIDIA Overlay banner (eye icon and
“Press Alt+Z to open NVIDIA Overlay”); that remains a plausible source for the
same symptom.
The user-specific visual cause is unresolved. The exact test process was closed and
no Riina process remains.

## REF-20 Windows controller translucency calibration · 2026-10-15

Measured flat video pixels immediately above and beneath the controller in the
pinned IINA image and the saved full-desktop Windows capture. The panels retain
0.69 and 0.34 of the underlying linear-sRGB luminance, respectively. The captures
use different colors and are not a matched pixel comparison, but show that the
saved Windows controller was too opaque. That Riina capture predates the earlier
fill reduction from 60% to 40%. Based on the IINA ratio and a simple linear blend
with the existing dark tint, the current Windows OSC dark fill is calibrated to
33% alpha (`#15151555`), down from 40%; the sampled-blur layer remains at 58%.
`cargo build --workspace --locked` and `cargo fmt --all -- --check` pass. A fresh
full-desktop capture could not verify the adjustment: the capture helper returned
a uniform-black image, so rendered verification remains open.

## Windows material sample ordering and fixed-screen black patch report · 2026-10-15

`cargo build --workspace --locked` and `cargo fmt --all -- --check` pass with
the WGC sampler fix. The capture callback now records each compositor frame's
QPC `SystemRelativeTime`; a slower older readback cannot replace a newer sample.
Visibility and media-generation invalidation also discard readbacks already in
flight. This addresses stale sample publication, but does not establish that
material lag or the reported black patch is fixed.

The user reports a black square that stays at the same screen coordinate while
the player moves. A fresh valid 3840×2160 desktop capture showed the video,
controls, sidebar and ordinary black aspect-fit bars moving with Riina after the
window moved 180×110 physical pixels. No separate patch remained at the old
position. The NVIDIA performance metrics stayed fixed at the upper-right. The
latest capture returned `HasNonBlackProbe: True`; an earlier uniform-black probe
was invalid and is superseded by this visual check. A later full-desktop capture
shows NVIDIA's black “Press Alt+Z to open NVIDIA Overlay” toast over the player,
beside its screen-fixed performance metrics. Six seconds later the toast is gone
while the metrics remain. This identifies a concrete external screen overlay that
can cover playback, but does not prove it was the same rectangle in the user's
report. The separate artifact remains unreproduced with the isolated fixture.
The saved local capture is
`reference/captures/riina-live-2026-10-15/windows-nvidia-overlay-toast.png`.
The exact test process was closed and no Riina process remains.

## REF-24 image-adjustment slider alignment · 2026-10-15

Measured the Equalizer card in the pinned REF-24 capture at 2× and the saved
Windows capture at 3×. Card widths agree at about 328 pt and the inner label
origins agree within one point, but the label-to-slider offset is about 65 pt in
IINA and 76 pt in Riina. Reduced the image-adjustment label column from 65 to
56 pt while retaining the source 8 pt row gap. The reset control slot is now
16 pt; its center moves from 23 pt to 21 pt inside the trailing card edge, close
to the reference's 20.5 pt. This changes slider travel and reset alignment while
preserving the measured card dimensions, row pitch, and video-backed Windows
material. Formatting and the full workspace build pass. A post-change capture is
still needed to confirm the rendered alignment; current screen capture continues
to return `CreateForMonitor` failure or uniform black.

## REF-24 Video Equalizer geometry and reset glyph · 2026-10-15

Compared the last Video panel card with the pinned `SidebarVideoPane.swift` and
`LayoutValue.swift`. Riina previously kept the Equalizer's row spacing at 8 pt
and inner padding at 8 pt in both UI modes. IINA uses 12/8 pt row spacing and
12/10 pt sidebar-container padding in spacious/compact mode, plus 2 pt of
Equalizer-internal vertical padding. Reduced the five slider/reset rows to 14 pt,
which gives the source-spaced 12/8 pt rows a 26/22 pt pitch and derives card
heights of 146/126 pt. Replaced the text reset glyph with an original, MIT-vector
reset symbol. `cargo build -p riina --locked` passed. The rebuilt 3840×2160
full-desktop captures at 300% scale show the spacious card at about 146 pt and
the compact card at about 126 pt. Their row pitches measure about 26/22 pt; all
five labels, sliders and reset controls are visible in both states, and UI
Automation reports every slider and reset button. A Light/Compact capture also
shows the Windows video-backed material through the panel and cards. Pixel-scored
material comparison in Light and exact reset-glyph matching remain open. Both
captures are indexed in `tests/visual/windows-captures.json`.

### Reset glyph source alignment · 2026-10-15

The pinned `SidebarVideoPane.swift` uses the filled circular counterclockwise
reset symbol on each video-equalizer row. Riina's first original vector drew
only a short open arc; it now uses a filled circle with a transparent reset-arrow
cutout, retaining the independent MIT vector asset and updated provenance hash.
`cargo build --workspace --locked` passed. A GUI session opened the Video panel
and UI Automation found its reset actions, but the full-desktop capture returned
uniform black after the panel opened. That capture is invalid; the glyph's
rendered silhouette remains unverified. The exact test process was closed and
no Riina process remains.

## Windows video material capture cadence · 2026-10-15

The Windows Graphics Capture callback now permits the bounded 320×180 compositor
sample to update at up to 30 Hz (33 ms) while a video-backed material is visible.
The independent mpv `screenshot-raw` fallback remains at 10 Hz; this preserves the
previous limit on that more expensive path. The change reduces WGC's scheduling
window for displaying a fresh sample, but does not prove frame synchronization or
acceptable playback cost. `cargo build --workspace --locked` and
`cargo fmt --all -- --check` pass; live moving-video capture and a controlled cost
measurement remain pending.

## Windows native material blur transform · 2026-10-15

The native-video blur now precomputes clamped horizontal/vertical tap positions
and adds its fixed kernel correction terms directly, retaining the prior integer
weights. In a 105-sample debug trace of the synthetic moving-square clip, WGC
readback took 5–7 ms, transform time was 17–20 ms (17 ms median, 18 ms p95),
and capture-to-display age was 30–63 ms (47 ms median, 60 ms p95). The earlier
native transform trace was 28–31 ms. This confirms a shorter transform in this
run, but overall sample age and playback cost still need a controlled comparison.
The local trace is `reference/captures/riina-live-2026-10-15/windows-material-transform-trace.log`;
the exact test process was closed.

## Quick Settings body width and native surface move check · 2026-10-15

The Layout and Video Quick Settings scroll views now size their viewports and
content layouts from the available sidebar width; the Video controls also take
the full body width. `cargo build --workspace --locked` passed. A live Windows
0.3.8 debug capture shows the Video cards and Layout groups extending across the
sidebar without the earlier narrow-strip clipping. The lower Layout content
continues below the visible viewport as a normal scroll area. These were bounded
Riina-window captures, not desktop captures.

During a focused drag with synthetic video visible, the host moved from
`(543,72)` to `(628,132)` and the mpv surface from `(546,215)` to `(631,275)`;
both moved by `(85,60)`. The post-move window capture still showed the video and
controller, with no black patch left behind in the captured player surfaces. A
green NVIDIA-eye badge was visible over the player in that capture. This supports
the existing NVIDIA-overlay lead, but the window-bounded capture cannot establish
the badge's desktop coordinates or prove it caused the reported black patch.
The player later advanced to `long-silence.wav`; its compact host and mpv child
also moved together by `(270,165)`. A DPI-aware desktop capture now returns the
full 3840×2160 screen with visible content, including the entire Riina window;
see the later material-worker verification entry. The exact Riina instance was
closed and no Riina window or process remains.

## Windows video-material UI responsiveness · 2026-10-15

The native sampled-video blur now runs on a single bounded worker. A one-frame
waiting queue drains to the newest sample, and only pixel buffers cross back to
the UI thread. Results from prior media generations and expired samples are
discarded. The WGC callback now bilinearly downsamples the mapped BGRA surface
directly into the bounded 320×180 sample, avoiding a full-resolution RGBA copy
and second downsample pass. The 0.3.8 debug app played an isolated animated
fixture; a full-desktop 3840×2160 capture confirms video across the playback
viewport with the translucent controller over it. The latest live trace measured
5–6 ms for readback, 28–31 ms for the worker transform, and 39–54 ms from
capture to display. The transform no longer blocks the UI timer. Rapid
clip-transition behavior and exact Windows Acrylic pixels remain open. See
`docs/verification.md` and the full-desktop capture manifest.

## Windows material sampler freshness and fixed-screen artifact · 2026-10-15

Material frames now use source-aware identities, preventing the first WGC frame
from colliding with mpv's fallback revision. Paused samples also expire after
750 ms, and native capture gets a 500 ms startup window before the mpv fallback
is allowed to render. A rebuilt playback run displayed WGC samples at 57–73 ms
age with 31–41 ms transforms. A full-desktop move check kept the video and
controls together and did not reproduce the black patch reported at a fixed
screen coordinate. A desktop-fixed NVIDIA performance overlay is visible and
may be relevant if the artifact returns. Rapid clip transitions and a direct
reproduction of the black patch remain unverified.

## Windows playback controller geometry · 2026-10-15

The REF-20 image was remeasured instead of relying on the previous 67-point
estimate. Its controller material is approximately 460×78 points; the source
measurements compose as 14 + 24 + 8 + 24 + 8 points. Riina now allocates that
78-point spacious height and 71 points in compact UI. The rebuilt 0.3.8 full-
screen Windows capture measures about 459×77 logical points at 300% display
scale, while the test video fills the complete display behind the controller.
The capture is indexed in `tests/visual/windows-captures.json`. Build, format,
and JSON validation passed; material tint, moving-video blur phase, and exact
pixel alignment remain open. The test process is closed.

## Windows playback toolbar proportions · 2026-10-15

The pinned IINA source sets each right-side playback action to a 24×24-point
frame and places them in a zero-spacing stack. Riina's former four-action
cluster occupied 132 points; it now uses the source-derived 96-point width.
Compact rows retain the 24-point action width while constraining height to the
available row. The rebuilt target-window capture shows the actions inside the
video-backed controller and is indexed in `tests/visual/windows-captures.json`.
Build and format checks pass; matched glyph shapes, hover states, and full-
desktop pixel comparison remain open.

## Windows playback toolbar symbols · 2026-10-15

Replaced the Windows PiP monitor-stand pictogram with a nested-picture-window
symbol and the play-arrow list with a bulleted playlist rectangle. These are
original Riina SVGs based on IINA's source symbol identifiers, with no Apple
artwork bundled. The rebuilt playback capture shows both in the corrected
24-point action frames and is indexed. Build and formatting pass; exact glyph
contours and interactive states remain open.

## Windows playback timeline geometry · 2026-10-15

Matched the timeline to IINA's 46-point centered elapsed/duration label slots,
2-point label-to-slider gaps, and 8-point outer inset. Small-video mode uses
34-point label minima to preserve seek-slider room. The rebuilt 918×619 capture
shows the tightened track and centered labels inside the measured 78-point
controller. It is indexed in the Windows capture manifest; pixel-scored
alignment and exact digit typography remain open.

## Windows fixed-screen black patch recheck · 2026-10-15

Opened `moving-square.avi` from an isolated temporary folder, captured the full
3840×2160 desktop, moved the Riina host 160×100 physical pixels, and captured
again while the clip was still playing. The video and its black aspect-fit bands
moved with the player; no black square remained at the old screen coordinates.
The screen-fixed NVIDIA performance metrics remained visible at the upper-right.
This controlled clip does not reproduce the user's media or playlist transitions,
so the reported patch is still open and no renderer change is justified by this
check. Both captures are indexed in `tests/visual/windows-captures.json`. Closed
the exact test process and confirmed no Riina process remains.

A second full-desktop check used the paused `moving-square.avi` fixture and moved
the player 180×110 physical pixels. The video, controls, Acrylic panel, and black
aspect-fit bars followed the window; no independent black square was visible at
the original screen position. This still does not reproduce the reported black
patch, so retain the renderer as-is until a capture made while the patch is visible
can identify its bounds and whether it belongs to the player or another overlay.

## REF-21 dark Windows material ratio · 2026-10-15

Pixel-sampled REF-21's flat green video (`#65C466`, `(2000,400)`) and un-carded
sidebar (`#459546`, `(2300,800)`). Their linear-luminance ratio is 53.7%. The
matched 300%-scale Windows Video-panel capture measures `#62B27D` outside the
sidebar and `#4C845F` in its un-carded strip, a 52.6% ratio at the same 360-point
sidebar width. This confirms dark sampled-Acrylic strength within 1.1 percentage
points for the two captures. It does not prove exact color or grain, Layout-card
fill, live frame phase, light-theme parity, or policy fallback behavior. The
Windows player is closed; this was offline image measurement, with no build or
automated tests.

## REF-21 Layout icon proportions · 2026-10-15

Compared the Layout-pane icon calls in the pinned `SidebarLayoutPane.swift` with
Riina's rendered component sizes. Corrected the Theme, Compact Interface and
video-setting symbols to 20 points, while the Liquid Glass heading and mini
material rows use 16 points. The generic sidebar toggle retains its 16-point
default, so Playlist/Chapters and other panes are unaffected. `docs/visual-spec.md`
records the per-row source measurements. The full-desktop capture script initially
failed because its DPI helper used unsupported C# syntax; that helper now compiles
in this PowerShell host. A fresh 3840×2160 capture after the change shows the full
960×540 video, the bottom controller, and the Layout pane in context. Its non-black
probes pass and its hash is indexed in `tests/visual/windows-captures.json`. The
short clip had reached its final frame, so transition behavior and pixel-scored
REF-21 comparison remain open. Closed the test window and confirmed no Riina
process remains.

## REF-23 Audio control geometry on Windows · 2026-10-15

The Audio Delay slider now uses 2-point round, low-contrast tick marks at the 21
supported positions. The track and knob retain their measured alignment, and the
neutral `0s` value appears above the centered thumb. Added 16-point clock-delay
and Equalizer heading icons and matched IINA's “Load External Audio Track…”
label. The custom SVGs are original MIT assets with provenance records. A rebuilt
0.3.8 player was inspected against the pinned Audio screenshot with the Quick
Settings pane open. The full-desktop capture shows Windows Acrylic over the video
and an opaque playback image. The inspection covers heading, tick, label and
surface details; exact whole-card scoring, light appearance, and live blur timing
remain open. Capture metadata is indexed in `tests/visual/windows-captures.json`.
The test app was closed and no Riina process remains.

The external-audio action was subsequently reduced from 30 to 24 points, with
12-point side insets, matching the reference control size within the fixed
48-point card. The rebuilt UI and full-desktop 3840×2160 capture confirm the
shorter control. See `windows-audio-action-24pt-full-desktop.png` in the capture
index.

The Equalizer now follows IINA's stacked heading, full-width profile, 120-point
faders, and separate 10-point frequency-label rows. The 300%-scale rebuilt capture
shows the corrected arrangement and video-backed Acrylic; scrolling exposes all
ten faders and labels at the current player height. Pixel-scored card bounds, light
appearance, and material timing remain open. Details and capture hashes are in
`docs/verification.md` and `tests/visual/windows-captures.json`.

## REF-21 Layout icon parity on Windows · 2026-10-15

Replaced the generic window, gear, chat, mouse, list, and filmstrip marks in the
Layout pane with independently drawn purpose symbols for Theme, Windows material,
OSD, OSC, sidebar, compact interface, edge-to-edge video, and aspect lock. Their
sizes follow the pinned 16/20-point source calls. The rebuilt full-desktop capture
shows these symbols over the live 4 fps green pattern with Windows video-backed
Acrylic. `cargo fmt --all -- --check`, `cargo build -p riina --locked`, JSON
parsing, and icon hash checks passed. Exact glyph contours, pixel-scored card/tab
edges, and light-theme appearance remain open. The test app was closed afterward.

## REF-21 Windows theme segment focus · 2026-10-15

The Layout Theme selector now hides its focus outline after mouse selection while
retaining the outline for keyboard focus. A rebuilt Windows capture verifies both
pointer and Space-key states over the live video in Light appearance; the full
desktop captures are indexed. Formatting and the application build passed. Light
Acrylic is visible, while pixel-scored card/tab and material comparison remains
open.

## REF-21 Quick Settings category icons · 2026-10-15

Corrected the tab-row glyphs to match IINA's Layout paintbrush, Video photo,
Audio waveform, and Subtitles caption bubble, using original Riina vectors at the
source's 14-point size. The rebuilt 300%-scale full-desktop capture and matched
header crop verify their meaning and proportions over Windows sampled-video
Acrylic. Increased the selected label from 11 to 12 points; its same-scale bounds
are now 76×22 px against IINA's 78×24 px, with the text still fitting. Exact icon
contours and card-edge pixel scoring remain open.

## REF-24 Video switch-card grouping · 2026-10-15

The pinned Video pane places Hardware Decoding and the optional HDR switch in
one sidebar container with an 8-point corner radius, 12/8-point row spacing,
and 12/10-point spacious/compact padding. Riina previously rendered these as
separate cards. The Video pane now groups both rows in one material card and
uses the source spacing and radius. The Aspect/Crop/Rotation card now also uses
the source's 8-point container radius. `cargo build --workspace --locked`
passed. Rebuilt window captures show the rounded cards in spacious and compact
modes over video-backed Acrylic; no controls are clipped. The app was restored to
Dark/Spacious and its test instance was closed. The pinned screenshot has no HDR
row, so verify the optional second row when a compatible HDR source/display is
available. Captures and hashes are in `tests/visual/windows-captures.json`.

## Windows transition-cover black-square guard · 2026-10-15

The native transition surface now verifies that an outgoing frame still exists
at the rendering boundary before enabling its opaque black matte. A missing
capture can no longer expose an empty black rectangle over live video. The
workspace build and formatting check passed. A fresh 3840×2160 desktop playback
check moved the player and let the fixture playlist advance; the video followed
the window and no stationary black square was visible. The user's affected media
is still needed to reproduce the reported symptom. The exact GUI process was
closed and no Riina process remains.

## REF-24 Aspect/Crop/Rotation card proportions · 2026-10-15

Source inspection found this Video card still used 8-point inner padding,
12-point spacing in both modes, and a fixed 200-point height. IINA uses
12/10-point spacious/compact padding and 12/8-point row spacing in
`SidebarVideoPane.swift`. Riina now applies those values and sizes the card to
its six source-height rows: 204 points spacious and 180 compact. A matched-size
rebuilt capture is needed to confirm card bounds and neighboring-card rhythm.

## REF-24 preset-rail mouse wheel behavior · 2026-10-15

The pinned `SidebarVideoPane.swift` converts vertical-only mouse-wheel motion
over Aspect and Crop rails into horizontal scrolling. Riina now handles that
input on the preset segments, clamps to the rail's measured content bounds, and
returns the event to the outer Video-panel scroll when the rail has no overflow.
Horizontal trackpad deltas remain owned by the ScrollView. Workspace compilation
and `cargo fmt --all -- --check` passed. Live wheel interaction remains open:
the Windows Computer Use capture service returned
`IGraphicsCaptureItemInterop.CreateForMonitor failed (0x80070057)` on both
capture attempts, so the wheel was not exercised in the UI. The test window was
closed afterward.

## Windows transition cover on window movement · 2026-10-15

When the native video host moves while an outgoing transition image is retained,
Riina now clears that opaque cover before realigning the transparent controls
surface. This prevents the old cover from being stranded at its previous desktop
coordinates. `cargo fmt --all -- --check` and `cargo build --workspace --locked`
passed.
The current full-desktop capture returned an all-black image even with Riina closed,
so this move-time behavior still needs a valid live capture. With Riina closed, no
small visible layered/topmost window was found beyond Explorer's 1×1 helper windows.

## Windows Quick Settings Acrylic detail · 2026-10-15

The saved REF-24 Video-panel capture shows the moving high-contrast test square
still reading sharply through the Windows sidebar. The Windows-only sampled-blur
opacity is now 80% (up from 53%); the calibrated panel tint, card fills, and
source-measured sidebar/card geometry are unchanged. `cargo fmt --all -- --check`
passed; rebuild and a valid high-contrast live capture are still required to judge
the result. The full-desktop capture helper currently returns an all-black image.

## Windows stale overlay on move · 2026-10-15

The move handler now requests a redraw when it clears a retained transition
cover. If the transparent controls popup fails its geometry check during a move,
Riina hides it until the 16 ms alignment retry confirms that it matches the video
host again. This targets a black patch that remains at the old desktop location.
The current full-desktop capture is all black and no Riina process is open, so
visual confirmation is pending. `cargo fmt --all -- --check`,
`cargo check --workspace --locked`, and `cargo build --workspace --locked` passed.

## REF-24 post-material-change capture attempt · 2026-10-15

Launched the built player with the isolated `moving-square.avi` fixture and
confirmed its window title, then closed that exact process. Windows Computer Use
returned a foreground-process error and then rejected a stale window ID while
listing Riina under a different current ID. The saved REF-24 live captures
predate the 80% sidebar blur change, so that setting remains visually
unverified; the all-black GDI desktop image is invalid evidence.

## Windows stale overlay retry recovery · 2026-10-15

The user observed a black patch remaining at a fixed desktop location while the
player moved. The move-event path already hid the controls surface after failed
alignment, but the independent 16 ms alignment retry only restored a hidden
surface after success; it did not hide a visible surface if that retry detected
drift without a timely move event. The retry now hides on any failed alignment
and restores visibility only after a later confirmed alignment. Formatting
passed. The rebuilt app launched to a blank welcome surface; a later capture
failed with `IGraphicsCaptureItemInterop.CreateForMonitor (0x80070057)`. The
exact Riina test process was closed, and visual confirmation remains pending.

## Windows overlay hidden during host moves · 2026-10-15

The user clarified that the black square covers playback and remains at a fixed
desktop position while the player moves. The move handler now hides the
transparent controls surface before attempting alignment, then restores it only
after the HWND bounds pass verification. This closes the brief path where an
owned popup could report stale geometry as aligned and leave its black-backed
surface at its previous position. `cargo fmt --all -- --check` and
`cargo build --workspace --locked` pass. The test instance was closed. Windows
Computer Use failed to capture its screen (`CreateForMonitor`, 0x80070057), so
the symptom's visual fix is not yet confirmed.

## REF-24 Speed card source geometry · 2026-10-15

The Video Speed card now follows the pinned `SidebarSliderView.swift` structure:
original 16-point speed glyph, title-row Reset, 50-point value field, live value
label at the slider thumb, 18-point title-to-slider gap, a 16-point bottom inset
made from a 2-point gap and 14-point tick row, and labels positioned at the
0, 1/3, 2/3, and 1 fractions. The labels now remain inside the card's vertical
layout; the prior nesting left them outside that layout. A pixel measurement of
the pinned screenshot gives a 92-point spacious card at its confirmed 2× scale;
the source 12/10-point container insets and 68-point content yield 92/88-point
spacious/compact cards. The slider now uses the reference's measured 10×18-point
thumb in a 20-point row; its value label sits 2 points above the row, and its
knob leaves 2 points before the tick labels. Slider tracks and markers now follow
the configured thumb center; default-width sliders keep their existing geometry.
The earlier 98/94 estimate used an oversized row and default 6×8-point thumb. A
rebuilt Windows capture is still needed. The SVG is
MIT-provenanced with a verified hash.
`cargo fmt --all -- --check` and `cargo build --workspace --locked` pass with the
measured row, thumb, and corrected nesting. The prior GUI capture attempt failed
with `IGraphicsCaptureItemInterop.CreateForMonitor (0x80070057)`; the latest build
has not been visually captured, and saved Windows captures predate this change.
No Riina test process remains open.

## REF-24 crop rail width follow-up · 2026-10-15

The saved Windows Video-panel capture shows part of the `Custom` crop choice next
to the scroll chevron, while the pinned REF-24 rail presents the six standard
choices before its cue. Pixel samples measure the selected Aspect `Default` at 68
points, Crop `None` at 56.5 points, and the five ratio segments at about
45/50/56/49/45 points. Riina now uses these measured widths, including the
fractional 56.5-point `None` segment, with no extra leading inset;
the 360-point spacious layout's 290-point viewport exposes the trailing portion
of `5:4` while leaving `Custom` beyond the scroll cue. Formatting passes. The
fractional 56.5-point rail refinement builds. UI Automation at 300% reports
Aspect `Default` at 204×72 px (68×24 pt) and Crop `None` at 169×72 px, consistent
with the 56.5-point target after integer screen rounding. The controls HWND aligns
to the native video client and is hit-testable above it. A valid full-desktop GDI
capture omitted the control panel, so rendered appearance remains unverified.

## REF-21 Sidebar scrollbar visibility · 2026-10-15

The sidebar scroll view now hides its vertical scrollbar at rest, matching the
pinned `SidebarPane.swift` setting `autohidesScrollers = true` and the reference
capture. Wheel and trackpad scrolling remain enabled. `cargo fmt --all -- --check`,
`cargo check --workspace --locked`, and `cargo build --workspace --locked`
passed. The rebuilt at-rest appearance and scroll interaction need a valid live
capture; the latest Windows Graphics Capture attempt failed with
`CreateForMonitor (0x80070057)`.

## REF-20 floating controller vertical bounds · 2026-10-15

The saved vertical position is a normalized bottom inset, but the Windows
controller previously applied the 25-point drag bound at the bottom of the
video area. Converted IINA's bottom-origin clamp to Slint's top-origin bounds:
the stored bottom inset is capped at `video height - 67 - 25`, and dragging keeps
the controller at least 25 points from the top while allowing its bottom edge
to reach the video edge. Horizontal dragging keeps IINA's 10-point left inset
while allowing the right edge to reach the video boundary. This preserves
IINA's 67-point position clamp for the 78-point visible panel. Formatting passes;
the workspace build passes. The rebuilt
position has not been visually captured; Windows Graphics Capture currently
fails with `CreateForMonitor (0x80070057)`.

## Saved-filter menu shortcut column · 2026-10-15

Saved Video Filter and Saved Audio Filter entries now pass their recorded key
and modifiers to native menu accelerators on both the shared player menu and
Windows native-video menu. This replaces whitespace appended to each title, so
the menu backend can align shortcuts in its reserved trailing column. The
filter-editor label fields remain available for editing. Formatting and the
workspace build pass. Native Windows menu alignment and activation still
need a live menu check.

## Windows fixed-position black patch follow-up · 2026-10-15

Rechecked the current Windows build with an isolated moving-square fixture.
Moving the host by 120×80 screen pixels moved its visible controls HWND by the
same amount and retained the measured 18×143-pixel client inset. The separate
36×36 window at desktop origin is Winit's layered, transparent
`Winit Thread Event Target` helper, not the controls surface. This confirms the
app windows followed the move, but does not show whether stale compositor pixels
or another desktop overlay left the reported black square behind: Windows
Graphics Capture still failed with `CreateForMonitor (0x80070057)`, and the GDI
desktop capture was uniformly black. NVIDIA Overlay processes were running, but
the unavailable visual capture cannot establish that they caused the patch.
The exact test process was closed and no Riina process remains.

## REF-24 Speed track width follow-up · 2026-10-15

A 2× crop of REF-24 measures the Speed slider track at about 246 pt. The rebuilt
300%-scale UI Automation bounds measured 248 pt for Riina, while its 50-point
input slot and 8-point gap already match the reference. Increased the spacious
and compact horizontal card insets by one point per side, producing a 246-point
slider bound. The post-change live UIA query confirms 738 physical pixels
(246 pt) at 300%. Formatting and the full workspace build pass. The full desktop
capture still returns a uniform-black image, so thumb artwork, value-label
placement and material appearance remain visually unverified.

## REF-21 Layout idle scrollbar correction · 2026-10-15

The saved REF-21 comparison shows no scrollbar in IINA's Layout pane, while
Riina's corresponding capture has a persistent right-edge indicator. The Layout
ScrollView had omitted the `always-off` policy already used by the other Quick
Settings panes. It now hides the scrollbar while retaining wheel scrolling.
`cargo fmt --all -- --check` and `cargo build -p riina --locked` pass. On the
rebuilt isolated-pattern run, UI Automation opened Layout and reported its
controls; a wheel input made the lower Settings row discoverable. Full-desktop
GDI capture still returns an all-black image, and Windows Graphics Capture is
unavailable, so the rebuilt idle appearance and access to the final lower rows
remain unverified. The exact test process was closed and no Riina process
remains.

## REF-21 Layout measured scroll extent · 2026-10-15

Replaced the Layout pane's fixed 760-point scroll height with the greater of its
visible viewport and the `VerticalLayout`'s preferred height. This derives the
scroll range from the actual card/section geometry as settings change, instead
of reserving an arbitrary blank tail or cutting off content. Formatting and the
locked application build pass. A clean GUI verification could not be completed:
Windows `ScreenClippingHost` owned a topmost 3840×2160 window over the desktop,
so the attempted player interaction and captures are not valid visual evidence.
The test Riina process was closed; the Windows capture overlay was left alone.

IINA's autohiding scrollbar also remains available during active scrolling. The
Layout pane now temporarily switches its native scrollbar on while its ScrollView
reports scrolling, then hides it 850 ms after activity stops. Repeated scroll
events restart the timer. The locked app build passes; the Windows snipping overlay
remains present, so the transient indicator and delay still need a live visual
check. Changing sidebar panes also clears its temporary visibility state.

## REF-21 Fit Window to Video visibility and card bounds · 2026-10-15

IINA's `SidebarLayoutPane` shows Fit Window to Video only when window-aspect
ratio lock is released. Riina previously also showed it whenever Edge-to-Edge
Video was disabled, adding an unreferenced action in Docked mode. The action now
follows the unlocked preference. The Video card height follows its visible rows:
44 points for the single Docked row, 76 for the two Edge-to-Edge rows, with the
32-point Fit action added only when unlocked. Formatting and the locked app build
pass. A clean state-by-state visual check remains pending while the Windows
snipping overlay covers the desktop.

## REF-21 OSC bounds around open sidebars · 2026-10-15

The saved full-desktop capture shows the floating on-screen controller extending
under the open trailing Quick Settings panel while Edge-to-Edge Video is enabled.
Its right-side actions can therefore sit beneath sidebar text. The controller's
horizontal position and clamp now use the unobscured video span after subtracting
visible leading and trailing sidebars. The video rendering remains edge-to-edge;
this only keeps its controls in the region the user can see and reach. Formatting
and the locked workspace build pass. Live UI Automation verifies the resulting
non-overlapping bounds; rendered alignment, pointer hit-testing, and drag behavior
remain pending a clean capture with Quick Settings open.
From the saved 3840×2160 capture at 300% scaling, the player window is about
915 points wide and its trailing sidebar about 360 points. That leaves a 555-point
visible span: the 460-point floating bar shifts from a full-window centered
origin near 228 points to about 48 points, removing roughly 132 points of overlap.
An isolated-fixture UI Automation run on the rebuilt binary confirms the last OSC
action ends at x=2033 px and the first Video-panel row starts at x=2264 px, a
77-point clear gap. The exact test process closed normally and no Riina process
remained. Further UIA checks verified Layout with its Settings panel leading,
Playlist and Plugins leading alongside Quick Settings trailing, and confirmed
that optional toolbar actions hide when the dual-sidebar video span is too
narrow while center transport controls remain within it. The second test process
also closed normally. The snipping overlay still prevents pixel/material
verification and the live drag-fraction check; see
`tests/visual/riina-osc-sidebar-geometry-evidence.json`.
The current UIA Subtitles-tab hit rectangle ends 18 physical pixels beyond
the window bound, but UIA does not mark it offscreen and invoking it opens the
panel successfully. The older saved screenshot shows its glyph inside the frame.
Treat this as a bounds discrepancy, not confirmed visual clipping; reconcile it
with a fresh pixel capture before changing the 192-point tab capsule.

## REF-21 On Screen Controller tile dimensions · 2026-10-15

Measured the pinned reference's selected position tile in the native-resolution
REF-21 capture: 132×148 px at the inferred 2× scale, or 66×74 points. The two
gaps are 16 px, or 8 points each; the complete three-tile row is 214 points.
Riina's prior 216-point row used 65.33-point tiles and 10-point gaps. The row
now uses the measured 66×74-point tiles and IINA's source-defined 8-point
`UIHelper.hStack` spacing. The existing Windows capture's tiles measure
65.33×74 points, so card height already matches; post-build UIA should verify
the width and spacing update. The earlier crop's overlapping OSC is stale for
the new sidebar-bounds fix and is not evidence of current overlap.
The source geometry review also found a 40×28-point preview viewport, 8-point
vertical gap, and 12-point item padding; the visible reference frame is about
32.5×25 points. Riina now lays out the same viewport and gap, with an inset
32×25-point frame. The incremental workspace build and formatting check passed.
The rebuilt app opened a prior `moving-square.avi` session, but the live capture
helper failed twice at `CreateForMonitor` (`0x80070057`). No UIA or pixel
measurements were obtained; the exact test process was closed and confirmed
absent. The tile bounds and reported fixed-screen patch remain unverified in this
run. A post-exit 3840×2160 desktop capture returned black at all 25 probes and
cannot classify the fixed-screen patch as Riina-owned or external.
The source tile outlines are also now represented: 8-point corners, 2-point
selected outline, and a 1-point theme-aware inactive outline. Formatting and the
incremental workspace build passed after this correction. Runtime pixel
verification remains pending. The OSC row also has the source's 8-point bottom
inset before the next 18-point section gap; the Windows Layout pane now places
the Sidebar Position heading 26 points after the tiles. Formatting and the
locked workspace build passed after the spacing correction; live pixels remain
unverified. The pinned section headings use bold 12-point labels; the previous
Riina capture's 11-point caption glyph measured about 16 px high versus IINA's
19 px at matched scale. The caption now uses 12 points; the locked workspace
build and formatting check passed, while live glyph measurement remains
pending. The Sidebar Position card now uses source-derived 24-point rows,
10/6-point spacious/compact spacing, and 12/10-point padding, with derived
118/106-point card heights and the existing 8-point transparent scroll-tail.
The geometry needs a rebuilt matched-scale capture; prior captures show the card
near the viewport's lower edge and cannot verify the new row bounds.

REF-25 Subtitle lane headers now follow the source's 16-point row height and
numbered primary/secondary badges, with 36×16-point Windows-accent switches
replacing 28-point rows and circular Tool buttons. The source group gap and
98-point track lists remain unchanged. The corrected workspace build and
formatting check passed; a matched Windows capture is still pending.

## REF-21 Quick Settings header bounds · 2026-10-15

The full-width header row previously ignored its own 10-point side insets. At
300% scale, UI Automation placed the Subtitles tab 18 physical pixels outside
the player edge. The row now subtracts both existing insets while preserving
the 192-point category capsule. The rebuilt app measures the capsule's outer
edge exactly 10 points inside the window; primary and secondary subtitle headers
measure 16 points high. Workspace build and formatting passed. The fresh
full-desktop capture helper returned an all-black image, so rendered-pixel
verification remains open.

## REF-24 Speed card live bounds · 2026-10-15

At 300% Windows scaling, UI Automation on the rebuilt `riina.exe` measures the
Speed slider at 246 points and the speed input slot at 50 points. The live-value
indicator, slider, and 14-point tick-label row appear in the intended sequence;
tick bounds follow the source fractions across the track. The Hardware Decoding
card begins below the 92-point spacious Speed card. The exact test process was
closed normally and no Riina process remains. This confirms accessible geometry
only: the Windows capture service did not provide valid rendered pixels, so the
thumb, text, and material still need visual verification. Measurements are in
`tests/visual/riina-osc-sidebar-geometry-evidence.json`.

## REF-24 Speed tick-label thumb centering · 2026-10-15

The 300%-scale UIA bounds placed the middle tick labels at 68 and 148 points
from the slider's left edge. `SidebarSliderView.xOffset` spaces interior labels
over the thumb travel using half the thumb width; the rebuilt Riina slider uses
a 10-point thumb. The middle labels now use that 10-point travel instead of the
old 6-point inset, shifting each by two physical pixels toward its corresponding
thumb center. A rebuilt bounds check is pending; prior bounds remain in the
geometry evidence file.

## REF-24 Hardware Decoding switch dimensions · 2026-10-15

The 2940×1898 pinned capture measures the selected switch track at about 88×40
physical pixels, or 44×20 points at the established 2× capture scale; its white
thumb is about 26×16 points. The previous Riina UIA bounds were 30×18 points at
300% scaling. The reference also places a 16-point CPU glyph before the default
13-point label: pixel measurement puts the label at a 36-point leading inset
and about 117.5 points wide. Riina now has an original chip glyph, an 8-point
gap, and the 13-point label. The Hardware Decoding card height follows its
20-point row in spacious and compact layouts; the optional-HDR card height
includes that row plus the existing source-sized gap and HDR row. The HDR row
now also has an original 16-point sun symbol and an 8-point gap before its
13-point label. The pinned source uses a small NSSwitch on macOS 26 and later;
the pinned 11d HDR preferences capture measures that system switch at 36×16
points with a 21×12-point selected thumb. Riina now uses those dimensions. The
locked workspace build and formatting check pass. HDR-row UIA bounds and
rendered pixels remain open.

An isolated `moving-square.avi` playback produced both a valid player-window
image and a 3840×2160 full-desktop capture. At frame 119, the video and its
letterbox bars moved with the player; no fixed black patch covered the image.
The floating bar looked consistent with IINA's 460-point width and lower
placement. The Quick Settings material could not be opened because the Windows
foreground-window guard rejected input, so its live translucency remains
unverified. The exact test process was closed afterward.

## Windows video material capture fallback · 2026-10-16

The saved REF-21 Layout capture shows desktop details through parts of the player
sidebar, even though that surface is meant to sample the playing video. The
sidebar now tracks whether its blurred video sample is current. On Windows it
uses the transparent sampled-video Acrylic layer only while a valid sample is
available; startup, capture failure, timeout, renderer reset, or disabled
transparency falls back to the solid app theme so desktop content cannot bleed
through the player. Other platform material paths keep their existing behavior.
The first build passed formatting and the locked workspace build. A later live
test opened the Video panel with the app's Control-Shift-V shortcut. On a moving
sample-pattern frame the panel showed the sampled video material; at the paused
end frame, its sample expired and the opaque fallback appeared. The sampler now
retains a displayed sample while playback remains paused at the same position
(within one 4 fps frame), and discards it after a seek or media-generation
change. `cargo fmt --all -- --check` passes; the locked workspace build for this
paused-frame correction passes. In the rebuilt UI capture, the Video panel stays
translucent over the blue paused frame after a 1.2-second wait, longer than the
750 ms sample timeout; the sample does not fall back to opaque and no desktop
content shows through. The live capture was inspected but not saved. A pixel-
scored comparison against REF-24 and the Light appearance remain open. The exact
test process was closed and no Riina window remains.
The captured Windows Video sidebar measures 360 logical pixels wide, matching
IINA's pinned 360-point default in `Preference.swift` and Riina's default width;
the live panel width does not need retuning.

## REF-21 Quick Settings selected-tab label alignment · 2026-10-15

The pinned REF-24 crop shows the selected icon and label on one horizontal
centerline. In Riina's saved REF-21 capture, the icon sat visibly higher than
the selected label inside the pill. `QuickSettingsTab` now centers its icon and
text cells on the horizontal layout's cross axis. The selected segment also
uses the measured label width, so Layout and Video no longer share one fixed
selected width. A native-resolution crop estimates the Layout capsule at
191–192 points and Video at about 186; each icon-only segment is 36 points.
The 0.3.8 executable opened with `moving-square.avi`, and a valid 3840×2160
desktop capture confirmed playback, the four right-side player tools, and the
filename in the window title. The desktop-control bridge listed no native apps,
so I could not activate Quick Settings and inspect its settings rows. The tab
alignment and settings fit still need a live overlay capture; no parity claim
is made from the playback-only capture.

## REF-24 Windows sampled-video material blur · 2026-10-15

The saved 300%-scale moving-square capture retains a recognizable high-contrast
edge through the Quick Settings material. The sample opacity was already raised
to 80%, but the Windows native/shared blur kernel still used sigma 8 across a
320×180 sample. It now uses sigma 10, which scales to roughly 29 points at the
measured 915-point player width. The calibrated tint and layer opacity remain
unchanged, so flat-frame luminance targets are preserved. Formatting and the
locked workspace build pass. A rebuilt visual capture is pending; the previous
Computer Use attempt failed before it could open Quick Settings.
