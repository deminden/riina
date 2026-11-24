//! Connect the timeline to its isolated thumbnail decoder and sampler.
use crate::{PlayerWindow, platform, plugin_host::PluginHostHandle};
use riina_core::sync::lock;
use riina_ui::PlayerPreferences;
use slint::ComponentHandle;
use std::{cell::RefCell, rc::Rc, time::Duration};

pub(super) struct Installed {
    pub(super) timer: Rc<slint::Timer>,
    pub(super) touch_bar_samples: platform::player_touch_bar::Samples,
}

pub(super) fn install(
    app: &PlayerWindow,
    latest_playback: &crate::PlaybackStore,
    plugin_host: Option<PluginHostHandle>,
) -> Installed {
    let thumbnail_preview_timer = Rc::new(slint::Timer::default());
    let thumbnail_controller =
        Rc::new(RefCell::new(crate::thumbnail_preview::Controller::default()));
    let touch_bar_samples = platform::player_touch_bar::Samples::new(&thumbnail_controller);
    let sample_observer = touch_bar_samples.clone();
    let thumbnail_hover = thumbnail_controller.clone();
    let thumbnail_weak = app.as_weak();
    app.global::<riina_ui::PlayerControlActions>()
        .on_timeline_hovered(move |seconds| {
            if let Some(app) = thumbnail_weak.upgrade() {
                thumbnail_hover.borrow_mut().hover(&app, f64::from(seconds));
            }
        });
    let thumbnail_weak = app.as_weak();
    let thumbnail_playback = latest_playback.clone();
    let thumbnail_tick = thumbnail_controller.clone();
    let thumbnail_library = platform::playback_library();
    let thumbnail_plugin_host = plugin_host;
    let weak_timer = Rc::downgrade(&thumbnail_preview_timer);
    let settings_timer = weak_timer.clone();
    app.global::<PlayerPreferences>()
        .on_thumbnail_settings_changed(move || {
            if let Some(timer) = settings_timer.upgrade() {
                super::features::wake_timer(&timer);
            }
        });
    thumbnail_preview_timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(80),
        move || {
            let Some(app) = thumbnail_weak.upgrade() else {
                return;
            };
            let mut controller = thumbnail_tick.borrow_mut();
            let previous = controller.sample_revision();
            let ready = crate::thumbnail_preferences::current_player(&app).is_ok_and(|settings| {
                let state = lock(&thumbnail_playback).clone();
                controller.observe(&app, &state, &settings, &thumbnail_library)
            });
            if !controller.is_extracting()
                && let Some(timer) = weak_timer.upgrade()
            {
                timer.stop();
            }
            let changed = previous != controller.sample_revision();
            drop(controller);
            if changed {
                sample_observer.changed();
            }
            if ready
                && let Some(host) = &thumbnail_plugin_host
                && let Err(error) = host.notify_thumbnails_ready()
            {
                crate::diagnostics::warn!(
                    "Could not notify plugins that thumbnails are ready: {error}"
                );
            }
        },
    );
    Installed {
        timer: thumbnail_preview_timer,
        touch_bar_samples,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use riina_core::thumbnail_preferences::Settings;
    use riina_ui::PlaybackTimelineState;

    #[test]
    fn idle_thumbnails_only_wake_for_a_settings_update() -> Result<(), slint::PlatformError> {
        i_slint_backend_testing::init_no_event_loop();
        let app = PlayerWindow::new()?;
        let timer = install(&app, &crate::PlaybackStore::default(), None).timer;
        let interval = Duration::from_millis(80);
        i_slint_backend_testing::mock_elapsed_time(interval);
        assert!(!timer.running());

        crate::thumbnail_preferences::install_player(
            &app,
            &Settings {
                width: 240,
                ..Default::default()
            },
        );
        assert!(timer.running());
        let timeline = app.global::<PlaybackTimelineState>();
        assert_eq!(timeline.get_thumbnail_preview_width(), 120.0);
        i_slint_backend_testing::mock_elapsed_time(interval);
        assert_eq!(timeline.get_thumbnail_preview_width(), 240.0);
        assert!(!timer.running());
        Ok(())
    }
}
