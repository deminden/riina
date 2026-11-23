//! Keep engine notifications and window projection separate from session composition.
use super::{
    observations::{UiUpdateResources, update_ui},
    observer_state::{ObserverId, ObserverRegistration, UiObserverState},
    playback_mailbox::{Delivery, Mailbox},
};
use crate::{
    NativeVideoHost, PlaybackStore, PlayerWindow, background::CoalescedUiCallback,
    crop_editor::Editor, opening_fullscreen::OpeningFullscreen,
    playback_history::PlaybackHistoryState, plugin_files::PluginPlayerFileContext,
    plugin_host::PluginHostHandle, plugins::plugin_players::Ticket, screenshot_output::Scratch,
    video_osd::VideoOsd, window_aspect::MediaGeometry,
};
use riina_core::{
    Phase, Playback, Preferences, RecentMedia, music_mode, sync::lock, video_filters,
};
use riina_mpv::CommandSender;
use riina_ui::PlayerMenuActions;
use slint::{ComponentHandle, winit_030::WinitWindowAccessor};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

pub(super) struct Inputs<'a> {
    pub(super) prefs: &'a Preferences,
    pub(super) media_geometry: MediaGeometry,
    pub(super) native_presentation: bool,
    pub(super) reported_playback: PlaybackStore,
    pub(super) history: HistoryObservation,
    pub(super) saved_video_filters: Arc<Mutex<video_filters::Settings>>,
    pub(super) saved_audio_filters: Arc<Mutex<video_filters::Settings>>,
    pub(super) audio_device_sender: Arc<Mutex<Option<CommandSender>>>,
    pub(super) picture_in_picture_state_dirty: Option<Arc<AtomicBool>>,
    pub(super) managed_visibility: Option<ManagedVisibility>,
}

pub(super) struct PreparedObservation {
    pub(super) notification: PlaybackNotification,
    pub(super) music_mode_state: Arc<Mutex<music_mode::State>>,
    pub(super) crop_editor: Editor,
    pub(super) observer_state: ObserverRegistration,
    pub(super) feedback_video_osd: Rc<RefCell<VideoOsd>>,
    pub(super) screenshot_scratch: Arc<Scratch>,
    pub(super) transition_release_generation: Arc<AtomicU64>,
}

pub(super) fn prepare_observation(app: &PlayerWindow, inputs: Inputs<'_>) -> PreparedObservation {
    let Inputs {
        prefs,
        media_geometry,
        native_presentation,
        reported_playback,
        history,
        saved_video_filters,
        saved_audio_filters,
        audio_device_sender,
        picture_in_picture_state_dirty,
        managed_visibility,
    } = inputs;
    let app_weak = app.as_weak();
    // Only one pending state snapshot is retained; a decoder can produce updates
    // faster than the UI consumes them without growing the application event queue.
    // State and capture completions share one handoff. A newer completion must
    // never be cleaned up while an older snapshot is still being rendered.
    let latest = Arc::new(Mailbox::default());
    let transition_overlay = Arc::new(AtomicU64::new(u64::MAX));
    let transition_release_generation = Arc::new(AtomicU64::new(u64::MAX));
    let notify_transition_overlay = transition_overlay.clone();
    let notify_transition_release_generation = transition_release_generation.clone();
    let music_mode_state = Arc::new(std::sync::Mutex::new(music_mode::State::new(
        prefs.auto_switch_to_music_mode,
    )));
    if !native_presentation {
        super::music_mode::install_window_resize(app);
    }
    let crop_editor = Editor::default();
    let observed_crop_editor = crop_editor.clone();
    let mut video_osd = VideoOsd::new(prefs.disable_osd_speed, prefs.disable_osd_pause_resume);
    video_osd.set_volume_limit(prefs.volume_limit());
    let observer_state = UiObserverState::new(video_osd, media_geometry);
    let feedback_video_osd = observer_state.video.clone();
    let observer_state = ObserverRegistration::new(observer_state);
    let observer_id = observer_state.id();
    let screenshot_scratch = Arc::new(Scratch::default());
    let opening_fullscreen = Arc::new(std::sync::Mutex::new(OpeningFullscreen::default()));
    let notification = PlaybackNotification {
        reported_playback,
        delivery_callback: CoalescedUiCallback::default(),
        latest,
        history,
        delivery: WindowDelivery {
            app: app_weak,
            observer_id,
            crop_editor: observed_crop_editor,
            saved_video_filters,
            saved_audio_filters,
            audio_device_sender,
            opening_fullscreen,
            music_mode_state: music_mode_state.clone(),
            transition_overlay: notify_transition_overlay,
            transition_release_generation: notify_transition_release_generation,
            managed_visibility,
        },
        picture_in_picture_state_dirty,
        _screenshot_scratch: screenshot_scratch.clone(),
    };
    PreparedObservation {
        notification,
        music_mode_state,
        crop_editor,
        observer_state,
        feedback_video_osd,
        screenshot_scratch,
        transition_release_generation,
    }
}

