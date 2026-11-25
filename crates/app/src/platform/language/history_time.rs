//! Calendar labels keep the host's regional formatting separate from row identity.
use chrono::{DateTime, Local};

#[cfg(target_os = "macos")]
#[path = "history_time/foundation.rs"]
mod system;
#[cfg(not(target_os = "macos"))]
#[path = "history_time/icu.rs"]
mod system;

#[derive(Clone, Copy)]
pub(crate) enum Style {
    DayHeading,
    Time,
    DateTime,
}

pub(crate) struct HistoryTime(system::Formatter);

impl HistoryTime {
    pub(crate) fn current() -> Self {
        Self(system::Formatter::current())
    }

    pub(crate) fn format(&self, local: &DateTime<Local>, style: Style) -> String {
        self.0.format(local, style).unwrap_or_else(|| {
            let fallback = match style {
                Style::DayHeading => "%Y-%m-%d",
                Style::Time => "%H:%M",
                Style::DateTime => "%Y-%m-%d %H:%M",
            };
            local.format(fallback).to_string()
        })
    }
}
