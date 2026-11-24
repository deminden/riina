//! Register the URL Apple event independently of the windowing delegate.
use crate::browser_links::{Inbox, Link};
use objc2::{
    DefinedClass, MainThreadMarker, MainThreadOnly, define_class, rc::Retained, runtime::NSObject,
    sel,
};
use objc2_foundation::{NSAppleEventDescriptor, NSAppleEventManager, NSObjectProtocol};

const URL_EVENT: u32 = u32::from_be_bytes(*b"GURL");
const DIRECT_OBJECT: u32 = u32::from_be_bytes(*b"----");

define_class!(
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = Inbox]
    struct BrowserTarget;

    // SAFETY: the superclass implements NSObject's protocol.
    unsafe impl NSObjectProtocol for BrowserTarget {}

    impl BrowserTarget {
        #[unsafe(method(riinaOpenURL:reply:))]
        fn open_url(&self, event: &NSAppleEventDescriptor, _reply: Option<&NSAppleEventDescriptor>) {
            // SAFETY: the direct object is an AEKeyword, and the returned object is optional.
            let descriptor: Option<Retained<NSAppleEventDescriptor>> =
                unsafe { objc2::msg_send![event, paramDescriptorForKeyword: DIRECT_OBJECT] };
            let Some(text) = descriptor.and_then(|descriptor| descriptor.stringValue()) else { return };
            match Link::parse(&text.to_string()) {
                Ok(link) => { self.ivars().push(link); }
                Err(error) => crate::diagnostics::warn!("Discarded browser link: {error}"),
            }
        }
    }
);

pub(crate) struct BrowserEvents {
    manager: Retained<NSAppleEventManager>,
    _target: Retained<BrowserTarget>,
}

impl BrowserEvents {
    pub(crate) fn new(inbox: Inbox) -> Option<Self> {
        let main = MainThreadMarker::new()?;
        let target = BrowserTarget::alloc(main).set_ivars(inbox);
        // SAFETY: NSObject initializes the allocated target. The method selector
        // uses the documented event/reply signature and the target stays retained.
        let (manager, target) = unsafe {
            let target: Retained<BrowserTarget> = objc2::msg_send![super(target), init];
            let manager = NSAppleEventManager::sharedAppleEventManager();
            let _: () = objc2::msg_send![
                &*manager,
                setEventHandler: &*target,
                andSelector: sel!(riinaOpenURL:reply:),
                forEventClass: URL_EVENT,
                andEventID: URL_EVENT
            ];
            (manager, target)
        };
        Some(Self {
            manager,
            _target: target,
        })
    }
}

impl Drop for BrowserEvents {
    fn drop(&mut self) {
        // SAFETY: this guard owns the matching event registration and retained target.
        let _: () = unsafe {
            objc2::msg_send![&*self.manager, removeEventHandlerForEventClass: URL_EVENT, andEventID: URL_EVENT]
        };
    }
}
