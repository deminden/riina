//! Apply the shared aspect policy to the winit window without coupling it to a host OS.

use crate::{NativeVideoHost, PlayerWindow};
use riina_core::window_aspect::{self as policy, PixelSize};
use slint::{
    ComponentHandle, PhysicalPosition, PhysicalSize,
    winit_030::{EventResult, WinitWindowAccessor, winit},
};
use std::{cell::Cell, rc::Rc, time::Duration};

pub(crate) fn connect(app: &PlayerWindow) {
    let filter = connect_with_target(app, WindowTarget::Player, pixel_size(app.window().size()));
    app.window().on_winit_window_event(filter);
}

/// Bind the same portable aspect policy to the real surface when video is hosted
/// outside the shared chrome window.
pub(crate) fn connect_to_native_host(
    app: &PlayerWindow,
    host: &NativeVideoHost,
) -> WindowEventFilter {
    connect_with_target(
        app,
        WindowTarget::NativeHost(host.as_weak()),
        pixel_size(host.window().size()),
    )
}

pub(crate) type WindowEventFilter =
    Box<dyn Fn(&slint::Window, &winit::event::WindowEvent) -> EventResult>;

#[derive(Clone)]
enum WindowTarget {
    Player,
    NativeHost(slint::Weak<NativeVideoHost>),
}

fn connect_with_target(
    app: &PlayerWindow,
    target: WindowTarget,
    initial_size: PixelSize,
) -> WindowEventFilter {
    let previous_size = Rc::new(Cell::new(initial_size));
    let action_target = target.clone();
    let resize_generation = Rc::new(Cell::new(app.get_media_request_generation()));
    let initial_geometry_applied = Rc::new(Cell::new(false));
    let media_resize_state = previous_size.clone();
    let resize_target = target.clone();
    let initial_state = initial_geometry_applied.clone();
    app.on_resize_window_for_media({
        let weak = app.as_weak();
        move || {
            let Some(app) = weak.upgrade() else { return };
            let settings = match crate::window_resize_preferences::from_player(&app) {
                Ok(settings) => settings,
                Err(error) => {
                    app.set_status(error.into());
                    return;
                }
            };
            let generation = app.get_media_request_generation();
            let manually_opened = generation != resize_generation.get();
            resize_generation.set(generation);
            let initial = !initial_state.get();
            if !initial && !settings.timing.applies_to(manually_opened) {
                return;
            }
            let weak_target = resize_target.clone();
            with_target_window(&weak_target, &app, |window| {
                let Some((maximum, true)) = window_limits(window) else {
                    return;
                };
                let scale = window.scale_factor();
                let Some(size) = riina_core::window_resize::target_size(
                    f64::from(app.get_video_window_width()),
                    f64::from(app.get_video_window_height()),
                    Some(i64::from(app.get_video_window_rotation())),
                    f64::from(scale),
                    settings,
                    maximum,
                    PixelSize {
                        width: (520.0 * scale).round() as u32,
                        height: (300.0 * scale).round() as u32,
                    },
                ) else {
                    return;
                };
                let current = pixel_size(window.size());
                if initial {
                    let Some((screen_origin, screen_size)) = screen_bounds(window, maximum) else {
                        return;
                    };
                    let Some(placement) = riina_core::window_resize::initial_placement(
                        size,
                        f64::from(size.width) / f64::from(size.height),
                        f64::from(scale),
                        riina_core::window_resize::ScreenRect {
                            x: screen_origin.x,
                            y: screen_origin.y,
                            size: screen_size,
                        },
                        PixelSize {
                            width: (520.0 * scale).round() as u32,
                            height: (300.0 * scale).round() as u32,
                        },
                        settings.initial_geometry,
                    ) else {
                        return;
                    };
                    window.set_size(PhysicalSize::new(
                        placement.size.width,
                        placement.size.height,
                    ));
                    window.set_position(PhysicalPosition::new(placement.x, placement.y));
                    window.request_redraw();
                    initial_state.set(true);
                    media_resize_state.set(placement.size);
                } else {
                    resize_centered(window, current, size);
                    media_resize_state.set(size);
                }
            });
        }
    });

    let level_target = target.clone();
    let level_pending = Rc::new(Cell::new(false));
    app.on_window_level_changed({
        let weak = app.as_weak();
        move || {
            if let Some(app) = weak.upgrade() {
                let level_target = level_target.clone();
                defer_window_update(&app, &level_pending, move |app| {
                    apply_window_level(&app, &level_target);
                });
            }
        }
    });
    apply_window_level(app, &target);

    let aspect_resize_state = previous_size.clone();
    let aspect_pending = Rc::new(Cell::new(false));
    app.on_window_aspect_changed({
        let weak = app.as_weak();
        move || {
            let Some(app) = weak.upgrade() else { return };
            let action_target = action_target.clone();
            let aspect_resize_state = aspect_resize_state.clone();
            defer_window_update(&app, &aspect_pending, move |app| {
                with_target_window(&action_target, &app, |window| {
                    let current = pixel_size(window.size());
                    aspect_resize_state.set(current);
                    if !aspect_lock_enabled(&app) {
                        return;
                    }
                    let Some((maximum, can_resize)) = window_limits(window) else {
                        return;
                    };
                    if !can_resize {
                        return;
                    }
                    let Some(target) = policy::constrain_resize(
                        current,
                        current,
                        f64::from(app.get_video_display_aspect_ratio()),
                        maximum,
                    ) else {
                        return;
                    };
                    resize_centered(window, current, target);
                    aspect_resize_state.set(target);
                });
            });
        }
    });

    let fit_state = previous_size.clone();
    let fit_target = target.clone();
    app.on_fit_window_to_video({
        let weak = app.as_weak();
        move || {
            let Some(app) = weak.upgrade() else { return };
            with_target_window(&fit_target, &app, |window| {
                let current = pixel_size(window.size());
                let Some((maximum, can_resize)) = window_limits(window) else {
                    return;
                };
                if !can_resize {
                    return;
                }
                let Some(target) = policy::fit_to_video(
                    current,
                    f64::from(app.get_video_display_aspect_ratio()),
                    maximum,
                ) else {
                    return;
                };
                resize_centered(window, current, target);
                fit_state.set(target);
            });
        }
    });

    resize_filter(app, previous_size)
}

