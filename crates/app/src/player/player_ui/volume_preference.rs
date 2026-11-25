//! Remember a profile's newest accepted volume without writing on every slider step.

use riina_core::{Preferences, RememberedVolume};
use slint::{Timer, TimerMode};
use std::{
    cell::Cell,
    path::{Path, PathBuf},
    rc::Rc,
    time::Duration,
};

pub(super) struct VolumePreference {
    path: PathBuf,
    fallback: Preferences,
    latest: Cell<Option<RememberedVolume>>,
    pending: Cell<Option<RememberedVolume>>,
    timer: Timer,
}

impl VolumePreference {
    pub(super) fn new(path: &Path, fallback: &Preferences) -> Self {
        Self {
            path: path.to_owned(),
            fallback: fallback.clone(),
            latest: Cell::default(),
            pending: Cell::default(),
            timer: Timer::default(),
        }
    }

    pub(super) fn pending_level(&self) -> Option<f64> {
        self.pending.get().map(|volume| volume.level)
    }

    pub(super) fn observe(self: &Rc<Self>, volume: Option<RememberedVolume>) {
        let Some(volume) = volume else { return };
        if self
            .latest
            .get()
            .is_some_and(|latest| latest.revision >= volume.revision)
        {
            return;
        }
        self.latest.set(Some(volume));
        self.pending.set(Some(volume));
        let service = Rc::downgrade(self);
        self.timer.start(
            TimerMode::SingleShot,
            Duration::from_millis(180),
            move || {
                if let Some(service) = service.upgrade()
                    && let Err(error) = service.flush()
                {
                    crate::diagnostics::warn!("Could not remember volume: {error}");
                }
            },
        );
    }

    pub(super) fn flush(&self) -> Result<(), String> {
        self.timer.stop();
        let Some(level) = self.pending_level() else {
            return Ok(());
        };
        crate::preference_store::update_automatically(&self.path, &self.fallback, |latest| {
            latest.volume = level.min(latest.volume_limit());
            Ok(())
        })?;
        self.pending.set(None);
        Ok(())
    }
}
