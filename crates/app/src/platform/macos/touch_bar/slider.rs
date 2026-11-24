//! Keep AppKit touch tracking and the player's temporary pause in one session.
use super::preview::{self, Preview};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, rc::Retained};
use objc2_app_kit::{NSEvent, NSSlider, NSTouchTypeMask};
use objc2_foundation::NSRect;
use std::{cell::Cell, rc::Rc};

pub(super) struct Tracking {
    pub(super) active: Rc<Cell<bool>>,
    changed: Box<dyn Fn(bool)>,
}

impl Tracking {
    pub(super) fn new(changed: impl Fn(bool) + 'static) -> Self {
        Self {
            active: Rc::default(),
            changed: Box::new(changed),
        }
    }

    fn set(&self, active: bool) {
        if self.active.replace(active) != active {
            (self.changed)(active);
        }
    }
}

define_class!(
    #[unsafe(super = NSSlider)]
    #[thread_kind = MainThreadOnly]
    #[ivars = Rc<Tracking>]
    pub(super) struct PlayerBarSlider;

    impl PlayerBarSlider {
        #[unsafe(method(touchesBeganWithEvent:))]
        fn begin_touch(&self, event: &NSEvent) {
            self.ivars().set(true);
            // SAFETY: continue AppKit slider tracking for the same delivered event.
            unsafe { objc2::msg_send![super(self), touchesBeganWithEvent: event] }
        }

        #[unsafe(method(touchesEndedWithEvent:))]
        fn end_touch(&self, event: &NSEvent) {
            // SAFETY: give AppKit the final sample before ending the scrub session.
            unsafe { objc2::msg_send![super(self), touchesEndedWithEvent: event] }
            self.ivars().set(false);
        }

        #[unsafe(method(touchesCancelledWithEvent:))]
        fn cancel_touch(&self, event: &NSEvent) {
            // SAFETY: AppKit must retire the same event from the inherited tracker.
            unsafe { objc2::msg_send![super(self), touchesCancelledWithEvent: event] }
            self.ivars().set(false);
        }
    }
);

pub(super) fn scrubber(
    marker: MainThreadMarker,
    tracking: Rc<Tracking>,
    thumbnails: Rc<Preview>,
) -> Retained<PlayerBarSlider> {
    let active = tracking.active.clone();
    let slider = PlayerBarSlider::alloc(marker).set_ivars(tracking);
    // SAFETY: initialize the allocated NSSlider subclass with its state already set.
    let slider: Retained<PlayerBarSlider> =
        unsafe { objc2::msg_send![super(slider), initWithFrame: NSRect::ZERO] };
    preview::install(&slider, thumbnails, active);
    slider.setMinValue(0.0);
    slider.setMaxValue(1.0);
    slider.setContinuous(true);
    slider.setAllowedTouchTypes(NSTouchTypeMask::Direct | NSTouchTypeMask::Indirect);
    slider
}
