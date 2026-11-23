//! A transparent AppKit container gives unclaimed hits to the Slint player.
use super::hit_test::{HitTest, Point};
use crate::platform::macos::view::{content_view, top_left_point};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained};
use objc2_app_kit::NSView;
use objc2_foundation::{NSPoint, NSRect, NSSize};
use slint::winit_030::winit::{
    raw_window_handle::{
        AppKitWindowHandle, HandleError, HasWindowHandle, RawWindowHandle, WindowHandle,
    },
    window::Window,
};
use std::{ptr::NonNull, sync::Arc};
use wry::{Rect, WebView, WebViewBuilder, dpi::LogicalPosition};

define_class!(
    #[unsafe(super = NSView)]
    #[thread_kind = MainThreadOnly]
    #[ivars = Arc<HitTest>]
    struct OverlayView;

    impl OverlayView {
        #[unsafe(method_id(hitTest:))]
        fn hit_test(&self, position: NSPoint) -> Option<Retained<NSView>> {
            // SAFETY: AppKit invokes hitTest on the UI thread while the views are live.
            let hit = unsafe { self.superview() }.is_some_and(|parent| {
                let window_point = parent.convertPoint_toView(position, None);
                self.ivars().contains(top_left_point(self, window_point))
            });
            if hit {
                // SAFETY: preserve NSView's native child traversal on the main thread.
                unsafe { msg_send![super(self), hitTest: position] }
            } else {
                None
            }
        }
    }
);

struct Container(Retained<OverlayView>);

impl HasWindowHandle for Container {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let pointer = NonNull::from(&*self.0).cast();
        let handle = RawWindowHandle::AppKit(AppKitWindowHandle::new(pointer));
        // SAFETY: this UI-thread container owns the live NSView for the borrow.
        Ok(unsafe { WindowHandle::borrow_raw(handle) })
    }
}

impl Drop for Container {
    fn drop(&mut self) {
        self.0.removeFromSuperview();
    }
}

pub(super) struct Surface {
    browser: WebView,
    container: Container,
}

impl Surface {
    pub(super) fn build(
        builder: WebViewBuilder<'_>,
        parent: &Window,
        bounds: Rect,
        hits: Arc<HitTest>,
    ) -> Result<Self, String> {
        let main =
            MainThreadMarker::new().ok_or("Plugin overlays must run on the AppKit thread")?;
        let parent = content_view(parent).ok_or("The overlay's AppKit parent is unavailable")?;
        let view = OverlayView::alloc(main).set_ivars(hits);
        // SAFETY: initialize a main-thread NSView subclass with its typed hit-test state.
        let view: Retained<OverlayView> =
            unsafe { msg_send![super(view), initWithFrame: frame(parent, bounds)] };
        parent.addSubview(&view);
        let container = Container(view);
        let browser = builder
            .with_bounds(local(bounds))
            .build_as_child(&container)
            .map_err(|error| error.to_string())?;
        Ok(Self { browser, container })
    }

    pub(super) fn browser(&self) -> &WebView {
        &self.browser
    }

    pub(super) fn set_bounds(&self, bounds: Rect) -> Result<(), String> {
        // SAFETY: the main thread owns the attached view and serializes its teardown.
        let parent = unsafe { self.container.0.superview() }
            .ok_or("The overlay's AppKit parent has closed")?;
        self.container.0.setFrame(frame(&parent, bounds));
        self.browser
            .set_bounds(local(bounds))
            .map_err(|error| error.to_string())
    }

    pub(super) fn pointer(&self) -> Option<Point> {
        let view = &self.container.0;
        let location = view.window()?.mouseLocationOutsideOfEventStream();
        let (x, y) = top_left_point(view, location);
        let size = view.bounds().size;
        (x >= 0.0 && x < size.width && y >= 0.0 && y < size.height).then_some((x, y))
    }
}

fn frame(parent: &NSView, bounds: Rect) -> NSRect {
    let position = bounds.position.to_logical::<f64>(1.0);
    let size = bounds.size.to_logical::<f64>(1.0);
    let parent_bounds = parent.bounds();
    let y = if parent.isFlipped() {
        position.y
    } else {
        parent_bounds.size.height - position.y - size.height
    };
    NSRect::new(
        NSPoint::new(
            parent_bounds.origin.x + position.x,
            parent_bounds.origin.y + y,
        ),
        NSSize::new(size.width, size.height),
    )
}

fn local(bounds: Rect) -> Rect {
    Rect {
        position: LogicalPosition::new(0.0, 0.0).into(),
        ..bounds
    }
}
