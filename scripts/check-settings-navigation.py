"""Check that the settings sidebar retains IINA's default page order and routes."""
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = (ROOT / "crates/app/ui/preferences.slint").read_text(encoding="utf-8")
EXPECTED = [
    "General", "Interface", "Video", "Audio", "Subtitles", "Network", "Control",
    "Key Bindings", "Advanced", "Plugins", "Utilities",
]


def main():
    assert re.search(r"in-out property <int> selected-pane: 0;", SOURCE), (
        "Settings must open on its first page"
    )
    nav = re.findall(
        r'PreferenceNavItem\s*\{[^{}]*label:\s*"([^"]+)";'
        r'[^{}]*icon:\s*@image-url\("([^"]+)"\);'
        r'[^{}]*selected:\s*root\.selected-pane\s*==\s*(\d+);'
        r'[^{}]*activated\s*=>\s*\{\s*root\.selected-pane\s*=\s*(\d+);',
        SOURCE,
        re.DOTALL,
    )
    expected_routes = [(label, str(index), str(index)) for index, label in enumerate(EXPECTED)]
    routes = [(label, selected, activated) for label, _, selected, activated in nav]
    assert routes == expected_routes, f"Unexpected settings navigation: {routes!r}"

    # Keep each captured page symbol connected to an original, published asset.
    icon_root = (ROOT / "assets/icons").resolve()
    icons = [icon for _, icon, _, _ in nav]
    assert len(set(icons)) == len(EXPECTED), f"Settings page symbols must be distinct: {icons!r}"
    for icon in icons:
        path = (ROOT / "crates/app/ui" / icon).resolve()
        assert path.is_relative_to(icon_root) and path.is_file(), f"Missing settings symbol: {icon}"

    bodies = {int(index) for index in re.findall(r"if root\.selected-pane == (\d+)", SOURCE)}
    assert bodies == set(range(len(EXPECTED))), f"Missing or extra page routes: {bodies!r}"
    audio = re.search(
        r"if root\.selected-pane == 3\s*:(.*?)(?=\n\s*if root\.selected-pane == \d+\s*:|\Z)",
        SOURCE,
        re.DOTALL,
    )
    advanced = re.search(
        r"if root\.selected-pane == 8\s*:(.*?)(?=\n\s*if root\.selected-pane == \d+\s*:|\Z)",
        SOURCE,
        re.DOTALL,
    )
    assert audio and "Allow volume boost (up to 200%)" in audio.group(1), (
        "The opt-in volume cap belongs with Audio settings"
    )
    assert advanced and "Allow volume boost" not in advanced.group(1), (
        "The volume control must not be hidden under Advanced"
    )
    print("Settings order, routes, page symbols and Audio volume control placement match the product contract.")


if __name__ == "__main__":
    main()
