# Initial audit · IINA v1.5.0

Commit: fbcff4df7183b335880396af0655f6bb89d46442. Windows source inspection only;
no macOS execution. Full files are in the excluded `reference/iina-upstream` checkout.

The repository was empty at acquisition. Swift comments use explanatory documentation,
particularly PlayerCore's A–B minimum and MPVController's event-queue caveats; riina
uses similarly purposeful comments adapted to Rust ownership.

Broad inventory (not yet a complete atomic feature transcription):

| Area | Actual source | Initial finding |
|---|---|---|
| Player sessions/commands | iina/PlayerCore.swift | multiple players; play, seek, loops, filters, screenshot workflows |
| Engine/events | iina/MPVController.swift; PlaybackInfo.swift | property observation; asynchronous replies; borrowed event data |
| Main window/video | iina/MainWindowController.swift; VideoView.swift | custom layout; fullscreen; renderer lifecycle |
| OSC | iina/OSCFloatingView.swift; OSCBottomView.swift; MainWindowController.swift; Preference.swift; OSCToolbarButton.swift | Default center controls are previous, play/pause, next; there is no Stop entry in the toolbar button type or default square Stop control. Four configurable right-side buttons default, in order, to Plugins, PiP, Playlist and Chapters, and Quick Settings. Stop remains a playback command elsewhere. |
| Quick settings | iina/QuickSettingViewController.swift | video/audio/subtitle panes |
| Playlist/chapters | iina/PlaylistViewController.swift; SidebarPlaylistPane.swift | sidebar order, selection, chapter workflows |
| Music | iina/MiniPlayerWindowController.swift | distinct compact audio window |
| Preferences | iina/Preference.swift; PreferenceWindowController.swift | defaultPreference; obsolete/internal settings require reachability classification |
| New Settings | iina/SettingsWindow.swift; SettingsPage*.swift | old and new settings coexist behind enableNewSettings |
| Menus/input | iina/MenuController.swift; config/iina-default-input.conf; Base.lproj/MainMenu.xib | shortcut resource includes IINA actions and mpv commands |
| History | iina/HistoryController.swift | local history/resume audit pending |
| Plugins | iina/JavascriptPlugin*.swift; JavascriptAPI*.swift | JavaScriptCore host and multiple public API surfaces |
| Localization | iina/en.lproj; ru.lproj; ar.lproj | bundled translations; provenance and placeholders need audit |
| Thumbnails/screenshots | iina/PlayerCore.swift; ThumbnailCache.swift | separate bounded workflows required |
| Platform | iina/AppDelegate.swift; VideoView.swift | Cocoa lifecycle and OS service adaptation required |
| Native ABI | deps/include/mpv/client.h; render.h; render_gl.h | client API 2.5; render backends OpenGL/software, not invented D3D FBO |

Read initially: LICENSE; PlayerCore/MPVController headers; Preference defaults;
iina-default-input.conf; OSCFloatingView; LayoutValue; the render header threading,
callback, initialization and teardown contracts. Deeper UI/plugin audit remains open.

Defaults confirmed: floating dark OSC, width 460; bottom ratio 0.1; center ratio 0.5;
auto-hide 2.5 seconds; soft volume 100; arrows control speed; keyboard seeks ±5 seconds;
keyboard volume ±5; compact UI false; keep-open at EOF true; sidebars 360.
Do not treat this initial inventory as the final parity denominator.

Online-subtitle provider audit (deeper pass): IINA v1.5.0 routes the default
OpenSubtitles, Shooter, Assrt and registered plugin providers through one
provider boundary in `iina/OnlineSubtitle.swift`. The Assrt source uses title
search, a 32-character account token, detail lookup and one or more provider
download files; the result description shows the native title, subtype/language
and upload date. Its API documents 15-result pages, 20 requests per minute,
optional archive file lists, and a service attribution requirement. Riina now
implements OpenSubtitles and a bounded Assrt path with secure per-provider
credentials, direct subtitle-file selection and an attribution link. Shooter,
plugin registration, Assrt archive extraction and live provider workflows
remain open. Assrt contract: https://2.assrt.net/api/doc. This is source/API
inspection, not an observed macOS provider-session capture.

IINA v1.5.0's online provider menu also offers Shooter.cn. It identifies local
media by four MD5 values from 4 KiB samples and submits those values with the
media path to the HTTPS `subapi.php` endpoint; responses can contain multiple
download links and a delay value. Riina implements a bounded HTTPS client with
explicit path/hash disclosure, accepted subtitle formats, secure-domain checks,
and integration into the existing result selection/cache path. Endpoint
availability and real service responses have not yet been verified.

General > History also exposes the default-on `recordRecentFiles` and
`trackAllFilesInRecentOpenMenu` preferences. IINA sends manually selected files
to `NSDocumentController` when recent recording is enabled, and notes other
successfully loaded media when all-played tracking is enabled. Its File > Open
Recent menu is system-managed. Riina currently implements the portable persisted
list and menu actions; the native macOS recent-document bridge remains open.

The pinned File > Delete Current File action is enabled only for a current local
URL. It removes that playlist position and moves the media file to the OS trash;
the menu reports failures. Riina wires an asynchronous system-trash operation
and removes the matching stable playlist ID only after the move succeeds. Active-
playback and platform runtime behavior remains part of grouped GUI verification.

IINA's Playback History menu opens an 800×600 resizable window with a three-column
outline grouped by date or file location, search by filename or full path, a
clear-history toolbar action, row progress, and context actions for play, new
window, reveal, history removal and moving local media to Trash. Riina implements
the grouping, search, row actions, expand/collapse controls, multi-selection and
multi-row context actions. Live selection/context-menu interaction, native row
artwork, localized date presentation and matched visual details remain open.

Daily-workflow audit: PlayerCore.playFileInPlaylist deliberately uses
playlist-play-index rather than playlist-pos to reload the current entry;
manual transitions pause before changing entries. playChapter uses the observed
chapter timestamp, absolute seek and resume. Clear preserves the playing entry.
LoadExternalSubFile reloads an already loaded external subtitle rather than adding
duplicates. MPVCommandWrappers preserves mpv playlist-move target semantics.
LoopMode.next is off → file → playlist → off. Audio delay sliders span -5…5 s
with a 0.05 s step, while the numeric input accepts more precise values outside
the slider range and an empty entry resets to zero. Riina already implements the
separate numeric input, localized formatting/parsing, and reset behavior in
`crates/app/ui/player.slint`, `crates/app/src/localized_number.rs`, and
`crates/core/src/timing_controls.rs`; focused parser and locale tests cover these
semantics. Live endpoint listening and matched macOS field geometry remain open.
SidebarTrackSelector has 26-point rows, a None entry and selection marker; Riina
now supports independent primary and secondary subtitle selection, visibility,
and delay. Matched panel rendering and cross-platform runtime checks remain open.
Playlist rows are 30 points spacious/26 compact; chapters are 40 points with
double-click playback. MPVTrack.readableString assembles title, codec, channel count,
sample rate and Default. Riina localizes track language badges using the system
display locale; live locale-change behavior and matched macOS captures remain open.
These are source-derived facts, not live macOS captures.
