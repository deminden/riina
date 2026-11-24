//! Keep application commands available when there is no Slint player menu.
use super::LaunchTarget;
use objc2::{
    MainThreadMarker, MainThreadOnly,
    rc::Retained,
    runtime::{AnyObject, Sel},
    sel,
};
use objc2_app_kit::{NSApplication, NSMenu, NSMenuItem};
use objc2_foundation::NSString;
use std::cell::RefCell;

thread_local! {
    static IDLE_MENU: RefCell<Option<Retained<NSMenu>>> = const { RefCell::new(None) };
}

pub(super) fn prepare(
    main: MainThreadMarker,
    target: &LaunchTarget,
    enable_new_window_shortcut: bool,
) {
    let menu = NSMenu::new(main);
    let app = NSMenu::initWithTitle(NSMenu::alloc(main), &NSString::from_str("Riina"));
    let file = NSMenu::initWithTitle(NSMenu::alloc(main), &NSString::from_str("File"));
    app.addItem(&item(main, "Hide Riina", Some(sel!(hide:)), "h", None));
    app.addItem(&item(
        main,
        "Quit Riina",
        Some(sel!(riinaIdleQuit:)),
        "q",
        Some(target),
    ));
    file.addItem(&item(
        main,
        "Open…",
        Some(sel!(riinaIdleOpen:)),
        "o",
        Some(target),
    ));
    if enable_new_window_shortcut {
        file.addItem(&item(
            main,
            "New Window",
            Some(sel!(riinaIdleNewWindow:)),
            "n",
            Some(target),
        ));
    }
    for (title, submenu) in [("Riina", app), ("File", file)] {
        let parent = item(main, title, None, "", None);
        parent.setSubmenu(Some(&submenu));
        menu.addItem(&parent);
    }
    IDLE_MENU.replace(Some(menu));
}

pub(crate) fn show_idle_menu() {
    let Some(main) = MainThreadMarker::new() else {
        return;
    };
    let app = NSApplication::sharedApplication(main);
    IDLE_MENU.with_borrow(|menu| {
        if let Some(menu) = menu
            && !app
                .mainMenu()
                .as_deref()
                .is_some_and(|active| std::ptr::eq(active, &**menu))
        {
            app.setMainMenu(Some(menu));
        }
    });
}

pub(super) fn clear() {
    let Some(menu) = IDLE_MENU.take() else { return };
    if let Some(main) = MainThreadMarker::new() {
        let app = NSApplication::sharedApplication(main);
        if app
            .mainMenu()
            .as_deref()
            .is_some_and(|active| std::ptr::eq(active, &*menu))
        {
            app.setMainMenu(None);
        }
    }
}

fn item(
    main: MainThreadMarker,
    title: &str,
    action: Option<Sel>,
    shortcut: &str,
    target: Option<&AnyObject>,
) -> Retained<NSMenuItem> {
    // SAFETY: the supplied selectors use the AppKit sender signature. The
    // registration guard retains custom targets while the menu is installed.
    unsafe {
        let item = NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(main),
            &NSString::from_str(title),
            action,
            &NSString::from_str(shortcut),
        );
        item.setTarget(target);
        item
    }
}
