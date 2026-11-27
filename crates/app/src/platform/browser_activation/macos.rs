//! Receive Finder, Dock and URL events while Winit retains the application delegate.
mod idle_menu;
mod playback_menus;
mod plugin_menu;
mod plugin_shortcuts;

use crate::platform::macos::menu_labels::MenuLabels;
use crate::{
    browser_links::{Delivery, Inbox, Link},
    player_manager::{PlayerManager, document_open::OpenGuard},
};
use block2::RcBlock;
use objc2::{
    DefinedClass, MainThreadMarker, MainThreadOnly, define_class,
    rc::Retained,
    runtime::{AnyObject, NSObject, ProtocolObject, Sel},
    sel,
};
use objc2_app_kit::{
    NSApplication, NSApplicationDidFinishLaunchingNotification,
    NSApplicationWillFinishLaunchingNotification, NSMenu, NSMenuDelegate, NSMenuItem,
};
use objc2_foundation::{
    NSAppleEventDescriptor, NSAppleEventManager, NSNotificationCenter, NSObjectProtocol,
};
use riina_core::ActionAfterLaunch;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

pub(crate) use idle_menu::show_idle_menu;
pub(crate) fn refresh_idle_menu() {
    idle_menu::refresh();
}

pub(crate) fn refresh_idle_plugins() {
    idle_menu::refresh_plugins(false);
}

const CORE_EVENT: u32 = u32::from_be_bytes(*b"aevt");
const OPEN_DOCUMENTS: u32 = u32::from_be_bytes(*b"odoc");
const REOPEN: u32 = u32::from_be_bytes(*b"rapp");
const QUIT: u32 = u32::from_be_bytes(*b"quit");
const URL_EVENT: u32 = u32::from_be_bytes(*b"GURL");
const DIRECT_OBJECT: u32 = u32::from_be_bytes(*b"----");

struct Launch {
    manager: PlayerManager,
    browser: Inbox,
    default_action: Cell<Option<ActionAfterLaunch>>,
    documents: RefCell<Vec<String>>,
    pending_documents: RefCell<Option<OpenGuard>>,
    document_timer: slint::Timer,
    plugins: plugin_menu::IdlePlugins,
    menu_labels: Rc<MenuLabels>,
}

impl Launch {
    fn open_documents(self: &Rc<Self>, paths: Vec<String>) {
        if paths.is_empty() || self.manager.is_quitting() {
            return;
        }
        self.default_action.take();
        if self.pending_documents.borrow().is_none() {
            let Some(pending) = self.manager.hold_document_open() else {
                return;
            };
            self.pending_documents.replace(Some(pending));
        }
        self.documents.borrow_mut().extend(paths);
        let launch = Rc::downgrade(self);
        // Finder may split a single multiple-file action across nearby events.
        self.document_timer.start(
            slint::TimerMode::SingleShot,
            Duration::from_millis(160),
            move || {
                if let Some(launch) = launch.upgrade() {
                    let _pending = launch.pending_documents.take();
                    launch
                        .manager
                        .open_application_documents(launch.documents.take());
                }
            },
        );
    }

    fn launch_default(&self) {
        if self.manager.is_empty()
            && !self.manager.is_quitting()
            && let Some(action) = self.default_action.take()
        {
            show_idle_menu();
            self.manager.launch_action(action);
        }
    }

    fn show_filters(&self, kind: crate::video_filter_window::Kind) {
        if self.manager.is_quitting() {
            return;
        }
        self.default_action.take();
        let action = || {
            let path = crate::platform::preferences_path().map_err(|error| error.to_string())?;
            let saved = crate::preference_load::load(&path).map_err(|error| error.to_string())?;
            let services = self.manager.services();
            services
                .filters
                .profile(&path, &saved.preferences)
                .show(kind, None, &services.history)
        };
        if let Err(error) = action() {
            crate::diagnostics::warn!("Could not open {}: {error}", kind.title());
        }
    }

