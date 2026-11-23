use super::page::Page;
use crate::{PlayerWindow, platform::unix::plugin_shortcuts, plugin_runtime::standalone::Endpoint};
use gtk::prelude::*;
use riina_core::plugin_window::{Frame, Presentation, Properties};
use slint::ComponentHandle;
use std::sync::Arc;
use wry::{WebView, WebViewBuilderExtUnix};

pub(super) struct Shell {
    window: gtk::Window,
    content: gtk::Box,
    player: slint::Weak<PlayerWindow>,
}

impl Shell {
    pub(super) fn new(
        app: &PlayerWindow,
        endpoint: &Arc<Endpoint>,
        view: &Presentation,
    ) -> Result<Self, String> {
        if !gtk::is_initialized_main_thread() {
            gtk::init().map_err(|error| format!("Could not start GTK: {error}"))?;
        }
        let window = gtk::Window::new(gtk::WindowType::Toplevel);
        window.set_default_size(600, 400);
        window.set_position(gtk::WindowPosition::Center);
        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        window.add(&content);
        let owner = Arc::downgrade(endpoint);
        window.connect_delete_event(move |window, _| {
            if let Some(owner) = owner.upgrade() {
                owner.user_closed();
            }
            window.hide();
            gtk::glib::Propagation::Stop
        });
        let shell = Self {
            window,
            content,
            player: app.as_weak(),
        };
        shell.set_properties(&endpoint.name, &view.properties);
        Ok(shell)
    }

    pub(super) fn can_build(&self) -> bool {
        true
    }

    pub(super) fn build_webview(
        &self,
        page: &Page,
        endpoint: &Arc<Endpoint>,
        url: &str,
    ) -> Result<WebView, String> {
        let webview = page
            .builder(endpoint, url)
            .build_gtk(&self.content)
            .map_err(|error| error.to_string())?;
        plugin_shortcuts::bind(&webview, self.player.clone());
        Ok(webview)
    }

    pub(super) fn set_visible(&self, visible: bool) -> Result<(), String> {
        if visible {
            self.window.show_all();
            self.window.present();
        } else {
            self.window.hide();
        }
        Ok(())
    }

    pub(super) fn set_properties(&self, name: &str, properties: &Properties) {
        self.window.set_title(&format!(
            "{} — {name}",
            properties.title.as_deref().unwrap_or("Window")
        ));
        self.window
            .set_resizable(properties.resizable.unwrap_or(true));
        self.window.set_decorated(
            !properties.hide_title_bar.unwrap_or(false)
                && !properties.full_size_content_view.unwrap_or(false),
        );
    }

    pub(super) fn set_frame(&self, frame: Frame, previous: Frame) {
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
