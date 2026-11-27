use super::Link;
use crate::background::CoalescedUiCallback;
use riina_core::sync::lock;
use std::{
    cell::RefCell,
    collections::VecDeque,
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

pub(crate) enum Delivery {
    Link(Link),
    #[cfg(target_os = "linux")]
    Documents(Vec<String>),
}

type Handler = Rc<dyn Fn(Delivery)>;

thread_local! {
    static HANDLER: RefCell<Option<Handler>> = RefCell::default();
}

#[derive(Clone, Default)]
pub(crate) struct Inbox {
    pending: Arc<Mutex<VecDeque<Delivery>>>,
    delivery: CoalescedUiCallback,
    ready: Arc<AtomicBool>,
}

impl Inbox {
    pub(crate) fn push(&self, delivery: Delivery) -> bool {
        {
            let mut pending = lock(&self.pending);
            if pending.len() >= 16 {
                return false;
            }
            pending.push_back(delivery);
        }
        if self.ready.load(Ordering::Acquire) {
            self.schedule();
        }
        true
    }

    pub(crate) fn register(&self, handler: impl Fn(Delivery) + 'static) -> Registration {
        HANDLER.with(|slot| slot.replace(Some(Rc::new(handler))));
        self.ready.store(true, Ordering::Release);
        self.schedule();
        Registration(self.ready.clone())
    }

    fn schedule(&self) {
        self.delivery.post(|| {
            let pending = self.pending.clone();
            move || {
                if let Some(handler) = HANDLER.with(|slot| slot.borrow().clone()) {
                    let batch = std::mem::take(&mut *lock(&pending));
                    for delivery in batch {
                        handler(delivery);
                    }
                }
            }
        });
    }
}

pub(crate) struct Registration(Arc<AtomicBool>);

impl Drop for Registration {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
        let _ = HANDLER.try_with(|slot| slot.replace(None));
    }
}
