# macOS/Linux development

These application ports are being built now. Native playback and GUI validation
are pending; Windows remains the supported application target.

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

On Ubuntu 24.04, install the WebView development packages before building:

```sh
sudo apt-get install libgtk-3-dev libwebkit2gtk-4.1-dev pkg-config
pkg-config --modversion gtk+-3.0 webkit2gtk-4.1
```

The application-port workflow installs and probes these dependencies before
linting the full workspace. On 2026-10-20, revision `f674279` passed strict
application lint on Linux and both macOS runners; Linux also passed the unit
tests and full application build. macOS unit tests exposed ordinary relative
paths being rewritten by alias resolution. Revision `948fd34` preserves unchanged
targets, and its [full application verification](https://github.com/deminden/riina/actions/runs/39351055679)
is in progress. The [release record](../tests/visual/windows-release-4.2.0-2026-10-20.json)
separates these source follow-ups from the Windows binary built at `5910648`.
Neither compilation nor these tests establish native playback support.

Library discovery uses `libmpv.so.2` on Linux and `libmpv.2.dylib` on macOS. A
library beside the executable takes precedence. macOS also checks an application
bundle's Frameworks directory and `/opt/homebrew/lib` or `/usr/local/lib`; the
system loader supplies the final fallback. Codec packages and binaries are not
included in this repository. Check their licenses before packaging.

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
