use block2::RcBlock;
use objc2::{MainThreadMarker, rc::Retained, runtime::AnyObject};
use objc2_app_kit::{NSEvent, NSEventMask};
use std::{ptr::NonNull, rc::Rc};

#[derive(Clone, Copy, Debug)]
pub(crate) struct NativeScrollEvent {
    pub(crate) delta_x: f64,
    pub(crate) delta_y: f64,
    pub(crate) precise: bool,
    pub(crate) natural_direction: bool,
    pub(crate) phase_empty: bool,
    pub(crate) phase_began: bool,
    pub(crate) phase_ended: bool,
    pub(crate) momentum_phase_empty: bool,
}

pub(crate) struct MacScrollEventMonitor {
    token: Retained<AnyObject>,
    _handler: RcBlock<dyn Fn(NonNull<NSEvent>) -> *mut NSEvent>,
}

impl MacScrollEventMonitor {
    pub(crate) fn new(callback: impl Fn(NativeScrollEvent) + 'static) -> Result<Self, String> {
        if MainThreadMarker::new().is_none() {
            return Err("macOS scroll monitoring must be installed on the main thread".into());
        }
        let callback = Rc::new(callback);
        let handler: RcBlock<dyn Fn(NonNull<NSEvent>) -> *mut NSEvent> =
            RcBlock::new(move |event: NonNull<NSEvent>| {
                // SAFETY: AppKit passes a live scroll event for the duration of this
                // local-monitor callback. It is returned unchanged to normal dispatch.
                let event_ref = unsafe { event.as_ref() };
                let phase = event_ref.phase();
                let momentum_phase = event_ref.momentumPhase();
                callback(NativeScrollEvent {
                    delta_x: event_ref.scrollingDeltaX(),
                    delta_y: event_ref.scrollingDeltaY(),
                    precise: event_ref.hasPreciseScrollingDeltas(),
                    natural_direction: event_ref.isDirectionInvertedFromDevice(),
                    phase_empty: phase.is_empty(),
                    phase_began: phase.contains(objc2_app_kit::NSEventPhase::Began),
                    phase_ended: phase.contains(objc2_app_kit::NSEventPhase::Ended),
                    momentum_phase_empty: momentum_phase.is_empty(),
                });
                event.as_ptr()
            });
        // SAFETY: AppKit calls this monitor synchronously on the main thread. Its
        // block is retained with the returned token and returns the original event.
        let token = unsafe {
            NSEvent::addLocalMonitorForEventsMatchingMask_handler(
                NSEventMask::ScrollWheel,
                &handler,
            )
        }
        .ok_or_else(|| "Could not install the native macOS scroll monitor".to_owned())?;
        Ok(Self {
            token,
            _handler: handler,
        })
    }
}

impl Drop for MacScrollEventMonitor {
    fn drop(&mut self) {
        // SAFETY: this token came from NSEvent's local-monitor registration and is
        // removed on the main thread when the owning player window is torn down.
        unsafe { NSEvent::removeMonitor(&self.token) };
    }
}
