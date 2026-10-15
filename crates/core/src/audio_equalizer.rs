//! Portable ten-band equalizer profiles and libmpv filter construction.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const FREQUENCIES_HZ: [u32; 10] = [32, 64, 125, 250, 500, 1_000, 2_000, 4_000, 8_000, 16_000];
pub const MIN_GAIN_DB: f64 = -12.0;
pub const MAX_GAIN_DB: f64 = 12.0;
pub const FILTER_LABEL: &str = "riina-audio-equalizer";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileId {
    Manual,
    Preset(String),
    User(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    #[serde(default)]
    pub gains_db: [f64; 10],
    #[serde(default)]
    pub selected_profile: Option<String>,
    #[serde(default)]
    pub selected_profile_is_user: bool,
    #[serde(default)]
    pub user_profiles: BTreeMap<String, [f64; 10]>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            gains_db: [0.0; 10],
            selected_profile: Some("custom".into()),
            selected_profile_is_user: false,
            user_profiles: BTreeMap::new(),
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<(), &'static str> {
        validate_gains(&self.gains_db)?;
        if self.user_profiles.iter().any(|(name, gains)| {
            !valid_profile_name(name) || name == "custom" || validate_gains(gains).is_err()
        }) {
            return Err("Audio equalizer profiles have an invalid name or gain");
        }
        if self.selected_profile.is_some() && self.selected_profile_index().is_none() {
            return Err("Selected audio equalizer profile does not exist");
        }
        Ok(())
    }

    pub fn select(&mut self, name: &str) -> Result<(), &'static str> {
        if name == "custom" {
            self.selected_profile = Some(name.to_owned());
            self.selected_profile_is_user = false;
            return Ok(());
        }
        if let Some(gains) = preset(name) {
            if self.user_profiles.contains_key(name) {
                return Err("Profile name is ambiguous; select the preset or saved profile by ID");
            }
            self.gains_db = gains;
            self.selected_profile = Some(name.to_owned());
            self.selected_profile_is_user = false;
            return Ok(());
        }
        self.select_user_profile(name)
    }

    pub fn select_id(&mut self, id: &ProfileId) -> Result<(), &'static str> {
        match id {
            ProfileId::Manual => {
                self.selected_profile = Some("custom".to_owned());
                self.selected_profile_is_user = false;
                Ok(())
            }
            ProfileId::Preset(name) => {
                let gains = preset(name).ok_or("Unknown audio equalizer profile")?;
                self.gains_db = gains;
                self.selected_profile = Some(name.to_owned());
                self.selected_profile_is_user = false;
                Ok(())
            }
            ProfileId::User(name) => self.select_user_profile(name),
        }
    }

    fn select_user_profile(&mut self, name: &str) -> Result<(), &'static str> {
        let gains = self
            .user_profiles
            .get(name)
            .copied()
            .ok_or("Unknown audio equalizer profile")?;
        self.gains_db = gains;
        self.selected_profile = Some(name.to_owned());
        self.selected_profile_is_user = true;
        Ok(())
    }

    pub fn profile_ids(&self) -> Vec<ProfileId> {
        let mut ids = vec![ProfileId::Manual];
        ids.extend(
            PRESETS
                .iter()
                .map(|(name, _)| ProfileId::Preset((*name).to_owned())),
        );
        ids.extend(self.user_profiles.keys().cloned().map(ProfileId::User));
        ids
    }

    pub fn selected_profile_index(&self) -> Option<usize> {
        let selected = self.selected_profile.as_deref()?;
        self.profile_ids().iter().position(|id| match id {
            ProfileId::Manual => !self.selected_profile_is_user && selected == "custom",
            ProfileId::Preset(name) => !self.selected_profile_is_user && selected == name,
            ProfileId::User(name) => {
                selected == name
                    && (self.selected_profile_is_user
                        || (preset(selected).is_none()
                            && self.user_profiles.contains_key(selected)))
            }
        })
    }

    pub fn selected_profile_is_user(&self) -> bool {
        self.selected_profile_index()
            .and_then(|index| self.profile_ids().get(index).cloned())
            .is_some_and(|id| matches!(id, ProfileId::User(_)))
    }

    pub fn is_manual_selected(&self) -> bool {
        self.selected_profile_index() == Some(0)
    }

    pub fn select_profile_at(&mut self, index: usize) -> Result<(), &'static str> {
        let id = self
            .profile_ids()
            .get(index)
            .cloned()
            .ok_or("Unknown audio equalizer profile")?;
        self.select_id(&id)
    }

    pub fn validate_new_profile_name(&self, name: &str) -> Result<(), &'static str> {
        if !valid_profile_name(name) {
            return Err("Profile name must be 1 to 64 printable characters");
        }
        if name == "custom" {
            return Err("A built-in profile already has that name");
        }
        if self.user_profiles.contains_key(name) {
            return Err("A saved profile already has that name");
        }
        Ok(())
    }

    pub fn save_profile(&mut self, name: &str) -> Result<(), &'static str> {
        self.validate_new_profile_name(name)?;
        validate_gains(&self.gains_db)?;
        self.user_profiles.insert(name.to_owned(), self.gains_db);
        self.selected_profile = Some(name.to_owned());
        self.selected_profile_is_user = true;
        Ok(())
    }

    pub fn remove_profile(&mut self, name: &str) -> Result<(), &'static str> {
        if self.user_profiles.remove(name).is_none() {
            return Err("Only saved audio equalizer profiles can be removed");
        }
        self.selected_profile = Some("custom".into());
        self.selected_profile_is_user = false;
        Ok(())
    }

    pub fn rename_profile(&mut self, current: &str, replacement: &str) -> Result<(), &'static str> {
        self.validate_renamed_profile_name(current, replacement)?;
        let gains = self
            .user_profiles
            .remove(current)
            .ok_or("Saved audio equalizer profile disappeared")?;
        self.user_profiles.insert(replacement.to_owned(), gains);
        self.selected_profile = Some(replacement.to_owned());
        self.selected_profile_is_user = true;
        Ok(())
    }

    pub fn validate_renamed_profile_name(
        &self,
        current: &str,
        replacement: &str,
    ) -> Result<(), &'static str> {
        if !self.user_profiles.contains_key(current) {
            return Err("Only saved audio equalizer profiles can be renamed");
        }
        if !valid_profile_name(replacement) {
            return Err("Profile name must be 1 to 64 printable characters");
        }
        if replacement == "custom" || self.user_profiles.contains_key(replacement) {
            return Err("An audio equalizer profile already has that name");
        }
        Ok(())
    }

    pub fn profile_names(&self) -> Vec<String> {
        let mut names = vec!["custom".to_owned()];
        names.extend(PRESETS.iter().map(|(name, _)| (*name).to_owned()));
        names.extend(self.user_profiles.keys().cloned());
        names
    }

    pub fn is_user_profile(&self, name: &str) -> bool {
        self.user_profiles.contains_key(name)
    }
}

