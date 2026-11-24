//! Run presentation work only while the framed owner can display it.
#[cfg(test)]
mod tests;

use slint::{Timer, winit_030::winit};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};
use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    UI::{
        Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
        WindowsAndMessaging::{IsIconic, IsWindow, IsWindowVisible, WM_WINDOWPOSCHANGED},
    },
};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

struct SurfaceTimers([Weak<Timer>; 3]);

impl SurfaceTimers {
    fn set_drawable(&self, drawable: bool) {
        for timer in self.0.iter().filter_map(Weak::upgrade) {
            match (drawable, timer.running()) {
                (true, false) => timer.restart(),
                (false, true) => timer.stop(),
                _ => {}
            }
        }
    }
}

pub(super) struct VisibilityMonitor {
    hwnd: HWND,
    signals: Option<Box<WindowSignals>>,
}

type MinimizeHandler = Box<dyn Fn(bool)>;

struct WindowSignals {
    timers: SurfaceTimers,
    minimized: Cell<bool>,
    on_minimized: RefCell<Option<MinimizeHandler>>,
}

impl VisibilityMonitor {
    pub(super) fn install(owner: isize, timers: [&Rc<Timer>; 3]) -> Result<Self, &'static str> {
        let hwnd = HWND(owner as *mut _);
        // SAFETY: the caller supplies the live framed HWND on its owner thread.
        let minimized = unsafe { IsIconic(hwnd) }.as_bool();
        let signals = Box::new(WindowSignals {
            timers: SurfaceTimers(timers.map(Rc::downgrade)),
            minimized: Cell::new(minimized),
            on_minimized: RefCell::new(None),
        });
        let pointer = (&*signals as *const WindowSignals) as usize;
        // SAFETY: the UI thread owns these signals through subclass removal.
        if !unsafe { SetWindowSubclass(hwnd, Some(visibility_proc), pointer, pointer) }.as_bool() {
            return Err("Could not observe native player visibility");
        }
        signals.timers.set_drawable(is_drawable(hwnd));
        Ok(Self {
            hwnd,
            signals: Some(signals),
        })
    }

    pub(super) fn on_minimized(&self, handler: impl Fn(bool) + 'static) {
        if let Some(signals) = &self.signals {
            signals.on_minimized.replace(Some(Box::new(handler)));
        }
    }
}

impl Drop for VisibilityMonitor {
    fn drop(&mut self) {
        let Some(signals) = self.signals.take() else {
            return;
        };
        signals.timers.set_drawable(false);
        signals.on_minimized.replace(None);
        let pointer = (&*signals as *const WindowSignals) as usize;
        // SAFETY: refdata remains live while the subclass is detached on its owner thread.
        let removed = unsafe { RemoveWindowSubclass(self.hwnd, Some(visibility_proc), pointer) };
        // SAFETY: the handle value can be queried after the native window closes.
        if !removed.as_bool() && unsafe { IsWindow(Some(self.hwnd)) }.as_bool() {
            // An attached subclass must retain valid refdata. Empty its timer references.
            Box::leak(signals).timers.0 = Default::default();
        }
    }
}

pub(super) fn window_is_drawable(window: &winit::window::Window) -> bool {
    let Ok(handle) = window.window_handle() else {
        return false;
    };
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return false;
    };
    is_drawable(HWND(handle.hwnd.get() as *mut _))
}

fn is_drawable(hwnd: HWND) -> bool {
    // SAFETY: these queries take a handle value and return false for an invalid window.
    unsafe { IsWindowVisible(hwnd).as_bool() && !IsIconic(hwnd).as_bool() }
}

unsafe extern "system" fn visibility_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    refdata: usize,
) -> LRESULT {
    if message == WM_WINDOWPOSCHANGED {
        // SAFETY: VisibilityMonitor retains this box until subclass removal.
        let signals = unsafe { &*(refdata as *const WindowSignals) };
        signals.timers.set_drawable(is_drawable(hwnd));
        // SAFETY: the subclass receives the HWND whose placement just changed.
        let minimized = unsafe { IsIconic(hwnd) }.as_bool();
        if signals.minimized.replace(minimized) != minimized
            && let Some(handler) = signals.on_minimized.borrow().as_ref()
        {
            handler(minimized);
        }
    }
    // SAFETY: preserve the subclass chain and Winit's move and size notifications.
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}
