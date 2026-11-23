//! Run presentation work only while the framed owner can display it.
#[cfg(test)]
mod tests;

use slint::{Timer, winit_030::winit};
use std::rc::{Rc, Weak};
use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    UI::{
        Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
        WindowsAndMessaging::{IsIconic, IsWindow, IsWindowVisible, WM_WINDOWPOSCHANGED},
    },
};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

struct SurfaceTimers([Weak<Timer>; 2]);

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
    timers: Option<Box<SurfaceTimers>>,
}

impl VisibilityMonitor {
    pub(super) fn install(
        owner: isize,
        alignment: &Rc<Timer>,
        material_policy: &Rc<Timer>,
    ) -> Result<Self, &'static str> {
        let hwnd = HWND(owner as *mut _);
        let timers = Box::new(SurfaceTimers([
            Rc::downgrade(alignment),
            Rc::downgrade(material_policy),
        ]));
        let pointer = (&*timers as *const SurfaceTimers) as usize;
        // SAFETY: the UI thread owns the stable timer references through subclass removal.
        if !unsafe { SetWindowSubclass(hwnd, Some(visibility_proc), pointer, pointer) }.as_bool() {
            return Err("Could not observe native player visibility");
        }
        timers.set_drawable(is_drawable(hwnd));
        Ok(Self {
            hwnd,
            timers: Some(timers),
        })
    }
}

impl Drop for VisibilityMonitor {
    fn drop(&mut self) {
        let Some(timers) = self.timers.take() else {
            return;
        };
        timers.set_drawable(false);
        let pointer = (&*timers as *const SurfaceTimers) as usize;
        // SAFETY: refdata remains live while the subclass is detached on its owner thread.
        let removed = unsafe { RemoveWindowSubclass(self.hwnd, Some(visibility_proc), pointer) };
        // SAFETY: the handle value can be queried after the native window closes.
        if !removed.as_bool() && unsafe { IsWindow(Some(self.hwnd)) }.as_bool() {
            // An attached subclass must retain valid refdata. Empty its timer references.
            Box::leak(timers).0 = [Weak::new(), Weak::new()];
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
        let timers = unsafe { &*(refdata as *const SurfaceTimers) };
        timers.set_drawable(is_drawable(hwnd));
    }
    // SAFETY: preserve the subclass chain and Winit's move and size notifications.
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}
