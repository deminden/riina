//! Persisted application preferences, defaults and schema validation.
use crate::{
    DefaultRepeatMode, advanced_preferences, audio_equalizer, audio_preferences,
    control_preferences, hdr_preferences, key_bindings, liquid_glass, network_preferences,
    online_subtitles, osc_preferences, pip_preferences, playback_history, playback_power,
    screenshot, subtitle_controls, thumbnail_preferences, valid_path, video_controls,
    video_filters, window_resize,
};
use serde::{Deserialize, Serialize, de::value::MapDeserializer};
use serde_json::value::RawValue;
use std::collections::BTreeMap;

fn default_true() -> bool {
    true
}

fn is_false(value: &bool) -> bool {
    !value
}

fn is_zero_revision(revision: &u64) -> bool {
    *revision == 0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
// Root-level preferences grow additively between releases. Older binaries must
// ignore fields they do not understand so a settings save cannot prevent media
// playback after a user temporarily launches an earlier installed version.
#[serde(default)]
pub struct Preferences {
    pub leading_sidebar_width: u32,
    pub trailing_sidebar_width: u32,
    pub settings_at_leading: bool,
    pub playlist_at_leading: bool,
    pub plugins_at_leading: bool,
    pub edge_to_edge_video: bool,
    pub docked_controls: bool,
    pub unlock_window_aspect_ratio: bool,
    pub window_resize: window_resize::Settings,
    pub compact_ui: bool,
    pub osc_position: u8,
    /// Horizontal center of the floating control bar as a fraction of video width.
    pub control_bar_position_horizontal: f64,
    /// Gap below the floating control bar as a fraction of video height.
    pub control_bar_position_vertical: f64,
    pub control_bar: osc_preferences::Settings,
    #[serde(skip_serializing_if = "crate::touch_bar::Settings::is_default")]
    pub touch_bar: crate::touch_bar::Settings,
    pub always_float_on_top: bool,
    pub always_show_on_top_icon: bool,
    pub action_after_launch: ActionAfterLaunch,
    #[serde(alias = "enableCmdN", skip_serializing_if = "is_false")]
    pub enable_new_window_shortcut: bool,
    pub allow_multiple_windows: bool,
    pub group_simultaneous_opens_in_playlist: bool,
    pub allow_duplicate_players: bool,
    pub playlist_auto_add: bool,
    pub playlist_auto_play_next: bool,
    pub playlist_show_metadata: bool,
    pub playlist_show_metadata_in_music_mode: bool,
    pub prefetch_playlist_video_duration: bool,
    pub auto_repeat: bool,
    pub default_repeat_mode: DefaultRepeatMode,
    pub quit_when_no_opened_window: bool,
    pub keep_open_on_file_end: bool,
    pub pause_when_open: bool,
    pub play_when_entering_fullscreen: bool,
    pub pause_when_leaving_fullscreen: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub use_legacy_fullscreen: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub black_out_other_monitors: bool,
    pub pause_when_inactive: bool,
    pub pause_when_minimized: bool,
    pub pause_when_goes_to_sleep: bool,
    pub playback_power: playback_power::Settings,
    pub fullscreen_when_open: bool,
    pub resume_last_position: bool,
    pub auto_switch_to_music_mode: bool,
    pub disable_animations: bool,
    pub music_mode_show_playlist: bool,
    pub music_mode_show_album_art: bool,
    pub schema_version: u32,
    pub volume: f64,
    pub volume_boost: bool,
    pub advanced: advanced_preferences::Settings,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inspector_watch_properties: Vec<String>,
    pub audio: audio_preferences::Settings,
    #[serde(default)]
    pub audio_equalizer: audio_equalizer::Settings,
    #[serde(default)]
    pub video_filters: video_filters::Settings,
    #[serde(default)]
    pub audio_filters: video_filters::Settings,
    pub hdr_enabled: bool,
    #[serde(default = "default_true")]
    pub load_icc_profile: bool,
    pub live_text_enabled: bool,
    pub hdr_tone_mapping: hdr_preferences::Settings,
    pub video_threads: u32,
    pub controls: control_preferences::Settings,
    pub key_bindings: key_bindings::Settings,
    pub liquid_glass: liquid_glass::SurfaceSettings,
    pub hardware_decoder: video_controls::HardwareDecoder,
    pub subtitle_ass_override: subtitle_controls::AssOverrideLevel,
    pub secondary_subtitle_ass_override: subtitle_controls::AssOverrideLevel,
    pub subtitle_position: subtitle_controls::PositionPreferences,
    pub subtitle_other: subtitle_controls::OtherPreferences,
    pub subtitle_auto_load: subtitle_controls::AutoLoadPreferences,
    pub online_subtitles: online_subtitles::Preferences,
    pub subtitle_font: String,
    pub subtitle_font_size: f64,
    pub subtitle_text_style: subtitle_controls::TextStylePreferences,
    // `None` means a settings file predates explicit appearance selection; in
    // that case keep honoring its saved `dark` bit instead of changing themes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme_material: Option<ThemeMaterial>,
    pub dark: bool,
    pub compact: bool,
    pub enable_osd: bool,
    pub disable_osd_file_start: bool,
    pub disable_osd_pause_resume: bool,
    pub disable_osd_speed: bool,
    pub disable_osd_seek: bool,
    pub osd_timeout_secs: f64,
    pub osd_text_size: f64,
    pub display_time_and_battery_in_fullscreen: bool,
    pub show_chapter_pos: bool,
    pub picture_in_picture: pip_preferences::Settings,
    pub screenshot: screenshot::Settings,
    pub network: network_preferences::Connection,
    pub network_cache: network_preferences::Cache,
    pub network_online_videos: network_preferences::OnlineVideos,
    pub thumbnail_preview: thumbnail_preferences::Settings,
    pub recent_media: Vec<RecentMedia>,
    #[serde(skip_serializing_if = "is_zero_revision")]
    pub recent_resume_revision: u64,
    #[serde(default = "default_true")]
    pub record_recent_files: bool,
    #[serde(default = "default_true")]
    pub track_all_files_in_recent_open_menu: bool,
    pub recent_open_documents: Vec<RecentMedia>,
    #[serde(default = "default_true")]
    pub record_playback_history: bool,
    #[serde(default, skip_serializing)]
    pub playback_history: Vec<playback_history::Entry>,
}

/// Appearance intent. `System` follows the effective theme reported by the
/// native window system while keeping the player palette in sync with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMaterial {
    Dark,
    Light,
    System,
}

