//! Install playback-powered windows and retain each feature's session resources.
use super::{
    PendingNavigation,
    catalog::playlist,
    discovery, fullscreen,
    history_session::Tracking,
    input, live_text,
    playlist_subtitles::{self, Assignments},
    thumbnail_actions, transitions,
};
use crate::{PlaybackStore, PlayerWindow, platform, plugin_host::PluginHostHandle};
use riina_core::{Playback, music_mode, sync::lock};
use riina_mpv::CommandSender;
use std::{
    path::Path,
    rc::Rc,
    sync::{Arc, Mutex, atomic::AtomicU64},
};

pub(super) struct PlaybackFeatures {
    _online_subtitles_window: crate::open_subtitles::SearchWindowGuard,
    _playback_history_window: crate::playback_history_window::Registration,
    _filter_target: crate::video_filter_window::Registration,
    _automatic_music_mode_timer: Rc<slint::Timer>,
    _file_drop_timer: Rc<slint::Timer>,
    _player_window_focus: input::PlayerWindowFocusRegistration,
}

pub(super) struct PlaybackTimers {
    thumbnail: Rc<slint::Timer>,
    discovery: Rc<slint::Timer>,
    transition: Option<transitions::TransitionTimer>,
    video_materials: platform::video_materials::Monitor,
    pub(super) playlist: playlist::MetadataRefresh,
    previous: Arc<Playback>,
}

impl PlaybackTimers {
    pub(super) fn observe(&mut self, app: &PlayerWindow, state: &Arc<Playback>) {
        if crate::thumbnail_preview::playback_selection_changed(&self.previous, state) {
            wake_timer(&self.thumbnail);
        }
        if let Some(transition) = &self.transition {
            transition.observe(state);
        }
        if self.previous.phase != state.phase
            || self.previous.generation != state.generation
            || !Arc::ptr_eq(&self.previous.playlist, &state.playlist)
            || !Arc::ptr_eq(&self.previous.tracks, &state.tracks)
        {
            wake_timer(&self.discovery);
        }
        self.video_materials.observe(app, state);
        if self.previous.ignore_path_in_watch_later_config
            != state.ignore_path_in_watch_later_config
        {
            app.invoke_refresh_playlist_metadata();
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
    pub(super) subtitle_assignments: Assignments,
}

pub(super) struct Inputs<'a> {
    pub(super) native: Option<&'a platform::NativePresentation>,
    pub(super) display: crate::player::surface::PlayerDisplay,
    pub(super) commands: &'a CommandSender,
    pub(super) playback: &'a PlaybackStore,
    pub(super) prefs_path: &'a Path,
    pub(super) history: &'a Tracking,
    pub(super) history_window: &'a Rc<crate::playback_history_window::ApplicationHistory>,
    pub(super) plugin_host: Option<&'a PluginHostHandle>,
    pub(super) plugin_commands: &'a Mutex<Option<CommandSender>>,
    pub(super) filters: &'a Rc<crate::video_filter_window::Profile>,
    pub(super) music_mode: &'a Arc<Mutex<music_mode::State>>,
    pub(super) transition_release_generation: &'a Arc<AtomicU64>,
    pub(super) picture_in_picture: &'a platform::PictureInPictureSession,
    pub(super) system_media: &'a platform::SystemMediaSession,
    pub(super) capture_transition_first: bool,
    pub(super) resize_filter: Option<crate::window_aspect::WindowEventFilter>,
}

pub(super) fn install(app: &PlayerWindow, inputs: Inputs<'_>) -> Installed {
    let Inputs {
        native,
        display,
        commands,
        playback,
        prefs_path,
        history,
        history_window,
        plugin_host,
        plugin_commands,
        filters,
        music_mode,
        transition_release_generation,
        picture_in_picture,
        system_media,
        capture_transition_first,
        resize_filter,
    } = inputs;
    let (history_archive, history_sync) =
        history.window_context(prefs_path, plugin_host.cloned(), playback.clone());
    let playback_history_window = history_window.install(
        app,
        commands.clone(),
        history_archive,
        history_sync,
        display,
    );
    let filter_target = crate::video_filter_window::install(
        app,
        playback.clone(),
        commands.clone(),
        filters,
        history_window,
    );
    let folder_series = crate::playlist_metadata::series::Cache::default();
    let playlist_metadata = playlist::install_metadata_refresh(
        app,
        playback.clone(),
        prefs_path,
        folder_series.clone(),
    );
    let automatic_music_mode_timer =
        crate::automatic_music_mode::install(app, playback.clone(), music_mode.clone());
    *lock(plugin_commands) = Some(commands.clone());
    let subtitle_discovery_ready = Arc::new(crate::subtitle_discovery::Readiness::default());
    let subtitle_assignments = Assignments::default();
    playlist_subtitles::install(
        app,
        &subtitle_assignments,
        playback,
        native.map(platform::NativePresentation::host),
    );
    let online_subtitles_window = crate::open_subtitles::connect_player(
        app,
        commands.clone(),
        playback.clone(),
        subtitle_discovery_ready.clone(),
        plugin_host.cloned(),
    );
    let subtitle_priority_timer = discovery::install(
        app,
        commands,
        playback.clone(),
        subtitle_discovery_ready,
        subtitle_assignments.clone(),
        folder_series,
    );
    picture_in_picture.bind(app, commands, playback);
    let fullscreen = fullscreen::install(
        app,
        native.map(platform::NativePresentation::host),
        music_mode,
    );
    #[cfg(not(target_os = "linux"))]
    drop(fullscreen);
    let input::InstalledInput {
        file_drop_timer,
        player_window_focus,
        scroll_monitor,
    } = input::install(
        app,
        commands,
        playback.clone(),
        resize_filter,
        #[cfg(target_os = "linux")]
        fullscreen,
        #[cfg(target_os = "windows")]
        native.map(platform::NativePresentation::host),
    );
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
    #[cfg(windows)]
    system_media.bind(commands, native.map(platform::NativePresentation::host));
    #[cfg(not(windows))]
    system_media.bind(commands);
    live_text::install_actions(app, prefs_path);
    super::startup::install_toolbar_order(app);
    let live_text_timer = live_text::install(app, commands.clone(), playback.clone());
    Installed {
        guards: PlaybackFeatures {
            _online_subtitles_window: online_subtitles_window,
            _playback_history_window: playback_history_window,
            _filter_target: filter_target,
            _automatic_music_mode_timer: automatic_music_mode_timer,
            _file_drop_timer: file_drop_timer,
            _player_window_focus: player_window_focus,
        },
        timers: PlaybackTimers {
            thumbnail: thumbnail_preview_timer,
            discovery: subtitle_priority_timer,
            transition: transition_frame_timer,
            video_materials,
            playlist: playlist_metadata,
            previous: Arc::default(),
        },
        live_text_timer,
        pending_navigation,
        scroll_monitor,
        subtitle_assignments,
    }
}

pub(super) fn wake_timer(timer: &slint::Timer) {
    if !timer.running() {
        timer.restart();
    }
}
