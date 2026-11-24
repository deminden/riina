//! User preferences for idle sleep during playback.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Idle,
    System,
    Display,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(default)]
pub struct Settings {
    pub prevent_screen_saver: bool,
    pub allow_screen_saver_for_audio: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            prevent_screen_saver: true,
            allow_screen_saver_for_audio: false,
        }
    }
}

impl Settings {
    pub fn request(self, first_playing_is_audio: Option<bool>) -> Mode {
        match first_playing_is_audio.filter(|_| self.prevent_screen_saver) {
            None => Mode::Idle,
            Some(true) if self.allow_screen_saver_for_audio => Mode::System,
            Some(_) => Mode::Display,
        }
    }
}
