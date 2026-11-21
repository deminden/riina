//! Connect the timeline to its isolated thumbnail decoder and sampler.
use crate::{PlayerWindow, platform, plugin_host::PluginHostHandle};
use riina_core::sync::lock;
use slint::ComponentHandle;
use std::{cell::RefCell, rc::Rc, time::Duration};

pub(super) fn install(
    app: &PlayerWindow,
    latest_playback: &crate::PlaybackStore,
    plugin_host: Option<PluginHostHandle>,
) -> Rc<slint::Timer> {
    let thumbnail_preview_timer = Rc::new(slint::Timer::default());
    let thumbnail_controller =
        Rc::new(RefCell::new(crate::thumbnail_preview::Controller::default()));
    let thumbnail_hover = thumbnail_controller.clone();
    let thumbnail_weak = app.as_weak();
    app.on_timeline_hovered(move |seconds| {
        if let Some(app) = thumbnail_weak.upgrade() {
            thumbnail_hover.borrow_mut().hover(&app, f64::from(seconds));
        }
    });
    let thumbnail_weak = app.as_weak();
    let thumbnail_playback = latest_playback.clone();
    let thumbnail_tick = thumbnail_controller.clone();
    let thumbnail_library = platform::playback_library();
    let thumbnail_plugin_host = plugin_host;
    thumbnail_preview_timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(80),
        move || {
            let Some(app) = thumbnail_weak.upgrade() else {
                return;
            };
            let Ok(settings) = crate::thumbnail_preferences::current_player(&app) else {
                return;
            };
            let state = lock(&thumbnail_playback).clone();
            let ready =
                thumbnail_tick
                    .borrow_mut()
                    .observe(&app, &state, &settings, &thumbnail_library);
            if ready && let Some(host) = &thumbnail_plugin_host {
                let _ = host.notify_thumbnails_ready();
            }
        },
    );
    thumbnail_preview_timer
}
