//! Platform placement and resize operations shared by window-aspect policy.
use crate::{NativeVideoHost, PlayerWindow};
use riina_core::window_aspect::PixelSize;
use slint::winit_030::WinitWindowAccessor;

pub(crate) const USES_NATIVE_RESIZE_FILTER: bool = cfg!(target_os = "windows");

#[cfg(windows)]
pub(crate) fn auxiliary_work_area(
    native: &slint::winit_030::winit::window::Window,
    position: Option<[i32; 2]>,
) -> Option<riina_core::window_resize::ScreenRect> {
    use super::windows::window_position::{work_area, work_area_near_point};
    position
        .and_then(|[x, y]| work_area_near_point(x.saturating_add(64), y.saturating_add(20)))
        .or_else(|| work_area(native))
}

#[derive(Clone, Copy)]
pub(crate) enum ResizeAnchor {
    WindowCenter,
    ScreenCenter,
    TopLeft,
}

pub(crate) fn disable_plugin_window_animation(window: &slint::Window) {
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    window.with_winit_window(|window| {
        use slint::winit_030::winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let Ok(handle) = window.window_handle() else {
            return;
        };
        #[cfg(target_os = "windows")]
        if let RawWindowHandle::Win32(handle) = handle.as_raw() {
            use windows::Win32::{
                Foundation::HWND,
                Graphics::Dwm::{DWMWA_TRANSITIONS_FORCEDISABLED, DwmSetWindowAttribute},
            };
            let disabled = 1_i32;
            // SAFETY: the borrowed HWND is live and DWM reads a BOOL of this size.
            let _ = unsafe {
                DwmSetWindowAttribute(
                    HWND(handle.hwnd.get() as *mut _),
                    DWMWA_TRANSITIONS_FORCEDISABLED,
                    (&raw const disabled).cast(),
                    size_of::<i32>() as u32,
                )
            };
        }
        #[cfg(target_os = "macos")]
        if let RawWindowHandle::AppKit(handle) = handle.as_raw() {
            use objc2_app_kit::{NSView, NSWindowAnimationBehavior};
            // SAFETY: Winit's borrowed handle points to the live NSView on the UI thread.
            let view = unsafe { &*handle.ns_view.as_ptr().cast::<NSView>() };
            if let Some(window) = view.window() {
                window.setAnimationBehavior(NSWindowAnimationBehavior::None);
            }
        }
    });
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let _ = window;
}

