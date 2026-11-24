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
const OPEN_RECENT: &str = "Open Recent";

pub(super) fn prepare(
    main: MainThreadMarker,
    target: &LaunchTarget,
    enable_new_window_shortcut: bool,
) {
    let menu = NSMenu::new(main);
    for submenu in [
        application_menu(main, target),
        file_menu(main, target, enable_new_window_shortcut),
        window_menu(main, target),
        help_menu(main, target),
    ] {
        let parent = item(main, &submenu.title().to_string(), None, "", None);
        parent.setSubmenu(Some(&submenu));
        menu.addItem(&parent);
    }
    IDLE_MENU.replace(Some(menu));
}

fn application_menu(main: MainThreadMarker, target: &LaunchTarget) -> Retained<NSMenu> {
    let app = NSMenu::initWithTitle(NSMenu::alloc(main), &NSString::from_str("Riina"));
    app.addItem(&item(
        main,
        "About Riina",
        Some(sel!(riinaIdleAbout:)),
        "",
        Some(target),
    ));
    app.addItem(&item(
        main,
        "Check for Updates…",
        Some(sel!(riinaIdleUpdates:)),
        "",
        Some(target),
    ));
    app.addItem(&NSMenuItem::separatorItem(main));
    let settings = item(
        main,
        "Settings…",
        Some(sel!(riinaIdleSettings:)),
        ",",
        Some(target),
    );
    // SAFETY: This immutable AppKit symbol is provided by the linked framework.
    let image =
        unsafe { objc2_app_kit::NSImage::imageNamed(objc2_app_kit::NSImageNamePreferencesGeneral) };
    settings.setImage(image.as_deref());
    app.addItem(&settings);
    app.addItem(&NSMenuItem::separatorItem(main));
    app.addItem(&item(main, "Hide Riina", Some(sel!(hide:)), "h", None));
    app.addItem(&item(
        main,
        "Quit Riina",
        Some(sel!(riinaIdleQuit:)),
        "q",
        Some(target),
    ));
    app
}

fn file_menu(
    main: MainThreadMarker,
    target: &LaunchTarget,
    enable_new_window_shortcut: bool,
) -> Retained<NSMenu> {
    let file = NSMenu::initWithTitle(NSMenu::alloc(main), &NSString::from_str("File"));
    for (titles, action, key, modifiers) in [
        (
            FILE_TITLES,
            sel!(riinaIdleOpen:),
            "o",
            NSEventModifierFlags::Command,
        ),
        (
            URL_TITLES,
            sel!(riinaIdleOpenURL:),
            "O",
            NSEventModifierFlags::Command | NSEventModifierFlags::Shift,
        ),
    ] {
        let primary = item(main, titles[0], Some(action), key, Some(target));
        primary.setKeyEquivalentModifierMask(modifiers);
        file.addItem(&primary);
        let alternative = item(main, titles[1], Some(action), key, Some(target));
        alternative.setKeyEquivalentModifierMask(modifiers | NSEventModifierFlags::Option);
        alternative.setAlternate(true);
        file.addItem(&alternative);
    }
    let recent = item(main, OPEN_RECENT, None, "", Some(target));
    let documents = NSMenu::initWithTitle(NSMenu::alloc(main), &NSString::from_str(OPEN_RECENT));
    documents.setAutoenablesItems(false);
    recent.setSubmenu(Some(&documents));
    file.addItem(&recent);
    if enable_new_window_shortcut {
        file.addItem(&NSMenuItem::separatorItem(main));
        file.addItem(&item(
            main,
            "New Window",
            Some(sel!(riinaIdleNewWindow:)),
            "n",
            Some(target),
        ));
    }
    let history = item(
        main,
        "Playback History",
        Some(sel!(riinaIdleHistory:)),
        "H",
        Some(target),
    );
    history
        .setKeyEquivalentModifierMask(NSEventModifierFlags::Command | NSEventModifierFlags::Shift);
    file.addItem(&NSMenuItem::separatorItem(main));
    file.addItem(&history);
    file
}

fn window_menu(main: MainThreadMarker, target: &LaunchTarget) -> Retained<NSMenu> {
    let window = NSMenu::initWithTitle(NSMenu::alloc(main), &NSString::from_str("Window"));
    window.addItem(&item(
        main,
        "Minimize",
        Some(sel!(performMiniaturize:)),
        "m",
        None,
    ));
    window.addItem(&item(main, "Zoom", Some(sel!(performZoom:)), "", None));
    window.addItem(&NSMenuItem::separatorItem(main));
    window.addItem(&item(
        main,
        "Bring All to Front",
        Some(sel!(arrangeInFront:)),
        "",
        None,
    ));
    window.addItem(&NSMenuItem::separatorItem(main));
    let logs = item(
        main,
        "Log Viewer",
        Some(sel!(riinaIdleLogs:)),
        "l",
        Some(target),
    );
    logs.setKeyEquivalentModifierMask(
        NSEventModifierFlags::Control | NSEventModifierFlags::Command,
    );
    window.addItem(&logs);
    window
}

