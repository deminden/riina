//! Recover mouse-event fields omitted by Winit while the AppKit event is current.
use super::view::{content_view, top_left_point};
use crate::platform::player_input::NativeMouseEvent;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSEventType};
use slint::winit_030::winit::{
    event::{ElementState, MouseButton, WindowEvent},
    window::Window,
};

enum MouseEventKind {
    Button(ElementState),
    Drag,
    Move,
}

pub(in crate::platform) fn mouse_details(
    window: &Window,
    event: &WindowEvent,
) -> Option<NativeMouseEvent> {
    let main = MainThreadMarker::new()?;
    let view = content_view(window)?;
    let owner = view.window()?;
    let native = NSApplication::sharedApplication(main).currentEvent()?;
    if native.window(main).as_deref().map(std::ptr::from_ref) != Some(std::ptr::from_ref(&*owner)) {
        return None;
    }
    let kind = match native.r#type() {
        NSEventType::LeftMouseDown | NSEventType::RightMouseDown | NSEventType::OtherMouseDown => {
            MouseEventKind::Button(ElementState::Pressed)
        }
        NSEventType::LeftMouseUp | NSEventType::RightMouseUp | NSEventType::OtherMouseUp => {
            MouseEventKind::Button(ElementState::Released)
        }
        NSEventType::LeftMouseDragged
        | NSEventType::RightMouseDragged
        | NSEventType::OtherMouseDragged => MouseEventKind::Drag,
        NSEventType::MouseMoved => MouseEventKind::Move,
        _ => return None,
    };
    let point = native.locationInWindow();
    let (button, click_count) = match (event, kind) {
        (WindowEvent::CursorMoved { position, .. }, kind) => {
            let (x, y) = top_left_point(view, point);
            let scale = window.scale_factor();
            if (position.x - x * scale).abs() > 0.5 || (position.y - y * scale).abs() > 0.5 {
                return None;
            }
            // Winit also emits a cursor update immediately before button events.
            // Only a native drag should produce a plugin drag callback.
            (
                matches!(kind, MouseEventKind::Drag).then(|| button(native.buttonNumber())),
                None,
            )
        }
        (
            WindowEvent::MouseInput {
                state,
                button: expected,
                ..
            },
            MouseEventKind::Button(native_state),
        ) if *state == native_state && *expected == button(native.buttonNumber()) => {
            (Some(*expected), u32::try_from(native.clickCount()).ok())
        }
        _ => return None,
    };
    let pressure = if button.is_some() {
        native.pressure()
    } else {
        0.0
    };
    Some(NativeMouseEvent {
        button,
        window_position: (point.x as f32, point.y as f32),
        click_count,
        pressure: if pressure.is_finite() { pressure } else { 0.0 },
    })
}

fn button(number: isize) -> MouseButton {
    match number {
        0 => MouseButton::Left,
        1 => MouseButton::Right,
        2 => MouseButton::Middle,
        3 => MouseButton::Back,
        4 => MouseButton::Forward,
        value => MouseButton::Other(value as u16),
    }
}
