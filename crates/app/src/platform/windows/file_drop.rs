//! Bind one OLE target to each live player or Welcome HWND.
mod payload;
mod target;

use crate::platform::desktop_drop::{Hover, OpenDrop};
use slint::{
    ComponentHandle, LogicalPosition, Timer, Window,
    winit_030::{
        WinitWindowAccessor,
        winit::raw_window_handle::{HasWindowHandle, RawWindowHandle},
    },
};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    ffi::OsString,
    path::PathBuf,
    rc::{Rc, Weak},
    time::Duration,
};
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM},
        System::Ole::{OleInitialize, OleUninitialize, RegisterDragDrop, RevokeDragDrop},
        UI::{
            Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
            WindowsAndMessaging::WM_NCDESTROY,
        },
    },
    core::Error,
};

struct Route {
    scale: Box<dyn Fn() -> Option<f32>>,
    hover: Box<dyn Fn(Hover)>,
    capture: Box<dyn Fn(LogicalPosition) -> Option<OpenDrop>>,
    active: Cell<Option<Key>>,
    generation: Cell<usize>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Key {
    hwnd: isize,
    token: usize,
}

impl Key {
    fn window(self) -> HWND {
        HWND(self.hwnd as *mut _)
    }
}

pub(crate) struct Registration {
    route: Rc<Route>,
    attached: Cell<Option<Key>>,
    owner: Cell<Option<Key>>,
}

struct Attachment {
    token: usize,
    route: Weak<Route>,
    _apartment: Apartment,
}

thread_local! {
    static ATTACHED: RefCell<BTreeMap<isize, Attachment>> = const { RefCell::new(BTreeMap::new()) };
    static NEXT_TOKEN: Cell<usize> = const { Cell::new(1) };
}

pub(crate) fn register<T: ComponentHandle + 'static>(
    component: &T,
    hover: impl Fn(Hover) + 'static,
    capture: impl Fn(LogicalPosition) -> Option<OpenDrop> + 'static,
) -> Registration {
    let window = component.as_weak();
    Registration {
        route: Rc::new(Route {
            scale: Box::new(move || {
                let app = window.upgrade()?;
                app.window()
                    .is_visible()
                    .then(|| app.window().scale_factor())
            }),
            hover: Box::new(hover),
            capture: Box::new(capture),
            active: Cell::new(None),
            generation: Cell::new(0),
        }),
        attached: Cell::new(None),
        owner: Cell::new(None),
    }
}

impl Registration {
    pub(crate) fn attach(&self, window: &Window) {
        self.attach_to(window, &self.attached);
    }

    pub(crate) fn attach_owner(&self, window: &Window) {
        self.attach_to(window, &self.owner);
    }

    fn attach_to(&self, window: &Window, slot: &Cell<Option<Key>>) {
        let Some(hwnd) = window
            .with_winit_window(|window| match window.window_handle().ok()?.as_raw() {
                RawWindowHandle::Win32(handle) => Some(handle.hwnd.get()),
                _ => None,
            })
            .flatten()
        else {
            return;
        };
        if slot.get().is_some_and(|key| key.hwnd == hwnd) {
            return;
        }
        if let Some(previous) = slot.take() {
            detach(previous);
        }
        let token = NEXT_TOKEN.with(|next| {
            let token = next.get();
            next.set(token.wrapping_add(1));
            token
        });
        let key = Key { hwnd, token };
        slot.set(Some(key));
        if let Err(error) = attach(key, &self.route) {
            crate::diagnostics::warn!("Could not register the Windows drop target: {error}");
        }
    }
}

impl Drop for Registration {
    fn drop(&mut self) {
        for key in [self.attached.take(), self.owner.take()]
            .into_iter()
            .flatten()
        {
            detach(key);
        }
    }
}

