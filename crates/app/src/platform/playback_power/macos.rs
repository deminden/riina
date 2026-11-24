use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_foundation::{NSActivityOptions, NSObjectProtocol, NSProcessInfo, NSString};
use riina_core::playback_power::Mode;

pub(super) struct Lease {
    process: Retained<NSProcessInfo>,
    activity: Retained<ProtocolObject<dyn NSObjectProtocol>>,
}

impl Lease {
    pub(super) fn new(mode: Mode) -> Option<Self> {
        let options = match mode {
            Mode::Idle => return None,
            Mode::System => NSActivityOptions::IdleSystemSleepDisabled,
            Mode::Display => NSActivityOptions::IdleDisplaySleepDisabled,
        };
        let process = NSProcessInfo::processInfo();
        let activity = process.beginActivityWithOptions_reason(
            options,
            &NSString::from_str("Riina playback is in progress"),
        );
        Some(Self { process, activity })
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        // SAFETY: the activity was created by this process and is ended once.
        unsafe { self.process.endActivity(&self.activity) };
    }
}
