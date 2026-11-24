//! Deliver native plugin work on the application thread when no player can present it.
use crate::background::CoalescedUiCallback;
use slint::{Timer, TimerMode};
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

static NEXT_DRIVER: AtomicU64 = AtomicU64::new(0);
thread_local! {
    static DRIVER: RefCell<Weak<Driver>> = const { RefCell::new(Weak::new()) };
}

pub(super) struct Driver {
    id: u64,
    pending: CoalescedUiCallback,
    frames: Timer,
    present: Box<dyn Fn() -> bool>,
}

impl Driver {
    pub(super) fn new(present: impl Fn() -> bool + 'static) -> Rc<Self> {
        let driver = Rc::new(Self {
            id: NEXT_DRIVER.fetch_add(1, Ordering::Relaxed),
            pending: CoalescedUiCallback::default(),
            frames: Timer::default(),
            present: Box::new(present),
        });
        DRIVER.with(|slot| slot.replace(Rc::downgrade(&driver)));
        driver
    }

    pub(super) fn waker(&self) -> impl Fn() + Send + Sync + use<> {
        let id = self.id;
        let pending = self.pending.clone();
        move || {
            pending.post(|| {
                move || {
                    let driver = DRIVER.with(|slot| slot.borrow().upgrade());
                    if let Some(driver) = driver.filter(|driver| driver.id == id) {
                        driver.refresh();
                    }
                }
            });
        }
    }

    pub(super) fn request(&self) {
        self.waker()();
    }

    fn refresh(&self) {
        if !(self.present)() {
            self.frames.stop();
        } else if !self.frames.running() {
            // A visible WebView needs resize and document acknowledgements
            // even while its JavaScript realm emits no new window intent.
            self.frames
                .start(TimerMode::Repeated, Duration::from_millis(50), self.waker());
        }
    }
}
