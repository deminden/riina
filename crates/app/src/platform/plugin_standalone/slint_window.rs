use crate::platform;
use riina_core::plugin_window::{Frame, Properties};
use riina_ui::PluginStandaloneWindow;
use slint::{CloseRequestResponse, ComponentHandle, winit_030::WinitWindowAccessor};
use wry::{
    WebView, WebViewBuilder,
    dpi::{LogicalPosition, LogicalSize},
};

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

    #[cfg(target_os = "macos")]
    pub(super) fn is_visible(&self) -> bool {
        self.window.window().is_visible()
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
                    let bounds = mac_bounds(window);
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

    pub(in crate::platform) fn set_properties(&self, title: &str, properties: &Properties) {
        self.window.set_window_title(title.into());
        self.window.window().with_winit_window(|window| {
            window.set_resizable(properties.resizable.unwrap_or(true));
            #[cfg(target_os = "windows")]
            window.set_decorations(
                !properties.hide_title_bar.unwrap_or(false)
                    && !properties.full_size_content_view.unwrap_or(false),
            );
            #[cfg(target_os = "macos")]
            mac_properties(window, properties);
        });
    }

    pub(in crate::platform) fn set_frame(&self, frame: Frame, previous: Frame) {
        self.window.window().with_winit_window(|window| {
            let scale = window.scale_factor();
            let current = window.inner_size().to_logical::<f64>(scale);
            let width = changed(frame.width, previous.width);
            let height = changed(frame.height, previous.height);
            if width.is_some() || height.is_some() {
                let _ = window.request_inner_size(LogicalSize::new(
                    width.unwrap_or(current.width),
                    height.unwrap_or(current.height),
                ));
            }
            let x = changed(frame.x, previous.x);
            let y = changed(frame.y, previous.y);
            if (x.is_some() || y.is_some())
                && let Ok(position) = window.outer_position()
            {
                let position = position.to_logical::<f64>(scale);
                let top = y.and_then(|y| {
                    window.primary_monitor().map(|monitor| {
                        let bottom =
                            f64::from(monitor.position().y) + f64::from(monitor.size().height);
                        let border = window
                            .outer_size()
                            .height
                            .saturating_sub(window.inner_size().height);
                        (bottom - f64::from(border)) / scale - y - height.unwrap_or(current.height)
                    })
                });
                window.set_outer_position(LogicalPosition::new(
                    x.unwrap_or(position.x),
                    top.unwrap_or(position.y),
                ));
            }
        });
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
        if let Some(bounds) = self.window.window().with_winit_window(mac_bounds)
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

#[cfg(target_os = "macos")]
fn mac_bounds(window: &slint::winit_030::winit::window::Window) -> wry::Rect {
    wry::Rect {
        position: LogicalPosition::new(0.0, 0.0).into(),
        size: window
            .inner_size()
            .to_logical::<f64>(window.scale_factor())
            .into(),
    }
}

fn changed(current: Option<f64>, previous: Option<f64>) -> Option<f64> {
    current.filter(|value| Some(*value) != previous)
}

#[cfg(target_os = "macos")]
fn mac_properties(window: &slint::winit_030::winit::window::Window, properties: &Properties) {
    use objc2_app_kit::{NSView, NSWindowStyleMask, NSWindowTitleVisibility};
    use slint::winit_030::winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let Ok(handle) = window.window_handle() else {
        return;
    };
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return;
    };
    // SAFETY: Winit owns this NSView on the calling AppKit thread for the duration of the borrow.
    let view = unsafe { handle.ns_view.cast::<NSView>().as_ref() };
    let Some(window) = view.window() else { return };
    let mut mask = window.styleMask();
    mask.set(
        NSWindowStyleMask::FullSizeContentView,
        properties.full_size_content_view.unwrap_or(false),
    );
    window.setStyleMask(mask);
    let hidden = properties.hide_title_bar.unwrap_or(false);
    window.setTitlebarAppearsTransparent(hidden);
    window.setTitleVisibility(if hidden {
        NSWindowTitleVisibility::Hidden
    } else {
        NSWindowTitleVisibility::Visible
    });
    window.setMovableByWindowBackground(hidden);
}