/// Startup action for launches without a media argument, matching IINA's
/// General > Behavior choice while keeping supplied media arguments primary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ActionAfterLaunch {
    #[default]
    WelcomeWindow,
    OpenPanel,
    None,
}

impl ActionAfterLaunch {
    pub const fn show_welcome(self, has_media_argument: bool) -> bool {
        !has_media_argument && matches!(self, Self::WelcomeWindow)
    }

    pub const fn open_file_panel(self, has_media_argument: bool) -> bool {
        !has_media_argument && matches!(self, Self::OpenPanel)
    }

    pub const fn from_index(index: i32) -> Option<Self> {
        match index {
            0 => Some(Self::WelcomeWindow),
            1 => Some(Self::OpenPanel),
            2 => Some(Self::None),
            _ => None,
        }
    }

    pub const fn index(self) -> i32 {
        match self {
            Self::WelcomeWindow => 0,
            Self::OpenPanel => 1,
            Self::None => 2,
        }
    }
}

impl ThemeMaterial {
    pub const fn from_index(index: i32) -> Option<Self> {
        match index {
            0 => Some(Self::Dark),
            1 => Some(Self::Light),
            2 => Some(Self::System),
            _ => None,
        }
    }

    pub const fn index(self) -> i32 {
        match self {
            Self::Dark => 0,
            Self::Light => 1,
            Self::System => 2,
        }
    }

