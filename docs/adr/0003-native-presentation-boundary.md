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

This boundary does not implement another OS or certify its window/rendering
behavior. The portable core and mpv interface have three-OS unit/lint evidence;
native dialog, storage, presentation, fullscreen/focus and playback tests remain
required per port. The shared GL composition remains the provisional SDR route.
