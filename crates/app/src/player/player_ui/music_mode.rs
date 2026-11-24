//! Integrate Music Mode commands and compact window sizing with the player.
use super::window_actions::request_fullscreen;
use crate::{NativeVideoHost, PlayerWindow, music_mode_layout, window_aspect};
use riina_core::{music_mode::State, sync::lock, window_aspect::PixelSize};
use riina_ui::PlayerMenuActions;
use slint::{ComponentHandle, winit_030::WinitWindowAccessor};
use std::{
    cell::Cell,
    rc::Rc,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

pub(super) fn install_window_resize(app: &PlayerWindow) {
    let saved_frame = Cell::new(None::<music_mode_layout::MainWindowFrame>);
    let weak = app.as_weak();
    app.on_music_mode_changed(move || {
        if let Some(app) = weak.upgrade() {
            let window = app.window();
            let (target, restore) = if app
                .global::<riina_ui::PlayerMusicModeState>()
                .get_music_mode()
            {
                if saved_frame.get().is_none() {
                    saved_frame.set(Some(music_mode_layout::MainWindowFrame::leave(window)));
                }
                (
                    music_mode_layout::compact_size(&app, f64::from(window.scale_factor())),
                    None,
                )
            } else if let Some(frame) = saved_frame.take() {
                (frame.size, Some(frame))
            } else {
                return;
            };
            let current = music_mode_layout::client_size(window);
            window_aspect::resize_centered(
                window,
                PixelSize {
                    width: current.width,
                    height: current.height,
                },
                PixelSize {
                    width: target.width,
                    height: target.height,
                },
                None,
            );
            if let Some(frame) = restore {
                frame.restore_placement(window);
            }
        }
    });
}

pub(super) fn install_actions(
    app: &PlayerWindow,
    state: &Arc<Mutex<State>>,
    host: Option<slint::Weak<NativeVideoHost>>,
) {
    let weak = app.as_weak();
    let manual_music_mode = state.clone();
    let wait = Rc::new(slint::Timer::default());
    app.global::<PlayerMenuActions>()
        .on_toggle_music_mode(move || {
            let Some(app) = weak.upgrade() else { return };
            if wait.running() {
                wait.stop();
                return;
            }
            let entering = !lock(&manual_music_mode).active();
            if entering && (app.get_fullscreen() || native_fullscreen(&app, host.as_ref())) {
                if !request_fullscreen(&app, host.as_ref(), false) {
                    return;
                }
                if native_fullscreen(&app, host.as_ref()) {
                    after_fullscreen_exit(&wait, &app, host.clone(), manual_music_mode.clone());
                    return;
                }
            }
            toggle_manually(&app, &manual_music_mode);
        });
    let automatic_music_mode = state.clone();
    app.on_auto_switch_music_mode_changed(move |enabled| {
        lock(&automatic_music_mode).set_auto_switch(enabled);
    });
}

fn toggle_manually(app: &PlayerWindow, state: &Mutex<State>) {
    let active = lock(state).toggle_manually();
    app.global::<riina_ui::PlayerMusicModeState>()
        .set_music_mode(active);
}

fn native_fullscreen(app: &PlayerWindow, host: Option<&slint::Weak<NativeVideoHost>>) -> bool {
    let host = host.and_then(slint::Weak::upgrade);
    host.as_ref()
        .map_or_else(|| app.window(), ComponentHandle::window)
        .with_winit_window(|window| window.fullscreen().is_some())
        .unwrap_or(false)
}

fn after_fullscreen_exit(
    wait: &Rc<slint::Timer>,
    app: &PlayerWindow,
    host: Option<slint::Weak<NativeVideoHost>>,
    state: Arc<Mutex<State>>,
) {
    let timer = Rc::downgrade(wait);
    let weak = app.as_weak();
    let deadline = Instant::now() + Duration::from_secs(5);
    // AppKit can retain the fullscreen space until its asynchronous exit completes.
    wait.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(32),
        move || {
            let Some(timer) = timer.upgrade() else { return };
            let Some(app) = weak.upgrade() else {
                timer.stop();
                return;
            };
            if app.get_fullscreen() {
                timer.stop();
            } else if !native_fullscreen(&app, host.as_ref()) {
                timer.stop();
                if !lock(&state).active() {
                    toggle_manually(&app, &state);
                }
            } else if Instant::now() >= deadline {
                timer.stop();
                app.set_fullscreen(true);
                app.set_status("The player could not leave fullscreen to enter Music Mode".into());
            }
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use riina_core::music_mode::AudioStatus;
    use riina_ui::PlayerMusicModeState;

    #[test]
    fn manual_music_mode_commits_only_after_the_fullscreen_owner_accepts_exit() {
        i_slint_backend_testing::init_no_event_loop();
        for accepted in [false, true] {
            let app = PlayerWindow::new().unwrap();
            app.set_fullscreen(true);
            let host = NativeVideoHost::new().unwrap();
            host.on_set_fullscreen_window(move |fullscreen| {
                assert!(!fullscreen);
                accepted
            });
            let state = Arc::new(Mutex::new(State::new(true)));
            install_actions(&app, &state, Some(host.as_weak()));
            app.global::<PlayerMenuActions>().invoke_toggle_music_mode();
            assert_eq!(app.get_fullscreen(), !accepted);
            assert_eq!(
                app.global::<PlayerMusicModeState>().get_music_mode(),
                accepted
            );
            assert_eq!(lock(&state).active(), accepted);
            if accepted {
                assert!(!request_fullscreen(&app, Some(&host.as_weak()), true));
                app.global::<PlayerMenuActions>().invoke_toggle_music_mode();
                assert!(!lock(&state).active());
            }
            let start = Instant::now();
            let mut state = lock(&state);
            assert_eq!(state.observe_at(AudioStatus::Audio, false, start), None);
            assert_eq!(
                state.observe_at(
                    AudioStatus::Audio,
                    false,
                    start + Duration::from_millis(300)
                ),
                Some(true),
            );
        }
    }
}
