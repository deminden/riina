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
| Development libmpv DLL and native dependencies | Existing terms, redistribution audit incomplete |

Slint itself has not been relicensed. Its royalty-free license permits desktop
applications with attribution. The public README displays the official badge
from the pinned v1.18.1 resource. This avoids selecting Slint's GPL option for
the Windows application. Future download pages must retain compliant attribution.

The selected GPLv3 package route is intended to permit a full-featured native
engine build, subject to verifying that every included component can be
redistributed together under those terms. The pinned development libmpv build has
not passed that audit. Its producing recipe enables FFmpeg's GPL mode,
`--enable-version3`, OpenSSL, x264,
x265 and dvdnav. At the pinned FFmpeg revision, `--enable-version3` upgrades the
resulting GPL libraries to GPLv3-or-later, and the configure script says OpenSSL
3 also requires that option in a GPL build. x264's licensing page says the
encoder is GPLv2-only; GNU compatibility guidance says GPLv2-only code cannot be
combined with GPLv3. The builder recipe fetches x264 without pinning a commit, so
we cannot resolve the exact grant for the source used in this DLL. The present
native build is therefore not cleared for the proposed GPLv3 package.

The GPLv2 option was checked as well. It is not a relabeling fix: the current
FFmpeg build is configured for GPLv3, and its OpenSSL 3 dependency requires the
version-3 mode. A GPLv2 build would need a different TLS dependency/configuration
and its own full compatibility audit. The lower-impact GPLv3 candidate is to
remove FFmpeg's optional x264 encoder. FFmpeg's pinned configure help identifies
`--enable-libx264` as H.264 encoding; the recipe does not disable FFmpeg's
built-in H.264 decoder. Rebuild and verify H.264 playback, seek, subtitle,
audio-track and HDR fixtures before treating that as parity-preserving. Pin all
native sources and patches, provide the corresponding source/build recipe and
notices, then test the complete clean package. The current development DLL is
not approved for release.

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
configuration flag, retains FFmpeg's built-in H.264 decoder, and packages the full
mpv runtime. The native build preserves each fetched source checkout and its
license files; the source archive also vendors every crate pinned by Cargo.lock.
The release job runs the complete native test suite and a playback test against the
engine extracted from the final portable ZIP. GitHub only publishes the assets when
those checks and source packaging succeed. These tests do not certify GUI, HDR
display, or pixel parity.

Primary references:

- [GNU FAQ: translating code](https://www.gnu.org/licenses/gpl-faq.en.html#TranslateCode)
- [GNU GPL version compatibility](https://www.gnu.org/licenses/gpl-faq.en.html#v2v3Compatibility)
- [x264 licensing terms](https://x264.org/licensing/)
- [FFmpeg license and legal considerations](https://www.ffmpeg.org/legal.html)
- [Pinned FFmpeg configure script](https://github.com/FFmpeg/FFmpeg/blob/538d10d1878fb546c3a2645efa56119848f9ccfc/configure)
- [Slint Royalty-free 2.0](https://slint.dev/agreements/slint-royalty-free-license.pdf)
- Pinned mpv Copyright, commit 36abaa32d00a7229ee206aae12dc0e97e7962dca
