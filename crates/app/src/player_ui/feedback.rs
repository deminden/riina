//! Player feedback and controller visibility share the current UI policy.
use crate::{PlayerWindow, platform};
use riina_core::Command;
use slint::ComponentHandle;
use std::{
    rc::Rc,
    sync::{Arc, Mutex},
    time::Duration,
};

pub(super) fn clear_screenshot_preview(app: &PlayerWindow) {
    app.set_screenshot_preview_visible(false);
    app.set_screenshot_preview_path("".into());
    app.set_screenshot_preview_image(slint::Image::default());
}

fn configured_osd_timeout(app: &PlayerWindow) -> Duration {
    Duration::try_from_secs_f64(f64::from(app.get_osd_timeout_secs())).unwrap_or_default()
}

#[derive(Default, Clone, Copy)]
struct ControlHide {
    enabled: bool,
    loaded: bool,
    dialog_open: bool,
    crop_editing: bool,
    title_bar_hovered: bool,
    control_bar_hovered: bool,
    control_bar_dragging: bool,
    settings_panel_open: bool,
}

impl ControlHide {
    fn allowed(self) -> bool {
        self.enabled
            && self.loaded
            && !self.dialog_open
            && !self.crop_editing
            && !self.control_bar_dragging
            && (self.settings_panel_open || (!self.title_bar_hovered && !self.control_bar_hovered))
    }
}

fn schedule_control_hide(app: &PlayerWindow, timer: &slint::Timer) {
    timer.stop();
    if !app.get_control_auto_hide_enabled()
        || !app.get_controls_visible()
        || !app.get_loaded()
        || app.get_dialog_open()
        || app.get_crop_editing()
    {
        return;
    }
    let timeout = Duration::try_from_secs_f64(f64::from(app.get_control_auto_hide_timeout_secs()))
        .unwrap_or_default();
    let weak = app.as_weak();
    // IINA's controller deadline is interaction-driven; pausing does not pin its chrome.
    timer.start(slint::TimerMode::SingleShot, timeout, move || {
        if let Some(app) = weak.upgrade() {
            // The native controls HWND can retain hover while a separate
            // Quick Settings surface covers it. Do not let that stale hover
            // keep the transport tray over the panel indefinitely.
            if (ControlHide {
                enabled: app.get_control_auto_hide_enabled(),
                loaded: app.get_loaded(),
                dialog_open: app.get_dialog_open(),
                crop_editing: app.get_crop_editing(),
                title_bar_hovered: app.get_title_bar_hovered(),
                control_bar_hovered: app.get_control_bar_hovered(),
                control_bar_dragging: app.get_control_bar_dragging(),
                settings_panel_open: app.get_sidebar() != 0 || app.get_leading_sidebar() != 0,
            })
            .allowed()
            {
                app.set_controls_visible(false);
            }
        }
    });
}

