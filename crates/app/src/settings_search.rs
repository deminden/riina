//! Search only controls currently represented in the Settings UI.
use crate::{PreferencesSearchResult, PreferencesWindow};
use slint::{ComponentHandle, Image, Model, ModelRc, VecModel};
use std::collections::BTreeSet;

#[derive(Clone, Copy)]
struct Entry {
    page_index: i32,
    page: &'static str,
    section: &'static str,
    title: &'static str,
    terms: &'static str,
}

// The index follows visible user-facing labels. Aliases make common queries
// useful without advertising controls that the current page does not expose.
const ENTRIES: &[Entry] = &[
    Entry {
        page_index: 0,
        page: "General",
        section: "",
        title: "General",
        terms: "preferences",
    },
    Entry {
        page_index: 0,
        page: "General",
        section: "Behavior",
        title: "When media is opened",
        terms: "pause fullscreen open",
    },
    Entry {
        page_index: 0,
        page: "General",
        section: "Behavior",
        title: "Pause",
        terms: "pause when opened",
    },
    Entry {
        page_index: 0,
        page: "General",
        section: "Behavior",
        title: "Enter fullscreen",
        terms: "full screen",
    },
    Entry {
        page_index: 0,
        page: "General",
        section: "Screenshots",
        title: "Save to",
        terms: "screenshot folder destination path",
    },
    Entry {
        page_index: 0,
        page: "General",
        section: "Screenshots",
        title: "Format",
        terms: "screenshot png jpeg webp jxl",
    },
    Entry {
        page_index: 0,
        page: "General",
        section: "Screenshots",
        title: "Copy to clipboard",
        terms: "screenshot copy",
    },
    Entry {
        page_index: 0,
        page: "General",
        section: "Screenshots",
        title: "Include subtitles",
        terms: "subtitle captions screenshot",
    },
    Entry {
        page_index: 0,
        page: "General",
        section: "Screenshots",
        title: "Show previews after taking screenshots",
        terms: "preview screenshot",
    },
    Entry {
        page_index: 0,
        page: "General",
        section: "History",
        title: "Resume last position",
        terms: "resume playback history",
    },
    Entry {
        page_index: 1,
        page: "Interface",
        section: "Appearance",
        title: "Appearance",
        terms: "dark light system theme",
    },
    Entry {
        page_index: 1,
        page: "Interface",
        section: "Appearance",
        title: "Liquid Glass",
        terms: "glass material translucency overlay controller sidebar",
    },
    Entry {
        page_index: 1,
        page: "Interface",
        section: "Appearance",
        title: "Compact interface",
        terms: "compact controls spacing",
    },
    Entry {
        page_index: 1,
        page: "Interface",
        section: "Window",
        title: "Initial window size",
        terms: "startup initial width height dimensions size points screen percent",
    },
    Entry {
        page_index: 1,
        page: "Interface",
        section: "Window",
        title: "Initial window position",
        terms: "startup initial position x y offset left right bottom top screen",
    },
    Entry {
        page_index: 1,
        page: "Interface",
        section: "Window",
        title: "Resize window to",
        terms: "resize fit screen video size half one 1.5x 2x always manually never",
    },
    Entry {
        page_index: 1,
        page: "Interface",
        section: "Window",
        title: "Use physical resolution on Retina displays",
        terms: "physical resolution retina pixels display scale",
    },
    Entry {
        page_index: 1,
        page: "Interface",
        section: "Window",
        title: "Always float on top",
        terms: "stay above other windows floating window level",
    },
    Entry {
        page_index: 1,
        page: "Interface",
        section: "Window",
        title: "Always show float on top status in the title bar",
        terms: "always show pin icon topmost status title bar",
    },
    Entry {
        page_index: 1,
        page: "Interface",
        section: "On-Screen Controller",
        title: "Floating, Top, Bottom",
        terms: "osc position layout",
    },
    Entry {
        page_index: 1,
        page: "Interface",
        section: "On-Screen Controller",
        title: "Dock control bar and titlebar",
        terms: "dock controls",
    },
    Entry {
        page_index: 1,
        page: "Interface",
        section: "On-Screen Display",
        title: "Enable OSD",
        terms: "overlay display messages",
    },
    Entry {
        page_index: 1,
        page: "Interface",
        section: "On-Screen Display",
        title: "Suppress file-start messages",
        terms: "overlay file start notification",
    },
    Entry {
        page_index: 1,
        page: "Interface",
        section: "On-Screen Display",
        title: "Suppress pause and resume messages",
        terms: "overlay pause resume playback",
    },
    Entry {
        page_index: 1,
        page: "Interface",
        section: "On-Screen Display",
        title: "Suppress seek messages",
        terms: "overlay seeking",
    },
    Entry {
        page_index: 1,
        page: "Interface",
        section: "On-Screen Display",
        title: "Suppress speed messages",
        terms: "overlay playback speed",
    },
    Entry {
        page_index: 1,
        page: "Interface",
        section: "On-Screen Display",
        title: "Auto hide after",
        terms: "overlay timeout seconds duration",
    },
    Entry {
        page_index: 1,
        page: "Interface",
        section: "On-Screen Display",
        title: "Text size",
        terms: "overlay font point size",
    },
    Entry {
        page_index: 1,
        page: "Interface",
        section: "On-Screen Display",
        title: "Time and battery in full screen",
        terms: "fullscreen status time clock battery information",
    },
    Entry {
        page_index: 2,
        page: "Video",
        section: "Decoding",
        title: "Number of threads",
        terms: "video decoder lavc count concurrency automatic",
    },
    Entry {
        page_index: 2,
        page: "Video",
        section: "Decoding",
        title: "Hardware decoder",
        terms: "disabled auto copy acceleration",
    },
    Entry {
        page_index: 2,
        page: "Video",
        section: "Live Text",
        title: "Live Text",
        terms: "recognize OCR selectable frame text accessibility",
    },
    Entry {
        page_index: 2,
        page: "Video",
        section: "Color and HDR",
        title: "Tone Mapping",
        terms: "hdr peak nits algorithm clip mobius reinhard hable bt2390 gamma linear parameter",
    },
    Entry {
        page_index: 3,
        page: "Audio",
        section: "Hardware",
        title: "Audio threads and driver",
        terms: "hardware backend output device wasapi coreaudio pipewire",
    },
    Entry {
        page_index: 3,
        page: "Audio",
        section: "Volume",
        title: "Initial volume and ReplayGain",
        terms: "volume replay gain preamp clipping fallback",
    },
    Entry {
        page_index: 3,
        page: "Audio",
        section: "Other",
        title: "Gapless playback and languages",
        terms: "gapless audio alang language",
    },
    Entry {
        page_index: 3,
        page: "Audio",
        section: "Volume",
        title: "Allow volume boost (up to 200%)",
        terms: "amplify gain volume cap",
    },
    Entry {
        page_index: 4,
        page: "Subtitles",
        section: "Auto Load",
        title: "Subtitle matching mode",
        terms: "disabled filename intelligent auto load",
    },
    Entry {
        page_index: 4,
        page: "Subtitles",
        section: "Auto Load",
        title: "Priority keywords",
        terms: "filename comma separated",
    },
    Entry {
        page_index: 4,
        page: "Subtitles",
        section: "Auto Load",
        title: "Search paths",
        terms: "directories subtitle folder",
    },
    Entry {
        page_index: 4,
        page: "Subtitles",
        section: "ASS Subtitles",
        title: "Override level",
        terms: "primary secondary no yes scale force strip",
    },
    Entry {
        page_index: 4,
        page: "Subtitles",
        section: "Text Subtitles",
        title: "Font and size",
        terms: "choose bold italic color background",
    },
    Entry {
        page_index: 4,
        page: "Subtitles",
        section: "Text Subtitles",
        title: "Border",
        terms: "outline size color",
    },
    Entry {
        page_index: 4,
        page: "Subtitles",
        section: "Text Subtitles",
        title: "Shadow and other styles",
        terms: "offset blur spacing advanced",
    },
    Entry {
        page_index: 4,
        page: "Subtitles",
        section: "Position",
        title: "Alignment and margins",
        terms: "left center right top bottom x y",
    },
    Entry {
        page_index: 4,
        page: "Subtitles",
        section: "Position",
        title: "Vertical position",
        terms: "letterboxes fullscreen window scaling",
    },
    Entry {
        page_index: 4,
        page: "Subtitles",
        section: "Other",
        title: "Preferred language",
        terms: "language codes",
    },
    Entry {
        page_index: 4,
        page: "Subtitles",
        section: "Other",
        title: "Default encoding",
        terms: "charset text",
    },
    Entry {
        page_index: 5,
        page: "Network",
        section: "Cache",
        title: "Prefetch and buffer network data",
        terms: "cache buffer networking download",
    },
    Entry {
        page_index: 5,
        page: "Network",
        section: "Cache",
        title: "Buffer before starting playback",
        terms: "startup initial wait buffering",
    },
    Entry {
        page_index: 5,
        page: "Network",
        section: "Cache",
        title: "Seconds to buffer before restarting",
        terms: "cache wait seconds threshold",
    },
    Entry {
        page_index: 5,
        page: "Network",
        section: "Cache",
        title: "Seconds to prefetch",
        terms: "network cache seconds duration",
    },
    Entry {
        page_index: 5,
        page: "Network",
        section: "Cache",
        title: "Cache buffer size (KiB)",
        terms: "memory size bytes",
    },
    Entry {
        page_index: 5,
        page: "Network",
        section: "Cache",
        title: "Show throbber when buffering",
        terms: "spinner loading progress",
    },
    Entry {
        page_index: 5,
        page: "Network",
        section: "Cache",
        title: "Show throbber when seeking",
        terms: "spinner loading progress",
    },
    Entry {
        page_index: 5,
        page: "Network",
        section: "",
        title: "Network",
        terms: "preferences",
    },
    Entry {
        page_index: 5,
        page: "Network",
        section: "Network",
        title: "User agent",
        terms: "browser client header http request",
    },
    Entry {
        page_index: 5,
        page: "Network",
        section: "Network",
        title: "HTTP proxy",
        terms: "proxy server address",
    },
    Entry {
        page_index: 5,
        page: "Network",
        section: "Network",
        title: "RTSP transport",
        terms: "lavf tcp udp http stream",
    },
    Entry {
        page_index: 5,
        page: "Network",
        section: "Online Videos (yt-dlp)",
        title: "Enable mpv's youtube-dl script",
        terms: "online video youtube yt-dlp extractor streaming",
    },
    Entry {
        page_index: 5,
        page: "Network",
        section: "Online Videos (yt-dlp)",
        title: "Custom youtube-dl path",
        terms: "directory executable binary folder yt-dlp",
    },
    Entry {
        page_index: 5,
        page: "Network",
        section: "Online Videos (yt-dlp)",
        title: "Raw options",
        terms: "youtube-dl yt-dlp format key value",
    },
    Entry {
        page_index: 6,
        page: "Control",
        section: "",
        title: "Control",
        terms: "preferences",
    },
    Entry {
        page_index: 7,
        page: "Key Bindings",
        section: "",
        title: "Key Bindings",
        terms: "keyboard shortcuts hotkeys",
    },
    Entry {
        page_index: 8,
        page: "Advanced",
        section: "",
        title: "Advanced",
        terms: "preferences",
    },
    Entry {
        page_index: 8,
        page: "Advanced",
        section: "",
        title: "Enable advanced settings",
        terms: "restart mpv options logging",
    },
    Entry {
        page_index: 8,
        page: "Advanced",
        section: "Logging",
        title: "Log level",
        terms: "verbose debug warning error",
    },
    Entry {
        page_index: 8,
        page: "Advanced",
        section: "Logging",
        title: "Enable logging to file",
        terms: "log directory viewer",
    },
    Entry {
        page_index: 8,
        page: "Advanced",
        section: "mpv Settings",
        title: "Use mpv's OSD",
        terms: "on screen display",
    },
    Entry {
        page_index: 8,
        page: "Advanced",
        section: "mpv Settings",
        title: "Use config directory",
        terms: "mpv folder configuration",
    },
    Entry {
        page_index: 8,
        page: "Advanced",
        section: "mpv Settings",
        title: "Additional mpv options",
        terms: "custom option key value",
    },
    Entry {
        page_index: 9,
        page: "Plugins",
        section: "",
        title: "Plugins",
        terms: "extensions",
    },
    Entry {
        page_index: 10,
        page: "Utilities",
        section: "",
        title: "Utilities",
        terms: "tools",
    },
    Entry {
        page_index: 10,
        page: "Utilities",
        section: "Default Application",
        title: "Set Riina as the Default Application",
        terms: "file associations default media player",
    },
    Entry {
        page_index: 10,
        page: "Utilities",
        section: "Restore Alerts",
        title: "Restore Suppressed Alerts",
        terms: "reset hidden notifications do not show again",
    },
    Entry {
        page_index: 10,
        page: "Utilities",
        section: "Clear Cache",
        title: "Clear Saved Playback Progress",
        terms: "watch later resume position delete",
    },
    Entry {
        page_index: 10,
        page: "Utilities",
        section: "Clear Cache",
        title: "Clear Playback History",
        terms: "recent files playback history",
    },
    Entry {
        page_index: 10,
        page: "Utilities",
        section: "Clear Cache",
        title: "Clear Thumbnail Cache",
        terms: "thumbnails preview cache",
    },
    Entry {
        page_index: 10,
        page: "Utilities",
        section: "Browser Extensions",
        title: "Get Browser Extensions for Riina",
        terms: "browser extension open links current webpage",
    },
];

