# Visual reference audit

The [CSV ledger](visual-reference-audit.csv) contains per-reference observations,
SHA-256 hashes, comparison evidence and remaining differences. This guide explains
the reference sets; it does not repeat their progress history.

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
| REF-01-19 | Preferences/reference images; inspect the individual ledger rows for exact state and defects |
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
5. Update the ledger with evidence and any remaining gap. Source changes or a
   successful build alone do not close visual parity.

Use the [interface checklist](interface-parity-plan.md) for scope and the
[visual specification](visual-spec.md) for shared metrics. Keep new observations
in the ledger rather than appending another chronological diary to this guide.
