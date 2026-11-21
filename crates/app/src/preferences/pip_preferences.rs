use super::model::{PreferenceSection, SectionError};
use crate::{PlayerWindow, PreferencesWindow};
use riina_core::{
    Preferences,
    pip_preferences::{Settings, WindowBehavior},
};

impl PreferenceSection for Settings {
    fn get(all: &Preferences) -> Self {
        all.picture_in_picture
    }

    fn put(self, all: &mut Preferences) {
        all.picture_in_picture = self;
    }

    fn read_window(window: &PreferencesWindow) -> Result<Self, SectionError> {
        current_preferences(window).map_err(SectionError::from)
    }

    fn write_window(&self, window: &PreferencesWindow) {
        install_preferences(window, *self);
    }

    fn write_player(&self, app: &PlayerWindow) {
        install_player(app, *self);
    }
}

pub(crate) fn current_player(window: &PlayerWindow) -> Result<Settings, String> {
    current(
        window.get_pip_window_behavior_index(),
        window.get_pip_pause_when_entering(),
        window.get_pip_toggle_by_minimizing(),
        window.get_pip_minimize_toggle_video_only(),
    )
}

pub(crate) fn current_preferences(window: &PreferencesWindow) -> Result<Settings, String> {
    current(
        window.get_pip_window_behavior_index(),
        window.get_pip_pause_when_entering(),
        window.get_pip_toggle_by_minimizing(),
        window.get_pip_minimize_toggle_video_only(),
    )
}

pub(crate) fn install_player(window: &PlayerWindow, settings: Settings) {
    window.set_pip_window_behavior_index(settings.window_behavior.index());
    window.set_pip_pause_when_entering(settings.pause_when_entering);
    window.set_pip_toggle_by_minimizing(settings.toggle_by_minimizing);
    window.set_pip_minimize_toggle_video_only(settings.minimize_toggle_video_only);
}

pub(crate) fn install_preferences(window: &PreferencesWindow, settings: Settings) {
    window.set_pip_window_behavior_index(settings.window_behavior.index());
    window.set_pip_pause_when_entering(settings.pause_when_entering);
    window.set_pip_toggle_by_minimizing(settings.toggle_by_minimizing);
    window.set_pip_minimize_toggle_video_only(settings.minimize_toggle_video_only);
}

fn current(
    behavior_index: i32,
    pause_when_entering: bool,
    toggle_by_minimizing: bool,
    minimize_toggle_video_only: bool,
) -> Result<Settings, String> {
    let window_behavior = WindowBehavior::from_index(behavior_index)
        .ok_or_else(|| "Choose a valid PiP window behavior".to_owned())?;
    Ok(Settings {
        window_behavior,
        pause_when_entering,
        toggle_by_minimizing,
        minimize_toggle_video_only,
    })
}
