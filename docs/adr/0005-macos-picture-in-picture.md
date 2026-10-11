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

Apple's public `AVPictureInPictureController` accepts an `AVPlayerLayer` or an
`AVSampleBufferDisplayLayer` as its content source. Riina currently supplies
neither. `AVSampleBufferDisplayLayer` would need timed sample buffers and a
playback delegate; the current borrowed Slint texture is not that media source.

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
surface, system controls, minimize-to-toggle behavior, screen movement, paused
frame restoration, HDR handling, accessibility, and matched IINA captures remain
unimplemented or unverified until the experiment succeeds.

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
