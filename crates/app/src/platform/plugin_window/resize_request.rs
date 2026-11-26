//! Distinguish an explicit outer frame request from an interactive resize.
use riina_core::window_aspect::PixelSize;
use slint::winit_030::{
    WinitWindowAccessor,
    winit::{
        dpi::PhysicalSize,
        window::{Window, WindowId},
    },
};
use std::{
    cell::RefCell,
    collections::HashMap,
    time::{Duration, Instant},
};

const REPLY_TIMEOUT: Duration = Duration::from_secs(1);

thread_local! {
    static REQUESTED: RefCell<HashMap<WindowId, (PhysicalSize<u32>, Instant)>> = RefCell::default();
}

pub(super) fn expect_outer(window: &Window, outer: PhysicalSize<u32>) -> PhysicalSize<u32> {
    let current_outer = window.outer_size();
    let current_inner = window.inner_size();
    let size = PhysicalSize::new(
        outer
            .width
            .saturating_sub(current_outer.width.saturating_sub(current_inner.width))
            .max(1),
        outer
            .height
            .saturating_sub(current_outer.height.saturating_sub(current_inner.height))
            .max(1),
    );
    REQUESTED.with_borrow_mut(|requests| {
        let now = Instant::now();
        requests.retain(|_, (_, expires)| *expires > now);
        if size == current_inner {
            requests.remove(&window.id());
        } else {
            requests.insert(window.id(), (size, now + REPLY_TIMEOUT));
        }
    });
    size
}

pub(super) fn take(window: &slint::Window, size: PixelSize) -> bool {
    window
        .with_winit_window(|window| {
            REQUESTED.with_borrow_mut(|requests| {
                let now = Instant::now();
                requests.retain(|_, (_, expires)| *expires > now);
                let id = window.id();
                if requests.get(&id).is_some_and(|(expected, _)| {
                    expected.width == size.width && expected.height == size.height
                }) {
                    requests.remove(&id);
                    return true;
                }
                false
            })
        })
        .unwrap_or_default()
}
