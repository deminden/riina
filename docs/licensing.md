# MIT source and GPLv3 application distribution

Riina's independently written source remains MIT licensed by the root LICENSE
and package metadata. The combined application is intended for distribution
under GPLv3 terms; the complete text is in LICENSE-GPL-3.0.txt. This does not
change third-party component licenses or grant permission to relicense them.
Distribution of a particular binary still requires the exact dependency, notice
and corresponding-source audit below.

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
publication. Existing records of source-informed behavior do not certify a
clean-room process: reference implementation code was inspected during earlier
development. The provenance review remains part of the release audit. MIT on
original contributions is not a blanket legal certification.

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
| Development libmpv DLL and native dependencies | Existing terms, redistribution audit incomplete |

Slint itself has not been relicensed. Its royalty-free license permits desktop
applications with attribution. The public README displays the official badge
from the pinned v1.18.1 resource. This avoids selecting Slint's GPL option for
the Windows application. Future download pages must retain compliant attribution.

The selected GPLv3 package route is intended to permit a full-featured native
engine build, subject to verifying that every included component can be
redistributed together under those terms. The pinned development libmpv build has
not passed that audit. Its producing recipe enables FFmpeg's GPL mode,
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
engine must pass the native playback suite and final package smoke test before
the bundle is published. The current development DLL is not approved for release.

The first reproducible native inspection is recorded in
[native-distribution-audit.json](native-distribution-audit.json). Run
`python scripts/audit-native.py` on Windows to inspect the hash-pinned development
DLL without loading media or user configuration. Its runtime configuration enables
dvdnav and prefers static libraries. The exact producing workflow revision is
`b2856ee6deed6957380b32465f359e67f67fafd8`; its FFmpeg recipe explicitly enables
GPL, x264 and x265. The runtime FFmpeg revision resolves to
`538d10d1878fb546c3a2645efa56119848f9ccfc`. These are concrete build facts, not an
approved distribution arrangement. A Windows-targeted Cargo metadata traversal
reached 344 packages and found no missing declared license fields; this does not
select every offered license, validate notices, or audit the native dependency
graph. Lack of embedded configuration strings does not establish complete
component inventory or license compatibility. Exact corresponding source, all
dependency notices, review of the combined executable, and a clean-package smoke
test remain outstanding.

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
