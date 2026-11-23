//! Source identity, user intent and display capability jointly own HDR output.
use crate::PlayerWindow;
use riina_core::Command;
use slint::ComponentHandle;
use std::{cell::Cell, rc::Rc};

pub(super) fn install(app: &PlayerWindow, send: impl Fn(Command) -> Result<(), String> + 'static) {
    let send = Rc::new(send);
    let requested = Rc::new(Cell::new(
        app.global::<riina_ui::PlayerPreferences>()
            .get_default_hdr_mode_enabled(),
    ));
    let request = requested.clone();
    let player = send.clone();
    let weak = app.as_weak();
    app.on_refresh_hdr_output(move || {
        if let Some(app) = weak.upgrade() {
            apply(&app, request.get(), player.as_ref());
        }
    });
    let request = requested.clone();
    let weak = app.as_weak();
    app.on_reset_hdr_output(move || {
        if let Some(app) = weak.upgrade() {
            request.set(
                app.global::<riina_ui::PlayerPreferences>()
                    .get_default_hdr_mode_enabled(),
            );
            app.invoke_refresh_hdr_output();
        }
    });
    let weak = app.as_weak();
    app.global::<riina_ui::PlayerSidebarActions>()
        .on_set_hdr_enabled(move |enabled| {
            let Some(app) = weak.upgrade() else { return };
            let video = app.global::<riina_ui::PlaybackVideoState>();
            if !video.get_hdr_source_detected() || (enabled && !video.get_hdr_display_active()) {
                app.set_status("HDR needs an HDR source and an HDR-enabled display".into());
                return;
            }
            if apply(&app, enabled, send.as_ref()) {
                requested.set(enabled);
                app.global::<riina_ui::PlayerControlActions>()
                    .invoke_activity();
            }
        });
}

fn apply(
    app: &PlayerWindow,
    requested: bool,
    send: &impl Fn(Command) -> Result<(), String>,
) -> bool {
    let video = app.global::<riina_ui::PlaybackVideoState>();
    let enabled = requested && video.get_hdr_source_detected() && video.get_hdr_display_active();
    match send(Command::SetHdrEnabledWithIcc {
        enabled,
        load_icc_profile: crate::platform::video_materials::use_display_icc_profile(
            app.global::<riina_ui::PlayerPreferences>()
                .get_load_icc_profile(),
            video.get_hdr_source_detected(),
            app.get_native_video(),
        ),
    }) {
        Ok(()) => {
            video.set_hdr_enabled(enabled);
            true
        }
        Err(error) => {
            crate::diagnostics::report(
                app,
                format!("Could not apply the HDR output preference: {error}"),
            );
            false
        }
    }
}

