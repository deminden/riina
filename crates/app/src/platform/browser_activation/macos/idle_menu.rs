//! Keep application commands available when there is no Slint player menu.
use super::{LaunchTarget, playback_menus, plugin_menu};
use crate::platform::macos::{edit_menu, menu_labels::MenuLabels};
use objc2::{
    DefinedClass, MainThreadMarker, MainThreadOnly,
    rc::Retained,
    runtime::{AnyObject, Sel},
    sel,
};
use objc2_app_kit::{NSApplication, NSEventModifierFlags, NSMenu, NSMenuItem};
use objc2_foundation::NSString;
use std::{cell::RefCell, rc::Rc};

thread_local! {
    static IDLE_MENU: RefCell<Option<Rc<IdleMenu>>> = const { RefCell::new(None) };
}

struct IdleMenu {
    native: Retained<NSMenu>,
    services: Retained<NSMenu>,
    file: Retained<NSMenu>,
    plugins: Retained<NSMenu>,
    window: Retained<NSMenu>,
    help: Retained<NSMenu>,
    target: Retained<LaunchTarget>,
    playback: Option<playback_menus::Menus>,
}

impl IdleMenu {
    fn refresh(&self, main: MainThreadMarker, force: bool) {
        let preferences = crate::platform::preferences_path()
            .ok()
            .and_then(|path| crate::preference_load::read_valid(&path).ok().flatten())
            .unwrap_or_default();
        refresh_file_menu(
            main,
            &self.file,
            &preferences,
            &self.target.ivars().menu_labels,
        );
        if let Some(playback) = &self.playback {
            playback.refresh(&self.target, &preferences);
        }
        self.refresh_plugins(force);
    }

    fn refresh_plugins(&self, force: bool) {
        self.target
            .ivars()
            .plugins
            .refresh(&self.native, &self.plugins, &self.target, force);
    }
}

const NEW_WINDOW_SEPARATOR_TAG: isize = 114083;
const NEW_WINDOW_TAG: isize = 114084;
const RECENT_DOCUMENTS_TAG: isize = 114085;

pub(super) fn prepare(
    main: MainThreadMarker,
    target: &Retained<LaunchTarget>,
    enable_new_window_shortcut: bool,
) {
    let menu = NSMenu::new(main);
    let (application, services) = application_menu(main, target);
    let file = file_menu(main, target, enable_new_window_shortcut);
    add_submenu(main, &menu, &application);
    add_submenu(main, &menu, &file);
    menu.addItem(&edit_menu::parent(main, &target.ivars().menu_labels));
    let playback = playback_menus::Menus::new(main)
        .inspect_err(|error| {
            crate::diagnostics::warn!("Could not initialize the idle playback menus: {error}")
        })
        .ok();
    if let Some(playback) = &playback {
        for submenu in playback.sections() {
            add_submenu(main, &menu, submenu);
        }
    }
    let plugins = plugin_menu::new(main, target);
    let window = window_menu(main, target);
    let help = help_menu(main, target);
    for submenu in [&plugins, &window, &help] {
        add_submenu(main, &menu, submenu);
    }
    IDLE_MENU.replace(Some(Rc::new(IdleMenu {
        native: menu,
        services,
        file,
        plugins,
        window,
        help,
        target: target.clone(),
        playback,
    })));
}

fn add_submenu(main: MainThreadMarker, menu: &NSMenu, submenu: &NSMenu) {
    let parent = item(main, &submenu.title().to_string(), None, "", None);
    parent.setSubmenu(Some(submenu));
    menu.addItem(&parent);
}

