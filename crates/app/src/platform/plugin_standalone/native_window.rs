//! An initially hidden native parent lets a standalone page load before Open.
use crate::PlayerWindow;
use riina_core::plugin_window::{Frame, Properties};
use slint::winit_030::{
    CustomApplicationHandler, EventResult, invoke_from_active_event_loop,
    winit::{
        dpi::LogicalSize,
        event::WindowEvent,
        event_loop::ActiveEventLoop,
        window::{Window, WindowId},
    },
};
#[cfg(target_os = "windows")]
use slint::{ComponentHandle, winit_030::WinitWindowAccessor};
use std::{
    cell::{OnceCell, RefCell},
    collections::HashMap,
    rc::{Rc, Weak},
    sync::{
        Arc,
        mpsc::{Receiver, TryRecvError, sync_channel},
    },
};
use wry::{WebView, WebViewBuilder};

type Close = dyn Fn();

struct CloseRoute {
    window: Weak<Window>,
    notify: Weak<Close>,
}

thread_local! {
    // Only custom Winit windows use this route. Slint still owns its windows.
    static WINDOWS: RefCell<HashMap<WindowId, CloseRoute>> = RefCell::default();
}

pub(crate) struct Events;

impl CustomApplicationHandler for Events {
    fn window_event(
        &mut self,
        _: &ActiveEventLoop,
        id: WindowId,
        _: Option<&Window>,
        _: Option<&slint::Window>,
        event: &WindowEvent,
    ) -> EventResult {
        #[cfg(target_os = "windows")]
        crate::platform::inactive_playback::window_event(id, event);
        if !matches!(event, WindowEvent::CloseRequested) {
            return EventResult::Propagate;
        }
        let owner = WINDOWS.with_borrow(|windows| {
            let route = windows.get(&id)?;
            Some((route.window.upgrade()?, route.notify.upgrade()?))
        });
        let Some((window, close)) = owner else {
            return EventResult::Propagate;
        };
        // The realm must observe close before a later Open can raise this same
        // page again. Hide the native window first for a synchronous observer.
        window.set_visible(false);
        close();
        EventResult::PreventDefault
    }
}

pub(super) struct Shell {
    pending: Receiver<Result<Window, String>>,
    native: OnceCell<Result<Rc<Window>, String>>,
    on_close: Rc<Close>,
    #[cfg(target_os = "macos")]
    browser_bounds: std::cell::Cell<Option<wry::Rect>>,
}

impl Shell {
    pub(super) fn new(
        parent: Option<&PlayerWindow>,
        title: &str,
        on_close: impl Fn() + 'static,
        wake: Option<Arc<dyn Fn() + Send + Sync>>,
    ) -> Result<Self, String> {
        let (reply, pending) = sync_channel(1);
        let attributes = Window::default_attributes()
            .with_title(title)
            .with_inner_size(LogicalSize::new(600.0, 400.0))
            .with_visible(false);
        #[cfg(target_os = "windows")]
        let parent = parent.map(ComponentHandle::as_weak);
        #[cfg(not(target_os = "windows"))]
        let _ = parent;
        invoke_from_active_event_loop(move |event_loop| {
            let result = event_loop
                .create_window(attributes)
                .map_err(|error| error.to_string())
                .inspect(|window| {
                    #[cfg(target_os = "windows")]
                    if let Some(parent) = parent.and_then(|parent| parent.upgrade()) {
                        parent.window().with_winit_window(|owner| {
                            super::super::windows::window_position::center_auxiliary_native(
                                window, owner,
                            );
                        });
                    }
                    #[cfg(target_os = "macos")]
                    super::appkit::center(window);
                    let _ = window;
                });
            let _ = reply.send(result);
            if let Some(wake) = wake {
                wake();
            }
        })
        .map_err(|error| error.to_string())?;
        Ok(Self {
            pending,
            native: OnceCell::new(),
            on_close: Rc::new(on_close),
            #[cfg(target_os = "macos")]
            browser_bounds: Default::default(),
        })
    }

    pub(super) fn can_build(&self) -> bool {
        if self.native.get().is_none() {
            let result = match self.pending.try_recv() {
                Ok(result) => result.map(Rc::new),
                Err(TryRecvError::Empty) => return false,
                Err(TryRecvError::Disconnected) => {
                    Err("The standalone window was not created".into())
                }
            };
            if let Ok(window) = &result {
                WINDOWS.with_borrow_mut(|windows| {
                    windows.insert(
                        window.id(),
                        CloseRoute {
                            window: Rc::downgrade(window),
                            notify: Rc::downgrade(&self.on_close),
                        },
                    );
                });
            }
            let _ = self.native.set(result);
        }
        true
    }

    fn window(&self) -> Result<&Window, String> {
        self.native
            .get()
            .ok_or_else(|| "The standalone window is still being created".to_owned())?
            .as_deref()
            .map_err(Clone::clone)
    }

    pub(super) fn is_visible(&self) -> bool {
        self.window()
            .is_ok_and(|window| window.is_visible() == Some(true))
    }

    pub(super) fn build_webview(&self, builder: WebViewBuilder<'_>) -> Result<WebView, String> {
        let window = self.window()?;
        #[cfg(target_os = "macos")]
        let result = {
            let bounds = super::appkit::content_bounds(window);
            self.browser_bounds.set(Some(bounds));
            builder.with_bounds(bounds).build_as_child(window)
        };
        #[cfg(not(target_os = "macos"))]
        let result = builder.build(window);
        result.map_err(|error| error.to_string())
    }

    pub(super) fn set_visible(&self, visible: bool) -> Result<(), String> {
        let window = self.window()?;
        window.set_visible(visible);
        if visible {
            window.focus_window();
        }
        Ok(())
    }

    pub(super) fn set_properties(&self, title: &str, properties: &Properties) {
        let Ok(window) = self.window() else { return };
        window.set_title(title);
        window.set_resizable(properties.resizable.unwrap_or(true));
        #[cfg(target_os = "windows")]
        window.set_decorations(
            !properties.hide_title_bar.unwrap_or(false)
                && !properties.full_size_content_view.unwrap_or(false),
        );
        #[cfg(target_os = "macos")]
        super::appkit::set_properties(window, properties);
    }

    pub(super) fn set_frame(&self, frame: Frame) {
        if frame == Frame::default() {
            return;
        }
        let Ok(window) = self.window() else { return };
        #[cfg(target_os = "macos")]
        super::appkit::set_frame(window, frame);
        #[cfg(not(target_os = "macos"))]
        set_frame(window, frame);
    }

    #[cfg(target_os = "macos")]
    pub(super) fn refresh_browser_bounds(&self, browser: &WebView) -> Result<(), String> {
        let bounds = super::appkit::content_bounds(self.window()?);
        if Some(bounds) != self.browser_bounds.get() {
            browser
                .set_bounds(bounds)
                .map_err(|error| error.to_string())?;
            self.browser_bounds.set(Some(bounds));
        }
        Ok(())
    }
}

impl Drop for Shell {
    fn drop(&mut self) {
        if let Ok(window) = self.window() {
            WINDOWS.with_borrow_mut(|windows| windows.remove(&window.id()));
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn set_frame(window: &Window, frame: Frame) {
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
