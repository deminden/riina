"""Check that the settings sidebar retains IINA's default page order and routes."""
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = (ROOT / "crates/ui/ui/preferences.slint").read_text(encoding="utf-8")
NAV_SOURCE = (ROOT / "crates/ui/ui/preferences-navigation.slint").read_text(encoding="utf-8")
CONTROLS_SOURCE = (ROOT / "crates/ui/ui/preferences-controls.slint").read_text(encoding="utf-8")
UTILITIES_SOURCE = (ROOT / "crates/ui/ui/preferences-utilities.slint").read_text(encoding="utf-8")
THEME_SOURCE = (ROOT / "crates/ui/ui/theme.slint").read_text(encoding="utf-8")
AUDIO_SOURCE = (ROOT / "crates/ui/ui/preferences-audio.slint").read_text(encoding="utf-8")
ADVANCED_SOURCE = (ROOT / "crates/ui/ui/preferences-advanced.slint").read_text(encoding="utf-8")
VIDEO_SOURCE = (ROOT / "crates/ui/ui/preferences-video.slint").read_text(encoding="utf-8")
GENERAL_SOURCE = (ROOT / "crates/ui/ui/preferences-general.slint").read_text(encoding="utf-8")
INTERFACE_SOURCE = (ROOT / "crates/ui/ui/preferences-interface.slint").read_text(encoding="utf-8")
SUBTITLES_SOURCE = (ROOT / "crates/ui/ui/preferences-subtitles.slint").read_text(encoding="utf-8")
NETWORK_SOURCE = (ROOT / "crates/ui/ui/preferences-network.slint").read_text(encoding="utf-8")
PLAYER_SOURCE = (ROOT / "crates/ui/ui/player.slint").read_text(encoding="utf-8")
PLAYER_CHROME_SOURCE = (ROOT / "crates/ui/ui/player-chrome-controls.slint").read_text(encoding="utf-8")
PLAYER_TITLE_SOURCE = (ROOT / "crates/ui/ui/player-title-bar.slint").read_text(encoding="utf-8")
SIDEBAR_FRAME_SOURCE = (ROOT / "crates/ui/ui/player-sidebar-frame.slint").read_text(encoding="utf-8")
SIDEBAR_TABS_SOURCE = (ROOT / "crates/ui/ui/player-sidebar-tabs.slint").read_text(encoding="utf-8")
PLAYER_OSD_SOURCE = (ROOT / "crates/ui/ui/player-osd.slint").read_text(encoding="utf-8")
QUICK_TABS_SOURCE = (ROOT / "crates/ui/ui/quick-settings-tabs.slint").read_text(encoding="utf-8")
QUICK_AUDIO_SOURCE = (ROOT / "crates/ui/ui/quick-settings-audio.slint").read_text(encoding="utf-8")
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
        NAV_SOURCE,
        re.DOTALL,
    )
    expected_routes = [(label, str(index), str(index)) for index, label in enumerate(EXPECTED)]
    routes = [(label, selected, activated) for label, _, selected, activated in nav]
    assert routes == expected_routes, f"Unexpected settings navigation: {routes!r}"

    assert "if root.selected-pane == 0 : GeneralPreferencesPage" in SOURCE
    assert (
        "GeneralPreferencesPage inherits ScrollView" in GENERAL_SOURCE
        and "content-y <=> root.general-scroll-position" in GENERAL_SOURCE
        and "general-scroll-position <=> root.general-scroll-position;" in SOURCE
    ), "General settings must remain one continuous scroll page"
    headings = [
        GENERAL_SOURCE.find(f'text: "{label}"')
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

    for section, target in (("History", "history"), ("Playlist", "playlist"), ("Screenshots", "screenshots")):
        assert f"13px - root.general-{target}-y" in SOURCE, (
            f"{section} navigation must derive its target from the visible heading"
        )
        assert re.search(
            rf'y: root\.general-{target}-y;\s*text: "{section}"',
            GENERAL_SOURCE,
        ), f"{section} heading and navigation must share one position"

    assert "if root.selected-pane == 4 : SubtitlesPreferencesPage" in SOURCE
    assert (
        "SubtitlesPreferencesPage inherits ScrollView" in SUBTITLES_SOURCE
        and "content-y <=> root.subtitle-scroll-position" in SUBTITLES_SOURCE
        and "subtitle-scroll-position <=> root.subtitle-scroll-position;" in SOURCE
    ), "Subtitles must remain a continuous scroll page"
    subtitle_body = SUBTITLES_SOURCE
    subtitle_sections = ["Auto Load", "ASS Subtitles", "Text Subtitles", "Position", "Online Subtitles", "Other"]
    headings = [subtitle_body.find(f'text: "{label}"') for label in subtitle_sections]
    assert all(position >= 0 for position in headings) and headings == sorted(headings), (
        "Subtitle sections must retain reference order, including Online before Other"
    )
    assert all(f"root.reveal-subtitle-section({index})" in NAV_SOURCE for index in range(6)), (
        "Every subtitle subsection needs a working reveal route"
    )
    assert "window.invoke_reveal_subtitle_section" in SEARCH_SOURCE, (
        "Search must reveal subtitle sections through their live layout, not fixed offsets"
    )
    assert all(token in SOURCE for token in (
        "78px + subtitle-autoload-height + 20px",
        "subtitle-text-y + 26px + subtitle-text-height + 20px",
        "subtitle-online-y + 26px + subtitle-online-height + 20px",
        "root.height + subtitle-other-y - 16px",
    )), "Expanded subtitle sections and provider variants must move later anchors and retain a full viewport"
    assert all(token in subtitle_body for token in (
        'accessible-label: "Primary subtitles"', 'accessible-label: "Secondary subtitles"',
        'accessible-label: "Choose subtitle font"', 'root.choose-subtitle-font();',
        'root.choose-subtitle-override(index);', 'root.add-subtitle-language-token(entry);',
        'root.remove-subtitle-language-token(index);',
    )), "Subtitle layout changes must retain lane, font and language actions"

    assert "if root.selected-pane == 5 : NetworkPreferencesPage" in SOURCE
    assert (
        "NetworkPreferencesPage inherits ScrollView" in NETWORK_SOURCE
        and "network-scroll-position <=> root.network-scroll-position;" in SOURCE
    ), "Network must remain one scrollable page"
    network_body = NETWORK_SOURCE
    assert "if root.selected-network-section" not in network_body, (
        "Network navigation must reveal sections rather than hide adjacent settings"
    )
    assert all(f"root.reveal-network-section({index})" in NAV_SOURCE for index in range(3)), (
        "Every Network subsection needs a reveal route"
    )
    assert "window.invoke_reveal_network_section" in SEARCH_SOURCE, (
        "Network search must use the same reveal route as navigation"
    )
    assert all(token in network_body for token in (
        "content-y <=> root.network-scroll-position", "content-height: root.network-content-height",
        "height: root.network-content-height", "root.network-connection-y + 26px",
        "root.network-online-y + 126px", "NetworkSettingsState.network-ytdl-enabled && !NetworkSettingsState.network-ytdl-plugin-active",
        "NetworkSettingsState.network-ytdl-enabled = NetworkSettingsState.network-ytdl-ui-enabled",
    )), "Network must preserve continuous extent, section geometry and extractor ownership rules"

    sidebar_sample = SIDEBAR_FRAME_SOURCE.index("sidebar-video-sample := Image")
    sidebar_tint = SIDEBAR_FRAME_SOURCE.index("sidebar-light-tint := Rectangle")
    sidebar_content_slot = SIDEBAR_FRAME_SOURCE.index("@children")
    assert sidebar_sample < sidebar_tint < sidebar_content_slot and re.search(
        r"sidebar-frame := PlayerSidebarFrame\s*\{.*?\n\s*sidebar-content := VerticalLayout",
        PLAYER_SOURCE,
        re.DOTALL,
    ), (
        "The light Windows sidebar tint must cover its video sample and stay behind controls"
    )
    assert "root.windows-sidebar-material-active && !root.dark-theme &&" in SIDEBAR_FRAME_SOURCE, (
        "The light tint must not change dark reference calibration or opaque policy fallback"
    )
    assert (
        "expanded-tab-labels: sidebar-frame.expanded-tab-labels;" in PLAYER_SOURCE
        and "show-label: root.expanded-tab-labels;" in SIDEBAR_TABS_SOURCE
        and QUICK_TABS_SOURCE.count("show-label: root.show-label;") == 4
    ), (
        "Every Quick Settings category must share narrow-window label compression"
    )
    assert "if root.selected && root.show-label : Text" in QUICK_TABS_SOURCE, (
        "Selected tab text must collapse with its width at narrow sidebar sizes"
    )
    sidebar_scroll = PLAYER_SOURCE.index("if pane-kind != 5 : QuickSettingsScrollPage")
    audio_page = PLAYER_SOURCE.index("if pane-kind == 1 : AudioQuickSettingsPage", sidebar_scroll)
    assert sidebar_scroll < audio_page < PLAYER_SOURCE.index("for row[index]", audio_page), (
        "Audio controls must be in the shared sidebar scrollable page"
    )
    audio_tracks = QUICK_AUDIO_SOURCE.index("tracks: root.audio-tracks;")
    equalizer_scroll = QUICK_AUDIO_SOURCE.index("equalizer-scroll := ScrollView")
    assert audio_tracks < equalizer_scroll, (
        "Audio tracks, load/delay controls and equalizer must share the Audio page"
    )
    assert "content-height: max(self.visible-height, sidebar-page.preferred-height);" in PLAYER_SOURCE, (
        "The shared sidebar scroll extent must include all settings, even in short windows"
    )
    input_guard = SIDEBAR_FRAME_SOURCE.index("Unhandled input belongs to this panel")
    assert sidebar_tint < input_guard < sidebar_content_slot and (
        "scroll-event(event) => { return accept; }"
        in SIDEBAR_FRAME_SOURCE[input_guard:sidebar_content_slot]
    ), (
        "Unused sidebar input must not fall through to video actions"
    )
    assert "content-width: max(self.visible-width, 300px);" in QUICK_AUDIO_SOURCE, (
        "A narrow equalizer must scroll instead of compressing or clipping its ten bands"
    )
    assert re.search(
        r"equalizer-scroll := ScrollView\s*\{\s*width: parent\.width - \(root\.compact-ui \? 20px : 24px\);",
        QUICK_AUDIO_SOURCE,
    ), "The equalizer viewport must retain its card's inner margins"

    interface_sections = re.findall(
        r'PreferenceSubNavItem\s*\{[^{}]*label:\s*"([^"]+)";'
        r'[^{}]*selected:\s*root\.selected-interface-section\s*==\s*(\d+);',
        NAV_SOURCE,
        re.DOTALL,
    )
    expected_sections = [
        (label, str(index)) for index, label in enumerate(EXPECTED_INTERFACE_SECTIONS)
    ]
    assert interface_sections == expected_sections, (
        f"Unexpected Interface subsection navigation: {interface_sections!r}"
    )
    assert "if root.selected-pane == 1 : InterfacePreferencesPage" in SOURCE
    assert (
        "InterfacePreferencesPage inherits ScrollView" in INTERFACE_SOURCE
        and "content-y <=> root.interface-scroll-position" in INTERFACE_SOURCE
        and "interface-scroll-position <=> root.interface-scroll-position;" in SOURCE
    ), "Interface settings must remain one continuous scroll page"
    scroll_sections = re.findall(
        r"(?:if|else if) root\.interface-scroll-position > root\.(interface-[a-z-]+-threshold) "
        r"\{ root\.selected-interface-section = (\d+); \}",
        INTERFACE_SOURCE,
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
    assert "else { root.selected-interface-section = 6; }" in INTERFACE_SOURCE, (
        "The final Interface subsection must remain selectable at the scroll limit"
    )
    assert "if root.selected-pane == 2 : VideoPreferencesPage" in SOURCE
    assert "VideoPreferencesPage inherits ScrollView" in VIDEO_SOURCE, (
        "Video preferences must keep a dedicated scrollable pane"
    )
    video = VIDEO_SOURCE
    assert "content-y <=> VideoSettingsState.video-scroll-position" in video, (
        "Video Settings search results must scroll to their selected section"
    )
    assert "video.set_video_scroll_position" in SEARCH_SOURCE
    for label in ["Decoding", "Number of threads", "Hardware decoder", "Color and HDR"]:
        assert f'text: "{label}"' in video, f"Video preferences are missing {label!r}"
    assert 'label: "Tone Mapping"' in video, "Video preferences are missing tone mapping"
    for prop in [
        "tone-mapping-enabled", "tone-mapping-target-peak-enabled",
        "tone-mapping-target-peak-text", "tone-mapping-algorithm-index",
        "tone-mapping-param-enabled", "tone-mapping-param-text",
    ]:
        assert f"<=> VideoSettingsState.{prop}" in video, f"Video tone mapping is not bound to {prop}"
        assert re.search(rf"in-out property <[^>]+> {prop}:", video), f"Missing Video Settings state for {prop}"
    assert 'model: VideoSettingsState.tone-mapping-algorithm-options;' in video, (
        "Tone-mapping algorithm choices must preserve the reference order"
    )
    section_routes = re.findall(
        r'PreferenceSubNavItem\s*\{[^{}]*label:\s*"([^"]+)";'
        r'[^{}]*selected:\s*root\.selected-interface-section\s*==\s*\d+;'
        r'[^{}]*activated\s*=>\s*\{[^{}]*root\.interface-scroll-position\s*=\s*(root\.[a-z-]+|0px);',
        NAV_SOURCE,
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
    assert 'label: "Always float on top"' in INTERFACE_SOURCE and "checked <=> root.always-float-on-top" in INTERFACE_SOURCE, (
        "The captured Window section must expose the implemented always-on-top option"
    )
    for label in ["Initial window size", "Initial window position"]:
        assert f'label: "{label}"' in INTERFACE_SOURCE, f"The Window section is missing {label!r}"
    for prop in [
        "initial-window-size-axis-index", "initial-window-size-value-text",
        "initial-window-size-unit-index", "initial-window-x-anchor-index",
        "initial-window-x-offset-text", "initial-window-x-unit-index",
        "initial-window-y-anchor-index", "initial-window-y-offset-text",
        "initial-window-y-unit-index",
    ]:
        assert f"<=> root.{prop}" in INTERFACE_SOURCE, f"The geometry control is not bound to {prop}"
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
        and 'label: root.surface-material-title' in INTERFACE_SOURCE
        and '"Liquid Glass"' in (ROOT / "crates/app/src/platform/mod.rs").read_text(encoding="utf-8")
        and '"Translucent Surfaces"' in (ROOT / "crates/app/src/platform/mod.rs").read_text(encoding="utf-8")
        and 'label: "On-Screen Display"' in INTERFACE_SOURCE
        and 'label: "On-Screen Controller"' in INTERFACE_SOURCE
        and 'label: "Sidebar"' in INTERFACE_SOURCE
        and "interface-window-title-y: interface-appearance-card-y + interface-appearance-card-height + 17px;" in SOURCE
        and "interface-window-y: interface-window-title-y + 28px;" in SOURCE
        and "interface-window-card-height: 82px + initial-window-size-detail-height + initial-window-position-detail-height;" in SOURCE
        and "interface-resize-y: interface-window-y + interface-window-card-height + 16px;" in SOURCE
        and "interface-physical-resolution-y: interface-resize-y + 134px;" in SOURCE
        and "height: 118px;" in INTERFACE_SOURCE
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
        assert label in INTERFACE_SOURCE, f"The captured Window section is missing {label!r}"
    assert "window-resize-option-index" in SOURCE and "window-resize-timing-index" in SOURCE, (
        "The visible resize choices must retain their selected state"
    )
    assert (
        'label: "Always show float on top status in the title bar"' in INTERFACE_SOURCE
        and "checked <=> root.always-show-on-top-icon" in INTERFACE_SOURCE
    ), "The captured title-bar pin visibility preference must be represented"
    assert (
        "root.always-float-on-top || root.always-show-on-top-icon : TopmostPin" in PLAYER_TITLE_SOURCE
        and "pinned: root.always-float-on-top;" in PLAYER_TITLE_SOURCE
        and "root.window-level-toggle-requested(value);" in PLAYER_TITLE_SOURCE
        and "always-float-on-top: root.always-float-on-top;" in PLAYER_SOURCE
        and "always-show-on-top-icon: root.always-show-on-top-icon;" in PLAYER_SOURCE
        and "root.layout-settings-changed();" in PLAYER_SOURCE
        and 'assets/icons/pin-fill.svg' in PLAYER_CHROME_SOURCE
        and 'assets/icons/pin.svg' in PLAYER_CHROME_SOURCE
    ), "The title-bar pin must reflect and toggle the persisted window level"

    # Keep each captured page symbol connected to an original, published asset.
    icon_root = (ROOT / "assets/icons").resolve()
    icons = [icon for _, icon, _, _ in nav]
    assert len(set(icons)) == len(EXPECTED), f"Settings page symbols must be distinct: {icons!r}"
    for icon in icons:
        path = (ROOT / "crates/ui/ui" / icon).resolve()
        assert path.is_relative_to(icon_root) and path.is_file(), f"Missing settings symbol: {icon}"

    bodies = {int(index) for index in re.findall(r"if root\.selected-pane == (\d+)", SOURCE)}
    assert bodies == set(range(len(EXPECTED))), f"Missing or extra page routes: {bodies!r}"
    assert "if root.selected-pane == 3 : AudioPreferencesPage" in SOURCE, (
        "Audio must remain the fourth Settings page"
    )
    assert "Allow volume boost (up to 200%)" in AUDIO_SOURCE, (
        "The opt-in volume cap belongs with Audio settings"
    )
    audio_body = AUDIO_SOURCE
    assert "ScrollView" in audio_body and "content-y <=> root.audio-scroll-position" in audio_body, (
        "Audio sections must share one scroll page"
    )
    assert "audio-scroll-position <=> root.audio-scroll-position;" in SOURCE
    headings = [audio_body.find(f'text: "{label}"') for label in ("Hardware", "Volume", "Other")]
    assert all(position >= 0 for position in headings) and headings == sorted(headings), (
        "Audio must expose Hardware, Volume and Other in reference order"
    )
    assert "if root.selected-audio-section" not in audio_body, (
        "Audio navigation must scroll to sections rather than hide adjacent sections"
    )
    assert "window.invoke_reveal_audio_section" in SEARCH_SOURCE and all(
        f"root.reveal-audio-section({index})" in NAV_SOURCE for index in range(3)
    ), "Audio search and navigation must use the same section reveal function"
    assert (
        "content-height: root.audio-content-height;" in audio_body
        and "height: root.audio-content-height;" in audio_body
        and "root.height + audio-other-title-y - 16px" in SOURCE
    ), "Jumping to Other must retain a full viewport of Audio background"
    assert "if root.selected-pane == 8 : AdvancedPreferencesPage" in SOURCE
    assert "Allow volume boost" not in ADVANCED_SOURCE, (
        "The volume control must not be hidden under Advanced"
    )
    assert (
        re.search(
            r"if fullscreen\s*&&\s*loaded\s*&&\s*controls-visible\s*&&\s*display-time-and-battery-in-fullscreen\s*&&\s*!crop-editing\s*:\s*FullscreenInfoOsd",
            PLAYER_SOURCE,
        )
        and all(
            f"{field}: root.{field};" in PLAYER_SOURCE and f"text: root.{field};" in PLAYER_OSD_SOURCE
            for field in ("fullscreen-info-time", "fullscreen-info-battery")
        )
        and 'changed display-time-and-battery-in-fullscreen => { root.refresh-fullscreen-info(); }' in PLAYER_SOURCE
        and re.search(r"changed fullscreen => \{[^}]*root\.refresh-fullscreen-info\(\);", PLAYER_SOURCE)
        and re.search(r"changed crop-editing => \{[^}]*root\.refresh-fullscreen-info\(\);", PLAYER_SOURCE)
    ), "Fullscreen clock/battery feedback must follow the saved preference and visible controls"

    utilities_route = re.search(
        r"if root\.selected-pane == 10\s*:\s*UtilitiesPreferencesPage\s*\{(.*?)(?=\n    \})",
        SOURCE,
        re.DOTALL,
    )
    assert utilities_route, "Utilities must remain the final settings page"
    assert all(
        label in UTILITIES_SOURCE
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

    for action in (
        "utilities-clear-progress", "utilities-clear-history", "utilities-restore-alerts",
        "utilities-open-default-apps", "utilities-clear-thumbnail-cache",
    ):
        assert f"root.{action}();" in UTILITIES_SOURCE, f"Utilities row must activate {action}"
        assert f"{action} => {{ root.{action}(); }}" in utilities_route.group(1), (
            f"Utilities page must forward {action} to its window"
        )
    history_row = re.search(
        r'UtilitiesActionRow\s*\{\s*y:\s*87px;(.*?)(?=\n\s*UtilitiesActionRow|\Z)',
        UTILITIES_SOURCE,
        re.DOTALL,
    )
    assert history_row and "action-enabled: false" not in history_row.group(1), (
        "Clear Playback History must be actionable when its store is available"
    )
    assert all(
        token in UTILITIES_SOURCE
        for token in (
            "content-height: 700px",
            "y: 81px; width: parent.width - 40px; height: 42px",
            "y: 290px; width: parent.width - 40px; height: 202px",
            "height: 87px",
            "y: 87px; width: parent.width; height: 74px",
            "y: 161px; width: parent.width; height: 41px",
        )
    ), "Utilities content should retain the measured reference card and cache-row geometry"
    assert (
        "Rectangle { width: parent.width; height: 700px;" in UTILITIES_SOURCE
        and "background: Theme.settings-content-surface(root.mica-backdrop-enabled);" in UTILITIES_SOURCE
        and re.search(
            r"function settings-content-surface\(material: bool\) -> color \{\s*"
            r"return material \? #ffffff80 : #ffffff;",
            THEME_SOURCE,
        )
    ), "Utilities should retain the measured content surface with the selected Windows material"
    assert (
        "x: parent.width - 50px; y: 0; width: 40px; height: parent.height" in CONTROLS_SOURCE
        and "width: 40px; height: 26px; border-radius: 7px" in CONTROLS_SOURCE
    ), "Utilities action pills should match the captured 40 by 26 point target"
    print("Settings routes, scrolling, sidebar compression and action inventories verified.")


if __name__ == "__main__":
    main()
