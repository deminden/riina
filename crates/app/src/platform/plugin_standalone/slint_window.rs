#[cfg(target_os = "macos")]
#[path = "appkit.rs"]
mod appkit;

use crate::platform;
use riina_core::plugin_window::{Frame, Properties};
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
                    let bounds = appkit::content_bounds(window);
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
            appkit::set_properties(window, properties);
        });
    }

    pub(in crate::platform) fn set_frame(&self, frame: Frame) {
        if frame == Frame::default() {
            return;
        }
        self.window.window().with_winit_window(|window| {
            #[cfg(target_os = "macos")]
            appkit::set_frame(window, frame);
            #[cfg(not(target_os = "macos"))]
            set_frame(window, frame);
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
        if let Some(bounds) = self
            .window
            .window()
            .with_winit_window(appkit::content_bounds)
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

#[cfg(not(target_os = "macos"))]
fn set_frame(window: &slint::winit_030::winit::window::Window, frame: Frame) {
    use wry::dpi::{LogicalPosition, LogicalSize};

    let scale = window.scale_factor();
    let outer = window.outer_size().to_logical::<f64>(scale);
    let inner = window.inner_size().to_logical::<f64>(scale);
    let width = frame.width.unwrap_or(outer.width);
    let height = frame.height.unwrap_or(outer.height);
    let monitor_bottom = window.primary_monitor().map(|monitor| {
        (f64::from(monitor.position().y) + f64::from(monitor.size().height)) / scale
    });
    let position = window
        .outer_position()
        .ok()
        .map(|value| value.to_logical::<f64>(scale));
    if frame.width.is_some() || frame.height.is_some() {
        let _ = window.request_inner_size(LogicalSize::new(
            (width - (outer.width - inner.width)).max(1.0),
            (height - (outer.height - inner.height)).max(1.0),
        ));
    }
    if let Some(position) = position {
        // Plugin coordinates start at the primary display's bottom edge, while
        // Winit positions the frame from its top edge. A height-only edit must
        // keep the lower edge in place as AppKit does.
        let top = frame
            .y
            .zip(monitor_bottom)
            .map_or(position.y + outer.height - height, |(y, bottom)| {
                bottom - y - height
            });
        if frame.x.is_some() || frame.y.is_some() || frame.height.is_some() {
            window.set_outer_position(LogicalPosition::new(frame.x.unwrap_or(position.x), top));
        }
    }
}
