# 0001 - OpenGL SDR feasibility path

**Status:** historical feasibility decision. Superseded for default Windows
presentation by [ADR 0002](0002-native-d3d11-experiment.md). Shared OpenGL remains
the SDR application-port path in [ADR 0004](0004-shared-gl-application-ports.md).

## Original experiment

The first spike used Rust 1.90.0, Slint/slint-build 1.13.1, glow 0.16.0,
libloading 0.8.9 and windows 0.61.3, with development mpv commit
`36abaa32d00a7229ee206aae12dc0e97e7962dca`. These are historical versions;
`Cargo.toml`, `Cargo.lock` and `rust-toolchain.toml` identify current dependencies.

The libmpv OpenGL render API writes a GPU RGBA8 framebuffer and exposes its texture
through Slint's borrowed-texture interface. Controls share the window above video;
ordinary playback does not read each frame back to the CPU.

## Invariants retained

- The event worker owns the player handle. GL calls use the creating context.
- Borrowed textures remain alive for their complete use in that context.
- Update callbacks marshal wakeups and do not reenter mpv or GL.
- Render resources are released before the player worker.
- RGBA8 composition is SDR. Decoding HDR does not prove HDR presentation.
- mpv's native D3D11 output is distinct from its OpenGL/software render API.

The production Windows route supplies a retained native host to mpv; it does not
borrow an unowned swap-chain pointer. Current ownership and fullscreen composition
requirements are in [ADR 0003](0003-native-presentation-boundary.md).
