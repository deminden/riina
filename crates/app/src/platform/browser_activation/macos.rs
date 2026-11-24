//! Receive Finder, Dock and URL events while Winit retains the application delegate.
mod idle_menu;

use crate::{
    browser_links::{Inbox, Link},
    player_manager::PlayerManager,
};
use block2::RcBlock;
use objc2::{
    DefinedClass, MainThreadMarker, MainThreadOnly, define_class,
    rc::Retained,
    runtime::{AnyObject, NSObject, ProtocolObject, Sel},
    sel,
};
use objc2_app_kit::{NSApplication, NSApplicationWillFinishLaunchingNotification};
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
    document_timer: slint::Timer,
}

impl Launch {
    fn open_documents(self: &Rc<Self>, paths: Vec<String>) {
        if paths.is_empty() || self.manager.is_quitting() {
            return;
        }
        self.default_action.take();
        self.documents.borrow_mut().extend(paths);
        let launch = Rc::downgrade(self);
        // Finder may split a single multiple-file action across nearby events.
        self.document_timer.start(
            slint::TimerMode::SingleShot,
            Duration::from_millis(160),
            move || {
                if let Some(launch) = launch.upgrade() {
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
                    if launch.browser.push(link) {
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
        fn idle_open(&self, _sender: Option<&AnyObject>) {
            self.ivars().manager.open_application_panel();
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
    _target: Retained<LaunchTarget>,
}

impl ApplicationEvents {
    pub(crate) fn new(
        browser: Inbox,
        manager: PlayerManager,
        default_action: Option<ActionAfterLaunch>,
    ) -> Option<Self> {
        let main = MainThreadMarker::new()?;
        let launch = Rc::new(Launch {
            manager,
            browser,
            default_action: Cell::new(default_action),
            documents: RefCell::default(),
            document_timer: slint::Timer::default(),
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
        idle_menu::prepare(main, &target);
        if default_action.is_some() {
            slint::Timer::single_shot(Duration::from_millis(220), move || launch.launch_default());
        }
        Some(Self {
            manager,
            center,
            will_launch,
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
            for (class, event, _) in handlers() {
                let _: () = objc2::msg_send![
                    &*self.manager, removeEventHandlerForEventClass: class, andEventID: event
                ];
            }
        }
    }
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
