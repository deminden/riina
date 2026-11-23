use slint::winit_030::winit;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum GesturePhase {
    Started,
    Moved,
    Ended,
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum ControlGesture {
    Pinch { delta: f64, phase: GesturePhase },
    Pressure { stage: i64 },
}

pub(crate) fn control_gesture(event: &winit::event::WindowEvent) -> Option<ControlGesture> {
    match event {
        winit::event::WindowEvent::PinchGesture { delta, phase, .. } => {
            Some(ControlGesture::Pinch {
                delta: *delta,
                phase: match phase {
                    winit::event::TouchPhase::Started => GesturePhase::Started,
                    winit::event::TouchPhase::Moved => GesturePhase::Moved,
                    winit::event::TouchPhase::Ended => GesturePhase::Ended,
                    winit::event::TouchPhase::Cancelled => GesturePhase::Cancelled,
                },
            })
        }
        winit::event::WindowEvent::TouchpadPressure { stage, .. } => {
            Some(ControlGesture::Pressure { stage: *stage })
        }
        _ => None,
    }
}
