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
| OSC | iina/OSCFloatingView.swift; OSCBottomView.swift | floating width 460, minimum 200, position ratios; compact variants |
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

Daily-workflow audit: PlayerCore.playFileInPlaylist deliberately uses
playlist-play-index rather than playlist-pos to reload the current entry;
manual transitions pause before changing entries. playChapter uses the observed
chapter timestamp, absolute seek and resume. Clear preserves the playing entry.
LoadExternalSubFile reloads an already loaded external subtitle rather than adding
duplicates. MPVCommandWrappers preserves mpv playlist-move target semantics.
LoopMode.next is off → file → playlist → off. Audio delay sliders span -5…5 s,
but source text input allows wider values; current riina slider lacks that input.
SidebarTrackSelector has 26-point rows, a None entry and selection marker; subtitle
pane additionally has secondary selection/visibility/delay, still pending here.
Playlist rows are 30 points spacious/26 compact; chapters are 40 points with
double-click playback. MPVTrack.readableString assembles title, codec, channel count,
sample rate and Default. Language badges use localized names upstream; riina currently
shows the raw language code. These are source-derived facts, not live macOS captures.