#[cfg(target_os = "windows")]
pub(crate) fn media_resize_allowed(initial: bool, manually_opened: bool) -> bool {
    initial || manually_opened
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn media_resize_allowed(_: bool, _: bool) -> bool {
    true
}

/// A platform that resizes synchronously can keep its owner and overlays aligned.
#[cfg(target_os = "windows")]
pub(crate) fn resize_native_content(
    window: &slint::Window,
    target: PixelSize,
    anchor: ResizeAnchor,
    overlay: Option<&slint::Window>,
) -> Option<Result<(), String>> {
    let resize_content = super::windows::window_geometry::resize_content;
    Some(
        window
            .with_winit_window(|native| {
                if let Some(overlay) = overlay {
                    overlay.with_winit_window(|overlay| {
                        resize_content(native, target, anchor, Some(overlay))
                    })
                } else {
                    Some(resize_content(native, target, anchor, None))
                }
            })
            .flatten()
            .unwrap_or_else(|| Err("Could not access the player window".into())),
    )
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn resize_native_content(
    _: &slint::Window,
    _: PixelSize,
    _: ResizeAnchor,
    _: Option<&slint::Window>,
) -> Option<Result<(), String>> {
    None
}

#[cfg(target_os = "windows")]
pub(crate) fn content_limit(window: &slint::Window) -> Option<Option<PixelSize>> {
    Some(
        window
            .with_winit_window(super::windows::window_position::content_work_area)
            .flatten(),
    )
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn content_limit(window: &slint::Window) -> Option<Option<PixelSize>> {
    window.with_winit_window(|native| {
        native.current_monitor().map(|monitor| {
            let size = monitor.size();
            // winit reports the full display; reserve room for desktop chrome and a small margin.
            PixelSize {
                width: size.width.saturating_mul(96) / 100,
                height: size.height.saturating_mul(90) / 100,
            }
        })
    })
}

#[cfg(target_os = "windows")]
pub(crate) fn initial_frame_size(window: &slint::Window, _: PixelSize) -> Option<PixelSize> {
    // Windows synchronously applies the resize, including any new row in the native menu.
    window.with_winit_window(|native| {
        let outer = native.outer_size();
        PixelSize {
            width: outer.width,
            height: outer.height,
        }
    })
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn initial_frame_size(
    window: &slint::Window,
    content_size: PixelSize,
) -> Option<PixelSize> {
    window.with_winit_window(|native| {
        let outer = native.outer_size();
        let client = native.inner_size();
        PixelSize {
            width: content_size
                .width
                .saturating_add(outer.width.saturating_sub(client.width)),
            height: content_size
                .height
                .saturating_add(outer.height.saturating_sub(client.height)),
        }
    })
}

#[cfg(target_os = "windows")]
pub(crate) fn resized_frame_size(window: &slint::Window, estimate: PixelSize) -> PixelSize {
    initial_frame_size(window, estimate).unwrap_or(estimate)
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn resized_frame_size(_: &slint::Window, estimate: PixelSize) -> PixelSize {
    estimate
}

pub(crate) struct Guard {
    #[cfg(target_os = "windows")]
    _handler: std::rc::Rc<std::cell::RefCell<Option<super::WindowSizing>>>,
    #[cfg(target_os = "macos")]
    _notifications: super::macos::live_resize::Notifications,
    #[cfg(target_os = "linux")]
    compact_resize: slint::Timer,
}

impl Guard {
    pub(crate) fn observe_resize(
        &self,
        _app: &PlayerWindow,
        _window: &slint::Window,
        _size: PixelSize,
    ) {
        #[cfg(target_os = "macos")]
        super::macos::live_resize::preview(_app, _window, _size.height);
        #[cfg(target_os = "linux")]
        super::unix::compact_resize::observe(_app, _window, _size, &self.compact_resize);
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub(crate) fn install(_: &PlayerWindow, _: Option<slint::Weak<NativeVideoHost>>) -> Guard {
    Guard {
        #[cfg(target_os = "linux")]
        compact_resize: slint::Timer::default(),
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn install(app: &PlayerWindow, _: Option<slint::Weak<NativeVideoHost>>) -> Guard {
    Guard {
        _notifications: super::macos::live_resize::Notifications::new(app),
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn install(app: &PlayerWindow, host: Option<slint::Weak<NativeVideoHost>>) -> Guard {
    use slint::ComponentHandle;
    use std::{cell::RefCell, rc::Rc};

    let handler = Rc::new(RefCell::new(None));
    let installed = handler.clone();
    let weak = app.as_weak();
    let scheduled = slint::spawn_local(async move {
        let Some(app) = weak.upgrade() else { return };
        let window = if let Some(host) = host {
            let Some(host) = host.upgrade() else { return };
            host.window().winit_window().await
        } else {
            app.window().winit_window().await
        };
        match window
            .map_err(|error| error.to_string())
            .and_then(|window| super::WindowSizing::install(&window, app.as_weak()))
        {
            Ok(handler) => *installed.borrow_mut() = Some(handler),
            Err(error) => crate::diagnostics::report(&app, error),
        }
    });
    if let Err(error) = scheduled {
        crate::diagnostics::warn!("Could not schedule window sizing setup: {error}");
    }
    Guard { _handler: handler }
}

#[cfg(all(test, target_os = "windows"))]
#[test]
fn automatic_playlist_changes_keep_the_current_native_video_surface() {
    assert!(media_resize_allowed(true, false));
    assert!(media_resize_allowed(false, true));
    assert!(!media_resize_allowed(false, false));
}
