//! The global plugin host owns each managed player until its controller retires.
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Target {
    Broadcast,
    Instance(u64),
    PlayerLabel(String),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Message {
    pub(crate) target: Option<Target>,
    pub(crate) name: String,
    pub(crate) data: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Options {
    pub(crate) source: Option<String>,
    pub(crate) label: Option<String>,
    pub(crate) enable_plugins: bool,
    pub(crate) disable_ui: bool,
    pub(crate) disable_window_animation: bool,
}

pub(crate) struct Ticket {
    pub(crate) package: String,
    pub(crate) instance: u64,
    pub(crate) options: Options,
    closed: AtomicBool,
    waiting_for_media: AtomicBool,
}

impl Ticket {
    pub(crate) fn new(package: String, instance: u64, options: Options) -> Arc<Self> {
        let waiting = options.source.is_none();
        Arc::new(Self {
            package,
            instance,
            options,
            closed: AtomicBool::new(false),
            waiting_for_media: AtomicBool::new(waiting),
        })
    }

    pub(crate) fn is_open(&self) -> bool {
        !self.closed.load(Ordering::Acquire)
    }

    pub(crate) fn close(&self) {
        self.closed.store(true, Ordering::Release);
    }

    pub(crate) fn is_waiting_for_media(&self) -> bool {
        self.waiting_for_media.load(Ordering::Acquire)
    }

    pub(crate) fn reveal_for_media(&self) -> bool {
        self.is_open() && self.waiting_for_media.swap(false, Ordering::AcqRel)
    }

    pub(crate) fn internal_label(&self) -> String {
        format!("{}-{}", self.instance, self.package)
    }

    pub(crate) fn allows(&self, package: &str) -> bool {
        self.is_open() && (self.options.enable_plugins || self.package == package)
    }
}
