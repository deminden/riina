# Official IINA reference capture

The manual IINA 1.5.0 visual-reference workflow downloads the exact official DMG
recorded in reference/iina-release-lock.json, verifies its SHA-256 and bundle
signature, and checks its displayed version. It runs outside riina product code
on an ephemeral macOS runner. No binary, icon, font or reference implementation
is added to the MIT application.

The official CLI opens the original catalog.mkv fixture paused at a requested
two-second position. Capture tooling records theme, screen scale and visible
IINA window metadata. It checks existing screen-recording permission without
requesting it or modifying OS security settings. It captures only a single player
window whose title contains the fixture filename. Failure preserves diagnostic
metadata; it does not create a replacement or imagined reference image.

The artifact retains the screenshot and metadata for fourteen days, with the
OS version, exact reference version and fixture/release hashes. It contains
research evidence, not original riina product assets or a new MIT grant on
the reference interface. Keep it separate from assets/provenance.json.

A successful workflow is only a capture attempt. Inspect the image before adding
it to tests/visual/reference-captures.json: verify media content, playback position,
actual client bounds, controls visibility, theme, scale, default layout and any
permission/update/welcome overlays. No window resizing or layout customization
is performed, so the initial capture is not automatically a matching-size golden.
Additional states and measured comparison remain required for visual parity.

Sources: [official release](https://github.com/iina/iina/releases/tag/v1.5.0),
[Quartz window metadata](https://developer.apple.com/documentation/coregraphics/cgwindowlistcopywindowinfo(_:_:)),
and the CLI help in the pinned reference checkout. Capture tooling is independently
authored; it does not translate upstream UI implementation.
