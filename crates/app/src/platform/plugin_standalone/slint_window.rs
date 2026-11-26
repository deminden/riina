use crate::platform;
use riina_ui::PluginStandaloneWindow;
use slint::{CloseRequestResponse, ComponentHandle, winit_030::WinitWindowAccessor};
use wry::{WebView, WebViewBuilder};

pub(in crate::platform) struct Shell {
    window: PluginStandaloneWindow,
    #[cfg(target_os = "macos")]
    browser_bounds: std::cell::Cell<Option<wry::Rect>>,
}

impl Shell {
    pub(in crate::platform) fn new(
        parent: Option<&(impl ComponentHandle + 'static)>,
        title: &str,
        on_close: impl Fn() -> CloseRequestResponse + 'static,
    ) -> Result<Self, String> {
        let window = PluginStandaloneWindow::new().map_err(|error| error.to_string())?;
        window.set_window_title(title.into());
        window.window().on_close_requested(on_close);
        if let Some(parent) = parent {
            platform::center_auxiliary_window(&window, parent);
        }
        platform::show_window(&window).map_err(|error| error.to_string())?;
        Ok(Self {
            window,
            #[cfg(target_os = "macos")]
            browser_bounds: Default::default(),
        })
    }

    pub(in crate::platform) fn can_build(&self) -> bool {
        self.window.window().has_winit_window()
    }

    pub(in crate::platform) fn build_webview(
        &self,
        builder: WebViewBuilder<'_>,
    ) -> Result<WebView, String> {
        self.window
            .window()
            .with_winit_window(|window| {
                #[cfg(target_os = "macos")]
                {
                    // Keep Slint's NSView attached so Winit retains its native
                    // window handle and titlebar properties after WKWebView attaches.
                    let bounds = super::appkit::content_bounds(window);
                    self.browser_bounds.set(Some(bounds));
                    builder.with_bounds(bounds).build_as_child(window)
                }
                #[cfg(not(target_os = "macos"))]
                builder.build(window)
            })
            .ok_or("The native plugin window is unavailable")?
            .map_err(|error| error.to_string())
    }

    pub(in crate::platform) fn set_visible(&self, visible: bool) -> Result<(), String> {
        if visible {
            platform::show_window(&self.window).map_err(|error| error.to_string())?;
            self.window
                .window()
                .with_winit_window(|window| window.focus_window());
        } else {
            self.window.hide().map_err(|error| error.to_string())?;
        }
        Ok(())
    }
}

impl Drop for Shell {
    fn drop(&mut self) {
        let _ = self.window.hide();
    }
}

#[cfg(target_os = "macos")]
impl Shell {
    pub(in crate::platform) fn refresh_browser_bounds(
        &self,
        browser: &WebView,
    ) -> Result<(), String> {
        if let Some(bounds) = self
            .window
            .window()
            .with_winit_window(super::appkit::content_bounds)
            && Some(bounds) != self.browser_bounds.get()
        {
            browser
                .set_bounds(bounds)
                .map_err(|error| error.to_string())?;
            self.browser_bounds.set(Some(bounds));
        }
        Ok(())
    }
}
