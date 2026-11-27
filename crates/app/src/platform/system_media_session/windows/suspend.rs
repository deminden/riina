//! Route the power notification from the framed owner to its playback session.
use super::{PlaybackStore, PlayerWindow, pause_for_suspend};
use slint::Weak;
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
};
use windows::Win32::{
    Foundation::{HANDLE, HWND, LPARAM, LRESULT, WPARAM},
    System::Power::{
        HPOWERNOTIFY, RegisterSuspendResumeNotification, UnregisterSuspendResumeNotification,
    },
    UI::{
        Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
        WindowsAndMessaging::{DEVICE_NOTIFY_WINDOW_HANDLE, PBT_APMSUSPEND, WM_POWERBROADCAST},
    },
};

type Route = (Weak<PlayerWindow>, PlaybackStore);

thread_local! {
    static ROUTES: RefCell<BTreeMap<usize, Route>> = const { RefCell::new(BTreeMap::new()) };
    static NEXT_ROUTE: Cell<usize> = const { Cell::new(0) };
}

pub(super) struct Registration {
    hwnd: HWND,
    route: usize,
    notification: HPOWERNOTIFY,
}

impl Registration {
    pub(super) fn open(
        hwnd: HWND,
        app: Weak<PlayerWindow>,
        playback: PlaybackStore,
    ) -> Result<Self, String> {
        let route = NEXT_ROUTE.with(|next| {
            let route = next.get() + 1;
            next.set(route);
            route
        });
        // SAFETY: the session supplies a live HWND owned by this thread.
        let notification = unsafe {
            RegisterSuspendResumeNotification(HANDLE(hwnd.0), DEVICE_NOTIFY_WINDOW_HANDLE)
        }
        .map_err(|error| error.to_string())?;
        let registration = Self {
            hwnd,
            route,
            notification,
        };
        // SAFETY: the owning UI thread installs and removes the callback. Its
        // refdata is an integer lookup key, with no borrowed native pointer.
        if !unsafe { SetWindowSubclass(hwnd, Some(on_power_message), route, 0) }.as_bool() {
            return Err("Could not attach Windows power notifications to the player".into());
        }
        ROUTES.with(|routes| routes.borrow_mut().insert(route, (app, playback)));
        Ok(registration)
    }
}

impl Drop for Registration {
    fn drop(&mut self) {
        ROUTES.with(|routes| routes.borrow_mut().remove(&self.route));
        // SAFETY: the session owns these exact registrations on this UI thread.
        // If Windows already destroyed the HWND, removing its subclass is a no-op.
        unsafe {
            let _ = RemoveWindowSubclass(self.hwnd, Some(on_power_message), self.route);
            let _ = UnregisterSuspendResumeNotification(self.notification);
        }
    }
}

unsafe extern "system" fn on_power_message(
    hwnd: HWND,
    message: u32,
    event: WPARAM,
    data: LPARAM,
    id: usize,
    _reference: usize,
) -> LRESULT {
    // SAFETY: pass the unchanged native message down the window's subclass chain.
    let result = unsafe { DefSubclassProc(hwnd, message, event, data) };
    if message != WM_POWERBROADCAST || event.0 != PBT_APMSUSPEND as usize {
        return result;
    }
    let route = ROUTES.with(|routes| routes.borrow().get(&id).cloned());
    if let Some((app, playback)) = route
        && let Some(app) = app.upgrade()
    {
        pause_for_suspend(&app, &playback);
    }
    LRESULT(1)
}
