"""Check that the settings sidebar retains IINA's default page order and routes."""
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = (ROOT / "crates/app/ui/preferences.slint").read_text(encoding="utf-8")
PLAYER_SOURCE = (ROOT / "crates/app/ui/player.slint").read_text(encoding="utf-8")
SEARCH_SOURCE = (ROOT / "crates/app/src/settings_search.rs").read_text(encoding="utf-8")
EXPECTED = [
    "General", "Interface", "Video", "Audio", "Subtitles", "Network", "Control",
    "Key Bindings", "Advanced", "Plugins", "Utilities",
]
EXPECTED_INTERFACE_SECTIONS = [
    "Appearance", "Window", "On Screen Controller", "On Screen Display",
    "Thumbnail Preview", "Picture-in-Picture", "Accessibility",
]
EXPECTED_GENERAL_SECTIONS = ["Behavior", "History", "Playlist", "Screenshots"]


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

    general_page = re.search(
        r"if root\.selected-pane == 0\s*:\s*ScrollView\s*\{(.*?)"
        r"(?=\n\s*if root\.selected-pane == 1\s*:)",
        SOURCE,
        re.DOTALL,
    )
    assert general_page and "content-y <=> root.general-scroll-position" in general_page.group(1), (
        "General settings must remain one continuous scroll page"
    )
    headings = [
        general_page.group(1).find(f'text: "{label}"')
        for label in EXPECTED_GENERAL_SECTIONS
    ]
    assert all(position >= 0 for position in headings) and headings == sorted(headings), (
        f"General sections must appear in order on the shared page: {headings!r}"
    )
    assert all(
        f'"{section}" => window.get_general_{target}_target()' in SEARCH_SOURCE
        for section, target in (
            ("History", "history"),
            ("Playlist", "playlist"),
            ("Screenshots", "screenshots"),
        )
    ) and "window.set_general_scroll_position(target);" in SEARCH_SOURCE, (
        "Settings search must scroll to the matching General section"
    )

    interface_sections = re.findall(
        r'PreferenceSubNavItem\s*\{[^{}]*label:\s*"([^"]+)";'
        r'[^{}]*selected:\s*root\.selected-interface-section\s*==\s*(\d+);',
        SOURCE,
        re.DOTALL,
    )
    expected_sections = [
        (label, str(index)) for index, label in enumerate(EXPECTED_INTERFACE_SECTIONS)
    ]
    assert interface_sections == expected_sections, (
        f"Unexpected Interface subsection navigation: {interface_sections!r}"
    )
    interface_body = re.search(
        r"if root\.selected-pane == 1\s*:\s*ScrollView\s*\{(\s*"
        r"x: 200px;.*?)(?=\n\s*if root\.selected-pane == 2\s*:)",
        SOURCE,
        re.DOTALL,
    )
    assert interface_body and "content-y <=> root.interface-scroll-position" in interface_body.group(1), (
        "Interface subsections must scroll the settings content"
    )
    scroll_sections = re.findall(
        r"(?:if|else if) root\.interface-scroll-position > root\.(interface-[a-z-]+-threshold) "
        r"\{ root\.selected-interface-section = (\d+); \}",
        interface_body.group(1),
    )
    expected_scroll_sections = [
        ("interface-appearance-threshold", "0"),
        ("interface-window-threshold", "1"),
        ("interface-osc-threshold", "2"),
        ("interface-osd-threshold", "3"),
        ("interface-thumbnail-threshold", "4"),
        ("interface-pip-threshold", "5"),
    ]
    assert scroll_sections == expected_scroll_sections, (
        f"Interface scroll positions must select their matching subsection: {scroll_sections!r}"
    )
    assert "else { root.selected-interface-section = 6; }" in interface_body.group(1), (
        "The final Interface subsection must remain selectable at the scroll limit"
    )
    video_body = re.search(
        r"if root\.selected-pane == 2\s*:\s*ScrollView\s*\{(.*?)"
        r"(?=\n\s*if root\.selected-pane == 4\s*:)" ,
        SOURCE,
        re.DOTALL,
    )
    assert video_body, "Video preferences must keep a dedicated scrollable pane"
    video = video_body.group(1)
    assert "content-y <=> root.video-scroll-position" in video, (
        "Video Settings search results must scroll to their selected section"
    )
    for label in ["Decoding", "Number of threads", "Hardware decoder", "Color and HDR"]:
        assert f'text: "{label}"' in video, f"Video preferences are missing {label!r}"
    assert 'label: "Tone Mapping"' in video, "Video preferences are missing tone mapping"
    for prop in [
        "tone-mapping-enabled", "tone-mapping-target-peak-enabled",
        "tone-mapping-target-peak-text", "tone-mapping-algorithm-index",
        "tone-mapping-param-enabled", "tone-mapping-param-text",
    ]:
        assert f"<=> root.{prop}" in video, f"Video tone mapping is not bound to {prop}"
    assert 'model: root.tone-mapping-algorithm-options;' in video, (
        "Tone-mapping algorithm choices must preserve the reference order"
    )
    section_routes = re.findall(
        r'PreferenceSubNavItem\s*\{[^{}]*label:\s*"([^"]+)";'
        r'[^{}]*selected:\s*root\.selected-interface-section\s*==\s*\d+;'
        r'[^{}]*activated\s*=>\s*\{[^{}]*root\.interface-scroll-position\s*=\s*(root\.[a-z-]+|0px);',
        SOURCE,
        re.DOTALL,
    )
    expected_routes = [
        ("Appearance", "0px"), ("Window", "root.interface-window-target"),
        ("On Screen Controller", "root.interface-osc-target"),
        ("On Screen Display", "root.interface-osd-target"),
        ("Thumbnail Preview", "root.interface-thumbnail-target"),
        ("Picture-in-Picture", "root.interface-pip-target"),
        ("Accessibility", "root.interface-accessibility-target"),
    ]
    assert section_routes == expected_routes, (
        f"Interface links must land on reachable section positions: {section_routes!r}"
    )
    assert 'label: "Always float on top"' in SOURCE and "checked <=> root.always-float-on-top" in SOURCE, (
        "The captured Window section must expose the implemented always-on-top option"
    )
    for label in ["Initial window size", "Initial window position"]:
        assert f'label: "{label}"' in SOURCE, f"The Window section is missing {label!r}"
    for prop in [
        "initial-window-size-axis-index", "initial-window-size-value-text",
        "initial-window-size-unit-index", "initial-window-x-anchor-index",
        "initial-window-x-offset-text", "initial-window-x-unit-index",
        "initial-window-y-anchor-index", "initial-window-y-offset-text",
        "initial-window-y-unit-index",
    ]:
        assert f"<=> root.{prop}" in SOURCE, f"The geometry control is not bound to {prop}"
    for prop in [
        "interface-appearance-card-y", "interface-appearance-card-height",
        "interface-window-title-y", "interface-window-y", "interface-window-card-height", "interface-resize-y",
        "interface-physical-resolution-y", "interface-float-y", "interface-osc-y",
        "interface-osd-y", "interface-thumbnail-y", "interface-pip-y",
        "interface-accessibility-y", "interface-content-height",
    ]:
        assert f"root.{prop}" in SOURCE, f"Expanded geometry must reflow through {prop}"
    assert (
        "interface-content-height: interface-accessibility-y + root.height - 13px;" in SOURCE
        and "interface-accessibility-target: 0px - (interface-accessibility-y - 13px);" in SOURCE
    ), "Expanded Interface layout must leave enough scroll range to reach Accessibility"
    assert (
        "interface-appearance-card-y: 81px;" in SOURCE
        and "interface-appearance-card-height: 122px + liquid-glass-detail-height;" in SOURCE
        and "liquid-glass-detail-height: liquid-glass-expanded ? 96px : 0px;" in SOURCE
        and 'label: root.surface-material-title' in SOURCE
        and '"Liquid Glass"' in (ROOT / "crates/app/src/platform.rs").read_text(encoding="utf-8")
        and '"Translucent Surfaces"' in (ROOT / "crates/app/src/platform.rs").read_text(encoding="utf-8")
        and 'label: "On-Screen Display"' in SOURCE
        and 'label: "On-Screen Controller"' in SOURCE
        and 'label: "Sidebar"' in SOURCE
        and "interface-window-title-y: interface-appearance-card-y + interface-appearance-card-height + 17px;" in SOURCE
        and "interface-window-y: interface-window-title-y + 28px;" in SOURCE
        and "interface-window-card-height: 82px + initial-window-size-detail-height + initial-window-position-detail-height;" in SOURCE
        and "interface-resize-y: interface-window-y + interface-window-card-height + 16px;" in SOURCE
        and "interface-physical-resolution-y: interface-resize-y + 134px;" in SOURCE
        and "height: 118px;" in interface_body.group(1)
    ), "The captured Appearance, Window, and resize cards must retain their default source proportions"
    for target in [
        "interface_window_target", "interface_osc_target", "interface_osd_target",
        "interface_thumbnail_target", "interface_pip_target", "interface_accessibility_target",
    ]:
        assert f"window.get_{target}()" in SEARCH_SOURCE, (
            f"Settings search must use the live {target.replace('_', ' ')} scroll target"
        )
    for label in [
        "Resize window to:",
        "Fit Screen",
        "0.5x Video Size",
        "1x Video Size",
        "1.5x Video Size",
        "2x Video Size",
        "Always when playing a new file",
        "When media is opened manually",
        "Do not resize",
        "Use physical resolution on Retina displays",
    ]:
        assert label in SOURCE, f"The captured Window section is missing {label!r}"
    assert "window-resize-option-index" in SOURCE and "window-resize-timing-index" in SOURCE, (
        "The visible resize choices must retain their selected state"
    )
    assert (
        'label: "Always show float on top status in the title bar"' in SOURCE
        and "checked <=> root.always-show-on-top-icon" in SOURCE
    ), "The captured title-bar pin visibility preference must be represented"
    assert (
        "root.always-float-on-top || root.always-show-on-top-icon : TopmostPin" in PLAYER_SOURCE
        and "root.layout-settings-changed();" in PLAYER_SOURCE
        and 'assets/icons/pin-fill.svg' in PLAYER_SOURCE
        and 'assets/icons/pin.svg' in PLAYER_SOURCE
    ), "The title-bar pin must reflect and toggle the persisted window level"

    # Keep each captured page symbol connected to an original, published asset.
    icon_root = (ROOT / "assets/icons").resolve()
    icons = [icon for _, icon, _, _ in nav]
    assert len(set(icons)) == len(EXPECTED), f"Settings page symbols must be distinct: {icons!r}"
    for icon in icons:
        path = (ROOT / "crates/app/ui" / icon).resolve()
        assert path.is_relative_to(icon_root) and path.is_file(), f"Missing settings symbol: {icon}"

    bodies = {int(index) for index in re.findall(r"if root\.selected-pane == (\d+)", SOURCE)}
    assert bodies == set(range(len(EXPECTED))), f"Missing or extra page routes: {bodies!r}"
    audio_pages = list(re.finditer(
        r"if root\.selected-pane == 3\s*:(.*?)(?=\n\s*if root\.selected-pane == \d+\s*:|\Z)",
        SOURCE,
        re.DOTALL,
    ))
    audio = next(
        (match for match in audio_pages if "x: 200px; y: 0;" in match.group(1)),
        None,
    )
    advanced = re.search(
        r"if root\.selected-pane == 8\s*:(.*?)(?=\n\s*if root\.selected-pane == \d+\s*:|\Z)",
        SOURCE,
        re.DOTALL,
    )
    assert audio and "Allow volume boost (up to 200%)" in audio.group(1), (
        "The opt-in volume cap belongs with Audio settings"
    )
    assert all(
        f"root.selected-audio-section == {index}" in audio.group(1)
        for index in range(3)
    ), "Audio must expose Hardware, Volume and Other sections"
    assert advanced and "Allow volume boost" not in advanced.group(1), (
        "The volume control must not be hidden under Advanced"
    )
    assert (
        re.search(
            r"if fullscreen\s*&&\s*loaded\s*&&\s*controls-visible\s*&&\s*display-time-and-battery-in-fullscreen\s*&&\s*!crop-editing",
            PLAYER_SOURCE,
        )
        and 'text: root.fullscreen-info-time' in PLAYER_SOURCE
        and 'text: root.fullscreen-info-battery' in PLAYER_SOURCE
        and 'changed display-time-and-battery-in-fullscreen => { root.refresh-fullscreen-info(); }' in PLAYER_SOURCE
        and re.search(r"changed fullscreen => \{[^}]*root\.refresh-fullscreen-info\(\);", PLAYER_SOURCE)
        and re.search(r"changed crop-editing => \{[^}]*root\.refresh-fullscreen-info\(\);", PLAYER_SOURCE)
    ), "Fullscreen clock/battery feedback must follow the saved preference and visible controls"
    utilities = re.search(
        r"if root\.selected-pane == 10\s*:\s*ScrollView\s*\{(.*?)(?=\n\s*\}\s*\n\s*if !root\.search-query|\Z)",
        SOURCE,
        re.DOTALL,
    )
    assert utilities and all(
        label in utilities.group(1)
        for label in (
            "Default Application",
            "Restore Alerts",
            "Clear Cache",
            "Clear Saved Playback Progress",
            "Clear Playback History",
            "Clear Thumbnail Cache",
            "Browser Extensions",
        )
    ), "Utilities must retain the source section and action hierarchy"
    assert (
        "utilities-clear-progress" in utilities.group(1)
        and "utilities-clear-history" in utilities.group(1)
        and "utilities-restore-alerts" in utilities.group(1)
        and "utilities-open-default-apps" in utilities.group(1)
    ), "Available Utilities actions must reach their application callbacks"
    history_row = re.search(
        r'UtilitiesActionRow\s*\{\s*y:\s*87px;(.*?)(?=\n\s*UtilitiesActionRow|\Z)',
        utilities.group(1),
        re.DOTALL,
    )
    assert history_row and "action-enabled: false" not in history_row.group(1), (
        "Clear Playback History must be actionable when its store is available"
    )
    assert all(
        token in utilities.group(1)
        for token in (
            "content-height: 700px",
            "Rectangle { width: parent.width; height: 700px; background: #ffffff;",
            "y: 81px; width: parent.width - 40px; height: 42px",
            "y: 290px; width: parent.width - 40px; height: 202px",
            "height: 87px",
            "y: 87px; width: parent.width; height: 74px",
            "y: 161px; width: parent.width; height: 41px",
        )
    ), "Utilities content should retain the measured reference card and cache-row geometry"
    assert (
        "x: parent.width - 50px; y: 0; width: 40px; height: parent.height" in SOURCE
        and "width: 40px; height: 26px; border-radius: 7px" in SOURCE
    ), "Utilities action pills should match the captured 40 by 26 point target"
    print("Settings order, page routes, Audio placement, fullscreen info and Utilities action routing match the current inventory.")


if __name__ == "__main__":
    main()