    fn reopen(self: &Rc<Self>) {
        self.default_action.take();
        let launch = self.clone();
        if let Err(error) = slint::spawn_local(async move {
            if launch.manager.is_quitting() || !launch.documents.borrow().is_empty() {
                return;
            }
            if let Some(main) = MainThreadMarker::new() {
                let app = NSApplication::sharedApplication(main);
                if app.isHidden() {
                    app.unhide(None);
                }
                let windows = app.windows();
                if windows.iter().any(|window| window.isVisible()) {
                    return;
                }
                if let Some(window) = windows.iter().find(|window| window.isMiniaturized()) {
                    window.deminiaturize(None);
                    return;
                }
                show_idle_menu();
                launch.manager.reopen();
            }
        }) {
            crate::diagnostics::warn!("Could not schedule Dock reopen: {error}");
        }
    }
}

define_class!(
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = Rc<Launch>]
    struct LaunchTarget;

    // SAFETY: NSObject provides the protocol implementation.
    unsafe impl NSObjectProtocol for LaunchTarget {}

    // SAFETY: only this application's Plugin menu uses this main-thread delegate.
    unsafe impl NSMenuDelegate for LaunchTarget {
        #[unsafe(method(menuNeedsUpdate:))]
        fn menu_needs_update(&self, _menu: &NSMenu) {
            refresh_idle_plugins();
        }

        #[unsafe(method(menuWillOpen:))]
        fn menu_will_open(&self, _menu: &NSMenu) {
            self.ivars().plugins.set_tracking(true);
        }

        #[unsafe(method(menuDidClose:))]
        fn menu_did_close(&self, _menu: &NSMenu) {
            self.ivars().plugins.set_tracking(false);
            let _ = slint::spawn_local(async { refresh_idle_plugins() });
        }
    }

    impl LaunchTarget {
        #[unsafe(method(riinaOpenURL:reply:))]
        fn open_url(&self, event: &NSAppleEventDescriptor, _reply: Option<&NSAppleEventDescriptor>) {
            let Some(text) = direct_object(event).and_then(|descriptor| descriptor.stringValue()) else {
                return;
            };
            let launch = self.ivars();
            if launch.manager.is_quitting() {
                return;
            }
            match Link::parse(&text.to_string()) {
                Ok(link) => {
                    if launch.browser.push(Delivery::Link(link)) {
                        launch.default_action.take();
                    }
                }
                Err(error) => crate::diagnostics::warn!("Discarded browser link: {error}"),
            }
        }

        #[unsafe(method(riinaOpenDocuments:reply:))]
        fn open_documents(&self, event: &NSAppleEventDescriptor, _reply: Option<&NSAppleEventDescriptor>) {
            if let Some(list) = direct_object(event) {
                match document_paths(&list) {
                    Ok(paths) => self.ivars().open_documents(paths),
                    Err(error) => crate::diagnostics::warn!("Could not read Finder documents: {error}"),
                }
            }
        }

        #[unsafe(method(riinaReopen:reply:))]
        fn reopen(&self, _event: &NSAppleEventDescriptor, _reply: Option<&NSAppleEventDescriptor>) {
            self.ivars().reopen();
        }

        #[unsafe(method(riinaQuit:reply:))]
        fn quit(&self, _event: &NSAppleEventDescriptor, _reply: Option<&NSAppleEventDescriptor>) {
            self.ivars().manager.request_quit();
        }

        #[unsafe(method(riinaIdleOpen:))]
        fn idle_open(&self, sender: Option<&objc2_app_kit::NSMenuItem>) {
            let launch = self.ivars();
            launch.default_action.take();
            let alternative = sender.is_some_and(objc2_app_kit::NSMenuItem::isAlternate);
            launch.manager.open_application_panel(alternative);
        }

        #[unsafe(method(riinaIdleOpenURL:))]
        fn idle_open_url(&self, _sender: Option<&AnyObject>) {
            let launch = self.ivars();
            launch.default_action.take();
            launch.manager.show_application_open_url();
        }

        #[unsafe(method(riinaIdleRecent:))]
        fn idle_recent(&self, sender: Option<&objc2_app_kit::NSMenuItem>) {
            if let Some(path) = sender
                .and_then(objc2_app_kit::NSMenuItem::representedObject)
                .and_then(|value| value.downcast::<objc2_foundation::NSString>().ok())
            {
                let launch = self.ivars();
                launch.default_action.take();
                launch.manager.open_application_recent(path.to_string());
            }
        }

        #[unsafe(method(riinaIdleClearRecent:))]
        fn idle_clear_recent(&self, _sender: Option<&AnyObject>) {
            self.ivars().manager.clear_application_recents();
            idle_menu::refresh();
        }

        #[unsafe(method(riinaIdleLogs:))]
        fn idle_logs(&self, _sender: Option<&AnyObject>) {
            let launch = self.ivars();
            launch.default_action.take();
            launch.manager.show_application_logs();
        }

        #[unsafe(method(riinaIdleUpdates:))]
        fn idle_updates(&self, _sender: Option<&AnyObject>) {
            let launch = self.ivars();
            launch.default_action.take();
            launch.manager.show_application_updates();
        }

        #[unsafe(method(riinaIdleHighlights:))]
        fn idle_highlights(&self, _sender: Option<&AnyObject>) {
            let launch = self.ivars();
            launch.default_action.take();
            launch.manager.show_application_highlights();
        }

        #[unsafe(method(riinaIdleSettings:))]
        fn idle_settings(&self, _sender: Option<&AnyObject>) {
            let launch = self.ivars();
            launch.default_action.take();
            launch.manager.show_application_settings(None);
        }

        #[unsafe(method(riinaIdleManagePlugins:))]
        fn idle_manage_plugins(&self, _sender: Option<&AnyObject>) {
            let launch = self.ivars();
            launch.default_action.take();
            launch.manager.show_application_settings(Some(crate::platform::application_settings::PLUGINS_PANE));
        }

        #[unsafe(method(riinaIdlePluginAction:))]
        fn idle_plugin_action(&self, sender: Option<&NSMenuItem>) {
            plugin_menu::activate(self, sender);
        }

        #[unsafe(method(riinaIdlePluginDeveloper:))]
        fn idle_plugin_developer(&self, sender: Option<&NSMenuItem>) {
            plugin_menu::open_console(self, sender);
        }

        #[unsafe(method(riinaIdleReloadPlugins:))]
        fn idle_reload_plugins(&self, _sender: Option<&AnyObject>) {
            plugin_menu::reload(self);
        }

        #[unsafe(method(riinaIdleHelpLink:))]
        fn idle_help_link(&self, sender: Option<&objc2_app_kit::NSMenuItem>) {
            let launch = self.ivars();
            if launch.manager.is_quitting() {
                return;
            }
            if let Some(destination) = sender
                .and_then(objc2_app_kit::NSMenuItem::representedObject)
                .and_then(|value| value.downcast::<objc2_foundation::NSString>().ok())
                .and_then(|value| crate::player_ui::help_menu_url(&value.to_string()))
                && let Err(error) = crate::platform::open_http_url_in_browser(destination)
            {
                crate::diagnostics::warn!("Could not open the help link: {error}");
            }
        }

        #[unsafe(method(riinaIdleDebugInfo:))]
        fn idle_debug_info(&self, _sender: Option<&AnyObject>) {
            let launch = self.ivars();
            if !launch.manager.is_quitting() {
                launch.default_action.take();
                launch.manager.services().debug_information.dump(None);
            }
        }

        #[unsafe(method(riinaIdleAbout:))]
        fn idle_about(&self, _sender: Option<&AnyObject>) {
            let launch = self.ivars();
            launch.default_action.take();
            launch.manager.show_application_about();
        }

        #[unsafe(method(riinaIdleHistory:))]
        fn idle_history(&self, _sender: Option<&AnyObject>) {
            let launch = self.ivars();
            launch.default_action.take();
            launch.manager.show_application_history();
        }

        #[unsafe(method(riinaIdleScreenshotFolder:))]
        fn idle_screenshot_folder(&self, _sender: Option<&AnyObject>) {
            let launch = self.ivars();
            if launch.manager.is_quitting() {
                return;
            }
            launch.default_action.take();
            let action = || {
                let path = crate::platform::preferences_path()?;
                let saved = crate::preference_load::load(&path)?;
                crate::platform::open_screenshot_directory(saved.preferences.screenshot.directory.as_deref())
            };
            if let Err(error) = action() {
                crate::diagnostics::warn!("Could not open the screenshot folder: {error}");
            }
        }

        #[unsafe(method(riinaIdleVideoFilters:))]
        fn idle_video_filters(&self, _sender: Option<&AnyObject>) {
            self.ivars().show_filters(crate::video_filter_window::Kind::Video);
        }

        #[unsafe(method(riinaIdleAudioFilters:))]
        fn idle_audio_filters(&self, _sender: Option<&AnyObject>) {
            self.ivars().show_filters(crate::video_filter_window::Kind::Audio);
        }

        #[unsafe(method(riinaIdleNewWindow:))]
        fn idle_new_window(&self, _sender: Option<&AnyObject>) {
            let launch = self.ivars();
            launch.default_action.take();
            launch.manager.request_welcome();
        }

        #[unsafe(method(riinaIdleQuit:))]
        fn idle_quit(&self, _sender: Option<&AnyObject>) {
            self.ivars().manager.request_quit();
        }
    }
);

