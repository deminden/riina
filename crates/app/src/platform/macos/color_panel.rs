//! Use AppKit's shared color panel for macOS subtitle color wells.

use objc2::{
    DefinedClass, MainThreadMarker, MainThreadOnly, define_class,
    rc::Retained,
    runtime::{AnyObject, NSObject},
    sel,
};
use objc2_app_kit::{NSColor, NSColorPanel, NSColorSpace};
use objc2_foundation::NSObjectProtocol;
use std::{cell::Cell, ptr, rc::Rc};

thread_local! {
    static ACTIVE_TARGET: Cell<*const ColorPanelTarget> = const { Cell::new(ptr::null()) };
}

struct ColorPanelTargetIvars {
    changed: Rc<dyn Fn([u8; 4])>,
}

define_class!(
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = ColorPanelTargetIvars]
    struct ColorPanelTarget;

    // SAFETY: NSObject provides the protocol implementation for this subclass.
    unsafe impl NSObjectProtocol for ColorPanelTarget {}

    impl ColorPanelTarget {
        #[unsafe(method(colorPanelDidChange:))]
        fn color_panel_did_change(&self, panel: &NSColorPanel) {
            if ACTIVE_TARGET.with(|target| ptr::eq(target.get(), self))
                && let Some(channels) = rgba_channels(&panel.color())
            {
                let changed = self.ivars().changed.clone();
                changed(channels);
            }
        }
    }
);

/// Keeps the shared panel's weak target alive while the requesting window owns it.
pub(crate) struct MacSubtitleColorPanel {
    target: Retained<ColorPanelTarget>,
}

impl MacSubtitleColorPanel {
    pub(crate) fn open(
        initial: [u8; 4],
        changed: impl Fn([u8; 4]) + 'static,
    ) -> Result<Self, String> {
        let main_thread = MainThreadMarker::new()
            .ok_or_else(|| "The macOS color panel must open on the main thread".to_owned())?;
        let panel = NSColorPanel::sharedColorPanel(main_thread);
        let initial = NSColor::colorWithSRGBRed_green_blue_alpha(
            f64::from(initial[0]) / 255.0,
            f64::from(initial[1]) / 255.0,
            f64::from(initial[2]) / 255.0,
            f64::from(initial[3]) / 255.0,
        );
        // Seeding the panel must never send the new well's color to its old owner.
        ACTIVE_TARGET.with(|target| target.set(ptr::null()));
        detach(&panel);
        panel.setShowsAlpha(true);
        panel.setContinuous(true);
        panel.setColor(&initial);

        let target = ColorPanelTarget::alloc(main_thread).set_ivars(ColorPanelTargetIvars {
            changed: Rc::new(changed),
        });
        // SAFETY: NSObject initializes the allocated, ivar-initialized target on the main thread.
        let target = unsafe { objc2::msg_send![super(target), init] };
        // NSControl stores the action target weakly; this wrapper retains it
        // until another color well replaces the shared panel's target.
        // SAFETY: a retained Objective-C target can be borrowed as its AnyObject base.
        let target_object = unsafe { &*Retained::as_ptr(&target).cast::<AnyObject>() };
        // SAFETY: the shared panel is on the main thread, and the retained target
        // implements this selector with the expected color panel argument.
        unsafe {
            panel.setTarget(Some(target_object));
            panel.setAction(Some(sel!(colorPanelDidChange:)));
        }
        ACTIVE_TARGET.with(|active| active.set(Retained::as_ptr(&target)));
        panel.makeKeyAndOrderFront(None);
        Ok(Self { target })
    }
}

impl Drop for MacSubtitleColorPanel {
    fn drop(&mut self) {
        let retired = ACTIVE_TARGET.try_with(|target| {
            if target.get() != Retained::as_ptr(&self.target) {
                return false;
            }
            target.set(ptr::null());
            true
        });
        if matches!(retired, Ok(true))
            && let Some(main_thread) = MainThreadMarker::new()
        {
            let panel = NSColorPanel::sharedColorPanel(main_thread);
            detach(&panel);
            panel.orderOut(None);
        }
    }
}

fn detach(panel: &NSColorPanel) {
    // SAFETY: both values are cleared, so AppKit cannot invoke a selector on a retired target.
    unsafe {
        panel.setTarget(None);
        panel.setAction(None);
    }
}

fn rgba_channels(color: &NSColor) -> Option<[u8; 4]> {
    let srgb = NSColorSpace::sRGBColorSpace();
    let color = color.colorUsingColorSpace(&srgb)?;
    let channel = |value: f64| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
    Some([
        channel(color.redComponent()),
        channel(color.greenComponent()),
        channel(color.blueComponent()),
        channel(color.alphaComponent()),
    ])
}