pub(super) fn install(
    app: &PlayerWindow,
    player: &riina_mpv::CommandSender,
    screenshot_scratch: Arc<crate::screenshot_output::Scratch>,
    last_video: Arc<Mutex<crate::video_osd::VideoOsd>>,
) {
    let disable_video_osd = last_video.clone();
    let pending_video_osd = last_video;
    let idle_timer = Rc::new(slint::Timer::default());
    let controls_fade_timer = Rc::new(slint::Timer::default());
    let controls_fade_parent = app.as_weak();
    let controls_fade_timeout = controls_fade_timer.clone();
    app.on_controls_visibility_changed(move |visible| {
        controls_fade_timeout.stop();
        let Some(app) = controls_fade_parent.upgrade() else {
            return;
        };
        if visible {
            app.set_controls_rendered(true);
        } else if app.get_disable_animations() || app.get_system_reduce_motion() {
            app.set_controls_rendered(false);
        } else {
            let weak = app.as_weak();
            controls_fade_timeout.start(
                slint::TimerMode::SingleShot,
                Duration::from_millis(250),
                move || {
                    if let Some(app) = weak.upgrade()
                        && !app.get_controls_visible()
                    {
                        app.set_controls_rendered(false);
                    }
                },
            );
        }
    });
    let seek_timer = Rc::new(slint::Timer::default());
    let video_timer = Rc::new(slint::Timer::default());
    let screenshot_timer = Rc::new(slint::Timer::default());
    let disable_seek_timer = seek_timer.clone();
    let disable_video_timer = video_timer.clone();
    let disable_screenshot_timer = screenshot_timer.clone();
    let weak = app.as_weak();
    app.on_osd_enabled_changed(move |enabled| {
        if enabled {
            return;
        }
        let Some(app) = weak.upgrade() else { return };
        // Disabling OSD is an immediate visibility policy, so dismiss an
        // already-visible message instead of leaving it until its old deadline.
        disable_seek_timer.stop();
        disable_video_timer.stop();
        disable_screenshot_timer.stop();
        if let Ok(mut video_osd) = disable_video_osd.lock() {
            video_osd.clear_pending();
        }
        app.set_seek_osd_visible(false);
        app.set_video_osd_visible(false);
        app.set_video_osd_request_feedback(false);
        clear_screenshot_preview(&app);
    });
    let screenshot_player = player.clone();
    let weak = app.as_weak();
    app.on_take_screenshot(move || {
        let Some(app) = weak.upgrade() else { return };
        if !app.get_screenshot_available() || app.get_crop_editing() || app.get_dialog_open() {
            return;
        }
        app.invoke_activity();
        let result = crate::screenshot_preferences::current(&app).and_then(|screenshot_settings| {
            if !screenshot_settings.destinations.enabled() {
                return Ok(());
            }
            (if screenshot_settings.destinations.save_to_file {
                platform::screenshot_directory(screenshot_settings.directory.as_deref())
            } else {
                screenshot_scratch.directory()
            })
            .map_err(|error| error.to_string())
            .and_then(|directory| {
                screenshot_player.send(Command::Screenshot {
                    directory: directory.to_string_lossy().into_owned(),
                    content: screenshot_settings.content(),
                    encoding: screenshot_settings.encoding.clone(),
                })
            })
        });
        if let Err(error) = result {
            app.set_status(error.into());
        }
    });
    let weak = app.as_weak();
    app.on_open_screenshot_folder(move || {
        let Some(app) = weak.upgrade() else { return };
        if app.get_crop_editing() || app.get_dialog_open() {
            return;
        }
        if let Err(error) = crate::screenshot_preferences::current(&app).and_then(|settings| {
            platform::screenshot_directory(settings.directory.as_deref())
                .and_then(|directory| platform::open_directory(&directory))
                .map_err(|error| error.to_string())
        }) {
            app.set_status(error.to_string().into());
        }
    });
    let weak = app.as_weak();
    app.on_show_seek_osd(move || {
        let Some(app) = weak.upgrade() else { return };
        if !app.get_osd_enabled() || app.get_disable_osd_seek() {
            return;
        }
        app.set_seek_osd_visible(true);
        clear_screenshot_preview(&app);
        app.set_video_osd_visible(false);
        let weak = weak.clone();
        seek_timer.start(
            slint::TimerMode::SingleShot,
            configured_osd_timeout(&app),
            move || {
                if let Some(app) = weak.upgrade() {
                    app.set_seek_osd_visible(false);
                }
            },
        );
    });
    let weak = app.as_weak();
    app.on_show_screenshot_preview(move || {
        let Some(app) = weak.upgrade() else { return };
        if !app.get_osd_enabled() || app.get_crop_editing() {
            return;
        }
        app.set_seek_osd_visible(false);
        app.set_video_osd_visible(false);
        app.set_screenshot_preview_visible(true);
        let weak = weak.clone();
        screenshot_timer.start(
            slint::TimerMode::SingleShot,
            Duration::from_secs(5),
            move || {
                if let Some(app) = weak.upgrade() {
                    clear_screenshot_preview(&app);
                }
            },
        );
    });
    let weak = app.as_weak();
    app.on_screenshot_preview_action(move |action| {
        let Some(app) = weak.upgrade() else { return };
        if !app.get_screenshot_preview_visible() || app.get_crop_editing() || app.get_dialog_open()
        {
            return;
        }
        let path = app.get_screenshot_preview_path();
        if path.is_empty() {
            return;
        }
        let path = std::path::Path::new(path.as_str());
        let result = match action {
            0 => {
                let _ = std::fs::remove_file(path);
                Ok(())
            }
            1 => platform::open_directory(path),
            2 => platform::reveal_file(path),
            _ => return,
        };
        clear_screenshot_preview(&app);
        if let Err(error) = result {
            app.set_status(error.to_string().into());
        }
    });
    let weak = app.as_weak();
    app.on_show_video_osd(move || {
        let Some(app) = weak.upgrade() else { return };
        if !app.get_osd_enabled() {
            return;
        }
        app.set_seek_osd_visible(false);
        clear_screenshot_preview(&app);
        app.set_video_osd_visible(true);
        let weak = weak.clone();
        let pending_video_osd = pending_video_osd.clone();
        video_timer.start(
            slint::TimerMode::SingleShot,
            configured_osd_timeout(&app),
            move || {
                let Some(app) = weak.upgrade() else { return };
                if !app.get_osd_enabled() {
                    if let Ok(mut video_osd) = pending_video_osd.lock() {
                        video_osd.clear_pending();
                    }
                    app.set_video_osd_visible(false);
                    app.set_video_osd_request_feedback(false);
                    return;
                }
                let next = pending_video_osd
                    .lock()
                    .ok()
                    .and_then(|mut video_osd| video_osd.take_pending());
                if let Some(message) = next {
                    app.set_video_osd_request_feedback(
                        message.kind == crate::video_osd::Kind::Aspect,
                    );
                    app.set_video_osd_text(message.text.into());
                    app.set_video_osd_progress(message.progress.unwrap_or(-1.0));
                    app.invoke_show_video_osd();
                } else {
                    app.set_video_osd_visible(false);
                    app.set_video_osd_request_feedback(false);
                }
            },
        );
    });
    let weak = app.as_weak();
    let activity_timer = idle_timer.clone();
    app.on_activity(move || {
        if let Some(app) = weak.upgrade()
            && !app.get_plugin_ui_disabled()
        {
            app.set_controls_visible(true);
            schedule_control_hide(&app, &activity_timer);
        }
    });
    let auto_hide_parent = app.as_weak();
    let auto_hide_timer = idle_timer.clone();
    app.on_control_bar_settings_changed(move || {
        let Some(app) = auto_hide_parent.upgrade() else {
            return;
        };
        if app.get_title_bar_hovered()
            || app.get_control_bar_hovered()
            || app.get_control_bar_dragging()
        {
            auto_hide_timer.stop();
            app.set_controls_visible(true);
        } else if app.get_control_auto_hide_enabled() {
            schedule_control_hide(&app, &auto_hide_timer);
        } else {
            auto_hide_timer.stop();
            if app.get_loaded() && !app.get_dialog_open() && !app.get_crop_editing() {
                app.set_controls_visible(true);
            }
        }
    });
    let hover_parent = app.as_weak();
    let hover_timer = idle_timer.clone();
    app.on_control_bar_hover_changed(move || {
        let Some(app) = hover_parent.upgrade() else {
            return;
        };
        if app.get_title_bar_hovered()
            || app.get_control_bar_hovered()
            || app.get_control_bar_dragging()
        {
            hover_timer.stop();
            app.set_controls_visible(true);
        } else {
            schedule_control_hide(&app, &hover_timer);
        }
    });
}