pub(super) fn initialize_display(
    app: &PlayerWindow,
    prefs: &riina_core::Preferences,
    native: Option<&crate::platform::NativePresentation>,
) {
    let video = app.global::<riina_ui::PlaybackVideoState>();
    video.set_hdr_enabled(prefs.hdr_enabled);
    app.global::<riina_ui::PlayerPreferences>()
        .set_default_hdr_mode_enabled(prefs.hdr_enabled);
    app.global::<riina_ui::PlayerPreferences>()
        .set_load_icc_profile(prefs.load_icc_profile);
    video.set_hdr_display_active(
        native.is_some_and(|presentation| presentation.hdr_display_active()),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::update_hdr_display;
    use std::cell::RefCell;

    #[test]
    #[cfg(target_os = "windows")]
    fn native_windows_hdr_conversion_keeps_the_swapchain_target_instead_of_an_sdr_profile() {
        i_slint_backend_testing::init_no_event_loop();
        let app = PlayerWindow::new().unwrap();
        app.global::<riina_ui::PlayerPreferences>()
            .set_load_icc_profile(true);
        app.set_native_video(true);
        let video = app.global::<riina_ui::PlaybackVideoState>();
        video.set_hdr_source_detected(true);
        let commands = Rc::new(RefCell::new(Vec::new()));
        let observed = commands.clone();
        install(&app, move |command| {
            observed.borrow_mut().push(command);
            Ok(())
        });
        // HDR-to-SDR conversion uses the native output target. Opening SDR
        // media restores the user's profile; shared rendering also retains it.
        app.invoke_reset_hdr_output();
        video.set_hdr_source_detected(false);
        app.invoke_reset_hdr_output();
        video.set_hdr_source_detected(true);
        app.set_native_video(false);
        app.invoke_reset_hdr_output();
        let profiles: Vec<_> = commands
            .borrow()
            .iter()
            .map(|command| match command {
                Command::SetHdrEnabledWithIcc {
                    enabled,
                    load_icc_profile,
                } => {
                    assert!(!enabled, "SDR desktop cannot enable HDR output");
                    *load_icc_profile
                }
                _ => panic!("Unexpected output command"),
            })
            .collect();
        assert_eq!(profiles, [false, true, true]);
        assert!(
            app.global::<riina_ui::PlayerPreferences>()
                .get_load_icc_profile(),
            "The saved preference is retained"
        );
    }

    #[test]
    fn paused_hdr_video_resets_output_when_desktop_changes_to_sdr_and_restores_intent() {
        i_slint_backend_testing::init_no_event_loop();
        let app = PlayerWindow::new().unwrap();
        app.global::<riina_ui::PlayerPreferences>()
            .set_default_hdr_mode_enabled(true);
        let video = app.global::<riina_ui::PlaybackVideoState>();
        video.set_hdr_source_detected(true);
        app.set_paused(true);
        app.global::<riina_ui::PlayerPreferences>()
            .set_load_icc_profile(false);
        let commands = Rc::new(RefCell::new(Vec::new()));
        let observed = commands.clone();
        install(&app, move |command| {
            observed.borrow_mut().push(command);
            Ok(())
        });
        update_hdr_display(&app, true);
        assert!(video.get_hdr_enabled());
        update_hdr_display(&app, false);
        assert!(!video.get_hdr_enabled());
        update_hdr_display(&app, false);
        update_hdr_display(&app, true);
        let modes: Vec<_> = commands
            .borrow()
            .iter()
            .map(|command| match command {
                Command::SetHdrEnabledWithIcc {
                    enabled,
                    load_icc_profile,
                } => {
                    assert!(!load_icc_profile);
                    *enabled
                }
                _ => panic!("Unexpected command"),
            })
            .collect();
        assert_eq!(modes, [true, false, true]);
    }

    #[test]
    fn manual_disable_survives_display_changes_until_the_next_file() {
        i_slint_backend_testing::init_no_event_loop();
        let app = PlayerWindow::new().unwrap();
        app.global::<riina_ui::PlayerPreferences>()
            .set_default_hdr_mode_enabled(true);
        let video = app.global::<riina_ui::PlaybackVideoState>();
        video.set_hdr_source_detected(true);
        install(&app, |_| Ok(()));
        update_hdr_display(&app, true);
        app.global::<riina_ui::PlayerSidebarActions>()
            .invoke_set_hdr_enabled(false);
        update_hdr_display(&app, false);
        update_hdr_display(&app, true);
        assert!(!video.get_hdr_enabled());
        app.invoke_reset_hdr_output();
        assert!(video.get_hdr_enabled());
        video.set_hdr_source_detected(false);
        app.invoke_refresh_hdr_output();
        assert!(!video.get_hdr_enabled());
    }

    #[test]
    fn sdr_display_cannot_enable_hdr_and_failed_updates_keep_confirmed_state() {
        i_slint_backend_testing::init_no_event_loop();
        let app = PlayerWindow::new().unwrap();
        let video = app.global::<riina_ui::PlaybackVideoState>();
        video.set_hdr_enabled(false);
        video.set_hdr_source_detected(true);
        app.global::<riina_ui::PlayerPreferences>()
            .set_default_hdr_mode_enabled(true);
        install(&app, |_| Err("engine closed".into()));
        app.global::<riina_ui::PlayerSidebarActions>()
            .invoke_set_hdr_enabled(true);
        assert!(!video.get_hdr_enabled());
        update_hdr_display(&app, true);
        assert!(!video.get_hdr_enabled());
        assert!(app.get_status().contains("engine closed"));
    }
}
