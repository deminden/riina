# IINA manual interface references

## Capture provenance

`reference/new/` contains 20 manually captured macOS desktop screenshots dated
2026-10-12. The user identified the reference machine as an M2 MacBook Air running
macOS 27.0.1 and IINA 1.5.0. The Welcome screenshot visibly identifies IINA 1.5.0;
the PNG metadata records the screenshot dimensions and display profile but not the
macOS build. Each image is 2940 × 1912 pixels with 144 dpi metadata. The folder is
ignored by Git on purpose; keep these local reference captures out of application
assets and release packages.

These are full-desktop research captures, not isolated, matched-state goldens. The
menu captures show IINA's native top menu bar and menus over a bright green video
surface. This is useful for menu ordering, labels, shortcut placement, selection,
submenu structure, and material compositing over video. Do not treat that green
surface as a menu color or use the whole desktop to measure menu text metrics.

## Indexed captures

All paths below are relative to `reference/new/`.

| Capture | What it records |
|---|---|
| `Screenshot 2026-10-12 at 19.17.33.png` | IINA application menu, Settings shortcut, standard app commands, and the macOS menu-bar sequence. |
| `Screenshot 2026-10-12 at 19.17.39.png` | File menu, Open Recent submenu, playback history, file actions, and Close shortcut. |
| `Screenshot 2026-10-12 at 19.17.47.png` | Native Edit menu and disabled text-edit commands. |
| `Screenshot 2026-10-12 at 19.17.53.png` | Playback command order, shortcuts, playlist loop, playlist/chapter navigation, and seek/step actions. |
| `Screenshot 2026-10-12 at 19.17.57.png` | Video menu order, panel and track commands, window sizing, PiP/full-screen actions, geometry submenus, filters, and Delogo. |
| `Screenshot 2026-10-12 at 19.18.01.png` | Aspect Ratio submenu, checked Default value, and the nine named ratios. |
| `Screenshot 2026-10-12 at 19.18.04.png` | Crop submenu, None state, named ratios, and Custom entry. |
| `Screenshot 2026-10-12 at 19.18.07.png` | Rotation submenu and the four checked angle choices. |
| `Screenshot 2026-10-12 at 19.18.09.png` | Flip submenu with Horizontal (Mirror) and Vertical (Flip). |
| `Screenshot 2026-10-12 at 19.18.14.png` | Audio menu order, volume/mute and delay commands, audio device, and filters. |
| `Screenshot 2026-10-12 at 19.18.17.png` | Audio Device submenu with auto-selection checked and named output devices. |
| `Screenshot 2026-10-12 at 19.18.26.png` | Subtitle tracks, visibility, external loading, online search, encoding, scale, delay, and font. |
| `Screenshot 2026-10-12 at 19.18.29.png` | Online subtitle provider submenu. |
| `Screenshot 2026-10-12 at 19.18.32.png` | Subtitle encoding submenu and its checked Auto detect choice. |
| `Screenshot 2026-10-12 at 19.18.37.png` | Plugin menu and Developer Tool submenu. |
| `Screenshot 2026-10-12 at 19.18.41.png` | Window menu, current-window checkmark, and Move & Resize submenu. |
| `Screenshot 2026-10-12 at 19.18.44.png` | Window > Full-Screen Tile submenu. |
| `Screenshot 2026-10-12 at 19.18.46.png` | Help menu, its search field, checked settings-window option, release highlights, and web links. |
| `Screenshot 2026-10-12 at 19.21.34.png` | Native Picture in Picture overlay, including close/restore controls, transport buttons, and timeline. |
| `Screenshot 2026-10-12 at 19.21.43.png` | IINA 1.5.0 Welcome window with Resume and recent items, overlapping the native file chooser. |

The menu text shows keyboard equivalents aligned at the right edge, native submenu
chevrons, checkmarks, separators, disabled states, and the blue highlighted row.
The Video menu's Aspect Ratio, Crop, Rotation, and Flip order is visible directly.
The PiP and Welcome captures are useful for native window content and ownership;
the surrounding desktop and overlapping applications are not part of those
surfaces.

## Use in parity work

Use this set with the traceable requirements in `docs/parity.csv` and the pinned
source tree at `reference/iina-upstream/`. The current source builds the top-level
menu sequence and platform-specific shortcut hints in
`crates/app/ui/player.slint` and `crates/app/ui/native-probe.slint`; the
`MENU-BAR-ORDER-001` requirement remains partial because the full live interaction,
dynamic shortcut states, and matched macOS/Windows captures are still open.
Compare the indexed states individually after the Riina playback window can be
captured reliably. These images establish menu content and structure; they do not
prove behavior, complete application parity, or current macOS 27.0.1 text metrics.
