# Implementation plan

Riina is an established, functioning media-player application with real Windows
playback and broad player and preferences workflows. The remaining work is closing
IINA v1.5.0 behavior and interface-parity requirements; this plan describes current
priorities, not a project timeline. The traceable requirement inventory is
`docs/parity.csv` and remains incomplete.

## Priority order

1. Close fully missing IINA behaviors in the requirement inventory. Choose work by
   user impact and dependency order; carry shared state and behavior through portable
   layers, then connect each platform adapter. Keep the opt-in 200% volume extension
   available while preserving the 0–100% default.
2. Measure and close visual gaps against actual pinned macOS captures. Compare complete
   player, menu, sidebar, preferences, dialog, and auxiliary-window surfaces at matched
   states and sizes. Preserve IINA's visual language; do not replace it with a redesign.
3. Build macOS and Linux application ports alongside Windows feature and visual work.
   Windows 11 x64 is the only supported application target until each port has a full
   app build, visible playback, input, dialogs, window lifecycle, and shutdown evidence.
4. Deliver Windows binaries from the pinned native build. The MIT source and GPLv3
   combined-package terms are settled. A release still requires exact runtime contents,
   notices, corresponding sources, rebuild evidence, and playback tests from the final
   extracted package.

## Current verification gates

- Every implemented behavior has typed intent, observed backend state where applicable,
  and tests at the layer that owns the behavior.
- Visual parity claims require Windows captures compared with the pinned macOS reference;
  source inspection and layout dimensions alone do not establish parity.
- Platform support requires full application and playback/lifecycle evidence. Library-only
  CI is insufficient.
- Binary publication requires successful validation of the exact candidate package and
  its corresponding-source bundle. The license choice itself is not a gate.
- Run long checks asynchronously and continue independent work while they run. Record
  which revision and components each check covers; do not wait idly or launch duplicates.

## Architecture invariants

Keep playback, preferences, playlist/catalog models, and user-visible policies portable.
Place HWND, display, file-dialog, file-manager, and OS lifecycle operations behind
platform adapters. Keep libmpv ownership explicit and tear down playback before its
presentation surface. Shared feature work must not introduce Windows assumptions into
macOS or Linux code. Record genuine platform differences in the requirement inventory.

Use small Rust modules, typed intent, and comments that explain semantics or invariants.
Keep the IINA v1.5.0 reference outside product source. Implement from observable behavior
and documented APIs; do not translate upstream implementation code or copy protected
assets.
