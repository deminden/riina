//! AppKit identifies completed drag gestures independently of layout changes.
use super::fullscreen::native_window;
use crate::{PlayerWindow, music_mode_layout};
use block2::RcBlock;
use objc2::{MainThreadMarker, rc::Retained, runtime::ProtocolObject};
use objc2_app_kit::NSWindowDidEndLiveResizeNotification;
use objc2_foundation::{NSNotification, NSNotificationCenter, NSObjectProtocol, NSOperationQueue};
use slint::ComponentHandle;
use std::ptr::NonNull;

pub(in crate::platform) fn preview(app: &PlayerWindow, window: &slint::Window, height: u32) {
    if music_mode_layout::can_resize(app)
        && native_window(window).is_some_and(|window| window.inLiveResize())
    {
        music_mode_layout::preview_resize(app, height, f64::from(window.scale_factor()));
    }
}

pub(crate) struct Notifications {
    center: Retained<NSNotificationCenter>,
    token: Retained<ProtocolObject<dyn NSObjectProtocol>>,
}

impl Notifications {
    pub(crate) fn new(app: &PlayerWindow) -> Self {
        let weak = app.as_weak();
        let block = RcBlock::new(move |notification: NonNull<NSNotification>| {
            if MainThreadMarker::new().is_none() {
                return;
            }
            let Some(app) = weak.upgrade() else { return };
            // SAFETY: the delivery retains the notification for the block call.
            let source = unsafe { notification.as_ref() }.object();
            if let (Some(source), Some(window)) = (source, native_window(app.window()))
                && Retained::as_ptr(&window).cast() == Retained::as_ptr(&source)
                && music_mode_layout::can_resize(&app)
            {
                let _ = weak.upgrade_in_event_loop(|app| {
                    if music_mode_layout::can_resize(&app) {
                        let window = app.window();
                        music_mode_layout::preview_resize(
                            &app,
                            music_mode_layout::client_size(window).height,
                            f64::from(window.scale_factor()),
                        );
                        app.invoke_music_mode_resize_finished();
                    }
                });
            }
        });
        let center = NSNotificationCenter::defaultCenter();
        // SAFETY: the center retains the block and dispatches it on the main queue.
        let token = unsafe {
            center.addObserverForName_object_queue_usingBlock(
                Some(NSWindowDidEndLiveResizeNotification),
                None,
                Some(&NSOperationQueue::mainQueue()),
                &block,
            )
        };
        Self { center, token }
    }
}

impl Drop for Notifications {
    fn drop(&mut self) {
        // SAFETY: this token belongs to the retained notification center.
        unsafe { self.center.removeObserver(self.token.as_ref()) };
    }
}