pub(crate) struct ApplicationEvents {
    manager: Retained<NSAppleEventManager>,
    center: Retained<NSNotificationCenter>,
    will_launch: Retained<ProtocolObject<dyn NSObjectProtocol>>,
    did_launch: Option<Retained<ProtocolObject<dyn NSObjectProtocol>>>,
    _target: Retained<LaunchTarget>,
}

impl ApplicationEvents {
    pub(crate) fn new(
        browser: Inbox,
        manager: PlayerManager,
        default_action: Option<ActionAfterLaunch>,
        preferences: &riina_core::Preferences,
        menu_labels: Rc<MenuLabels>,
    ) -> Option<Self> {
        let main = MainThreadMarker::new()?;
        let launch = Rc::new(Launch {
            manager,
            browser,
            default_action: Cell::new(default_action),
            documents: RefCell::default(),
            pending_documents: RefCell::default(),
            document_timer: slint::Timer::default(),
            plugins: plugin_menu::IdlePlugins::default(),
            menu_labels,
        });
        let allocated = LaunchTarget::alloc(main).set_ivars(launch.clone());
        // SAFETY: the target's superclass is NSObject and supplies this initializer.
        let target: Retained<LaunchTarget> = unsafe { objc2::msg_send![super(allocated), init] };
        let manager = NSAppleEventManager::sharedAppleEventManager();
        register(&manager, &target);
        let center = NSNotificationCenter::defaultCenter();
        let observed_manager = manager.clone();
        let observed_target = target.clone();
        let block = RcBlock::new(move |_| {
            if MainThreadMarker::new().is_some() {
                register(&observed_manager, &observed_target);
            }
        });
        // SAFETY: AppKit posts this notification on the main thread. Register after
        // AppKit's defaults and before it dispatches the initial document event.
        let will_launch = unsafe {
            center.addObserverForName_object_queue_usingBlock(
                Some(NSApplicationWillFinishLaunchingNotification),
                None,
                None,
                &block,
            )
        };
        let did_launch = preferences
            .record_recent_files
            .then(|| restore_recents(main, &center))
            .flatten();
        idle_menu::prepare(main, &target, preferences.enable_new_window_shortcut);
        if default_action.is_some() {
            slint::Timer::single_shot(Duration::from_millis(220), move || launch.launch_default());
        }
        Some(Self {
            manager,
            center,
            will_launch,
            did_launch,
            _target: target,
        })
    }
}

