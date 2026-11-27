//! Player feedback and controller visibility share the current UI policy.
use crate::{PlayerWindow, platform, player::presentation::playback_osd_allowed};
use riina_core::Command;
use riina_ui::PlayerMenuActions;
use slint::{ComponentHandle, winit_030::WinitWindowAccessor};
use std::{cell::RefCell, rc::Rc, sync::Arc, time::Duration};

pub(super) fn clear_screenshot_preview(app: &PlayerWindow) {
    let osd = app.global::<riina_ui::PlayerOsdState>();
    osd.set_screenshot_preview_visible(false);
    osd.set_screenshot_preview_path("".into());
    osd.set_screenshot_preview_image(slint::Image::default());
}

fn configured_osd_timeout(app: &PlayerWindow) -> Duration {
    Duration::try_from_secs_f64(f64::from(
        app.global::<riina_ui::PlayerPreferences>()
            .get_osd_timeout_secs(),
    ))
    .unwrap_or_default()
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
    if !app
        .global::<riina_ui::PlayerPreferences>()
        .get_control_auto_hide_enabled()
        || !app
            .global::<riina_ui::PlayerControlsState>()
            .get_controls_visible()
        || !app.get_loaded()
        || app
            .global::<riina_ui::PlayerDialogState>()
            .get_dialog_open()
        || app.global::<riina_ui::PlayerCropState>().get_crop_editing()
    {
        return;
    }
    let timeout = Duration::try_from_secs_f64(f64::from(
        app.global::<riina_ui::PlayerPreferences>()
            .get_control_auto_hide_timeout_secs(),
    ))
    .unwrap_or_default();
    let weak = app.as_weak();
    // IINA's controller deadline is interaction-driven; pausing does not pin its chrome.
    timer.start(slint::TimerMode::SingleShot, timeout, move || {
        if let Some(app) = weak.upgrade() {
            // The native controls HWND can retain hover while a separate
            // Quick Settings surface covers it. Do not let that stale hover
            // keep the transport tray over the panel indefinitely.
            if (ControlHide {
                enabled: app
                    .global::<riina_ui::PlayerPreferences>()
                    .get_control_auto_hide_enabled(),
                loaded: app.get_loaded(),
                dialog_open: app
                    .global::<riina_ui::PlayerDialogState>()
                    .get_dialog_open(),
                crop_editing: app.global::<riina_ui::PlayerCropState>().get_crop_editing(),
                title_bar_hovered: app
                    .global::<riina_ui::PlayerControlsState>()
                    .get_title_bar_hovered(),
                control_bar_hovered: app
                    .global::<riina_ui::PlayerControlsState>()
                    .get_control_bar_hovered(),
                control_bar_dragging: app
                    .global::<riina_ui::PlayerControlsState>()
                    .get_control_bar_interaction_held(),
                settings_panel_open: app.global::<riina_ui::PlayerSidebarState>().get_sidebar()
                    != 0
                    || app
                        .global::<riina_ui::PlayerSidebarState>()
                        .get_leading_sidebar()
                        != 0,
            })
            .allowed()
            {
                app.global::<riina_ui::PlayerControlsState>()
                    .set_controls_visible(false);
            }
        }
    });
}

pub(super) fn install(
    app: &PlayerWindow,
    player: &riina_mpv::CommandSender,
    screenshot_scratch: Arc<crate::screenshot_output::Scratch>,
    last_video: Rc<RefCell<crate::video_osd::VideoOsd>>,
) {
    install_controls_fade(app);
    let seek_timer = Rc::new(slint::Timer::default());
    let video_timer = Rc::new(slint::Timer::default());
    let screenshot_timer = Rc::new(slint::Timer::default());
    install_osd_policy(
        app,
        &seek_timer,
        &video_timer,
        &screenshot_timer,
        last_video.clone(),
    );
    install_screenshot_actions(app, player, screenshot_scratch);
    install_seek_osd(app, seek_timer);
    install_screenshot_preview(app, screenshot_timer);
    install_video_osd(app, video_timer, last_video);
    install_controls_deadline(app);
}

