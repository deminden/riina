//! Keep engine notifications and window projection separate from session composition.
use super::{
    observations::{UiUpdateResources, update_ui},
    observer_state::ObserverId,
    playback_mailbox::{Delivery, Mailbox},
};
use crate::{
    PlaybackStore, PlayerWindow, crop_editor::Editor, opening_fullscreen::OpeningFullscreen,
    playback_history::PlaybackHistoryState, plugin_files::PluginPlayerFileContext,
    plugin_host::PluginHostHandle, screenshot_output::Scratch,
};
use riina_core::{Phase, Playback, RecentMedia, music_mode, sync::lock, video_filters};
use riina_mpv::CommandSender;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

pub(super) struct PlaybackNotification {
    pub(super) reported_playback: PlaybackStore,
    pub(super) queued: Arc<AtomicBool>,
    pub(super) latest: Arc<Mailbox>,
    pub(super) history: HistoryObservation,
    pub(super) delivery: WindowDelivery,
    pub(super) picture_in_picture_state_dirty: Option<Arc<AtomicBool>>,
    // The engine callback keeps the capture scratch alive until native destruction.
    pub(super) _screenshot_scratch: Arc<Scratch>,
}

impl PlaybackNotification {
    pub(super) fn publish(&self, state: Arc<Playback>) {
        *lock(&self.reported_playback) = state.clone();
        self.history.publish(&self.delivery.app, &state);
        if let Some(changed) = &self.picture_in_picture_state_dirty {
            changed.store(true, Ordering::Release);
        }

        self.latest.publish(state);
        if self.queued.swap(true, Ordering::AcqRel) {
            return;
        }
        let latest = self.latest.clone();
        let queued = self.queued.clone();
        let window = self.delivery.clone();
        let _ = slint::invoke_from_event_loop(move || {
            queued.store(false, Ordering::Release);
            window.deliver(&latest);
        });
    }
}

pub(super) struct HistoryObservation {
    pub(super) recent_media: Arc<Mutex<Vec<RecentMedia>>>,
    pub(super) history: Arc<Mutex<PlaybackHistoryState>>,
    pub(super) record_history: Arc<AtomicBool>,
    pub(super) session_id: u64,
    pub(super) plugin: Option<PluginHostHandle>,
    pub(super) last_plugin_context: Mutex<PluginPlayerFileContext>,
}

impl HistoryObservation {
    fn publish(&self, app: &slint::Weak<PlayerWindow>, state: &Playback) {
        let (plugin_context, history_error) = crate::plugin_snapshot::observe_playback(
            state,
            &self.recent_media,
            &self.history,
            self.record_history.load(Ordering::Acquire),
            self.session_id,
        );
        if let Some(error) = history_error {
            let _ = crate::background::to_ui(app.clone(), move |app| {
                app.set_status(format!("Could not save playback history: {error}").into());
            });
        }
        let mut last_context = lock(&self.last_plugin_context);
        if *last_context != plugin_context
            && let Some(host) = &self.plugin
            && host
                .update_player_file_context(plugin_context.clone())
                .is_ok()
        {
            *last_context = plugin_context;
        }
    }
}

#[derive(Clone)]
pub(super) struct WindowDelivery {
    pub(super) app: slint::Weak<PlayerWindow>,
    pub(super) observer_id: ObserverId,
    pub(super) crop_editor: Editor,
    pub(super) saved_video_filters: Arc<Mutex<video_filters::Settings>>,
    pub(super) saved_audio_filters: Arc<Mutex<video_filters::Settings>>,
    pub(super) audio_device_sender: Arc<Mutex<Option<CommandSender>>>,
    pub(super) opening_fullscreen: Arc<Mutex<OpeningFullscreen>>,
    pub(super) music_mode_state: Arc<Mutex<music_mode::State>>,
    pub(super) transition_overlay: Arc<AtomicU64>,
    #[cfg(target_os = "windows")]
    pub(super) transition_release_generation: Arc<AtomicU64>,
}

impl WindowDelivery {
    fn deliver(&self, latest: &Mailbox) {
        if let Some(app) = self.app.upgrade()
            && let Some(observer) = self.observer_id.lookup()
            && let Some(Delivery { state, captures }) = latest.take()
        {
            let first_loaded = (!app.get_loaded()
                || observer.borrow().media_geometry.generation() != Some(state.generation))
                && matches!(
                    state.phase,
                    Phase::Playing | Phase::Paused | Phase::Buffering | Phase::Ended
                );
            let enter_fullscreen = lock(&self.opening_fullscreen).observe(
                &state,
                app.get_fullscreen_when_open(),
                app.get_fullscreen(),
                false,
            );
            let clipboard_error = crate::screenshot_output::copy(&app, &captures);
            self.crop_editor.observe(&app, &state);
            let audio_device_sender = lock(&self.audio_device_sender).clone();
            crate::automatic_music_mode::observe(&app, &state, &self.music_mode_state);
            let resize_for_media = update_ui(
                &app,
                state,
                UiUpdateResources {
                    audio_device_player: audio_device_sender.as_ref(),
                    saved_video_filters: &self.saved_video_filters,
                    saved_audio_filters: &self.saved_audio_filters,
                    transition_overlay: self.transition_overlay.as_ref(),
                    #[cfg(target_os = "windows")]
                    transition_release_generation: self.transition_release_generation.as_ref(),
                },
                &mut observer.borrow_mut(),
            );
            if resize_for_media {
                // Apply the complete geometry after automatic music mode has
                // restored the video window. Coalesced snapshots may omit
                // Loading, so media identity owns this transition.
                app.invoke_resize_window_for_media();
            }
            if first_loaded {
                app.invoke_initial_media_loaded();
            }
            crate::screenshot_output::cleanup(&app, &captures);
            if enter_fullscreen {
                app.invoke_toggle_fullscreen();
            }
            if let Some(error) = clipboard_error {
                app.set_status(format!("Could not copy screenshot: {error}").into());
            }
        }
    }
}
