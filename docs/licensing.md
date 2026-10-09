# MIT and independent implementation

The user requires MIT and independent development. The root LICENSE grants MIT
rights to original wiina contributions. This does not grant permission to
relicense third-party work. The operating system and programming language are
not a copyright exception: translating protected GPL code into Rust does not
make it MIT code.

## Implementation policy

- Use IINA to establish observable behavior, defaults, compatibility facts and
  expected test results. Keep the pinned reference outside the product source.
- Write original Rust architecture and algorithms from those requirements and
  documented libmpv, Windows and Slint interfaces. Do not mechanically port Swift
  methods, data structures, XIBs or source assets.
- Preserve compatibility/provenance records. If protected IINA expression is
  found in wiina, the MIT grant does not cover it: replace it with an independent
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
| Original wiina Rust/Slint definitions, scripts and documentation | MIT, root LICENSE |
| mpv ABI definitions | Retained ISC notice in THIRD_PARTY_NOTICES.md |
| Original synthetic fixture bytes | CC0-1.0, fixture README and manifest |
| Excluded IINA reference | Existing GPL terms, outside the MIT grant |
| Slint 1.13.1 and slint-build | Royalty-free 2.0, public README badge, section 2(b) |
| Development libmpv DLL and native dependencies | Existing terms, redistribution audit incomplete |

Slint itself has not been relicensed. Its royalty-free license permits desktop
applications with attribution. The public README displays the official badge
from the pinned v1.13.1 resource. This avoids selecting Slint's GPL option for
the Windows application. Future download pages must retain compliant attribution.

Original source's MIT license does not establish the license of a combined
binary. The pinned development libmpv build has not been audited for distribution.
mpv documents its default GPL mode and an optional LGPL configuration; setting
gpl=false alone is not an LGPL grant. A release must select and verify compatible
native builds, codecs, notices and corresponding-source obligations. No native
release binaries have been published.

Primary references:

- [GNU FAQ: translating code](https://www.gnu.org/licenses/gpl-faq.en.html#TranslateCode)
- [Slint Royalty-free 2.0](https://slint.dev/agreements/slint-royalty-free-license.pdf)
- Pinned mpv Copyright, commit 36abaa32d00a7229ee206aae12dc0e97e7962dca
