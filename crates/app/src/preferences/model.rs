//! Own the saved preference snapshot and project its sections onto the UI.

use crate::{PlayerWindow, PreferencesWindow, preference_store::Writer};
use riina_core::{
    Command, Preferences, control_preferences, liquid_glass, network_preferences, online_subtitles,
    osc_preferences, pip_preferences, subtitle_controls, thumbnail_preferences, window_resize,
};
use slint::ComponentHandle;
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
    fn live_commands(&self, _previous: &Self) -> Vec<Command> {
        Vec::new()
    }
    fn needs_restart(&self, _previous: &Self) -> bool {
        false
    }
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

    pub(crate) fn retain_subtitle_credentials(&self, app: &PlayerWindow) {
        let api_key = app
            .global::<riina_ui::PlayerPreferences>()
            .get_open_subtitles_api_key()
            .to_string();
        let assrt_token = app
            .global::<riina_ui::PlayerPreferences>()
            .get_assrt_token()
            .to_string();
        let mut current = self.0.current.borrow_mut();
        current.online_subtitles.api_key = api_key;
        current.online_subtitles.assrt_token = assrt_token;
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

type Projection = fn(&Preferences, &PlayerWindow);

const PLAYER_SECTIONS: &[Projection] = &[
    install_player_section::<control_preferences::Settings>,
    install_player_section::<subtitle_controls::PositionPreferences>,
    install_player_section::<subtitle_controls::OtherPreferences>,
    install_player_section::<subtitle_controls::AutoLoadPreferences>,
    install_player_section::<online_subtitles::Preferences>,
    install_player_section::<network_preferences::Cache>,
    install_player_section::<network_preferences::OnlineVideos>,
    install_player_section::<crate::osd_preferences::Settings>,
    install_player_section::<osc_preferences::Settings>,
    install_player_section::<thumbnail_preferences::Settings>,
    install_player_section::<window_resize::Settings>,
    install_player_section::<liquid_glass::SurfaceSettings>,
    install_player_section::<pip_preferences::Settings>,
];

pub(crate) fn install_player_sections(prefs: &Preferences, app: &PlayerWindow) {
    for project in PLAYER_SECTIONS {
        project(prefs, app);
    }
}

fn install_player_section<S: PreferenceSection>(prefs: &Preferences, app: &PlayerWindow) {
    S::get(prefs).write_player(app);
}
