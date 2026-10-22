//! Player intent and observed state. The backend is the only playback clock.

pub mod advanced_preferences;
pub mod audio_equalizer;
pub mod audio_preferences;
pub mod control_preferences;
pub mod crop;
pub mod crop_drag;
pub mod crop_geometry;
pub mod crop_session;
pub mod delogo;
pub mod file_selection;
pub mod hdr_preferences;
pub mod key_bindings;
pub mod liquid_glass;
pub mod media_reference;
pub mod music_mode;
pub mod network_preferences;
pub mod online_subtitles;
pub mod osc_preferences;
pub mod pip_lifecycle;
pub mod pip_preferences;
pub mod playback_history;
pub mod playlist;
pub mod plugin;
pub mod plugin_menu;
pub mod plugin_order;
pub mod plugin_sidebar;
pub mod screenshot;
pub mod speed_controls;
pub mod subtitle_color;
pub mod subtitle_controls;
pub mod thumbnail_preferences;
pub mod time_input;
pub mod timing_controls;
mod track_labels;
pub mod video_controls;
pub mod video_filters;
pub mod video_reflection;
pub mod video_viewport;
pub mod window_aspect;
pub mod window_resize;

mod captured_frame;
mod command;
mod loop_mode;
mod media_catalog;
mod playback;
mod preferences;

pub use captured_frame::CapturedFrame;
pub use command::{AbLoopPoint, Command, CommandPlan};
pub use loop_mode::{DefaultRepeatMode, LoopMode};
pub use media_catalog::{Chapter, PlaylistEntry, Track, TrackKind};
pub use playback::{
    MAX_RETAINED_MPV_EVENTS, MpvEventRecord, Phase, Playback, SubtitleStyleAvailability,
    VideoFrameInfo, VideoParameters, time_label,
};
pub use preferences::{ActionAfterLaunch, Preferences, RecentMedia, ThemeMaterial};

pub const MAX_PLUGIN_OBSERVED_MPV_PROPERTIES: usize = 128;

fn valid_path(path: &str) -> bool {
    !path.is_empty() && !path.contains('\0')
}

pub fn playlist_end_policy(
    keep_open_on_file_end: bool,
    playlist_auto_play_next: bool,
) -> &'static str {
    if !playlist_auto_play_next {
        "always"
    } else if keep_open_on_file_end {
        "yes"
    } else {
        "no"
    }
}

#[cfg(test)]
mod tests;
