# Differences and validation gaps

Plugin HTTP transports share one independent security policy across platforms:
redirects revalidate the manifest's allowed hosts, credentials are removed when
scheme/host/effective port changes, and HTTPS-to-HTTP redirects are rejected.
Explicit initial HTTP requests remain allowed for approved hosts. This is a
documented hardening choice, not a claim of identical IINA networking behavior.

Shared primary shortcuts use Command on macOS and Control on Windows/Linux.
IINA's Command+Control track-cycle shortcuts use Control+Alt on Windows/Linux,
because mapping both macOS modifiers to Control would lose their distinction.
Command+Control+A/S/V remain unchanged on macOS; Control+Alt+A/S/V cycle audio,
primary subtitle and video tracks on the other targets. Alternative layouts,
AltGr interaction and actual other-OS routing are not yet verified.

Windows 11 x64 MSVC is the sole supported application target. The shared core and
mpv interface tests/lint pass on Windows, macOS and Linux; full application ports
are near-term work. Other Windows versions and Windows ARM64 are untested.
Native Windows file dialog/titlebar adapt Cocoa surfaces. Client-area window controls
and native hit testing are pending. Owned Windows picker modality and cancellation have runtime evidence. GUI
acceptance, duplicate-open/owner-close stress and dispatch-failure coverage remain
open. Media preparation runs off the GUI thread after modality is restored.

The ordinary player still uses provisional SDR RGBA8 OpenGL composition. The native
D3D11 experiment now reports a 16-bit float scRGB swap chain and actual HDR desktop
mode, with separate source/target parameters and an interactive owned Slint overlay.
IINA's macOS path requests extended-range PQ with source BT.2020/P3 primaries; the
Windows candidate uses linear Rec.709 scRGB as the Windows compositor input. This
platform adaptation preserves an HDR-capable route but needs calibrated validation,
SDR UI luminance checks and faithful clipping/blur/sidebar integration. Source HDR
classification follows IINA; per-player HDR toggle and tone-mapping preferences
are tracked as missing. Live OS HDR mode changes during playback remain unverified.

The macOS package now carries the app-bundle and imported UTI metadata needed for
Launch Services to recognize media files, and Utilities registers selected file
categories. The implementation follows the pinned IINA bulk association action
through Launch Services' deprecated setter; Apple's current `NSWorkspace`
replacement is asynchronous and may require user consent. A current-macOS run is
still needed to verify its behavior. Linux now installs a user-scoped desktop
entry and applies selected MIME associations through `xdg-mime` on a worker; desktop-session
integration and launch behavior remain unverified.

Plugin sidebar pages use Wry child WebViews on Linux X11 and require GTK 3 and
WebKitGTK 4.1 development packages plus `pkg-config`. GTK initialization and event
processing run on the player UI thread. Wayland embedding still needs a GTK
container integration; the current child-window path rejects Wayland rather than
calling Wry's X11-only child API with an unsupported surface.

