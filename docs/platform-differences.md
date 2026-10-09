# Differences and validation gaps

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

AppKit material blur, Apple font metrics, SF Symbols and macOS OS services are not
redistributed. Current colors and exact icon geometry remain estimates; original MIT vector
controls render on GL/native routes. Sampled-video blur is absent. Accessibility
inspection covers selected controls and keyboard workflows at 300% DPI. Text
entry, other DPI values and multi-display coverage remain incomplete.

Full playlist, track, chapter and subtitle parity, history, multiple windows, music/PiP, screenshots,
OCR, plugins/webviews, helpers, localization, shell associations and media controls
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