fn page_icon(page_index: i32) -> Image {
    let data: &[u8] = match page_index {
        0 => include_bytes!("../ui/../../../assets/icons/settings.svg"),
        1 => include_bytes!("../ui/../../../assets/icons/interface.svg"),
        2 => include_bytes!("../ui/../../../assets/icons/video-page.svg"),
        3 => include_bytes!("../ui/../../../assets/icons/audio-page.svg"),
        4 => include_bytes!("../ui/../../../assets/icons/subtitles-page.svg"),
        5 => include_bytes!("../ui/../../../assets/icons/network.svg"),
        6 => include_bytes!("../ui/../../../assets/icons/control.svg"),
        7 => include_bytes!("../ui/../../../assets/icons/keybindings.svg"),
        8 => include_bytes!("../ui/../../../assets/icons/advanced.svg"),
        9 => include_bytes!("../ui/../../../assets/icons/plugins.svg"),
        _ => include_bytes!("../ui/../../../assets/icons/utilities.svg"),
    };
    Image::load_from_svg_data(data).unwrap_or_default()
}

// These default offsets label search entries; activation reads live Slint targets
// so expanding preference details never leaves search with stale coordinates.
fn anchor_y(entry: &Entry) -> i32 {
    match (entry.page_index, entry.section, entry.title) {
        (0, "Behavior", "When media is opened") => 60,
        (0, "Behavior", _) => 80,
        (0, "Screenshots", "Save to") => 160,
        (0, "Screenshots", "Format") => 188,
        (0, "Screenshots", "Copy to clipboard") => 212,
        (0, "Screenshots", "Include subtitles") => 268,
        (0, "Screenshots", _) => 292,
        (0, "History", _) => 334,
        (1, "Appearance", _) => 0,
        (1, "Window", _) => 207,
        (1, "On-Screen Controller", _) => 619,
        (1, "On-Screen Display", _) => 735,
        (1, "Thumbnail Preview", _) => 843,
        (1, "Picture-in-Picture", _) => 939,
        (1, "Accessibility", _) => 1035,
        (2, "Decoding", "Number of threads") => 80,
        (2, "Decoding", _) => 145,
        (2, "Live Text", _) => 378,
        (2, "Color and HDR", _) => 420,
        (3, _, _) => 52,
        (4, "Auto Load", "Subtitle matching mode") => 56,
        (4, "Auto Load", "Priority keywords") => 132,
        (4, "Auto Load", _) => 193,
        (4, "ASS Subtitles", _) => 291,
        (4, "Text Subtitles", "Border") => 505,
        (4, "Text Subtitles", "Shadow and other styles") => 661,
        (4, "Text Subtitles", _) => 411,
        (4, "Position", "Vertical position") => 850,
        (4, "Position", _) => 772,
        (4, "Other", "Default encoding") => 1000,
        (4, "Other", _) => 950,
        (5, "Cache", _) | (5, "Network", _) => 80,
        _ => 0,
    }
}

