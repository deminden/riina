//! Merge Slint's app menu with the native macOS application menu.

use block2::RcBlock;
use objc2::{MainThreadMarker, rc::Retained, runtime::ProtocolObject};
use objc2_app_kit::{
    NSApplication, NSImage, NSImageNamePreferencesGeneral, NSMenu,
    NSMenuDidBeginTrackingNotification, NSMenuItem,
};
use objc2_foundation::{NSNotificationCenter, NSObjectProtocol, NSOperationQueue, NSString};

pub(crate) struct MacApplicationMenuObserver {
    notification_center: Retained<NSNotificationCenter>,
    observer: Retained<ProtocolObject<dyn NSObjectProtocol>>,
}

impl MacApplicationMenuObserver {
    pub(crate) fn new() -> Self {
        let notification_center = NSNotificationCenter::defaultCenter();
        let block = RcBlock::new(move |_| {
            let Some(main_thread) = MainThreadMarker::new() else {
                return;
            };
            normalize_application_menu(main_thread);
        });
        // SAFETY: AppKit posts menu tracking on its main queue; the callback
        // verifies that it is running on the main thread before touching AppKit.
        let observer = unsafe {
            notification_center.addObserverForName_object_queue_usingBlock(
                Some(NSMenuDidBeginTrackingNotification),
                None,
                Some(&NSOperationQueue::mainQueue()),
                &block,
            )
        };
        Self {
            notification_center,
            observer,
        }
    }

    pub(crate) fn normalize(&self) {
        if let Some(main_thread) = MainThreadMarker::new() {
            normalize_application_menu(main_thread);
        }
    }
}

impl Drop for MacApplicationMenuObserver {
    fn drop(&mut self) {
        // SAFETY: the retained token came from this notification center.
        unsafe {
            self.notification_center
                .removeObserver(self.observer.as_ref());
        }
    }
}

fn normalize_application_menu(main_thread: MainThreadMarker) {
    let application = NSApplication::sharedApplication(main_thread);
    let Some(main_menu) = application.mainMenu() else {
        return;
    };

    let riina_title = NSString::from_str("riina");
    let riina_index = (0..main_menu.numberOfItems()).find(|index| {
        main_menu
            .itemAtIndex(*index)
            .is_some_and(|item| item.title().isEqualToString(&riina_title))
    });
    let Some(riina_index) = riina_index else {
        return;
    };

    let Some(system_app_item) = main_menu.itemAtIndex(0) else {
        return;
    };
    let system_app_title = NSString::from_str("App");
    if !system_app_item.title().isEqualToString(&system_app_title) {
        return;
    }
    let Some(system_app_menu) = system_app_item.submenu() else {
        return;
    };
    let Some(riina_item) = main_menu.itemAtIndex(riina_index) else {
        return;
    };
    let Some(riina_menu) = riina_item.submenu() else {
        return;
    };

    // These two menu layouts are constructed by the pinned Slint 1.18.1
    // backend and this app's Slint MenuBar. Bail out if either layout changes.
    if system_app_menu.numberOfItems() != 9 || riina_menu.numberOfItems() != 9 {
        return;
    }
    let Some(system_items) = menu_items(&system_app_menu) else {
        return;
    };
    let Some(riina_items) = menu_items(&riina_menu) else {
        return;
    };
    let expected_about = NSString::from_str("About Riina");
    let expected_settings = NSString::from_str("Settings…");
    let legacy_preferences = NSString::from_str("Preferences…");
    if !riina_items[0].title().isEqualToString(&expected_about)
        || (!riina_items[3].title().isEqualToString(&expected_settings)
            && !riina_items[3].title().isEqualToString(&legacy_preferences))
    {
        return;
    }
    // SAFETY: this immutable AppKit symbol is provided by the linked framework.
    let preferences_image = unsafe { NSImage::imageNamed(NSImageNamePreferencesGeneral) };
    riina_items[3].setImage(preferences_image.as_deref());

    // Keep the Slint-owned About, update, Preferences, and Quit actions so
    // their generated menu identifiers continue to dispatch through Slint.
    // Keep AppKit's live Services and visibility commands in their native slot.
    let merged = [
        riina_items[0].clone(),
        riina_items[1].clone(),
        riina_items[2].clone(),
        riina_items[3].clone(),
        riina_items[4].clone(),
        system_items[2].clone(),
        riina_items[6].clone(),
        system_items[4].clone(),
        system_items[5].clone(),
        system_items[6].clone(),
        system_items[7].clone(),
        riina_items[8].clone(),
    ];

    system_app_menu.removeAllItems();
    for item in merged {
        system_app_menu.addItem(&item);
    }
    system_app_item.setTitle(&NSString::from_str("Riina"));
    main_menu.removeItemAtIndex(riina_index);
}

fn menu_items(menu: &NSMenu) -> Option<Vec<Retained<NSMenuItem>>> {
    (0..menu.numberOfItems())
        .map(|index| menu.itemAtIndex(index))
        .collect()
}