fn attach(key: Key, route: &Rc<Route>) -> windows::core::Result<()> {
    let apartment = Apartment::new()?;
    let target = target::DropTarget::interface(key);
    // SAFETY: this live HWND belongs to the current UI thread's OLE apartment.
    unsafe { RegisterDragDrop(key.window(), &target) }?;
    // SAFETY: the callback uses only the uniquely numbered entry for this HWND.
    if !unsafe { SetWindowSubclass(key.window(), Some(destroy_subclass), key.token, 0) }.as_bool() {
        let error = Error::from_thread();
        // SAFETY: this call pairs with the successful registration above.
        let _ = unsafe { RevokeDragDrop(key.window()) };
        return Err(error);
    }
    ATTACHED.with_borrow_mut(|attached| {
        attached.insert(
            key.hwnd,
            Attachment {
                token: key.token,
                route: Rc::downgrade(route),
                _apartment: apartment,
            },
        );
    });
    Ok(())
}

fn detach(key: Key) {
    let removed = ATTACHED.with_borrow_mut(|attached| {
        if attached
            .get(&key.hwnd)
            .is_some_and(|entry| entry.token == key.token)
        {
            attached.remove(&key.hwnd)
        } else {
            None
        }
    });
    if let Some(_attachment) = removed {
        // SAFETY: the matching record owns these registrations for the live HWND.
        // Keep the OLE apartment initialized through revocation.
        unsafe {
            let _ = RemoveWindowSubclass(key.window(), Some(destroy_subclass), key.token);
            let _ = RevokeDragDrop(key.window());
        }
    }
}