fn normalized_query(query: &str) -> String {
    query
        .replace('　', " ")
        .trim()
        .trim_end_matches(';')
        .trim()
        .to_lowercase()
}

fn results_for_query(query: &str) -> Vec<PreferencesSearchResult> {
    let query = normalized_query(query);
    if query.is_empty() {
        return Vec::new();
    }

    let terms = query.split_whitespace().collect::<Vec<_>>();
    let mut matches = ENTRIES
        .iter()
        .filter(|entry| {
            // IINA indexes an entry's own title and explicit aliases; page and
            // section labels organize the results but are not query terms.
            let searchable = format!("{} {}", entry.title, entry.terms).to_lowercase();
            terms.iter().all(|term| searchable.contains(term))
        })
        .collect::<Vec<_>>();
    // The reference groups results by page, then orders them by section/title.
    matches.sort_by_key(|entry| {
        (
            entry.page_index,
            entry.section.to_lowercase(),
            entry.title.to_lowercase(),
        )
    });
    let mut seen = BTreeSet::new();
    matches.retain(|entry| seen.insert((entry.page_index, entry.section, entry.title)));

    let mut results = Vec::new();
    let mut active_page = None;
    for entry in matches {
        if active_page != Some(entry.page_index) {
            active_page = Some(entry.page_index);
            results.push(PreferencesSearchResult {
                page_index: entry.page_index,
                page_name: entry.page.into(),
                section_name: "".into(),
                title: "".into(),
                is_header: true,
                icon: page_icon(entry.page_index),
                anchor_y: -1,
            });
        }
        results.push(PreferencesSearchResult {
            page_index: entry.page_index,
            page_name: entry.page.into(),
            section_name: entry.section.into(),
            title: entry.title.into(),
            is_header: false,
            icon: Image::default(),
            anchor_y: anchor_y(entry),
        });
    }
    results
}

