# Platform differences and validation gaps

Windows 11 x64 is the supported application target. macOS and Linux application
ports remain in development. This page records deliberate adaptations and
unverified boundaries; [status](status.md) summarizes overall progress.

## Rendering and materials

| Area | Implementation or adaptation | Still needs verification |
|---|---|---|
| Windows video | Native D3D11 presentation is the default; `--shared-gl` selects SDR GL composition | HDR/color calibration, display changes, drivers and full DPI/monitor coverage |
| macOS/Linux video | Shared Slint/OpenGL application path behind portable presentation boundaries | Full native application playback, dialogs, fullscreen and shutdown acceptance |
| Windows window materials | Mica on persistent windows; Desktop Acrylic on selected auxiliary/transient windows | Visible pixels and policy transitions across windows; DWM attribute acceptance alone is insufficient |
| Playback materials | In-app video-backed blur/tint; native Windows sampling prefers window-scoped Graphics Capture with an mpv fallback | Motion synchronization, readback/GPU cost, nonuniform video, light appearance and matched reference pixels |
| macOS materials | Target AppKit Liquid Glass and semantic surface roles | macOS 27.0.1 native material rendering and matched captures |
| Linux materials | Available compositor material or documented in-app fallback | Desktop/compositor-specific runtime behavior |

Windows playback overlays must show video beneath them rather than desktop
wallpaper. SDR samples are bounded to 320 x 180; HDR uses a tint fallback. Grain
stays anchored to surface coordinates. Policy disables sampling and uses opaque
fills when required: advanced effects and High Contrast gate materials; Battery
Saver disables transient/sample-based Acrylic while leaving Mica eligible.
Unreadable policy uses the opaque fallback. Backdrop retry and theme handling
live in `crates/app/src/platform/windows/materials.rs`.

The separate Windows controls surface needs explicit alpha composition. In
fullscreen it is one physical scanline shorter than the full-size native video
host, avoiding the display blackout observed when revealing controls. Windowed
bounds restore exactly. See [ADR 0003](adr/0003-native-presentation-boundary.md)
for the ownership and composition invariants.

## Windows interaction adaptations

- Native window frames, file pickers and menus adapt Cocoa-owned surfaces. Popup
  menu appearance can follow the Windows system theme even when the player has
  an explicit theme; exact native menu pixels are not claimed.
- The floating controller is clamped to the video span unobscured by an open
  sidebar. IINA's edge-to-edge macOS controller can remain anchored to the full
  video container. Video still fills the selected viewport.
- Primary shortcuts use Command on macOS and Control on Windows/Linux.
  Command+Control track cycling maps to Control+Alt on Windows/Linux so the
  modifiers stay distinct. AltGr, alternate layouts and physical system-key
  routing require platform testing.
- Apple fonts, SF Symbols and restricted artwork are not bundled. Original
  licensed vectors and host typography preserve meaning and geometry; exact
  glyph rasterization remains platform-dependent.

## OS integration boundaries

| Feature | Current boundary and open evidence |
|---|---|
| File dialogs | Owned Windows picker has modality/cancellation evidence; acceptance, owner-close stress and each port still need coverage |
| File associations | Windows uses its Default Apps surface; macOS has bundle/UTI and Launch Services adapters; Linux uses user-scoped desktop/MIME registration. Platform consent and launch behavior require native checks |
| Recent documents | Portable persistence exists; complete native macOS document-menu integration remains open |
| Picture in Picture | Windows uses a native presentation adapter. macOS has an AVKit/IOSurface sample-buffer implementation that is not yet a verified runtime path; [ADR 0005](adr/0005-macos-picture-in-picture.md) |
| Plugin WebViews | Windows and Linux X11 presenters use confined package resources. Linux needs GTK/WebKitGTK; the current child-window adapter rejects Wayland. Native Mac/Linux and matched geometry remain open |
| OCR/system media | OS adapters exist; hardware input, recognition quality, accessibility and each port's runtime behavior need separate evidence |
| Accessibility/localization | Selected Windows controls and keyboard routes have evidence at 300% DPI; Narrator, IME, RTL/long strings, other scales and live locale changes remain incomplete |

## Plugin network policy

All plugin HTTP transports share one independent hardening policy:

- Revalidate allowed manifest hosts at each redirect.
- Remove authorization, cookies and related sensitive headers on an origin change
  (scheme, host or effective port).
- Reject HTTPS-to-HTTP redirects; explicit initial HTTP requests remain allowed
  for approved hosts.
- Bound redirects, deadlines and request/response sizes.

This policy is an intentional adaptation, not a claim of identical IINA
networking behavior. Native code, external helpers and plugin permissions retain
the trust boundaries described in the [product contract](product-spec.md).

## Evidence required for closure

Use real platform builds and observed behavior. Check audible/pass-through audio,
HDR/color output, material policy, display changes, accessibility and window
lifecycle separately. The full feature scope remains in [parity.csv](parity.csv);
implemented workflows must not be relabeled as missing simply because visual
comparison is still open.
