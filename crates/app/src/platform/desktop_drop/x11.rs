//! Present Xdnd on a proxy while leaving Winit's window and input under its control.
mod protocol;
mod selection;

use super::{Hover, OpenDrop};
use riina_core::sync::lock;
use slint::{
    ComponentHandle, LogicalPosition, Window,
    winit_030::{
        WinitWindowAccessor,
        winit::raw_window_handle::{HasWindowHandle, RawWindowHandle},
    },
};
use std::{
    cell::RefCell,
    collections::HashMap,
    ffi::OsString,
    path::PathBuf,
    rc::{Rc, Weak},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
type Point = (i16, i16);

static NEXT: AtomicU64 = AtomicU64::new(1);
thread_local! {
    static TARGETS: RefCell<HashMap<u64, Weak<Target>>> = RefCell::default();
}

pub(super) struct Registration {
    target: Rc<Target>,
    attached: RefCell<Option<Attachment>>,
}

struct Attachment {
    window: u32,
    id: u64,
    _proxy: Option<protocol::Proxy>,
}

impl Drop for Attachment {
    fn drop(&mut self) {
        TARGETS.with_borrow_mut(|targets| targets.remove(&self.id));
    }
}

impl Registration {
    pub(super) fn new<T: ComponentHandle + 'static>(
        window: &T,
        hover: impl Fn(Hover) + 'static,
        capture: impl Fn(LogicalPosition) -> Option<OpenDrop> + 'static,
    ) -> Self {
        let window = window.as_weak();
        Self {
            target: Rc::new(Target {
                scale: Box::new(move || {
                    let component = window.upgrade()?;
                    let window = component.window();
                    window.is_visible().then(|| window.scale_factor())
                }),
                hover: Box::new(hover),
                capture: Box::new(capture),
                released: RefCell::new(None),
            }),
            attached: RefCell::new(None),
        }
    }

    pub(super) fn attach(&self, window: &Window) {
        let Some(xid) = window
            .with_winit_window(|native| match native.window_handle().ok()?.as_raw() {
                RawWindowHandle::Xlib(handle) => u32::try_from(handle.window).ok(),
                RawWindowHandle::Xcb(handle) => Some(handle.window.get()),
                _ => None,
            })
            .flatten()
        else {
            return;
        };
        let mut attached = self.attached.borrow_mut();
        if attached.as_ref().is_some_and(|old| old.window == xid) {
            return;
        }
        if attached.take().is_some() {
            self.target.update(Notice::Left);
        }
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let proxy = match protocol::Proxy::start(xid, id) {
            Ok(proxy) => Some(proxy),
            Err(error) => {
                crate::diagnostics::warn!("Could not register the X11 drop proxy: {error}");
                None
            }
        };
        if proxy.is_some() {
            TARGETS.with_borrow_mut(|targets| targets.insert(id, Rc::downgrade(&self.target)));
        }
        *attached = Some(Attachment {
            window: xid,
            id,
            _proxy: proxy,
        });
    }
}

struct Target {
    scale: Box<dyn Fn() -> Option<f32>>,
    hover: Box<dyn Fn(Hover)>,
    capture: Box<dyn Fn(LogicalPosition) -> Option<OpenDrop>>,
    released: RefCell<Option<(u64, OpenDrop)>>,
}

impl Target {
    fn point(&self, (x, y): Point) -> Option<LogicalPosition> {
        let scale = (self.scale)()?;
        Some(LogicalPosition::new(
            f32::from(x) / scale,
            f32::from(y) / scale,
        ))
    }

    fn update(&self, notice: Notice) {
        match notice {
            Notice::Hover(position, paths) => {
                if let Some(position) = self.point(position) {
                    (self.hover)(Hover::files(paths, position));
                }
            }
            Notice::Moved(position) => {
                if let Some(position) = self.point(position) {
                    (self.hover)(Hover::Moved(position));
                }
            }
            Notice::Left => {
                (self.hover)(Hover::Left);
                self.released.borrow_mut().take();
            }
            Notice::Release(serial, position) => {
                let open = self.point(position).and_then(|point| (self.capture)(point));
                // The capture owns the insertion gap before hover closes a temporary pane.
                (self.hover)(Hover::Left);
                *self.released.borrow_mut() = open.map(|open| (serial, open));
            }
            Notice::Finished(serial, paths) => {
                let open = self.released.borrow_mut().take_if(|(id, _)| *id == serial);
                if let Some((_, open)) = open
                    && !paths.is_empty()
                {
                    open(paths);
                }
            }
        }
    }
}

enum Notice {
    Hover(Point, Vec<PathBuf>),
    Moved(Point),
    Left,
    Release(u64, Point),
    Finished(u64, Vec<OsString>),
}

#[derive(Default)]
struct Batch {
    notices: Vec<Notice>,
    scheduled: bool,
}

#[derive(Clone)]
struct Ui {
    id: u64,
    pending: Arc<Mutex<Batch>>,
}

impl Ui {
    fn new(id: u64) -> Self {
        Self {
            id,
            pending: Arc::default(),
        }
    }

    fn update(&self, notice: Notice) {
        let mut pending = lock(&self.pending);
        match (pending.notices.last_mut(), notice) {
            (Some(Notice::Moved(previous)), Notice::Moved(position)) => *previous = position,
            (_, notice) => pending.notices.push(notice),
        }
        if pending.scheduled {
            return;
        }
        pending.scheduled = true;
        drop(pending);
        let ui = self.clone();
        let _ = slint::invoke_from_event_loop(move || {
            let batch = std::mem::take(&mut *lock(&ui.pending));
            if let Some(target) = TARGETS.with_borrow(|targets| targets.get(&ui.id)?.upgrade()) {
                for notice in batch.notices {
                    target.update(notice);
                }
            }
        });
    }
}
