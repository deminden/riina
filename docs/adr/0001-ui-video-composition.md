# 0001 · OpenGL SDR feasibility path (provisional)

Original spike: Rust 1.90.0; Slint/slint-build 1.13.1; glow 0.16.0; libloading 0.8.9;
windows 0.61.3; development libmpv mpv-winbuild-cmake 20261009 / mpv 36abaa32d0.

Use the documented libmpv OpenGL render API to draw into a GPU RGBA8 FBO and expose
its texture through Slint's BorrowedOpenGLTextureBuilder. UI draws controls in the same
window above the Image. No per-frame CPU readback. Event worker owns the mpv handle;
only render APIs run in Slint's current GL context. Update callbacks marshal wakeups;
they do not call mpv or GL. Render context dies before the worker/player.

Selected dependency docs/source: Slint v1.13.1 examples/opengl_texture/main.rs;
the downloaded crate features; pinned IINA mpv client.h/render.h/render_gl.h.
Borrowed texture is valid only in the creating context and remains alive until teardown.

This is not yet the production renderer decision. This RGBA8 GL surface is SDR.
The native gpu-next/D3D11 candidate now has an opt-in shared-UI route and observed
HDR swap-chain/lifecycle evidence in ADR 0002. Blur, color accuracy, WebView2 and
full window/DPI coverage remain open before selecting a production direction.
A child HWND alone is insufficient evidence.

Measurements/runtime evidence belong in status.md. No performance numbers are invented.

Pinned native-engine source acquired separately at mpv
`36abaa32d00a7229ee206aae12dc0e97e7962dca`; it does not alter the IINA baseline.
Its DOCS/man/options.rst documents d3d11-output-mode=composition as creating a
swapchain without a window, exposed by display-swapchain for host attachment.
Therefore a bare composition option is not an embedding implementation. With
gpu-next, rgba16f requests scRGB as a best-effort hint; libplacebo chooses the actual
format. Native HWND/DComp attachment and observed output format remain unimplemented
and unverified. Production direction remains provisional.
