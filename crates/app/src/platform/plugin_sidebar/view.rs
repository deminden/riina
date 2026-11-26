//! Select a native child or a composited surface without changing document ownership.
use slint::winit_030::winit::window::Window;
use wry::{Rect, WebView, WebViewBuilder};

#[cfg(target_os = "linux")]
use super::super::unix::{offscreen_webview::OffscreenWebView, plugin_shortcuts};
#[cfg(target_os = "linux")]
use gtk::gdk;
#[cfg(target_os = "linux")]
use riina_ui::PlayerPluginPointerPhase as Phase;
#[cfg(target_os = "linux")]
use std::cell::Cell;

pub(super) enum View {
    Native(WebView),
    #[cfg(target_os = "linux")]
    Composited(CompositedView),
}

impl View {
    pub(super) fn new(
        builder: WebViewBuilder,
        parent: &Window,
        viewport: Rect,
        url: &str,
    ) -> Result<Self, String> {
        #[cfg(target_os = "linux")]
        {
            use slint::winit_030::winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

            let handle = parent
                .window_handle()
                .map_err(|error| format!("Could not inspect the Linux player window: {error}"))?;
            match handle.as_raw() {
                RawWindowHandle::Wayland(_) => {
                    return Ok(Self::Composited(CompositedView {
                        surface: OffscreenWebView::build(builder, viewport, url)?,
                        held: Cell::default(),
                        focused: Cell::default(),
                    }));
                }
                RawWindowHandle::Xlib(_) => {
                    gtk::init().map_err(super::webview_error)?;
                }
                _ => return Err("The Linux player window cannot host a plugin sidebar".into()),
            }
        }
        #[cfg(not(target_os = "linux"))]
        let _ = viewport;
        builder
            .with_url(url)
            .build_as_child(parent)
            .map(Self::Native)
            .map_err(super::webview_error)
    }

    pub(super) fn browser(&self) -> &WebView {
        match self {
            Self::Native(view) => view,
            #[cfg(target_os = "linux")]
            Self::Composited(view) => view.surface.browser(),
        }
    }

    pub(super) fn set_bounds(&self, bounds: Rect) -> Result<(), String> {
        match self {
            Self::Native(view) => view.set_bounds(bounds).map_err(super::webview_error),
            #[cfg(target_os = "linux")]
            Self::Composited(view) => view.surface.set_bounds(bounds),
        }
    }

    pub(super) fn set_visible(&self, visible: bool) -> Result<(), String> {
        match self {
            Self::Native(view) => view.set_visible(visible).map_err(super::webview_error),
            #[cfg(target_os = "linux")]
            Self::Composited(view) => {
                if !visible {
                    view.focus(false);
                    view.pointer_left();
                    view.surface.invalidate();
                }
                Ok(())
            }
        }
    }

    #[cfg(target_os = "linux")]
    pub(super) fn composited(&self) -> Option<&CompositedView> {
        match self {
            Self::Native(_) => None,
            Self::Composited(view) => Some(view),
        }
    }

    #[cfg(target_os = "linux")]
    pub(super) fn bind_shortcuts(&self, owner: slint::Weak<crate::PlayerWindow>) {
        plugin_shortcuts::bind(self.browser(), move || Some(owner.clone()), None);
    }
}

#[cfg(target_os = "linux")]
impl Drop for View {
    fn drop(&mut self) {
        if let Self::Native(view) = self {
            // Unmap the GTK child before Wry destroys its foreign X11 window.
            let _ = view.set_visible(false);
        }
    }
}

#[cfg(target_os = "linux")]
pub(super) struct CompositedView {
    pub(super) surface: OffscreenWebView,
    held: Cell<u32>,
    focused: Cell<bool>,
}

#[cfg(target_os = "linux")]
impl CompositedView {
    pub(super) fn focus(&self, focused: bool) {
        if self.focused.replace(focused) != focused {
            if !focused {
                self.cancel_press();
            }
            self.surface.focus(focused);
        }
    }

    pub(super) fn pointer(&self, phase: Phase, button: i32, point: (f64, f64), modifiers: u32) {
        use gdk::EventType;
        let held = self.held.get();
        let button = u32::try_from(button).unwrap_or_default();
        match phase {
            Phase::Move => self
                .surface
                .pointer(EventType::MotionNotify, point, 0, held, modifiers),
            Phase::Cancel => self.cancel_press(),
            Phase::Down if (1..=3).contains(&button) => {
                self.focus(true);
                self.surface
                    .pointer(EventType::ButtonPress, point, button, held, modifiers);
                self.held.set(held | (1 << (button - 1)));
            }
            Phase::Up if (1..=3).contains(&button) => {
                let mask = 1 << (button - 1);
                if held & mask != 0 {
                    self.surface
                        .pointer(EventType::ButtonRelease, point, button, held, modifiers);
                    self.held.set(held & !mask);
                }
            }
            _ => {}
        }
    }

    pub(super) fn pointer_left(&self) {
        self.surface.pointer(
            gdk::EventType::MotionNotify,
            (-1.0, -1.0),
            0,
            self.held.get(),
            0,
        );
    }

    pub(super) fn scroll(&self, point: (f64, f64), delta: (f64, f64), modifiers: u32) {
        self.surface
            .scroll(point, delta, self.held.get(), modifiers);
    }

    fn cancel_press(&self) {
        let mut held = self.held.take();
        for button in 1..=3 {
            let mask = 1 << (button - 1);
            if held & mask != 0 {
                self.surface
                    .pointer(gdk::EventType::ButtonRelease, (-1.0, -1.0), button, held, 0);
                held &= !mask;
            }
        }
    }
}