AppKit material blur, Apple font metrics, SF Symbols and macOS OS services are not
redistributed. Current colors and exact icon geometry remain estimates; original MIT vector
controls render on GL/native routes. Shared-OpenGL playback has a downsampled
sampled-video blur beneath the translucent sidebar, title row, floating controller,
seek/video/plugin OSDs, and fullscreen information card. The native Windows D3D11
path now requests `screenshot-raw` samples only while enabled material surfaces are
visible, throttled to at most 10 requests per second during playback and one per
paused position. Samples older than 750 ms are discarded instead of leaving stale
colors in the Acrylic layer. The player worker
bilinearly downsamples each SDR sample from the owned
RGBA buffer to at most 320×180 before retaining it in playback state. The UI blurs
that small sample, adds spatially stable grain, and supplies it to Slint; HDR keeps
the current tint because this sample path is SDR. It is an in-app video-backed Acrylic
approximation, not DWM Acrylic. Windows Graphics Capture can target one HWND via
[IGraphicsCaptureItemInterop::CreateForWindow](https://learn.microsoft.com/en-us/windows/win32/api/windows.graphics.capture.interop/nf-windows-graphics-capture-interop-igraphicscaptureiteminterop-createforwindow)
and exposes the compositor's QPC frame time through
[Direct3D11CaptureFrame.SystemRelativeTime](https://learn.microsoft.com/en-us/uwp/api/windows.graphics.capture.direct3d11captureframe?view=winrt-26100).
Evaluate it as a frame-synchronized source for native video materials; require
visible child-surface capture and bounded readback cost before replacing mpv's
current screenshot path. The native Windows Quick Settings sidebar uses an
80% sample layer to suppress sharp high-contrast video detail; other Windows
native-video surfaces retain 58%. The macOS native-video sidebar retains 53%;
shared-renderer sidebar layers remain at 32%. All native samples use an 8 px
Gaussian blur at the 320×180 sample size. Windows Quick Settings cards use WinUI's dark/light
`LayerOnAcrylicFillColorDefault` values (`#09FFFFFF` / `#40FFFFFF`) while the
sampled-video sidebar material is enabled; macOS and opaque fallback card fills
remain unchanged. This lets more video show through in light mode than the former
generic 53%-opaque card fill. The Windows sample-layer increase leaves the
independent tint calibration unchanged; verify high-contrast motion in a valid
capture. Earlier whole-desktop Windows Audio/Video captures confirm that playback
shows through the sidebar and card layers. A 4 fps color sequence still puts the sampled image
out of phase with the presented frame, and matched tint, light-theme and policy
fallback comparisons remain open. The Windows sidebar has 12 px rounded corners
with a subtle one-pixel edge.
The `Fit Window to Video` action also uses WinUI `ControlFillColorDefault` and
`ControlFillColorSecondary` in dark mode (`#0FFFFFFF` / `#15FFFFFF` ARGB), and
`ControlFillColorTertiary` / `ControlFillColorSecondary` in light mode
(`#4DF9F9F9` / `#80F9F9F9` ARGB), with theme-appropriate foregrounds while
sampled-video material is active. Shared and opaque fallback styling is
unchanged. These control pixels still need live Windows verification.
The controls popup is a DWM-composited transparent client. After Slint realizes
the window, Riina clears `WS_EX_TRANSPARENT` so the controls receive pointer
input, preserves `WS_EX_LAYERED` if the backend supplied it, and reapplies
`DwmEnableBlurBehindWindow`. This legacy call does not blur on Windows 8 and
later, but [Microsoft documents](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/nf-dwmapi-dwmenableblurbehindwindow)
that it still makes DWM honor the window's alpha values. Video blur remains the
sampled-video Acrylic approximation described above.
The native-video Acrylic composition is now visible in a full-desktop capture,
but GPU/readback cost, matched IINA pixels, system-policy fallbacks, and playback
transitions still need Windows GUI verification. Other-platform blur/material
appearance needs matched comparison.
Accessibility
inspection covers selected controls and keyboard workflows at 300% DPI. Text
entry, other DPI values and multi-display coverage remain incomplete. A live
native-video capture now shows the video visible through the Windows controls
overlay and the tinted Quick Settings material; matched reference pixels and
policy transitions remain open.

Material design follows each host OS. macOS uses IINA's Liquid Glass appearance
and the system's Liquid Glass material. Windows 11 uses Mica on the persistent
window backdrop and Acrylic on transient surfaces; video overlays must composite
against the playing video rather than reveal desktop wallpaper. Windows' advanced
effects setting and High Contrast gate both DWM backdrops; Battery Saver gates
Desktop Acrylic and the custom video-backed overlay while leaving Mica active.
Video surfaces switch to a solid fill and stop frame sampling when their material
policy disables Acrylic. If a required policy cannot be read, Riina uses the opaque
fallback. The persistent player host and registered auxiliary windows
refresh their DWM backdrop from the cached policy every 500 ms. Failed player-host
operations retry with increasing delays up to 30 seconds while preserving the opaque
client fallback; policy changes reset the delay. When effects are disabled, the host
switches to opaque immediately even if the native Winit handle is momentarily absent.
Auxiliary windows retry a DWM backdrop update while the client surface still disagrees
with the current policy, covering transient failures that occur without a policy change.
The retry delay grows to 30 seconds and identical failures are logged once until the
backdrop recovers or the policy changes.
Those auxiliary windows also set their Winit native theme from the Slint surface
palette: fixed light utility panes request light frames, the Welcome and Font
Picker request their resolved light/dark frames, and About/Video Filters follow
their live dark-theme property. The reusable Font Picker also follows the player's
resolved theme while open. This aligns DWM Acrylic tint and native caption controls
with client content; exact live appearance still needs a Windows capture.
The native Windows player host now enables an alpha client only after DWM accepts
the requested backdrop type and returns it on readback. This confirms the attribute,
not the pixels rendered under system policy. The host exposes the backdrop through
its transparent Slint surface while retaining a solid fallback if DWM rejects the
request. The Settings page and navigation bases, including the nested Online Media
page, use a low-opacity Windows content layer fill over Mica only while DWM accepts
the backdrop; Preferences card containers now use the same fill, while edit fields,
selected rows and controls retain solid fills. The original solid cards return when
Mica is unavailable. This uses the light
WinUI Mica layer fill value (`#ffffff80`). The Settings title remains “Translucent Surfaces”
because its three per-surface switches still change RGBA tint/opacity. The About window
requests DWM's transient Acrylic backdrop and exposes it through transparent content
panes only after DWM accepts the request; rejected requests keep solid fills. The Welcome
window uses Mica. Visible
system material pixels, Acrylic on the selected auxiliary windows, native-video
overlay materials, and macOS 27 material rendering remain runtime verification gaps.
Live DWM readback now confirms Mica for Settings/player windows and Desktop
Acrylic for About; this attribute evidence still does not confirm visible pixels.
The Video Filters, Playback History, Log Viewer, Online Subtitles and Font Picker
windows now expose a transparent root only after DWM accepts transient Acrylic.
Toolbar, table-header, and unselected list/backing surfaces use WinUI's dark/light
Acrylic layer values where applicable; data-entry fields and selected rows retain
solid contrast fills. The reusable Font Picker follows the resolved player theme
and switches between WinUI light/dark Acrylic layer fills, list colors and preview
contrast; its native frame follows through Winit (including AppKit appearance on
macOS). Selected rows remain opaque.
Rejected requests
keep the platform's solid fallback. The Windows PiP frame now requests transient
Acrylic once when it first opens; its video viewport stays black-backed beneath
the DWM thumbnail while the bottom control strip uses a tinted translucent fill.
Its solid colors remain when DWM rejects Acrylic. Renderer-backed
Windows video surfaces add subtle shader grain over their sampled blur as an
in-app Acrylic approximation. Native-overlay grain stays anchored to surface
coordinates across video frames to avoid texture shimmer. The native D3D11
playback overlay now samples a
downscaled SDR video frame while a translucent material surface is visible; this
keeps the video behind the overlay instead of using a desktop-derived backdrop.
The sampler follows Windows' advanced-effects and Battery Saver policy. Playback
cost and rendered material output remain unverified. The
Welcome requests Mica behind the WinUI dark `LayerFillColorDefault`
(`#4C3A3A3A` ARGB, expressed as `#3A3A3A4C` in Slint). Its opaque dark surface
remains when DWM rejects the request. The Settings content currently uses the
WinUI light layer fill (`#ffffff80`). These values follow the [WinUI theme
resources](https://github.com/microsoft/microsoft-ui-xaml/blob/main/controls/dev/CommonStyles/Common_themeresources_any.xaml)
and [Mica layering guidance](https://learn.microsoft.com/en-us/windows/apps/design/style/mica).
Visible output awaits capture.

Full playlist, track, chapter and subtitle parity, history, multiple windows, music/PiP, screenshots,
OCR, plugins/webviews, helpers, localization, other shell associations and media controls
are outstanding parity work, not removed scope. Preferences persist volume and opt-in volume boost
(default off, cap 100%; enabled cap 200%); dark/compact fields reserve validated defaults without claiming UI switches.

Basic chapter, primary track/subtitle and playlist editing workflows now have native
tests. Windows file picker supports multiple playlist files. Recursive folder startup,
extension filtering and owned modality are implemented. GUI folder selection,
acceptance and link/permission/resource-limit coverage remain open. Manual playlist transitions resume
at mpv file-loaded; upstream waits for window/video-size readiness. High-resolution
first-frame synchronization still needs a presentation-aware gate. Sidebar width/side
defaults now follow source (360, 80% cap; playlist leading/settings trailing); full
tab groups, dual panels, resize and animation remain missing.

Windows' on-screen controller is clamped to the unobscured video
span while a Quick Settings, playlist, or plugin sidebar overlays the
edge-to-edge video. IINA's macOS controller remains anchored to its full-width
edge-to-edge video container; this Windows adjustment keeps controller actions
visible above native acrylic sidebar content. The underlying video still fills
the full window. Verify controller bounds, hit targets, and drag fraction with
each sidebar position in a clean full-desktop capture.
