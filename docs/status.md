# Current project status

## Application state

### Current window and controller work

Small video windows keep the transport centered and the timeline visible. The
volume and toolbar groups disappear together when there is insufficient space,
following the pinned IINA floating-controller behavior. Live Windows captures
confirmed the portrait layout, a native border drag to a small landscape window,
and restoration of the full controller on maximize. The gear opens Quick Settings;
the native Video menu then displays “Hide Video Panel” and its shortcut.

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

Riina is an established Rust media-player application, beyond its initial feasibility
and playback milestones. The current development version is 0.3.4. It opens and plays local media through libmpv on Windows,
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
Command-line HTTP, HTTPS, RTSP and RTSPS media references now share URL validation
with the Open URL workflow and retain their position among local paths; live URL
playback and the wider IINA command-line option grammar remain open.
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
removal of both points produces one clear message. Matched OSD rendering, live
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
Riina now includes manual OpenSubtitles search and download, stores
the user's key in the platform credential store, migrates any saved plain-text key,
and can open a search for local media above the configured duration threshold when
no subtitle tracks are found after discovery settles.
IINA's provider submenu also offers Assrt, Shooter, and plugin-backed providers;
Riina currently implements OpenSubtitles only, so matching those providers and
their menu actions remains a tracked feature gap.
Search uses preferred subtitle languages and an OpenSubtitles media hash when
available. The Auto Load matcher uses bounded prefix and suffix distances for
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
`.url` files resolve only validated HTTP, HTTPS, RTSP or RTSPS targets before media
selection. Raw URL/text drops, dropping onto playlist rows, and live
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

The latest published GitHub release is Riina 0.3.3 and includes a Windows x86_64
portable application, checksums, and a release verification record. Its successful
workflow reused a previously verified native engine, passed native playback and
shutdown checks, and audited the exact package against its tagged application source
and native source manifest. The [v0.3.3 release](https://github.com/deminden/riina/releases/tag/v0.3.3)
and [successful release workflow](https://github.com/deminden/riina/actions/runs/38332824004)
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

Release assets:

- [Windows portable ZIP](https://github.com/deminden/riina/releases/download/v0.3.3/riina-0.3.3-windows-x86_64.zip)
- [Tagged source code at v0.3.3](https://github.com/deminden/riina/archive/refs/tags/v0.3.3.tar.gz)
- [SHA-256 checksums](https://github.com/deminden/riina/releases/download/v0.3.3/SHA256SUMS.txt)
- [Verification record](https://github.com/deminden/riina/releases/download/v0.3.3/RELEASE-VERIFICATION.json)

v0.3.3 is published with its Windows portable binary. The exact 72.6 MB package
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
permission and network URLs require network permission plus a manifest-allowed
domain for HTTP(S) and SMB. Other URL schemes, including RTSP, follow IINA's
filesystem-permission branch. The playback-history getter is implemented, and
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
The requested opt-in volume-boost switch remains between delay and EQ. Backend
track switching is covered by the full native playback suite. The Slint
layout compiles and the workspace test suite passes; the matched live capture
remains unverified because the current monitor-capture request fails with
`0x80070057`.

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
