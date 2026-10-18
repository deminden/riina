# Interface parity checklist

This is the surface checklist for IINA v1.5.0. The
[implementation plan](implementation-plan.md) defines priority; the
[reference ledger](visual-reference-audit.csv) records individual comparisons.
A surface can be implemented and still have open visual or interaction gaps.

## Reference state

The user-supplied reference machine is an M2 MacBook Air with IINA 1.5.0 on
macOS 27.0.1. Automated reference captures also exist from macOS 26.6.2.
Keep those environments distinct. See the [reference audit](visual-reference-audit.md)
and [manual index](macos-manual-reference-index.md).

Match fixture/frame, client size, display scale, theme, compact mode, selected
panel and visible controls before comparing. Check geometry, typography, glyphs,
materials, focus, enabled states, scrolling and animation. Preserve remaining
differences in the ledger rather than declaring parity from source dimensions.

## Player and auxiliary surfaces

| Surface | Compare |
|---|---|
| Welcome | Window size, non-resizable behavior, Open File/Open URL, resume item, recent rows, selection and keyboard navigation |
| Main player | Filename title, native frame/menu, video fit and matte bands, minimum size, centering and monitor recovery |
| Floating controls | Previous/play/next, default four toolbar actions, volume, timeline, drag position, compact/spacious and narrow layouts, hide/reveal |
| Top/bottom controls | Reserved space, title/sidebars, resize behavior and focus |
| Quick Settings Layout | Theme/material, OSD, controller tiles, sidebar placement, compact UI, edge-to-edge video, docked bar, aspect lock and fit action |
| Quick Settings Video | Track selector, aspect/crop/rotation, speed, decoder, deinterlace, picture adjustments and scroll bounds |
| Quick Settings Audio | Tracks, output devices, delay, equalizer and profile menus |
| Quick Settings Subtitles | Primary/secondary lanes, visibility, delays, styling and warning states |
| Playlist/chapters | Tabs, rows, metadata, selections, resize, footer/context actions, drag/drop and multi-selection |
| Menus | Top-level and submenu order, labels, separators, checks, disabled items, shortcut column and dynamic contents |
| Music Mode | Art, metadata, controls, timeline, optional playlist and return to video |
| Picture in Picture | Native controls, parent-window policy, state restoration and playback continuity |
| Fullscreen | Entry/exit, hidden/revealed controls, additional info, panels, focus and display changes |
| Other windows | Crop/Delogo, font picker, screenshot preview, history, inspector, logs, About, online subtitles and plugin tools |
| Transient UI | OSD, hover thumbnails, tooltips, alerts, confirmation, file/URL dialogs and ownership |

## All eleven Preferences categories

| Category | Required coverage |
|---|---|
| General | Behavior, History, Playlist and Screenshots; disclosure states, recent-file policy, resume and output paths |
| Interface | Appearance, initial size/position, resize policy, float-on-top, controller customization, OSD and fullscreen information |
| Video | Hardware decoder, threads, ICC, HDR and nested tone mapping; capability-dependent visibility |
| Audio | Hardware, Volume and Other sections; devices, pass-through, ReplayGain, gapless and languages; default-off volume boost |
| Subtitles | Auto Load, ASS lanes, Text Subtitles, Position, Other and online providers; expanded groups and long fields |
| Network | Cache, connection and online-video helper settings; restart-required details and plugin state |
| Control | Trackpad, Mouse, vertical/horizontal scroll assignment, seek type and sensitivity |
| Key Bindings | Profiles, search/raw values, keycaps, recording/action builder, import/export, conflicts and system keys |
| Advanced | Enablement, logging/viewer, mpv OSD/config directory and additional-options editor |
| Plugins | Empty/installed lists, local/GitHub install, permission review, updates, removal and contributed UI |
| Utilities | Associations, alerts, progress/history/thumbnail clearing and browser-extension status |

Across all pages, compare the source-list sidebar, category/section navigation,
search results, disclosure layout, persistence, restart behavior and the ability
to reach every control at the reference size.

## Comparison order

Start with REF-20 through REF-25: floating controller, Layout, Music Mode, Audio,
Video and Subtitles. Review menus by family using REF-26 through REF-45, then
complete the Preferences and auxiliary-window states. Use both light and dark
appearances and include narrow windows, enlarged windows, high DPI and keyboard
navigation. The checklist covers the full interface; it is not a completion score.
