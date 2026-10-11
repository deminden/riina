# Third-party notices

Original riina Rust, Slint definitions, scripts and documentation are MIT; see
LICENSE. The combined application package is distributed under GPLv3 terms; see
LICENSE-GPL-3.0.txt. The riina-mpv ABI declarations retain the ISC grant below.
Third-party components retain their own licenses and notices. The Windows release
package includes the licenses collected from the exact native source checkouts in
THIRD_PARTY_LICENSES/, the locked Windows Rust dependency licenses and notices,
and the Slint Royalty-free 2.0 agreement; the corresponding source archive
includes those materials, the native source checkouts, locked Rust crate sources
and build recipes.

The Riina identity assets in `assets/branding/` retain the pack's MIT license.
The vector wordmark includes letter outlines based on Inter; its OFL 1.1 notice is
included alongside the brand guide. The Inter font file is not bundled. See
`assets/branding/THIRD_PARTY_NOTICES.md` for details.

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

Video sidebar requirements additionally reference SidebarVideoPane (Hechen
Li/lhc, 2026), AppData preset values and PlayerCore video actions at the same
pinned commit. The original Rust typed intent, native observations and Slint
controls are independently implemented; native AppKit code/artwork is not copied.

Local selection behavior and extension lists in crates/core/src/file_selection.rs
follow PlayerCore.openURLs (lhc, 2016), DraggingDetect.getPlayableFiles (Yuze Jiang,
2017; copyright lhc) and Utility (lhc, 2016), at the pinned commit above. Windows
Explorer natural comparison replaces Foundation's localized comparison; directory
expansion and alias resolution remain incomplete.

Slint 1.18.1/slint-build 1.18.1: select LicenseRef-Slint-Royalty-free-2.0 from
the crate's offered licenses. This Windows desktop application uses its section
2(b) attribution option: the official Slint badge is displayed in the public
repository README. Slint itself is not relicensed as MIT. The badge is linked
from the pinned v1.18.1 upstream resource rather than copied into riina assets.
OpenGL example source by SixtyFPS GmbH is MIT; inspected as interop guidance. Our
renderer implementation is independently written with mpv ABI declarations.
glow 0.18.0: MIT/Apache-2.0/Zlib; libloading 0.9.0: ISC;
serde/serde_json: MIT/Apache-2.0; windows 0.62.2: MIT/Apache-2.0.
The Windows resource build helper `winresource` 0.1.31 is MIT.
Cargo.lock pins the complete dependency graph. The source archive vendors the
locked crates with their original license files, alongside this notice and the
Unicode notice under third-party-licenses/.

regex 1.13.1 (https://github.com/rust-lang/regex): selected MIT option from
MIT OR Apache-2.0, Rust Project Developers. Already present in the dependency
graph; now a direct core dependency for Unicode decimal-digit classification.
The core requests only std and Unicode general-category support. Original
ratio syntax/intent logic remains independently implemented; third-party
license files are retained in the pinned development cache.

ICU4X icu_experimental 0.6.0 / icu_experimental_data 0.6.0 and icu_locale
2.3.1 / icu_locale_data 2.3.0: Unicode-3.0, Unicode, Inc. The complete notice is
retained in third-party-licenses/Unicode-3.0.txt. Compiled display-name data uses
CLDR 48.2.1 and ICU release-78.1rc (the pinned data crate's README). The lockfile
records additional ICU4X formatting/provider/data crates. These are Unicode
locale data, not copied IINA strings or Apple assets. The experimental formatter
is pinned; its locale patterns and macOS name equivalence remain unverified.

sys-locale 0.3.2 (https://github.com/1Password/sys-locale): selected MIT option
from MIT OR Apache-2.0. Already resolved through Slint, now used directly for
Linux display-locale discovery. Windows uses GetUserDefaultLocaleName and macOS
uses Foundation's localized locale-identifier API.

fontdb 0.24.0 (https://github.com/RazrFalcon/fontdb): MIT, copyright 2020
Yevhenii Reizner. Promoted from the existing rendering dependency graph to a
direct dependency for installed-font metadata discovery. The pinned registry
license is retained in the development dependency cache; no system font files
are bundled or copied into riina. Desktop font resolution and any font packaging
require separate verification.

RQuickJS 0.14.0, rquickjs-core 0.14.0 and rquickjs-sys 0.14.0 (https://github.com/DelSkayn/rquickjs): MIT. The embedded QuickJS source and its upstream LICENSE are also MIT. The runtime is used only for permission-reviewed plugin scripts; platform, filesystem and network APIs are not enabled by this integration. Cargo.lock pins the Rust crates, and the source package retains their license files.

ttf-parser 0.25.1 (https://github.com/harfbuzz/ttf-parser): selected MIT option
from MIT OR Apache-2.0, copyright 2018 Yevhenii Reizner. Already present through
the font dependency graph; now used directly to read face-label metadata.
The registry MIT grant is retained. Font data is inspected locally during
discovery, not copied into riina's source or distribution.

rfd 0.17.2 (https://github.com/PolyMeilex/rfd): MIT, copyright 2022
Bartłomiej Maryńczak. Its portal window-identifier module retains the MIT notice
of Bilal Elmoussaoui (2020). Used only by macOS/Linux application backends;
macOS uses native panels and Linux uses XDG Desktop Portal with Wayland support.
dirs 7.0.0 and dirs-sys 0.5.0: MIT OR Apache-2.0, used for those platforms'
configuration directories. Exact resolved packages and checksums are in Cargo.lock.
Their complete license texts remain with the locked Rust sources in the source
archive. The Windows package carries the applicable native license files and
this consolidated notice.

objc2 0.6.5 and objc2-foundation 0.3.2: MIT. Direct macOS-only dependencies
for Foundation's native filename ordering. Calls use their safe retained-string
and autorelease-pool APIs; no Foundation implementation source is copied.

mpv public ABI headers: Copyright (C) 2018 the mpv developers (render_gl.h), ISC
permission and warranty disclaimer retained below. The Windows release rebuilds
mpv and FFmpeg from the pinned builder recipe at
`b2856ee6deed6957380b32465f359e67f67fafd8`. FFmpeg's `--enable-version3` remains
enabled; its optional x264 encoder dependency and flag are removed by the included
patch because x264 is GPLv2-only. FFmpeg's built-in H.264 decoder remains enabled.
The package carries the exact native source checkouts and their license files;
`SOURCE-MANIFEST.tsv` records each source revision and source-file hash.
`RUNTIME-MANIFEST.tsv` records the packaged engine files and hashes. The engine
build and native playback tests must succeed before the workflow publishes assets.

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

Screenshot clipboard integration uses arboard 3.6.1 (MIT OR Apache-2.0; MIT
selected). The nine newly locked clipboard/Wayland dependency notices are retained
under licenses/ and versions/features/hashes are recorded in
docs/clipboard-dependencies.json. tree_magic_mini loads the desktop MIME database
at runtime; its optional GPL database embedding feature is not enabled. Combined
distribution and non-Windows runtime remain subject to the existing audit gates.
