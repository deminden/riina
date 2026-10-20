# Native presentation boundary

Accepted 2026-10-10. Windows is implemented; macOS/Linux backends remain open.

The shared controller must not know whether a native target is an HWND, an NSView
or another platform surface. Passing a Windows integer into configure_player made
that implementation detail part of every future port's interface.

The platform module now supplies an opaque NativePresentation. Its platform-only
factory obtains a real, realized window on the event thread and validates its
native handle. The shared controller asks it to start the engine and obtain the
host's shared UI reference. Native ownership and handle conversion stay inside
the backend. Windows owner-popup creation can access the private owner identifier;
the shared controller cannot.

UiSession retains the presentation after its player field. Native callbacks hold
command senders, not player owners. Shutdown joins the sole native player worker
before releasing the retained window. Both ordinary field destruction and the
explicit native close path must preserve that ordering. New backends must retain
their actual native surface until their decoder/presentation worker has stopped.

Native diagnostics use the application-data directory. They must not require the
build machine's source checkout to remain present. Library discovery stays behind
the platform module; native redistribution and packaging are separate open gates.

The Windows menu subclass consumes `WM_SETREDRAW` on the native playback host.
Slint 1.18.1/Muda uses it to batch menu rebuilds, including volume-label changes
and fullscreen menu visibility. The default Windows handler removes/restores
`WS_VISIBLE`, also changing visibility of mpv's child surface. Menu batching must
not interrupt native video presentation. The host has no Slint client controls
requiring redraw suppression; Muda still redraws its menu after rebuilding it.
See [Microsoft's WM_SETREDRAW contract](https://learn.microsoft.com/en-us/windows/win32/gdi/wm-setredraw).
Actual hide, minimize and close operations keep their normal lifecycle.

The opt-in package smoke runs inside the actual native application rather than
loading only its DLL in a headless test. A retained timer dispatches typed player
actions; it verifies the realized host, controls and visible mpv child, stable
menu/client geometry on volume updates, fullscreen extent and controller reveal,
and windowed restoration. The outer script owns an isolated profile and fixture,
validates the report's process/executable identity, waits for normal exit, and
checks that session shutdown preserved newer preferences. Timeout cleanup targets
only its own process. This driver does not certify physical compositor pixels,
audible output, or HDR.

Volume, speed and delay changes do not feed the native host's Slint menu model.
Their disabled information rows are updated with `SetMenuItemInfoW` at
`WM_INITMENUPOPUP`, keeping
the root/submenu handles, command IDs and client bounds stable during playback.
Fullscreen and Music Mode use `SetMenu` to detach/restore the same Muda-owned root
menu. The Slint tree remains alive while detached, preserving its native lifetime.
The controls popup queues a native redraw when restored after geometry alignment.

In fullscreen, the controls popup is one physical scanline shorter than the
native video host. The host and mpv child keep the full monitor bounds. A
monitor-sized WGL surface can lose alpha composition on the physical Windows
display even when desktop captures show the correct composited pixels. The
bounded framebuffer probe confirmed zero alpha outside controls with them both
visible and hidden (eight alpha bits, default framebuffer). The user reproduced
black video on controller reveal with the original 3840×2160 popup, then confirmed
that video stayed visible with the 3840×2159 popup. This is a Windows presentation
workaround; it leaves the last video scanline uncovered, affects control layout
by at most one physical pixel, and restores exact popup bounds on fullscreen exit.
No video resize, SDR conversion, or native-engine change is involved.
The [GLFW maintainer's direct experiment](https://discourse.glfw.org/t/black-screen-when-setting-window-to-transparent-and-size-to-1920x1080/2585/5)
independently documents alpha failure for a monitor-sized undecorated OpenGL
window and avoiding that driver path by changing its extent.

Music Mode retains the same host and controls ownership. Its mpv child occupies
only the artwork region above the current compact controls and optional playlist;
the controls popup still covers the whole client. Hidden artwork uses a retained
one-pixel child placed wholly outside the parent's client clip. Track selection,
decoder lifetime and playback continue; returning to the main player restores
the full client region. The alignment retry checks that requested region rather
than forcing every decoder child back to the full window. The fullscreen
one-scanline controls workaround remains independent from this video region.

The framed host has no controls FocusScope. When it owns keyboard focus in
fullscreen, its input adapter sends Escape through the configured shortcut
dispatcher before using the shared fullscreen-exit fallback. Crop editors and
dialogs retain their own input scope. Modifier state is local to the host and
cleared on focus loss; handled events do not propagate a second time.

Keyboard system-menu entry belongs to the framed owner. A retained controls
subclass forwards `SC_KEYMENU` with its original character after the controls
HWND is realized. Pending requests coalesce while Windows owns menu input. The
[menu-exit message](https://learn.microsoft.com/en-us/windows/win32/menurc/wm-exitmenuloop)
queues generation-checked restoration; a closed popup alone is not loop exit.
While this owner/controls pair retains foreground, the
[current physical Alt state](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getasynckeystate)
determines whether Slint needs an internal Alt release. A held Alt, unrelated
modifiers and dialog focus remain intact; no keyboard input is injected.

This boundary does not implement another OS or certify its window/rendering
behavior. The portable core and mpv interface have three-OS unit/lint evidence;
native dialog, storage, presentation, fullscreen/focus and playback tests remain
required per port. The shared GL composition remains the provisional SDR route.

## Resize ownership

Resize the top-level host and owned controls together, then apply matching bounds
to mpv's child windows in their own parent group before returning to the event loop.
[DeferWindowPos requires a single parent per batch](https://learn.microsoft.com/windows/win32/api/winuser/nf-winuser-deferwindowpos);
including the video child in the top-level batch is invalid. Hidden native-window
regressions exercise the owner, owned popup and child geometry without taking focus.

Music Mode owns its compact frame. Attached artwork can supply video dimensions,
but those dimensions must not activate the main video's initial/manual size policy.
The floating controller, compact controls and native playback surface retain their
separate layout and lifecycle boundaries.