fn defer_window_update(
    app: &PlayerWindow,
    pending: &Rc<Cell<bool>>,
    action: impl FnOnce(PlayerWindow) + 'static,
) {
    if pending.replace(true) {
        return;
    }
    let pending = pending.clone();
    let weak = app.as_weak();
    // Property change handlers can run while AccessKit constructs its tree.
    // Resizing there pumps WM_GETOBJECT into an already borrowed adapter. Defer
    // native mutation until the tree read unwinds, coalescing to the latest state.
    slint::Timer::single_shot(Duration::ZERO, move || {
        pending.set(false);
        if let Some(app) = weak.upgrade() {
            action(app);
        }
    });
}

fn apply_window_level(app: &PlayerWindow, target: &WindowTarget) {
    let level = if app.get_always_float_on_top() {
        winit::window::WindowLevel::AlwaysOnTop
    } else {
        winit::window::WindowLevel::Normal
    };
    with_target_window(target, app, |window| {
        window.with_winit_window(|native| native.set_window_level(level));
    });
}

fn with_target_window(
    target: &WindowTarget,
    app: &PlayerWindow,
    action: impl FnOnce(&slint::Window),
) {
    match target {
        WindowTarget::Player => action(app.window()),
        WindowTarget::NativeHost(host) => {
            if let Some(host) = host.upgrade() {
                action(host.window());
            }
        }
    }
}

fn resize_filter(app: &PlayerWindow, previous_size: Rc<Cell<PixelSize>>) -> WindowEventFilter {
    let weak = app.as_weak();
    Box::new(move |window, event| {
        let winit::event::WindowEvent::Resized(size) = event else {
            return EventResult::Propagate;
        };
        let requested = PixelSize {
            width: size.width,
            height: size.height,
        };
        if let Some(app) = weak.upgrade()
            && aspect_lock_enabled(&app)
            && let Some((maximum, true)) = window_limits(window)
            && let Some(target) = policy::constrain_resize(
                previous_size.get(),
                requested,
                f64::from(app.get_video_display_aspect_ratio()),
                maximum,
            )
            && target != requested
        {
            // Keep the last accepted size so the corrective resize is not treated as a new drag.
            previous_size.set(target);
            resize_centered(window, requested, target);
            return EventResult::PreventDefault;
        }
        previous_size.set(requested);
        EventResult::Propagate
    })
}

fn aspect_lock_enabled(app: &PlayerWindow) -> bool {
    app.get_loaded()
        && app.get_edge_to_edge_video()
        && !app.get_unlock_window_aspect_ratio()
        && f64::from(app.get_video_display_aspect_ratio()).is_finite()
        && app.get_video_display_aspect_ratio() > 0.0
}

fn window_limits(window: &slint::Window) -> Option<(Option<PixelSize>, bool)> {
    window.with_winit_window(|native| {
        let can_resize = !native.is_maximized() && native.fullscreen().is_none();
        let maximum = native.current_monitor().map(|monitor| {
            let size = monitor.size();
            // winit reports the full display; reserve the taskbar/menu-bar and a small edge margin.
            PixelSize {
                width: size.width.saturating_mul(96) / 100,
                height: size.height.saturating_mul(90) / 100,
            }
        });
        (maximum, can_resize)
    })
}

fn screen_bounds(
    window: &slint::Window,
    usable_size: Option<PixelSize>,
) -> Option<(PhysicalPosition, PixelSize)> {
    window
        .with_winit_window(|native| {
            native.current_monitor().map(|monitor| {
                let origin = monitor.position();
                let screen_size = monitor.size();
                let size = usable_size.unwrap_or(PixelSize {
                    width: screen_size.width,
                    height: screen_size.height,
                });
                (PhysicalPosition::new(origin.x, origin.y), size)
            })
        })
        .flatten()
}

fn pixel_size(size: PhysicalSize) -> PixelSize {
    PixelSize {
        width: size.width,
        height: size.height,
    }
}

fn resize_centered(window: &slint::Window, current: PixelSize, target: PixelSize) {
    if target == current || target.width == 0 || target.height == 0 {
        return;
    }
    let position = window.position();
    let offset_x = i32::try_from(current.width).unwrap_or(i32::MAX)
        - i32::try_from(target.width).unwrap_or(i32::MAX);
    let offset_y = i32::try_from(current.height).unwrap_or(i32::MAX)
        - i32::try_from(target.height).unwrap_or(i32::MAX);
    window.set_size(PhysicalSize::new(target.width, target.height));
    window.set_position(PhysicalPosition::new(
        position.x.saturating_add(offset_x / 2),
        position.y.saturating_add(offset_y / 2),
    ));
    window.request_redraw();
}