impl Drop for ApplicationEvents {
    fn drop(&mut self) {
        idle_menu::clear();
        // SAFETY: both registrations and the observer belong to this guard.
        unsafe {
            self.center.removeObserver(self.will_launch.as_ref());
            if let Some(observer) = self.did_launch.as_ref() {
                self.center.removeObserver(observer.as_ref());
            }
            for (class, event, _) in handlers() {
                let _: () = objc2::msg_send![
                    &*self.manager, removeEventHandlerForEventClass: class, andEventID: event
                ];
            }
        }
    }
}

fn restore_recents(
    main: MainThreadMarker,
    center: &NSNotificationCenter,
) -> Option<Retained<ProtocolObject<dyn NSObjectProtocol>>> {
    use crate::platform::macos::recent_documents;

    let application = NSApplication::sharedApplication(main);
    if application.isRunning() {
        recent_documents::restore(main);
        return None;
    }
    let restore = RcBlock::new(|_| {
        if let Some(main) = MainThreadMarker::new() {
            recent_documents::restore(main);
        }
    });
    // SAFETY: AppKit posts this notification on the main thread. The registration
    // is removed with the application event guard.
    Some(unsafe {
        center.addObserverForName_object_queue_usingBlock(
            Some(NSApplicationDidFinishLaunchingNotification),
            None,
            None,
            &restore,
        )
    })
}

