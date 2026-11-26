//! Native input, focus registration and gesture state belong to one player window.
mod scroll;

use self::scroll::ScrollInputState;
use super::{embedded_input, playlist_drop, playlist_hover::PlaylistHover};
use crate::{PlayerWindow, platform};
use riina_core::Command;
use riina_ui::PlayerMenuActions;
use slint::{
    ComponentHandle,
    winit_030::{EventResult, WinitWindowAccessor, winit},
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

static NEXT_PLAYER_WINDOW_REGISTRATION: AtomicU64 = AtomicU64::new(1);

fn dispatch_control_mouse_action(
    app: &PlayerWindow,
    player: &riina_mpv::CommandSender,
    index: i32,
) {
    use riina_core::control_preferences::MouseEffect;

    let Some(action) = riina_core::control_preferences::MouseAction::from_index(index) else {
        app.set_status("Invalid mouse action preference".into());
        return;
    };
    if app.global::<riina_ui::PlayerCropState>().get_crop_editing()
        || app
            .global::<riina_ui::PlayerDialogState>()
            .get_dialog_open()
    {
        return;
    }
    let effect = action.effect(app.get_paused());
    let result = match effect {
        MouseEffect::None => return,
        MouseEffect::Fullscreen => {
            app.global::<PlayerMenuActions>().invoke_toggle_fullscreen();
            return;
        }
        MouseEffect::SetPaused(paused) => {
            if !app.get_loaded() {
                return;
            }
            player.set_paused(paused)
        }
        MouseEffect::HideControls => {
            if app
                .global::<riina_ui::PlayerPreferences>()
                .get_control_auto_hide_enabled()
            {
                app.global::<riina_ui::PlayerControlsState>()
                    .set_controls_visible(false);
            }
            return;
        }
        MouseEffect::PictureInPicture => {
            app.global::<PlayerMenuActions>()
                .invoke_toggle_picture_in_picture();
            return;
        }
        MouseEffect::CycleAbLoop => {
            if !app.get_loaded() {
                return;
            }
            player.send(Command::CycleAbLoop)
        }
        MouseEffect::ResetSpeed => {
            if !app.get_loaded() {
                return;
            }
            player.send(Command::Speed(1.0))
        }
    };
    if let Err(error) = result {
        crate::diagnostics::report(app, error);
    }
}

thread_local! {
    static PLAYER_WINDOWS: RefCell<Vec<(u64, slint::Weak<PlayerWindow>)>> = const { RefCell::new(Vec::new()) };
}

pub(super) struct PlayerWindowFocusRegistration {
    _focused: Rc<platform::PlayerFocus>,
    registry_id: u64,
}

impl Drop for PlayerWindowFocusRegistration {
    fn drop(&mut self) {
        PLAYER_WINDOWS.with(|registered| {
            registered
                .borrow_mut()
                .retain(|(id, _)| *id != self.registry_id);
        });
    }
}

fn install_window_events(
    app: &PlayerWindow,
    player: riina_mpv::CommandSender,
    playback: crate::PlaybackStore,
    trackpad_pause_generation: Rc<Cell<Option<u64>>>,
    scroll_input: Rc<Cell<ScrollInputState>>,
    native_host: Option<slint::Weak<crate::NativeVideoHost>>,
    mut handle_window_event: impl FnMut(&slint::Window, &winit::event::WindowEvent) -> EventResult
    + 'static,
) -> (Rc<slint::Timer>, PlayerWindowFocusRegistration) {
    let registry_id = NEXT_PLAYER_WINDOW_REGISTRATION.fetch_add(1, Ordering::Relaxed);
    PLAYER_WINDOWS.with(|registered| {
        registered.borrow_mut().push((registry_id, app.as_weak()));
    });
    let weak_app = app.as_weak();
    let drop_app = weak_app.clone();
    let timer_player = player.clone();
    let (timer, file_drop) = platform::batched_file_drops(
        move |window| {
            drop_app
                .upgrade()
                .map_or(Ok(playlist_drop::Target::Player), |app| {
                    playlist_drop::target(&app, window)
                })
        },
        move |target, paths| {
            if let Some(app) = weak_app.upgrade() {
                playlist_drop::handle(&app, &timer_player, target, paths);
            }
        },
    );
    let focused = Rc::new(platform::PlayerFocus::new(
        app.window()
            .with_winit_window(|window| window.has_focus())
            .unwrap_or(false),
    ));
    let event_scroll_input = scroll_input;
    let event_player = player;
    let event_playback = playback;
    let event_app = app.as_weak();
    let event_trackpad_pause_generation = trackpad_pause_generation;
    let event_focused = Rc::downgrade(&focused);
    let playlist_hover = Rc::new(PlaylistHover::new(app));
    let native_drop = playlist_drop::register_desktop(app, &event_player, playlist_hover.clone());
    native_drop.attach_owner(native_host);
    let mut script_keys = embedded_input::WindowKeys::default();
    app.window().on_winit_window_event(move |window, event| {
        native_drop.attach(window);
        // The native event arrives before Slint can match a menu accelerator.
        if script_keys.handle(&event_app, event) {
            return EventResult::PreventDefault;
        }
        if handle_window_event(window, event) == EventResult::PreventDefault {
            return EventResult::PreventDefault;
        }
        if matches!(event, winit::event::WindowEvent::CursorMoved { .. })
            && let Some(app) = event_app.upgrade()
        {
            platform::track_plugin_pointer(&app, window, event);
        }
        file_drop(window, event);
        // The target of the first file is captured before the temporary sidebar closes.
        playlist_hover.on_event(event);
        if let winit::event::WindowEvent::Focused(now_focused) = event
            && let Some(focused) = event_focused.upgrade()
        {
            focused.update(*now_focused);
            if *now_focused && let Some(app) = event_app.upgrade() {
                app.invoke_history_player_activated();
                app.invoke_inspector_player_activated();
            }
        }
        let mut input = event_scroll_input.get();
        input.note_window_event(window, &event_app, event);
        event_scroll_input.set(input);
        if matches!(event, winit::event::WindowEvent::MouseWheel { .. }) {
            if input.suppressed() {
                return EventResult::PreventDefault;
            }
            if input.gesture_ended()
                && let Some(generation) = event_trackpad_pause_generation.replace(None)
                && {
                    let current = riina_core::sync::lock(&event_playback).generation;
                    current == generation
                }
                && let Err(error) = event_player.set_paused(false)
                && let Some(app) = event_app.upgrade()
            {
                crate::diagnostics::report(&app, error);
            }
        }
        EventResult::Propagate
    });
    (
        timer,
        PlayerWindowFocusRegistration {
            _focused: focused,
            registry_id,
        },
    )
}

pub(super) fn open_player_windows() -> Vec<PlayerWindow> {
    PLAYER_WINDOWS.with(|registered| {
        let mut registered = registered.borrow_mut();
        registered.retain(|(_, weak)| weak.upgrade().is_some());
        registered
            .iter()
            .filter_map(|(_, weak)| weak.upgrade())
            .collect()
    })
}

pub(super) struct InstalledInput {
    pub(super) file_drop_timer: Rc<slint::Timer>,
    pub(super) player_window_focus: PlayerWindowFocusRegistration,
    pub(super) scroll_monitor: platform::PlayerScrollMonitor,
}

pub(super) fn install(
    app: &PlayerWindow,
    player: &riina_mpv::CommandSender,
    latest_playback: crate::PlaybackStore,
    resize_filter: Option<crate::window_aspect::WindowEventFilter>,
    fullscreen: Rc<super::fullscreen::Controller>,
    native_host: Option<slint::Weak<crate::NativeVideoHost>>,
) -> InstalledInput {
    let scroll_input = Rc::new(Cell::new(ScrollInputState::default()));
    let scroll_monitor = scroll::install_monitor(app, scroll_input.clone());
    let trackpad_pause_generation = Rc::new(Cell::new(None));
    let mut gestures = platform::player_gesture_handler(app, player, dispatch_control_mouse_action);
    let probe_fullscreen = super::fullscreen::probe_after_window_event(fullscreen);
    // Slint has one Winit callback per window. Keep native gestures and size
    // constraints on the same route as file drops, wheel input and focus.
    let (file_drop_timer, player_window_focus) = install_window_events(
        app,
        player.clone(),
        latest_playback.clone(),
        trackpad_pause_generation.clone(),
        scroll_input.clone(),
        native_host,
        move |window, event| {
            probe_fullscreen(event);
            gestures(event);
            resize_filter
                .as_ref()
                .map_or(EventResult::Propagate, |resize| resize(window, event))
        },
    );
    let pending_single_click = Rc::new(RefCell::new(None::<i32>));
    let single_click_timer = Rc::new(slint::Timer::default());
    let pending = pending_single_click.clone();
    let timer = single_click_timer.clone();
    let single_player = player.clone();
    let single_app = app.as_weak();
    app.global::<riina_ui::PlayerControlActions>()
        .on_single_click_action(move |action| {
            if let Some(app) = single_app.upgrade()
                && app
                    .global::<riina_ui::PlayerPreferences>()
                    .get_control_double_click_index()
                    == 0
            {
                dispatch_control_mouse_action(&app, &single_player, action);
                return;
            }
            *pending.borrow_mut() = Some(action);
            let pending = pending.clone();
            let player = single_player.clone();
            let weak = single_app.clone();
            // Slint reports clicked before double-clicked. Delay the single action
            // so a double click invokes only its configured action.
            timer.start(
                slint::TimerMode::SingleShot,
                platform::double_click_interval(),
                move || {
                    let action = pending.borrow_mut().take();
                    if let (Some(action), Some(app)) = (action, weak.upgrade()) {
                        dispatch_control_mouse_action(&app, &player, action);
                    }
                },
            );
        });
    let pending = pending_single_click;
    let timer = single_click_timer;
    let double_player = player.clone();
    let double_app = app.as_weak();
    app.global::<riina_ui::PlayerControlActions>()
        .on_double_click_action(move |action| {
            timer.stop();
            pending.borrow_mut().take();
            let app = double_app.clone();
            let player = double_player.clone();
            // Fullscreen can move/hide the surface delivering this pointer release.
            // Let capture and double-click dispatch unwind before changing HWNDs.
            slint::Timer::single_shot(Duration::ZERO, move || {
                if let Some(app) = app.upgrade() {
                    dispatch_control_mouse_action(&app, &player, action);
                }
            });
        });
    let button_player = player.clone();
    let button_app = app.as_weak();
    app.global::<riina_ui::PlayerControlActions>()
        .on_mouse_button_action(move |action| {
            if let Some(app) = button_app.upgrade() {
                dispatch_control_mouse_action(&app, &button_player, action);
            }
        });
    scroll::install_handlers(
        app,
        player,
        latest_playback,
        scroll_input,
        trackpad_pause_generation,
    );
    InstalledInput {
        file_drop_timer,
        player_window_focus,
        scroll_monitor,
    }
}
pub(super) fn install_mpv_input_action(app: &PlayerWindow, context: &super::PlayerContext) {
    use super::bind;
    bind!(
        context,
        app.global::<riina_ui::PlayerControlActions>(),
        on_execute_mpv_input_action,
        |ctx, action| {
            let Some(app) = ctx.app() else { return };
            app.global::<riina_ui::PlayerControlActions>()
                .invoke_activity();
            ctx.send(Command::MpvInputAction(action.to_string()));
        }
    );
}
