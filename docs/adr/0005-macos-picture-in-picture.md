# macOS Picture-in-Picture surface

Status: implementation experiment present; full Mac application build and
visible PiP/lifecycle verification are still required. This is not a supported
runtime path.

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
enqueued. The session publishes player state after AVKit confirms start or
stop. A start is cancellable and times out when AVKit leaves the source
unavailable. `pip_frame.rs` creates a three-buffer IOSurface-backed Core Video
pool, attaches a surface to a framebuffer in the player's existing CGL context, and
wraps each output in a timed `CMSampleBuffer`. The player renders about 30
samples per second while opening or active. It flushes stale samples on media
replacement and completed seeks. Frames are capped at 640 pixels on the long
edge.

When the player is occluded, hidden, or minimized, a main-thread timer borrows
the same libmpv surface, makes its CGL context current for the IOSurface draw,
and restores the previous context afterward. The renderer notifier still owns
the context and retires the offscreen source during rendering teardown. The
path passes Apple Silicon source checks. Linking and playback on macOS remain
open.

Apple's public `AVPictureInPictureController` accepts an `AVPlayerLayer` or an
`AVSampleBufferDisplayLayer` as its content source. Riina now supplies the
sample-buffer layer and timed producer described above; the borrowed Slint
texture alone is not a media source. The bridge also supplies a playback
delegate. macOS 14 and later use the layer's `sampleBufferRenderer` APIs.
macOS 12 and 13 use the older layer queue APIs. Riina checks runtime
availability and handles a controller that rejects the content source.
The producer bounds queued buffers using the renderer readiness state.
[Apple controller initialization](https://developer.apple.com/documentation/avkit/avpictureinpicturecontroller/init%28contentsource%3A%29)

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

The macOS adapter uses the shared window policy after confirmed entry. AppKit
handles hide, minimize, and restore. The AVKit Return control requests the
owning player window. Minimize-to-toggle listens to the player's own AppKit
notifications and tracks whether PiP initiated the minimization. Closing the
player stops its PiP session before releasing the renderer. The BGRA8
producer discards HDR metadata. Window policy, native controls, frame pacing,
screen movement, accessibility, and matched IINA captures require Mac runtime
verification.

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
