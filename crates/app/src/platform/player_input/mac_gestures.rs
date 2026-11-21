//! Use the trackpad's native pressure and pinch phases for player actions.
use crate::{PlayerWindow, platform};
use riina_mpv::CommandSender;
use slint::{ComponentHandle, winit_030::WinitWindowAccessor};
use std::cell::Cell;

fn resize_for_trackpad_pinch(window: &slint::Window, delta: f64) {
    use slint::winit_030::winit::dpi::{PhysicalPosition, PhysicalSize};

    if !delta.is_finite() || delta <= -1.0 {
        return;
    }
    let _ = window.with_winit_window(|native| {
        if native.fullscreen().is_some() || native.is_maximized() {
            return;
        }
        let current = native.inner_size();
        if current.width == 0 || current.height == 0 {
            return;
        }
        let scale = native.scale_factor();
        let monitor = native.current_monitor().map(|monitor| monitor.size());
        let min_width = (285.0 * scale).round().max(1.0) as u32;
        let min_height = (120.0 * scale).round().max(1.0) as u32;
        let (max_width, max_height) = monitor.map_or((u32::MAX, u32::MAX), |size| {
            (
                (f64::from(size.width) * 0.98).round() as u32,
                (f64::from(size.height) * 0.94).round() as u32,
            )
        });
        let lower = (f64::from(min_width) / f64::from(current.width))
            .max(f64::from(min_height) / f64::from(current.height));
        let upper = (f64::from(max_width) / f64::from(current.width))
            .min(f64::from(max_height) / f64::from(current.height));
        if !upper.is_finite() || upper < lower {
            return;
        }
        let factor = (1.0 + delta).clamp(lower, upper);
        let size = PhysicalSize::new(
            (f64::from(current.width) * factor).round() as u32,
            (f64::from(current.height) * factor).round() as u32,
        );
        let position = native.outer_position().ok();
        if let Some(position) = position {
            let width_change = i64::from(size.width) - i64::from(current.width);
            let height_change = i64::from(size.height) - i64::from(current.height);
            native.set_outer_position(PhysicalPosition::new(
                (i64::from(position.x) - width_change / 2)
                    .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
                (i64::from(position.y) - height_change / 2)
                    .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
            ));
        }
        let _ = native.request_inner_size(size);
    });
}

pub(super) fn install(
    app: &PlayerWindow,
    player: CommandSender,
    dispatch: fn(&PlayerWindow, &CommandSender, i32),
) {
    let weak = app.as_weak();
    let force_touch_active = Cell::new(false);
    let mut pinch_fullscreen_direction = None::<bool>;
    platform::install_control_gestures(app.window(), move |gesture| {
        let Some(app) = weak.upgrade() else { return };
        match gesture {
            platform::ControlGesture::Pressure { stage } => {
                if stage < 2 {
                    force_touch_active.set(false);
                } else if stage == 2 && !force_touch_active.replace(true) {
                    dispatch(&app, &player, app.get_control_force_touch_index());
                }
            }
            platform::ControlGesture::Pinch { delta, phase } => {
                use platform::GesturePhase;
                use riina_core::control_preferences::PinchAction;

                if app.get_crop_editing() || app.get_dialog_open() {
                    return;
                }
                let Ok(settings) = crate::control_preferences::current_player(&app) else {
                    return;
                };
                match settings.pinch_action {
                    PinchAction::None => {}
                    PinchAction::Fullscreen => {
                        if phase == GesturePhase::Started {
                            pinch_fullscreen_direction = None;
                        }
                        if matches!(phase, GesturePhase::Started | GesturePhase::Moved)
                            && delta.is_finite()
                            && delta != 0.0
                        {
                            let wants_fullscreen = delta > 0.0;
                            if pinch_fullscreen_direction.is_none()
                                && wants_fullscreen != app.get_fullscreen()
                            {
                                app.invoke_toggle_fullscreen();
                            }
                            pinch_fullscreen_direction = Some(wants_fullscreen);
                        }
                        if matches!(phase, GesturePhase::Ended | GesturePhase::Cancelled) {
                            pinch_fullscreen_direction = None;
                        }
                    }
                    PinchAction::AdjustWindowSize => {
                        if app.get_fullscreen() {
                            return;
                        }
                        if matches!(phase, GesturePhase::Started | GesturePhase::Moved) {
                            resize_for_trackpad_pinch(app.window(), delta);
                        } else if matches!(phase, GesturePhase::Ended | GesturePhase::Cancelled) {
                            app.invoke_window_aspect_changed();
                        }
                    }
                }
            }
        }
    });
}
