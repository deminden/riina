//! Own borderless covers on displays other than the fullscreen window's screen.
use super::fullscreen::native_window;
use objc2::{MainThreadMarker, MainThreadOnly, rc::Retained};
use objc2_app_kit::{
    NSApplication, NSBackingStoreType, NSColor, NSMainMenuWindowLevel, NSScreen, NSWindow,
    NSWindowCollectionBehavior, NSWindowStyleMask,
};
use objc2_foundation::NSRect;

#[derive(Default)]
pub(in crate::platform) struct Covers(Vec<Cover>);

impl Covers {
    pub(in crate::platform) fn refresh(&mut self, owner: &slint::Window, _: &slint::Window) {
        let Some(main) = MainThreadMarker::new() else {
            return;
        };
        let Some(window) = native_window(owner).filter(|window| {
            window.isMainWindow() && NSApplication::sharedApplication(main).isActive()
        }) else {
            self.clear();
            return;
        };
        let Some(active) = window.screen() else {
            self.clear();
            return;
        };
        let frames: Vec<_> = NSScreen::screens(main)
            .iter()
            .map(|screen| screen.frame())
            .filter(|frame| *frame != active.frame())
            .collect();
        self.0.retain(|cover| frames.contains(&cover.frame));
        for frame in frames {
            if self.0.iter().all(|cover| cover.frame != frame) {
                self.0.push(Cover::new(main, frame));
            }
        }
    }

    pub(in crate::platform) fn clear(&mut self) {
        self.0.clear();
    }
}

struct Cover {
    frame: NSRect,
    window: Retained<NSWindow>,
}

impl Cover {
    fn new(main: MainThreadMarker, frame: NSRect) -> Self {
        // SAFETY: AppKit runs on the main thread and accepts this display frame.
        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(main),
                frame,
                NSWindowStyleMask::Borderless,
                NSBackingStoreType::Buffered,
                false,
            )
        };
        window.setBackgroundColor(Some(&NSColor::blackColor()));
        window.setIgnoresMouseEvents(true);
        window.setHidesOnDeactivate(true);
        window.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::FullScreenAuxiliary,
        );
        window.setLevel(NSMainMenuWindowLevel + 1);
        window.setFrame_display(frame, true);
        window.orderFront(None);
        Self { frame, window }
    }
}

impl Drop for Cover {
    fn drop(&mut self) {
        self.window.orderOut(None);
    }
}