pub(crate) fn connect(window: &PreferencesWindow) {
    window.set_search_results(ModelRc::new(VecModel::from(Vec::new())));
    let weak = window.as_weak();
    window.on_search_query_changed(move |query| {
        if let Some(window) = weak.upgrade() {
            window.set_selected_search_index(-1);
            window.set_search_results(ModelRc::new(VecModel::from(results_for_query(&query))));
        }
    });

    let weak = window.as_weak();
    window.on_search_selection_moved(move |direction| {
        let Some(window) = weak.upgrade() else { return };
        let rows = result_rows(&window);
        let current = usize::try_from(window.get_selected_search_index()).ok();
        let next = next_selectable_index(&rows, current, direction);
        window.set_selected_search_index(next.map_or(-1, |index| index as i32));
    });

    let weak = window.as_weak();
    window.on_search_selection_activated(move || {
        let Some(window) = weak.upgrade() else { return };
        let selected = usize::try_from(window.get_selected_search_index()).ok();
        let Some(result) = selected.and_then(|index| window.get_search_results().row_data(index))
        else {
            return;
        };
        activate_result(
            &window,
            result.page_index,
            &result.section_name,
            &result.title,
            result.anchor_y,
        );
    });

    let weak = window.as_weak();
    window.on_search_result_activated(move |page, section, title, anchor| {
        if let Some(window) = weak.upgrade() {
            activate_result(&window, page, &section, &title, anchor);
        }
    });
}

