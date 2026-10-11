# MIT source and GPLv3 application distribution

Riina's independently written source remains MIT licensed by the root LICENSE
and package metadata. The combined application package uses GPLv3 terms; the
complete text is in LICENSE-GPL-3.0.txt. This does not change third-party
component licenses or grant permission to relicense them. The distribution choice
is settled. The release candidate workflow verifies that the
exact candidate includes its notices and corresponding source and passes package
and playback checks before publication.

The source license and combined-package terms describe different scopes: the
former grants rights to Riina's original source; the latter governs redistribution
of the application assembled with its native engine and dependencies. A GPLv3
package must include its license text, applicable third-party notices, and the
required corresponding source and build information for that exact package.

## Implementation policy

- Use IINA to establish observable behavior, defaults, compatibility facts and
  expected test results. Keep the pinned reference outside the product source.
- Write original Rust architecture and algorithms from those requirements and
  documented libmpv, Windows and Slint interfaces. Do not mechanically port Swift
  methods, data structures, XIBs or source assets.
- Preserve compatibility/provenance records. If protected IINA expression is
  found in riina, the MIT grant does not cover it: replace it with an independent
  implementation or obtain the appropriate permission.
- Keep full portable parity as the target. Independent implementation must not
  mean dropping functionality or redesigning the interface.

The workspace began empty. Its Rust command queue, observed-state model,
Windows COM ownership, native HWND lifecycle and Slint rendering integration
were written here. The reference checkout and Apple artwork are excluded from
publication. Provenance notes are maintained separately in
[upstream-audit.md](upstream-audit.md); they do not change the settled MIT source
and GPLv3 application package terms.

## Dependency and data licenses

| Scope | Terms |
|---|---|
| Original riina Rust/Slint definitions, scripts and documentation | MIT, root LICENSE and Cargo package metadata |
| Combined application package | GPLv3 terms, LICENSE-GPL-3.0.txt |
| mpv ABI definitions | Retained ISC notice in THIRD_PARTY_NOTICES.md |
| Original synthetic fixture bytes | CC0-1.0, fixture README and manifest |
| Excluded IINA reference | Existing GPL terms, outside the MIT grant |
| Slint 1.18.1 and slint-build | Royalty-free 2.0, public README badge, section 2(b) |
| ICU4X locale/display-name code and compiled Unicode data | Unicode-3.0, notice in third-party-licenses/Unicode-3.0.txt |
| Native `subrandr` Rust dependency `unicode-ident` 1.0.13 | Unicode-DFS-2016; retain its package notice in the native source bundle |
| Chrono 0.4.45 and battery 0.7.8 | MIT OR Apache-2.0; Rust dependency notices are generated from Cargo.lock |
| Development libmpv DLL and native dependencies | Local development input; release workflow rebuilds a separate engine |

Slint itself has not been relicensed. Its royalty-free license permits desktop
applications with attribution. The public README displays the official badge
from the pinned v1.18.1 resource. This avoids selecting Slint's GPL option for
the Windows application. Future download pages must retain compliant attribution.

The GPLv3 package route is the settled release choice. The pinned development
libmpv build is kept separate from release packaging. Its producing recipe enables
FFmpeg's GPL mode,
`--enable-version3`, OpenSSL, x264, x265 and dvdnav. FFmpeg's pinned configure
upgrades the GPL libraries to GPLv3-or-later and accepts OpenSSL only when it
detects OpenSSL 3 and GPLv3 mode. OpenSSL 3 is Apache-2.0 licensed, which the FSF
identifies as compatible with GPLv3. The general FFmpeg legal page still gives a
broad OpenSSL incompatibility warning; the release build relies on the exact
pinned configure check and OpenSSL 3 license rather than applying older OpenSSL
terms to this build.

