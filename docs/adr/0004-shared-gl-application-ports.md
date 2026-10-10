# Shared GL application ports

Accepted 2026-10-10. macOS/Linux application builds are under validation;
runtime support is not established.

The shared Slint UI, controller and GL composition should build before native HDR
backends are added. The dialog Purpose belongs to shared application behavior;
Windows retains its existing owned COM picker, while macOS/Linux use pinned
rfd 0.17.2 asynchronous native panels/desktop portals. The realized parent window
is retained through the request. The shared dialog-open gate prevents duplicate
requests and parent close while a picker is pending. Acceptance uses the same
background discovery and typed commands as Windows.

Preferences use dirs 7.0.0 configuration-directory discovery: macOS Application
Support and Linux XDG configuration locations. POSIX rename replaces a fully
written, synchronized temporary file; Linux also synchronizes its parent directory.
Replacement files are created exclusively with a process/sequence name so
concurrent sessions cannot truncate each other's pending writes. A cleanup guard
removes only the file successfully created by that save if replacement fails.
Native tests cover simultaneous Unicode saves and failed destination replacement.
Windows keeps its existing atomic replacement and legacy migration behavior.
No preference migration from IINA is implied.

libmpv discovery checks adjacent versioned libraries, macOS bundle Frameworks and
standard Homebrew library directories, then the platform loader's versioned name.
An installed libmpv must satisfy the existing ABI check; dependency installation,
codec provenance and native packaging remain separate gates.

NativePresentation is uninhabited on these targets until a real native surface and
its ownership/lifecycle are implemented. This avoids fabricated handles and allows
the actual shared GL player to compile. --native-video gives a clear unsupported
route error on these systems. The D3D11 probe is isolated to Windows.

Linux's provisional numeric filename comparator handles arbitrary-length ASCII
digit runs without integer overflow and preserves Unicode characters; localized
collation remains a Linux parity gap. macOS now calls Foundation's
localizedStandardCompare through pinned objc2 bindings, with an autorelease pool
on the discovery worker. Its native worker/Unicode ordering test awaits macOS CI.
Portal absence/failure reporting, modal input routing, focus restoration and
native panel cancellation need runtime checks. rfd's optional result does not
distinguish every backend failure from user cancellation.

Application CI now lints all targets, runs workspace unit tests and links the full
application on macOS/Linux. This must succeed before claiming build portability.
Playback, dialogs, fullscreen, GL surface lifecycle, accessibility, DPI, graceful
shutdown, HDR and reference visual comparison still require real runtime evidence.

API references: [rfd async dialog](https://docs.rs/rfd/0.17.2/rfd/struct.AsyncFileDialog.html),
[dirs configuration directory](https://docs.rs/dirs/7.0.0/dirs/fn.config_dir.html),
[XDG base directories](https://specifications.freedesktop.org/basedir/0.8/).
Filename sorting: [Foundation localizedStandardCompare](https://developer.apple.com/documentation/foundation/nsstring/localizedstandardcompare(_:)),
[objc2 NSString](https://docs.rs/objc2-foundation/0.3.2/objc2_foundation/struct.NSString.html).
