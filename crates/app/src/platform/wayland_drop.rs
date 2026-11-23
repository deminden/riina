//! Receive drops on Winit's Wayland surfaces through a queue on the same display.
mod payload;
mod protocol;

use slint::{
    ComponentHandle, LogicalPosition,
    winit_030::{
        CustomApplicationHandler, EventResult, WinitWindowAccessor,
        winit::{
            event_loop::{ActiveEventLoop, OwnedDisplayHandle},
            raw_window_handle::{
                HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle,
            },
        },
    },
};
use std::{
    cell::RefCell,
    ffi::OsString,
    path::PathBuf,
    rc::{Rc, Weak},
};
use wayland_client::{Connection, EventQueue, backend::Backend};

pub(crate) enum Hover {
    Entered(Vec<PathBuf>, LogicalPosition),
    Moved(LogicalPosition),
    Left,
}

pub(crate) type OpenDrop = Box<dyn FnOnce(Vec<OsString>)>;

struct Target {
    surface: Box<dyn Fn() -> Option<(usize, f32)>>,
    hover: Box<dyn Fn(Hover)>,
    capture: Box<dyn Fn(LogicalPosition) -> Option<OpenDrop>>,
}

thread_local! {
    static TARGETS: RefCell<Vec<Weak<Target>>> = const { RefCell::new(Vec::new()) };
}

pub(crate) struct Registration {
    _target: Rc<Target>,
}

pub(crate) fn register<T: ComponentHandle + 'static>(
    window: &T,
    hover: impl Fn(Hover) + 'static,
    capture: impl Fn(LogicalPosition) -> Option<OpenDrop> + 'static,
) -> Registration {
    let weak = window.as_weak();
    let target = Rc::new(Target {
        surface: Box::new(move || {
            let component = weak.upgrade()?;
            let window = component.window();
            if !window.is_visible() {
                return None;
            }
            window.with_winit_window(|window| match window.window_handle().ok()?.as_raw() {
                RawWindowHandle::Wayland(handle) => Some((
                    handle.surface.as_ptr().addr(),
                    window.scale_factor() as f32 / component.window().scale_factor(),
                )),
                _ => None,
            })?
        }),
        hover: Box::new(hover),
        capture: Box::new(capture),
    });
    TARGETS.with(|targets| {
        let mut targets = targets.borrow_mut();
        targets.retain(|target| target.strong_count() != 0);
        targets.push(Rc::downgrade(&target));
    });
    Registration { _target: target }
}

fn find_target(surface: usize) -> Option<(Rc<Target>, f32)> {
    TARGETS.with(|targets| {
        targets
            .borrow()
            .iter()
            .filter_map(Weak::upgrade)
            .find_map(|target| {
                let (address, scale) = (target.surface)()?;
                (address == surface).then_some((target, scale))
            })
    })
}

#[derive(Default)]
pub(crate) struct Handler(Option<Receiver>);

struct Receiver {
    state: protocol::State,
    queue: EventQueue<protocol::State>,
    // Keep the owner alive until the borrowed queue and all its proxies are dropped.
    _display: OwnedDisplayHandle,
}

impl Receiver {
    fn new(display: OwnedDisplayHandle) -> Option<Self> {
        let RawDisplayHandle::Wayland(native) = display.display_handle().ok()?.as_raw() else {
            return None;
        };
        // SAFETY: OwnedDisplayHandle owns Winit's display while this guest queue exists.
        let backend = unsafe { Backend::from_foreign_display(native.display.as_ptr().cast()) };
        let connection = Connection::from_backend(backend);
        let queue = connection.new_event_queue();
        let registry = connection.display().get_registry(&queue.handle(), ());
        Some(Self {
            state: protocol::State::new(registry),
            queue,
            _display: display,
        })
    }
}

impl CustomApplicationHandler for Handler {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) -> EventResult {
        if self.0.is_none() {
            self.0 = Receiver::new(event_loop.owned_display_handle());
        }
        EventResult::Propagate
    }

    fn about_to_wait(&mut self, _: &ActiveEventLoop) -> EventResult {
        if let Some(receiver) = self.0.as_mut() {
            // Winit reads the shared socket. The guest dispatches only its own queue.
            if let Err(error) = receiver.queue.dispatch_pending(&mut receiver.state) {
                crate::diagnostics::warn!("Wayland drop queue closed: {error}");
                self.0 = None;
            } else {
                receiver.state.deliver_payloads();
                let _ = receiver.queue.flush();
            }
        }
        EventResult::Propagate
    }

    fn exiting(&mut self, _: &ActiveEventLoop) -> EventResult {
        self.0 = None;
        EventResult::Propagate
    }
}
