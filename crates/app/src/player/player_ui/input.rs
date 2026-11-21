//! Native input, focus registration and gesture state belong to one player window.
use crate::{PlayerWindow, media_open, platform};
use riina_core::{Command, Phase};
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
    if app.get_crop_editing() || app.get_dialog_open() {
        return;
    }
    let effect = action.effect(app.get_paused());
    let result = match effect {
        MouseEffect::None => return,
        MouseEffect::Fullscreen => {
            app.invoke_toggle_fullscreen();
            return;
        }
        MouseEffect::SetPaused(paused) => {
            if !app.get_loaded() {
                return;
            }
            player.set_paused(paused)
        }
        MouseEffect::HideControls => {
            if app.get_control_auto_hide_enabled() {
                app.set_controls_visible(false);
            }
            return;
        }
        MouseEffect::PictureInPicture => {
            app.invoke_toggle_picture_in_picture();
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
        app.set_status(error.into());
    }
}

thread_local! {
    static PLAYER_WINDOWS: RefCell<Vec<(u64, slint::Weak<PlayerWindow>)>> = const { RefCell::new(Vec::new()) };
}

pub(super) struct PlayerWindowFocusRegistration {
    _focused: Rc<platform::PlayerFocus>,
    registry_id: u64,
}

#[derive(Clone, Copy, Default)]
struct ScrollInputState {
    axis: Option<bool>,
    native_delta: Option<(f64, f64)>,
    precise: Option<bool>,
    natural_direction: Option<bool>,
    phase_empty: bool,
    phase_began: bool,
    phase_ended: bool,
    momentum_allowed: bool,
    mouse_in_window: bool,
    suppress_current: bool,
    appkit: bool,
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

fn install_file_drop(
    app: &PlayerWindow,
    player: riina_mpv::CommandSender,
    playback: crate::PlaybackStore,
    trackpad_pause_generation: Rc<Cell<Option<u64>>>,
    _scroll_input: Rc<Cell<ScrollInputState>>,
) -> (Rc<slint::Timer>, PlayerWindowFocusRegistration) {
    let registry_id = NEXT_PLAYER_WINDOW_REGISTRATION.fetch_add(1, Ordering::Relaxed);
    PLAYER_WINDOWS.with(|registered| {
        registered.borrow_mut().push((registry_id, app.as_weak()));
    });
    let pending_paths = Rc::new(RefCell::new(Vec::<std::ffi::OsString>::new()));
    let flush_paths = pending_paths.clone();
    let weak_app = app.as_weak();
    let timer_player = player.clone();
    let timer = Rc::new(slint::Timer::default());
    timer.start(
        slint::TimerMode::SingleShot,
        Duration::from_millis(80),
        move || {
            let paths = std::mem::take(&mut *flush_paths.borrow_mut());
            if !paths.is_empty()
                && let Some(app) = weak_app.upgrade()
            {
                media_open::dropped(&app, paths, timer_player.clone());
            }
        },
    );
    let event_paths = pending_paths;
    let event_timer = timer.clone();
    let focused = Rc::new(platform::PlayerFocus::new(
        app.window()
            .with_winit_window(|window| window.has_focus())
            .unwrap_or(false),
    ));
    let event_scroll_input = _scroll_input;
    let event_player = player;
    let event_playback = playback;
    let event_app = app.as_weak();
    let event_trackpad_pause_generation = trackpad_pause_generation;
    let event_focused = Rc::downgrade(&focused);
    app.window().on_winit_window_event(move |_window, event| {
        if let winit::event::WindowEvent::DroppedFile(path) = event {
            event_paths.borrow_mut().push(path.clone().into_os_string());
            event_timer.restart();
        }
        if let winit::event::WindowEvent::Focused(now_focused) = event
            && let Some(focused) = event_focused.upgrade()
        {
            focused.update(*now_focused);
        }
        if matches!(event, winit::event::WindowEvent::CursorEntered { .. }) {
            let mut input = event_scroll_input.get();
            input.mouse_in_window = true;
            event_scroll_input.set(input);
        } else if matches!(event, winit::event::WindowEvent::CursorLeft { .. }) {
            let mut input = event_scroll_input.get();
            input.mouse_in_window = false;
            input.momentum_allowed = false;
            event_scroll_input.set(input);
        }
        #[cfg(not(target_os = "macos"))]
        if let winit::event::WindowEvent::MouseWheel { delta, phase, .. } = event {
            use winit::event::{MouseScrollDelta, TouchPhase};
            let (delta_x, delta_y, precise) = match delta {
                MouseScrollDelta::LineDelta(x, y) => (f64::from(*x), f64::from(*y), false),
                MouseScrollDelta::PixelDelta(position) => {
                    let position = position.to_logical::<f64>(f64::from(_window.scale_factor()));
                    (position.x, position.y, true)
                }
            };
            let mut input = event_scroll_input.get();
            input.appkit = false;
            input.native_delta = Some((delta_x, delta_y));
            input.precise = Some(precise);
            input.natural_direction = None;
            input.phase_empty = !precise;
            input.phase_began = *phase == TouchPhase::Started;
            input.phase_ended = matches!(phase, TouchPhase::Ended | TouchPhase::Cancelled);
            if !precise {
                // Mouse-wheel events are independent. IINA prioritizes horizontal
                // whenever an event contains a horizontal delta.
                input.axis = (delta_x != 0.0 || delta_y != 0.0).then_some(delta_x != 0.0);
            } else {
                match phase {
                    TouchPhase::Started => {
                        // IINA chooses the first non-zero axis when a gesture begins,
                        // then keeps it through diagonal trackpad motion.
                        let axis = if delta_x != 0.0 {
                            Some(true)
                        } else if delta_y != 0.0 {
                            Some(false)
                        } else {
                            None
                        };
                        input.axis = axis;
                    }
                    TouchPhase::Ended | TouchPhase::Cancelled => {
                        input.axis = None;
                    }
                    TouchPhase::Moved if input.axis.is_none() => {
                        if delta_x != 0.0 || delta_y != 0.0 {
                            input.axis = Some(delta_x != 0.0);
                        }
                    }
                    TouchPhase::Moved => {}
                }
            }
            event_scroll_input.set(input);
        }
        if matches!(event, winit::event::WindowEvent::MouseWheel { .. }) {
            let input = event_scroll_input.get();
            if input.suppress_current {
                return EventResult::PreventDefault;
            }
            if !input.phase_empty
                && input.phase_ended
                && let Some(generation) = event_trackpad_pause_generation.replace(None)
                && {
                    let current = riina_core::sync::lock(&event_playback).generation;
                    current == generation
                }
                && let Err(error) = event_player.set_paused(false)
                && let Some(app) = event_app.upgrade()
            {
                app.set_status(error.into());
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

fn scroll_action_handler(
    app: slint::Weak<PlayerWindow>,
    player: riina_mpv::CommandSender,
    playback: crate::PlaybackStore,
    scroll_input: Rc<Cell<ScrollInputState>>,
    trackpad_pause_generation: Rc<Cell<Option<u64>>>,
    override_action: Option<riina_core::control_preferences::ScrollAction>,
) -> impl FnMut(f32, f32) -> bool + 'static {
    move |delta_x, delta_y| {
        let Some(app) = app.upgrade() else {
            return false;
        };
        if app.get_scroll_input_suppressed() {
            return true;
        }
        let input = scroll_input.get();
        let horizontal = if input.appkit {
            input.axis.unwrap_or(false)
        } else {
            input.axis.unwrap_or_else(|| delta_x.abs() > delta_y.abs())
        };
        let precise = input
            .precise
            .unwrap_or_else(|| delta_x.abs().max(delta_y.abs()) < 60.0);
        let (mut delta_x, mut delta_y) = input.native_delta.unwrap_or_else(|| {
            let divisor = if precise { 1.0 } else { 60.0 };
            (f64::from(delta_x) / divisor, f64::from(delta_y) / divisor)
        });
        if input.appkit {
            if input.natural_direction == Some(true) {
                delta_y = -delta_y;
            } else {
                delta_x = -delta_x;
            }
        }
        let delta = if horizontal { delta_x } else { delta_y };
        if delta == 0.0 {
            return false;
        }
        let Ok(settings) = crate::control_preferences::current_player(&app) else {
            return false;
        };
        let action = override_action.unwrap_or(if horizontal {
            settings.horizontal_scroll_action
        } else {
            settings.vertical_scroll_action
        });
        let effect = riina_core::control_preferences::scroll_effect_for_input(
            &settings,
            action,
            horizontal,
            delta,
            precise,
            input.phase_empty,
        );
        let command = match effect {
            riina_core::control_preferences::ScrollEffect::None => return false,
            riina_core::control_preferences::ScrollEffect::AdjustVolume(delta) => {
                Command::AdjustVolume {
                    delta,
                    boost: app.get_volume_boost(),
                }
            }
            riina_core::control_preferences::ScrollEffect::Seek(seconds) => {
                if !input.phase_empty
                    && input.phase_began
                    && trackpad_pause_generation.get().is_none()
                {
                    let generation = {
                        let state = riina_core::sync::lock(&playback);
                        (state.phase == Phase::Playing && !state.paused).then_some(state.generation)
                    };
                    if let Some(generation) = generation {
                        match player.set_paused(true) {
                            Ok(()) => trackpad_pause_generation.set(Some(generation)),
                            Err(error) => app.set_status(error.into()),
                        }
                    }
                }
                Command::SeekWithType {
                    seconds,
                    seek_type: settings.seek_type,
                }
            }
            riina_core::control_preferences::ScrollEffect::MultiplySpeed(factor) => {
                let current = f64::from(app.get_speed_value()).clamp(0.05, 4.0);
                let target = (current * factor).clamp(0.05, 4.0);
                Command::MultiplySpeed(target / current)
            }
        };
        app.invoke_activity();
        if let Err(error) = player.send(command) {
            app.set_status(error.into());
        }
        true
    }
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
) -> InstalledInput {
    let scroll_input = Rc::new(Cell::new(ScrollInputState::default()));
    let scroll_monitor = {
        let native_input = scroll_input.clone();
        platform::PlayerScrollMonitor::install(app, move |event| {
            let mut input = native_input.get();
            input.appkit = true;
            // AppKit's coarse wheel semantics use direction only. Other hosts
            // preserve fractional wheel lines because they lack its precise flag.
            input.native_delta = Some(if event.precise {
                (event.delta_x, event.delta_y)
            } else {
                let unit = |value: f64| if value == 0.0 { 0.0 } else { value.signum() };
                (unit(event.delta_x), unit(event.delta_y))
            });
            input.precise = Some(event.precise);
            input.natural_direction = Some(event.natural_direction);
            input.phase_empty = event.phase_empty;
            input.phase_began = event.phase_began;
            input.phase_ended = event.phase_ended;
            if !event.momentum_phase_empty && !input.momentum_allowed {
                input.suppress_current = true;
                native_input.set(input);
                return;
            }
            input.suppress_current = false;
            input.momentum_allowed = event.phase_ended || input.mouse_in_window;
            if event.phase_empty || event.phase_began {
                input.axis = if event.delta_x != 0.0 {
                    Some(true)
                } else if event.delta_y != 0.0 {
                    Some(false)
                } else {
                    None
                };
            } else if event.phase_ended {
                input.axis = None;
            }
            native_input.set(input);
        })
    };
    let trackpad_pause_generation = Rc::new(Cell::new(None));
    platform::install_player_gestures(app, player, dispatch_control_mouse_action);
    let (file_drop_timer, player_window_focus) = install_file_drop(
        app,
        player.clone(),
        latest_playback.clone(),
        trackpad_pause_generation.clone(),
        scroll_input.clone(),
    );
    let pending_single_click = Rc::new(RefCell::new(None::<i32>));
    let single_click_timer = Rc::new(slint::Timer::default());
    let pending = pending_single_click.clone();
    let timer = single_click_timer.clone();
    let single_player = player.clone();
    let single_app = app.as_weak();
    app.on_single_click_action(move |action| {
        if let Some(app) = single_app.upgrade()
            && app.get_control_double_click_index() == 0
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
    app.on_double_click_action(move |action| {
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
    app.on_mouse_button_action(move |action| {
        if let Some(app) = button_app.upgrade() {
            dispatch_control_mouse_action(&app, &button_player, action);
        }
    });
    let scroll_player = player.clone();
    let scroll_playback = latest_playback.clone();
    let scroll_pause_generation = trackpad_pause_generation.clone();
    app.on_scroll_action_request(scroll_action_handler(
        app.as_weak(),
        scroll_player.clone(),
        scroll_playback.clone(),
        scroll_input.clone(),
        scroll_pause_generation.clone(),
        None,
    ));
    app.on_seek_scroll_action_request(scroll_action_handler(
        app.as_weak(),
        scroll_player.clone(),
        scroll_playback.clone(),
        scroll_input.clone(),
        scroll_pause_generation.clone(),
        Some(riina_core::control_preferences::ScrollAction::Seek),
    ));
    app.on_volume_scroll_action_request(scroll_action_handler(
        app.as_weak(),
        scroll_player,
        scroll_playback,
        scroll_input,
        scroll_pause_generation,
        Some(riina_core::control_preferences::ScrollAction::Volume),
    ));
    InstalledInput {
        file_drop_timer,
        player_window_focus,
        scroll_monitor,
    }
}
pub(super) fn install_mpv_input_action(app: &PlayerWindow, context: &super::PlayerContext) {
    use super::bind;
    bind!(context, app, on_execute_mpv_input_action, |ctx, action| {
        let Some(app) = ctx.app() else { return };
        app.invoke_activity();
        ctx.send(Command::MpvInputAction(action.to_string()));
    });
}
