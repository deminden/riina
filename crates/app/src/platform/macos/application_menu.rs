//! Merge Slint's app menu with the native macOS application menu.

use super::menu_labels::MenuLabels;
use block2::RcBlock;
use objc2::{MainThreadMarker, rc::Retained, runtime::ProtocolObject, sel};
use objc2_app_kit::{
    NSApplication, NSEventModifierFlags, NSImage, NSImageNamePreferencesGeneral, NSMenu,
    NSMenuDidBeginTrackingNotification, NSMenuItem, NSWindowDidBecomeKeyNotification,
};
use objc2_foundation::{NSNotificationCenter, NSObjectProtocol, NSOperationQueue, NSString};
use std::{rc::Rc, time::Duration};

pub(crate) struct MacApplicationMenuObserver {
    labels: Rc<MenuLabels>,
    notification_center: Retained<NSNotificationCenter>,
    observers: [Retained<ProtocolObject<dyn NSObjectProtocol>>; 2],
}

impl MacApplicationMenuObserver {
    pub(crate) fn new() -> Self {
        if let Some(main) = MainThreadMarker::new() {
            NSApplication::sharedApplication(main)
                .setAutomaticCustomizeTouchBarMenuItemEnabled(true);
        }
        let notification_center = NSNotificationCenter::defaultCenter();
        let labels = Rc::new(MenuLabels::new());
        let tracking_labels = labels.clone();
        let tracking = RcBlock::new(move |_| {
            let Some(main_thread) = MainThreadMarker::new() else {
                return;
            };
            normalize_application_menu(main_thread, &tracking_labels);
        });
        let key_window_labels = labels.clone();
        let key_window = RcBlock::new(move |_| {
            // Let the window backend finish installing the key window's menu.
            let labels = key_window_labels.clone();
            slint::Timer::single_shot(Duration::ZERO, move || {
                if let Some(main) = MainThreadMarker::new() {
                    normalize_application_menu(main, &labels);
                }
            });
        });
        // SAFETY: both blocks run on the main queue. The returned tokens own
        // the registrations until this observer shuts down.
        let observers = unsafe {
            [
                notification_center.addObserverForName_object_queue_usingBlock(
                    Some(NSMenuDidBeginTrackingNotification),
                    None,
                    Some(&NSOperationQueue::mainQueue()),
                    &tracking,
                ),
                notification_center.addObserverForName_object_queue_usingBlock(
                    Some(NSWindowDidBecomeKeyNotification),
                    None,
                    Some(&NSOperationQueue::mainQueue()),
                    &key_window,
                ),
            ]
        };
        Self {
            labels,
            notification_center,
            observers,
        }
    }

    pub(crate) fn labels(&self) -> Rc<MenuLabels> {
        self.labels.clone()
    }

    pub(crate) fn normalize(&self) {
        if let Some(main_thread) = MainThreadMarker::new() {
            normalize_application_menu(main_thread, &self.labels);
        }
    }
}

impl Drop for MacApplicationMenuObserver {
    fn drop(&mut self) {
        for observer in &self.observers {
            // SAFETY: each retained token came from this notification center.
            unsafe { self.notification_center.removeObserver(observer.as_ref()) };
        }
    }
}

fn normalize_application_menu(main_thread: MainThreadMarker, labels: &MenuLabels) {
    let application = NSApplication::sharedApplication(main_thread);
    let Some(main_menu) = application.mainMenu() else {
        return;
    };

    install_system_menus(&application, &main_menu, labels);
    if let Some(edit) = main_menu.itemWithTitle(&NSString::from_str(&labels.edit_title)) {
        super::edit_menu::install(main_thread, &edit, labels);
    }
    register_services_menu(&application, &main_menu, labels);

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
    let expected_about = NSString::from_str(&labels.about);
    let expected_settings = NSString::from_str(&labels.settings);
    let legacy_preferences = NSString::from_str(&labels.preferences);
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

    for (item, caption) in [
        (&system_items[2], &labels.services_title),
        (&system_items[4], &labels.hide),
        (&system_items[5], &labels.hide_others),
        (&system_items[6], &labels.show_all),
    ] {
        item.setTitle(&NSString::from_str(caption));
    }
    system_app_menu.removeAllItems();
    for item in merged {
        system_app_menu.addItem(&item);
    }
    system_app_item.setTitle(&NSString::from_str("Riina"));
    main_menu.removeItemAtIndex(riina_index);
    register_services_menu(&application, &main_menu, labels);
}

