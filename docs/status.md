# Current project status

## Application state

Riina is an established Rust media-player application, beyond its initial feasibility
and playback milestones. The current development version is 0.3.3. It opens and plays local media through libmpv on Windows,
with native D3D11 video presentation, working transport controls, and broad player
and preferences workflows. Regression tests cover shared state, playback, settings,
file handling, and lifecycle behavior. The pinned Windows libmpv also passes a
native HTTP playback check that serves the repository's moving-square fixture over
localhost and verifies advancing playback, duration, and the selected audio track.
This does not verify physical audio hardware, public network streams, or macOS playback.
The stored Disable Animations preference combines with live Windows, macOS, and
Linux desktop Reduce Motion settings while the player or reusable Preferences
window is open. Linux reads the standardized XDG Settings value and follows its
change signal; native portal behavior and the complete animation inventory
remain open.
Interface > On-Screen Controller now persists independent visibility choices for
all nine IINA toolbar action types, including Full Screen, Music Mode, Subtitle
Track, Screenshot, and Live Text. Each action can be moved up or down in the saved
order; the default remains Plugins, PiP, Playlist, Quick Settings. Matching
IINA's drag/drop presentation and grouped GUI interaction review remain open.
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
preserve video aspect while resizing around the window center. A moving-square
fixture debug run started, but the current desktop-control session exposed no
inspectable application windows, so live menu behavior remains unverified.
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
File, Playback, Video, Audio and Subtitles sections now follow the pinned
`MainMenu.xib` order and separators for implemented entries, including nested
audio-track choices and the reference's alternate speed, volume and delay steps.
Shortcut metadata is configured for the menu's right-hand column using Command
on macOS and Control on Windows. These are static source-defined equivalents;
matching user-customized IINA key bindings dynamically remains open. Windows
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
and open history views refresh from the shared revision. Plugin state,
cross-process history/duplicate synchronization, and live lifecycle verification
remain open.
General settings now persist IINA's quit-after-all-windows-close preference
(default off) and keep-window-open-after-playback preference (default on). On the
shared-renderer path, closing the last player restores Welcome when quitting is off;
the Windows native-video lifecycle and live close/reopen behavior remain open. The
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
are implemented. Track-selection feedback now includes the selected track ID and
localized language name, with IINA's synthetic `#0 <None>` entry when a track is
cleared; matched OSD rendering and live locale changes remain open.
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
flag. In intelligent mode it also recognizes numbered episode groups from three
or more sibling media files and uses a bounded reciprocal nearest-name fallback
when direct and episode matching find nothing. Episode grouping preserves stable
filename tokens while matching multiple varying numeric identifiers, such as season
and episode numbers, across three or more sibling media files. Explicit episode-number
conflicts and subtitles already matched to neighboring media are excluded from that fallback.
Configured priority keywords can then choose the preferred external track.
Riina now includes manual OpenSubtitles search and download, stores
the user's key in the platform credential store, migrates any saved plain-text key,
and can open a search for local media above the configured duration threshold when
no subtitle tracks are found after discovery settles.
Search uses preferred subtitle languages and an OpenSubtitles media hash when
available. The Auto Load matcher now measures bounded prefix and suffix distances
for numeric-pattern groups; its group thresholds and assignment rules still differ
from IINA's recursive matcher. Live provider playback, captured UI comparison, and
matched macOS visual behavior remain open.

Windows 11 x64 is the supported application target. macOS and Linux application
ports are in development and are not supported yet. Full application builds, visible
playback, input, dialogs, window lifecycle, and shutdown evidence are required for
support.

## Parity state

The pinned baseline is official IINA v1.5.0. The parity inventory currently traces
243 atomic requirements, and the inventory is still incomplete. Many requirements
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

The latest published GitHub release is Riina 0.3.2 and includes a Windows x86_64
portable application, checksums, and a release verification record. Its successful
candidate run reused a previously built native engine, passed native playback and
shutdown checks, and verified the package against its tagged application source and
native source manifest. The [release workflow](https://github.com/deminden/riina/actions/runs/38259354701)
and artifact record are available online.

The v0.3.0 source archive measured 1.60 GB compressed and 2.57 GB expanded.
Its native-source snapshot included 1.01 GB of Git object metadata across 5,568
entries. The v0.3.0 archive remains unchanged. The v0.3.2 release does not attach
duplicate native or vendored Rust source trees: the exact application source is
served from the release tag, Cargo.lock pins Rust sources by checksum, and the
package's `SOURCE-MANIFEST.tsv` identifies each public native repository, revision
and source-file hash. `SOURCE-ACCESS.md` provides retrieval and rebuild directions.
The audit also found a 166 MB Opus DNN model archive among the native build inputs;
the pinned builder recipe retrieves it from its upstream source.

Release assets:

- [Windows portable ZIP](https://github.com/deminden/riina/releases/download/v0.3.2/riina-0.3.2-windows-x86_64.zip)
- [Tagged source code at v0.3.2](https://github.com/deminden/riina/archive/refs/tags/v0.3.2.tar.gz)
- [SHA-256 checksums](https://github.com/deminden/riina/releases/download/v0.3.2/SHA256SUMS.txt)
- [Verification record](https://github.com/deminden/riina/releases/download/v0.3.2/RELEASE-VERIFICATION.json)

The next v0.3.3 candidate must pass the exact package, source-access, and playback
gates before its Windows binary is public. Earlier native shutdown failures
and their resolution remain in `docs/native-distribution-audit.json` as
release-engineering evidence.
The x264 encoder is excluded from the release engine; FFmpeg's built-in H.264
decoder remains enabled. The development DLL is not the release runtime. License
terms and package provenance are recorded in `docs/licensing.md` and
`docs/native-distribution-audit.json`.

## Verification state

The parity inventory contains 243 currently inventoried requirements,
while explicitly reporting that the inventory is incomplete. The 0.3.3 candidate
must run the complete package, corresponding-source, and playback gates. The Unix presentation marker
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
UTF-8/16/32, strict ASCII, Latin-1, and common WHATWG labels are decoded; Foundation
encodings absent from or mapped differently by the portable decoder remain open.
Move accepts the reviewed absolute-path permission for either endpoint;
overwriting external files through text `write` is denied to match the plugin API
behavior.
`@data` and `@tmp` are plugin-scoped; `.` and in-root `..` segments are normalized,
root escapes and storage symlinks are rejected, and absolute paths require the
reviewed file-system permission. `@current/<path>` now expands against the current
media directory and requires that grant; `@video/<id>`, `@audio/<id>`, and `@sub/<id>`
resolve the current external track paths from playback context. Trash, unsupported
Foundation encodings, cross-volume move behavior, and exact IINA path semantics
remain open. `showInFinder` now resolves a plugin path and delegates to
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
