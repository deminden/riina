# Documentation

Riina targets IINA v1.5.0. Windows 11 x64 is supported; macOS and Linux
application ports are in development.

## Start here

| I want to... | Read |
|---|---|
| Build or run Riina | [Development](development.md) |
| See what works and what remains | [Current status](status.md) |
| Choose the next implementation task | [Implementation plan](implementation-plan.md) |
| Understand the acceptance requirements | [Product contract](product-spec.md) |
| Run checks or review the latest evidence | [Verification](verification.md) |

## Feature and interface parity

- [Requirement inventory](parity.csv): individual requirements, implementation,
  tests and remaining differences. The inventory is incomplete.
- [Interface checklist](interface-parity-plan.md): surfaces and states to compare.
- [Visual specification](visual-spec.md): reference geometry and design rules.
- [Reference audit](visual-reference-audit.md): capture sets and comparison ledger.
- [Manual capture index](macos-manual-reference-index.md): the supplied menu and
  window screenshots.
- [Reference capture procedure](reference-capture.md): reproducible upstream captures.
- [Upstream source map](upstream-audit.md): pinned source locations and defaults.
- [Platform differences](platform-differences.md): deliberate adaptations and gaps.
- [Plugin compatibility](plugin-compatibility.csv): API coverage and limitations.

## Architecture and ports

| Decision | Role |
|---|---|
| [ADR 0001](adr/0001-ui-video-composition.md) | Historical SDR feasibility path; superseded for default Windows presentation |
| [ADR 0002](adr/0002-native-d3d11-experiment.md) | Default Windows D3D11 presentation |
| [ADR 0003](adr/0003-native-presentation-boundary.md) | Surface ownership, shutdown and Windows composition invariants |
| [ADR 0004](adr/0004-shared-gl-application-ports.md) | Shared GL application ports |
| [ADR 0005](adr/0005-macos-picture-in-picture.md) | macOS Picture in Picture boundary |

See [port development](port-development.md) for setup and support gates.

## Distribution and history

- [Licensing](licensing.md) and [source access](SOURCE-ACCESS.md).
- [Native distribution audit](native-distribution-audit.json) and
  [clipboard dependency audit](clipboard-dependencies.json).
- [Release notes](release-notes/README.md) are version-specific history, not current status.
  The [initial preview notes](release-notes/0.1.0-preview.1.md) are historical too.
- [Brand guide](../assets/branding/BRAND_GUIDE.md),
  [asset provenance](../assets/provenance.json) and
  [third-party notices](../THIRD_PARTY_NOTICES.md).

## Keeping these docs useful

Update the page that owns the information. Keep status short, the plan actionable,
and verification tied to a revision and environment. Record individual parity
results in the CSV ledgers. Superseded investigations belong in Git history;
release notes and architecture decisions retain their historical context.