unsafe extern "system" fn destroy_subclass(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    token: usize,
    _: usize,
) -> LRESULT {
    if message == WM_NCDESTROY {
        detach(Key {
            hwnd: hwnd.0 as isize,
            token,
        });
    }
    // SAFETY: forward the message on the current window's subclass chain.
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

struct Apartment;

impl Apartment {
    fn new() -> windows::core::Result<Self> {
        // SAFETY: this is the GUI thread. Balance every successful initialization.
        unsafe { OleInitialize(None) }?;
        Ok(Self)
    }
}

impl Drop for Apartment {
    fn drop(&mut self) {
        // SAFETY: the apartment guard is created and released on the GUI thread.
        unsafe { OleUninitialize() };
    }
}

enum Preview {
    Files(Vec<PathBuf>),
    Link,
}

enum Event {
    Entered(Option<Preview>, POINT),
    Motion(POINT),
    Left,
    Drop(Vec<OsString>, POINT),
}

fn queue(key: Key, event: Event) {
    // OLE can dispatch while the native message loop is nested. Resolve the weak
    // route in Slint's loop, outside the COM call and any window subclass borrow.
    let _ = slint::invoke_from_event_loop(move || {
        let route = ATTACHED.with_borrow(|attached| {
            let entry = attached.get(&key.hwnd)?;
            (entry.token == key.token).then_some(())?;
            entry.route.upgrade()
        });
        if let Some(route) = route {
            route.deliver(key, event);
        }
    });
}

impl Route {
    fn deliver(self: &Rc<Self>, key: Key, event: Event) {
        let Some(scale) = (self.scale)() else {
            self.finish();
            return;
        };
        let logical =
            |point: POINT| LogicalPosition::new(point.x as f32 / scale, point.y as f32 / scale);
        match event {
            Event::Entered(preview, point) => {
                let handoff = self
                    .active
                    .replace(Some(key))
                    .is_some_and(|previous| previous != key);
                self.generation.set(self.generation.get().wrapping_add(1));
                let point = logical(point);
                if handoff {
                    (self.hover)(Hover::Moved(point));
                } else {
                    match preview {
                        Some(Preview::Files(paths)) => (self.hover)(Hover::Entered(paths, point)),
                        Some(Preview::Link) => (self.hover)(Hover::Link(point)),
                        None => (self.hover)(Hover::Left),
                    }
                }
            }
            Event::Motion(point) if self.active.get() == Some(key) => {
                (self.hover)(Hover::Moved(logical(point)))
            }
            Event::Motion(_) => {}
            Event::Left => self.leave(key),
            Event::Drop(paths, point) => {
                let open = (self.capture)(logical(point));
                self.finish();
                if let Some(open) = open {
                    open(paths);
                }
            }
        }
    }

    fn leave(self: &Rc<Self>, key: Key) {
        let weak = Rc::downgrade(self);
        let generation = self.generation.get();
        // OLE sends a leave when the pointer crosses from the video HWND to
        // its aligned controls HWND. Give the same route a chance to retake it.
        Timer::single_shot(Duration::from_millis(50), move || {
            if let Some(route) = weak.upgrade()
                && route.active.get() == Some(key)
                && route.generation.get() == generation
            {
                route.finish();
            }
        });
    }

    fn finish(&self) {
        self.active.set(None);
        (self.hover)(Hover::Left);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::{
        Win32::{
            Foundation::DRAGDROP_E_ALREADYREGISTERED,
            UI::WindowsAndMessaging::{
                CreateWindowExW, DestroyWindow, WINDOW_EX_STYLE, WINDOW_STYLE,
            },
        },
        core::w,
    };

    #[test]
    fn native_window_destruction_revokes_an_ole_drop_target() -> windows::core::Result<()> {
        let _apartment = Apartment::new()?;
        // SAFETY: STATIC is a system class. This test creates an unshown window on its own thread.
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                w!("Riina drop registration test"),
                WINDOW_STYLE(0),
                0,
                0,
                0,
                0,
                None,
                None,
                None,
                None,
            )
        }?;
        let key = Key {
            hwnd: hwnd.0 as isize,
            token: 1,
        };
        let route = Rc::new(Route {
            scale: Box::new(|| Some(1.0)),
            hover: Box::new(|_| {}),
            capture: Box::new(|_| None),
            active: Cell::new(None),
            generation: Cell::new(0),
        });
        let checked = attach(key, &route).and_then(|()| {
            let duplicate = target::DropTarget::interface(key);
            // SAFETY: this thread owns the live HWND and its OLE initialization.
            let result = unsafe { RegisterDragDrop(hwnd, &duplicate) };
            match result {
                Err(error) if error.code() == DRAGDROP_E_ALREADYREGISTERED => Ok(()),
                Err(error) => Err(error),
                Ok(()) => Err(windows::Win32::Foundation::E_FAIL.into()),
            }
        });
        // SAFETY: tear down exactly the hidden window created by this test.
        unsafe { DestroyWindow(hwnd) }?;
        checked?;
        assert!(!ATTACHED.with_borrow(|attached| attached.contains_key(&key.hwnd)));
        Ok(())
    }

    #[test]
    fn crossing_from_video_to_controls_keeps_the_same_playlist_drag() {
        i_slint_backend_testing::init_no_event_loop();
        let events = Rc::new(RefCell::new(Vec::new()));
        let hover = events.clone();
        let capture = events.clone();
        let route = Rc::new(Route {
            scale: Box::new(|| Some(2.0)),
            hover: Box::new(move |event| {
                hover.borrow_mut().push(match event {
                    Hover::Entered(_, _) => "files",
                    Hover::Link(_) => "link",
                    Hover::Moved(_) => "move",
                    Hover::Left => "left",
                });
            }),
            capture: Box::new(move |position| {
                assert_eq!(position, LogicalPosition::new(501.0, 50.0));
                capture.borrow_mut().push("capture");
                let capture = capture.clone();
                Some(Box::new(move |paths| {
                    assert_eq!(paths, ["https://example.test/video"]);
                    capture.borrow_mut().push("open");
                }))
            }),
            active: Cell::new(None),
            generation: Cell::new(0),
        });
        let video = Key { hwnd: 1, token: 1 };
        let controls = Key { hwnd: 2, token: 2 };
        let point = POINT { x: 1002, y: 100 };
        route.deliver(video, Event::Entered(Some(Preview::Link), point));
        route.deliver(video, Event::Left);
        route.deliver(controls, Event::Entered(Some(Preview::Link), point));
        i_slint_backend_testing::testing_backend::mock_elapsed_time(60);
        slint::platform::update_timers_and_animations();
        route.deliver(
            controls,
            Event::Drop(vec!["https://example.test/video".into()], point),
        );
        assert_eq!(
            *events.borrow(),
            ["link", "move", "capture", "left", "open"]
        );
    }
}
