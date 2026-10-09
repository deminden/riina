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
because it is loaded at runtime. Ubuntu GUI build prerequisites are recorded in
`.github/workflows/application-ports.yml`. Linux file selection needs an available
XDG Desktop Portal backend; the dialog dependency supports X11 and Wayland parents.

Library discovery uses `libmpv.so.2` on Linux and `libmpv.2.dylib` on macOS. A
library beside the executable takes precedence. macOS also checks an application
bundle's Frameworks directory and `/opt/homebrew/lib` or `/usr/local/lib`; the
system loader supplies the final fallback. Codec packages and binaries are not
included in this repository. Check their licenses before packaging.

Preferences use macOS `~/Library/Application Support/riina/preferences.json` or
Linux `$XDG_CONFIG_HOME/riina/preferences.json` (normally
`~/.config/riina/preferences.json`). Volume stays capped at 100% unless the user
enables the shared 200% option. Windows retains its own preferences location.

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
