# Architecture

Riina keeps playback policy in Rust and uses libmpv for decoding and the
playback clock. Slint components display state and emit user intents. Native
window and service adapters live under `crates/app/src/platform/` and share a
facade in `crates/app/src/platform/mod.rs`.

## Workspace

```mermaid
flowchart LR
    App[riina app] --> Core[riina-core]
    App --> Engine[riina-mpv]
    App --> UI[riina-ui]
    Engine --> Core
    Engine --> MPV[libmpv]
    App -. tests .-> Support[riina-test-support]
    Engine -. tests .-> Support
```

| Crate | Responsibility |
|---|---|
| `riina` | Application entry point, player sessions, preferences, plugins, auxiliary windows and platform adapters |
| `riina-core` | Typed commands, native command plans, playback state, preference validation, catalogs and portable policy |
| `riina-mpv` | Native ABI, command queue, engine worker, observations and video renderer |
| `riina-ui` | Compiled Slint components shared by the application and widget tests |
| `riina-test-support` | Test-only fixture paths, isolated temporary directories and snapshot waits |
| `xtask` | Workspace checks, focused native runs and optional coverage reporting |

The `riina` executable delegates to `riina_app`. The player composition entry
point is `player/player_ui.rs`. Its feature modules install window lifecycle,
transport, audio and subtitle controls, media actions, plugins and rendering.
`PlayerContext` shares weak window access, the command sender and state needed
by UI callbacks. It does not own the native player.

`riina-core::Command::plan` routes intents to the playback, seek, audio, video,
filter, subtitle, playlist, track, capture, network and screenshot modules.
Command, preference and plugin-manifest errors retain their structured data
until a caller needs a message for the user or plugin. The engine returns
distinct errors when its command queue is full or its worker has stopped.
See [Add a playback command](recipes/add-a-command.md).

## Embedded mpv script input

The engine owns the console overlay, history, statistics pages and their
bindings. The player observes the console's availability, open state and active
bindings. A bounded queue retains early console keystrokes until the engine
installs the script's key handlers. libmpv does not notify clients when
bindings change, so the worker refreshes only during a script transition.
It provisions room for the batch and checks ownership again before delivery.
The app owns the ordinary key profile and disables mpv's standalone defaults,
which can otherwise claim printable console keys on older engines.
The window's input handler runs before Slint dispatches menu shortcuts, so the
console receives text that matches the ordinary profile.
The player gives active statistics bindings priority over plugin and playback
shortcuts while leaving other keys available to the application.

## Standalone plugin windows

The realm owns the document, queued messages, frame requests and visibility.
Loading a package page or entering Simple Mode creates a hidden native host.
The page can exchange messages before Open and after a native close. Open
raises the existing window and preserves the current document. Loading another
page advances the document identity and retires the previous message listeners.

GTK owns the Linux host. On Windows and macOS, Winit creates the hidden
host inside the application's existing event loop. The presenter drops the
WebView before the native window. On macOS, native creation and page-load
callbacks wake the windowless presenter so a hidden page can finish starting.

## Wayland plugin sidebar input

On the player's Wayland display connection, the guest event queue owns one
browser text-input object per seat. The offscreen WebKit view provides the
focused editor, caret rectangle and surrounding-text selection for the entered
player surface. The seat binds each compositor edit to the corresponding
advertised state before delivering it to WebKit. Password fields publish
their purpose and caret only. WebKit owns composition, DOM editing events
and selection. The browser releases the seat synchronously before a Slint
editor takes over with Winit's text-input object. If the compositor returns
a reply on the disabled browser object, the guest queue accepts only the
current serial and forwards composition to the focused writable Slint field
on the entered surface.

## Playback and threading

```mermaid
flowchart LR
    Action[Slint callback] --> Queue[Bounded command queue]
    Queue --> Worker[mpv worker]
    Worker --> Engine[libmpv]
    Engine --> Event[Native event]
    Event --> Worker
    Worker --> Mailbox[Playback snapshot and completions]
    Mailbox --> UI[Slint event loop]
    UI --> Projection[Window properties and catalog models]
```

The UI thread owns windows, callbacks, timers and the preference model. The mpv
worker in `crates/mpv/src/worker.rs` serializes intents with native events. It
validates each intent against the observed `Playback` state before submitting the
plan. Its `Handle` ties each borrowed native event to the wait that returned it.
The worker copies payloads before advancing the event queue.

The shared playback profile distributes committed ReplayGain options to open
Settings forms and engines. The worker acknowledges user volume adjustments
separately from temporary volume restoration. The profile keeps the newest
acknowledgement and saves it after a short idle interval or when a player
closes. Individual window saves leave this field under profile ownership.

Screenshot completions retain their playback generation and sequence across
the UI handoff. Background decoding shares one result between the preview
and native clipboard. PNG, JPEG and WebP use the Rust image decoder. JPEG XL
and AVIF use a separate local-only libmpv decoder with a bounded wait and no
presentation surface. A newer request can supersede pending preview work.
Clipboard-only files stay under the session's scratch ownership through
decoding and are removed before the UI presents the result.
AVIF defaults to 10-bit 4:2:0 input for the configured SVT encoder. This
converts RGB from GPU-rendered captures and subsampled sources that the
encoder otherwise rejects. Advanced mpv options can override the format.

The playback worker checks a saved preferred audio device against libmpv's
device list before the first file. The discovery wait is bounded and an
unavailable preference leaves the engine on Automatic. An explicit mpv
option or config retains its own device selection. During playback, the
worker repairs a disappearing known device once per observed change.
It uses the matching Apple endpoint when changing between Core Audio and
AVFoundation and submits recovery on the same native queue as explicit
device selections. Recovery updates the session without saving a preference.

The playback worker owns the chapter list, native chapter index and current
performer. Compact controls and desktop media sessions share the chapter
metadata projection while the full player's document title follows the file.
Timeline hover resolves the pointer's chapter separately from the playhead.
The time and chapter label remain available independently of the isolated
thumbnail decoder and its cache.
Chapter controls submit the observed media generation, chapter index, and
start time. The worker retires stale selections before seeking. An accepted
chapter menu seek publishes its title to the player's transient feedback.

Live Text reacts to paused picture changes and capture replies. Each player
debounces capture for the current media generation and holds one recognition
task. Seeking, resuming, disabling Live Text, entering Picture in Picture or
Music Mode, and editing the crop retire that task and clear its overlay. The
decoder and rendering thread remain independent of text analysis. Linux sends
the cancellation to Tesseract's progress monitor. Windows requests cancellation
through WinRT. The macOS adapter discards results when a Vision call returns
after its frame was retired. The overlay takes text input inside a recognized
region and leaves the regular video gestures available elsewhere.

Windows system media controls belong to the player's native toplevel. The
session publishes observed titles, playback state and seek bounds. Native
callbacks return to the UI thread and use the player's active key profile.
The callback's published media generation must still match when it runs.
Disabling system media controls or closing the player withdraws the session
and removes its callbacks before the native window goes away.

The macOS application owns one MediaPlayer registration for all player windows.
It follows the most recently focused player when Settings or another application
takes focus. Before dispatch, it checks the window and media generation carried
by each queued remote action against the current Now Playing item.
Playback readiness, the system-media preference, player focus and native
activity or transport callbacks wake the session. Its existing refresh interval
continues while there is a published item. The last player withdraws the
registration and stops the refresh during cleanup.

On macOS, the platform adapter registers each player's AppKit Touch Bar with
the playback observer and retires it before native window cleanup. The scrubber
samples the player's existing thumbnail cache. The timeline timer publishes
sample changes after releasing the cache borrow so AppKit can redraw safely.
Now Playing uses that same cache as its artwork fallback after the current
media's timeline finishes generating or loading. It keeps a weak reference to
the cache and lets cover art or Quick Look replace the fallback when available.
Artwork cancellation signals the background worker without blocking playback
updates or window shutdown.
A touch gesture records both the current file and
open request before pausing. Its seek and resume commands also carry the media
generation through the mpv queue. The Touch Bar's end-clock mode belongs to the
saved profile and is shared by its windows independently of the controller clock.

