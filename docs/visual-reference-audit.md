# Visual reference audit

The [current CSV ledger](visual-reference-audit.csv) describes implementation,
verification scope and remaining gaps for all 69 reference records. It is a
snapshot, not a progress diary. No row currently establishes full visual parity.

The [historical ledger](history/README.md) preserves the earlier observations and
capture links separately. Historical capture failures and superseded gaps are
not current blockers unless reproduced in the current ledger.

## Reading the current ledger

- `id`, reference path, SHA-256 and environment fields preserve capture identity.
  Paths are relative to the repository root; unknown scale remains unknown.
- `reference_quality`: `usable`, `near-duplicate`, `failed`, `missing` or
  `context-only`. Usable does not mean matched to Riina.
- `current_implementation` describes the maintained implementation snapshot;
  source inspection is not runtime or visual acceptance. Evidence records name
  their revision or base revision plus exact modified-source hashes.
- `verification`: `source-only`, `partial-gui` or `reference-blocked`.
  `partial-gui` is limited to the named evidence record, not every state in a row.
- `current_evidence` links a scoped dated record. An empty value means there is
  no current GUI evidence for that row. Duplicate references may share evidence.
- `remaining_gaps` and `next_action` describe outstanding work. `historical_evidence`
  identifies the archived row by ID; CSV fragments are lookup keys, not HTML anchors.

The [Preferences follow-up](../tests/visual/windows-preferences-readability-2026-10-18.json)
inspects all eleven initial category viewports and records corrected light-widget
contrast, Audio continuity and auxiliary width. The [row follow-up](../tests/visual/windows-preferences-rows-2026-10-18.json)
corrects Utilities/Advanced overlap, numeric-cell affordances, shortcut keycaps
and glyph visibility, and verifies a saved numeric edit. Subtitle overflow and
General/Network navigation differences remain open. Lower viewports, dialogs, broader themes, complete
menu states and matched macOS geometry remain open. A successful build does not
close these gaps. The [initial inspection](../tests/visual/windows-ledger-review-2026-10-18.json)
preserves the earlier failures as dated evidence.

## Reference sets

| Set | Environment and purpose | Location |
|---|---|---|
| Automated player/Settings captures | Official IINA 1.5.0 on macOS 26.6.2; paused player/OSD and eleven light Settings pages | [Capture manifest](../tests/visual/reference-captures.json) |
| Supplied playback/Preferences set | IINA 1.5.0; user identified macOS 27.0.1 and M2 MacBook Air; scale was not recorded | `reference/captures/iina-reference-set-2026-10-12/` (ignored) |
| Supplied menu/window captures | Twenty 2940 x 1912 desktop images from the same user-reported machine; menu structure and contextual windows | `reference/new/` (ignored); [manual index](macos-manual-reference-index.md) |

The automated captures include playback observations and metadata. They are
research references, not automatically matched Windows goldens. The supplied
sets have unknown display scale: image pixels alone do not establish logical
point dimensions. Do not merge their OS versions or scale assumptions.

### Reference defects and privacy

- `00-start-window.png` is listed by the imported set's README but was not supplied.
- 09a/10a/15b are near-duplicates; 13b is a failed capture. The ledger retains them.
- REF-26-45 include unrelated desktop content. Keep the originals local. Publish
  only reviewed app-surface crops with private content removed.
- Green test video helps identify translucent menu boundaries; it is not a UI color.
- Audio devices, providers, plugins, tracks, recent files and checks are runtime
  examples, not a fixed menu inventory. The encoding menu is only partly visible.
- REF-44/45 contain overlapping windows. Use them for ownership/context, not clean
  pixel measurements.

## Capture map

| IDs | Surface |
|---|---|
| REF-09-19 and lettered variants | Supplied Preferences images; inspect individual rows for exact state and defects |
| REF-00 | Missing start-window image |
| REF-20 | Floating playback controller |
| REF-21 | Quick Settings Layout |
| REF-22 | Music Mode |
| REF-23 | Quick Settings Audio |
| REF-24 | Quick Settings Video |
| REF-25 | Quick Settings Subtitles |
| REF-26-34 | Application, File, Edit, Playback and Video menu families |
| REF-35-39 | Audio and Subtitle menus/submenus |
| REF-40 | Plugins |
| REF-41-42 | Native Window/tiling menus |
| REF-43 | Help |
| REF-44-45 | Contextual Open/PiP and Welcome windows |
| WIN-PLAYER-THEME-001 | Windows platform appearance adaptation; historical dark/light evidence |

## Source anchors

Pinned files live under `reference/iina-upstream/iina/`:

- `Base.lproj/MainMenu.xib`: menu skeleton and labels.
- `MenuController.swift` and `MainMenuActions.swift`: dynamic items, checks,
  shortcut state and dispatch.
- `AppDelegate.swift`: native Open panel and recent-document registration.
- `InitialWindowController.swift`: Welcome geometry and behavior.
- `SidebarLayoutPane.swift`: Fit Window to Video, which belongs in Layout rather
  than the Video menu.
- `PluginViewController.swift` and `JavascriptDevTool.swift`: plugin surfaces.

Delegate macOS-owned Edit, Window tiling and native dialog behavior to platform
APIs. Keep the narrow Windows equivalents in [platform differences](platform-differences.md).

## Recording a comparison

1. Review one image and verify its hash, visible state and suitability.
2. Record matching fixture/frame, scale, client dimensions, theme and panel state.
3. Capture Riina and inspect the full desktop first for composition reports.
4. Measure differences by region; distinguish observed, source-derived and
   estimated dimensions. Unknown scale remains unknown.
5. Replace the row's current findings and evidence with the new scoped result.
   Keep older dated evidence in history; do not append chronological status tags.
   Source changes or a successful build alone do not close visual parity.

Use the [interface checklist](interface-parity-plan.md) for scope and the
[visual specification](visual-spec.md) for shared metrics. Keep new observations
in the ledger rather than appending another chronological diary to this guide.
