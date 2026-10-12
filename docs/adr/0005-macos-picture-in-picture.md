# macOS Picture-in-Picture surface

Status: unresolved; a native macOS PiP implementation is still required.

## Reference behavior

IINA v1.5.0 transfers its existing video view to a Picture-in-Picture controller,
keeps playback state synchronized, applies the configured main-window behavior,
and restores the player window when PiP closes. Closing PiP does not pause media;
the PiP Stop action pauses it. The pinned IINA project links Apple's private
`PIP.framework`, so that implementation detail cannot be carried into Riina.

## Current Riina boundary

The macOS application currently renders libmpv into a borrowed OpenGL texture in
the Slint player window. The pinned libmpv render contract permits only one render
context per player core and requires every OpenGL render call to use the context
that created that render context. Creating another window and passing it the same
texture is therefore not a supported multi-window design without a proven shared
context and resource lifetime. Slint 1.18.1's pinned Winit/FemtoVG backend creates
a new GL display context for each window; it does not request a shared context.
The borrowed texture cannot be assumed to work in a second Slint window either.
`riina-mpv` now exposes a same-context render operation for a caller-owned,
complete framebuffer. This preserves the single mpv render context and can target
GPU-backed storage such as an IOSurface without reading pixels back to the CPU.
The API is only a rendering seam. Riina now has an AVKit
`AVSampleBufferDisplayLayer` content source, playback delegate, and controller
owner that remains alive for the player session. The delegate routes play, pause,
and skip requests into the existing player, reports duration and paused state,
and refreshes AVKit playback controls when player state changes. Unknown duration
is reported as live playback. Starting PiP is gated on a real sample having been
enqueued. `pip_frame.rs` now creates a three-buffer IOSurface-backed Core Video
pool, binds each surface to a temporary rectangle texture in the existing CGL
context, renders through the same libmpv surface, and wraps it in a timed
`CMSampleBuffer`. The player rendering notifier feeds this producer, limits
active samples to about 30 fps and inactive refreshes to one per second, and
resets the source after completed seeks. Frames are capped at 640 pixels on the
long edge. This source type-checks for `x86_64-apple-darwin`; it has not been
linked or run on a Mac, so it is not yet a verified or supported PiP path.

Apple's public `AVPictureInPictureController` accepts an `AVPlayerLayer` or an
`AVSampleBufferDisplayLayer` as its content source. Riina now supplies the
sample-buffer layer and timed producer described above; the borrowed Slint
texture alone is not a media source. The bridge also supplies a playback
delegate. For macOS 14 and later, submit buffers through the layer's
`sampleBufferRenderer.enqueueSampleBuffer` API; Apple discourages the older layer
`enqueueSampleBuffer` entry point. The producer bounds queued buffers using the
renderer readiness state and flushes it across seek discontinuities.

The current bridge experiment binds an IOSurface-backed Core Video pixel
buffer directly to a framebuffer in the existing CGL context. Apple documents
the empty `kCVPixelBufferIOSurfacePropertiesKey` dictionary for requesting this
backing. Do not use `CVOpenGLTextureCache`: Apple [marks that API deprecated](https://developer.apple.com/documentation/corevideo/cvopengltexturecache-780). The
direct CGL path is an experiment against the app's existing OpenGL renderer, not
a claim that OpenGL is an appropriate long-term macOS rendering backend. If the
current macOS target cannot run that path, renderer migration is part of the
port rather than a reason to fall back to per-frame CPU copies.

## Required implementation experiment

Keep the existing single-renderer path until a macOS experiment demonstrates a
public-API PiP surface that displays the same decoder output, preserves timestamps,
color/HDR metadata, and playback controls, and restores the main window correctly.
Evaluate a GPU-backed IOSurface/sample-buffer bridge and the documented Cocoa
video-output path. Measure frame pacing and GPU copies. Do not use per-frame CPU
readback, create a second unsynchronized player, or present a static image as PiP.

The lifecycle contract is already shared: entering PiP honors the configured
window action and optional pause; closing restores only state changed by that
session without changing playback; the explicit Stop action pauses. The macOS
surface policy/restoration, system controls, minimize-to-toggle behavior, screen
movement, paused-frame restoration, HDR and color handling, accessibility, and
matched IINA captures remain unimplemented or unverified until the experiment
succeeds. The current BGRA8 producer does not preserve HDR metadata.

## Primary references

- Pinned IINA v1.5.0: `reference/iina-upstream/iina/MainWindowController.swift`,
  `reference/iina-upstream/iina/VideoPIPViewController.swift`, and
  `reference/iina-upstream/iina.xcodeproj/project.pbxproj`.
- Pinned libmpv render contract: `tools/mpv/include/mpv/render.h`, “Context and
  handle lifecycle” and OpenGL threading requirements.
- Slint 1.18.1 Winit/FemtoVG backend: `i-slint-backend-winit`'s
  `renderer/femtovg/glcontext.rs` creates a GL context per window without a
  shared-context handle.
- [Apple AVPictureInPictureController](https://developer.apple.com/documentation/avkit/avpictureinpicturecontroller)
  and [AVPictureInPictureController.ContentSource](https://developer.apple.com/documentation/avkit/avpictureinpicturecontroller/contentsource-swift.class).
- [Apple AVSampleBufferVideoRenderer](https://developer.apple.com/documentation/avfoundation/avsamplebuffervideorenderer)
  and [AVSampleBufferDisplayLayer enqueue guidance](https://developer.apple.com/documentation/avfoundation/avsamplebufferdisplaylayer/enqueue%28_%3A%29).
- [Apple kCVPixelBufferIOSurfacePropertiesKey](https://developer.apple.com/documentation/CoreVideo/kCVPixelBufferIOSurfacePropertiesKey)
