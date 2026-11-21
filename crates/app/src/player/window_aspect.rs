//! Apply the shared aspect policy to the winit window without coupling it to a host OS.

use crate::{NativeVideoHost, PlayerWindow, platform::window_sizing};
use riina_core::{
    window_aspect::{self as policy, PixelSize},
    window_resize::{ManualSizeCommand, ManualSizeContext, ResizeTiming, manual_size_target},
};
use riina_ui::PlayerMenuActions;
use slint::{
    ComponentHandle, PhysicalPosition, PhysicalSize,
    winit_030::{EventResult, WinitWindowAccessor, winit},
};
use std::{cell::Cell, rc::Rc, time::Duration};

/// Media identity for window sizing, shared by the playback observer and window owner.
#[derive(Clone, Default)]
pub(crate) struct MediaGeometry(Rc<Cell<Option<u64>>>);

impl MediaGeometry {
    pub(crate) fn generation(&self) -> Option<u64> {
        self.0.get()
    }

    pub(crate) fn observe(&self, generation: u64) -> Option<u64> {
        self.0.replace(Some(generation))
    }
}

pub(crate) fn connect(app: &PlayerWindow, source: &MediaGeometry) {
    let filter = connect_with_target(
        app,
        WindowTarget(None),
        pixel_size(app.window().size()),
        source,
    );
    app.window().on_winit_window_event(filter);
}

/// Bind the same portable aspect policy to the real surface when video is hosted
/// outside the shared chrome window.
#[cfg(target_os = "windows")]
pub(crate) fn connect_to_native_host(
    app: &PlayerWindow,
    host: &NativeVideoHost,
    source: &MediaGeometry,
) -> WindowEventFilter {
    connect_with_target(
        app,
        WindowTarget(Some(host.as_weak())),
        pixel_size(host.window().size()),
        source,
    )
}

pub(crate) type WindowEventFilter =
    Box<dyn Fn(&slint::Window, &winit::event::WindowEvent) -> EventResult>;

#[derive(Clone)]
struct WindowTarget(Option<slint::Weak<NativeVideoHost>>);

fn connect_with_target(
    app: &PlayerWindow,
    target: WindowTarget,
    initial_size: PixelSize,
    source: &MediaGeometry,
) -> WindowEventFilter {
    let previous_size = Rc::new(Cell::new(initial_size));
    let initial_geometry_applied = Rc::new(Cell::new(false));
    register_media_resize(
        app,
        target.clone(),
        previous_size.clone(),
        initial_geometry_applied.clone(),
    );
    register_window_level(app, &target);
    register_aspect_resize(
        app,
        target.clone(),
        previous_size.clone(),
        initial_geometry_applied,
        source.clone(),
    );
    register_fit_to_video(app, target.clone(), previous_size.clone());
    register_resize_command(app, target.clone(), previous_size.clone());
    resize_filter(app, previous_size, target)
}

fn register_media_resize(
    app: &PlayerWindow,
    resize_target: WindowTarget,
    media_resize_state: Rc<Cell<PixelSize>>,
    initial_state: Rc<Cell<bool>>,
) {
    let resize_generation = Rc::new(Cell::new(app.get_media_request_generation()));
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
                    app.global::<riina_ui::PlayerMusicModeState>()
                        .get_music_mode(),
                ) {
                    mark_window_size_adjusted(&app);
                    // The configured timing also governs automatic playlist
                    // changes. Reshaping for the new video's aspect here can
                    // clear the native Windows swap chain while the next
                    // decoder is starting, even when the resize option says
                    // "when opened manually" or "never".
                    return;
                }
                if !window_sizing::media_resize_allowed(initial, manually_opened) {
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
                        place_initial_media(
                            window,
                            &resize_target,
                            &app,
                            placement,
                            settings.initial_geometry.position.is_none(),
                        );
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
}

fn place_initial_media(
    window: &slint::Window,
    target: &WindowTarget,
    app: &PlayerWindow,
    placement: riina_core::window_resize::Placement,
    center: bool,
) {
    let positioned = center
        && window_sizing::resize_native_content(
            window,
            placement.size,
            true,
            resize_overlay(target, app),
        )
        .is_some_and(|result| result.is_ok());
    if !positioned {
        window.set_size(PhysicalSize::new(
            placement.size.width,
            placement.size.height,
        ));
        let position = PhysicalPosition::new(placement.x, placement.y);
        let position = if center {
            center_outer_frame(window, placement.size).unwrap_or(position)
        } else {
            position
        };
        window.set_position(position);
    }
    window.request_redraw();
}

fn register_window_level(app: &PlayerWindow, target: &WindowTarget) {
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
    apply_window_level(app, target);
}