fn install_controls_fade(app: &PlayerWindow) {
    let controls_fade_timer = Rc::new(slint::Timer::default());
    let controls_fade_parent = app.as_weak();
    let controls_fade_timeout = controls_fade_timer.clone();
    app.on_controls_visibility_changed(move |visible| {
        controls_fade_timeout.stop();
        let Some(app) = controls_fade_parent.upgrade() else {
            return;
        };
        // The pointer follows the same interaction-driven deadline as the
        // controller. Movement shows the controls and restores the pointer.
        app.window().with_winit_window(|window| {
            window.set_cursor_visible(
                visible
                    || app.global::<riina_ui::PlayerSidebarState>().get_sidebar() != 0
                    || app
                        .global::<riina_ui::PlayerSidebarState>()
                        .get_leading_sidebar()
                        != 0,
            );
        });
        if visible {
            app.global::<riina_ui::PlayerControlsState>()
                .set_controls_rendered(true);
        } else if app
            .global::<riina_ui::PlayerPreferences>()
            .get_disable_animations()
            || app
                .global::<riina_ui::PlayerAppearance>()
                .get_system_reduce_motion()
        {
            app.global::<riina_ui::PlayerControlsState>()
                .set_controls_rendered(false);
        } else {
            let weak = app.as_weak();
            controls_fade_timeout.start(
                slint::TimerMode::SingleShot,
                Duration::from_millis(250),
                move || {
                    if let Some(app) = weak.upgrade()
                        && !app
                            .global::<riina_ui::PlayerControlsState>()
                            .get_controls_visible()
                    {
                        app.global::<riina_ui::PlayerControlsState>()
                            .set_controls_rendered(false);
                    }
                },
            );
        }
    });
}

fn install_osd_policy(
    app: &PlayerWindow,
    seek_timer: &Rc<slint::Timer>,
    video_timer: &Rc<slint::Timer>,
    screenshot_timer: &Rc<slint::Timer>,
    disable_video_osd: Rc<RefCell<crate::video_osd::VideoOsd>>,
) {
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
        disable_screenshot_timer.stop();
        disable_video_osd.borrow_mut().clear_pending();
        let osd = app.global::<riina_ui::PlayerOsdState>();
        osd.set_seek_osd_visible(false);
        if !osd.get_video_osd_required()
            || app
                .global::<riina_ui::PlayerMusicModeState>()
                .get_music_mode()
        {
            disable_video_timer.stop();
            osd.set_video_osd_visible(false);
            osd.set_video_osd_required(false);
            app.set_video_osd_request_feedback(false);
        }
        clear_screenshot_preview(&app);
    });
}

fn install_screenshot_actions(
    app: &PlayerWindow,
    player: &riina_mpv::CommandSender,
    screenshot_scratch: Arc<crate::screenshot_output::Scratch>,
) {
    let screenshot_player = player.clone();
    let weak = app.as_weak();
    let capture = move |binding: Option<riina_core::screenshot::Binding>| {
        let Some(app) = weak.upgrade() else { return };
        if !app.get_screenshot_available()
            || app.global::<riina_ui::PlayerCropState>().get_crop_editing()
            || app
                .global::<riina_ui::PlayerDialogState>()
                .get_dialog_open()
        {
            return;
        }
        app.global::<riina_ui::PlayerControlActions>()
            .invoke_activity();
        let result = crate::screenshot_preferences::current(&app).and_then(|screenshot_settings| {
            if !screenshot_settings.destinations.enabled() {
                return Ok(());
            }
            let content = screenshot_settings.content();
            let mut encoding = screenshot_settings.encoding;
            let directory = if screenshot_settings.destinations.save_to_file {
                platform::screenshot_directory(screenshot_settings.directory.as_deref())
            } else {
                // An absolute template can bypass mpv's destination directory.
                encoding.template = "%n".into();
                screenshot_scratch.directory()
            }
            .map_err(|error| error.to_string())?
            .into_os_string()
            .into_string()
            .map_err(|_| "The screenshot directory path is not valid Unicode")?;
            let command = match binding {
                Some(binding) => Command::ScreenshotBinding {
                    directory,
                    default_content: content,
                    encoding,
                    binding,
                },
                None => Command::Screenshot {
                    directory,
                    content,
                    encoding,
                },
            };
            screenshot_player.send(command).map_err(String::from)
        });
        if let Err(error) = result {
            app.set_status(error.into());
        }
    };
    let menu_capture = capture.clone();
    app.global::<PlayerMenuActions>()
        .on_take_screenshot(move || menu_capture(None));
    app.global::<riina_ui::PlayerControlActions>()
        .on_take_key_screenshot(move |action| {
            if let Some(binding) = riina_core::screenshot::Binding::parse(action.as_str()) {
                capture(Some(binding));
            }
        });
    let weak = app.as_weak();
    app.global::<PlayerMenuActions>()
        .on_open_screenshot_folder(move || {
            let Some(app) = weak.upgrade() else { return };
            if app.global::<riina_ui::PlayerCropState>().get_crop_editing()
                || app
                    .global::<riina_ui::PlayerDialogState>()
                    .get_dialog_open()
            {
                return;
            }
            if let Err(error) = crate::screenshot_preferences::current(&app).and_then(|settings| {
                platform::open_screenshot_directory(settings.directory.as_deref())
                    .map_err(|error| error.to_string())
            }) {
                app.set_status(error.to_string().into());
            }
        });
}

