//! Keep pointer ownership and native metadata with the player that received them.
use crate::{
    platform,
    plugin_runtime::input::{Arguments, Kind},
};
use riina_ui::PlayerPluginPointerPhase;
use slint::winit_030::winit::event::{ElementState, MouseButton, WindowEvent};
use std::{cell::RefCell, rc::Rc, time::Instant};

const CODES: [&str; 3] = ["*mouse", "*rightMouse", "*otherMouse"];

#[derive(Clone, Default)]
pub(in crate::player::player_ui) struct PointerInput(Rc<RefCell<MouseState>>);

impl PointerInput {
    pub(in crate::player::player_ui) fn observe(
        &self,
        window: &slint::Window,
        event: &WindowEvent,
    ) {
        match event {
            WindowEvent::CursorMoved { .. } => {
                let sample = platform::native_mouse_details(window, event);
                self.0.borrow_mut().native_motion = sample;
            }
            WindowEvent::MouseInput { state, .. } => {
                let sample = platform::native_mouse_details(window, event);
                self.0.borrow_mut().native_button = sample.map(|sample| (*state, sample));
            }
            WindowEvent::Focused(false) => self.reset(),
            _ => {}
        }
    }

    pub(super) fn reset(&self) {
        *self.0.borrow_mut() = MouseState::default();
    }

    pub(super) fn event(
        &self,
        phase: PlayerPluginPointerPhase,
        code: &str,
        position: (f32, f32),
        recognized_clicks: i32,
    ) -> Option<(Kind, &'static str, Arguments)> {
        self.0
            .borrow_mut()
            .event(phase, code, position, recognized_clicks)
    }
}

#[derive(Default)]
struct MouseState {
    buttons: [Button; 3],
    position: Option<(f32, f32)>,
    // Slint can deliver a buffered move just before the following button event.
    native_motion: Option<platform::NativeMouseEvent>,
    native_button: Option<(ElementState, platform::NativeMouseEvent)>,
}

#[derive(Default)]
struct Button {
    last_down: Option<(Instant, (f32, f32))>,
    clicks: u32,
    pressed: bool,
}

impl MouseState {
    fn event(
        &mut self,
        phase: PlayerPluginPointerPhase,
        code: &str,
        position: (f32, f32),
        recognized_clicks: i32,
    ) -> Option<(Kind, &'static str, Arguments)> {
        let moved = self.position.replace(position) != Some(position);
        match phase {
            PlayerPluginPointerPhase::Cancel => {
                *self = Self::default();
                None
            }
            PlayerPluginPointerPhase::Move => self.motion(position, moved),
            PlayerPluginPointerPhase::Down | PlayerPluginPointerPhase::Up => {
                self.button(phase, code, position, recognized_clicks)
            }
        }
    }

    fn button(
        &mut self,
        phase: PlayerPluginPointerPhase,
        code: &str,
        position: (f32, f32),
        recognized_clicks: i32,
    ) -> Option<(Kind, &'static str, Arguments)> {
        let native = self.native_button.take();
        self.native_motion = None;
        let index = CODES.iter().position(|key| *key == code)?;
        let pressed = phase == PlayerPluginPointerPhase::Down;
        let native = native.and_then(|(state, sample)| {
            (state.is_pressed() == pressed && sample.button.map(button_index) == Some(index))
                .then_some(sample)
        });
        let native_clicks = native
            .and_then(|sample| sample.click_count)
            .filter(|&count| count != 0);
        let button = &mut self.buttons[index];
        let (kind, clicks) = if pressed {
            button.press(position, native_clicks);
            (Kind::MouseDown, button.clicks)
        } else {
            let recognized = u32::try_from(recognized_clicks).unwrap_or_default().max(1);
            let pressed = std::mem::take(&mut button.pressed);
            let fallback = if pressed {
                button.clicks.max(recognized)
            } else {
                recognized
            };
            (Kind::MouseUp, native_clicks.unwrap_or(fallback))
        };
        Some((kind, CODES[index], arguments(position, clicks, native)))
    }

    fn motion(
        &mut self,
        position: (f32, f32),
        moved: bool,
    ) -> Option<(Kind, &'static str, Arguments)> {
        let native = self.native_motion.take();
        let index = if let Some(sample) = native {
            button_index(sample.button?)
        } else {
            if !moved {
                return None;
            }
            self.buttons
                .iter()
                .enumerate()
                .filter(|(_, button)| button.pressed)
                .max_by_key(|(_, button)| button.last_down.map(|(time, _)| time))?
                .0
        };
        let button = &self.buttons[index];
        button.pressed.then(|| {
            (
                Kind::MouseDrag,
                CODES[index],
                arguments(position, button.clicks, native),
            )
        })
    }
}