The controller arrows and Touch Bar transport buttons read the same saved arrow
action. A held speed arrow keeps the controller visible and applies speed changes
to the media that received the press. AppKit pressure events for an active arrow
gesture reach that session before the general Force Touch action. Playlist and
seek modes use the existing transport commands.

Linux publishes one MPRIS service per application with an eligible player.
It follows the focused player while retaining the last focus when such
observations are available. Without a keyboard seat it selects the newest
ready player. The bus worker exposes a playback snapshot and queues requests.
The UI verifies the player and media generation again before dispatch. The
TrackList interface exposes up to 24 entries around the current track. Object
paths include the window and native queue entry IDs, so repeated copies of a
file have distinct identities. The service keeps its name while the selected
window loads another track. An insertion that also selects the new track uses
the entry ID returned by libmpv and waits for it to appear in the observed queue.

Manual playlist navigation applies a temporary pause. The worker associates it
with the native command reply and the media generation that starts next. It
checks the current pause-on-open preference when mpv reports playback ready
after initialization. With normal latency settings, mpv waits for its video
output to finish the first frame before that event. Audio-only entries reach
the same event. An explicit pause or play command retires the temporary pause's
automatic resume.

Single-file folder discovery belongs to the native entry returned by the open
command. Its result carries the playback generation back to the worker, where
existing filenames are excluded before the insertion is planned. Clipboard
paste and desktop drops use stable destination row IDs so unrelated inserts
or reorders do not shift the intended destination. Insertions and removals
keep the widgets for surviving rows, preserving focus and selections.

The main and compact timelines send exact percentage seeks for media with a
known duration. A pointer gesture captures the media generation for its seeks
and A/B marker changes. The event worker folds adjacent timeline requests for
that generation and leaves playback commands in order. Timestamp commands such
as Jump to Time use their own route.

File and Playlist Loop menu actions toggle the observed repeat mode. The
playlist footer cycles Off, File and Playlist. A profile's raw file-loop
shortcut still changes only the file-repeat property. Menus and playback
feedback use the same interpretation of finite counts and native repeat modes.

Callbacks and `playback_mailbox` share `Arc<Playback>` snapshots. Tracks, chapters,
playlists, audio devices and retained plugin events are shared across snapshots
until the worker replaces or updates them. `playback_mailbox` coalesces pending
updates to the UI thread.
The event-loop reservation is released when delivery begins or when the loop
discards the callback. Rendering uses the same ownership for redraw requests.
Screenshot and frame completions travel alongside the snapshot so coalescing
cannot discard their side effects. `observations` projects playback state and
catalogs into Slint. The UI observer wakes thumbnail, native transition,
subtitle discovery and material timers when a playback snapshot introduces
work for them. The playlist tag reader takes a bounded batch and notifies
subscribed windows as each lookup completes. It reads title and artist and
skips cover art and playback properties. Each player schedules a refresh of its
own visible rows and owns its audio, video, subtitle, timeline, catalog and
feedback state.
Visible playlists share one background duration reader for local files. It
uses a separate paused libmpv client with null output and file references
disabled. Closing a playlist withdraws its queued requests and cancels probes
that other windows no longer need. Both readers exit when their queues drain
and start again when there is work. The current player's observed duration
takes precedence. Metadata retains the duration of previously played streams,
including distinct URLs with different query strings.
Each player reads saved resume positions for its visible playlist in one
background job. The read uses the profile's watch-later directory and the
decoder's observed filename policy. Native save and delete replies and initial
playback restart signal a possible change to the profile's files. Playlists in
that profile refresh even when their filenames stay the same. Row ordering and
playback clock ticks reuse completed lookups. Hiding the playlist withdraws
pending reads. Clearing saved playback progress supersedes any read in flight.
On close the engine worker completes the configured save before destroying
the native client, then signals the profile so other windows can read it.
Folder discovery also supplies common series filename prefixes to the
player's playlist. The grouping rules are shared with subtitle matching.
The row keeps its full title for accessibility and folds the shared part
behind a button in the sidebar and Music Mode. Expansion follows the
playlist entry when it moves. Custom titles and tagged track names retain
their own display text.
The sidebar, layout, controls and dialogs read state from the owning window.
Rendered frames and material samples share
that lifetime. Keyboard input and both menu presentations invoke the actions
registered for that player. Plugin entry workers have a separate runtime and
send intents through the existing command path.

Quick Settings timing controls keep a pointer draft inside the delay editor.
Audio drags snap to 50 ms and subtitle drags snap to half-second ticks. Release
sends one typed command to the selected lane. A lane change discards the old
draft and cancels its drag. Keyboard and accessibility slider actions submit
directly. The text editor owns ordinary input and lets Tab and Shift-Tab
move between controls. Unhandled Control or Command shortcuts reach the
player. A reset that disappears gives focus to its slider before dispatch.

The track lists retain the focused engine ID across model changes. When it
vanishes, focus clamps its former position to the remaining rows. A page keeps
the latest focus reveal requested during scroll restoration and applies it to
the same pane after restoring its offset.

Slint has one Winit event callback per window. The player combines file drops,
focus, wheel input, trackpad gestures and shared-renderer resize events in that
callback. The framed Windows video owner registers its own event callback.