fn install_seek_osd(app: &PlayerWindow, seek_timer: Rc<slint::Timer>) {
    let weak = app.as_weak();
    app.global::<riina_ui::PlayerControlActions>()
        .on_show_seek_osd(move || {
            let Some(app) = weak.upgrade() else { return };
            if !playback_osd_allowed(&app)
                || app
                    .global::<riina_ui::PlayerPreferences>()
                    .get_disable_osd_seek()
            {
                return;
            }
            app.global::<riina_ui::PlayerOsdState>()
                .set_seek_osd_visible(true);
            clear_screenshot_preview(&app);
            app.global::<riina_ui::PlayerOsdState>()
                .set_video_osd_visible(false);
            let weak = weak.clone();
            seek_timer.start(
                slint::TimerMode::SingleShot,
                configured_osd_timeout(&app),
                move || {
                    if let Some(app) = weak.upgrade() {
                        app.global::<riina_ui::PlayerOsdState>()
                            .set_seek_osd_visible(false);
                    }
                },
            );
        });
}

fn install_screenshot_preview(app: &PlayerWindow, screenshot_timer: Rc<slint::Timer>) {
    let weak = app.as_weak();
    app.on_show_screenshot_preview(move || {
        let Some(app) = weak.upgrade() else { return };
        if !playback_osd_allowed(&app)
            || app.global::<riina_ui::PlayerCropState>().get_crop_editing()
        {
            return;
        }
        app.global::<riina_ui::PlayerOsdState>()
            .set_seek_osd_visible(false);
        app.global::<riina_ui::PlayerOsdState>()
            .set_video_osd_visible(false);
        app.global::<riina_ui::PlayerOsdState>()
            .set_screenshot_preview_visible(true);
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
        if !app
            .global::<riina_ui::PlayerOsdState>()
            .get_screenshot_preview_visible()
            || app.global::<riina_ui::PlayerCropState>().get_crop_editing()
            || app
                .global::<riina_ui::PlayerDialogState>()
                .get_dialog_open()
        {
            return;
        }
        let path = app
            .global::<riina_ui::PlayerOsdState>()
            .get_screenshot_preview_path();
        if path.is_empty() {
            return;
        }
        let path = std::path::Path::new(path.as_str());
        let result = match action {
            0 => {
                if let Err(error) = std::fs::remove_file(path)
                    && error.kind() != std::io::ErrorKind::NotFound
                {
                    crate::diagnostics::warn!("Could not delete the previewed screenshot: {error}");
                }
                Ok(())
            }
            1 => platform::open_directory(path),
            2 => {
                let path = path.to_path_buf();
                let weak = app.as_weak();
                crate::background::spawn_named("riina-screenshot-reveal", move || {
                    if let Err(error) = platform::reveal_file(&path) {
                        let _ = crate::background::to_ui(weak, move |app| {
                            crate::diagnostics::report(&app, error.to_string());
                        });
                    }
                })
                .map(|_| ())
            }
            _ => return,
        };
        clear_screenshot_preview(&app);
        if let Err(error) = result {
            crate::diagnostics::report(&app, error.to_string());
        }
    });
}