impl Button {
    fn press(&mut self, position: (f32, f32), native_clicks: Option<u32>) {
        let now = Instant::now();
        self.clicks = native_clicks.unwrap_or_else(|| {
            let nearby_click = self.last_down.is_some_and(|(previous, (x, y))| {
                now.duration_since(previous) <= platform::double_click_interval()
                    && (position.0 - x).abs() <= 4.0
                    && (position.1 - y).abs() <= 4.0
            });
            if nearby_click {
                self.clicks.saturating_add(1)
            } else {
                1
            }
        });
        self.last_down = Some((now, position));
        self.pressed = true;
    }
}

fn button_index(button: MouseButton) -> usize {
    match button {
        MouseButton::Left => 0,
        MouseButton::Right => 1,
        _ => 2,
    }
}

fn arguments(
    position: (f32, f32),
    click_count: u32,
    native: Option<platform::NativeMouseEvent>,
) -> Arguments {
    let ((x, y), pressure) = native.map_or((position, 0.0), |sample| {
        (sample.window_position, sample.pressure)
    });
    Arguments::Mouse {
        x,
        y,
        click_count,
        pressure,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use PlayerPluginPointerPhase::{Cancel, Down, Move, Up};

    fn sample(
        button: Option<MouseButton>,
        clicks: Option<u32>,
        pressure: f32,
    ) -> platform::NativeMouseEvent {
        platform::NativeMouseEvent {
            button,
            click_count: clicks,
            pressure,
            window_position: (52.5, 461.0),
        }
    }

    fn payload(
        event: Option<(Kind, &'static str, Arguments)>,
        kind: Kind,
        code: &str,
    ) -> serde_json::Value {
        let (actual, button, arguments) = event.unwrap();
        assert_eq!((actual, button), (kind, code));
        serde_json::to_value(arguments).unwrap()
    }

    #[test]
    fn native_motion_and_button_metadata_stay_with_their_own_event() {
        let mut input = MouseState {
            native_button: Some((
                ElementState::Pressed,
                sample(Some(MouseButton::Left), Some(3), 0.25),
            )),
            ..MouseState::default()
        };
        let position = (51.0, 459.0);
        let press = payload(
            input.event(Down, CODES[0], position, 0),
            Kind::MouseDown,
            CODES[0],
        );
        assert_eq!(
            press,
            serde_json::json!({"x":52.5, "y":461.0, "clickCount":3, "pressure":0.25})
        );
        input.event(Down, CODES[1], position, 0);

        input.native_motion = Some(sample(Some(MouseButton::Left), None, 0.625));
        input.native_button = Some((
            ElementState::Released,
            sample(Some(MouseButton::Right), Some(2), 0.125),
        ));
        let drag = payload(
            input.event(Move, CODES[2], position, 0),
            Kind::MouseDrag,
            CODES[0],
        );
        assert_eq!(drag["pressure"], 0.625);
        assert_eq!(drag["clickCount"], 3);
        let release = payload(
            input.event(Up, CODES[1], position, 1),
            Kind::MouseUp,
            CODES[1],
        );
        assert_eq!(release["pressure"], 0.125);
        assert_eq!(release["clickCount"], 2);

        input.native_motion = Some(sample(None, None, 0.0));
        assert!(input.event(Move, CODES[2], (55.0, 459.0), 0).is_none());
        input.native_motion = Some(sample(Some(MouseButton::Right), None, 0.5));
        assert!(input.event(Move, CODES[2], (56.0, 459.0), 0).is_none());
    }

    #[test]
    fn cancellation_and_unmatched_native_samples_preserve_portable_input() {
        let mut input = MouseState {
            native_button: Some((
                ElementState::Pressed,
                sample(Some(MouseButton::Left), Some(3), 0.5),
            )),
            ..MouseState::default()
        };
        let position = (35.0, 40.0);
        input.event(Down, CODES[0], position, 0);
        let release = payload(
            input.event(Up, CODES[0], position, 1),
            Kind::MouseUp,
            CODES[0],
        );
        assert_eq!(release["clickCount"], 3);
        assert_eq!(release["pressure"], 0.0);

        input.event(Cancel, "", position, 0);
        input.native_button = Some((
            ElementState::Pressed,
            sample(Some(MouseButton::Right), Some(4), 0.5),
        ));
        let press = payload(
            input.event(Down, CODES[0], position, 0),
            Kind::MouseDown,
            CODES[0],
        );
        assert_eq!(
            press,
            serde_json::json!({"x":35.0, "y":40.0, "clickCount":1, "pressure":0.0})
        );
        assert!(input.event(Move, CODES[2], position, 0).is_none());
        assert!(input.event(Move, CODES[2], (36.0, 40.0), 0).is_some());
        input.event(Cancel, "", position, 0);
        assert!(input.event(Move, CODES[2], (37.0, 40.0), 0).is_none());
    }
}