fn register_aspect_resize(
    app: &PlayerWindow,
    action_target: WindowTarget,
    aspect_resize_state: Rc<Cell<PixelSize>>,
    aspect_initial_state: Rc<Cell<bool>>,
    media_geometry: MediaGeometry,
) {
    let aspect_pending = Rc::new(Cell::new(false));
    let aspect_source = Rc::new(Cell::new(None::<Option<u64>>));
    let aspect_request_generation = Rc::new(Cell::new(app.get_media_request_generation()));
    let aspect_resize_allowed = Rc::new(Cell::new(true));
    app.on_window_aspect_changed({
        let weak = app.as_weak();
        move || {
            let Some(app) = weak.upgrade() else { return };
            let action_target = action_target.clone();
            let aspect_resize_state = aspect_resize_state.clone();
            let aspect_source = aspect_source.clone();
            let media_geometry = media_geometry.clone();
            let aspect_request_generation = aspect_request_generation.clone();
            let aspect_resize_allowed = aspect_resize_allowed.clone();
            let aspect_initial_state = aspect_initial_state.clone();
            defer_window_update(&app, &aspect_pending, move |app| {
                with_target_window(&action_target, &app, |window| {
                    let current = pixel_size(window.size());
                    aspect_resize_state.set(current);
                    let source = media_geometry.generation();
                    if aspect_source.get() != Some(source) {
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
                            app.global::<riina_ui::PlayerMusicModeState>()
                                .get_music_mode(),
                        );
                        let resize_allowed = resize_allowed
                            && window_sizing::media_resize_allowed(initial, manually_opened);
                        aspect_resize_allowed.set(resize_allowed);
                        aspect_source.set(Some(source));
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
}

fn register_fit_to_video(
    app: &PlayerWindow,
    fit_target: WindowTarget,
    fit_state: Rc<Cell<PixelSize>>,
) {
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
}

fn register_resize_command(
    app: &PlayerWindow,
    menu_size_target: WindowTarget,
    menu_size_state: Rc<Cell<PixelSize>>,
) {
    app.global::<PlayerMenuActions>().on_resize_window_command({
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
                        use_physical_resolution: app
                            .global::<riina_ui::PlayerPreferences>()
                            .get_use_physical_resolution(),
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
                    // The Windows adapter measures the menu and moves grouped surfaces together.
                    if let Some(result) = window_sizing::resize_native_content(
                        window,
                        size,
                        true,
                        resize_overlay(&menu_size_target, &app),
                    ) {
                        if let Err(error) = result {
                            app.set_status(error.into());
                            return;
                        }
                    } else {
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
                    }
                } else {
                    resize_centered(window, current, size, None);
                }
                menu_size_state.set(size);
            });
        }
    });
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
    if let Some(host) = &target.0 {
        if let Some(host) = host.upgrade() {
            action(host.window());
        }
    } else {
        action(app.window());
    }
}

fn resize_overlay<'a>(target: &WindowTarget, app: &'a PlayerWindow) -> Option<&'a slint::Window> {
    target.0.is_some().then_some(app.window())
}

fn resize_filter(
    app: &PlayerWindow,
    previous_size: Rc<Cell<PixelSize>>,
    target: WindowTarget,
) -> WindowEventFilter {
    let native_sizing = window_sizing::install(app, target.0);
    let weak = app.as_weak();
    Box::new(move |window, event| {
        let _native_sizing = &native_sizing;
        let winit::event::WindowEvent::Resized(size) = event else {
            return EventResult::Propagate;
        };
        let requested = PixelSize {
            width: size.width,
            height: size.height,
        };
        if !window_sizing::USES_NATIVE_RESIZE_FILTER
            && let Some(app) = weak.upgrade()
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
        && !app
            .global::<riina_ui::PlayerMusicModeState>()
            .get_music_mode()
        && app.get_edge_to_edge_video()
        && !app.get_unlock_window_aspect_ratio()
        && f64::from(app.get_video_display_aspect_ratio()).is_finite()
        && app.get_video_display_aspect_ratio() > 0.0
}

fn window_limits(window: &slint::Window) -> Option<(Option<PixelSize>, bool)> {
    let can_resize = window
        .with_winit_window(|native| !native.is_maximized() && native.fullscreen().is_none())?;
    let maximum = window_sizing::content_limit(window)?;
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
    let target_frame = window_sizing::initial_frame_size(window, content_size)?;
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
    if target == current || target.width == 0 || target.height == 0 {
        return;
    }
    if window_sizing::resize_native_content(window, target, false, overlay)
        .is_some_and(|result| result.is_ok())
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
    let target_frame = window_sizing::resized_frame_size(window, target_frame);
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
    use super::{MediaGeometry, media_resize_applies};

    #[test]
    fn window_geometry_owners_share_the_full_native_generation() {
        let source = MediaGeometry::default();
        let frames = source.clone();
        let generation = (1_u64 << 53) + 1;
        assert_eq!(frames.observe(generation), None);
        assert_eq!(source.generation(), Some(generation));
        assert_eq!(frames.observe(generation + 1), Some(generation));
        assert_eq!(source.generation(), Some(generation + 1));
    }
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
        use crate::platform::window_sizing::media_resize_allowed as windows_media_resize_applies;

        assert!(windows_media_resize_applies(true, false));
        assert!(windows_media_resize_applies(false, true));
        assert!(!windows_media_resize_applies(false, false));
    }
}