x264's current licensing page offers GPLv2-only or a commercial license. GPLv2
only cannot be combined with GPLv3, even though FFmpeg's general codec page
describes x264 as GPLv2-or-later. The release build therefore removes the
optional x264 encoder and its source dependency, while retaining FFmpeg's built-in
H.264 decoder. Riina's current player does not expose encoding. The candidate
engine preserves built-in H.264 decoding. This license selection is recorded in
the release recipe; the development DLL remains development-only. CI must still
build the candidate and verify its exact native inventory, notices, source archive
and final portable package before publication.

The first reproducible native inspection is recorded in
[native-distribution-audit.json](native-distribution-audit.json). Run
`python scripts/audit-native.py` on Windows to inspect the hash-pinned development
DLL without loading media or user configuration. Its runtime configuration enables
dvdnav and prefers static libraries. The exact producing workflow revision is
`b2856ee6deed6957380b32465f359e67f67fafd8`; its FFmpeg recipe explicitly enables
GPL, x264 and x265. The runtime FFmpeg revision resolves to
`538d10d1878fb546c3a2645efa56119848f9ccfc`. A Windows-targeted Cargo metadata
traversal reached 344 packages and found no missing declared license fields.
Release CI regenerates notices against the exact lockfile and packages native
sources from captured revisions. The remaining checks verify those exact
candidate artifacts and their rebuild/playback results; they do not reopen the
MIT source and GPLv3 package choice or the x264 exclusion.

## Windows release build

The release workflow does not use that development DLL. It rebuilds the engine from
the pinned builder revision, removes the optional x264 encoder dependency and
configuration flag, retains FFmpeg's built-in H.264 decoder, and verifies the
OpenSSL 3 source revision and Apache-2.0 license used by the GPLv3 build. The native
build preserves each fetched source checkout and its license files; the source
archive also vendors every crate pinned by Cargo.lock.
The release job runs the complete native test suite and a playback test against the
engine extracted from the final portable ZIP. GitHub only publishes the assets when
those checks and source packaging succeed. These tests do not certify GUI, HDR
display, or pixel parity.

Before loading the downloaded engine, release packaging verifies every file against
its runtime hash manifest. It also checks the native source archive's checksum
before extraction and both final asset checksums before publication. Application
import checks produce a separate report so the recorded engine report stays intact.

The final package audit compares the engine's source manifest with the distributed
native source tree, checks each captured source hash and repository revision, and
matches packaged notices to those sources. The source archive also records the
application commit. An explicitly requested release dispatch publishes only after
the complete native suite, extracted-package playback, source packaging and this
audit succeed in the same run. The verification report is a release asset.

The pinned `subrandr` helper also brings in `unicode-ident 1.0.13`, which declares
the Unicode License Agreement for Data Files and Software (2016). The Free
Software Foundation classifies that permissive grant as compatible with all GPL
versions; the build accepts its SPDX identifier only after retaining the
dependency's copyright and license notice in the corresponding-source bundle
([GNU license list](https://www.gnu.org/licenses/license-list.html#Unicode)).

Primary references:

- [GNU FAQ: translating code](https://www.gnu.org/licenses/gpl-faq.en.html#TranslateCode)
- [GNU GPL version compatibility](https://www.gnu.org/licenses/gpl-faq.en.html#v2v3Compatibility)
- [x264 licensing terms](https://x264.org/licensing/)
- [FFmpeg license and legal considerations](https://www.ffmpeg.org/legal.html)
- [Pinned FFmpeg configure script](https://github.com/FFmpeg/FFmpeg/blob/538d10d1878fb546c3a2645efa56119848f9ccfc/configure)
- [OpenSSL license](https://openssl-library.org/source/license/index.html)
- [GNU license compatibility](https://www.gnu.org/licenses/license-compatibility.en.html)
- [Slint Royalty-free 2.0](https://slint.dev/agreements/slint-royalty-free-license.pdf)
- Pinned mpv Copyright, commit 36abaa32d00a7229ee206aae12dc0e97e7962dca
