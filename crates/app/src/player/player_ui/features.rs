//! Install playback-powered windows and retain each feature's session resources.
use super::{
    PendingNavigation, catalog::install_playlist_metadata_refresh, discovery,
    history_session::Tracking, input, live_text, thumbnail_actions, transitions,
};
use crate::{PlaybackStore, PlayerWindow, platform, plugin_host::PluginHostHandle};
use riina_core::{Playback, music_mode, sync::lock, video_filters};
use riina_mpv::CommandSender;
use std::{
    path::Path,
    rc::Rc,
    sync::{Arc, Mutex, atomic::AtomicU64},
};

pub(super) struct PlaybackFeatures {
    _video_materials: platform::video_materials::Monitor,
    _subtitle_priority_timer: Rc<slint::Timer>,
    _online_subtitles_window: crate::open_subtitles::SearchWindowGuard,
    _playback_history_window: crate::playback_history_window::Guard,
    _filter_windows: crate::video_filter_window::Guard,
    _playlist_metadata_timer: Rc<slint::Timer>,
    _automatic_music_mode_timer: Rc<slint::Timer>,
    _file_drop_timer: Rc<slint::Timer>,
    _player_window_focus: input::PlayerWindowFocusRegistration,
}

pub(super) struct PlaybackTimers {
    thumbnail: Rc<slint::Timer>,
    transition: Option<transitions::TransitionTimer>,
    previous: Arc<Playback>,
}

impl PlaybackTimers {
    pub(super) fn observe(&mut self, state: &Arc<Playback>) {
        if crate::thumbnail_preview::playback_selection_changed(&self.previous, state) {
            thumbnail_actions::wake(&self.thumbnail);
        }
        if let Some(transition) = &self.transition {
            transition.observe(state);
        }
        self.previous.clone_from(state);
    }
}

pub(super) struct Installed {
    pub(super) guards: PlaybackFeatures,
    pub(super) timers: PlaybackTimers,
    pub(super) live_text_timer: Option<Rc<slint::Timer>>,
    pub(super) pending_navigation: PendingNavigation,
    pub(super) scroll_monitor: platform::PlayerScrollMonitor,
}

pub(super) struct Inputs<'a> {
    pub(super) native: Option<&'a platform::NativePresentation>,
    pub(super) commands: &'a CommandSender,
    pub(super) playback: &'a PlaybackStore,
    pub(super) prefs_path: &'a Path,
    pub(super) history: &'a Tracking,
    pub(super) plugin_host: Option<&'a PluginHostHandle>,
    pub(super) plugin_commands: &'a Mutex<Option<CommandSender>>,
    pub(super) saved_video_filters: &'a Arc<Mutex<video_filters::Settings>>,
    pub(super) saved_audio_filters: &'a Arc<Mutex<video_filters::Settings>>,
    pub(super) music_mode: &'a Arc<Mutex<music_mode::State>>,
    pub(super) transition_release_generation: &'a Arc<AtomicU64>,
    pub(super) picture_in_picture: &'a platform::PictureInPictureSession,
    pub(super) system_media: &'a platform::SystemMediaSession,
    pub(super) capture_transition_first: bool,
}

pub(super) fn install(app: &PlayerWindow, inputs: Inputs<'_>) -> Installed {
    let Inputs {
        native,
        commands,
        playback,
        prefs_path,
        history,
        plugin_host,
        plugin_commands,
        saved_video_filters,
        saved_audio_filters,
        music_mode,
        transition_release_generation,
        picture_in_picture,
        system_media,
        capture_transition_first,
    } = inputs;
    let playback_history_window = history.install_window(
        app,
        commands,
        prefs_path,
        plugin_host.cloned(),
        playback.clone(),
    );
    let filter_windows = crate::video_filter_window::install(
        app,
        playback.clone(),
        saved_video_filters.clone(),
        saved_audio_filters.clone(),
        commands.clone(),
        prefs_path.to_path_buf(),
    );
    let playlist_metadata_timer = install_playlist_metadata_refresh(app, playback.clone());
    let automatic_music_mode_timer =
        crate::automatic_music_mode::install(app, playback.clone(), music_mode.clone());
    *lock(plugin_commands) = Some(commands.clone());
    let subtitle_discovery_ready = Arc::new(crate::subtitle_discovery::Readiness::default());
    let online_subtitles_window = crate::open_subtitles::connect_player(
        app,
        commands.clone(),
        playback.clone(),
        subtitle_discovery_ready.clone(),
    );
    let subtitle_priority_timer =
        discovery::install(app, commands, playback.clone(), subtitle_discovery_ready);
    picture_in_picture.bind(app, commands, playback);
    let input::InstalledInput {
        file_drop_timer,
        player_window_focus,
        scroll_monitor,
    } = input::install(app, commands, playback.clone());
    let thumbnail_preview_timer = thumbnail_actions::install(app, playback, plugin_host.cloned());
    let (transition_frame_timer, pending_navigation) = transitions::install(
        capture_transition_first,
        native,
        commands,
        playback,
        transition_release_generation,
    );
    let video_materials =
        platform::video_materials::Monitor::install(app, native, commands, playback);
    system_media.bind(commands);
    let live_text_timer = live_text::install(app, commands.clone(), playback.clone());
    Installed {
        guards: PlaybackFeatures {
            _video_materials: video_materials,
            _subtitle_priority_timer: subtitle_priority_timer,
            _online_subtitles_window: online_subtitles_window,
            _playback_history_window: playback_history_window,
            _filter_windows: filter_windows,
            _playlist_metadata_timer: playlist_metadata_timer,
            _automatic_music_mode_timer: automatic_music_mode_timer,
            _file_drop_timer: file_drop_timer,
            _player_window_focus: player_window_focus,
        },
        timers: PlaybackTimers {
            thumbnail: thumbnail_preview_timer,
            transition: transition_frame_timer,
            previous: Arc::default(),
        },
        live_text_timer,
        pending_navigation,
        scroll_monitor,
    }
}
