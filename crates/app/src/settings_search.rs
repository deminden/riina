//! Search only controls currently represented in the Settings UI.
use crate::{GeneralSettingsState, PreferencesSearchResult, PreferencesWindow, VideoSettingsState};
use slint::{ComponentHandle, Image, Model, ModelRc, VecModel};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy)]
struct Entry {
    page_index: i32,
    page: &'static str,
    section: &'static str,
    title: &'static str,
    terms: &'static str,
}

const fn general(section: &'static str, title: &'static str, terms: &'static str) -> Entry {
    Entry {
        page_index: 0,
        page: "General",
        section,
        title,
        terms,
    }
}

const fn control(section: &'static str, title: &'static str, terms: &'static str) -> Entry {
    Entry {
        page_index: 6,
        page: "Control",
        section,
        title,
        terms,
    }
}

// The index follows visible user-facing labels. Aliases make common queries
// useful without advertising controls that the current page does not expose.
const ENTRIES: &[Entry] = &[
    general("", "General", "preferences"),
    general(
        "Behavior",
        "At launch",
        "startup welcome window open file panel nothing",
    ),
    general("Behavior", "When media is opened", "pause fullscreen open"),
    general("Behavior", "Pause", "pause when opened"),
    general("Behavior", "Enter fullscreen", "full screen"),
    general(
        "Behavior",
        "Automatically switch to Music Mode",
        "audio music mode auto open",
    ),
    general(
        "Behavior",
        "Show playlist in Music Mode",
        "audio music mode queue",
    ),
    general(
        "Behavior",
        "Show album artwork in Music Mode",
        "audio music mode cover art",
    ),
    general("Behavior", "Pause/resume when", "playback pause policy"),
    general(
        "Behavior",
        "Play when entering fullscreen",
        "resume full screen",
    ),
    general(
        "Behavior",
        "Pause when leaving fullscreen",
        "exit fullscreen",
    ),
    general(
        "Behavior",
        "Window becomes inactive/active",
        "pause window focus",
    ),
    general(
        "Behavior",
        "Minimized/un-minimized",
        "pause minimize restore",
    ),
    general(
        "Behavior",
        "Pause when machine goes to sleep",
        "sleep suspend power",
    ),
    general(
        "Behavior",
        "Always open media in new window",
        "multiple player windows",
    ),
    general(
        "Behavior",
        "Group media in playlist when opening multiple files at once",
        "multiple open group",
    ),
    general(
        "Behavior",
        "Allow opening the same file in multiple windows",
        "duplicate players",
    ),
    general(
        "Behavior",
        "Quit after all windows are closed",
        "exit quit on last window",
    ),
    general(
        "Behavior",
        "Keep window open after playback finishes",
        "keep open at end of file",
    ),
    general(
        "Behavior",
        "Prevent screen saver",
        "prevent display sleep idle power during playback",
    ),
    general(
        "Behavior",
        "Allow screen saver for audio playback",
        "display sleep audio music mode idle power",
    ),
    general(
        "Behavior",
        "Check for updates automatically",
        "software update background interval hour day week month",
    ),
    general(
        "Behavior",
        "Update check interval",
        "check every software update schedule",
    ),
    general(
        "Behavior",
        "Receive beta updates",
        "software update prerelease preview release channel",
    ),
    general("History", "Resume last position", "resume playback history"),
    general(
        "History",
        "Record playback history",
        "save playback history",
    ),
    general(
        "History",
        "Enable \"Open Recent\" menu",
        "recent files documents",
    ),
    general(
        "History",
        "Track all played files in \"Open Recent\" menu",
        "all recent files documents",
    ),
    general(
        "Playlist",
        "Add files in the same folder automatically",
        "playlist autoload folder",
    ),
    general(
        "Playlist",
        "Play next item automatically",
        "playlist advance",
    ),
    general(
        "Playlist",
        "Show artist and track name for audio files when available",
        "playlist artist title metadata audio tags music",
    ),
    general(
        "Playlist",
        "Only in Music Mode",
        "playlist audio metadata artist track title music mode only",
    ),
    general(
        "Playlist",
        "Loop when a file is opened manually",
        "repeat file playlist",
    ),
    general(
        "Screenshots",
        "Save to",
        "screenshot folder destination path",
    ),
    general("Screenshots", "Format", "screenshot png jpeg webp jxl"),
    general("Screenshots", "Copy to clipboard", "screenshot copy"),
    general(
        "Screenshots",
        "Include subtitles",
        "subtitle captions screenshot",
    ),
    general(
        "Screenshots",
        "Show previews after taking screenshots",
        "preview screenshot",
    ),
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
        title: "Platform Materials",
        terms: "liquid glass mica acrylic glass material translucency overlay controller sidebar",
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
        section: "On-Screen Controller",
        title: "Show chapter position in progress bar",
        terms: "chapter timeline marker position progress bar",
    },
    Entry {
        page_index: 1,
        page: "Interface",
        section: "On-Screen Controller",
        title: "Snap to center when dragging",
        terms: "center align floating osc control bar",
    },
    Entry {
        page_index: 1,
        page: "Interface",
        section: "On-Screen Controller",
        title: "Show remaining time instead of total duration",
        terms: "time label countdown controller timeline",
    },
    Entry {
        page_index: 1,
        page: "Interface",
        section: "On-Screen Controller",
        title: "Base remaining time on playback speed",
        terms: "scale countdown controller timeline",
    },
    Entry {
        page_index: 1,
        page: "Interface",
        section: "On-Screen Controller",
        title: "Use left/right button for",
        terms: "osc transport arrow playback speed playlist navigation seek scrub 10 seconds",
    },
    Entry {
        page_index: 1,
        page: "Interface",
        section: "On-Screen Controller",
        title: "Prevent scroll wheel from changing playback position",
        terms: "disable seek slider scrolling mouse",
    },
    Entry {
        page_index: 1,
        page: "Interface",
        section: "On-Screen Controller",
        title: "Prevent scroll wheel from changing volume",
        terms: "disable volume slider scrolling mouse",
    },
    Entry {
        page_index: 1,
        page: "Interface",
        section: "On-Screen Controller",
        title: "Toolbar buttons",
        terms: "arrange reorder hide customization",
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
        section: "On-Screen Controller",
        title: "Auto hide after",
        terms: "controls visibility fade inactivity duration",
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
        page_index: 1,
        page: "Interface",
        section: "Picture-in-Picture",
        title: "Window after entering",
        terms: "pip do nothing hide minimize main player window",
    },
    Entry {
        page_index: 1,
        page: "Interface",
        section: "Picture-in-Picture",
        title: "Automatically pause playback",
        terms: "pip pause playback enter",
    },
    Entry {
        page_index: 1,
        page: "Interface",
        section: "Picture-in-Picture",
        title: "Toggle by minimizing the window",
        terms: "pip minimize unminimize toggle window",
    },
    Entry {
        page_index: 1,
        page: "Interface",
        section: "Picture-in-Picture",
        title: "Only for video files",
        terms: "pip minimize audio only video",
    },
    Entry {
        page_index: 1,
        page: "Interface",
        section: "Thumbnail Preview",
        title: "Enable thumbnail preview",
        terms: "timeline scrub hover",
    },
    Entry {
        page_index: 1,
        page: "Interface",
        section: "Thumbnail Preview",
        title: "Maximum cache size",
        terms: "thumbnail previews disk limit MB",
    },
    Entry {
        page_index: 1,
        page: "Interface",
        section: "Thumbnail Preview",
        title: "Generate previews for files on remote volumes",
        terms: "thumbnail network remote drives",
    },
    Entry {
        page_index: 1,
        page: "Interface",
        section: "Thumbnail Preview",
        title: "Thumbnail width",
        terms: "thumbnail preview dimensions size",
    },
    Entry {
        page_index: 1,
        page: "Interface",
        section: "Accessibility",
        title: "Disable animations",
        terms: "accessibility reduce motion animation vestibular",
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
        title: "Load ICC profile",
        terms: "color management display profile SDR",
    },
    Entry {
        page_index: 2,
        page: "Video",
        section: "Color and HDR",
        title: "Enable HDR mode",
        terms: "enable high dynamic range by default",
    },
    Entry {
        page_index: 2,
        page: "Video",
        section: "Color and HDR",
        title: "Tone Mapping",
        terms: "hdr map high dynamic range brightness",
    },
    Entry {
        page_index: 2,
        page: "Video",
        section: "Color and HDR",
        title: "Override target peak",
        terms: "hdr tone mapping luminance brightness nits",
    },
    Entry {
        page_index: 2,
        page: "Video",
        section: "Color and HDR",
        title: "Tone mapping algorithm",
        terms: "hdr tone mapping algorithm clip mobius reinhard hable bt2390 gamma linear",
    },
    Entry {
        page_index: 2,
        page: "Video",
        section: "Color and HDR",
        title: "Override tone-mapping parameter",
        terms: "hdr tone mapping parameter",
    },
    Entry {
        page_index: 3,
        page: "Audio",
        section: "Hardware",
        title: "Audio decoding threads",
        terms: "audio decoder parallel decode automatic",
    },
    Entry {
        page_index: 3,
        page: "Audio",
        section: "Hardware",
        title: "Audio driver",
        terms: "audio threads and driver hardware backend wasapi coreaudio pipewire",
    },
    Entry {
        page_index: 3,
        page: "Audio",
        section: "Hardware",
        title: "Preferred audio device",
        terms: "audio output device speakers headphones",
    },
    Entry {
        page_index: 3,
        page: "Audio",
        section: "Hardware",
        title: "S/PDIF output",
        terms: "spdif digital passthrough ac3 eac3 dts truehd",
    },
    Entry {
        page_index: 3,
        page: "Audio",
        section: "Volume",
        title: "Initial volume",
        terms: "initial audio volume start",
    },
    Entry {
        page_index: 3,
        page: "Audio",
        section: "Volume",
        title: "ReplayGain",
        terms: "initial volume and replay gain track album off normalization",
    },
    Entry {
        page_index: 3,
        page: "Audio",
        section: "Volume",
        title: "ReplayGain preamp (dB)",
        terms: "replay gain preamplifier",
    },
    Entry {
        page_index: 3,
        page: "Audio",
        section: "Volume",
        title: "Prevent clipping",
        terms: "replay gain distortion",
    },
    Entry {
        page_index: 3,
        page: "Audio",
        section: "Volume",
        title: "ReplayGain fallback (dB)",
        terms: "replay gain amplification without tags",
    },
    Entry {
        page_index: 3,
        page: "Audio",
        section: "Volume",
        title: "Allow volume boost (up to 200%)",
        terms: "amplify gain volume cap",
    },
    Entry {
        page_index: 3,
        page: "Audio",
        section: "Other",
        title: "Gapless playback",
        terms: "audio seamless disabled weak strong",
    },
    Entry {
        page_index: 3,
        page: "Audio",
        section: "Other",
        title: "Preferred audio languages",
        terms: "gapless playback and languages audio language alang",
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
        page_index: 4,
        page: "Subtitles",
        section: "Online Subtitles",
        title: "Subtitle provider",
        terms: "Assrt.net OpenSubtitles Shooter.cn source service local file hash",
    },
    Entry {
        page_index: 4,
        page: "Subtitles",
        section: "Online Subtitles",
        title: "Assrt.net token",
        terms: "account API key credential",
    },
    Entry {
        page_index: 4,
        page: "Subtitles",
        section: "Online Subtitles",
        title: "OpenSubtitles API key",
        terms: "online subtitle search download provider account key",
    },
    Entry {
        page_index: 4,
        page: "Subtitles",
        section: "Online Subtitles",
        title: "Automatic subtitle search",
        terms: "online automatically duration threshold minutes",
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
    control("", "Control", "preferences"),
    control("Trackpad", "Pinch to", "gesture trackpad zoom resize"),
    control("Trackpad", "Force Touch to", "trackpad hard press pressure"),
    control("Mouse", "Single click to", "mouse button one"),
    control("Mouse", "Double click to", "mouse button two"),
    control("Mouse", "Right click to", "mouse secondary button context"),
    control("Mouse", "Middle click to", "mouse wheel button"),
    control(
        "Mouse",
        "Accepts first mouse click when not focused",
        "inactive background focus click-through",
    ),
    control("Scroll", "Scroll vertically to", "mouse wheel up down"),
    control("Scroll", "Scroll horizontally to", "mouse wheel left right"),
    control("Scrolling sensitivity", "Seek type", "keyframe exact auto"),
    control("Scrolling sensitivity", "Relative seek amount", "skip step"),
    control(
        "Scrolling sensitivity",
        "Sensitivity for speed",
        "scroll playback speed",
    ),
    control(
        "Scrolling sensitivity",
        "Sensitivity for volume",
        "scroll audio volume",
    ),
    Entry {
        page_index: 7,
        page: "Key Bindings",
        section: "",
        title: "Key Bindings",
        terms: "keyboard shortcuts hotkeys",
    },
    Entry {
        page_index: 7,
        page: "Key Bindings",
        section: "",
        title: "Use system media control",
        terms: "multimedia keys now playing remote",
    },
    Entry {
        page_index: 7,
        page: "Key Bindings",
        section: "",
        title: "Key binding set",
        terms: "profile keyboard preset configuration",
    },
    Entry {
        page_index: 7,
        page: "Key Bindings",
        section: "",
        title: "Display raw values",
        terms: "show literal mpv input command",
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
        section: "Logging",
        title: "Open the log viewer",
        terms: "view diagnostics player logs",
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
        page_index: 9,
        page: "Plugins",
        section: "",
        title: "Get Plugins…",
        terms: "add install github repository",
    },
    Entry {
        page_index: 9,
        page: "Plugins",
        section: "",
        title: "Install Local Package…",
        terms: "import iinaplgz archive file",
    },
    Entry {
        page_index: 9,
        page: "Plugins",
        section: "",
        title: "Check for plugin updates",
        terms: "refresh installed addons versions",
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
        0 => include_bytes!("../../../assets/icons/settings.svg"),
        1 => include_bytes!("../../../assets/icons/interface.svg"),
        2 => include_bytes!("../../../assets/icons/video-page.svg"),
        3 => include_bytes!("../../../assets/icons/audio-page.svg"),
        4 => include_bytes!("../../../assets/icons/subtitles-page.svg"),
        5 => include_bytes!("../../../assets/icons/network.svg"),
        6 => include_bytes!("../../../assets/icons/control.svg"),
        7 => include_bytes!("../../../assets/icons/keybindings.svg"),
        8 => include_bytes!("../../../assets/icons/advanced.svg"),
        9 => include_bytes!("../../../assets/icons/plugins.svg"),
        _ => include_bytes!("../../../assets/icons/utilities.svg"),
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
        (0, "Playlist", _) => 720,
        (1, "Appearance", _) => 0,
        (1, "Window", _) => 207,
        (1, "On-Screen Controller", _) => 619,
        (1, "On-Screen Display", _) => 769,
        (1, "Thumbnail Preview", _) => 877,
        (1, "Picture-in-Picture", _) => 973,
        (1, "Accessibility", _) => 1069,
        (2, "Decoding", "Number of threads") => 80,
        (2, "Decoding", _) => 145,
        (2, "Live Text", _) => 378,
        (2, "Color and HDR", _) => 420,
        (3, _, _) => 52,
        (4, "Auto Load", _) => 52,
        (4, "ASS Subtitles", _) => 180,
        (4, "Text Subtitles", _) => 580,
        (4, "Position", _) => 870,
        (4, "Online Subtitles", _) => 1164,
        (4, "Other", _) => 1454,
        (5, "Cache", _) | (5, "Network", _) => 80,
        (6, "Mouse", _) => 174,
        (6, "Scroll", _) => 418,
        (6, "Scrolling sensitivity", _) => 538,
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

type SearchLabels = BTreeMap<String, slint::SharedString>;

fn translated<'a>(labels: &'a SearchLabels, text: &'a str) -> &'a str {
    labels.get(text).map_or(text, slint::SharedString::as_str)
}

struct IndexedEntry {
    source: &'static Entry,
    row: PreferencesSearchResult,
    searchable: String,
}

fn build_index(labels: &SearchLabels) -> Vec<IndexedEntry> {
    let mut index = ENTRIES
        .iter()
        .enumerate()
        .map(|(target, source)| {
            let title = translated(labels, source.title);
            let visible_label = match source.title {
                "Platform Materials" => "Translucent Surfaces",
                "Preferred audio languages" => "Preferred languages",
                "Tone mapping algorithm" => "Algorithm",
                "Suppress file-start messages" => "File start",
                "Suppress pause and resume messages" => "Pause / Resume",
                "Suppress seek messages" => "Seek",
                "Suppress speed messages" => "Speed",
                "Subtitle matching mode" => "Auto load",
                "Priority keywords" => "Filename priority keywords",
                "Search paths" => "Also search these directories",
                "Shadow and other styles" => "Other styles",
                "Subtitle provider" => "Provider",
                "Automatic subtitle search" => "Search online subtitles automatically",
                "Preferred language" => "Preferred languages",
                _ => source.title,
            };
            IndexedEntry {
                source,
                row: PreferencesSearchResult {
                    target: target as i32,
                    page_index: source.page_index,
                    page_name: translated(labels, source.page).into(),
                    section_name: translated(labels, source.section).into(),
                    title: title.into(),
                    anchor_y: anchor_y(source),
                    ..Default::default()
                },
                // Page and section labels organize results. Titles and aliases match queries.
                searchable: format!(
                    "{} {title} {} {}",
                    source.title,
                    source.terms,
                    translated(labels, visible_label)
                )
                .to_lowercase(),
            }
        })
        .collect::<Vec<_>>();
    index.sort_by_cached_key(|item| {
        (
            item.row.page_index,
            item.row.section_name.to_lowercase(),
            item.row.title.to_lowercase(),
        )
    });
    index
}

fn localized_results_for_query(
    query: &str,
    index: &[IndexedEntry],
    platform_macos: bool,
    live_text_available: bool,
) -> Vec<PreferencesSearchResult> {
    let query = normalized_query(query);
    if query.is_empty() {
        return Vec::new();
    }

    let terms = query.split_whitespace().collect::<Vec<_>>();
    let matches = index.iter().filter(|item| {
        let available = match item.source.title {
            "Window becomes inactive/active"
            | "Pause when machine goes to sleep"
            | "Load ICC profile" => platform_macos,
            "Live Text" => live_text_available,
            _ => true,
        };
        available && terms.iter().all(|term| item.searchable.contains(term))
    });
    let mut seen = BTreeSet::new();

    let mut results = Vec::new();
    let mut active_page = None;
    for item in matches {
        let source = item.source;
        if !seen.insert((source.page_index, source.section, source.title)) {
            continue;
        }
        if active_page != Some(source.page_index) {
            active_page = Some(source.page_index);
            results.push(PreferencesSearchResult {
                target: -1,
                page_index: source.page_index,
                page_name: item.row.page_name.clone(),
                is_header: true,
                icon: page_icon(source.page_index),
                anchor_y: -1,
                ..Default::default()
            });
        }
        results.push(item.row.clone());
    }
    results
}

pub(crate) fn connect(window: &PreferencesWindow) {
    window.set_search_results(ModelRc::new(VecModel::from(Vec::new())));
    let labels = window
        .global::<riina_ui::SettingsSearchText>()
        .get_captions()
        .iter()
        .map(|caption| (caption.source.to_string(), caption.localized))
        .collect::<SearchLabels>();
    let index = build_index(&labels);
    let weak = window.as_weak();
    window.on_search_query_changed(move |query| {
        if let Some(window) = weak.upgrade() {
            window.set_selected_search_index(-1);
            window.set_search_results(ModelRc::new(VecModel::from(localized_results_for_query(
                &query,
                &index,
                window.get_platform_macos(),
                window.get_live_text_available(),
            ))));
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
        activate_target(&window, result.target);
    });

    let weak = window.as_weak();
    window.on_search_result_activated(move |target| {
        if let Some(window) = weak.upgrade() {
            activate_target(&window, target);
        }
    });
}

fn activate_target(window: &PreferencesWindow, target: i32) {
    let Some(entry) = usize::try_from(target)
        .ok()
        .and_then(|index| ENTRIES.get(index))
    else {
        return;
    };
    activate_result(window, entry.page_index, entry.section, entry.title);
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

fn general_target(window: &PreferencesWindow, section: &str, title: &str) -> f32 {
    match section {
        "Behavior" => match title {
            "Pause"
            | "Enter fullscreen"
            | "Automatically switch to Music Mode"
            | "Show playlist in Music Mode"
            | "Show album artwork in Music Mode" => {
                window
                    .global::<GeneralSettingsState>()
                    .set_when_media_opened_expanded(true);
                window.get_general_media_opened_target()
            }
            "Play when entering fullscreen"
            | "Pause when leaving fullscreen"
            | "Window becomes inactive/active"
            | "Minimized/un-minimized"
            | "Pause when machine goes to sleep" => {
                window
                    .global::<GeneralSettingsState>()
                    .set_pause_resume_when_expanded(true);
                window.get_general_pause_policy_target()
            }
            "Pause/resume when" => window.get_general_pause_policy_target(),
            "Always open media in new window"
            | "Group media in playlist when opening multiple files at once"
            | "Allow opening the same file in multiple windows"
            | "Quit after all windows are closed"
            | "Keep window open after playback finishes" => window.get_general_window_target(),
            "Prevent screen saver" | "Allow screen saver for audio playback" => {
                window.get_general_power_target()
            }
            "Check for updates automatically"
            | "Update check interval"
            | "Receive beta updates" => window.get_general_updates_target(),
            _ => 0.0,
        },
        "History" => window.get_general_history_target(),
        "Playlist" => window.get_general_playlist_target(),
        "Screenshots" => window.get_general_screenshots_target(),
        _ => 0.0,
    }
}

fn reveal_network_result(window: &PreferencesWindow, section: &str, title: &str) {
    let section = match section {
        "Network" => 1,
        "Online Videos (yt-dlp)" => 2,
        _ => 0,
    };
    let inset = match title {
        "Seconds to buffer before restarting" => 168.0,
        "Seconds to prefetch" | "Cache buffer size (KiB)" => 240.0,
        "Show throbber when buffering" | "Show throbber when seeking" => 330.0,
        _ => 0.0,
    };
    window.invoke_reveal_network_setting(section, inset);
}

fn reveal_additional_settings(window: &PreferencesWindow, page: i32, section: &str, title: &str) {
    match page {
        7 => window.invoke_reveal_key_binding_settings(title == "Display raw values"),
        8 => window.invoke_reveal_advanced_setting(match (section, title) {
            ("Logging", _) => 1,
            ("mpv Settings", "Additional mpv options") => 3,
            ("mpv Settings", _) => 2,
            _ => 0,
        }),
        9 => window.set_plugin_scroll_position(0.0),
        10 => window.invoke_reveal_utility_setting(match section {
            "Restore Alerts" => 1,
            "Clear Cache" => 2,
            "Browser Extensions" => 3,
            _ => 0,
        }),
        _ => {}
    }
}

fn activate_result(window: &PreferencesWindow, page: i32, section: &str, title: &str) {
    if page == 0 {
        let target = general_target(window, section, title);
        window.set_general_scroll_position(target);
    }
    if page == 1 {
        match title {
            "Platform Materials" => window.set_liquid_glass_expanded(true),
            "Suppress file-start messages"
            | "Suppress pause and resume messages"
            | "Suppress seek messages"
            | "Suppress speed messages" => window.set_osd_suppression_expanded(true),
            _ => {}
        }
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
        let video = window.global::<VideoSettingsState>();
        let target = match title {
            "Number of threads" => 52.0,
            "Hardware decoder" => 125.0,
            "Live Text" => 378.0,
            "Load ICC profile" => window.get_video_color_card_y(),
            "Enable HDR mode" => window.get_video_hdr_card_y(),
            _ => window.get_video_tone_mapping_card_y(),
        };
        if matches!(
            title,
            "Tone Mapping"
                | "Override target peak"
                | "Tone mapping algorithm"
                | "Override tone-mapping parameter"
        ) {
            video.set_tone_mapping_expanded(true);
        }
        window.invoke_reveal_video_setting(target);
    }
    if page == 4 {
        if section == "Auto Load" && title != "Subtitle matching mode" {
            window
                .global::<crate::SubtitleSettingsState>()
                .set_subtitle_auto_load_advanced_expanded(true);
        }
        if title == "Shadow and other styles" {
            window
                .global::<crate::SubtitleSettingsState>()
                .set_subtitle_text_advanced_expanded(true);
        }
        window.invoke_reveal_subtitle_section(match section {
            "ASS Subtitles" => 1,
            "Text Subtitles" => 2,
            "Position" => 3,
            "Online Subtitles" => 4,
            "Other" => 5,
            _ => 0,
        });
    }
    if page == 5 {
        reveal_network_result(window, section, title);
    }
    if page == 6 {
        window.invoke_reveal_control_section(match section {
            "Mouse" => 1,
            "Scroll" => 2,
            "Scrolling sensitivity" => 3,
            _ => 0,
        });
    }
    if page == 3 {
        window.invoke_reveal_audio_section(match section {
            "Volume" => 1,
            "Other" => 2,
            _ => 0,
        });
    }
    reveal_additional_settings(window, page, section, title);
    window.set_selected_pane(page);
    window.set_search_query("".into());
    window.set_selected_search_index(-1);
    window.set_search_results(ModelRc::new(VecModel::from(Vec::new())));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn results_for_query(query: &str) -> Vec<PreferencesSearchResult> {
        localized_results_for_query(query, &build_index(&SearchLabels::new()), true, true)
    }

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
        for (query, expected_section, title) in [
            ("threads driver", "Hardware", "Audio driver"),
            ("ReplayGain preamp", "Volume", "ReplayGain preamp (dB)"),
            ("gapless languages", "Other", "Preferred audio languages"),
        ] {
            let result = item_results(query);
            assert_eq!(result.len(), 1, "query: {query}");
            assert_eq!(result[0].page_name, "Audio");
            assert_eq!(result[0].section_name, expected_section);
            assert_eq!(result[0].title, title);
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
    fn launch_action_is_searchable_in_general_behavior() {
        let results = item_results("open panel at launch");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].page_name, "General");
        assert_eq!(results[0].section_name, "Behavior");
        assert_eq!(results[0].title, "At launch");
    }

    #[test]
    fn localized_general_results_reveal_their_controls() -> Result<(), Box<dyn std::error::Error>> {
        i_slint_backend_testing::init_no_event_loop();
        let window = PreferencesWindow::new()?;
        window
            .window()
            .set_size(slint::LogicalSize::new(600.0, 480.0));
        window.show()?;
        let general = window.global::<GeneralSettingsState>();
        for (locale, query, page, section, title, artwork) in [
            (
                "ru",
                "историю воспроизведения",
                "Основные",
                "История",
                "Сохранять историю воспроизведения",
                "Показывать обложку в режиме музыки",
            ),
            (
                "ar",
                "حفظ سجل التشغيل",
                "عام",
                "السجل",
                "حفظ سجل التشغيل",
                "إظهار غلاف الألبوم في وضع الموسيقى",
            ),
        ] {
            slint::select_bundled_translation(locale)?;
            connect(&window);
            window.set_selected_pane(2);
            window.invoke_search_query_changed(query.into());
            let rows = window.get_search_results();
            assert_eq!(
                rows.row_data(0).ok_or("Missing search group")?.page_name,
                page
            );
            let (index, row) = rows
                .iter()
                .enumerate()
                .find(|(_, row)| row.title == title)
                .ok_or("Missing localized history setting")?;
            assert_eq!(row.section_name, section);
            window.set_selected_search_index(i32::try_from(index)?);
            window.invoke_search_selection_activated();
            slint::platform::update_timers_and_animations();
            assert_eq!(window.get_selected_pane(), 0);
            assert!(
                (window.get_general_scroll_position() - window.get_general_history_target()).abs()
                    < 0.5
            );

            general.set_when_media_opened_expanded(false);
            window.invoke_search_query_changed("album artwork".into());
            let row = window
                .get_search_results()
                .iter()
                .find(|row| row.title == artwork)
                .ok_or("Missing localized artwork setting for English alias")?;
            window.invoke_search_result_activated(row.target);
            assert!(general.get_when_media_opened_expanded());
            assert!(
                (window.get_general_scroll_position() - window.get_general_media_opened_target())
                    .abs()
                    < 0.5
            );
        }
        slint::select_bundled_translation("en")?;
        window.hide()?;
        Ok(())
    }

    #[test]
    fn localized_media_results_open_live_sections() -> Result<(), Box<dyn std::error::Error>> {
        i_slint_backend_testing::init_no_event_loop();
        let window = PreferencesWindow::new()?;
        window
            .window()
            .set_size(slint::LogicalSize::new(600.0, 480.0));
        window.show()?;
        let video = window.global::<VideoSettingsState>();

        for (locale, suppression, peak, languages) in [
            (
                "ru",
                "сообщение о перемотке",
                "пиковую яркость",
                "предпочтительные языки",
            ),
            ("ar", "رسائل التنقل", "ذروة السطوع", "اللغات المفضّلة"),
        ] {
            slint::select_bundled_translation(locale)?;
            window.set_platform_macos(locale == "ru");
            window.set_live_text_available(false);
            connect(&window);
            window.invoke_search_query_changed("selectable frame text".into());
            assert_eq!(window.get_search_results().row_count(), 0);
            window.set_live_text_available(true);
            window.invoke_search_query_changed("selectable frame text".into());
            assert_eq!(window.get_search_results().row_count(), 2);
            window.set_live_text_available(locale == "ru");
            window.set_osd_suppression_expanded(false);
            video.set_tone_mapping_expanded(false);

            for (query, page) in [(suppression, 1), (peak, 2), (languages, 3)] {
                window.invoke_search_query_changed(query.into());
                let row = window
                    .get_search_results()
                    .iter()
                    .find(|row| !row.is_header && row.page_index == page)
                    .ok_or("Missing localized Settings result")?;
                window.invoke_search_result_activated(row.target);
                slint::platform::update_timers_and_animations();
                assert_eq!(window.get_selected_pane(), page);
                match page {
                    1 => {
                        assert!(window.get_osd_suppression_expanded());
                        assert_eq!(window.get_selected_interface_section(), 3);
                    }
                    2 => {
                        assert!(video.get_tone_mapping_expanded());
                        let card_top = video.get_video_scroll_position()
                            + window.get_video_tone_mapping_card_y();
                        assert!((62.0..=66.0).contains(&card_top));
                    }
                    _ => assert_eq!(window.get_selected_audio_section(), 2),
                }
            }
        }
        slint::select_bundled_translation("en")?;
        window.hide()?;
        Ok(())
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
            ("auto hide controls", "On-Screen Controller", 619),
            (
                "chapter position in progress bar",
                "On-Screen Controller",
                619,
            ),
            ("overlay display messages", "On-Screen Display", 769),
            ("pip minimize main window", "Picture-in-Picture", 973),
            (
                "automatically pause playback pip",
                "Picture-in-Picture",
                973,
            ),
            ("toggle by minimizing window", "Picture-in-Picture", 973),
            ("only for video files pip", "Picture-in-Picture", 973),
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
    fn localized_subtitle_network_and_control_results_reveal_settings()
    -> Result<(), Box<dyn std::error::Error>> {
        i_slint_backend_testing::init_no_event_loop();
        let window = PreferencesWindow::new()?;
        window
            .window()
            .set_size(slint::LogicalSize::new(600.0, 480.0));
        window.show()?;
        let subtitles = window.global::<crate::SubtitleSettingsState>();
        for (locale, paths, seeking, mouse) in [
            (
                "ru",
                "искать также в этих папках",
                "показывать значок при перемотке",
                "правая кнопка мыши",
            ),
            (
                "ar",
                "البحث أيضًا في هذه المجلدات",
                "إظهار المؤشر عند التنقل",
                "النقر بالزر الأيمن",
            ),
        ] {
            slint::select_bundled_translation(locale)?;
            connect(&window);
            subtitles.set_subtitle_auto_load_advanced_expanded(false);

            for (query, page) in [(paths, 4), (seeking, 5), (mouse, 6)] {
                window.invoke_search_query_changed(query.into());
                let row = window
                    .get_search_results()
                    .iter()
                    .find(|row| !row.is_header && row.page_index == page)
                    .ok_or("Missing localized Settings result")?;
                window.invoke_search_result_activated(row.target);
                slint::platform::update_timers_and_animations();
                assert_eq!(window.get_selected_pane(), page);
                match page {
                    4 => {
                        assert!(subtitles.get_subtitle_auto_load_advanced_expanded());
                        assert_eq!(window.get_selected_subtitle_section(), 0);
                    }
                    5 => {
                        assert_eq!(window.get_selected_network_section(), 0);
                        assert!((window.get_network_scroll_position() + 330.0).abs() < 0.5);
                    }
                    _ => assert!((window.get_control_scroll_position() + 158.0).abs() < 0.5),
                }
            }
            window.invoke_search_query_changed("scroll audio volume".into());
            let row = window
                .get_search_results()
                .iter()
                .find(|row| !row.is_header && row.page_index == 6)
                .ok_or("Missing Control result for English alias")?;
            window.invoke_search_result_activated(row.target);
            assert!((window.get_control_scroll_position() + 280.0).abs() < 0.5);
        }
        slint::select_bundled_translation("en")?;
        window.hide()?;
        Ok(())
    }

    #[test]
    fn localized_key_binding_advanced_plugin_and_utility_search_reveals_controls()
    -> Result<(), Box<dyn std::error::Error>> {
        i_slint_backend_testing::init_no_event_loop();
        let window = PreferencesWindow::new()?;
        window
            .window()
            .set_size(slint::LogicalSize::new(600.0, 480.0));
        window
            .global::<riina_ui::KeyBindingSettingsState>()
            .set_key_binding_rows(ModelRc::new(VecModel::from(
                vec![riina_ui::KeyBindingRow::default(); 20],
            )));
        window.show()?;
        for (locale, raw_values, options, install, thumbnails) in [
            (
                "ru",
                "показывать исходные значения",
                "дополнительные параметры mpv",
                "локальный пакет",
                "очистить кэш эскизов",
            ),
            (
                "ar",
                "عرض القيم الأصلية",
                "خيارات mpv الإضافية",
                "تثبيت حزمة محلية",
                "مسح ذاكرة الصور المصغّرة",
            ),
        ] {
            slint::select_bundled_translation(locale)?;
            connect(&window);
            let targets = [
                (raw_values, 7, -450.0),
                (options, 8, -280.0),
                (install, 9, 0.0),
                (thumbnails, 10, -220.0),
            ];
            for (query, page, expected) in targets {
                window.invoke_search_query_changed(query.into());
                let row = window
                    .get_search_results()
                    .iter()
                    .find(|row| !row.is_header && row.page_index == page)
                    .ok_or("Missing localized Settings result")?;
                window.invoke_search_result_activated(row.target);
                slint::platform::update_timers_and_animations();
                assert_eq!(window.get_selected_pane(), page);
                let scroll = match page {
                    7 => window.get_key_binding_scroll_position(),
                    8 => window.get_advanced_scroll_position(),
                    9 => window.get_plugin_scroll_position(),
                    _ => window.get_utilities_scroll_position(),
                };
                assert!((scroll - expected).abs() < 0.5);
            }
        }
        slint::select_bundled_translation("en")?;
        window.hide()?;
        Ok(())
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
            ("clear thumbnail cache", "Clear Thumbnail Cache"),
        ] {
            let result = item_results(query);
            assert_eq!(result.len(), 1, "query: {query}");
            assert_eq!(result[0].page_name, "Utilities");
            assert_eq!(result[0].title, expected_title);
        }
        let results = item_results("thumbnail cache");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "Maximum cache size");
        assert_eq!(results[1].title, "Clear Thumbnail Cache");
    }

    #[test]
    fn subtitle_results_include_online_controls_in_section_order() {
        let subtitle_items = results_for_query("subtitle")
            .into_iter()
            .filter(|result| !result.is_header && result.page_index == 4)
            .map(|result| result.title.to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            subtitle_items,
            [
                "Search paths",
                "Subtitle matching mode",
                "Automatic subtitle search",
                "OpenSubtitles API key",
                "Subtitle provider",
            ]
        );
        let assrt_items = results_for_query("assrt")
            .into_iter()
            .filter(|result| !result.is_header && result.page_index == 4)
            .map(|result| result.title.to_string())
            .collect::<Vec<_>>();
        assert_eq!(assrt_items, ["Assrt.net token", "Subtitle provider"]);
        let shooter_items = results_for_query("shooter")
            .into_iter()
            .filter(|result| !result.is_header && result.page_index == 4)
            .map(|result| result.title.to_string())
            .collect::<Vec<_>>();
        assert_eq!(shooter_items, ["Subtitle provider"]);
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
        assert_eq!(border[0].anchor_y, 580);

        let auto_load = item_results("priority keywords");
        assert_eq!(auto_load.len(), 1);
        assert_eq!(auto_load[0].anchor_y, 52);
        assert_eq!(item_results("search paths")[0].anchor_y, 52);
        assert_eq!(item_results("vertical position")[0].anchor_y, 870);
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
