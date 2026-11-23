//! Route AppKit drags through the live Slint window that receives them.
mod destination;
mod payload;

use super::view::{content_view, top_left_point};
use crate::platform::desktop_drop::{Hover, OpenDrop};
use objc2_app_kit::NSView;
use objc2_foundation::NSPoint;
use slint::{ComponentHandle, LogicalPosition, Window, winit_030::WinitWindowAccessor};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    ffi::OsString,
    rc::{Rc, Weak},
};

struct Route {
    scale: Box<dyn Fn() -> Option<f32>>,
    hover: Box<dyn Fn(Hover)>,
    capture: Box<dyn Fn(LogicalPosition) -> Option<OpenDrop>>,
    accepts_drag: Cell<bool>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Key {
    view: usize,
    token: usize,
}

thread_local! {
    static ROUTES: RefCell<BTreeMap<usize, (Key, Weak<Route>)>> = const {
        RefCell::new(BTreeMap::new())
    };
    static NEXT_TOKEN: Cell<usize> = const { Cell::new(0) };
}

pub(crate) struct Registration {
    route: Rc<Route>,
    attachment: RefCell<Option<destination::Attachment>>,
}

pub(crate) fn register<T: ComponentHandle + 'static>(
    component: &T,
    hover: impl Fn(Hover) + 'static,
    capture: impl Fn(LogicalPosition) -> Option<OpenDrop> + 'static,
) -> Registration {
    let window = component.as_weak();
    let registration = Registration {
        route: Rc::new(Route {
            scale: Box::new(move || {
                let app = window.upgrade()?;
                app.window()
                    .is_visible()
                    .then(|| app.window().scale_factor())
            }),
            hover: Box::new(hover),
            capture: Box::new(capture),
            accepts_drag: Cell::new(false),
        }),
        attachment: RefCell::new(None),
    };
    registration.attach(component.window());
    registration
}

impl Registration {
    pub(crate) fn attach(&self, window: &Window) {
        window.with_winit_window(|window| {
            let Some(view) = content_view(window) else {
                return;
            };
            if self
                .attachment
                .borrow()
                .as_ref()
                .is_some_and(|target| target.owns(view))
            {
                return;
            }
            drop(self.attachment.take());
            self.route.accepts_drag.set(false);
            self.attachment
                .replace(destination::Attachment::new(view, &self.route));
        });
    }
}

impl Key {
    fn new(view: &NSView) -> Self {
        Self {
            view: std::ptr::from_ref(view).addr(),
            token: NEXT_TOKEN.with(|next| next.replace(next.get().wrapping_add(1))),
        }
    }
}

fn lookup(view: &NSView) -> Option<(Key, Rc<Route>)> {
    ROUTES.with_borrow(|routes| {
        let (key, route) = routes.get(&std::ptr::from_ref(view).addr())?;
        Some((*key, route.upgrade()?))
    })
}

#[derive(Clone, Copy)]
struct Position {
    x: f64,
    y: f64,
    backing_scale: f64,
}

impl Position {
    fn new(view: &NSView, point: NSPoint) -> Option<Self> {
        let (x, y) = top_left_point(view, point);
        Some(Self {
            x,
            y,
            backing_scale: view.window()?.backingScaleFactor(),
        })
    }

    fn logical(self, slint_scale: f32) -> LogicalPosition {
        let factor = self.backing_scale / f64::from(slint_scale);
        LogicalPosition::new((self.x * factor) as f32, (self.y * factor) as f32)
    }
}

enum Event {
    Entered(Vec<OsString>, Position),
    Moved(Position),
    Left,
    Dropped(Vec<OsString>, Position),
}

fn queue(key: Key, event: Event) {
    // AppKit can run its drag loop while Slint is handling a window event.
    let _ = slint::invoke_from_event_loop(move || {
        let route = ROUTES.with_borrow(|routes| {
            let (attached, route) = routes.get(&key.view)?;
            (*attached == key).then_some(())?;
            route.upgrade()
        });
        if let Some(route) = route {
            route.deliver(event);
        }
    });
}

impl Route {
    fn deliver(&self, event: Event) {
        let Some(scale) = (self.scale)() else {
            (self.hover)(Hover::Left);
            return;
        };
        match event {
            Event::Entered(paths, position) => (self.hover)(Hover::Entered(
                paths.into_iter().map(Into::into).collect(),
                position.logical(scale),
            )),
            Event::Moved(position) => (self.hover)(Hover::Moved(position.logical(scale))),
            Event::Left => (self.hover)(Hover::Left),
            Event::Dropped(paths, position) => {
                let open = (self.capture)(position.logical(scale));
                (self.hover)(Hover::Left);
                if let Some(open) = open {
                    open(paths);
                }
            }
        }
    }
}
