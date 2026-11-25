//! Optional platform delivery for releases selected by the shared updater.
#[cfg(target_os = "windows")]
pub(crate) use super::windows::update_installer::{Asset, InstallFlow, Package};
#[cfg(target_os = "windows")]
pub(crate) type Assets = Vec<Asset>;

#[cfg(not(target_os = "windows"))]
pub(crate) use browser::{Assets, InstallFlow, Package};

#[cfg(not(target_os = "windows"))]
mod browser {
    use crate::UpdateWindow;
    use semver::Version;

    pub(crate) type Assets = serde::de::IgnoredAny;

    pub(crate) enum Package {}

    impl Package {
        pub(crate) fn select(_: Assets, _: &str, _: &Version) -> Option<Self> {
            None
        }
    }

    #[derive(Default)]
    pub(crate) struct InstallFlow;

    impl InstallFlow {
        pub(crate) fn bind(&self, _: &UpdateWindow) {}

        pub(crate) fn active(&self) -> bool {
            false
        }

        pub(crate) fn offer(&self, _: &UpdateWindow, _: Option<Package>) {}

        pub(crate) fn dismiss(&self, _: &UpdateWindow) {}
    }
}