fn help_menu(main: MainThreadMarker, target: &LaunchTarget) -> Retained<NSMenu> {
    let help = NSMenu::initWithTitle(NSMenu::alloc(main), &NSString::from_str("Help"));
    help.addItem(&help_link(main, target, "Riina Help", "help", "?"));
    help.addItem(&item(
        main,
        "Dump Debug Info",
        Some(sel!(riinaIdleDebugInfo:)),
        "",
        Some(target),
    ));
    help.addItem(&NSMenuItem::separatorItem(main));
    help.addItem(&item(
        main,
        "Release Highlights",
        Some(sel!(riinaIdleHighlights:)),
        "",
        Some(target),
    ));
    help.addItem(&NSMenuItem::separatorItem(main));
    help.addItem(&help_link(main, target, "GitHub", "github", ""));
    help.addItem(&help_link(main, target, "Website", "website", ""));
    help
}

fn help_link(
    main: MainThreadMarker,
    target: &LaunchTarget,
    title: &str,
    destination: &str,
    shortcut: &str,
) -> Retained<NSMenuItem> {
    let link = item(
        main,
        title,
        Some(sel!(riinaIdleHelpLink:)),
        shortcut,
        Some(target),
    );
    // SAFETY: the item retains the immutable destination while it can send its action.
    unsafe {
        link.setRepresentedObject(Some(&NSString::from_str(destination)));
    }
    link
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
            refresh_file_menu(main, menu);
            app.setMainMenu(Some(menu));
            if let Some(window) = menu
                .itemWithTitle(&NSString::from_str("Window"))
                .and_then(|item| item.submenu())
            {
                app.setWindowsMenu(Some(&window));
            }
            if let Some(help) = menu
                .itemWithTitle(&NSString::from_str("Help"))
                .and_then(|item| item.submenu())
            {
                app.setHelpMenu(Some(&help));
            }
        }
    });
}

pub(super) fn refresh() {
    if let Some(main) = MainThreadMarker::new() {
        IDLE_MENU.with_borrow(|menu| {
            if let Some(menu) = menu {
                refresh_file_menu(main, menu);
            }
        });
    }
}

fn refresh_file_menu(main: MainThreadMarker, menu: &NSMenu) {
    let Some(file) = menu.itemAtIndex(1).and_then(|item| item.submenu()) else {
        return;
    };
    let preferences = crate::platform::preferences_path()
        .ok()
        .and_then(|path| crate::preference_load::read_valid(&path).ok().flatten())
        .unwrap_or_default();
    let opens_new = preferences.allow_multiple_windows;
    rebuild_recent_documents(main, &file, &preferences);
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

fn rebuild_recent_documents(
    main: MainThreadMarker,
    file: &NSMenu,
    preferences: &riina_core::Preferences,
) {
    let Some(recent) = file.itemWithTitle(&NSString::from_str(OPEN_RECENT)) else {
        return;
    };
    recent.setHidden(!preferences.record_recent_files);
    let (Some(menu), Some(target)) = (recent.submenu(), recent.target()) else {
        return;
    };
    menu.removeAllItems();
    for document in &preferences.recent_open_documents {
        let title = if document.title.is_empty() {
            &document.path
        } else {
            &document.title
        };
        let row = item(main, title, Some(sel!(riinaIdleRecent:)), "", Some(&target));
        let path = NSString::from_str(&document.path);
        // SAFETY: the menu item retains this immutable NSString while its action can run.
        unsafe {
            row.setRepresentedObject(Some(&path));
        }
        menu.addItem(&row);
    }
    let any_documents = !preferences.recent_open_documents.is_empty();
    if any_documents {
        menu.addItem(&NSMenuItem::separatorItem(main));
    }
    let clear = item(
        main,
        "Clear Menu",
        Some(sel!(riinaIdleClearRecent:)),
        "",
        Some(&target),
    );
    clear.setEnabled(any_documents);
    menu.addItem(&clear);
}

pub(super) fn clear() {
    let Some(menu) = IDLE_MENU.take() else { return };
    if let Some(main) = MainThreadMarker::new() {
        let app = NSApplication::sharedApplication(main);
        if let Some(window) = menu
            .itemWithTitle(&NSString::from_str("Window"))
            .and_then(|item| item.submenu())
            && app
                .windowsMenu()
                .as_deref()
                .is_some_and(|active| std::ptr::eq(active, &*window))
        {
            app.setWindowsMenu(None);
        }
        if let Some(help) = menu
            .itemWithTitle(&NSString::from_str("Help"))
            .and_then(|item| item.submenu())
            && app
                .helpMenu()
                .as_deref()
                .is_some_and(|active| std::ptr::eq(active, &*help))
        {
            app.setHelpMenu(None);
        }
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