    pub const fn dark_for(self, system_is_dark: bool) -> bool {
        match self {
            Self::Dark => true,
            Self::Light => false,
            Self::System => system_is_dark,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecentMedia {
    pub path: String,
    pub title: String,
    pub position: f64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreferenceError {
    pub key: &'static str,
    pub reason: String,
}

impl PreferenceError {
    pub(crate) fn new(key: &'static str, reason: impl Into<String>) -> Self {
        Self {
            key,
            reason: reason.into(),
        }
    }
}

impl std::fmt::Display for PreferenceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.reason)
    }
}

impl std::error::Error for PreferenceError {}

impl From<PreferenceError> for String {
    fn from(error: PreferenceError) -> Self {
        error.reason
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Rejection {
    /// A serialized preference key, or `$` for the entire document.
    pub key: String,
    pub reason: String,
}

impl Default for Preferences {
    fn default() -> Self {
        // IINA Preference.defaultPreference: softVolume=100, themeMaterial=dark.
        Self {
            leading_sidebar_width: 360,
            trailing_sidebar_width: 360,
            settings_at_leading: false,
            playlist_at_leading: true,
            plugins_at_leading: true,
            edge_to_edge_video: true,
            docked_controls: false,
            unlock_window_aspect_ratio: false,
            window_resize: window_resize::Settings::default(),
            compact_ui: false,
            osc_position: 0,
            control_bar_position_horizontal: 0.5,
            control_bar_position_vertical: 0.1,
            control_bar: osc_preferences::Settings::default(),
            touch_bar: crate::touch_bar::Settings::default(),
            always_float_on_top: false,
            always_show_on_top_icon: false,
            action_after_launch: ActionAfterLaunch::default(),
            enable_new_window_shortcut: false,
            allow_multiple_windows: true,
            group_simultaneous_opens_in_playlist: false,
            allow_duplicate_players: false,
            playlist_auto_add: true,
            playlist_auto_play_next: true,
            playlist_show_metadata: true,
            playlist_show_metadata_in_music_mode: true,
            prefetch_playlist_video_duration: true,
            auto_repeat: false,
            default_repeat_mode: DefaultRepeatMode::Playlist,
            quit_when_no_opened_window: false,
            keep_open_on_file_end: true,
            pause_when_open: false,
            play_when_entering_fullscreen: false,
            pause_when_leaving_fullscreen: false,
            use_legacy_fullscreen: false,
            black_out_other_monitors: false,
            pause_when_inactive: false,
            pause_when_minimized: false,
            pause_when_goes_to_sleep: true,
            playback_power: playback_power::Settings::default(),
            fullscreen_when_open: false,
            resume_last_position: true,
            auto_switch_to_music_mode: true,
            disable_animations: false,
            music_mode_show_playlist: false,
            music_mode_show_album_art: true,
            schema_version: 1,
            volume: 100.0,
            volume_boost: false,
            advanced: advanced_preferences::Settings::default(),
            inspector_watch_properties: Vec::new(),
            audio: audio_preferences::Settings::default(),
            audio_equalizer: audio_equalizer::Settings::default(),
            video_filters: video_filters::Settings::default(),
            audio_filters: video_filters::Settings::default(),
            hdr_enabled: true,
            load_icc_profile: true,
            live_text_enabled: false,
            hdr_tone_mapping: hdr_preferences::Settings::default(),
            video_threads: 0,
            controls: control_preferences::Settings::default(),
            key_bindings: key_bindings::Settings::default(),
            liquid_glass: liquid_glass::SurfaceSettings::default(),
            hardware_decoder: video_controls::HardwareDecoder::Auto,
            subtitle_ass_override: subtitle_controls::AssOverrideLevel::Scale,
            secondary_subtitle_ass_override: subtitle_controls::AssOverrideLevel::Scale,
            subtitle_position: subtitle_controls::PositionPreferences::default(),
            subtitle_other: subtitle_controls::OtherPreferences::default(),
            subtitle_auto_load: subtitle_controls::AutoLoadPreferences::default(),
            online_subtitles: online_subtitles::Preferences::default(),
            subtitle_font: subtitle_controls::DEFAULT_SUBTITLE_FONT.into(),
            subtitle_font_size: 55.0,
            subtitle_text_style: subtitle_controls::TextStylePreferences::default(),
            theme_material: Some(ThemeMaterial::Dark),
            dark: true,
            compact: false,
            enable_osd: true,
            disable_osd_file_start: false,
            disable_osd_pause_resume: false,
            disable_osd_speed: false,
            disable_osd_seek: false,
            osd_timeout_secs: 1.0,
            osd_text_size: 20.0,
            display_time_and_battery_in_fullscreen: false,
            show_chapter_pos: false,
            picture_in_picture: pip_preferences::Settings::default(),
            screenshot: screenshot::Settings::default(),
            network: network_preferences::Connection::default(),
            network_cache: network_preferences::Cache::default(),
            network_online_videos: network_preferences::OnlineVideos::default(),
            thumbnail_preview: thumbnail_preferences::Settings::default(),
            recent_media: Vec::new(),
            recent_resume_revision: 0,
            record_recent_files: true,
            track_all_files_in_recent_open_menu: true,
            recent_open_documents: Vec::new(),
            record_playback_history: true,
            playback_history: Vec::new(),
        }
    }
}

impl Preferences {
    pub fn effective_theme_material(&self) -> ThemeMaterial {
        self.theme_material.unwrap_or(if self.dark {
            ThemeMaterial::Dark
        } else {
            ThemeMaterial::Light
        })
    }

    pub fn volume_limit(&self) -> f64 {
        if self.volume_boost { 200.0 } else { 100.0 }
    }

    pub fn remember_media(&mut self, media: RecentMedia) {
        // Keep one durable resume point per path and cap history like the compact
        // welcome list; paths are user data and are never canonicalized here.
        self.recent_media.retain(|item| item.path != media.path);
        self.recent_media.insert(0, media);
        self.recent_media.truncate(10);
    }

    pub fn parse(text: &str) -> Result<Self, PreferenceError> {
        let prefs: Self = serde_json::from_str(text)
            .map_err(|error| PreferenceError::new("$", error.to_string()))?;
        prefs.validated()
    }

    pub fn parse_lenient(text: &str) -> (Self, Vec<Rejection>) {
        // Borrow each raw value so an out-of-range JSON number affects only its
        // own section, and preserve the input for deserialization-only fields.
        let Ok(mut fields) = serde_json::from_str::<BTreeMap<String, &RawValue>>(text) else {
            return rejected_document("Expected a complete JSON object");
        };
        let mut rejected = Vec::new();
        fields.retain(|key, value| {
            let entry = MapDeserializer::new([(key.as_str(), *value)].into_iter());
            if let Err(reason) = Self::validate_recovered(entry) {
                rejected.push(Rejection {
                    key: key.clone(),
                    reason: reason.reason,
                });
                false
            } else {
                true
            }
        });
        // Apply interdependent normalization once with all surviving settings.
        // In particular, the independent volume check must not discard boost.
        let map = MapDeserializer::new(fields.iter().map(|(key, value)| (key.as_str(), *value)));
        match Self::validate_recovered(map) {
            Ok(mut prefs) => {
                // A missing theme is the legacy dark-bit format. A malformed
                // explicit selection instead resets to the current default.
                if rejected.iter().any(|entry| entry.key == "theme_material") {
                    prefs.theme_material = Self::default().theme_material;
                }
                (prefs, rejected)
            }
            Err(reason) => rejected_document(reason.reason),
        }
    }

    fn validate_recovered<'de>(
        map: impl serde::Deserializer<'de, Error = serde_json::Error>,
    ) -> Result<Self, PreferenceError> {
        // Serde type errors can echo private values. Recovery reasons go in the log.
        Self::deserialize(map)
            .map_err(|_| PreferenceError::new("$", "Invalid value or structure"))?
            .validated()
    }

    fn validated(mut self) -> Result<Self, PreferenceError> {
        if self.schema_version != 1 {
            return Err(PreferenceError::new(
                "schema_version",
                format!(
                    "Unsupported preference schema version {}",
                    self.schema_version
                ),
            ));
        }
        if !self.volume.is_finite() || !(0.0..=200.0).contains(&self.volume) {
            return Err(PreferenceError::new(
                "volume",
                "Volume must be between 0 and 200",
            ));
        }
        if self.video_threads > i32::MAX as u32 {
            return Err(PreferenceError::new(
                "video_threads",
                "Video thread count exceeds the supported range",
            ));
        }
        if !subtitle_controls::valid_preference_font_size(self.subtitle_font_size) {
            return Err(PreferenceError::new(
                "subtitle_font_size",
                "Subtitle font size is outside the supported range",
            ));
        }
        if !subtitle_controls::valid_font_name(&self.subtitle_font) {
            return Err(PreferenceError::new(
                "subtitle_font",
                "Subtitle font name is invalid",
            ));
        }
        self.validate_sections()?;
        // Old settings keep their default cap unless boosting was explicitly enabled.
        self.volume = self.volume.min(self.volume_limit());
        self.leading_sidebar_width = self.leading_sidebar_width.clamp(300, 500);
        self.trailing_sidebar_width = self.trailing_sidebar_width.clamp(300, 500);
        if self.osc_position > 2 {
            return Err(PreferenceError::new(
                "osc_position",
                "Unsupported on-screen controller position",
            ));
        }
        for (key, position) in [
            (
                "control_bar_position_horizontal",
                self.control_bar_position_horizontal,
            ),
            (
                "control_bar_position_vertical",
                self.control_bar_position_vertical,
            ),
        ] {
            if !(0.0..=1.0).contains(&position) {
                return Err(PreferenceError::new(
                    key,
                    "Floating control bar position must be between 0 and 1",
                ));
            }
        }
        self.screenshot
            .validate()
            .map_err(|reason| PreferenceError::new("screenshot", reason))?;
        if !valid_recent_media(&self.recent_media) {
            return Err(PreferenceError::new(
                "recent_media",
                "Recent media history is invalid",
            ));
        }
        if !valid_recent_media(&self.recent_open_documents) {
            return Err(PreferenceError::new(
                "recent_open_documents",
                "Open Recent documents are invalid",
            ));
        }
        if self.playback_history.iter().any(|entry| !entry.is_valid()) {
            return Err(PreferenceError::new(
                "playback_history",
                "Playback history is invalid",
            ));
        }
        // Persisted values must be representable by the event-loop timer. Do not
        // silently turn an invalid duration into an immediate or endless OSD.
        let timeout =
            std::time::Duration::try_from_secs_f64(self.osd_timeout_secs).map_err(|_| {
                PreferenceError::new("osd_timeout_secs", "Invalid on-screen display timeout")
            })?;
        if timeout.as_millis() > u64::MAX as u128 {
            return Err(PreferenceError::new(
                "osd_timeout_secs",
                "On-screen display timeout exceeds timer capacity",
            ));
        }
        if !(5.0..=500.0).contains(&self.osd_text_size) {
            return Err(PreferenceError::new(
                "osd_text_size",
                "On-screen display text size must be between 5 and 500 points",
            ));
        }
        Ok(self)
    }

    fn validate_sections(&self) -> Result<(), PreferenceError> {
        self.subtitle_text_style.validate()?;
        self.subtitle_position.validate()?;
        self.network_cache.validate()?;
        self.thumbnail_preview.validate()?;
        self.network.validate()?;
        self.network_online_videos.validate()?;
        self.audio.validate()?;
        self.audio_equalizer
            .validate()
            .map_err(|reason| PreferenceError::new("audio_equalizer", reason.to_string()))?;
        self.video_filters
            .validate()
            .map_err(|reason| PreferenceError::new("video_filters", reason.to_string()))?;
        self.audio_filters
            .validate()
            .map_err(|reason| PreferenceError::new("audio_filters", reason.to_string()))?;
        self.hdr_tone_mapping.validate()?;
        self.advanced.validate()?;
        self.controls.validate()?;
        self.control_bar.validate()?;
        self.key_bindings.validate()?;
        self.subtitle_other.validate()?;
        self.subtitle_auto_load.validate()?;
        self.online_subtitles.validate()?;
        Ok(())
    }
}

fn rejected_document(reason: impl Into<String>) -> (Preferences, Vec<Rejection>) {
    (
        Preferences::default(),
        vec![Rejection {
            key: "$".into(),
            reason: reason.into(),
        }],
    )
}

fn valid_recent_media(items: &[RecentMedia]) -> bool {
    items.len() <= 10
        && items.iter().all(|item| {
            valid_path(&item.path)
                && !item.title.contains('\0')
                && item.position.is_finite()
                && item.position >= 0.0
        })
}
