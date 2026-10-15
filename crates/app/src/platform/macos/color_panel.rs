//! Use AppKit's shared color panel for macOS subtitle color wells.

use objc2::{
    DefinedClass, MainThreadMarker, MainThreadOnly, define_class,
    rc::Retained,
    runtime::{AnyObject, NSObject},
    sel,
};
use objc2_app_kit::{NSColor, NSColorPanel, NSColorSpace};
use objc2_foundation::NSObjectProtocol;
use std::rc::Rc;

struct ColorPanelTargetIvars {
    changed: Rc<dyn Fn([u8; 4])>,
}

define_class!(
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = ColorPanelTargetIvars]
    struct ColorPanelTarget;

    unsafe impl NSObjectProtocol for ColorPanelTarget {}

    impl ColorPanelTarget {
        #[unsafe(method(colorPanelDidChange:))]
        fn color_panel_did_change(&self, panel: &NSColorPanel) {
            if let Some(channels) = rgba_channels(&panel.color()) {
                (self.ivars().changed)(channels);
            }
        }
    }
);

/// Keeps the panel's weak target alive while the Preferences window owns it.
pub(crate) struct MacSubtitleColorPanel {
    _target: Retained<ColorPanelTarget>,
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
        panel.setShowsAlpha(true);
        panel.setColor(&initial);

        let target = ColorPanelTarget::alloc(main_thread).set_ivars(ColorPanelTargetIvars {
            changed: Rc::new(changed),
        });
        let target = unsafe { objc2::msg_send![super(target), init] };
        // NSControl stores the action target weakly; this wrapper retains it
        // until another color well replaces the shared panel's target.
        let target_object = unsafe { &*Retained::as_ptr(&target).cast::<AnyObject>() };
        unsafe {
            panel.setTarget(Some(target_object));
            panel.setAction(Some(sel!(colorPanelDidChange:)));
        }
        panel.makeKeyAndOrderFront(None);
        Ok(Self { _target: target })
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