fn result_rows(window: &PreferencesWindow) -> Vec<PreferencesSearchResult> {
    let results = window.get_search_results();
    (0..results.row_count())
        .filter_map(|index| results.row_data(index))
        .collect()
}

fn next_selectable_index(
    results: &[PreferencesSearchResult],
    current: Option<usize>,
    direction: i32,
) -> Option<usize> {
    let selectable = results
        .iter()
        .enumerate()
        .filter_map(|(index, result)| (!result.is_header).then_some(index))
        .collect::<Vec<_>>();
    if selectable.is_empty() || direction == 0 {
        return current.filter(|index| selectable.contains(index));
    }

    let current_position =
        current.and_then(|index| selectable.iter().position(|item| *item == index));
    let next_position = match (current_position, direction.signum()) {
        (None, 1) => 0,
        (None, -1) => selectable.len() - 1,
        (Some(position), 1) => (position + 1).min(selectable.len() - 1),
        (Some(position), -1) => position.saturating_sub(1),
        _ => return current,
    };
    Some(selectable[next_position])
}

fn activate_result(window: &PreferencesWindow, page: i32, section: &str, title: &str, anchor: i32) {
    if page == 0 {
        window.set_selected_general_section(match section {
            "History" => 1,
            "Playlist" => 2,
            "Screenshots" => 3,
            _ => 0,
        });
    }
    if page == 1 {
        window.set_selected_interface_section(match section {
            "Window" => 1,
            "On-Screen Controller" => 2,
            "On-Screen Display" => 3,
            "Thumbnail Preview" => 4,
            "Picture-in-Picture" => 5,
            "Accessibility" => 6,
            _ => 0,
        });
        let target = match section {
            "Window" => window.get_interface_window_target(),
            "On-Screen Controller" => window.get_interface_osc_target(),
            "On-Screen Display" => window.get_interface_osd_target(),
            "Thumbnail Preview" => window.get_interface_thumbnail_target(),
            "Picture-in-Picture" => window.get_interface_pip_target(),
            "Accessibility" => window.get_interface_accessibility_target(),
            _ => 0.0,
        };
        window.set_interface_scroll_position(target);
    }
    if page == 2 {
        if title == "Tone Mapping" {
            window.set_tone_mapping_expanded(true);
        }
        window.set_video_scroll_position((52 - anchor).max(-540) as f32);
    }
    if page == 4 {
        if section == "Auto Load" && title != "Subtitle matching mode" {
            window.set_subtitle_auto_load_advanced_expanded(true);
        }
        if title == "Shadow and other styles" {
            window.set_subtitle_text_advanced_expanded(true);
        }
        window.set_subtitle_scroll_position((52 - anchor).max(-570) as f32);
    }
    if page == 5 {
        window.set_selected_network_section(match section {
            "Network" => 1,
            "Online Videos (yt-dlp)" => 2,
            _ => 0,
        });
    }
    if page == 3 {
        window.set_selected_audio_section(match section {
            "Volume" => 1,
            "Other" => 2,
            _ => 0,
        });
    }
    window.set_selected_pane(page);
    window.set_search_query("".into());
    window.set_selected_search_index(-1);
    window.set_search_results(ModelRc::new(VecModel::from(Vec::new())));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item_results(query: &str) -> Vec<PreferencesSearchResult> {
        results_for_query(query)
            .into_iter()
            .filter(|result| !result.is_header)
            .collect()
    }

    #[test]
    fn search_matches_case_insensitive_substrings_in_multiple_words() {
        let results = item_results("BOOST　200;");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].page_name, "Audio");
        assert_eq!(results[0].title, "Allow volume boost (up to 200%)");
    }

    #[test]
    fn audio_preferences_search_into_their_matching_sections() {
        for (query, expected_section) in [
            ("threads driver", "Hardware"),
            ("ReplayGain preamp", "Volume"),
            ("gapless languages", "Other"),
        ] {
            let result = item_results(query);
            assert_eq!(result.len(), 1, "query: {query}");
            assert_eq!(result[0].page_name, "Audio");
            assert_eq!(result[0].section_name, expected_section);
        }
    }

    #[test]
    fn video_preferences_search_into_decoding_and_color_sections() {
        for (query, section, anchor) in [
            ("hardware decoder", "Decoding", 145),
            ("video threads", "Decoding", 80),
            ("OCR frame", "Live Text", 378),
            ("hdr bt2390", "Color and HDR", 420),
        ] {
            let result = item_results(query);
            assert_eq!(result.len(), 1, "query: {query}");
            assert_eq!(result[0].page_name, "Video");
            assert_eq!(result[0].section_name, section);
            assert_eq!(result[0].anchor_y, anchor);
        }
    }

    #[test]
    fn interface_search_results_use_the_captured_subsection_scroll_anchors() {
        for (query, section, anchor) in [
            ("system appearance", "Appearance", 0),
            ("liquid glass", "Appearance", 0),
            ("compact interface", "Appearance", 0),
            ("initial window size", "Window", 207),
            ("initial window position", "Window", 207),
            ("video size", "Window", 207),
            ("physical resolution", "Window", 207),
            ("stay above other windows", "Window", 207),
            ("title bar pin status", "Window", 207),
            ("osc position", "On-Screen Controller", 619),
            ("overlay display messages", "On-Screen Display", 735),
        ] {
            let result = item_results(query);
            assert_eq!(result.len(), 1, "query: {query}");
            assert_eq!(result[0].page_name, "Interface");
            assert_eq!(result[0].section_name, section);
            assert_eq!(result[0].anchor_y, anchor);
        }
    }

    #[test]
    fn search_groups_hits_in_reference_page_order() {
        let results = results_for_query("subtitle");
        let pages = results
            .iter()
            .filter(|result| result.is_header)
            .map(|result| result.page_name.to_string())
            .collect::<Vec<_>>();
        assert_eq!(pages, ["General", "Subtitles"]);
    }

    #[test]
    fn network_cache_controls_are_searchable_in_the_cache_section() {
        let results = item_results("buffer before playback");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].page_name, "Network");
        assert_eq!(results[0].section_name, "Cache");
        assert_eq!(results[0].title, "Buffer before starting playback");
        assert_eq!(results[0].anchor_y, 80);
    }

    #[test]
    fn connection_controls_are_searchable_in_the_network_section() {
        let result = item_results("RTSP transport");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].page_name, "Network");
        assert_eq!(result[0].section_name, "Network");
        assert_eq!(result[0].title, "RTSP transport");
        assert_eq!(result[0].anchor_y, 80);
    }

    #[test]
    fn online_video_options_are_searchable_in_their_reference_subsection() {
        let result = item_results("yt-dlp raw options");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].page_name, "Network");
        assert_eq!(result[0].section_name, "Online Videos (yt-dlp)");
        assert_eq!(result[0].title, "Raw options");
    }

    #[test]
    fn utility_actions_are_searchable_on_their_reference_page() {
        for (query, expected_title) in [
            (
                "watch later resume position",
                "Clear Saved Playback Progress",
            ),
            ("hidden notifications", "Restore Suppressed Alerts"),
            ("thumbnail cache", "Clear Thumbnail Cache"),
        ] {
            let result = item_results(query);
            assert_eq!(result.len(), 1, "query: {query}");
            assert_eq!(result[0].page_name, "Utilities");
            assert_eq!(result[0].title, expected_title);
        }
    }

    #[test]
    fn subtitle_results_keep_reference_section_order() {
        let subtitle_items = results_for_query("subtitle")
            .into_iter()
            .filter(|result| !result.is_header && result.page_index == 4)
            .map(|result| result.title.to_string())
            .collect::<Vec<_>>();
        assert_eq!(subtitle_items, ["Search paths", "Subtitle matching mode"]);
    }

    #[test]
    fn empty_and_unmatched_queries_have_no_rows() {
        assert!(results_for_query(" ; ").is_empty());
        assert!(results_for_query("unsupported-control-xyz").is_empty());
    }

    #[test]
    fn subtitle_search_results_retain_their_visible_scroll_anchor() {
        let border = item_results("border");
        assert_eq!(border.len(), 1);
        assert_eq!(border[0].anchor_y, 505);

        let auto_load = item_results("priority keywords");
        assert_eq!(auto_load.len(), 1);
        assert_eq!(auto_load[0].anchor_y, 132);
        assert_eq!(item_results("search paths")[0].anchor_y, 193);
        assert_eq!(item_results("vertical position")[0].anchor_y, 850);
    }

    #[test]
    fn keyboard_search_navigation_skips_page_headers_and_stays_in_bounds() {
        let results = results_for_query("subtitle");
        assert_eq!(next_selectable_index(&results, None, 1), Some(1));
        assert_eq!(next_selectable_index(&results, Some(1), 1), Some(3));
        assert_eq!(next_selectable_index(&results, Some(3), -1), Some(1));
        assert_eq!(next_selectable_index(&results, Some(3), 0), Some(3));
        let last = results
            .iter()
            .rposition(|result| !result.is_header)
            .unwrap();
        assert_eq!(next_selectable_index(&results, Some(last), 1), Some(last));
        assert_eq!(next_selectable_index(&results, Some(1), -1), Some(1));
    }
}
