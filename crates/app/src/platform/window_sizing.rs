//! Platform placement and resize operations shared by window-aspect policy.
use crate::{NativeVideoHost, PlayerWindow};
use riina_core::window_aspect::PixelSize;
use slint::winit_030::WinitWindowAccessor;

pub(crate) const USES_NATIVE_RESIZE_FILTER: bool = cfg!(target_os = "windows");

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
    center: bool,
    overlay: Option<&slint::Window>,
) -> Option<Result<(), String>> {
    let resize_content = super::windows::window_geometry::resize_content;
    Some(
        window
            .with_winit_window(|native| {
                if let Some(overlay) = overlay {
                    overlay.with_winit_window(|overlay| {
                        resize_content(native, target, center, Some(overlay))
                    })
                } else {
                    Some(resize_content(native, target, center, None))
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
    _: bool,
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
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn install(_: &PlayerWindow, _: Option<slint::Weak<NativeVideoHost>>) -> Guard {
    Guard {}
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
