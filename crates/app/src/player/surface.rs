//! A playback session can present its own window or a separate native frame.
use crate::{NativeVideoHost, PlayerWindow, WelcomeWindow};
use slint::{ComponentHandle, winit_030::WinitWindowAccessor};

#[derive(Clone, Default)]
pub(crate) struct PlayerDisplay {
    pub(crate) welcome: Option<slint::Weak<WelcomeWindow>>,
    pub(crate) host: Option<slint::Weak<NativeVideoHost>>,
}

impl PlayerDisplay {
    pub(crate) fn reveal(&self, app: &PlayerWindow) -> Result<(), slint::PlatformError> {
        if let Some(welcome) = self.welcome.as_ref().and_then(slint::Weak::upgrade)
            && welcome.window().is_visible()
        {
            crate::platform::set_native_video_host_visible(self.host.as_ref(), true);
            crate::platform::show_window(app)?;
            welcome.hide()?;
        }
        Ok(())
    }

    pub(crate) fn focus(&self, app: &PlayerWindow) {
        let host = self.host.as_ref().and_then(slint::Weak::upgrade);
        activate(
            host.as_ref()
                .map_or_else(|| app.window(), ComponentHandle::window),
        );
    }
}

fn activate(window: &slint::Window) {
    window.with_winit_window(|native| {
        native.set_minimized(false);
        native.focus_window();
    });
}

pub(crate) enum SwitchableWindow {
    Welcome(WelcomeWindow),
    Native(NativeVideoHost),
    Shared(PlayerWindow),
}

impl SwitchableWindow {
    pub(crate) fn focus(&self) {
        let handle = match self {
            Self::Welcome(window) => window.window(),
            Self::Native(window) => window.window(),
            Self::Shared(window) => window.window(),
        };
        activate(handle);
    }
}

impl PlayerDisplay {
    pub(crate) fn switchable(&self, app: &PlayerWindow) -> Option<SwitchableWindow> {
        if let Some(welcome) = self.welcome.as_ref().and_then(slint::Weak::upgrade)
            && listed(welcome.window())
        {
            return Some(SwitchableWindow::Welcome(welcome));
        }
        if let Some(host) = &self.host {
            let host = host.upgrade()?;
            return listed(host.window()).then_some(SwitchableWindow::Native(host));
        }
        listed(app.window()).then(|| SwitchableWindow::Shared(app.clone_strong()))
    }
}

fn listed(window: &slint::Window) -> bool {
    // A minimized frame remains a destination. The native visibility check
    // also excludes idle managed frames hidden directly by the platform.
    window
        .with_winit_window(|native| {
            native.is_minimized() == Some(true)
                || native.is_visible().unwrap_or_else(|| window.is_visible())
        })
        .unwrap_or_else(|| window.is_visible())
}
