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

Volume changes do not feed the native host's Slint menu model. Its disabled
informational row is updated with `SetMenuItemInfoW` at `WM_INITMENUPOPUP`, keeping
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

This boundary does not implement another OS or certify its window/rendering
behavior. The portable core and mpv interface have three-OS unit/lint evidence;
native dialog, storage, presentation, fullscreen/focus and playback tests remain
required per port. The shared GL composition remains the provisional SDR route.
