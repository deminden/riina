# Implementation plan

Read [current status](status.md) for completed work and
[the product contract](product-spec.md) for acceptance requirements.
The baseline remains IINA v1.5.0.

## Next work: everyday playback workflows

Close end-to-end behavior gaps in Quick Settings input, folder/playlist/history
flows and window transitions, then compare the main player and its menus against
the pinned captures. Keep each batch local until related changes and checks fit.

The 4.1.0 release collects these completed batches:

- [Quick Settings numeric editing and focus](../tests/visual/windows-quick-settings-input-2026-10-19.json):
  delay/speed drafts, submission/cancellation, observed updates and controller auto-hide.
- [Category traversal](../tests/visual/windows-quick-settings-navigation-2026-10-19.json):
  one category Tab stop, arrows/Home/End, visible focus and narrow scrolling.
- [Windows menu input](../tests/visual/windows-system-menu-alt-2026-10-19.json):
  framed-owner routing and lost Alt release recovery after the native menu loop.
  Retain Windows menu-bar navigation between the first and second Escape.
- [Player proportions](../tests/visual/windows-player-proportions-2026-10-19.json):
  balanced floating transport, bounded theme selection, aligned Layout symbols,
  centered controller previews and explicit narrow scroll-page geometry.

Use those records for tested scope and remaining gaps. Other DPI, input layouts,
IME, cross-application cancellation and matched macOS interaction remain separate
acceptance work. Next exercise folder/playlist/history and window transitions as
complete workflows, retaining the playback/fullscreen regression gates below.

The [local track-list follow-up](../tests/visual/windows-quick-settings-tracks-2026-10-19.json)
adds shared Audio/Video/subtitle list ownership, repeated-key traversal, row reveal
and flexible language badges. Native checks cover all four selectors and narrow
light Audio. Continue Quick Settings work on the remaining page focus graph,
modified native-menu shortcuts while a panel owns focus, accessibility action
bounds and matched macOS control geometry. Music Mode artwork still needs a
separate native video viewport: letterboxing and art under the compact control
strip remain visible, even though compact frame sizing is now correct.

The latest local workflow batch adds keyboard-operated equalizer naming, optical
play-glyph centering, separate Music Mode transport/window controls, independent
pane scroll positions and visible playlist rows with a compact six-action footer.
It also fixes first-click slider input, quiet paused-media Music Mode transitions
and Windows watch-later path identity. See the dated workflow record in
[verification](verification.md) for final tested scope rather than treating these
implementation changes as complete acceptance.

Windows is the only available test machine. Linux CI now provisions the GTK and
WebKit dependencies needed by the full application; neither that configuration
change nor shared tests establish macOS/Linux playback support. Physical HDR,
pass-through audio and other display/DPI combinations still need equipment.

## Work order

### 1. Close missing behavior

Use [parity.csv](parity.csv) and [plugin-compatibility.csv](plugin-compatibility.csv)
to choose a missing workflow by user impact and dependency order. Inspect the
pinned source's ownership and lifecycle, implement shared typed behavior, connect
platform adapters, and record implementation separately from verification.

Prioritize incomplete workflows over cosmetic polish. Keep the complete IINA
scope, existing defaults and the default-off 200% volume extension.

### 2. Close interface differences one surface at a time

Use the [interface checklist](interface-parity-plan.md),
[visual specification](visual-spec.md) and [reference ledger](visual-reference-audit.csv).
For each surface:

1. Select a usable IINA reference and record state, scale, dimensions and media.
2. Capture the corresponding Riina state and identify measurable differences.
3. Correct layout, text, icons, materials and interaction together.
4. Recheck both themes, compact/narrow layouts, focus and scrolling.
5. Update the ledger with evidence and remaining gaps.

The immediate comparison set is the floating controller (REF-20), Layout
(REF-21), Music Mode (REF-22), Audio (REF-23), Video (REF-24), Subtitles (REF-25)
and the menu families (REF-26-45). Do not mark a surface complete from source
sizes or compilation alone.

Materials follow the host platform: Liquid Glass on macOS, Mica/Acrylic on
Windows, and available compositor materials or a documented fallback on Linux.
Playback overlays use video as their backdrop. Compare light/dark appearance,
transparency, Battery Saver and High Contrast behavior separately.

### 3. Develop the application ports alongside shared work

Preserve portable feature boundaries. Follow [port development](port-development.md)
and ADRs [0004](adr/0004-shared-gl-application-ports.md) and
[0005](adr/0005-macos-picture-in-picture.md). Support requires a full application
build plus visible playback, input, dialogs, fullscreen and shutdown evidence.
Library-only checks do not establish a supported port.

### 4. Prepare verified Windows release candidates

Follow [development and packaging](development.md#release-candidates).
Reuse a native-engine artifact only when its full build identity matches.
Run packaging tests even on the reuse path, then test the extracted executable.
Keep notices and source access tied to the exact package. Public updates follow
[AGENTS.md](../AGENTS.md); ordinary development stays local.

## Regression gates

Keep these checks in the focused Windows sweep:

- Continuous volume changes preserve video, client bounds and native menu identity.
- Fullscreen entry, controller hide/reveal and exit preserve visible playback.
  The controls popup is one physical scanline shorter only in fullscreen; the
  video host and mpv child retain full monitor bounds.
- Wide, standard, portrait and audio playlist transitions keep usable window
  placement. Include audio-to-video transitions, small controls and enlargement.
- Sidebars retain their selection, width and placement across save/restart;
  resizing does not overwrite unrelated preferences.
- Moving the player keeps host, controls and mpv child aligned in screen position
  and size. Diagnose stationary patches with before/after full-desktop captures.
- Opening, cancellation, owner-close and normal shutdown leave no stale process.

Use [Verification](verification.md) for commands and evidence requirements.

## Architecture constraints

Keep domain state, preferences and playback policies portable. Put native handles,
windowing, file dialogs, shell actions and OS lifecycle behind platform adapters.
The playback worker must stop before its presentation surface is released.
Preserve the single Windows event router, deferred geometry updates and stable
native-menu ownership described in [ADR 0003](adr/0003-native-presentation-boundary.md).

Use small modules, explicit ownership and typed intent. Develop independently
from observable IINA behavior and documented APIs. Run long checks asynchronously
and continue independent work; record which revision they cover.
