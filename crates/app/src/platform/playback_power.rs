//! Map app-wide playback activity to the host's idle power APIs.
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
#[path = "playback_power/macos.rs"]
mod native;
#[cfg(target_os = "windows")]
#[path = "playback_power/windows.rs"]
mod native;

#[cfg(target_os = "linux")]
pub(crate) use linux::Controller;

#[cfg(not(target_os = "linux"))]
#[derive(Default)]
pub(crate) struct Controller {
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    lease: Option<native::Lease>,
}

#[cfg(not(target_os = "linux"))]
impl Controller {
    pub(crate) fn set(&mut self, mode: riina_core::playback_power::Mode) {
        #[cfg(any(target_os = "windows", target_os = "macos"))]
        {
            self.lease = native::Lease::new(mode);
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        let _ = mode;
    }
}
