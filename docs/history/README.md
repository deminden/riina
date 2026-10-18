# Historical evidence

These records preserve earlier observations. They do not describe current
implementation or acceptance status. Use the [current ledger](../visual-reference-audit.csv)
and [verification report](../verification.md) for current findings.

## Ledger through 994f14c

[Archived CSV](visual-reference-audit-through-994f14c.csv), separated on
2026-10-18 from the ledger at code revision
`994f14c92a3fe09f81c1f47e102c6cf363016b9a`.

- All 69 original rows and bytes are preserved, including old capture paths,
  unavailable-capture reports, accumulated statuses and superseded gaps.
- File SHA-256: `ea1074c3bab469bc7496111d295e10da6db3199cb4e316bf1a69b4cdaefaf43b`.
- Match current and historical entries by `id`. Reference/capture paths inside
  both CSVs are relative to the repository root, regardless of archive location.
- Many captures are ignored local files. Their paths do not guarantee they are
  available in a fresh clone. Desktop images may contain private content.

Do not update this snapshot to reflect later work. Record new scoped evidence
with its revision and update the current ledger instead.

## Native distribution audit through 734a883

[Archived JSON](native-distribution-audit-through-734a883.json) preserves the
earlier native inspection, candidate failures and original 0.3.0 release evidence,
including its superseded large source archive. It is not the current package policy.

- Separated on 2026-10-19; original Git bytes preserved.
- SHA-256: `1d69f0339d4fbc349247d150e5a2261bb8e2cf668faf44f3d813ee02d4443a7f`.
- Use [the current native audit](../native-distribution-audit.json) for reusable
  engine identity, current package requirements and the latest recorded release.
