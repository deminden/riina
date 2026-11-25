# Current status

Updated 2026-11-25. Published release: **4.2.1**. Reference: **IINA v1.5.0**
at `fbcff4df7183b335880396af0655f6bb89d46442`.

Riina is a functioning media player. Full feature, behavior and visual parity
remain active goals; the [requirement inventory](parity.csv) is incomplete.

## Platforms

| Platform | State |
|---|---|
| Windows 11 x64 | Supported application target; native D3D11 video is the default |
| macOS | Application port in development; full build, visible playback and lifecycle acceptance remain open |
| Linux | Application port in development; full desktop playback and lifecycle acceptance remain open |
| Other Windows versions and ARM64 | Not verified |

`--shared-gl` selects the shared OpenGL path. Its SDR output and Windows HDR
composition limitations are documented in the [rendering decisions](README.md#architecture-and-ports).

## Implemented workflows

Implementation does not imply every state or platform is verified.

- **Playback:** files, folders and URLs; play/pause, seeking, volume/mute, speed,
  frame stepping, loops, chapters, playlist editing, history and resume.
  Pause When Open also applies when the playlist automatically advances.
- **macOS Touch Bar:** the player defines customizable native playback controls,
  time displays, and a scrubber with cached thumbnails. The second clock has
  its own saved mode. AppKit supplies the customization menu on supported Macs.
  Physical Touch Bar interaction and appearance still need a native macOS session.
- **macOS Now Playing artwork:** cover art and Quick Look take precedence over
  the current video's completed timeline thumbnails. The fallback uses the
  frame just before the midpoint, capped at 60 seconds. One application-wide
  Now Playing session follows the most recently focused player.
- **Controller arrows:** Interface Settings selects playback speed, playlist
  navigation, or a 10-second seek. The main controller and Touch Bar use the
  selected action. The Music Mode arrows navigate the playlist.
- **Audio startup and ReplayGain:** a new player uses the configured initial
  volume when enabled and the saved volume otherwise. An explicit launch
  volume takes precedence, within the active 100% or 200% limit. A deliberate
  volume change is available to new players while the current one remains
  open. ReplayGain mode, preamp, clipping and fallback edits apply to all
  open players using the saved profile.
- **Audio devices:** an unavailable saved device opens with automatic device
  selection and remains saved for a future launch. During playback, device
  removal falls back to Automatic. Core Audio and AVFoundation use the same
  endpoint after a driver change when libmpv lists both names.
- **Chapter metadata and timeline:** the active chapter follows the native
  playhead. Music Mode and desktop media controls for audio use the chapter's
  title and performer when available. The timeline shows the hovered chapter
  and time even when thumbnail images are disabled or unavailable.
- **Music Mode catalog:** the compact player opens Playlist and Chapters in
  the same panel. The toolbar remembers the active tab, and both lists keep
  their scroll positions when switching between the main and compact player.
  Chapter rows show the time range. Navigation moves the row selection and
  activation seeks to the chapter.
  Resize the compact window to reveal or hide the panel. It stays open
  with at least 200 points of space below the controls and artwork, and
  keeps the dragged height at or above that threshold. Explicit panel and
  artwork choices save to the profile. Dragging changes the current player.
- **Music Mode controls:** the title and timeline remain visible when the
  player is idle, including with the catalog open. Hovering over artwork or
  controls reveals transport. Keyboard focus reveals the focused group of
  actions. Volume expands from the same button with a slider and numeric
  readout. Scrolling the artwork shows that readout when the gesture changes
  volume. Linux exposes the player menu through a compact button and F10.
  A media session can render subtitles on an audio-only canvas without
  adding a video track. The canvas stays with the player when artwork rolls up.
- **Automatic Music Mode:** the player gives each pending switch one
  deadline. When video returns, it applies the saved resize policy after
  restoring the main frame, even if the decoder reported the size while
  the compact frame was still active.
- **File opens:** the primary File commands follow the new-window preference,
  and their modifier shortcuts invert it. Active menus show the actual destination.
  The native Windows picker has verified grouped and separate opens and reuse
  of an existing player when duplicate players are disabled. Queued group
  feedback follows the first loaded file and reports accepted entries.
  A replacement request or a closing player cancels its folder scan.
- **Playlist editing:** the sidebar and Music Mode share the playlist
  controls. Add opens the file and URL choices, with a separate folder choice
  on Windows and Linux. Sort opens the four filename and path orders.
  Transient add and clear feedback follows native command replies. Clearing
  retains the current item and retires pending sibling discovery. A single-file
  open can discover siblings around the current item. Navigating the resulting
  playlist keeps rows the user removed
  out of the queue. The sidebar and Music Mode accept focused edit shortcuts.
  Paste expands folders and inserts before the first selected row. Windows
  publishes native file lists and Unicode text for local selections. A separate
  GTK app read both formats on X11 and Wayland. An X11 test clipboard manager
  retained the copied files after Riina exited. Edit menu commands follow the
  focused playlist row or text editor on the shared and Windows native menu paths.
- **Open URL:** one dialog serves the File menu, Welcome and the windowless
  macOS application menu. It previews the default HTTP scheme, offers HTTP
  authentication, and can stop a pending load.
  Direct browser links and groups that begin with a network URL share
  its loading and retry controls. Retrying the first stream preserves the
  remaining playlist.
- **Recent activity:** player windows share Open Recent with Welcome and
  plugins. The macOS application menu also opens and clears recent documents
  while no player is open. Settings Utilities clears history, recent documents
  and the saved Welcome resume item together after confirmation.
- **Playback History:** the macOS application menu can open the shared history
  window and launch playback from it before any player window has opened.
  Visible local files are checked in the background. Unavailable files hide
  their progress, and reopening History checks whether they have returned.
  Playback decodes saved local file URLs before handing the path to the player.
  Playback actions recheck selected files off the interface thread and choose a
  visible destination when the check finishes. Single-row opens honor the
  saved window and resume settings. The row menu is reachable from the keyboard.
  Saved positions come from mpv's files and refresh when History opens.
  Clearing saved progress in Utilities updates the open History window and
  preserves its entries.
  Date headings and played-at timestamps use the system locale and local
  time zone. Collapse and chronological ordering use local calendar days.
- In **Quick Settings Layout**, the selected Theme option is one Tab stop.
  Arrow keys, Home and End choose a theme. Tab navigation scrolls narrow
  Layout panes to reveal the focused switch or controller choice.
- In **Quick Settings Video**, each preset rail has one Tab stop and reveals
  the focused choice. Custom aspect entry supports cancellation. The
  **Subtitles** page displays the engine's current font size and border
  width, including values outside the preset lists.
- **Subtitle color controls** use the shared AppKit panel on macOS in both
  Quick Settings and Settings. The portable popup opens with keyboard
  focus on its first channel and keeps changes applied before dismissal.
  Numeric sidebar edits retain their draft when a Control or Command
  shortcut reaches the player.
- **Subtitle font chooser:** the player applies the chosen font to its
  current playback session. Settings opens the same chooser with the saved
  default and shares a committed change with players using that profile.
  The chooser retains its filter and sample across openings.
- **Utilities:** Settings owns the confirmations for alerts, saved progress,
  history and thumbnails. Cache sizes refresh when the page opens or data is cleared.
- **Russian and Arabic labels** are bundled for Welcome, Open URL, Playback
  History, Log Viewer, About, the audio and video filter windows, and the
  top-level player menu families. The application and idle macOS menus use
  the same captions where they share commands. The system language selects the
  catalog, with English as the fallback. Settings headings, controls, search
  results and confirmations use these catalogs across all eleven pages.
  Plugin permission descriptions use the same
  catalog as the installation and update reviews. Search accepts translated
  titles and English aliases. The player encoding menu, other player controls
  and some runtime messages still need translations.
- **Linux menus** scroll in short players with either the regular bar or the
  compact Menu button. Menu titles switch on hover while a menu is open.
  Keyboard navigation and a Back control keep overlapping submenus reachable.
- **Controller clocks:** Interface Settings controls remaining time, play-speed
  scaling and center snapping. Click the right clock to switch between
  duration and remaining time, and right-click either clock for precision.
  Players using the same profile update together, including while paused.
- **Log Viewer:** Window and Advanced Settings open the shared viewer for
  application and mpv logs. Filter by multiple subsystems, severity and message,
  copy selected rows or export the full or filtered view. The macOS Window menu
  can open it before playback starts.
- **Window switching:** the Window menu lists and switches between players on
  Windows and X11, including minimized players. On Wayland it requests
  activation from the compositor. The macOS port registers Window and Help
  with AppKit for the system window list, standard commands and Help search.
- **macOS legacy fullscreen:** General Settings can keep fullscreen playback
  in the current Space. The default uses standard macOS fullscreen. The legacy
  path restores the previous window frame and Dock and menu-bar presentation,
  and keeps playback below the camera housing on equipped displays. Native
  macOS verification remains open.
- **Fullscreen display blackout:** the optional General setting blackens other
  displays while the fullscreen player owns focus. The default is off. Two-display
  X11 and Wayland sessions verified video visibility, click-through and cleanup.
  Wayland requires layer shell and viewporter. Windows and macOS multi-display
  runtime verification remains open.
- **New Window:** the optional File menu shortcut opens an independent Welcome
  window. The command stays hidden with the default profile.
- **Debug Info:** Help saves one report with playback snapshots from every
  open player in the application. The idle macOS Help menu can save a report
  before playback starts.
- **Inspector** opens from the Window menu and shows the focused player's
  general media details, tracks, file metadata and playback status. Watched
  mpv properties are saved in the profile and update while the window is visible.
- **Audio and Video Filters:** each editor follows the last active player and
  retains saved presets when playback closes. The macOS Audio and Video menus
  can open the editors before playback. Preset changes are shared across
  player menus and shortcuts.
- **Release Highlights:** Help opens the current release notes. Released builds
  show the bundled guide on the first launch of each version with a player or
  Welcome window. The idle macOS Help menu can open the guide on demand.
- **Software Update:** the idle macOS application menu can check on demand,
  and the opt-in schedule starts with the application.
- **macOS Settings:** the app menu can open the saved profile without a
  player. Edits apply to subsequent playback, and a Welcome window with an
  initialized player can use its live Settings form. AppKit verification
  remains pending.
- **About:** player menus and the idle macOS menu share one window. About can
  check the installed libmpv and FFmpeg versions before the first player opens.
- **Command line:** `riina-cli` starts playback from files, URLs or standard
  input. It can return immediately or wait for the player it started.
- **macOS launch routing:** the port accepts Finder document groups, uses an
  AppKit panel for mixed file and folder selection, and follows the launch
  setting when reopened from the Dock. The windowless menu provides Open File
  and Quit. These routes still need a native macOS session.
- **Video/audio/subtitles:** track selection and external tracks, geometry and
  crop controls, filters/equalizers, subtitle styling/timing, screenshots,
  thumbnails, Music Mode and Picture in Picture.
- **Interface:** floating/top/bottom controls, Quick Settings, configurable
  sidebars, native menus and shortcut hints, Welcome and Preferences windows.
  All eleven Preferences categories have implementation; completeness varies.
- **Extensions and services:** browser links and the Open in Riina web
  extension, plugin management, QuickJS host APIs and WebView surfaces,
  online-subtitle providers, online-media helpers, OCR and system-media
  adapters. Compatibility and platform coverage remain partial.
- **Plugin packages:** the macOS bundle registers `.iinaplgz` for Finder.
  Finder and Settings share the permission review and replacement flow.
  Supported packages enable after approval, and unsupported packages stay
  installed and disabled. Finder delivery still needs native verification.
- **macOS plugin startup:** enabled global plugins can initialize without a
  player and request a tool window, prompt, picker or new player. The
  application Plugin menu shows global contributions and opens Plugins
  Settings or a global developer console between playback sessions.
  Application-owned tools participate in shutdown. Native macOS desktop
  verification remains pending.

Volume defaults to 0-100%; the optional boost setting allows 200%.

## Recent reliability work

- **Thumbnail extraction:** the isolated decoder can sample its retained final
  frame and finish a complete preview cache. The interactive Live Text capture
  requires paused playback.

- The remaining video contrast loss came from the native controls window's DWM
  frame tint. Full frame extension now leaves the underlying video unchanged;
  all 12 sampled HDR patches match with controls shown, hidden and restored in
  both Windows HDR modes. The reported movie and fullscreen playback were also
  checked. See the [composition diagnosis](verification.md#hdr-display-lifecycle-review-2026-11-09)
  for desktop evidence and calibration limits.

- The reported HDR10 contrast issue was reproduced in a same-frame desktop
  comparison. Windows native HDR conversion now keeps its linear output target
  instead of applying an automatic SDR ICC profile. PQ/HLG/SDR transitions retain
  the ICC preference for SDR video. See the [contrast review](verification.md#hdr-display-lifecycle-review-2026-11-09)
  for measured scope and remaining display validation.

- HDR output now follows native display changes during the same video, including
  paused playback. A manual HDR-off choice survives monitor changes; a new file
  restores the configured default. See the [HDR lifecycle review](verification.md#hdr-display-lifecycle-review-2026-11-09)
  for regression scope and remaining physical-display acceptance.

- The input review separates key conversion, compiled dispatch, menu accelerators
  and profile edits. Encoded special keys now reach profile actions, duplicate-key
  menu hints follow effective bindings, and malformed modifiers cannot shadow bare
  keys. Plugin grant writes serialize across players and preserve concurrent
  changes. See [the input review](verification.md#input-and-plugin-preferences-review-2026-10-23)
  for verification and remaining interactive/platform acceptance.

- The preferences review separates the window's responsibilities from screenshot
  settings and makes Music Mode choices commit before changing the live player.
  Disabling speed/pause feedback clears queued messages of that category.
  Plugin storage keeps distinct long, uppercase and nonportable identifiers
  separate; file capabilities, player snapshots and text decoding have their own
  modules. See [the preferences review](verification.md#preferences-and-feedback-review-2026-10-22)
  for verification and scope.

- The player review separates input, lifecycle, feedback, LiveText, discovery and
  rendering, and unifies session construction. It corrects automatic resume and
  Windows minimization ownership, stale recognition after a paused seek, repeated
  idle frame copies, controller-hide guards and superseded subtitle-scan completion.
  See [the player review](verification.md#player-controller-review-2026-10-22) for
  verified scope and remaining native/visual acceptance work.
- The latest local quality review removes an inverted history/recent-media lock
  order, timer starvation under queued plugin events, and duplicate history saves
  during selection removal. History persistence and plugin snapshots now have
  separate modules. Plugin writes create external destinations exclusively, and
  text reads enforce their limit against bytes actually read. Verification is
  recorded separately in [the review results](verification.md#history-and-plugin-quality-review-2026-10-22).
- The follow-up local review separates player observations, catalogs, materials,
  plugin integration and audio/video/subtitle callbacks, retaining session resource
  order. Plugin startup and callbacks now drain Promise jobs, package `require()`
  retains its native bridge, and shutdown cancels queued/executing JavaScript.
  Network request limits include workers waiting to deliver their result. See the
  [continuation verification](verification.md#runtime-and-player-review-2026-10-22)
  for passing checks and the unusable visual captures.
- Local review separates core command planning, playback observations, preferences
  and plugin menu responsibilities into focused modules while preserving the core
  public API. Auto seeks retain their per-file timing decision, disabled plugin
  menu ancestors suppress child actions, and invalid native clock values clear
  stale timestamps. Native shutdown is reserved for the final player owner.
  These changes are local; the published release remains 4.2.0. See
  [verification](verification.md#local-review-2026-10-22) for checked scope.
- Refreshed 4.2.0 provides a current-user Windows installer and a portable ZIP,
  with the same application and playback engine. Install/reinstall, shortcuts,
  installed playback and uninstall passed. A local upgrade from the earlier
  preview preserved preferences/history. The
  [refresh record](../tests/visual/windows-release-4.2.0-refresh-2026-10-20.json)
  separates the exact packages from the superseded release and test failures.
- The settings/Music Mode follow-up retains the animated sidebar scroll
  position across Layout and other categories. Windows artwork has its own
  native region above compact controls and the playlist; hiding it no longer
  leaves artwork under the controls. Fit to Screen uses measured native frame
  geometry and preserves IINA's shrink-only semantics. Scoped 300% DPI checks,
  return to the main player and fullscreen are recorded in the
  [settings/viewport record](../tests/visual/windows-settings-viewport-2026-10-20.json).
  These changes are included in refreshed 4.2.0; matched Music Mode
  control height/materials, broader resizing and other platforms remain open.
- The playback-input follow-up preserves fractional Windows wheel lines
  instead of promoting every report to a notch. Hide Riina is enabled, using
  restorable taskbar minimization on Windows. Click defaults remain IINA's:
  single hides controls, double toggles fullscreen, right toggles pause. Native
  checks cover those defaults, configured single-click Pause/Resume, horizontal
  and timeline wheel amounts, and Hide/restore; physical Magic Mouse feel and
  live macOS comparison remain open. See the
  [input record](../tests/visual/windows-playback-input-2026-10-20.json). These
  changes are included in refreshed 4.2.0.
- Preference saves merge changed fields under a cross-process lock, preserve
  unrelated newer settings, and remain retryable after failed writes.
- Accepted playback commands keep FIFO order; a full queue returns an error.
- Plugin HTTP transports share redirect validation and credential handling.
- Windows volume changes update the native menu label in place. Fullscreen and
  Music Mode retain and detach/restore the native menu.
- The fullscreen controls surface is one physical scanline shorter than the
  native video surface. The user confirmed visible video on controller reveal
  with this workaround; other drivers and HDR remain separate checks.
- Release validation exercises the extracted executable, persisted settings and
  clean shutdown, including when a matching native-engine artifact is reused.
- Preferences standard controls use the same light palette as their panels.
  Audio sections share one scroll page; auxiliary windows retain their width
  when only their height needs to fit the desktop.
- Quick Settings keeps every category reachable in narrow windows. Audio uses
  one scroll page with horizontal equalizer scrolling when required. Unused panel
  input no longer changes playback volume or dismisses the sidebar.
- Native menu shortcut hints include shifted punctuation and prefer the primary
  default chord while retaining aliases. Subtitle scale menu actions use the
  reference multiplicative steps; information labels refresh in place on opening.
- Light Quick Settings tints its sampled video before drawing controls. Fullscreen
  Escape also works when the framed playback host has keyboard focus.

## Changes in 4.2.0

Quick Settings now shares one track-list control across Audio, Video and both
subtitle lanes. Repeated arrows advance without waiting for playback observations;
Home/End and Page Up/Down reveal the focused row. Each list is one Tab stop, while
the active-track marker continues to follow the engine. Language badges fit their
text and reserve space for the track name, including in the Video pane.

The [track-list record](../tests/visual/windows-quick-settings-tracks-2026-10-19.json)
identifies that earlier executable and its scoped native checks. The newer
[workflow record](../tests/visual/windows-workflows-2026-10-20.json) separates the
current checks from intermediate failures and remaining acceptance work.

The follow-up adds keyboard-operated equalizer Save/Rename, Unicode profile
persistence, semantic light-theme handles and subtitle warning colors. The play
triangle has an independent optical correction; Music Mode owns separate transport
geometry and top-left window actions. The superseded package's 285 x 141 / 300%
check reset Audio after a Layout round trip and reported undersized Fit to Screen.
The follow-up included in refreshed 4.2.0 verifies restoration and client-preserving
centering at 285 x 160. Pinned source inspection confirms that Fit to Screen
centers small windows without enlarging them; its old expectation was incorrect.
Playlist rows take their available width, with six footer actions
and less frequent editing in row context menus.

First-pointer slider/color input, paused-catalog Music Mode debounce and Windows
watch-later path identity are corrected. History accepts late duration discovery
without recording another visit or resetting saved progress. Shared auxiliary-window acrylic fills
use neutral RGBA colors rather than opaque cyan ARGB spellings. Folder discovery
now has real temporary-filesystem regressions for ordering, exclusions and stale results. Configurable
Control bindings from the framed host share the existing dispatch route; native-
menu-only accelerators from every focus owner remain separate work.

Only Windows was available for desktop checks. The refreshed source includes the
Linux GUI dependency and macOS alias-path repairs described in
[port development](port-development.md). Full-application source checks and native
desktop acceptance remain separate. Native macOS/Linux playback, physical audio/HDR,
other DPI, complete focus/accessibility behavior and matched reference pixels
remain unverified. The Windows release collects the playback/settings changes
above; no supported macOS/Linux binaries are published. Exact-package evidence
is separate from the development checks below.

The workflow batch passed build, formatting, strict Clippy, 656 workspace tests
(56 ignored), 40 widget regressions, 62 Python tests and a 5832 ms debug playback
smoke. The record separates native engine checks, GUI candidates and remaining
artwork underlay/clipping, focus, shortcut and visual-comparison gaps.

## Previous release

The [4.1.0 notes](release-notes/4.1.0.md) and
[release record](../tests/visual/windows-release-4.1.0-2026-10-19.json) retain the
earlier numeric-editing, category-navigation, Windows Alt-state and Layout work.
Those dated checks are separate from the newer workflow and package evidence.
The original portable-only 4.2.0 release is superseded; its
[historical record](../tests/visual/windows-release-4.2.0-2026-10-20.json)
retains source `5910648`, old hashes and the gaps found in that package.

## Verification summary

**[Refreshed 4.2.0](https://github.com/deminden/riina/releases/tag/v4.2.0) is published.**
Its [release record](../tests/visual/windows-release-4.2.0-refresh-2026-10-20.json)
identifies source `2e9aa46`, both download hashes and the unchanged engine reused
from 4.1.0. The user explicitly requested replacement of the earlier 4.2.0 assets
and tag; both source identities are retained in the record.

Build, formatting, strict Clippy, 656 workspace tests (56 ignored), 51 native
integration tests and 68 packaging regressions passed. Saved binaries passed
corrected install/reinstall/uninstall verification without recompilation.
Hosted portable and installed playback passed in 6261 ms and 6878 ms; local
installed playback passed in 5890 ms. Full-desktop before/fullscreen/after captures
were inspected, and the test process exited normally. Hosted software OpenGL was
test-only and is absent from both downloads. The public source ZIP is 2.61 MB;
all 559 files match the release commit, with no duplicated build/vendor trees.

The development checks for the new changes are listed above. Earlier settings,
menu and playback results remain dated in the [verification register](verification.md),
including the [0.4.0 release record](../tests/visual/windows-release-0.4.0-2026-10-19.json).
The [visual ledger](visual-reference-audit.csv) records current interface gaps and
links historical evidence separately. Physical compositor, audible-output and HDR
coverage remain separate.

## Remaining acceptance work

1. Close fully missing IINA behavior using the requirement and plugin inventories.
2. Compare every interface surface against matched IINA captures, including
   narrow windows, both themes, compact layouts, scrolling and interaction.
3. Complete plugin conformance, localization and native accessibility coverage.
4. Verify macOS/Linux application playback, dialogs, window ownership and shutdown.
5. Validate HDR/color output, audible/pass-through audio, display changes,
   system-material policy and broader DPI/multi-monitor behavior on hardware.
6. Verify each final release package, its notices and exact source/build identity.

See the [implementation plan](implementation-plan.md) for the work order and
[platform differences](platform-differences.md) for specific adaptations.
