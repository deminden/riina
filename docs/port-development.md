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
GTK 3, WebKitGTK 4.1 and `pkg-config`. The current plugin child-WebView adapter
supports X11, not Wayland embedding. Linux file selection needs an available
XDG Desktop Portal backend; the dialog dependency supports X11 and Wayland parents.

Native file drops in the player, Welcome window and playlist have X11 and
Wayland desktop evidence. Wayland also accepts plain-text media URLs.
[X11 playlist drop checks](verification.md#files-dropped-into-the-playlist-2026-11-23),
[Wayland drop checks](verification.md#wayland-file-and-url-drops-2026-11-23)

The macOS drop adapter needs a native desktop check. On one isolated profile,
drop a Finder file on Welcome, then drop a folder into the middle of the active
player's playlist. Check the right-edge reveal, cancellation, insertion order,
and paused playback position. Drop a Safari link to a loopback media fixture and
plain-text references on the player. Close the window during a drag and verify
that the next window receives its own drops. Record the binary and OS with the
[macOS drop checks](verification.md#macos-file-and-url-drops-2026-11-23).

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
