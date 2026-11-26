//! AppKit already exposes the outer frame in the coordinates plugins expect.
use super::{Frame, Observation, Screen};
use crate::platform::macos::fullscreen::native_window;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSScreen, NSWindowOcclusionState};
use objc2_foundation::{NSNumber, NSPoint, NSRect, NSSize, ns_string};
use slint::winit_030::{WinitWindowAccessor, winit::dpi::LogicalSize};

pub(crate) fn observe(window: &slint::Window) -> Option<Observation> {
    let window = native_window(window)?;
    let current = window.screen().as_deref().and_then(screen_id);
    let main = NSScreen::mainScreen(MainThreadMarker::new()?)
        .as_deref()
        .and_then(screen_id);
    let screens = NSScreen::screens(MainThreadMarker::new()?)
        .iter()
        .map(|screen| {
            let id = screen_id(&screen);
            Screen {
                frame: frame(screen.frame()),
                main: id.is_some() && id == main,
                current: id.is_some() && id == current,
            }
        })
        .collect();
    Some(Observation {
        frame: frame(window.frame()),
        current_screen: current.map(|id| id.to_string()),
        screens,
        visible: Some(
            window
                .occlusionState()
                .contains(NSWindowOcclusionState::Visible),
        ),
        minimized: Some(window.isMiniaturized()),
        main: window.isMainWindow(),
    })
}

pub(crate) fn set_frame(window: &slint::Window, frame: Frame) -> Result<(), String> {
    if let Some(native) = native_window(window)
        && let (Some(x), Some(y)) = (frame.x, frame.y)
    {
        window.with_winit_window(|window| {
            super::resize_request::expect_outer(
                window,
                LogicalSize::new(frame.width, frame.height).to_physical(window.scale_factor()),
            );
        });
        native.setFrame_display(
            NSRect::new(NSPoint::new(x, y), NSSize::new(frame.width, frame.height)),
            true,
        );
    }
    Ok(())
}

pub(crate) fn set_minimized(window: &slint::Window, minimized: bool) {
    if let Some(window) = native_window(window) {
        if minimized {
            window.miniaturize(None);
        } else {
            window.deminiaturize(None);
        }
    }
}

fn frame(frame: NSRect) -> Frame {
    Frame::positioned(
        frame.origin.x,
        frame.origin.y,
        frame.size.width,
        frame.size.height,
    )
}

fn screen_id(screen: &NSScreen) -> Option<u32> {
    screen
        .deviceDescription()
        .objectForKey(ns_string!("NSScreenNumber"))?
        .downcast::<NSNumber>()
        .ok()
        .map(|number| number.unsignedIntValue())
}
