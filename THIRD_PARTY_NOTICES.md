# Third-party notices · development audit

Original riina Rust, Slint definitions, scripts and documentation are MIT; see
LICENSE. The riina-mpv ABI declarations retain the ISC grant below. This grants
rights to riina's original contributions, not to third-party code or assets.
Dependency licenses and the combined binary's distribution obligations are
separate from the MIT license on original source. See docs/licensing.md.

IINA v1.5.0, https://github.com/iina/iina, commit fbcff4df7183b335880396af0655f6bb89d46442:
GPL version 3 text. Inspected headers attribute PlayerCore/MPVController to lhc (2016),
OSCFloatingView/LayoutValue to Hechen Li/lhc (2026). Geometry/defaults/behavior are
recorded as behavioral references in docs/parity.csv. The excluded checkout keeps
its GPL license; it is not part of the MIT grant. No Apple artwork or fonts are
bundled. These source credits identify compatibility research, not permission to
translate protected implementation code into MIT-licensed code.

Additional behavior/layout references: SidebarTrackSelector, SidebarAudioPane,
SidebarSubtitlesPane, SidebarPlaylistPane and SidebarChaptersPane by Hechen Li/lhc
(2026); MPVTrack by lhc (2016); MPVCommandWrappers by Yuze Jiang (2025).
Command plans and track labels in crates/core/src/lib.rs follow the pinned sources;
Rust implementations preserve explicit mpv ownership and have independent tests.

Local selection behavior and extension lists in crates/core/src/file_selection.rs
follow PlayerCore.openURLs (lhc, 2016), DraggingDetect.getPlayableFiles (Yuze Jiang,
2017; copyright lhc) and Utility (lhc, 2016), at the pinned commit above. Windows
Explorer natural comparison replaces Foundation's localized comparison; directory
expansion and alias resolution remain incomplete.

Slint 1.13.1/slint-build 1.13.1: select LicenseRef-Slint-Royalty-free-2.0 from
the crate's offered licenses. This Windows desktop application uses its section
2(b) attribution option: the official Slint badge is displayed in the public
repository README. Slint itself is not relicensed as MIT. The badge is linked
from the pinned v1.13.1 upstream resource rather than copied into riina assets.
OpenGL example source by SixtyFPS GmbH is MIT; inspected as interop guidance. Our
renderer implementation is independently written with mpv ABI declarations.
glow 0.16.0: MIT/Apache-2.0/Zlib; libloading 0.8.9: ISC;
serde/serde_json: MIT/Apache-2.0; windows 0.61.3: MIT/Apache-2.0.
Resolved transitive dependency licenses must be inventoried before distribution.

ICU4X icu_experimental 0.6.0 / icu_experimental_data 0.6.0 and icu_locale
2.3.0 / icu_locale_data 2.3.0: Unicode-3.0, Unicode, Inc. The complete notice is
retained in third-party-licenses/Unicode-3.0.txt. Compiled display-name data uses
CLDR 48.2.1 and ICU release-78.1rc (the pinned data crate's README). The lockfile
records additional ICU4X formatting/provider/data crates. These are Unicode
locale data, not copied IINA strings or Apple assets. The experimental formatter
is pinned; its locale patterns and macOS name equivalence remain unverified.

sys-locale 0.3.2 (https://github.com/1Password/sys-locale): selected MIT option
from MIT OR Apache-2.0. Already resolved through Slint, now used directly for
Linux display-locale discovery. Windows uses GetUserDefaultLocaleName and macOS
uses Foundation's localized locale-identifier API.

fontdb 0.23.0 (https://github.com/RazrFalcon/fontdb): MIT, copyright 2020
Yevhenii Reizner. Promoted from the existing rendering dependency graph to a
direct dependency for installed-font metadata discovery. The pinned registry
license is retained in the development dependency cache; no system font files
are bundled or copied into riina. Desktop font resolution and any font packaging
require separate verification.

ttf-parser 0.25.1 (https://github.com/harfbuzz/ttf-parser): selected MIT option
from MIT OR Apache-2.0, copyright 2018 Yevhenii Reizner. Already present through
the font dependency graph; now used directly to read face-label metadata.
The registry MIT grant is retained. Font data is inspected locally during
discovery, not copied into riina's source or distribution.

rfd 0.17.2 (https://github.com/PolyMeilex/rfd): MIT, copyright 2022
Bartłomiej Maryńczak. Its portal window-identifier module retains the MIT notice
of Bilal Elmoussaoui (2020). Used only by macOS/Linux application backends;
macOS uses native panels and Linux uses XDG Desktop Portal with Wayland support.
dirs 6.0.0 and dirs-sys 0.5.0: MIT OR Apache-2.0, used for those platforms'
configuration directories. Exact resolved packages and checksums are in Cargo.lock.
Their complete license texts remain in the dependency sources and must accompany
any applicable binary distribution; no native release package is produced here.

objc2 0.6.5 and objc2-foundation 0.3.2: MIT. Direct macOS-only dependencies
for Foundation's native filename ordering. Calls use their safe retained-string
and autorelease-pool APIs; no Foundation implementation source is copied.

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
The gamma/primaries combinations are compatibility facts; the MIT grant is for
riina's original implementation, not for VideoView.swift. Windows scRGB presentation
is an independent platform implementation; no Apple implementation or artwork is bundled.

Player vector icons under assets/icons are original riina geometric artwork,
MIT under the root LICENSE. Their authorship and exact hashes are recorded in
assets/provenance.json and checked by scripts/check-assets.py. They do not
contain copied SF Symbols or other Apple artwork. Exact reference matching
remains unverified.
