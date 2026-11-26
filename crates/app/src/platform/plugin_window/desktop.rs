//! Translate between desktop pixels and the proxy's outer frame coordinates.
#[cfg(windows)]
mod win32;

use super::{Frame, Observation, Screen};
use slint::winit_030::{
    WinitWindowAccessor,
    winit::{
        dpi::{PhysicalPosition, PhysicalSize},
        monitor::MonitorHandle,
        window::Window,
    },
};
use std::collections::BTreeSet;

pub(crate) fn observe(window: &slint::Window) -> Option<Observation> {
    window.with_winit_window(|window| {
        let current = window.current_monitor();
        let primary = window.primary_monitor();
        let mut seen = BTreeSet::new();
        let monitors = window
            .available_monitors()
            .filter(|monitor| seen.insert(screen_identity(monitor)))
            .collect::<Vec<_>>();
        let origin = primary.as_ref().or(monitors.first()).map(primary_bottom);
        let frame = logical_frame(
            window.outer_position().ok(),
            window.outer_size(),
            window.scale_factor(),
            origin,
        );
        let screens = monitors
            .into_iter()
            .map(|monitor| Screen {
                frame: logical_frame(
                    Some(monitor.position()),
                    monitor.size(),
                    monitor.scale_factor(),
                    origin,
                ),
                main: primary.as_ref() == Some(&monitor),
                current: current.as_ref() == Some(&monitor),
            })
            .collect();
        let minimized = window.is_minimized();
        let visible = if minimized == Some(true) {
            Some(false)
        } else {
            window.is_visible()
        };
        Observation {
            frame,
            screens,
            current_screen: current.as_ref().map(screen_identity),
            visible,
            minimized,
            main: window.has_focus(),
        }
    })
}

pub(crate) fn set_frame(window: &slint::Window, frame: Frame) -> Result<(), String> {
    window
        .with_winit_window(|window| {
            let scale = window.scale_factor();
            let size = PhysicalSize::new(
                (frame.width * scale)
                    .round()
                    .clamp(1.0, f64::from(i32::MAX)) as u32,
                (frame.height * scale)
                    .round()
                    .clamp(1.0, f64::from(i32::MAX)) as u32,
            );
            let origin = window
                .primary_monitor()
                .or_else(|| window.available_monitors().next());
            let position = window.outer_position().ok().and_then(|_| {
                physical_position(frame, size.height, scale, primary_bottom(&origin?))
            });
            apply_frame(window, position, size)
        })
        .unwrap_or(Ok(()))
}

pub(crate) fn set_minimized(window: &slint::Window, minimized: bool) {
    window.with_winit_window(|window| window.set_minimized(minimized));
}

fn primary_bottom(monitor: &MonitorHandle) -> f64 {
    f64::from(monitor.position().y) + f64::from(monitor.size().height)
}

fn logical_frame(
    position: Option<PhysicalPosition<i32>>,
    size: PhysicalSize<u32>,
    scale: f64,
    primary_bottom: Option<f64>,
) -> Frame {
    let logical = size.to_logical::<f64>(scale);
    Frame {
        x: position.map(|position| f64::from(position.x) / scale),
        y: position.zip(primary_bottom).map(|(position, bottom)| {
            (bottom - f64::from(position.y) - f64::from(size.height)) / scale
        }),
        width: logical.width,
        height: logical.height,
    }
}

fn physical_position(
    frame: Frame,
    height: u32,
    scale: f64,
    bottom: f64,
) -> Option<PhysicalPosition<i32>> {
    Some(PhysicalPosition::new(
        (frame.x? * scale).round() as i32,
        (bottom - frame.y? * scale - f64::from(height)).round() as i32,
    ))
}

#[cfg(windows)]
fn screen_identity(monitor: &MonitorHandle) -> String {
    use slint::winit_030::winit::platform::windows::MonitorHandleExtWindows;
    monitor.native_id()
}

#[cfg(target_os = "linux")]
fn screen_identity(monitor: &MonitorHandle) -> String {
    use slint::winit_030::winit::platform::wayland::MonitorHandleExtWayland;
    // Both Winit Linux backends use the compositor or XRandR output ID.
    monitor.native_id().to_string()
}

#[cfg(not(any(windows, target_os = "linux")))]
fn screen_identity(monitor: &MonitorHandle) -> String {
    monitor.name().unwrap_or_default()
}

#[cfg(windows)]
use win32::apply_frame;

#[cfg(not(windows))]
fn apply_frame(
    window: &Window,
    position: Option<PhysicalPosition<i32>>,
    outer: PhysicalSize<u32>,
) -> Result<(), String> {
    let size = super::resize_request::expect_outer(window, outer);
    // Under Wayland the compositor owns position and can constrain this size.
    let _ = window.request_inner_size(size);
    if let Some(position) = position {
        window.set_outer_position(position);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_frame_preserves_outer_size_and_round_trips_a_negative_origin() {
        let size = PhysicalSize::new(960, 600);
        let position = PhysicalPosition::new(-200, -120);
        let frame = logical_frame(Some(position), size, 1.5, Some(1080.0));
        assert_eq!(frame.width, 640.0);
        assert_eq!(frame.height, 400.0);
        assert_eq!(frame.y, Some(400.0));
        assert_eq!(
            physical_position(frame, size.height, 1.5, 1080.0),
            Some(position)
        );
        let unknown = logical_frame(None, size, 1.5, Some(1080.0));
        assert_eq!(unknown.x, None);
        assert_eq!(unknown.y, None);
        assert_eq!(unknown.width, frame.width);
    }
}
