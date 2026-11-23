use gtk::prelude::*;
use riina_core::plugin_window::{Frame, Properties};
use slint::{CloseRequestResponse, ComponentHandle};
use wry::{WebView, WebViewBuilder, WebViewBuilderExtUnix};

pub(in crate::platform) struct Shell {
    window: gtk::Window,
    content: gtk::Box,
}

impl Shell {
    pub(in crate::platform) fn new(
        _parent: &(impl ComponentHandle + 'static),
        title: &str,
        on_close: impl Fn() -> CloseRequestResponse + 'static,
    ) -> Result<Self, String> {
        if !gtk::is_initialized_main_thread() {
            gtk::init().map_err(|error| format!("Could not start GTK: {error}"))?;
        }
        let window = gtk::Window::new(gtk::WindowType::Toplevel);
        window.set_title(title);
        window.set_default_size(600, 400);
        window.set_position(gtk::WindowPosition::Center);
        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        window.add(&content);
        window.connect_delete_event(move |window, _| {
            if matches!(on_close(), CloseRequestResponse::HideWindow) {
                window.hide();
            }
            gtk::glib::Propagation::Stop
        });
        Ok(Self { window, content })
    }

    pub(in crate::platform) fn can_build(&self) -> bool {
        true
    }

    pub(in crate::platform) fn build_webview(
        &self,
        builder: WebViewBuilder<'_>,
    ) -> Result<WebView, String> {
        builder
            .build_gtk(&self.content)
            .map_err(|error| error.to_string())
    }

    pub(in crate::platform) fn set_visible(&self, visible: bool) -> Result<(), String> {
        if visible {
            self.window.show_all();
            self.window.present();
        } else {
            self.window.hide();
        }
        Ok(())
    }

    pub(in crate::platform) fn set_properties(&self, title: &str, properties: &Properties) {
        self.window.set_title(title);
        self.window
            .set_resizable(properties.resizable.unwrap_or(true));
        self.window.set_decorated(
            !properties.hide_title_bar.unwrap_or(false)
                && !properties.full_size_content_view.unwrap_or(false),
        );
    }

    pub(in crate::platform) fn set_frame(&self, frame: Frame, previous: Frame) {
        let (width, height) = if self.window.is_visible() {
            self.window.size()
        } else {
            self.window.default_size()
        };
        let width = frame.width.map_or(width, |width| width.round() as i32);
        let height = frame.height.map_or(height, |height| height.round() as i32);
        if frame.width != previous.width || frame.height != previous.height {
            self.window.resize(width, height);
        }
        if frame.x != previous.x || frame.y != previous.y {
            let (x, y) = self.window.position();
            let x = frame
                .x
                .filter(|x| Some(*x) != previous.x)
                .map_or(x, |x| x.round() as i32);
            let y = frame
                .y
                .filter(|y| Some(*y) != previous.y)
                .and_then(|position| {
                    let monitor = gtk::gdk::Display::default()?.primary_monitor()?.geometry();
                    Some(monitor.y() + monitor.height() - height - position.round() as i32)
                })
                .unwrap_or(y);
            self.window.move_(x, y);
        }
    }
}

impl Drop for Shell {
    fn drop(&mut self) {
        // SAFETY: the parent is on the GTK UI thread and Wry's child has dropped.
        unsafe { self.window.destroy() };
    }
}
