//! Apply the shared aspect policy to the winit window without coupling it to a host OS.

#[cfg(target_os = "windows")]
use crate::NativeVideoHost;
use crate::PlayerWindow;
use riina_core::{
    window_aspect::{self as policy, PixelSize},
    window_resize::{ManualSizeCommand, ManualSizeContext, ResizeTiming, manual_size_target},
};
use slint::{
    ComponentHandle, PhysicalPosition, PhysicalSize,
    winit_030::{EventResult, WinitWindowAccessor, winit},
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

pub(crate) fn connect(app: &PlayerWindow) {
    let filter = connect_with_target(app, WindowTarget::Player, pixel_size(app.window().size()));
    app.window().on_winit_window_event(filter);
}

/// Bind the same portable aspect policy to the real surface when video is hosted
/// outside the shared chrome window.
#[cfg(target_os = "windows")]
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
    #[cfg(target_os = "windows")]
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
    let media_pending = Rc::new(Cell::new(false));
    app.on_resize_window_for_media({
        let weak = app.as_weak();
        move || {
            let Some(app) = weak.upgrade() else { return };
            let resize_generation = resize_generation.clone();
            let initial_state = initial_state.clone();
            let resize_target = resize_target.clone();
            let media_resize_state = media_resize_state.clone();
            defer_window_update(&app, &media_pending, move |app| {
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
                if !media_resize_applies(
                    initial,
                    settings.timing,
                    manually_opened,
                    app.get_music_mode(),
                ) {
                    mark_window_size_adjusted(&app);
                    // The configured timing also governs automatic playlist
                    // changes. Reshaping for the new video's aspect here can
                    // clear the native Windows swap chain while the next
                    // decoder is starting, even when the resize option says
                    // "when opened manually" or "never".
                    return;
                }
                #[cfg(target_os = "windows")]
                if !windows_media_resize_applies(initial, manually_opened) {
                    mark_window_size_adjusted(&app);
                    // Playlist handoffs can replace the native D3D11 surface
                    // even when the new frame is larger. Keep the Windows
                    // player frame stable for the entire automatic sequence;
                    // the video renderer fits each source into that viewport.
                    return;
                }
                let weak_target = resize_target.clone();
                with_target_window(&weak_target, &app, |window| {
                    let Some((maximum, true)) = window_limits(window) else {
                        return;
                    };
                    let scale = window.scale_factor();
                    let current = pixel_size(window.size());
                    let size = riina_core::window_resize::target_size(
                        f64::from(app.get_video_window_width()),
                        f64::from(app.get_video_window_height()),
                        Some(i64::from(app.get_video_window_rotation())),
                        f64::from(scale),
                        settings,
                        maximum,
                        PixelSize {
                            width: (285.0 * scale).round() as u32,
                            height: (120.0 * scale).round() as u32,
                        },
                    );
                    let Some(size) = size else {
                        return;
                    };
                    if initial {
                        // The size limit reserves room for desktop chrome, but it is
                        // not the monitor's origin-relative work area. Using that
                        // reduced height as the placement bounds pins tall videos
                        // against the top edge instead of centering them on-screen.
                        let Some((screen_origin, screen_size)) = screen_bounds(window, None) else {
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
                                width: (285.0 * scale).round() as u32,
                                height: (120.0 * scale).round() as u32,
                            },
                            settings.initial_geometry,
                        ) else {
                            return;
                        };
                        #[cfg(target_os = "windows")]
                        let positioned = settings.initial_geometry.position.is_none()
                            && window
                                .with_winit_window(|native| {
                                    if let Some(overlay) = resize_overlay(&resize_target, &app) {
                                        overlay
                                            .with_winit_window(|overlay| {
                                                crate::platform::resize_content(
                                                    native,
                                                    placement.size,
                                                    true,
                                                    Some(overlay),
                                                )
                                                .is_ok()
                                            })
                                            .unwrap_or(false)
                                    } else {
                                        crate::platform::resize_content(
                                            native,
                                            placement.size,
                                            true,
                                            None,
                                        )
                                        .is_ok()
                                    }
                                })
                                .unwrap_or(false);
                        #[cfg(not(target_os = "windows"))]
                        let positioned = false;
                        if !positioned {
                            window.set_size(PhysicalSize::new(
                                placement.size.width,
                                placement.size.height,
                            ));
                            let position = PhysicalPosition::new(placement.x, placement.y);
                            let position = if settings.initial_geometry.position.is_none() {
                                center_outer_frame(window, placement.size).unwrap_or(position)
                            } else {
                                position
                            };
                            window.set_position(position);
                        }
                        window.request_redraw();
                        initial_state.set(true);
                        media_resize_state.set(placement.size);
                    } else {
                        resize_centered(
                            window,
                            current,
                            size,
                            resize_overlay(&resize_target, &app),
                        );
                        media_resize_state.set(size);
                    }
                });
                // IINA emits windowSizeAdjusted after applying its geometry policy,
                // including playlist changes where the frame stays the same.
                mark_window_size_adjusted(&app);
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
    let aspect_source = Rc::new(RefCell::new(None::<slint::SharedString>));
    let aspect_request_generation = Rc::new(Cell::new(app.get_media_request_generation()));
    let aspect_resize_allowed = Rc::new(Cell::new(true));
    let aspect_initial_state = initial_geometry_applied.clone();
    app.on_window_aspect_changed({
        let weak = app.as_weak();
        move || {
            let Some(app) = weak.upgrade() else { return };
            let action_target = action_target.clone();
            let aspect_resize_state = aspect_resize_state.clone();
            let aspect_source = aspect_source.clone();
            let aspect_request_generation = aspect_request_generation.clone();
            let aspect_resize_allowed = aspect_resize_allowed.clone();
            let aspect_initial_state = aspect_initial_state.clone();
            defer_window_update(&app, &aspect_pending, move |app| {
                with_target_window(&action_target, &app, |window| {
                    let current = pixel_size(window.size());
                    aspect_resize_state.set(current);
                    let source = app.get_window_geometry_source();
                    if aspect_source.borrow().as_ref() != Some(&source) {
                        let timing = match crate::window_resize_preferences::from_player(&app) {
                            Ok(settings) => settings.timing,
                            Err(error) => {
                                app.set_status(error.into());
                                return;
                            }
                        };
                        let generation = app.get_media_request_generation();
                        let manually_opened = generation != aspect_request_generation.get();
                        aspect_request_generation.set(generation);
                        let initial = !aspect_initial_state.get();
                        let resize_allowed = media_resize_applies(
                            initial,
                            timing,
                            manually_opened,
                            app.get_music_mode(),
                        );
                        #[cfg(target_os = "windows")]
                        let resize_allowed = resize_allowed
                            && windows_media_resize_applies(initial, manually_opened);
                        aspect_resize_allowed.set(resize_allowed);
                        *aspect_source.borrow_mut() = Some(source);
                        return;
                    }
                    if !aspect_lock_enabled(&app) || !aspect_resize_allowed.get() {
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
                    let Some(target) = policy::bounded_size(
                        target,
                        f64::from(app.get_video_display_aspect_ratio()),
                        minimum_content_size(window),
                        maximum,
                    ) else {
                        return;
                    };
                    resize_centered(window, current, target, None);
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
                let Some(target) = policy::bounded_size(
                    target,
                    f64::from(app.get_video_display_aspect_ratio()),
                    minimum_content_size(window),
                    maximum,
                ) else {
                    return;
                };
                resize_centered(window, current, target, None);
                fit_state.set(target);
            });
        }
    });

    let menu_size_target = target.clone();
    let menu_size_state = previous_size.clone();
    app.on_resize_window_command({
        let weak = app.as_weak();
        move |index| {
            let Some(app) = weak.upgrade() else { return };
            let Some(command) = ManualSizeCommand::from_index(index) else {
                return;
            };
            with_target_window(&menu_size_target, &app, |window| {
                let current = pixel_size(window.size());
                let Some((maximum, can_resize)) = window_limits(window) else {
                    return;
                };
                if !can_resize {
                    return;
                }
                let scale = f64::from(window.scale_factor());
                let Some(size) = manual_size_target(
                    command,
                    ManualSizeContext {
                        current,
                        video_width: f64::from(app.get_video_window_width()),
                        video_height: f64::from(app.get_video_window_height()),
                        rotation: Some(i64::from(app.get_video_window_rotation())),
                        display_scale: scale,
                        use_physical_resolution: app.get_use_physical_resolution(),
                        maximum,
                        minimum: PixelSize {
                            width: (285.0 * scale).round() as u32,
                            height: (120.0 * scale).round() as u32,
                        },
                    },
                ) else {
                    return;
                };
                if command == ManualSizeCommand::FitScreen {
                    let Some((origin, bounds)) = screen_bounds(window, maximum) else {
                        return;
                    };
                    window.set_size(PhysicalSize::new(size.width, size.height));
                    window.set_position(PhysicalPosition::new(
                        origin.x.saturating_add(
                            i32::try_from(bounds.width.saturating_sub(size.width) / 2)
                                .unwrap_or(i32::MAX),
                        ),
                        origin.y.saturating_add(
                            i32::try_from(bounds.height.saturating_sub(size.height) / 2)
                                .unwrap_or(i32::MAX),
                        ),
                    ));
                    window.request_redraw();
                } else {
                    resize_centered(window, current, size, None);
                }
                menu_size_state.set(size);
            });
        }
    });

    resize_filter(app, previous_size, target)
}

fn media_resize_applies(
    initial: bool,
    timing: ResizeTiming,
    manually_opened: bool,
    music_mode: bool,
) -> bool {
    // Attached cover images still report video dimensions. The compact player
    // owns its frame; the main video's resize policy must not overwrite it.
    !music_mode && (initial || timing.applies_to(manually_opened))
}

#[cfg(target_os = "windows")]
fn windows_media_resize_applies(initial: bool, manually_opened: bool) -> bool {
    initial || manually_opened
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

fn mark_window_size_adjusted(app: &PlayerWindow) {
    app.set_window_size_adjusted_event(app.get_window_size_adjusted_event().wrapping_add(1));
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
        #[cfg(target_os = "windows")]
        WindowTarget::NativeHost(host) => {
            if let Some(host) = host.upgrade() {
                action(host.window());
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn resize_overlay<'a>(target: &WindowTarget, app: &'a PlayerWindow) -> Option<&'a slint::Window> {
    matches!(target, WindowTarget::NativeHost(_)).then_some(app.window())
}

#[cfg(not(target_os = "windows"))]
fn resize_overlay<'a>(_: &WindowTarget, _: &'a PlayerWindow) -> Option<&'a slint::Window> {
    None
}

fn resize_filter(
    app: &PlayerWindow,
    previous_size: Rc<Cell<PixelSize>>,
    target: WindowTarget,
) -> WindowEventFilter {
    #[cfg(target_os = "windows")]
    let native_sizing = install_native_sizing(app, target);
    #[cfg(not(target_os = "windows"))]
    let _ = target;
    #[cfg(not(target_os = "windows"))]
    let weak = app.as_weak();
    Box::new(move |window, event| {
        #[cfg(target_os = "windows")]
        let _ = (&native_sizing, window);
        let winit::event::WindowEvent::Resized(size) = event else {
            return EventResult::Propagate;
        };
        let requested = PixelSize {
            width: size.width,
            height: size.height,
        };
        #[cfg(not(target_os = "windows"))]
        if let Some(app) = weak.upgrade()
            && requested != previous_size.get()
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
            resize_centered(window, requested, target, None);
            return EventResult::PreventDefault;
        }
        previous_size.set(requested);
        EventResult::Propagate
    })
}

#[cfg(target_os = "windows")]
fn install_native_sizing(
    app: &PlayerWindow,
    target: WindowTarget,
) -> Rc<RefCell<Option<crate::platform::WindowSizing>>> {
    let sizing = Rc::new(RefCell::new(None));
    let installed = sizing.clone();
    let weak = app.as_weak();
    let _ = slint::spawn_local(async move {
        let Some(app) = weak.upgrade() else { return };
        let window = match target {
            WindowTarget::Player => app.window().winit_window().await,
            WindowTarget::NativeHost(host) => {
                let Some(host) = host.upgrade() else { return };
                host.window().winit_window().await
            }
        };
        match window
            .map_err(|error| error.to_string())
            .and_then(|window| crate::platform::WindowSizing::install(&window, app.as_weak()))
        {
            Ok(handler) => *installed.borrow_mut() = Some(handler),
            Err(error) => app.set_status(error.into()),
        }
    });
    sizing
}

fn minimum_content_size(window: &slint::Window) -> PixelSize {
    // IINA v1.5.0 AppData.mainWindowMinSize is 285 by 120 logical points.
    let scale = window.scale_factor();
    PixelSize {
        width: (285.0 * scale).round() as u32,
        height: (120.0 * scale).round() as u32,
    }
}

fn aspect_lock_enabled(app: &PlayerWindow) -> bool {
    app.get_loaded()
        && !app.get_music_mode()
        && app.get_edge_to_edge_video()
        && !app.get_unlock_window_aspect_ratio()
        && f64::from(app.get_video_display_aspect_ratio()).is_finite()
        && app.get_video_display_aspect_ratio() > 0.0
}

fn window_limits(window: &slint::Window) -> Option<(Option<PixelSize>, bool)> {
    let can_resize = window
        .with_winit_window(|native| !native.is_maximized() && native.fullscreen().is_none())?;
    #[cfg(target_os = "windows")]
    let maximum = crate::platform::window_content_work_area(window);
    #[cfg(not(target_os = "windows"))]
    let maximum = window.with_winit_window(|native| {
        native.current_monitor().map(|monitor| {
            let size = monitor.size();
            // winit reports the full display; reserve the taskbar/menu-bar and a small edge margin.
            PixelSize {
                width: size.width.saturating_mul(96) / 100,
                height: size.height.saturating_mul(90) / 100,
            }
        })
    })?;
    Some((maximum, can_resize))
}

fn screen_bounds(
    window: &slint::Window,
    usable_size: Option<PixelSize>,
) -> Option<(PhysicalPosition, PixelSize)> {
    if let Some(bounds) = crate::platform::window_work_area(window) {
        return Some((
            PhysicalPosition::new(bounds.x, bounds.y),
            usable_size.unwrap_or(bounds.size),
        ));
    }
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

fn center_outer_frame(window: &slint::Window, content_size: PixelSize) -> Option<PhysicalPosition> {
    let target_frame = window.with_winit_window(|native| {
        let outer_size = native.outer_size();
        #[cfg(target_os = "windows")]
        {
            let _ = content_size;
            // Windows has already applied the synchronous resize, including
            // any extra native menu row at this width.
            PixelSize {
                width: outer_size.width,
                height: outer_size.height,
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let client_size = native.inner_size();
            PixelSize {
                width: content_size
                    .width
                    .saturating_add(outer_size.width.saturating_sub(client_size.width)),
                height: content_size
                    .height
                    .saturating_add(outer_size.height.saturating_sub(client_size.height)),
            }
        }
    })?;
    let (screen_origin, screen_size) = screen_bounds(window, None)?;
    let area = riina_core::window_resize::ScreenRect {
        x: screen_origin.x,
        y: screen_origin.y,
        size: screen_size,
    };
    let margin = window
        .with_winit_window(|native| (24.0 * native.scale_factor()).round().max(0.0) as u32)
        .unwrap_or(0);
    let placement =
        riina_core::window_resize::centered_window_placement(target_frame, area, margin)?;
    Some(PhysicalPosition::new(placement.x, placement.y))
}

pub(crate) fn resize_centered(
    window: &slint::Window,
    current: PixelSize,
    target: PixelSize,
    overlay: Option<&slint::Window>,
) {
    #[cfg(not(target_os = "windows"))]
    let _ = overlay;
    if target == current || target.width == 0 || target.height == 0 {
        return;
    }
    #[cfg(target_os = "windows")]
    if window
        .with_winit_window(|native| {
            let Some(overlay) = overlay else {
                return crate::platform::resize_content(native, target, false, None).is_ok();
            };
            overlay
                .with_winit_window(|overlay| {
                    crate::platform::resize_content(native, target, false, Some(overlay)).is_ok()
                })
                .unwrap_or(false)
        })
        .unwrap_or(false)
    {
        window.request_redraw();
        return;
    }
    let frame = window
        .with_winit_window(|native| {
            let position = native.outer_position().ok()?;
            let outer = native.outer_size();
            let inner = native.inner_size();
            let frame_width = outer.width.saturating_sub(inner.width);
            let frame_height = outer.height.saturating_sub(inner.height);
            Some((
                PhysicalPosition::new(position.x, position.y),
                PixelSize {
                    width: outer.width,
                    height: outer.height,
                },
                PixelSize {
                    width: target.width.saturating_add(frame_width),
                    height: target.height.saturating_add(frame_height),
                },
            ))
        })
        .flatten();
    let (position, current_frame, target_frame) =
        frame.unwrap_or((window.position(), current, target));
    let bounds = screen_bounds(window, None);
    let margin = window
        .with_winit_window(|native| (24.0 * native.scale_factor()).round().max(0.0) as u32)
        .unwrap_or(0);
    window.set_size(PhysicalSize::new(target.width, target.height));
    #[cfg(target_os = "windows")]
    let target_frame = window
        .with_winit_window(|native| {
            let actual = native.outer_size();
            PixelSize {
                width: actual.width,
                height: actual.height,
            }
        })
        .unwrap_or(target_frame);
    let offset_x = i32::try_from(current_frame.width).unwrap_or(i32::MAX)
        - i32::try_from(target_frame.width).unwrap_or(i32::MAX);
    let offset_y = i32::try_from(current_frame.height).unwrap_or(i32::MAX)
        - i32::try_from(target_frame.height).unwrap_or(i32::MAX);
    let centered = PhysicalPosition::new(
        position.x.saturating_add(offset_x / 2),
        position.y.saturating_add(offset_y / 2),
    );
    let position = bounds
        .map(|(origin, size)| {
            let (x, y) = riina_core::window_resize::clamp_position_with_margin(
                centered.x,
                centered.y,
                target_frame,
                riina_core::window_resize::ScreenRect {
                    x: origin.x,
                    y: origin.y,
                    size,
                },
                margin,
            );
            PhysicalPosition::new(x, y)
        })
        .unwrap_or(centered);
    window.set_position(position);
    window.request_redraw();
}

#[cfg(test)]
mod tests {
    use super::media_resize_applies;
    use riina_core::window_resize::ResizeTiming;

    #[test]
    fn configured_resize_timing_gates_playlist_geometry_changes() {
        assert!(media_resize_applies(
            true,
            ResizeTiming::Never,
            false,
            false
        ));
        assert!(media_resize_applies(
            false,
            ResizeTiming::WhenOpenedManually,
            true,
            false
        ));
        assert!(!media_resize_applies(
            false,
            ResizeTiming::WhenOpenedManually,
            false,
            false
        ));
        assert!(!media_resize_applies(
            false,
            ResizeTiming::Never,
            true,
            false
        ));
        assert!(media_resize_applies(
            false,
            ResizeTiming::Always,
            false,
            false
        ));
    }

    #[test]
    fn cover_art_cannot_apply_main_video_geometry_in_music_mode() {
        for initial in [false, true] {
            for manual in [false, true] {
                for timing in [
                    ResizeTiming::Always,
                    ResizeTiming::WhenOpenedManually,
                    ResizeTiming::Never,
                ] {
                    assert!(!media_resize_applies(initial, timing, manual, true));
                }
            }
        }
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_automatic_playlist_changes_keep_the_current_native_video_surface() {
        use super::windows_media_resize_applies;

        assert!(windows_media_resize_applies(true, false));
        assert!(windows_media_resize_applies(false, true));
        assert!(!windows_media_resize_applies(false, false));
    }
}