const PRESETS: [(&str, [f64; 10]); 22] = [
    ("flat", [0., 0., 0., 0., 0., 0., 0., 0., 0., 0.]),
    (
        "acoustic",
        [5., 4.9, 3.95, 1.05, 2.15, 1.75, 3.5, 4.1, 3.55, 2.15],
    ),
    (
        "classical",
        [4.75, 3.75, 3., 2.5, -1.5, -1.5, 0., 2.25, 3.25, 3.75],
    ),
    (
        "dance",
        [3.57, 6.55, 4.99, 0., 1.92, 3.65, 5.15, 4.54, 3.59, 0.],
    ),
    (
        "deep",
        [4.95, 3.55, 1.75, 1., 2.85, 2.5, 1.45, -2.15, -3.55, -4.6],
    ),
    (
        "electronic",
        [4.25, 3.8, 1.2, 0., -2.15, 2.25, 0.85, 1.25, 3.95, 4.8],
    ),
    ("hip_hop", [5., 4.25, 1.5, 3., -1., -1., 1.5, -0.5, 2., 3.]),
    (
        "increase_bass",
        [5.5, 4.25, 3.5, 2.5, 1.25, 0., 0., 0., 0., 0.],
    ),
    (
        "increase_treble",
        [0., 0., 0., 0., 0., 1.25, 2.5, 3.5, 4.25, 5.5],
    ),
    (
        "increase_vocal",
        [-1.5, -3., -3., 1.5, 3.75, 3.75, 3., 1.5, 0., -1.5],
    ),
    ("jazz", [4., 3., 1.5, 2.25, -1.5, -1.5, 0., 1.5, 3., 3.75]),
    ("latin", [4.5, 3., 0., 0., -1.5, -1.5, -1.5, 0., 3., 4.5]),
    ("loundness", [6., 4., 0., 0., -2., 0., -1., -5., 5., 1.]),
    ("lounge", [-3., -1.5, -0.5, 1.5, 4., 2.5, 0., -1.5, 2., 1.]),
    ("piano", [3., 2., 0., 2.5, 3., 1.5, 3.5, 4.5, 3., 3.5]),
    ("pop", [-1.5, -1., 0., 2., 4., 4., 2., 0., -1., -1.5]),
    (
        "rnb",
        [2.62, 6.92, 5.65, 1.33, -2.19, -1.5, 2.32, 2.65, 3., 3.75],
    ),
    (
        "reduce_bass",
        [-5.5, -4.25, -3.5, -2.5, -1.25, 0., 0., 0., 0., 0.],
    ),
    (
        "reduce_treble",
        [0., 0., 0., 0., 0., -1.25, -2.5, -3.5, -4.25, -5.5],
    ),
    ("rock", [5., 4., 3., 1.5, -0.5, -1., 0.5, 2.5, 3.5, 4.5]),
    (
        "small_speaker",
        [5.5, 4.25, 3.5, 2.5, 1.25, 0., -1.25, -2.5, -3.5, -4.25],
    ),
    (
        "spoken_word",
        [-3.46, -0.47, 0., 0.69, 3.46, 4.61, 4.84, 4.28, 2.54, 0.],
    ),
];