#[cfg(test)]
mod control_auto_hide_tests {
    use super::ControlHide;

    #[test]
    fn deadline_hides_only_an_idle_loaded_controller() {
        assert!(
            ControlHide {
                enabled: true,
                loaded: true,
                ..ControlHide::default()
            }
            .allowed()
        );
        let blocked_states = [
            (false, true, false, false, false, false, false),
            (true, false, false, false, false, false, false),
            (true, true, true, false, false, false, false),
            (true, true, false, true, false, false, false),
            (true, true, false, false, true, false, false),
            (true, true, false, false, false, true, false),
            (true, true, false, false, false, false, true),
        ];
        for (enabled, loaded, dialog, crop, title, controller, dragging) in blocked_states {
            assert!(
                !ControlHide {
                    enabled,
                    loaded,
                    dialog_open: dialog,
                    crop_editing: crop,
                    title_bar_hovered: title,
                    control_bar_hovered: controller,
                    control_bar_dragging: dragging,
                    settings_panel_open: false,
                }
                .allowed()
            );
        }
    }

    #[test]
    fn a_sidebar_can_override_stale_hover_but_cannot_override_visibility_guards() {
        let sidebar = ControlHide {
            enabled: true,
            loaded: true,
            settings_panel_open: true,
            title_bar_hovered: true,
            control_bar_hovered: true,
            ..ControlHide::default()
        };
        assert!(sidebar.allowed());
        for blocked in [
            ControlHide {
                enabled: false,
                ..sidebar
            },
            ControlHide {
                loaded: false,
                ..sidebar
            },
            ControlHide {
                dialog_open: true,
                ..sidebar
            },
            ControlHide {
                crop_editing: true,
                ..sidebar
            },
            ControlHide {
                control_bar_dragging: true,
                ..sidebar
            },
        ] {
            assert!(!blocked.allowed());
        }
    }
}