On Windows, the framed video window and its controls share one OLE drop route.
The window that receives the drop supplies its client coordinates. The route
preserves playlist hover as the pointer moves between the two windows and
queues media commands on Slint's event loop.
[Win32 drop-target interface](https://learn.microsoft.com/en-us/windows/win32/api/oleidl/nn-oleidl-idroptarget)

The player and Welcome window bind desktop drops through the same platform
registration. It attaches the correct native windows on Windows or macOS and
retains the Wayland data-device route. On X11, an Xdnd proxy receives URI lists
and plain-text references. A separate X11 connection reads direct and
incremental selections. Each offer owns a requestor window, so cancellation
retires replies before the next drag. The UI captures the playlist insertion
gap at release, including when transfer completion arrives later. Winit still
handles file events sent directly to its window.
[Xdnd protocol](https://www.freedesktop.org/specifications/xdnd/)

A video drop opens media in the player that received the event. Welcome and
the File menu use the application-level window-opening policy. This follows
the video and initial-window drop routes in the pinned IINA source.

The main sidebar and Music Mode share playlist row gestures and insertion
geometry. An active row drag retains its pointer in window coordinates, so the
drop indicator follows the rows while the view scrolls beneath a stationary
pointer. The scroll view owns the edge timer and stops it on release or
cancellation. Selection and metadata refresh update existing rows in place
when their identities and order are stable. Structural changes retire the
gesture before a release can act on a different order.

The owning player stores the active catalog tab, the Playlist and Chapters
scroll positions, and the Chapter selection. Main-window and Music Mode
views restore those values when mounted. Each file starts with a fresh chapter
cursor and scroll position. The native chapter index updates the sidebar's
playing marker in place. The menu checks the entry whose time range contains
the observed playhead and aligns start times to the final entry's width.

The Music Mode window owner supplies the panel height to the controls and
artwork during resize. Windows settles it after the sizing loop, and
AppKit reports the end of live resize. On Linux the shared presenter uses
the last changed client size after configure events settle. The resize
updates the current window. Explicit show and hide actions update the
profile through its existing preference transaction.

The compact controls have their own lifecycle. The main player's idle timer
can hide its controller while Music Mode keeps the title and timeline mounted.
Pointer tracking covers the artwork and control strip and stops at the catalog.
Keyboard focus can reveal compact actions independently. Linux projects the
same live player menu through a compact button and F10.

The compact volume button keeps its place when the slider and level expand.
The regular scroll dispatcher opens the inline readout only when an artwork
gesture resolves to volume. A mouse wheel refreshes the OSD timeout. A
phased trackpad gesture dismisses its readout when the gesture ends. The
slider continues to use the player's volume limit and wheel preference.
Entering Music Mode clears the main player's queued on-screen messages.
The saved on-screen display preference still applies to the main player.

Audio-only subtitle rendering uses the player's existing libmpv output.
The shared OpenGL presenter signals readiness after creating the render
context, and the event worker then permits subtitle-only frames. The
embedded Windows presenter starts its video output at media open. An
explicit mpv force-window choice takes precedence on both paths.

Automatic Music Mode uses the track catalog delivered to the UI. A
pending switch owns a single deadline, which a new media generation or
incompatible catalog replaces. Stable playback leaves no timer running.
The window owner retains a decoded video's pending size through the
compact-to-main handoff. It restores the main frame before applying the
saved resize policy. A replacement item clears the old request, and a
subtitle-only canvas cannot supply the main player's natural dimensions.

Each player publishes file and folder selections, paste, external drops and
URL additions in submission order. Discovery runs off the UI thread. New
additions wait for a pending file open to dispatch. Clear retires pending
additions, and a new open or Stop retires the old media request and its edits.
The worker inserts each reference directly at its native
position instead of appending and moving it in a second operation. User
additions and clears collect native replies for one edit and publish the latest
outcome for the owning player's OSD. Silent application and plugin commands
keep the same queue and editing behavior.

The filter editor, saved-filter menus and shortcuts send audio and video
edits through the requesting player's media-scoped queue. The worker waits
for each native command reply before reading the chain for the next edit.
This lets successive toggles see the result even when the observed property
has yet to update. Row edits compare the displayed chain with the current
native chain, then edit the selected occurrence by index. The worker captures
the added filter's display name and publishes feedback after the native add
or remove succeeds. A replacement media generation retires obsolete replies.
Removal rebuilds the chain and preserves separators inside retained parameter
values using the native byte-count notation. Pause and resume feedback retains
the native clock values from the observation that produced it. The subtitle
export confirms a completed save for the owning media even when optional
messages are disabled.

Playlist drags send the selected entry IDs and destination entry ID to the
engine as one request. Play Next resolves the current entry and moves the
selected entries immediately after it in playlist order. The engine plans the
ordered moves before sending them to mpv, and only selected entries move.
The sidebar and Music Mode share one selection of stable entry IDs. Keyboard
focus and the context-menu target are independent of that selection. The
footer removes the selected IDs, and the UI reads localized summary text
from formatted total and selected durations supplied by the player.
Both views expose the playlist as a list of selectable rows. The player
retains an activation key's ownership until release so a held key cannot
spill into a different control when an action moves focus.

Playlist file actions use the selection resolved by the context menu. Trash work
runs off the UI thread and groups repeated references to the same path. Successful
moves send the original entry IDs and filenames back to the engine. The engine
skips vanished or changed entries and removes the current entry last so playback
can advance to a remaining item. Reveal groups items for Explorer, Finder or
the freedesktop file-manager service.

Playback History stores an optional decoder title beside the media reference.
Late title and duration observations belong to the active decoder and its
recorded visit. Replaying the same media in another window retires the older
visit's metadata updates. The outline uses media references as selection
anchors across group changes and refreshes. Updates to progress and file
availability change the existing rows in place when their identities and
order stay the same.
Each new entry keeps the resume filename chosen by mpv at the time of playback.
Matching keys replace earlier visits and read the same saved position after a
path-policy change. Stream keys preserve the original address. History reads
saved progress through the recorded key. Native file changes invalidate
in-flight reads. The visible History window coalesces changes and requests a
background read. Opening History also reads the current files. Rows without a
key stay visible and gain one when played again. The standalone archive owns
the rows, independently of Settings. Resume positions are read from mpv and
kept in memory.
Moving a file to Trash refreshes availability and keeps its record available
for separate removal. Clear, remove and trash confirmations belong to the
History window. It holds one pending action, defaults focus to Cancel and
discards the action when the window closes. The application owns one History
window. Each live player registers a weak window reference and its command
sender. Focus changes update the playback destination, and player teardown
removes its registration.
The History owner saves the last normal window bounds and the chosen column
widths in the profile. The table clamps widths to the current viewport while
keeping the saved preferences available for larger windows. Restore keeps a
position on an attached display and leaves placement to the Wayland
compositor. Window bounds are sampled while the outline is visible and written
only when it closes or hides.
History checks selected local references on a background worker when playback
is requested. A newer History playback action or closing the outline retires
the pending check. When the files are ready, the outline finds the most
recently focused visible player, including a minimized player or Welcome.
A player opened from the outline uses the same launch path when History is
the only visible window. Closing History applies the last-window policy.

Playlist subtitle associations belong to the player session and use media
filenames. The subtitle picker verifies the playlist entry before saving its
selection. Manual matches load the next time the entry starts. Wrong Subtitle
clears the associations for the selected rows and inhibits automatic rematching
until Stop. A single-file open matches siblings in one background scan of the
media directory and configured subtitle locations. Rows can use those results
before playback. The results stay with the playlist as playback advances and
expire on replacement or a discovery settings change. An explicit playlist
scans each entry when it starts. The decoder accepts matched files only for
the generation that requested them.
The in-window context menu scrolls when its actions exceed the available
height in either player view.

The playback worker owns subtitle encoding changes. It reloads external
subtitle tracks, waits for the native replies, and reads the refreshed track
list. It restores the primary and secondary choices by external path or
embedded track ID because a native reload may assign a new ID and select
the replacement as primary. The worker waits for the selected IDs in a new
track-list observation before submitting queued cycles. This preserves the
native order and command options for forward and reverse profile bindings.
A later explicit choice supersedes queued cycles for its lane. Replacing
the media retires the pending reload and selections.

The desktop drop adapters publish the position of a playable offer to the same
scroll view. X11 samples the native cursor after Winit reports the file list.
The sidebar and Music Mode use their current scroll offsets for the cue and
for the release position. Riina captures the destination entry's identity
before it clears the hover or hides a temporarily revealed sidebar, then
prepares the offered files off the UI thread.

On Wayland, the application registers one data device per seat on Winit's
display. Winit reads the display socket and Riina dispatches its drop queue
from the same event loop. Drop payloads are read in the background, and the
compositor's surface-relative coordinates select the owning window and playlist
row. Winit keeps ownership of the display.

On macOS, AppKit routes Finder files, browser URLs and text to the Winit content
view for the player or Welcome window. A subclass adds the drag destination
methods to that view for the lifetime of its registration. AppKit supplies
the release position, and a weak route delivers the drop on Slint's event loop.
[AppKit drag destinations](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/DragandDrop/Concepts/dragdestination.html)

Each player retains its latest 16 engine notices. The status line and the
plugin snapshot expose the active notice. Clearing it leaves prior notices in
the history, and the engine logs each new notice when it occurs.

The app's named background tasks register until their work returns. Their
owners retain join handles where teardown depends on the work completing.
The application logs tasks still running when the main event loop exits.
UI deliveries that depend on a live window use a weak component handle.

`UiSession` releases its player ownership before the native presentation drops.
Windows native playback joins its worker before the target window can disappear.
On the shared GL route, the renderer retains player ownership until rendering
teardown frees the GL surface. The worker unregisters its wakeup callback, destroys
the native engine and only then releases the callback context.

On Windows, one application owns the event loop and all native players opened
from its windows. Each player owns its framed window, controls, timers and
playback session. Each new frame receives its own controls owner handle. Close
releases one player. Quit releases all remaining players before ending the
Windows event loop. A separately launched Riina process has its own application
owner.

Software Update is an application service shared by its players. Either menu
opens the same window and joins an in-flight scheduled check. On macOS the
service starts with the application and the idle app menu can check on demand.
The General settings edit one update policy per profile. Independent app
processes share the policy and claim a due check under a file lock. A worker retrieves public
release metadata and compares versions. A scheduled check opens the window
only when a newer release is available. The window can open the project's
release page. The Windows update adapter owns the installer offer, cancellation
and Setup actions. For a registered installation, its download worker stages
the exact installer named by the release and verifies the published size and
SHA-256. Opening Setup requires a second user action. The adapter reinspects
the installation and keeps the verified file read-locked through the handoff
to the shell.

Playback power also belongs to the application. The first actively playing
session in creation order determines the request. General settings can
release it or let audio and Music Mode keep only the system awake. Player
close releases that session's registration before decoder teardown. Windows
uses process-owned power requests, macOS uses a Foundation activity, and
Linux uses desktop idle inhibitors and the logind idle lock when available.
The Linux requests run off the window thread.

Pause on sleep follows the native suspend notification and the current
preference of each player. Windows registers the framed playback owner, or
the shared-renderer window, for suspend events. The registration ends with
that session. Linux owns one system-bus logind subscription per application
and routes `PrepareForSleep(true)` to its open players on the UI thread.
macOS uses its existing Workspace notification and shares the pause policy.
Idle and shutting-down engines receive no pause request. [Windows suspend notifications](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-registersuspendresumenotification) · [logind suspend signals](https://www.freedesktop.org/software/systemd/man/latest/org.freedesktop.login1.html#Signals)

Media discovery chooses destination players from the original selection before
enumerating folders or filtering files. A folder stays with its assigned player,
and a standalone playlist goes to mpv as a single reference. The Windows and
shared player managers use each player's current media for duplicate checks on
separate opens. An explicit grouped selection opens together even when only one
playable item remains after filtering. The player cancels its active
selection scan when a new request replaces it or the session closes. macOS
document opens keep the original duplicate reference through asynchronous
preparation and resolve the current destination when the files are ready.
The native worker reports an opened group after playback restarts on the
lead file and the playlist additions settle. Playback observations update the
feedback baseline quietly during loading and for 200 ms after the first
restart. The group message can then appear without an automatic track
selection replacing it.

Playback History sends selected rows to the current player or to one new player
according to the chosen action. These actions open an explicit playlist. A
double-click opens one entry using the saved window and resume preferences and
shows the stream progress dialog when the entry is a supported stream.
mpv applies the saved position at load time. The History window reads saved
positions for display.
Play in New Window in the playlist menu uses
the same explicit playlist open. The engine retains the selection policy through
navigation and appends. Same-folder media discovery skips it until a regular
media open replaces it.

### Plugin mpv operations

The player attaches its command sender before the plugin host starts entry
scripts. Getters and setters send typed requests to that player's mpv worker.
The worker copies read replies while their native event owns the data. It
holds write values until libmpv has copied them. Only the plugin worker waits
for a reply, within the deadline of its callback. Property replies stay out
of the public playback event stream and global plugin entries have no
player engine.

Binary node properties arrive as arrays of unsigned numbers, including when
the bytes are nested inside other native values. Each reply shares a byte
copy budget before those values are expanded into JavaScript numbers.

Playback options follow the pinned IINA property inventory. Transport
options require a reviewed network grant. External file lists require a
filesystem grant, and URL entries also use the plugin's allowed domains.
The worker clamps volume writes to the range selected in Preferences and
updates that range in the same queue as property requests.

Plugin commands use the same queue and wait for native replies. Standard
seek, Stop and playlist actions use existing player intents. Supported
native commands carry separate arguments through to libmpv. Media loading
and external tracks use reviewed file and network access. Quit is routed
to the application's shutdown handler so the player can release its
rendering and plugin resources.

Initial media opens after enabled plugin entries have initialized. The UI can
accept another open request during startup and honors the newer request.
Browser activation passes one validated link through a profile-scoped local
endpoint to the application event loop. Windows restricts its named pipe to
the current logon session. Unix uses a private socket with peer checks.
The application resolves the most recently active player before choosing
append, replace or a new player. Local link delivery shares the ordinary
media generation and window presentation paths. [Named Pipe Security](https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipe-security-and-access-rights)

Hooks register on the player's native client. Each plugin realm owns its
registrations and runs the callbacks on its JavaScript worker. The native
worker keeps receiving events and answering property requests while a hook
awaits its continuation. A synchronous callback completes on return. An async
callback calls `next()` when ready. A failure, released registration, rejected
delivery or 15-second deadline releases a pending native hook. The worker
consumes each native continuation once and reuses vacant slots when that
preserves libmpv's registration order.

### Plugin subtitle providers

The plugin host catalogs the providers declared by enabled manifests. Menus and
Settings use stable provider IDs so changing the plugin list keeps the selected
provider associated with its package. Each search runs in the owning player
realm. The JavaScript objects stay in that realm. Only the display text and
result indices reach the chooser.

Downloads use the same realm that produced the selected results. The plugin
file scope resolves returned paths, and the download worker copies subtitle
files into the application cache. Built-in and plugin requests carry the
player's open request and native media generation. The chooser retires old
callbacks on cancellation, provider changes and player close. The worker
checks the generation again before attaching downloads.

Export uses the owning player's native Save dialog with the provider's filename.
The selected cache file is read on a worker and the destination is replaced
after the complete byte stream reaches disk.

### Plugin WebSocket servers

Each realm may start one WebSocket listener after permission review. A network
thread owns the listener and its connections on a single event loop. It sends
typed lifecycle, message and send-result events to the realm. Only the realm
invokes JavaScript callbacks and settles `sendText` promises. The realm's
memory limit covers incoming message bytes that a plugin retains. Text and
typed arrays are created on request.

Server replacement retires the previous listener and all its connections.
Each event identifies its listener generation so an old socket cannot update a
replacement server. Same-port replacement retains the listening endpoint while
closing the old connections so the new server can start immediately. The
listener shuts down and joins its network thread when
the realm closes, including during plugin reload. Connections, message sizes
and send queues are bounded. A busy realm pauses socket reads until delivery
can resume.

### Plugin runtimes and player ownership

The application owns one global plugin host. Each enabled package can register
one global realm, which survives closing or opening a player. Player entries
run in their own playback sessions. The application host distributes package
changes to those sessions and waits for their acknowledgements before an
installer can replace an active package. Closing the application retires the
global entries.

The mpv worker copies values for requested property changes into the same
sequenced history as raw engine events. A property callback precedes the
generic notification for the same native change. Snapshots retain up to 256
records and 4 MiB of encoded property values so UI coalescing preserves
intermediate notifications within those limits. The plugin host uses each
sequence once. Startup entries read the latest available snapshot while the
host retains the queued notifications for delivery after registration.
Callbacks preserve the distinction between no argument and a JSON null.

The same coordinator delivers global menu actions from players and the
application. The application attaches a windowless presenter on macOS.
That presenter opts into its own menu snapshot and tracks changes to global
standalone windows. Windows and Linux use the player routes without that
extra snapshot or change counter. The platform adapter also supplies AppKit
close notifications and Linux standalone window shortcut ownership.

On macOS, startup loads enabled global entries even if the selected launch
action leaves the app without a player. The coordinator wakes the UI when a
global entry requests a player or changes its standalone window, menu or
developer registration. The AppKit application menu reads published global
contributions and defers menu mutations until tracking ends. An action carries
the generation of its owning realm so a selection from before reload cannot
reach a replacement entry. The Developer Tool uses the same global console
whether or not a player is open, and retires it when its realm stops.
The application checks WebView frames and console output while the tools are
visible. Prompts and file pickers can use the application when no visible
player can host them. Plugin tools and application dialogs participate in the
same quit policy as other app-owned tools. The platform presenter chooses
the owner. A notice occupies the same dialog slot as a prompt or picker and
is cancelled when the owning plugin stops.

Settings and both plugin hosts share package discovery. Links and development
folders keep their project location, with permissions and preferences stored
in the Riina profile. If two packages declare the same identifier, discovery
assigns the additional copy an identity derived from its installed filename
while both copies are present.

Uninstall waits for the application and player hosts to retire the plugin
before moving its installed folder out of the catalog. If saving the disabled
state fails, Riina restores the package and its grants. The package lock
serializes installation, replacement, and removal while the profile keeps the
plugin's data and preferences.

A controller can reserve a player ID and ask the application to open an
independent playback session. The request uses the same reviewed file and
network access rules as the player's open API. The player starts its owner's
entry by default. A controller can opt into the full enabled plugin set.
The controller owns the player until either the window closes or the
controller retires. Reusing a numeric ID after reload creates a new ownership
token and cannot address the retired session.

Messages from a player carry its internal label to the package's controller.
A controller's numeric target selects one of its players and a null target
selects all of them. A string target resolves an ordinary player's internal
label. The user label supplied at creation is returned by the child API.
Messages sent before a new player registers stay in a bounded queue until
its entry can receive them. The application currently allows 16 live players
per controller and 32 pending launches across controllers.

### Plugin application menus

The player keeps global and player contributions grouped by their package.
Menu rows carry their depth and role up to the compiled Slint menu provider.
The provider restores the subtree before Slint supplies entries to the native
menu, the in-window menu or its shortcut dispatcher. Contributions after the
fifth top-level row of a package go into More. Leaf entries retain the item
identities that dispatch actions through the player to their owning entry.

The provider extends the pinned Slint runtime interface in the UI crate.
Slint continues to own presentation and menu lifetime on every platform.
The provider refreshes the contribution tree when Slint assigns a new root
identity after a binding change.

### Playlist context menus

The sidebar and Music Mode share one playlist footer. Add and Sort use the
platform playlist menu adapter and remain attached to their opening surface.
The toolbar menus require only the surface identity. Row menus also keep the
selection and identities needed for row and plugin commands. The in-player
menu restores its previous focus on close.

The player captures the playlist entry identities, filenames and selected
indices when opening a menu in the sidebar or Music Mode. File and URL action
availability comes from that selection. Copy URLs preserves the playlist
order of supported stream URLs. The browser launcher receives eligible HTTP
and HTTPS URLs. Both commands apply when the selection also contains files.

The player builds localized titles from the displayed filename or selection
count. It supplies section headings as distinct entries without actions.
AppKit uses native section headers on macOS 14 and later. Older AppKit menus,
Win32 and GTK use disabled labels. The in-window popup exposes headings as
static text and skips them during keyboard traversal.

Built-in actions share the presentation path for plugin contributions. A click
below the rows passes an empty selection to each builder. The plugin host
starts builders in the player realms and gathers their replies under one
deadline. Each realm owns its callbacks and retains the latest popup's items
until an action is chosen or another request supersedes them. The player
discards replies and activations when the playlist, selection or originating
surface changes. Win32, AppKit and GTK track native popup menus. Wayland and
the native fallback use the scrollable in-window presenter.

### Subtitle chooser menus

The Subtitles panel and on-screen controller use the contextual menu
presenters. The controller also lists the engine's loaded subtitle tracks.
Each player allows one manual folder scan at a time. It shares candidate
discovery with subtitle auto-loading and delivers the result only while the
same media, open request, preferences and source control still apply.
Selections go through the owning player's media-scoped command channel.

### Plugin input

Player realms register normalized keys and video-surface mouse events. Each
player maintains weak subscriptions to its own realms and publishes a snapshot
when installing the initial profile and applying profile changes. Publication
completes before a startup entry can read the profile. The UI calls the matching
higher-priority key listeners before the normal binding and the lower-priority
listeners afterward. A truthy reply stops propagation. Mouse-up listeners run
before the configured click action.

The existing window event filter captures macOS mouse data when AppKit's
current event matches the player and its Winit event. Button transitions carry
the native click count, window coordinates and pressure. Drag callbacks carry
the native pointer coordinates and pressure with the associated press count.
Separate move and button samples keep a buffered drag from taking data from
the following release. The video surface remains the input owner. Focus loss
and pointer cancellation discard the player's previous gesture and click state.
When another control handles a mouse release, the next native pointer event
retires that button's gesture. Slint can still deliver a buffered final drag
before the release.
Other platforms use Slint coordinates and the player's click recognition.
[AppKit current event](https://developer.apple.com/documentation/appkit/nsapplication/currentevent)
[AppKit pointer pressure](https://developer.apple.com/documentation/appkit/nsevent/pressure)

The UI gives both priority groups one 16 ms deadline. Input with no matching
listener proceeds immediately. Requests that expire before execution are
discarded, and a reply that arrives after the player has moved on has no effect.
Text editors, dialogs, crop controls and reserved native menus keep their own
input paths.

The installed keyboard profile resolves conflicts for plugin menu shortcuts
using the same platform key normalization as playback. The player retains the
requested shortcut so it can restore the accelerator after a profile change.
Both menu presenters receive the resolved shortcut and the conflict count.
Desktop media adapters send transport buttons through the player's installed
profile dispatcher. The native idle menu projects its shortcuts from the same
ordered mappings.

Application directives share player menu callbacks for file and URL opening,
playlist and subtitle exports, file-manager reveal and both reflection axes.
The keyboard path consumes repeats of both file-removal directives. The shared
file worker captures the native playlist identity and filename before removing
a local file. Successful removals submit those identities to libmpv and leave
failed or superseded entries alone.

The Settings shortcut editor keeps the recording, generated command and note
in its draft. The portable builder emits one profile command from the chosen
action or property inputs. The user can edit the resulting command through
the same raw field used for imported profiles.
The application publishes committed shortcut settings to players using the
same preferences path. File edits reload the installed mappings for that set
after the write succeeds. The editor captures the selected profile and file
contents on open. A stable lock serializes profile writes and rejects a
replacement based on older contents. System media defaults belong to runtime
resolution and stay out of custom mapping files unless explicitly assigned.

### Plugin sidebar surfaces

Each player owns its plugin sidebar documents and selects one tab for display.
The sidebar shares the confined package protocol with the overlay and standalone
windows. It tags each document and routes browser messages to its owning plugin
instance, so an old page cannot deliver messages after that plugin is reloaded.
Windows, AppKit and X11 attach native browser children. On Wayland the selected
WebKitGTK document supplies a transparent image inside the shared sidebar. Its
pointer and wheel input uses sidebar-local coordinates. While the browser has
focus, WebKit handles text editing and the GTK shortcut adapter routes reserved
window actions to the owning player. Closing or switching tabs releases browser
focus and pending captures while retaining the document.

### Plugin file operations

Player and global plugin entries use the same filesystem scope. Private paths
are resolved inside the package's data and temporary directories. Current media
paths require the reviewed filesystem grant. Scoped track aliases resolve through
the player's observed external tracks. Listing paths can be appended directly to
the requested directory alias. Recursive listings retain Riina's entry limit.

Text writes and downloads stage the complete bytes beside the destination before
publishing. Private writes replace the name while readers retain the version they
opened. External writes publish only when the name is still vacant. A move uses
the native no-replace operation. Across volumes, Riina reserves the original
entry beside its source and copies to a temporary directory on the destination
volume. The finished entry is placed with the same no-replace operation. If
copying or publication fails, the original returns to its starting name unless
another writer has claimed it. In that case the error identifies the retained
source. If source cleanup fails after publication, the error identifies any
remaining original data. Directory copies reject mounts and unsupported entries.
Moves and Trash preserve the final external directory entry, including when it
points to a symbolic link. Common text encodings use the shared strict decoder.
Additional legacy encodings use the runtime's native converter. Reveal uses the
desktop file manager's item selection for both individual files and groups.
Windows converts a canonical drive or share path to a Shell path only when
normal path resolution yields the same entry. Linux uses the file manager's
D-Bus selection API and falls back to opening the containing folder.
Player and Settings actions dispatch selection on a worker so the shell
request can complete while the window continues responding.

[Windows rename semantics](https://learn.microsoft.com/windows-hardware/drivers/ddi/ntifs/ns-ntifs-_file_rename_information)
[Windows volume moves](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-movefileexw)
[Windows Explorer selection](https://learn.microsoft.com/en-us/windows/win32/api/shlobj_core/nf-shlobj_core-shopenfolderandselectitems)
[FileManager1 item selection](https://wiki.freedesktop.org/www/Specifications/file-manager-interface/)

### Plugin XML-RPC calls

XML-RPC uses the shared HTTP worker and checks the same reviewed domains.
Dates and byte sequences cross the worker boundary with type information.
Ordinary structs preserve their member names, including names used by the
internal encoding. A protocol fault rejects the plugin promise without a
value. HTTP and response errors reject with the method's status, reason and
description.

### Plugin video overlays

The player owns each overlay WebView. On native Windows it sits in the video
host. The shared Windows and AppKit players attach their overlays within the
video viewport. Linux renders each WebKitGTK view offscreen and composites
its premultiplied pixels above the video and below player controls. Each
visible view has at most one snapshot pending. Static pages are sampled less
often, and unchanged pixels keep the existing Slint image. The shared presenters
hide overlays during crop edits, modal dialogs and Music Mode while retaining
the documents.

Overlays and standalone plugin windows share the confined page protocol. The
page origin isolates browser storage and a document ID ties messages to the
active page. Overlay replies also carry the owning plugin realm so a retired
WebView cannot send a message to a replacement entry. Hit testing uses the last
DOM result for the current pointer position. Moving the pointer, changing the
document or hiding the view invalidates the result. On Linux the frontmost
clickable document receives the press. Its release belongs to the same document
even when the pointer moves outside the element.

### Standalone plugin windows

Each player or global entry owns one standalone window state and a distinct
browser origin. The UI reads that state and creates a native window on the
first open. Close hides the WebView so reopening retains the document. Each
open request can raise the native window even when a close and a reopen arrive
before the same UI update. Frame requests record each supplied axis so repeated
values can restore a manually moved or resized window while preserving other
axes. AppKit applies those changes to the live window frame in screen points.

When an entry retires, the UI drops its WebView and native window. The
application owns the global standalone and developer windows. On macOS, a
pending global window open participates in application lifetime while the
native shell is being created. Each player owns its own plugin windows and
queues presenter work separately from the plugin timer. The Linux global WebView
resolves application shortcuts through the current player. Global utility
dialogs use the focused player when one is available.

Package assets use a confined custom protocol. Simple-mode markup and style
can be set while the window is hidden or its page is loading. A document ID
ties bounded messages to the page that sent them. Loading another document
clears pending messages and listeners. IPC callbacks also check the owning
entry's lifetime.

Windows and AppKit use a Slint window with a native WebView. Linux uses a
GTK top-level window and WebKitGTK, with event delivery on the player UI
thread. The compositor controls final placement under Wayland.

### Plugin utilities

The player and global realms share a utility API. Filename lookup and path
resolution follow the filesystem grant and reuse the plugin's existing path
scope. Helper execution also requires the reviewed `process-execution` grant.
A helper receives an argument vector and a limited environment. Its supervisor
reads stdout and stderr independently and delivers output and completion to
the owning realm. Each realm allows two active helpers with a two-minute
deadline and 1 MiB per output stream. Closing the realm cancels those jobs.
Native helpers run with the user's account permissions.

The `open-external` grant permits HTTP(S) links and scoped folders to open in
other applications. Local file associations also require `process-execution`,
since an association can execute native code. Utility credentials are stored
by the platform credential service under the plugin, service and account
identity. The player permits one plugin picker at a time. Picker results
return only to the requesting realm, and a stale selection is discarded.

Confirmations and text prompts share the picker slot. The JavaScript call
waits in its realm while the UI event loop keeps running. User wait time is
excluded from the remaining JavaScript turn and startup budgets. The host
signals dialog cancellation before joining its worker on shutdown. A stale
prompt closes when its realm or player retires.

## Preferences

Plugin preferences use a separate store per installed identifier and profile.
Player and global realms share pending values, and each realm uses the
package's manifest defaults when a key has no override. Explicit sync merges
pending keys into the latest saved values while holding the file lock, then
uses the same atomic replacement as application settings.
An IINA property list supplies the saved values when the JSON store is absent.
Riina matches the installed identifier against the filename on disk and keeps
the original property list when a sync saves the merged values as JSON.

An installed package can declare a local `preferencesPage`. Settings opens it
in a native WebView with an isolated origin and confined package resources. Its
bridge reads the same defaults and pending values as the plugin's realms.
Form changes update that store. Closing the page, leaving Plugins, or closing
the owning Settings window saves the pending edits and releases the WebView.
The WebView uses the same native window shell as standalone plugin windows.

Plugin information in Settings shows the manifest metadata, permissions, source
and author contacts without starting the package. A declared `helpPage` can
open a confined package document or an HTTP(S) page. Help uses a private native
WebView with no plugin API, and its lifetime follows the Settings window.
GitHub update checks initiated from the information panel use the existing
package staging and permission-review flow.

Finder groups containing an IINA plugin archive route the first package to
Plugins in Settings. The local picker, Finder and GitHub installs stage
packages on a worker, then show a review dialog under the Settings window.
The reviewed package holds the package lock through placement, grant and
enable changes, and rollback. Closing Settings cancels an outstanding
review. An accepted installation can finish in the background. A supported
package with an available runtime starts after the user approves the review.

`preference_load` reads and validates the startup settings. Recovery preserves a
copy of invalid input and loads recoverable fields. `PreferenceModel` holds the
committed settings on the UI thread and delegates persistence to
`preference_store::Writer`. Section bindings read and validate their controls and
project the saved settings into the Settings form and `PlayerPreferences`.
The Settings form initializes from the saved profile before the player attaches
its live state and actions. General playback preferences and screenshot controls
use the same form projection when they refresh from a player. Screenshot edits
preserve formats and templates outside the form and project to a player after
the save succeeds. Update and playback power controls save through application
services, with failures reported by the Settings owner.
Settings search builds a localized index when the window connects. Each result
carries an index into the original actions, so keyboard and pointer activation
share the same disclosure and scroll targets. General and Interface read the
live layout after expanding a disclosure. Video search shares the rendered
card positions, including the offsets introduced by Live Text or the macOS
display profile control. Search checks feature availability on each query
because text recognition can become available after the window opens.
Subtitles search opens the relevant disclosure, and Network and Control
search scroll the matching groups into view. The localizable subtitle
encoding menu uses the portable ordered encoding table for the mpv values.
Key Bindings, Advanced and Utilities search share the scroll positions of
their visible controls. The Plugins page scrolls the entire installed list
while its modal dialogs remain attached to the Settings window. Plugin
information and permission reviews use translations from that window.
Audio device models keep the recorded name and description separate from the
display caption, including when the last selected device is disconnected.
The controller clock reads libmpv's elapsed, duration, remaining and scaled
remaining values. Its precision changes only the text. The slider still uses
the original clock values. A committed controller option refreshes players
and open Settings forms using the same profile, including paused players.
Clicking the right clock and selecting precision from either clock's menu
use the same persisted profile values as Settings.
The idle macOS AppKit menu opens an application-owned Settings form from the
current profile. It saves validated edits under the same profile lock as
player sessions. The store merges differences between successive form values,
which preserves concurrent changes to other fields and values rounded for
display. The form uses the application plugin host for package changes and the
shared recent-file and history services for Utilities actions. The app menu
routes to the current player when one is configured, including behind Welcome.
The idle form can remain visible when playback starts. Ordinary edits in that
form change the profile used by subsequent players.
At shutdown, `session_preferences::SessionState` saves runtime settings over
the model's committed settings. Its resume write checks the profile's current
resume revision under the preference lock. Clearing history advances the
revision so a player that was already open cannot restore the cleared entry
at shutdown.

`RecentActivity` owns each profile's Open Recent list and Welcome resume
projection at application scope. Player menus, Welcome and plugin snapshots
use the same document list. A local path and its file URL share a document
identity. The newest entry retains the reference it was opened with. Welcome
preserves selection across those aliases and changes from other players. Resume
uses its own reference and time. A file already shown as Resume is omitted from the
document list, including when it was opened through a file URL or symlink.
The idle macOS File menu reads the profile list and sends opens and clears
through the shared services. Manual groups update the profile in one
transaction. Windows registers local files with the Shell and macOS registers
files and folders through AppKit. The default-off macOS recovery stores local
bookmarks as an application preference and restores them on launch if AppKit's
list is empty. It resolves bookmarks without opening dialogs or mounting disks.
Clearing recents deletes the backup along with the native list. Linux updates
the desktop XBEL store and clears only Riina's registrations, keeping entries
registered by other applications.
Open Recent can clear independently.
Settings Utilities also clears the shared Playback History archive and the
resume entries. The separate History window's Clear action edits the archive.
The Settings window owns Utilities confirmations and discards them when it
closes. A confirmed action calls its profile service once. Acknowledgements
stay on the owning window. Cache sizes refresh on entry, reopening and clearing.
OpenSubtitles and Assrt credentials load directly from the system credential
store. Profile recovery only retains the parsed settings and backup location.
See [Add a preference](recipes/add-a-preference.md) for settings changes.

The application Settings adapter owns the macOS profile form, the reusable
subtitle font chooser, and the choice of destination for plugin archives.
The chooser routes acceptance to the invoking player or the Settings form.
The player writes the runtime font without changing the profile. Settings
edits save the default and distribute it to players on the same profile.
Settings attaches the chooser as an AppKit sheet or an owned modal window
on Windows. Linux retains the separate chooser and blocks the form's input
until it closes. A player Settings request
can raise an already visible application form. Otherwise the player owns its
own form. Both build their controls from the same profile and apply edits
through the same Settings store.

The Log Viewer is application-owned. The Window menu and Advanced Settings
open the same window and share its filters and selection. On macOS the idle
Window menu can also open it. The viewer reads the
profile's diagnostic and optional mpv log in bounded batches only while it is
open. Each file keeps its original order and records from the two clocks are
combined in read order. Unchanged polls keep the current list and selection.
Export captures the chosen records before opening the viewer's save dialog.

About shares one window across player menus and the idle macOS menu. When
opened before playback, it reads the installed libmpv and FFmpeg version
properties on a background thread. The query uses a short-lived client with
null outputs and loads no media or user configuration. The platform adapter
holds the result and a weak About window. Opening About from a player clears
that window reference, so the idle probe cannot overwrite the player's versions.

The application lifecycle adapter determines which tools can delay last-player
shutdown and closes the appropriate tools on Quit. History and the filter editor
can outlive a player on every desktop. The idle AppKit menu also retains About,
Log Viewer, Open URL, Software Update, Release Highlights, Settings and plugin
tools. Each tool owns its own visibility, cleanup and close notification.

The Window menu adapter owns a shared catalog on Windows and Linux. Each
session supplies the display it owns and retires its registration before native
teardown. The catalog keeps minimized players and omits hidden playback frames
that have no visible Welcome surface. It samples window-manager state while
players exist and publishes menu rows only when titles or membership change.
Bring All to Front on X11 uses the visible surfaces and retains the initiating
player in front. macOS registers Window, Help and Services with AppKit as the
application switches between menu bars. Close, Minimize, Zoom and Bring All to
Front follow the native responder chain. Both menu bars use the same Edit menu.
Native responders, including browser editors, handle their own edits. The
adapter for Slint resolves the focused text item in the key window and uses that
item's clipboard, selection and undo history. It keeps only weak window handles.

Wayland window selection uses the guest queue on Winit's display connection.
The selected player stays weak while the compositor returns an activation
token. The request uses the latest input serial for the menu's surface and
seat. The guest queue submits the request after dispatching those input events.
A newer selection replaces a pending target.

Release Highlights and About each have one application-owned window. Highlights
reads notes bundled for the package version and opens the browser only through
its release link. The macOS Help menu keeps the guide, support links and the
debug report action available before a player opens. AppKit supplies Help search
for both menu states. A profile lock coordinates automatic launch across
processes.
The version marker is created after the guide opens. About follows the appearance
of the player that opened it and stays usable when that player closes.

Open URL is application-owned and receives the initiating player from the File
menu or Welcome. Its normal and alternate actions use the opening preference
to select a current or new player. The window keeps the address for retry when a
load fails. The worker associates each attempt with mpv's playlist entry ID,
including the successor entry for a playlist redirect. A replacement invalidates
the pending attempt before cancellation can act on the new file.

Network-first startup groups and immediate browser opens use the same stream
attempt. A group keeps the prepared file order and submits the leading load and
playlist tail through one native command plan. Discovery and the first load
share a request generation so a replacement can invalidate them together.
Browser enqueue only adds the item to the playlist. A retry keeps the original
external playback options or the grouped playlist tail. The shared window
displays the latest request. Starting a stream in another player leaves earlier
players running. Native completion is tied to the leading load command's reply,
which can precede commands that apply browser playback settings.

For HTTP authentication, the platform credential service stores an opted-in
account under the scheme, host and port. The worker applies a credentialed
address at mpv's load hook. The playlist, displayed title and watched transport
properties use the public address. [Pinned mpv command contract](https://github.com/mpv-player/mpv/blob/36abaa32d00a7229ee206aae12dc0e97e7962dca/DOCS/man/input.rst)

The playback snapshot carries native cache fill, forward-buffered bytes and
input rate. The stream feedback card uses the active playlist entry's identity
and the observed loading, cache pause or seek state. It reads the two display
preferences directly from the player so they apply during an ongoing wait.
Changing a display preference leaves the native cache settings in place.
Cache fill is shown only while the engine is waiting for its cache. The card
uses the player's OSD material and stops animating when hidden or when
animations are disabled.

The Help menu's debug report takes one snapshot of each live player in creation
order. Each session registers its playback store and retires the registration
when it closes. The application writes the snapshot on a background thread.
The initiating window shows the result. The report strips user information
from standalone URLs in the title, playlist and external-track fields.

The Inspector uses the player focus events already dispatched by each window's
platform adapter. Its shared window follows the newest live player and uses
cached playback and track observations plus read-only libmpv requests. Only a
visible Inspector starts a sample, with one request in flight. Changing the
player, media generation or active tab invalidates the request before it can
update the view. The profile's watch list uses the preference store's locked
read and update path.

## Platform boundary

The Windows native presentation owns the video target used by the D3D11 route.
The platform startup adapter selects the presentation, configures the shared
backend and owns application menu integration. The shared GL renderer uses
the current Slint context. Video textures and the
blur pass use the same framebuffer binding operations. Window focus, dialog
modality, clipboard access, window document identity, file associations, and OS
media integration belong to platform services. `SessionExtras` keeps each
platform's input and auxiliary presentation alive through decoder shutdown.

When libmpv sends its shutdown event, the worker stops accepting playback
requests and releases pending replies before notifying the UI. The UI uses
the application Quit handler so peer players and auxiliary windows close
with the initiating player. The worker keeps its handle until the last owner
releases it. A shared OpenGL surface frees its render context before that
release.

New Window and separate media opens use the player manager in the running
application. An attached player routes prepared media, duplicate identification
and any resume position to that manager. Beginning close retires the window's
ability to forward requests, including those queued during media discovery.
Windows computes monitor placement from the owning window in the manager.

The application's Stop closes the playback frame after the native reply,
idle notification and empty playlist arrive. A Stop sent directly to mpv
closes the frame once the worker confirms that the loaded file's generation
is still idle. The engine retains the queue if the command requested it.
Natural EOF follows the same close path once mpv goes idle. The worker clears
the exhausted playlist only for that file generation.
The manager rechecks the request before closing so a new selection can take
over. When mpv keeps the final frame active, the playback window stays open.
An open dialog defers the close. Dismissing it rechecks the latest native
playback state even when the engine has stopped sending updates.
An unreadable file follows this close path only when the native core is idle
before completing the open. A playable successor owns the next generation.
Open URL cancellation identifies the pending stream by its attempt, even before
mpv assigns a file generation. The manager releases that player once the engine
confirms it is idle. The Open URL window keeps the draft and can retry with a
new player. It also keeps a matching failed stream available for retry. Other
failures use a separate alert. The alert and Open URL window keep the application
alive until dismissal or retry.
An unused player or a retained stopped player with an empty queue can handle
an ordinary open. The manager reserves it before revealing its frame.
A retained player stays out of window switching and History destinations.

macOS can retain a stopped player while the idle app menu is active. Windows
and Linux retain stopped players alongside other playback windows. They
release those sessions when the last player closes and apply the existing
Welcome or quit policy. Fullscreen, Music Mode, Picture in Picture and
sessions with startup-specific engine options use their normal teardown.
Plugin-managed windows keep their own lifetime.
The File picker resolves its destination from the current window preference.
Separate selections prepare independently of the caller's media request and
pass through the manager's duplicate check before reserving an idle session.

macOS registers the Open Documents, Reopen, Quit and URL Apple events alongside
Winit's application delegate. Finder groups short bursts of documents before
opening them. Explicit playback requests bypass the deferred default launch
action. The shared manager prepares windowless Finder selections off the UI
thread using the saved grouping and window preferences. An AppKit Open panel
can choose files and folders in one selection, as a sheet for an existing
player or as an application panel while idle. Playback History shares its
archive and window with the idle application menu. About, Log Viewer, History,
Open URL, Software Update, Release Highlights and the audio and video filter
editors can outlive the last player. The platform startup registration owns
the idle AppKit menu and event loop. The shared player manager restores that
menu while the application has no visible player.

An accepted Finder event holds the application open through grouping and file
preparation. The Open panel holds it until the selection is handed off or the
picker is dismissed. Closing the last visible player defers the automatic quit
decision until these requests and any remaining tools have finished. A new
player replaces the deferred close. Closing an idle tool or a hidden plugin
player leaves the native application available.

Windows and Linux keep the event loop running while resolving the last close.
When automatic quit is disabled, closing the last playback window returns to
Welcome in the same process. Closing Welcome explicitly ends the session once
the remaining tools close. The pending decision retains the closing window's
role while History or a filter editor is open. A new player clears that decision.
The idle Playback, Video, Audio and Subtitles sections read the generated
player menu catalog without showing a player or starting a decoder. Their
profile shortcuts, static choices and saved filter names use the same models
as playback, including translated menu labels. The hidden catalog supplies
stable keys for menu families and the commands available while idle. The
AppKit dispatcher uses those keys to open the screenshot folder or the two
filter editors. A similarly named saved preset retains its own menu entry.

The application, File, Edit, Plugin, Window and Help labels come from a shared
Slint catalog. AppKit uses a snapshot of those labels to build the idle menus and
identify the generated player menus. The idle owner retains its submenu handles
and identifies the mutable File items by tag. The native Plugin observers use
the localized caption to resolve the current platform menu.

The Windows owner formats its informational rows through the same localized
messages as the shared menu. It refreshes volume, speed and delays in place
as each native submenu opens, preserving the menu and video client's bounds.

The Linux player's title strip uses the shared in-client popup presenter.
At regular widths a title opens its menu directly, and moving the
pointer to another title switches menus. When the localized titles would
overflow, a Menu button opens the full list of families. Both layouts scroll
long lists, support Home and End, and add a Back row when a submenu overlaps
its parent. The generated bar owns shortcuts, and resizing dismisses the popup.

The application owns one audio and one video filter editor for the profile.
Both read saved presets before playback and follow the last active player when
a playback session is available. Changes to the active filter chain are checked
against the displayed player, media generation and filter list before positional
edits are sent. Preset edits merge into the latest profile under its write lock,
and every registered player's menu and keyboard shortcuts use the shared saved
list. Windows and Linux defer the last-player shutdown decision while a filter
editor is visible.
[AppKit application event handling](https://developer.apple.com/documentation/appkit/nsapplication)
[AppKit recent documents](https://developer.apple.com/documentation/appkit/nsdocumentcontroller)
[AppKit Window menu](https://developer.apple.com/documentation/appkit/nsapplication/windowsmenu)
[AppKit Help menu](https://developer.apple.com/documentation/appkit/nsapplication/helpmenu)

The player keeps pending fullscreen intent separate from the observed
presentation. Windows confirms through its native owner. macOS uses AppKit
window notifications, Wayland uses compositor configure state, and X11 uses
the window manager's EWMH state. The platform fullscreen adapter owns these
observations and uses the player's existing Winit event filter. The shared
controller applies playback preferences and exits Picture in Picture on
confirmed transitions. Music Mode waits for the fullscreen exit before
resizing the window. Its resize adapter uses drag completion on Windows and
macOS, and settles after the configure stream goes quiet on Linux.

macOS also offers legacy fullscreen as a default-off preference. Winit's
simple fullscreen keeps the same AppKit window in its current Space. The
adapter restores the original frame and window attributes and keeps video
below a camera housing when the display reports one. The preferred mode
applies on the next entry. The mode already entered determines how it exits.
An application-wide count preserves Dock and menu-bar settings while any
legacy fullscreen window remains. Closing a player releases its fullscreen
and blackout resources before renderer teardown.
[Winit simple fullscreen](https://docs.rs/winit/0.30.13/winit/platform/macos/trait.WindowExtMacOS.html#tymethod.set_simple_fullscreen)

A confirmed fullscreen change refreshes the paused frame and closes an active
crop or delogo editor before applying the window's playback preference. On
Wayland, the player can repeat the native fullscreen request when both axes
remain substantially smaller than the output. It waits for the undersized frame
to be presented before requesting the same output again. If the window remains
small, the adapter can leave and reenter native fullscreen once. Each leg has
a deadline. A player request to change modes cancels recovery. The temporary windowed
state does not trigger playback, Music Mode or plugin effects. The compositor
still owns the geometry.

The default-off General setting for blacking out other monitors attaches
display covers to the focused fullscreen player. The window that owns the
video determines the excluded display. Native focus and display topology are
sampled only while the policy is active. The covers release when the owner
loses focus, leaves fullscreen, closes, or disables the setting.

Windows uses non-activating layered HWNDs, macOS uses borderless AppKit
windows, and X11 uses override-redirect windows with empty input shapes.
Wayland requires layer shell and viewporter. The guest registry checks their
availability. A separate Wayland connection owns the output proxies and
blackout layers, so the toolkit's output callbacks retain their own proxy data.
The worker blocks on the compositor socket and the owner's wake socket.
[Win32 layered window input](https://learn.microsoft.com/windows/win32/winmsg/window-features#layered-windows)
[Wayland layer-shell protocol](https://github.com/swaywm/wlr-protocols/blob/master/unstable/wlr-layer-shell-unstable-v1.xml)

Fullscreen, Music Mode, and Picture-in-Picture confirmations enter the player's
plugin command queue in order. The window proxy publishes the owning native
window's first snapshot before `iina.window-loaded`. Later snapshots report the
outer frame, focus, minimization, and displays. Screen-change callbacks track the
active display's native identifier. During each mode callback and its Promise
jobs, the plugin sees the presentation state from that transition.

Frame and minimize requests reach the same owner after the presenter timer returns.
Native Windows resizes its framed owner and realigns the controls and video. AppKit
uses the `NSWindow` frame directly. X11 adapts the bottom-left logical coordinates
to the desktop coordinates. Wayland can request a size and leaves placement with
the compositor. An acknowledged frame request bypasses interactive aspect locking.
Sidebar requests use the same pane placement and tab selection as player controls.

The Windows framed player owns native file pickers. Both player surfaces are
blocked until the picker result reaches its caller, then the controls regain
input through local Win32 activation. Picture-in-Picture observes minimize and
restore changes on that same framed owner. Closing the preview restores the
placement it took over when entering and preserves moves made while the main
window remained open.

On Linux, the Picture-in-Picture preview uses the player's existing libmpv
render context. A fenced pixel buffer transfers bounded SDR frames into the
preview's independent Slint window. The player can draw into this buffer on
its own GL context when the main surface stops drawing. The X11 adapter owns
window visibility and minimize transitions. The Wayland compositor owns
placement and keeps control of the main surface's visibility.

Test playback can use null native output or a recording engine to exercise UI
callbacks without a native window. Windows software-rendered pixel
baselines cover the Slint surfaces. The platform service recipe shows how to
add a native query without coupling its caller to the target.
[Add a platform service](recipes/add-a-platform-service.md)

See [port development](port-development.md) and [verification](verification.md)
for runtime coverage on each target.