fn application_menu(
    main: MainThreadMarker,
    target: &LaunchTarget,
) -> (Retained<NSMenu>, Retained<NSMenu>) {
    let labels = &target.ivars().menu_labels;
    let app = NSMenu::initWithTitle(NSMenu::alloc(main), &NSString::from_str("Riina"));
    app.addItem(&item(
        main,
        &labels.about,
        Some(sel!(riinaIdleAbout:)),
        "",
        Some(target),
    ));
    app.addItem(&item(
        main,
        &labels.check_updates,
        Some(sel!(riinaIdleUpdates:)),
        "",
        Some(target),
    ));
    app.addItem(&NSMenuItem::separatorItem(main));
    let settings = item(
        main,
        &labels.settings,
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
    let services = item(main, &labels.services_title, None, "", None);
    let submenu = NSMenu::initWithTitle(
        NSMenu::alloc(main),
        &NSString::from_str(&labels.services_title),
    );
    services.setSubmenu(Some(&submenu));
    app.addItem(&services);
    app.addItem(&NSMenuItem::separatorItem(main));
    app.addItem(&item(main, &labels.hide, Some(sel!(hide:)), "h", None));
    let others = item(
        main,
        &labels.hide_others,
        Some(sel!(hideOtherApplications:)),
        "h",
        None,
    );
    others
        .setKeyEquivalentModifierMask(NSEventModifierFlags::Command | NSEventModifierFlags::Option);
    app.addItem(&others);
    app.addItem(&item(
        main,
        &labels.show_all,
        Some(sel!(unhideAllApplications:)),
        "",
        None,
    ));
    app.addItem(&NSMenuItem::separatorItem(main));
    app.addItem(&item(
        main,
        &labels.quit,
        Some(sel!(riinaIdleQuit:)),
        "q",
        Some(target),
    ));
    (app, submenu)
}

fn file_menu(
    main: MainThreadMarker,
    target: &LaunchTarget,
    enable_new_window_shortcut: bool,
) -> Retained<NSMenu> {
    let labels = &target.ivars().menu_labels;
    let file = NSMenu::initWithTitle(NSMenu::alloc(main), &NSString::from_str(&labels.file_title));
    for (titles, action, key, modifiers) in [
        (
            [labels.open.as_str(), labels.open_in_new_window.as_str()],
            sel!(riinaIdleOpen:),
            "o",
            NSEventModifierFlags::Command,
        ),
        (
            [
                labels.open_url.as_str(),
                labels.open_url_in_new_window.as_str(),
            ],
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
    let recent = item(main, &labels.open_recent, None, "", Some(target));
    recent.setTag(RECENT_DOCUMENTS_TAG);
    let documents = NSMenu::initWithTitle(
        NSMenu::alloc(main),
        &NSString::from_str(&labels.open_recent),
    );
    documents.setAutoenablesItems(false);
    recent.setSubmenu(Some(&documents));
    file.addItem(&recent);
    let separator = NSMenuItem::separatorItem(main);
    separator.setTag(NEW_WINDOW_SEPARATOR_TAG);
    separator.setHidden(!enable_new_window_shortcut);
    file.addItem(&separator);
    let new_window = item(
        main,
        &labels.new_window,
        Some(sel!(riinaIdleNewWindow:)),
        "n",
        Some(target),
    );
    new_window.setTag(NEW_WINDOW_TAG);
    new_window.setHidden(!enable_new_window_shortcut);
    file.addItem(&new_window);
    let history = item(
        main,
        &labels.playback_history,
        Some(sel!(riinaIdleHistory:)),
        "H",
        Some(target),
    );
    history
        .setKeyEquivalentModifierMask(NSEventModifierFlags::Command | NSEventModifierFlags::Shift);
    file.addItem(&NSMenuItem::separatorItem(main));
    file.addItem(&history);
    file.addItem(&NSMenuItem::separatorItem(main));
    for title in [
        &labels.reveal_in_finder,
        &labels.delete_current,
        &labels.save_current_playlist,
    ] {
        let unavailable = item(main, title, None, "", None);
        unavailable.setEnabled(false);
        file.addItem(&unavailable);
    }
    file.addItem(&NSMenuItem::separatorItem(main));
    file.addItem(&item(
        main,
        &labels.close,
        Some(sel!(performClose:)),
        "w",
        None,
    ));
    file
}

fn window_menu(main: MainThreadMarker, target: &LaunchTarget) -> Retained<NSMenu> {
    let labels = &target.ivars().menu_labels;
    let window = NSMenu::initWithTitle(
        NSMenu::alloc(main),
        &NSString::from_str(&labels.window_title),
    );
    window.addItem(&item(
        main,
        &labels.minimize,
        Some(sel!(performMiniaturize:)),
        "m",
        None,
    ));
    window.addItem(&item(
        main,
        &labels.zoom,
        Some(sel!(performZoom:)),
        "",
        None,
    ));
    window.addItem(&NSMenuItem::separatorItem(main));
    window.addItem(&item(
        main,
        &labels.bring_to_front,
        Some(sel!(arrangeInFront:)),
        "",
        None,
    ));
    window.addItem(&NSMenuItem::separatorItem(main));
    let logs = item(
        main,
        &labels.log_viewer,
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
    let labels = &target.ivars().menu_labels;
    let help = NSMenu::initWithTitle(NSMenu::alloc(main), &NSString::from_str(&labels.help_title));
    help.addItem(&help_link(main, target, &labels.help, "help", "?"));
    help.addItem(&item(
        main,
        &labels.dump_debug,
        Some(sel!(riinaIdleDebugInfo:)),
        "",
        Some(target),
    ));
    help.addItem(&NSMenuItem::separatorItem(main));
    help.addItem(&item(
        main,
        &labels.release_highlights,
        Some(sel!(riinaIdleHighlights:)),
        "",
        Some(target),
    ));
    help.addItem(&NSMenuItem::separatorItem(main));
    help.addItem(&help_link(main, target, "GitHub", "github", ""));
    help.addItem(&help_link(main, target, &labels.website, "website", ""));
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
    if let Some(menu) = IDLE_MENU.with_borrow(Clone::clone)
        && !owns_menu(app.mainMenu(), &menu.native)
    {
        menu.refresh(main, false);
        app.setMainMenu(Some(&menu.native));
        app.setServicesMenu(Some(&menu.services));
        app.setWindowsMenu(Some(&menu.window));
        app.setHelpMenu(Some(&menu.help));
    }
}

pub(super) fn refresh() {
    if let Some(main) = MainThreadMarker::new()
        && let Some(menu) = IDLE_MENU.with_borrow(Clone::clone)
    {
        menu.refresh(main, true);
    }
}

pub(super) fn refresh_plugins(force: bool) {
    if let Some(menu) = IDLE_MENU.with_borrow(Clone::clone) {
        menu.refresh_plugins(force);
    }
}

fn refresh_file_menu(
    main: MainThreadMarker,
    file: &NSMenu,
    preferences: &riina_core::Preferences,
    labels: &MenuLabels,
) {
    let opens_new = preferences.allow_multiple_windows;
    rebuild_recent_documents(main, file, preferences, labels);
    let hide_new_window = !preferences.enable_new_window_shortcut;
    if let Some(new_window) = file.itemWithTag(NEW_WINDOW_TAG) {
        new_window.setHidden(hide_new_window);
    }
    if let Some(separator) = file.itemWithTag(NEW_WINDOW_SEPARATOR_TAG) {
        separator.setHidden(hide_new_window);
    }
    for (first, titles) in [
        (
            0,
            [labels.open.as_str(), labels.open_in_new_window.as_str()],
        ),
        (
            2,
            [
                labels.open_url.as_str(),
                labels.open_url_in_new_window.as_str(),
            ],
        ),
    ] {
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
    labels: &MenuLabels,
) {
    let Some(recent) = file.itemWithTag(RECENT_DOCUMENTS_TAG) else {
        return;
    };
    recent.setHidden(!preferences.record_recent_files);
    let (Some(menu), Some(target)) = (recent.submenu(), recent.target()) else {
        return;
    };
    menu.removeAllItems();
    for document in crate::player_ui::distinct_documents(&preferences.recent_open_documents) {
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
        &labels.clear_recent,
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
        if owns_menu(app.servicesMenu(), &menu.services) {
            app.setServicesMenu(None);
        }
        if owns_menu(app.windowsMenu(), &menu.window) {
            app.setWindowsMenu(None);
        }
        if owns_menu(app.helpMenu(), &menu.help) {
            app.setHelpMenu(None);
        }
        if owns_menu(app.mainMenu(), &menu.native) {
            app.setMainMenu(None);
        }
    }
}

fn owns_menu(active: Option<Retained<NSMenu>>, expected: &NSMenu) -> bool {
    active
        .as_deref()
        .is_some_and(|menu| std::ptr::eq(menu, expected))
}

pub(super) fn item(
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
