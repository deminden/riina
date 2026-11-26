# macOS/Linux development

Windows is the supported application target. Linux has a full x64 application
build and playback, file-dialog and shutdown evidence on isolated X11 and native
Wayland displays. macOS needs full application and runtime verification.
See [Verification](verification.md) for the tested configurations and open
checks.

Use the repository's pinned Rust toolchain and lockfile:

```sh
cargo build -p riina --locked
cargo run -p riina --locked -- /absolute/path/to/media.mkv
```

The shared GL route requires an OpenGL-capable desktop session and libmpv client
API 2.5 or a compatible newer 2.x. Building does not require libmpv to be installed
because it is loaded at runtime. CI setup is recorded in
`.github/workflows/application-ports.yml`; Linux WebView compilation also requires
GTK 3, WebKitGTK 4.1 and `pkg-config`. Linux video overlays composite offscreen
WebKitGTK views inside the player on X11 and Wayland. Plugin sidebars use a
native browser child on X11 and the shared compositing path on Wayland. Linux
file selection needs an available XDG Desktop Portal backend with X11 or Wayland
parent support.

Wayland sidebar editors use the player's compositor connection for text input.
WebKit renders interim text, commits composition and supplies the
candidate-window anchor. WebKitGTK 2.54.1 and later can also supply surrounding
text for replacements and deletion around a caret. Password fields keep
surrounding text private. Earlier runtimes retain composition support.
Deleting around an existing selection while retaining that selection remains
open because WebKit exposes one contiguous delete range.
[WebKitGTK 2.54.1 release notes](https://webkitgtk.org/2026/10/02/webkitgtk2.54.1-released.html)

Slint 1.18.1 selects X11 when it detects WSL. An isolated native Wayland lab
can hide those detection markers inside the test's private mount namespace.
Record the actual surface protocol when reporting Wayland results.

Native file drops in the player, Welcome window and playlist have X11 and
Wayland desktop evidence. Both receivers accept plain-text media URLs.
[X11 playlist drop checks](verification.md#files-dropped-into-the-playlist-2026-11-23),
[Wayland drop checks](verification.md#wayland-file-and-url-drops-2026-11-23),
[X11 link and selection checks](verification.md#x11-media-link-and-file-drops-2026-11-26)

Playlist clipboard editing has [X11 and Wayland desktop evidence](verification.md#x11-playlist-clipboard-formats-2026-11-25).
Local file selections offer `text/uri-list` and plain text on X11 and on
Wayland compositors with data-control support. On X11, the file-manager
clipboard format advertises a Copy operation for both Copy and Cut in the
playlist. Riina asks a clipboard manager to retain the offered formats
when it exits. [Clipboard Manager specification](https://www.freedesktop.org/specifications/clipboard-manager-spec/)

X11 uses the GTK theme for plugin-contributed playlist popups on the owning
player window. Wayland shows the contributed menu inside the player surface.
It keeps submenu bounds within the client and scrolls lists that exceed its
height. In narrow windows, a submenu can overlap its parent. Left Arrow
returns to the parent. Both presenters keep the player event loop active.

Linux desktop media controls use MPRIS over the session bus. They follow the
active player and honor the system media controls preference. Desktop clients
can browse and edit the portion of the playlist around the playing track.
Opening a local file or an HTTP(S) media URL through MPRIS adds it to that
playlist and starts the new entry. [MPRIS player interface](https://specifications.freedesktop.org/mpris/latest/Player_Interface.html),
[MPRIS TrackList interface](https://specifications.freedesktop.org/mpris/latest/Track_List_Interface.html)

Linux registers local recent media in the per-user GTK bookmark store.
Clearing Open Recent removes Riina's registration from each bookmark and
retains media registered by another application. Updates reread the store
when GIO detects a simultaneous change. [GLib desktop bookmarks](https://docs.gtk.org/glib/struct.BookmarkFile.html)

Live Text on Linux uses the installed Tesseract 5 library and language data.
Riina looks for `libtesseract.so.5`, prefers a model for the desktop language,
and includes English when its model is available. `TESSDATA_PREFIX` can point
to a directory containing the recognition data. The feature is off by default.
With the runtime installed, enable it in Video settings or the Video menu and
pause on a frame to select recognized text. [Tesseract 5 C API](https://github.com/tesseract-ocr/tesseract/blob/5.5.0/include/tesseract/capi.h)

With the runtime and English model installed, run
`cargo test -p riina --lib --locked native_tesseract_detects_selectable_lines_and_original_pixel_bounds -- --ignored`
for the native recognition check. The fixture is drawn in memory and checks
both text and geometry from a downsampled frame.

Picture-in-Picture uses the same playback session on Linux. The X11 preview
requests always-on-top and supports the player's hide, minimize and restore
policies. On Wayland it is a regular toplevel, and entering keeps the player
open. Wayland does not provide the minimize observation needed for automatic
entry. Placement and stacking depend on the compositor. Test the shared
controls, live frames, and shutdown on both backends.

The macOS drop adapter needs a native desktop check. On one isolated profile,
drop a Finder file on Welcome, then drop a folder into the middle of the active
player's playlist. Check the right-edge reveal, cancellation, insertion order,
and paused playback position. Drop a Safari link to a loopback media fixture and
plain-text references on the player. Close the window during a drag and verify
that the next window receives its own drops. Record the binary and OS with the
[macOS drop checks](verification.md#macos-file-and-url-drops-2026-11-23).

Legacy fullscreen needs a native macOS check. On an isolated profile, verify
normal fullscreen opens a separate Space and the General preference keeps
legacy fullscreen in the current Space. Test Escape and Music Mode exits,
playback preferences, the green window control and restored frame geometry.
Change the preference while fullscreen and verify the next entry uses it.
On a notched display, check that video stays below the camera housing and the
menu bar can still appear. Close legacy fullscreen players in both orders
with two displays and verify the Dock and menu bar restore after the last
legacy window exits.

On Ubuntu 24.04, install the WebView development packages before building:

```sh
sudo apt-get install libgtk-3-dev libwebkit2gtk-4.1-dev pkg-config
pkg-config --modversion gtk+-3.0 webkit2gtk-4.1
```

The application-port workflow installs and probes these dependencies before
linting the full workspace. On 2026-10-20, revision `f674279` passed strict
application lint on Linux and both macOS runners; Linux also passed the unit
tests and full application build. macOS unit tests exposed ordinary relative
paths being rewritten by alias resolution. Revision `948fd34` preserves those
targets. The [release record](../tests/visual/windows-release-4.2.0-2026-10-20.json)
separates those source follow-ups from the Windows binary built at `5910648`.

Library discovery uses `libmpv.so.2` on Linux and `libmpv.2.dylib` on macOS. A
library beside the executable takes precedence. macOS also checks an application
bundle's Frameworks directory and `/opt/homebrew/lib` or `/usr/local/lib`; the
system loader supplies the final fallback. Codec packages and binaries are not
included in this repository. Check their licenses before packaging.

Custom libmpv builds need a scripting or C-plugin backend to expose the
`load-scripts` option. Riina sets that option to `no` during startup.
[libmpv option definition](https://github.com/mpv-player/mpv/blob/v0.41.0/options/options.c#L551-L557).
The embedded console and statistics shortcuts require a Lua-enabled build.

Use a Linux runtime with mpv's scaler-padding and float-option fixes. An
affected runtime can show black video on Mesa llvmpipe or reject the native
`default` reset for `tone-mapping-param`. Both fixes ship in libmpv 0.42.0.
A local 0.41.0 build with both backports passed the native suite.
[Scaler-padding fix](https://github.com/mpv-player/mpv/commit/72d43dc9c999a21d867cdc0f934f3e4cd2195aa9),
[float-option fix](https://github.com/mpv-player/mpv/commit/ae119448b2c388f4fdee30e59bb1ab84f2cde428),
[Linux engine verification](verification.md#linux-tone-mapping-default-2026-11-23).

Preferences use macOS `~/Library/Application Support/riina/preferences.json` or
Linux `$XDG_CONFIG_HOME/riina/preferences.json` (normally
`~/.config/riina/preferences.json`). Volume stays capped at 100% unless the user
enables the shared 200% option. Windows retains its own preferences location.

Native surface materials are a port requirement: Liquid Glass on macOS and
available compositor materials or a documented fallback on Linux. Windows has
implemented Mica/Acrylic and sampled-video paths. Native material acceptance
on the ports remains open. See [visual rules](visual-spec.md#platform-materials-and-artwork)
and [platform boundaries](platform-differences.md#rendering-and-materials) for
implementation and runtime gaps.

Validate first-frame rendering, play/pause, seek, stop/reopen, each picker purpose,
cancel/reopen, external tracks, fullscreen/Escape, resize/DPI, owned-dialog close
handling and shutdown on each target. Record exact OS, architecture, library,
binary hash and observed outcomes. Application CI proves compilation and unit
tests only. Native HDR presentation, localized filename ordering, application
bundles/installers and pixel-perfect reference comparisons remain open.

See [ADR 0004](adr/0004-shared-gl-application-ports.md) for the service boundary.

Real decoding/catalog tests are available independently of GUI builds:

```sh
RIINA_TEST_MPV_LIBRARY=/absolute/path/to/libmpv.2.dylib \
  cargo test -p riina-mpv --test native --locked -- --ignored --nocapture --test-threads=1
```

Use the Linux library path on Linux. The manual macOS libmpv integration workflow
records its provisioned Homebrew runtime before running these tests. Null video
output exercises actual decoding, seeking, playlist/track commands and metadata;
it does not validate visible rendering or sound.