pub(super) struct PlaybackNotification {
    reported_playback: PlaybackStore,
    delivery_callback: CoalescedUiCallback,
    latest: Arc<Mailbox>,
    history: HistoryObservation,
    delivery: WindowDelivery,
    picture_in_picture_state_dirty: Option<Arc<AtomicBool>>,
    // The engine callback keeps the capture scratch alive until native destruction.
    _screenshot_scratch: Arc<Scratch>,
}

impl PlaybackNotification {
    pub(super) fn publish(&self, state: Arc<Playback>) {
        *lock(&self.reported_playback) = state.clone();
        self.history.publish(&self.delivery.app, &state);
        if let Some(changed) = &self.picture_in_picture_state_dirty {
            changed.store(true, Ordering::Release);
        }

        self.latest.publish(state);
        self.delivery_callback.post(|| {
            let latest = self.latest.clone();
            let window = self.delivery.clone();
            move || window.deliver(&latest)
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
            // The result has no recipient once its window or event loop closes.
            let _ = crate::background::to_ui(app.clone(), move |app| {
                crate::diagnostics::report(
                    &app,
                    format!("Could not save playback history: {error}"),
                );
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
    pub(super) transition_release_generation: Arc<AtomicU64>,
    managed_visibility: Option<ManagedVisibility>,
}

#[derive(Clone)]
pub(super) struct ManagedVisibility {
    pub(super) ticket: Arc<Ticket>,
    pub(super) native: Option<slint::Weak<NativeVideoHost>>,
}

impl ManagedVisibility {
    fn reveal(&self, app: &PlayerWindow, state: &Playback) {
        // A hidden surface can delay native video output. Reveal on the first
        // media request observed by the engine, before waiting for decoded video.
        if state.generation == 0 || state.phase == Phase::Stopped || !self.ticket.reveal_for_media()
        {
            return;
        }
        if let Some(host) = self.native.as_ref().and_then(slint::Weak::upgrade) {
            host.window()
                .with_winit_window(|window| window.set_visible(true));
        }
        if let Err(error) = crate::platform::show_window(app) {
            crate::diagnostics::report(app, error.to_string());
        } else if self.native.is_none() && self.ticket.options.disable_window_animation {
            crate::platform::window_sizing::disable_plugin_window_animation(app.window());
        }
    }
}

impl WindowDelivery {
    fn deliver(&self, latest: &Mailbox) {
        if let Some(app) = self.app.upgrade()
            && let Some(observer) = self.observer_id.lookup()
            && let Some(Delivery { state, captures }) = latest.take()
        {
            if let Some(managed) = &self.managed_visibility {
                managed.reveal(&app, &state);
            }
            let first_loaded = (!app.get_loaded()
                || observer.borrow().media_geometry.generation() != Some(state.generation))
                && matches!(
                    state.phase,
                    Phase::Playing | Phase::Paused | Phase::Buffering | Phase::Ended
                );
            let enter_fullscreen = lock(&self.opening_fullscreen).observe(
                &state,
                app.global::<riina_ui::PlayerPreferences>()
                    .get_fullscreen_when_open(),
                app.get_fullscreen(),
                false,
            );
            let clipboard_error = crate::screenshot_output::copy(&app, &captures);
            self.crop_editor.observe(&app, &state);
            let audio_device_sender = lock(&self.audio_device_sender).clone();
            crate::automatic_music_mode::observe(&app, &state, &self.music_mode_state);
            let resize_for_media = update_ui(
                &app,
                &state,
                UiUpdateResources {
                    audio_device_player: audio_device_sender.as_ref(),
                    saved_video_filters: &self.saved_video_filters,
                    saved_audio_filters: &self.saved_audio_filters,
                    transition_overlay: self.transition_overlay.as_ref(),
                    transition_release_generation: self.transition_release_generation.as_ref(),
                },
                &mut observer.borrow_mut(),
            );
            if let Some(timers) = &mut observer.borrow_mut().timers {
                timers.observe(&app, &state);
            }
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
                app.global::<PlayerMenuActions>().invoke_toggle_fullscreen();
            }
            if let Some(error) = clipboard_error {
                crate::diagnostics::report(&app, format!("Could not copy screenshot: {error}"));
            }
        }
    }
}
