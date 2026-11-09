//! Source identity, user intent and display capability jointly own HDR output.
use crate::PlayerWindow;
use riina_core::Command;
use slint::ComponentHandle;
use std::{cell::Cell, rc::Rc};

pub(super) fn install(app: &PlayerWindow, send: impl Fn(Command) -> Result<(), String> + 'static) {
    let send = Rc::new(send);
    let requested = Rc::new(Cell::new(app.get_default_hdr_mode_enabled()));
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
            request.set(app.get_default_hdr_mode_enabled());
            app.invoke_refresh_hdr_output();
        }
    });
    let weak = app.as_weak();
    app.on_set_hdr_enabled(move |enabled| {
        let Some(app) = weak.upgrade() else { return };
        if !app.get_hdr_source_detected() || (enabled && !app.get_hdr_display_active()) {
            app.set_status("HDR needs an HDR source and an HDR-enabled display".into());
            return;
        }
        if apply(&app, enabled, send.as_ref()) {
            requested.set(enabled);
            app.invoke_activity();
        }
    });
}

fn apply(
    app: &PlayerWindow,
    requested: bool,
    send: &impl Fn(Command) -> Result<(), String>,
) -> bool {
    let enabled = requested && app.get_hdr_source_detected() && app.get_hdr_display_active();
    match send(Command::SetHdrEnabledWithIcc {
        enabled,
        load_icc_profile: app.get_load_icc_profile(),
    }) {
        Ok(()) => {
            app.set_hdr_enabled(enabled);
            true
        }
        Err(error) => {
            app.set_status(format!("Could not apply the HDR output preference: {error}").into());
            false
        }
    }
}

pub(crate) fn update_display(app: &PlayerWindow, active: bool) {
    if app.get_hdr_display_active() != active {
        app.set_hdr_display_active(active);
        // Display observations arrive independently of playback snapshots,
        // including while paused or after switching Windows HDR off.
        app.invoke_refresh_hdr_output();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[test]
    fn paused_hdr_video_resets_output_when_desktop_changes_to_sdr_and_restores_intent() {
        i_slint_backend_testing::init_no_event_loop();
        let app = PlayerWindow::new().unwrap();
        app.set_default_hdr_mode_enabled(true);
        app.set_hdr_source_detected(true);
        app.set_paused(true);
        app.set_load_icc_profile(false);
        let commands = Rc::new(RefCell::new(Vec::new()));
        let observed = commands.clone();
        install(&app, move |command| {
            observed.borrow_mut().push(command);
            Ok(())
        });
        update_display(&app, true);
        assert!(app.get_hdr_enabled());
        update_display(&app, false);
        assert!(!app.get_hdr_enabled());
        update_display(&app, false);
        update_display(&app, true);
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
        app.set_default_hdr_mode_enabled(true);
        app.set_hdr_source_detected(true);
        install(&app, |_| Ok(()));
        update_display(&app, true);
        app.invoke_set_hdr_enabled(false);
        update_display(&app, false);
        update_display(&app, true);
        assert!(!app.get_hdr_enabled());
        app.invoke_reset_hdr_output();
        assert!(app.get_hdr_enabled());
        app.set_hdr_source_detected(false);
        app.invoke_refresh_hdr_output();
        assert!(!app.get_hdr_enabled());
    }

    #[test]
    fn sdr_display_cannot_enable_hdr_and_failed_updates_keep_confirmed_state() {
        i_slint_backend_testing::init_no_event_loop();
        let app = PlayerWindow::new().unwrap();
        app.set_hdr_enabled(false);
        app.set_hdr_source_detected(true);
        app.set_default_hdr_mode_enabled(true);
        install(&app, |_| Err("engine closed".into()));
        app.invoke_set_hdr_enabled(true);
        assert!(!app.get_hdr_enabled());
        update_display(&app, true);
        assert!(!app.get_hdr_enabled());
        assert!(app.get_status().contains("engine closed"));
    }
}
