//! Keep application commands available when there is no Slint player menu.
use super::LaunchTarget;
use objc2::{
    MainThreadMarker, MainThreadOnly,
    rc::Retained,
    runtime::{AnyObject, Sel},
    sel,
};
use objc2_app_kit::{NSApplication, NSEventModifierFlags, NSMenu, NSMenuItem};
use objc2_foundation::NSString;
use std::cell::RefCell;

thread_local! {
    static IDLE_MENU: RefCell<Option<Retained<NSMenu>>> = const { RefCell::new(None) };
}

const FILE_TITLES: [&str; 2] = ["Open…", "Open in New Window…"];
const URL_TITLES: [&str; 2] = ["Open URL…", "Open URL in New Window…"];

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
    for (titles, action, modifiers) in [
        (
            FILE_TITLES,
            sel!(riinaIdleOpen:),
            NSEventModifierFlags::Command,
        ),
        (
            URL_TITLES,
            sel!(riinaIdleOpenURL:),
            NSEventModifierFlags::Command | NSEventModifierFlags::Shift,
        ),
    ] {
        let primary = item(main, titles[0], Some(action), "o", Some(target));
        primary.setKeyEquivalentModifierMask(modifiers);
        file.addItem(&primary);
        let alternative = item(main, titles[1], Some(action), "o", Some(target));
        alternative.setKeyEquivalentModifierMask(modifiers | NSEventModifierFlags::Option);
        alternative.setAlternate(true);
        file.addItem(&alternative);
    }
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
            refresh_file_labels(menu);
            app.setMainMenu(Some(menu));
        }
    });
}

fn refresh_file_labels(menu: &NSMenu) {
    let Some(file) = menu.itemAtIndex(1).and_then(|item| item.submenu()) else {
        return;
    };
    let opens_new = crate::platform::preferences_path()
        .ok()
        .and_then(|path| crate::preference_load::read_valid(&path).ok().flatten())
        .unwrap_or_default()
        .allow_multiple_windows;
    for (first, titles) in [(0, FILE_TITLES), (2, URL_TITLES)] {
        let Some(primary) = file.itemAtIndex(first) else {
            continue;
        };
        let Some(alternate) = file.itemAtIndex(first + 1) else {
            continue;
        };
        primary.setTitle(&NSString::from_str(titles[usize::from(opens_new)]));
        alternate.setTitle(&NSString::from_str(titles[usize::from(!opens_new)]));
    }
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
