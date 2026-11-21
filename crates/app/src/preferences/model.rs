//! Own the saved preference snapshot and project its sections onto the UI.

use crate::{PlayerWindow, PreferencesWindow, preference_store::Writer};
use riina_core::{Preferences, thumbnail_preferences::Settings as ThumbnailPreview};
use std::{cell::RefCell, path::PathBuf};

pub(crate) struct SectionError(String);

impl From<String> for SectionError {
    fn from(message: String) -> Self {
        Self(message)
    }
}

pub(crate) trait PreferenceSection: Clone + PartialEq {
    fn get(all: &Preferences) -> Self;
    fn put(self, all: &mut Preferences);
    fn read_window(window: &PreferencesWindow) -> Result<Self, SectionError>;
    fn write_window(&self, window: &PreferencesWindow);
    fn write_player(&self, app: &PlayerWindow);
}

pub(crate) struct PreferenceModel {
    current: RefCell<Preferences>,
    writer: Writer,
}

impl PreferenceModel {
    pub(crate) fn new(path: PathBuf, initial: Preferences) -> Self {
        Self {
            writer: Writer::new(path, initial.clone()),
            current: RefCell::new(initial),
        }
    }

    pub(crate) fn save(&self, candidate: Preferences) -> Result<(), String> {
        self.writer.save(&candidate)?;
        self.current.replace(candidate);
        Ok(())
    }

    pub(crate) fn read_section<S: PreferenceSection>(
        &self,
        window: &PreferencesWindow,
    ) -> Result<S, String> {
        S::read_window(window).map_err(|error| {
            let previous = S::get(&self.current.borrow());
            previous.write_window(window);
            error.0
        })
    }

    pub(crate) fn write_player<S: PreferenceSection>(&self, app: &PlayerWindow) {
        let section = S::get(&self.current.borrow());
        section.write_player(app);
    }
}

pub(crate) fn install_player_sections(prefs: &Preferences, app: &PlayerWindow) {
    ThumbnailPreview::get(prefs).write_player(app);
}

impl PreferenceSection for ThumbnailPreview {
    fn get(all: &Preferences) -> Self {
        all.thumbnail_preview.clone()
    }

    fn put(self, all: &mut Preferences) {
        all.thumbnail_preview = self;
    }

    fn read_window(window: &PreferencesWindow) -> Result<Self, SectionError> {
        crate::thumbnail_preferences::current_window(window).map_err(SectionError::from)
    }

    fn write_window(&self, window: &PreferencesWindow) {
        crate::thumbnail_preferences::install_window(window, self);
    }

    fn write_player(&self, app: &PlayerWindow) {
        crate::thumbnail_preferences::install_player(app, self);
    }
}