pub fn preset(name: &str) -> Option<[f64; 10]> {
    PRESETS
        .iter()
        .find(|(key, _)| *key == name)
        .map(|(_, gains)| *gains)
}

/// Decode only the filter carrying Riina's label. Unrelated native filters are
/// ignored, and malformed or externally edited graphs remain unknown.
pub fn observed_gains(filters: &[serde_json::Value]) -> Option<[f64; 10]> {
    let filter = filters.iter().find(|filter| {
        filter
            .get("label")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|label| label == FILTER_LABEL)
    });
    let Some(filter) = filter else {
        return Some([0.0; 10]);
    };
    let graph = filter.get("params")?.get("graph")?.as_str()?;
    let mut gains = [0.0; 10];
    let mut found = [false; 10];
    for band in graph.trim_matches(['[', ']']).split(',') {
        let mut frequency = None;
        let mut gain = None;
        for field in band.split(':') {
            if let Some(value) = field
                .strip_prefix("equalizer=f=")
                .or_else(|| field.strip_prefix("f="))
            {
                frequency = value.parse::<u32>().ok();
            } else if let Some(value) = field.strip_prefix("g=") {
                gain = value.parse::<f64>().ok();
            }
        }
        let (Some(frequency), Some(gain)) = (frequency, gain) else {
            continue;
        };
        let index = FREQUENCIES_HZ.iter().position(|item| *item == frequency)?;
        if found[index] || !gain.is_finite() || !(MIN_GAIN_DB..=MAX_GAIN_DB).contains(&gain) {
            return None;
        }
        gains[index] = gain;
        found[index] = true;
    }
    found.iter().all(|found| *found).then_some(gains)
}

