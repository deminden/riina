use block2::RcBlock;
use objc2::{MainThreadMarker, rc::Retained, runtime::ProtocolObject};
use objc2_app_kit::{NSApplication, NSMenu, NSMenuDidBeginTrackingNotification, NSView};
use objc2_foundation::{NSNotification, NSNotificationCenter, NSObjectProtocol, NSString};
use slint::winit_030::winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::{ComponentHandle, winit_030::WinitWindowAccessor};
use std::ptr::NonNull;

pub(crate) struct MacPluginMenuObserver {
    notification_center: Retained<NSNotificationCenter>,
    observer: Retained<ProtocolObject<dyn NSObjectProtocol>>,
}

impl MacPluginMenuObserver {
    pub(crate) fn new(
        window: &crate::PlayerWindow,
        host: crate::plugin_host::PluginHostHandle,
    ) -> Result<Self, String> {
        let window_identity = window
            .window()
            .with_winit_window(|window| {
                let handle = window.window_handle().ok()?;
                let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
                    return None;
                };
                // SAFETY: the borrowed AppKit handle points to this live Winit window's NSView.
                let view = unsafe { &*handle.ns_view.as_ptr().cast::<NSView>() };
                view.window()
                    .map(|window| Retained::as_ptr(&window) as usize)
            })
            .flatten()
            .ok_or_else(|| "Could not identify the player window for its Plugin menu".to_owned())?;

        let notification_center = NSNotificationCenter::defaultCenter();
        let menu_title = NSString::from_str("Plugin");
        let block = RcBlock::new(move |notification: NonNull<NSNotification>| {
            let Some(main_thread) = MainThreadMarker::new() else {
                return;
            };
            // AppKit posts this notification with its NSMenu as the object.
            // SAFETY: the notification name below guarantees the object is an NSMenu.
            let Some(menu_object) = (unsafe { notification.as_ref() }).object() else {
                return;
            };
            let menu = unsafe { &*Retained::as_ptr(&menu_object).cast::<NSMenu>() };
            if !menu.title().isEqualToString(&menu_title) {
                return;
            }

            // Slint installs the active player's menu on NSApp. Multiple player
            // windows therefore observe the same notification; only the key
            // window owning this observer may refresh its player-scoped plugins.
            let application = NSApplication::sharedApplication(main_thread);
            if application
                .keyWindow()
                .is_some_and(|active| Retained::as_ptr(&active) as usize == window_identity)
            {
                let _ = host.notify_menu_update();
            }
        });
        // SAFETY: AppKit's menu-tracking notification is posted during main-thread
        // menu interaction; the block touches AppKit only after checking that thread.
        let observer = unsafe {
            notification_center.addObserverForName_object_queue_usingBlock(
                Some(NSMenuDidBeginTrackingNotification),
                None,
                None,
                &block,
            )
        };
        Ok(Self {
            notification_center,
            observer,
        })
    }
}

impl Drop for MacPluginMenuObserver {
    fn drop(&mut self) {
        // SAFETY: the token was returned by this notification center and is retained here.
        unsafe {
            self.notification_center
                .removeObserver(self.observer.as_ref());
        }
    }
}