fn install_video_osd(
    app: &PlayerWindow,
    video_timer: Rc<slint::Timer>,
    pending_video_osd: Rc<RefCell<crate::video_osd::VideoOsd>>,
) {
    let weak = app.as_weak();
    app.global::<riina_ui::PlayerControlActions>()
        .on_show_video_osd(move || {
            let Some(app) = weak.upgrade() else { return };
            let osd = app.global::<riina_ui::PlayerOsdState>();
            if !playback_osd_allowed(&app) && !osd.get_video_osd_required() {
                return;
            }
            osd.set_seek_osd_visible(false);
            clear_screenshot_preview(&app);
            osd.set_video_osd_visible(true);
            let weak = weak.clone();
            let pending_video_osd = pending_video_osd.clone();
            video_timer.start(
                slint::TimerMode::SingleShot,
                configured_osd_timeout(&app),
                move || {
                    let Some(app) = weak.upgrade() else { return };
                    let osd = app.global::<riina_ui::PlayerOsdState>();
                    if !playback_osd_allowed(&app) {
                        pending_video_osd.borrow_mut().clear_pending();
                        osd.set_video_osd_visible(false);
                        osd.set_video_osd_required(false);
                        app.set_video_osd_request_feedback(false);
                        return;
                    }
                    let next = pending_video_osd.borrow_mut().take_pending();
                    if let Some(message) = next {
                        message.present(&app);
                    } else {
                        osd.set_video_osd_visible(false);
                        osd.set_video_osd_required(false);
                        app.set_video_osd_request_feedback(false);
                    }
                },
            );
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use riina_ui::{PlayerOsdState, PlayerPreferences};

    #[test]
    fn saved_subtitle_feedback_survives_disabled_osd_until_its_deadline()
    -> Result<(), slint::PlatformError> {
        i_slint_backend_testing::init_no_event_loop();
        let app = PlayerWindow::new()?;
        app.set_loaded(true);
        let preferences = app.global::<PlayerPreferences>();
        preferences.set_osd_timeout_secs(1.0);
        preferences.set_osd_enabled(false);
        let timer = Rc::new(slint::Timer::default());
        let pending = Rc::new(RefCell::new(crate::video_osd::VideoOsd::default()));
        install_osd_policy(
            &app,
            &Rc::new(slint::Timer::default()),
            &timer,
            &Rc::new(slint::Timer::default()),
            pending.clone(),
        );
        install_video_osd(&app, timer, pending);
        let osd = app.global::<PlayerOsdState>();

        super::super::show_osd_message(&app, "Optional", false);
        assert!(!osd.get_video_osd_visible());
        super::super::show_subtitle_saved(&app);
        assert!(osd.get_video_osd_visible());
        assert!(osd.get_video_osd_required());
        assert_eq!(osd.get_video_osd_text(), "Subtitle saved");
        crate::video_osd::progress_message(Some(1.0), Some(5.0))
            .unwrap()
            .present(&app);
        assert_eq!(osd.get_video_osd_text(), "Subtitle saved");

        i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(1100));
        assert!(!osd.get_video_osd_visible());
        assert!(!osd.get_video_osd_required());
        super::super::show_subtitle_saved(&app);
        crate::player::presentation::set_music_mode(&app, true);
        assert!(!osd.get_video_osd_visible());
        Ok(())
    }
}

fn install_controls_deadline(app: &PlayerWindow) {
    let idle_timer = Rc::new(slint::Timer::default());
    let weak = app.as_weak();
    let activity_timer = idle_timer.clone();
    app.global::<riina_ui::PlayerControlActions>()
        .on_activity(move || {
            if let Some(app) = weak.upgrade()
                && !app
                    .global::<riina_ui::PlayerControlsState>()
                    .get_plugin_ui_disabled()
            {
                app.global::<riina_ui::PlayerControlsState>()
                    .set_controls_visible(true);
                schedule_control_hide(&app, &activity_timer);
            }
        });
    let auto_hide_parent = app.as_weak();
    let auto_hide_timer = idle_timer.clone();
    app.global::<riina_ui::PlayerPreferences>()
        .on_control_bar_settings_changed(move || {
            let Some(app) = auto_hide_parent.upgrade() else {
                return;
            };
            if app
                .global::<riina_ui::PlayerControlsState>()
                .get_title_bar_hovered()
                || app
                    .global::<riina_ui::PlayerControlsState>()
                    .get_control_bar_hovered()
                || app
                    .global::<riina_ui::PlayerControlsState>()
                    .get_control_bar_interaction_held()
            {
                auto_hide_timer.stop();
                app.global::<riina_ui::PlayerControlsState>()
                    .set_controls_visible(true);
            } else if app
                .global::<riina_ui::PlayerPreferences>()
                .get_control_auto_hide_enabled()
            {
                schedule_control_hide(&app, &auto_hide_timer);
            } else {
                auto_hide_timer.stop();
                if app.get_loaded()
                    && !app
                        .global::<riina_ui::PlayerDialogState>()
                        .get_dialog_open()
                    && !app.global::<riina_ui::PlayerCropState>().get_crop_editing()
                {
                    app.global::<riina_ui::PlayerControlsState>()
                        .set_controls_visible(true);
                }
            }
        });
    let hover_parent = app.as_weak();
    let hover_timer = idle_timer.clone();
    app.global::<riina_ui::PlayerControlsState>()
        .on_control_bar_hover_changed(move || {
            let Some(app) = hover_parent.upgrade() else {
                return;
            };
            if app
                .global::<riina_ui::PlayerControlsState>()
                .get_title_bar_hovered()
                || app
                    .global::<riina_ui::PlayerControlsState>()
                    .get_control_bar_hovered()
                || app
                    .global::<riina_ui::PlayerControlsState>()
                    .get_control_bar_interaction_held()
            {
                hover_timer.stop();
                app.global::<riina_ui::PlayerControlsState>()
                    .set_controls_visible(true);
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
