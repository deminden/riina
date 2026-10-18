# MIT source and GPLv3 application distribution

Riina's independently written source remains MIT licensed by the root LICENSE
and package metadata. The combined application package uses GPLv3 terms; the
complete text is in LICENSE-GPL-3.0.txt. This does not change third-party
component licenses or grant permission to relicense them. The distribution choice
is settled. The release workflow verifies exact package notices and corresponding
source access through the immutable application tag, locked crate sources and
pinned native source repositories before publication.

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
| Native `subrandr` Rust dependency `unicode-ident` 1.0.13 | Unicode-DFS-2016; retain its package notice in the native license inventory |
| Chrono 0.4.45 and battery 0.7.8 | MIT OR Apache-2.0; Rust dependency notices are generated from Cargo.lock |
| `webpki-roots` 1.0.9 certificate data | CDLA-Permissive-2.0; retain its crate `LICENSE` and generated `CCADB-ATTRIBUTION.txt` with the Rust license inventory |
| `trash` 5.2.9 | MIT; the system Trash/Recycle Bin adapter; generated dependency inventory preserves its crate notice |
| `lofty` 0.25.4 | MIT OR Apache-2.0; reads supported local audio tags for playlist display; generated dependency inventory preserves its crate notice |
| Development libmpv DLL and native dependencies | Local development input; release workflow rebuilds a separate engine |

`webpki-roots` 1.0.9 is an exact, package-scoped data-license exception in the
Rust license collector. The crate describes its root-certificate data as derived
from the Common CA Database (CCADB); its Cargo license expression identifies
CDLA-Permissive-2.0 for that data. This does not add CDLA to the general
GPL-compatible software-grant list or relicense the Riina application. The
collector requires the crate's original CDLA `LICENSE` text and generates a
CCADB attribution notice into the Windows package and corresponding-source
inventory. The exception is tied to this exact crate name, version and license
expression; other CDLA packages or versions still require review.

References: [webpki-roots](https://github.com/rustls/webpki-roots),
[CDLA-Permissive-2.0](https://cdla.dev/permissive-2-0/), and
[CCADB Data Usage Terms](https://www.ccadb.org/rootstores/usage).

Slint itself has not been relicensed. Its selected royalty-free license permits
desktop applications with attribution; this does not select Slint's GPL option.
Project and download pages must retain the attribution required by the pinned
license, alongside the packaged notices.

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
build the candidate and verify its exact native inventory, notices, source access
references and final portable package before publication.

The first reproducible native inspection is preserved in the
[historical audit](history/native-distribution-audit-through-734a883.json).
The [current audit](native-distribution-audit.json) identifies the reusable release
engine, latest recorded release and outstanding package checks. Run
`python scripts/audit-native.py` on Windows to inspect the hash-pinned development
DLL without loading media or user configuration. Its runtime configuration enables
dvdnav and prefers static libraries. The exact producing workflow revision is
`b2856ee6deed6957380b32465f359e67f67fafd8`; its FFmpeg recipe explicitly enables
GPL, x264 and x265. The runtime FFmpeg revision resolves to
`538d10d1878fb546c3a2645efa56119848f9ccfc`.
Release CI regenerates notices against the exact lockfile and records native
source access through captured public revisions and the build manifest. The
package checks verify each exact candidate and its playback results; they do not reopen the
MIT source and GPLv3 package choice or the x264 exclusion.

## Windows release build

The release workflow uses a separate engine built from the pinned builder
revision, or a verified matching artifact from an earlier run. Its recipe removes
the optional x264 encoder dependency and configuration flag, retains FFmpeg's
built-in H.264 decoder, and verifies the
OpenSSL 3 source revision and Apache-2.0 license used by the GPLv3 build. The native
build records public repository URLs and immutable revisions for each fetched
source, plus SHA-256 hashes for used files and symlink targets. It retains required
native license notices in the package. Native and vendored Rust source trees are
not duplicated into release assets: `SOURCE-MANIFEST.tsv` and the release-tagged
build recipe provide retrieval instructions, while Cargo.lock pins Rust crate
sources and checksums.
The release job runs the native suite, extracted-engine playback test and actual
extracted-application smoke, including shutdown and persisted settings. GitHub
only publishes the assets when those checks and the source-access audit succeed.
Automated smoke does not certify physical-display pixels, audible output, HDR
calibration or IINA visual parity.

Before loading the downloaded engine, release packaging verifies every file against
its runtime hash manifest, validates the native source index and packaged notices,
and checks the final asset checksums before publication. Application import checks
produce a separate report so the recorded engine report stays intact.

The final package audit validates the engine's native source manifest against its
pinned build recipe, matches packaged native notices to recorded source hashes,
and records the exact application commit. Native code remains available from the
immutable public revisions named in the manifest. An explicitly requested release
dispatch publishes only after the complete native suite, extracted-package
playback, source-access check and package audit succeed in the same run. The
verification report is a release asset.

The pinned `subrandr` helper also brings in `unicode-ident 1.0.13`, which declares
the Unicode License Agreement for Data Files and Software (2016). The Free
Software Foundation classifies that permissive grant as compatible with all GPL
versions; the build accepts its SPDX identifier only after retaining the
dependency's copyright and license notice in the release package
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