fn handlers() -> [(u32, u32, Sel); 4] {
    [
        (URL_EVENT, URL_EVENT, sel!(riinaOpenURL:reply:)),
        (CORE_EVENT, OPEN_DOCUMENTS, sel!(riinaOpenDocuments:reply:)),
        (CORE_EVENT, REOPEN, sel!(riinaReopen:reply:)),
        (CORE_EVENT, QUIT, sel!(riinaQuit:reply:)),
    ]
}

fn register(manager: &NSAppleEventManager, target: &LaunchTarget) {
    for (class, event, selector) in handlers() {
        // SAFETY: each selector takes the Apple event and optional reply. The
        // registration guard retains the matching target throughout dispatch.
        unsafe {
            let _: () = objc2::msg_send![
                manager, setEventHandler: target, andSelector: selector,
                forEventClass: class, andEventID: event
            ];
        }
    }
}

fn direct_object(event: &NSAppleEventDescriptor) -> Option<Retained<NSAppleEventDescriptor>> {
    // SAFETY: the direct-object keyword is a four-byte AEKeyword.
    unsafe { objc2::msg_send![event, paramDescriptorForKeyword: DIRECT_OBJECT] }
}

fn document_paths(list: &NSAppleEventDescriptor) -> Result<Vec<String>, &'static str> {
    (1..=list.numberOfItems())
        .map(|index| {
            let url = list
                .descriptorAtIndex(index)
                // Foundation can also coerce text into a local file URL.
                .filter(|item| {
                    matches!(
                        &item.descriptorType().to_be_bytes(),
                        b"furl" | b"alis" | b"fsrf"
                    )
                })
                .and_then(|item| item.fileURLValue())
                .ok_or("The event contains a non-file reference")?;
            crate::platform::macos::file_panel::file_path(&url)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::document_paths;
    use objc2_foundation::{NSAppleEventDescriptor as Descriptor, NSString, NSURL};

    #[test]
    fn finder_document_lists_keep_file_order_spaces_and_unicode() {
        let list = Descriptor::listDescriptor();
        let expected = ["/tmp/First File.avi", "/tmp/山 clip.mkv"];
        for (index, path) in (1..).zip(expected) {
            let url = NSURL::fileURLWithPath(&NSString::from_str(path));
            list.insertDescriptor_atIndex(&Descriptor::descriptorWithFileURL(&url), index);
        }
        assert_eq!(document_paths(&list).unwrap(), expected);
        let invalid = Descriptor::descriptorWithString(&NSString::from_str("https://example.org"));
        list.insertDescriptor_atIndex(&invalid, 3);
        assert!(document_paths(&list).is_err());
    }
}
