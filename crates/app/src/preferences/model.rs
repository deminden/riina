//! Own the saved preference snapshot and project its sections onto the UI.

use crate::{PlayerWindow, PreferencesWindow, preference_store::Writer};
use riina_core::Preferences;
use std::{cell::RefCell, path::PathBuf, rc::Rc};

pub(crate) struct SectionError(String);

impl From<String> for SectionError {
    fn from(message: String) -> Self {
        Self(message)
    }
}

impl SectionError {
    pub(crate) fn into_message(self) -> String {
        self.0
    }
}

pub(crate) trait PreferenceSection: Clone + PartialEq {
    fn get(all: &Preferences) -> Self;
    fn put(self, all: &mut Preferences);
    fn read_window(window: &PreferencesWindow) -> Result<Self, SectionError>;
    fn write_window(&self, window: &PreferencesWindow);
    // Engine-only sections are consumed by engine configuration or commands.
    fn write_player(&self, _app: &PlayerWindow) {}
}

#[derive(Clone)]
pub(crate) struct PreferenceModel(Rc<PreferenceState>);

struct PreferenceState {
    current: RefCell<Preferences>,
    writer: Writer,
}

impl PreferenceModel {
    pub(crate) fn new(path: PathBuf, initial: Preferences) -> Self {
        Self(Rc::new(PreferenceState {
            writer: Writer::new(path, initial.clone()),
            current: RefCell::new(initial),
        }))
    }

    pub(crate) fn save(&self, candidate: Preferences) -> Result<(), String> {
        self.0.writer.save(&candidate)?;
        self.0.current.replace(candidate);
        Ok(())
    }

    pub(crate) fn save_session(&self, candidate: Preferences) -> Result<(), String> {
        self.0.writer.save_session(&candidate)?;
        self.0.current.replace(candidate);
        Ok(())
    }

    pub(crate) fn snapshot(&self) -> Preferences {
        self.0.current.borrow().clone()
    }

    pub(crate) fn read_section<S: PreferenceSection>(
        &self,
        window: &PreferencesWindow,
    ) -> Result<S, String> {
        S::read_window(window).map_err(|error| {
            let previous = S::get(&self.0.current.borrow());
            previous.write_window(window);
            error.into_message()
        })
    }

    pub(crate) fn write_player<S: PreferenceSection>(&self, app: &PlayerWindow) {
        let section = S::get(&self.0.current.borrow());
        section.write_player(app);
    }
}

pub(crate) fn install_player_section<S: PreferenceSection>(
    prefs: &Preferences,
    app: &PlayerWindow,
) {
    S::get(prefs).write_player(app);
}
