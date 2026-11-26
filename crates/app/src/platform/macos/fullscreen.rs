//! AppKit owns fullscreen spaces and reports their actual transition lifecycle.
mod legacy;

use crate::PlayerWindow;
use block2::RcBlock;
pub(in crate::platform) use legacy::Legacy;
use objc2::{MainThreadMarker, rc::Retained, runtime::ProtocolObject};
use objc2_app_kit::{
    NSView, NSWindowDidEnterFullScreenNotification, NSWindowDidExitFullScreenNotification,
    NSWindowWillEnterFullScreenNotification, NSWindowWillExitFullScreenNotification,
};
use objc2_foundation::{NSNotification, NSNotificationCenter, NSObjectProtocol, NSOperationQueue};
use slint::{
    ComponentHandle,
    winit_030::{
        WinitWindowAccessor,
        winit::{
            raw_window_handle::{HasWindowHandle, RawWindowHandle},
            window::Fullscreen,
        },
    },
};
use std::ptr::NonNull;

pub(crate) fn request_fullscreen(window: &slint::Window, fullscreen: bool) -> Option<()> {
    window.with_winit_window(|native| {
        native.set_fullscreen(fullscreen.then_some(Fullscreen::Borderless(None)));
    })
}

pub(crate) struct FullscreenNotifications {
    center: Retained<NSNotificationCenter>,
    tokens: Vec<Retained<ProtocolObject<dyn NSObjectProtocol>>>,
}

impl FullscreenNotifications {
    pub(crate) fn new(app: &PlayerWindow) -> Self {
        let center = NSNotificationCenter::defaultCenter();
        // SAFETY: AppKit publishes these static notification names. Each callback
        // is delivered on the main queue and verifies its current window identity.
        let notifications = unsafe {
            [
                (NSWindowWillEnterFullScreenNotification, true, false),
                (NSWindowDidEnterFullScreenNotification, true, true),
                (NSWindowWillExitFullScreenNotification, false, false),
                (NSWindowDidExitFullScreenNotification, false, true),
            ]
        };
        let tokens = notifications
            .into_iter()
            .map(|(name, fullscreen, finished)| {
                let weak = app.as_weak();
                let block = RcBlock::new(move |notification: NonNull<NSNotification>| {
                    if MainThreadMarker::new().is_none() {
                        return;
                    }
                    let Some(app) = weak.upgrade() else { return };
                    // SAFETY: the notification is live for the duration of the block.
                    let notification = unsafe { notification.as_ref() };
                    if belongs_to(notification, &app) {
                        let _ = weak.upgrade_in_event_loop(move |app| {
                            app.invoke_native_fullscreen_observed(fullscreen, finished);
                        });
                    }
                });
                // SAFETY: the center retains a copy of the block and dispatches it
                // on the supplied main queue.
                unsafe {
                    center.addObserverForName_object_queue_usingBlock(
                        Some(name),
                        None,
                        Some(&NSOperationQueue::mainQueue()),
                        &block,
                    )
                }
            })
            .collect();
        Self { center, tokens }
    }
}

impl Drop for FullscreenNotifications {
    fn drop(&mut self) {
        for token in &self.tokens {
            // SAFETY: each token was registered with this retained center.
            unsafe { self.center.removeObserver(token.as_ref()) };
        }
    }
}

fn belongs_to(notification: &NSNotification, app: &PlayerWindow) -> bool {
    let Some(source) = notification.object() else {
        return false;
    };
    native_window(app.window())
        .is_some_and(|window| Retained::as_ptr(&window).cast() == Retained::as_ptr(&source))
}

pub(in crate::platform) fn native_window(
    window: &slint::Window,
) -> Option<Retained<objc2_app_kit::NSWindow>> {
    window.with_winit_window(appkit_window)?
}

fn appkit_window(
    window: &slint::winit_030::winit::window::Window,
) -> Option<Retained<objc2_app_kit::NSWindow>> {
    let handle = window.window_handle().ok()?;
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return None;
    };
    // SAFETY: Winit lends the live NSView for the duration of the borrow.
    let view = unsafe { &*handle.ns_view.as_ptr().cast::<NSView>() };
    view.window()
}