fn install_system_menus(application: &NSApplication, main_menu: &NSMenu, labels: &MenuLabels) {
    // AppKit supplies the window roster, the standard window actions and
    // Help search. Slint retains the application's Inspector and Log Viewer.
    if let Some(menu) = named_submenu(main_menu, &labels.file_title) {
        install_file_open_alternates(&menu);
        if let Some(close) = menu.itemWithTitle(&NSString::from_str(&labels.close)) {
            use_first_responder(&close, sel!(performClose:));
        }
    }
    if let Some(menu) = named_submenu(main_menu, &labels.window_title) {
        install_window_actions(&menu, labels);
        if !application
            .windowsMenu()
            .as_deref()
            .is_some_and(|active| std::ptr::eq(active, &*menu))
        {
            application.setWindowsMenu(Some(&menu));
        }
    }
    if let Some(menu) = named_submenu(main_menu, &labels.help_title)
        && !application
            .helpMenu()
            .as_deref()
            .is_some_and(|active| std::ptr::eq(active, &*menu))
    {
        application.setHelpMenu(Some(&menu));
    }
}

fn services_menu(bar: &NSMenu, labels: &MenuLabels) -> Option<Retained<NSMenu>> {
    let application = bar.itemAtIndex(0)?.submenu()?;
    named_submenu(&application, &labels.services_title)
}

fn register_services_menu(application: &NSApplication, bar: &NSMenu, labels: &MenuLabels) {
    if let Some(menu) = services_menu(bar, labels)
        && !application
            .servicesMenu()
            .as_deref()
            .is_some_and(|active| std::ptr::eq(active, &*menu))
    {
        application.setServicesMenu(Some(&menu));
    }
}

fn install_file_open_alternates(menu: &NSMenu) {
    // Each Option action immediately follows its primary File command. Ask
    // AppKit to exchange the pair as the user presses or releases Option.
    for index in [0, 2] {
        let (Some(primary), Some(alternate)) =
            (menu.itemAtIndex(index), menu.itemAtIndex(index + 1))
        else {
            continue;
        };
        let primary_key = primary.keyEquivalent();
        if !alternate.isAlternate()
            && !primary_key.is_empty()
            && alternate.keyEquivalent().isEqualToString(&primary_key)
            && alternate.keyEquivalentModifierMask()
                == (primary.keyEquivalentModifierMask() | NSEventModifierFlags::Option)
        {
            alternate.setAlternate(true);
        }
    }
}

fn install_window_actions(menu: &NSMenu, labels: &MenuLabels) {
    for (title, action) in [
        (&labels.minimize, sel!(performMiniaturize:)),
        (&labels.zoom, sel!(performZoom:)),
        (&labels.bring_to_front, sel!(arrangeInFront:)),
    ] {
        if let Some(item) = menu.itemWithTitle(&NSString::from_str(title)) {
            use_first_responder(&item, action);
        }
    }
}

fn use_first_responder(item: &NSMenuItem, action: objc2::runtime::Sel) {
    if item.action() != Some(action) {
        // SAFETY: these standard AppKit selectors take the menu item as sender.
        // A nil target asks AppKit to resolve the current responder.
        unsafe {
            item.setTarget(None);
            item.setAction(Some(action));
        }
    }
}

fn named_submenu(menu: &NSMenu, title: &str) -> Option<Retained<NSMenu>> {
    menu.itemWithTitle(&NSString::from_str(title))?.submenu()
}

fn menu_items(menu: &NSMenu) -> Option<Vec<Retained<NSMenuItem>>> {
    (0..menu.numberOfItems())
        .map(|index| menu.itemAtIndex(index))
        .collect()
}
