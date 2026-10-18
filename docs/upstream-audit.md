# Pinned upstream source map

Baseline: official IINA v1.5.0, commit
`fbcff4df7183b335880396af0655f6bb89d46442`.
Files below are relative to the ignored `reference/iina-upstream/` checkout.
This is a source-navigation guide, not an implementation or runtime report.
Use [parity.csv](parity.csv) for individual coverage and
[reference captures](visual-reference-audit.md) for observed interface states.

## Subsystems

| Area | Actual source | Behavior/source anchors |
|---|---|---|
| Player sessions/commands | iina/PlayerCore.swift | multiple players; play, seek, loops, filters, screenshot workflows |
| Engine/events | iina/MPVController.swift; PlaybackInfo.swift | property observation; asynchronous replies; borrowed event data |
| Main window/video | iina/MainWindowController.swift; VideoView.swift | custom layout; fullscreen; renderer lifecycle |
| OSC | iina/OSCFloatingView.swift; OSCBottomView.swift; MainWindowController.swift; Preference.swift; OSCToolbarButton.swift | Default center controls are previous, play/pause, next; there is no Stop entry in the toolbar button type or default square Stop control. Four configurable right-side buttons default, in order, to Plugins, PiP, Playlist and Chapters, and Quick Settings. Stop remains a playback command elsewhere. |
| Quick settings | iina/QuickSettingViewController.swift | Layout/Video/Audio/Subtitles panes |
| Playlist/chapters | iina/PlaylistViewController.swift; SidebarPlaylistPane.swift | sidebar order, selection, chapter workflows |
| Music | iina/MiniPlayerWindowController.swift | distinct compact audio window |
| Preferences | iina/Preference.swift; PreferenceWindowController.swift | defaultPreference; obsolete/internal settings require reachability classification |
| New Settings | iina/SettingsWindow.swift; SettingsPage*.swift | old and new settings coexist behind enableNewSettings |
| Menus/input | iina/MenuController.swift; config/iina-default-input.conf; Base.lproj/MainMenu.xib | shortcut resource includes IINA actions and mpv commands |
| History | iina/HistoryController.swift | history, grouping and resume policies |
| Plugins | iina/JavascriptPlugin*.swift; JavascriptAPI*.swift | JavaScriptCore host and multiple public API surfaces |
| Localization | iina/en.lproj; ru.lproj; ar.lproj | bundled translations; provenance and placeholders need audit |
| Thumbnails/screenshots | iina/PlayerCore.swift; ThumbnailCache.swift | separate bounded workflows required |
| Platform | iina/AppDelegate.swift; VideoView.swift | Cocoa lifecycle and OS service adaptation required |
| Native ABI | deps/include/mpv/client.h; render.h; render_gl.h | client API 2.5; render backends OpenGL/software, not invented D3D FBO |

## Confirmed defaults

| Setting | Pinned default |
|---|---|
| Controller | Floating, width 460; bottom fraction 0.1, horizontal center 0.5 |
| Controller toolbar | Plugins, PiP, Playlist and Chapters, Quick Settings |
| Hide delay | 2.5 seconds |
| Volume | 100 |
| Compact UI | Off |
| Keep open at EOF | On |
| Sidebars | Width 360; playlist leading, settings trailing |
| Keyboard seek/volume increments | 5 seconds / 5 volume units |

The toolbar type has no Stop entry. Stop remains a playback action elsewhere.
See [visual metrics](visual-spec.md) for the controller and sidebar details.

## Behavior anchors worth preserving

- `PlayerCore.playFileInPlaylist` uses `playlist-play-index` to reload even the
  current item; explicit transitions pause before changing entries.
- Chapter selection uses its observed timestamp, an absolute seek and resume.
- Clearing a playlist preserves the playing entry. Stable identity and display
  index are separate concerns.
- Loading an already loaded external subtitle reloads it rather than duplicating
  it. Playlist move operations retain mpv's target-index semantics.
- Loop cycling is off -> file -> playlist -> off.
- Audio delay sliders cover -5 to 5 seconds in 0.05 steps; numeric input preserves
  finer/out-of-range values and blank entry resets zero.
- Recent-file recording and all-played tracking are distinct default-on
  preferences. macOS Open Recent delegates to `NSDocumentController`.
- Delete Current File is available for local media and moves it to the OS Trash.
- Playback History is an 800 x 600 resizable, grouped outline with search,
  progress and context actions. Do not confuse history with recent documents or
  watch-later resume state.

## Provider and plugin inspection

`OnlineSubtitle.swift` defines OpenSubtitles, Shooter, Assrt and plugin-provider
routing. Audit authentication, search/download selection, provider-specific
hashes, errors and attribution independently. Source inspection does not confirm
that an external endpoint is available or that a live provider session works.

For plugins, inspect the JavaScript host/API files, manifests and pinned type
definitions together. Track public API semantics in
[plugin-compatibility.csv](plugin-compatibility.csv); a single working plugin is
not conformance evidence for the whole host.

## Audit boundaries

The map and requirement inventory remain incomplete. Classify reachable user
behavior separately from internal, deprecated or dead code. Inspect lifecycle
and ownership before each implementation; write independent Rust code rather
than translating the reference. Preserve the pinned baseline and asset/license
boundaries in [the product contract](product-spec.md).
