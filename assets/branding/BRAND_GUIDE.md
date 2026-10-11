# Riina brand usage

## Name and capitalisation

Use **Riina** as the product name in ordinary writing. Use **riina** as the
designed lowercase wordmark and for technical identifiers.

| Context | Recommended form |
| --- | --- |
| GitHub repository | `deminden/riina` |
| Repository URL | `https://github.com/deminden/riina` |
| README title | `# Riina` |
| README prose | `Riina is a desktop media player built with Rust, inspired by IINA.` |
| GitHub release title | `Riina v0.2.15` (use the actual release version) |
| Issue / discussion title | `Riina crashes when opening…` |
| Image alt text | `Riina logo` |
| App display name / Windows entry | `Riina` |
| Executable / crate / package identifiers | `riina`, `riina.exe` |
| Command | `cargo run -p riina` |
| Visual wordmark | `riina` |

Do not force lowercase throughout prose. The wordmark is an intentional visual
form; it does not determine grammar or the spelling of app-list entries.

Suggested concise repository description:

> A desktop media player built with Rust, inspired by IINA.

For a README header that changes with GitHub's appearance:

```html
<picture>
  <source media="(prefers-color-scheme: dark)"
          srcset="assets/branding/svg/riina-logo-horizontal-dark.svg">
  <img src="assets/branding/svg/riina-logo-horizontal-light.svg"
       alt="Riina logo" width="420">
</picture>
```

Retain a plain `# Riina` heading so the document also has a searchable text title.

## Identity

The Forward r symbol is a rounded vertical stem and shoulder with a diagonal,
forward-facing terminal. Use the supplied geometry consistently. The wordmark
echoes that terminal in its custom r and uses a lighter, medium/semibold texture
for the remaining letters. The two i dots are separate, and the a keeps its
readable double-storey form.

## Palette

| Role | HEX | RGB |
| --- | --- | --- |
| Teal accent | `#0D9488` | 13, 148, 136 |
| Charcoal | `#181D20` | 24, 29, 32 |
| Warm white | `#F6F5F1` | 246, 245, 241 |
| Reversed mark | `#FFFFFF` | 255, 255, 255 |

For a light surface, use the teal symbol and charcoal lettering. For a dark
surface, use the teal symbol and white lettering. Use the all-black or all-white
version when only one colour is available. Choose the opposite theme's version
if a logo lacks contrast on the actual surface.

## Spacing and sizing

- Keep at least one quarter of the symbol's height clear around a standalone mark.
- The horizontal logo SVG already includes internal spacing; preserve its aspect
  ratio and leave extra space around it in the surrounding layout.
- Prefer the standalone symbol or app icon below approximately 120 px of total
  logo width. Use a wordmark only where the letters remain comfortably readable.
- Use the small app-icon exports for 16–32 px contexts. Avoid reducing a large
  logo or a screenshot of the original concept to make a taskbar icon.
- Keep all letters, dots and the diagonal cut intact. Do not stretch, rotate or
  separately recolour individual letters.

## Default app treatment

Use the charcoal rounded tile with a white symbol as the default Windows app
icon, matching the selected concept's application example. The teal-tile version
is an included alternative, and the unplated teal symbol is suitable where a
platform surface benefits from a free-standing silhouette.

Use the identity on the welcome/About screen, documentation and release material.
The main playback interface continues to follow the project's interface
specification; branding colours do not automatically become control colours.

## Development scope

The pack provides valid SVG/PNG/ICO files and an editable generator. It does not
claim that Windows shell integration, theme selection or a rebuilt Riina binary
has been tested. After embedding the ICO, check the executable, pinned shortcut,
taskbar and Open With display on a Windows machine.