fn valid_profile_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= 64 && name.chars().all(|character| !character.is_control())
}

fn validate_gains(gains: &[f64; 10]) -> Result<(), &'static str> {
    if gains
        .iter()
        .any(|gain| !gain.is_finite() || !(MIN_GAIN_DB..=MAX_GAIN_DB).contains(gain))
    {
        Err("Audio equalizer gains must be finite and between -12 and +12 dB")
    } else {
        Ok(())
    }
}

/// Build the labeled FFmpeg filter used by libmpv. A flat curve means removal.
pub fn lavfi_filter(gains_db: &[f64; 10]) -> Result<Option<String>, &'static str> {
    validate_gains(gains_db)?;
    if gains_db.iter().all(|gain| *gain == 0.0) {
        return Ok(None);
    }
    let bands = FREQUENCIES_HZ
        .iter()
        .zip(gains_db)
        .map(|(frequency, gain)| {
            let width_hz = *frequency as f64 / 1.224_744_871;
            format!("equalizer=f={frequency}:t=h:width={width_hz:.6}:g={gain:.2}")
        });
    Ok(Some(format!(
        "@{FILTER_LABEL}:lavfi=[{}]",
        bands.collect::<Vec<_>>().join(",")
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gains_validate_and_build_filter_graph() {
        assert_eq!(lavfi_filter(&[0.; 10]).unwrap(), None);
        let mut gains = [0.; 10];
        gains[0] = -12.;
        gains[9] = 12.;
        let graph = lavfi_filter(&gains).unwrap().unwrap();
        assert!(graph.starts_with("@riina-audio-equalizer:lavfi=[equalizer=f=32:"));
        assert!(graph.contains("equalizer=f=16000:"));
        gains[3] = f64::NAN;
        assert!(lavfi_filter(&gains).is_err());
    }

    #[test]
    fn defaults_flat_and_presets_are_selectable() {
        let mut settings = Settings::default();
        assert_eq!(settings.gains_db, [0.; 10]);
        settings.select("rock").unwrap();
        assert_eq!(settings.gains_db, preset("rock").unwrap());
        settings.gains_db[0] = 1.5;
        settings.save_profile("My mix").unwrap();
        settings.gains_db = [0.; 10];
        settings.select("My mix").unwrap();
        assert_eq!(
            settings.rename_profile("My mix", "My mix"),
            Err("An audio equalizer profile already has that name")
        );
        assert_eq!(settings.gains_db[0], 1.5);
        settings.rename_profile("My mix", "Renamed").unwrap();
        assert_eq!(settings.user_profiles.get("Renamed").unwrap()[0], 1.5);
        settings.remove_profile("Renamed").unwrap();
        assert!(settings.user_profiles.is_empty());
    }

    #[test]
    fn saved_profile_names_preserve_exact_prompt_input() {
        let mut settings = Settings::default();
        assert!(settings.validate_new_profile_name("").is_err());
        assert!(settings.validate_new_profile_name("  ").is_ok());
        settings.save_profile("  ").unwrap();
        assert!(settings.user_profiles.contains_key("  "));
        assert_eq!(settings.selected_profile.as_deref(), Some("  "));
        assert!(settings.rename_profile("  ", " Padded ").is_ok());
        assert!(settings.user_profiles.contains_key(" Padded "));
        assert!(
            settings
                .validate_renamed_profile_name(" Padded ", " Padded ")
                .is_err()
        );
    }

    #[test]
    fn user_profiles_can_share_an_internal_preset_key() {
        let mut settings = Settings::default();
        settings.gains_db[0] = 3.5;
        settings.save_profile("rock").unwrap();
        assert!(settings.validate().is_ok());
        assert!(settings.selected_profile_is_user());
        assert_eq!(
            settings.select("rock"),
            Err("Profile name is ambiguous; select the preset or saved profile by ID")
        );

        let ids = settings.profile_ids();
        let preset_index = ids
            .iter()
            .position(|id| *id == ProfileId::Preset("rock".into()))
            .unwrap();
        let user_index = ids
            .iter()
            .position(|id| *id == ProfileId::User("rock".into()))
            .unwrap();
        settings.select_profile_at(preset_index).unwrap();
        assert!(!settings.selected_profile_is_user());
        assert_eq!(settings.gains_db, preset("rock").unwrap());
        settings.select_profile_at(user_index).unwrap();
        assert!(settings.selected_profile_is_user());
        assert_eq!(settings.gains_db[0], 3.5);

        let encoded = serde_json::to_string(&settings).unwrap();
        let decoded: Settings = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded.profile_ids(), settings.profile_ids());
        assert_eq!(decoded.selected_profile_index(), Some(user_index));
        assert_eq!(decoded.gains_db, settings.gains_db);
    }

    #[test]
    fn legacy_saved_profile_selection_is_inferred_without_the_kind_field() {
        let encoded = serde_json::json!({
            "gains_db": vec![0.0; 10],
            "selected_profile": "Cinema",
            "user_profiles": {"Cinema": vec![0.0; 10]}
        });
        let settings: Settings = serde_json::from_value(encoded).unwrap();
        assert!(settings.validate().is_ok());
        assert!(settings.selected_profile_is_user());
        assert_eq!(settings.selected_profile_index(), Some(23));
    }

    #[test]
    fn native_observation_ignores_unrelated_filters_and_decodes_riina_filter() {
        let unrelated = serde_json::json!([{
            "name": "acompressor", "label": "compressor", "params": {}
        }]);
        assert_eq!(
            observed_gains(unrelated.as_array().unwrap()),
            Some([0.; 10])
        );

        let graph = lavfi_filter(&[1.25, 0., 0., 0., 0., 0., 0., 0., 0., -2.5])
            .unwrap()
            .unwrap();
        let graph = graph.split_once("lavfi=").unwrap().1;
        let observed = serde_json::json!([{
            "name": "lavfi", "label": FILTER_LABEL, "params": {"graph": graph}
        }]);
        assert_eq!(
            observed_gains(observed.as_array().unwrap()),
            Some([1.25, 0., 0., 0., 0., 0., 0., 0., 0., -2.5])
        );
    }

    #[test]
    fn saved_profiles_round_trip_with_application_preferences() {
        let mut preferences = crate::Preferences::default();
        preferences.audio_equalizer.gains_db[4] = 2.5;
        preferences.audio_equalizer.save_profile("Cinema").unwrap();
        let encoded = serde_json::to_string(&preferences).unwrap();
        let decoded = crate::Preferences::parse(&encoded).unwrap();
        assert_eq!(decoded.audio_equalizer, preferences.audio_equalizer);
    }
}

#[cfg(test)]
mod command_tests {
    use crate::{Command, Playback};

    #[test]
    fn command_plans_add_replace_and_remove_only_the_labeled_filter() {
        let mut gains = [0.; 10];
        gains[0] = 3.5;
        let mut state = Playback {
            audio_equalizer: Some([0.; 10]),
            ..Playback::default()
        };
        let added = Command::AudioEqualizer(gains).plan(&state).unwrap();
        assert_eq!(added.commands.len(), 1);
        assert_eq!(added.commands[0][..2], ["af", "add"]);
        assert!(added.commands[0][2].contains("@riina-audio-equalizer"));

        state.audio_equalizer = Some(gains);
        let replaced = Command::AudioEqualizer([0.; 10]).plan(&state).unwrap();
        assert_eq!(
            replaced.commands,
            vec![vec!["af", "remove", "@riina-audio-equalizer"]]
        );

        state.audio_equalizer = Some([0.; 10]);
        assert!(
            Command::AudioEqualizer([0.; 10])
                .plan(&state)
                .unwrap()
                .commands
                .is_empty()
        );
        assert!(
            Command::AudioEqualizer([f64::NAN; 10])
                .plan(&state)
                .is_err()
        );
    }
}
