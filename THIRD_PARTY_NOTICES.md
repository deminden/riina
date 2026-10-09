# Third-party notices · development audit

wiina application code is GPL-3.0-only; see LICENSE. Independent implementation.

IINA v1.5.0, https://github.com/iina/iina, commit fbcff4df7183b335880396af0655f6bb89d46442:
GPL version 3 text. Inspected headers attribute PlayerCore/MPVController to lhc (2016),
OSCFloatingView/LayoutValue to Hechen Li/lhc (2026). Geometry/defaults/behavior are
reimplemented with those files recorded in docs/parity.csv. No Apple artwork or fonts
are bundled. Confirm project-wide grants before redistribution; the generic license
appendix is not itself an assertion of an or-later grant.

Additional behavior/layout adaptations: SidebarTrackSelector, SidebarAudioPane,
SidebarSubtitlesPane, SidebarPlaylistPane and SidebarChaptersPane by Hechen Li/lhc
(2026); MPVTrack by lhc (2016); MPVCommandWrappers by Yuze Jiang (2025).
Command plans and track labels in crates/core/src/lib.rs follow the pinned sources;
Rust implementations preserve explicit mpv ownership and have independent tests.

Local selection behavior and extension lists in crates/app/src/file_selection.rs
follow PlayerCore.openURLs (lhc, 2016), DraggingDetect.getPlayableFiles (Yuze Jiang,
2017; copyright lhc) and Utility (lhc, 2016), at the pinned commit above. Windows
Explorer natural comparison replaces Foundation's localized comparison; directory
expansion and alias resolution remain incomplete.

Slint 1.13.1/slint-build 1.13.1: select GPL-3.0-only from the crate's offered licenses.
OpenGL example source by SixtyFPS GmbH is MIT; inspected as interop guidance. Our
renderer implementation is independently written with mpv ABI declarations.
glow 0.16.0: MIT/Apache-2.0/Zlib; libloading 0.8.9: ISC;
serde/serde_json: MIT/Apache-2.0; windows 0.61.3: MIT/Apache-2.0.
Resolved transitive dependency licenses must be inventoried before distribution.

mpv public ABI headers: Copyright (C) 2018 the mpv developers (render_gl.h), ISC
permission and warranty disclaimer retained in excluded reference source. The
development DLL is from shinchiro/mpv-winbuild-cmake, tag 20261009, mpv git 36abaa32d0.
Archive SHA-256 is pinned against the GitHub release API digest, which is not an
independently signed attestation. Full FFmpeg/codec/native dependency configuration,
license texts, source revisions and reproducible corresponding-source instructions
remain pending. Do not redistribute this DLL as a release package yet.

Permission notice for ABI definitions adapted from mpv headers:

Copyright (C) 2017, 2018 the mpv developers

Permission to use, copy, modify, and/or distribute this software for any
purpose with or without fee is hereby granted, provided that the above
copyright notice and this permission notice appear in all copies.

THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

Workspace-local Rust/Microsoft SDK/CRT/7-Zip tools are excluded development dependencies.
Microsoft payload integrity was checked against Microsoft's VS manifest SHA-256 values.
No system installation or policy changes were made. They are not application assets.

HDR source classification in crates/core/src/lib.rs adapts the behavior of
IINA v1.5.0 iina/VideoView.swift, VideoView.isHDRVideo (Copyright © 2016 lhc).
The same pinned GPL-3.0-only licensing plan applies. Windows scRGB presentation
is an independent platform adaptation; no Apple implementation or artwork is bundled.
