use super::model::{PreferenceSection, SectionError};
use crate::{PreferencesWindow, VideoSettingsState};
use riina_core::Preferences;
use riina_core::hdr_preferences::{Algorithm, Settings};
use slint::ComponentHandle;

pub(crate) fn install_window(window: &PreferencesWindow, settings: &Settings) {
    let video = window.global::<VideoSettingsState>();
    video.set_tone_mapping_algorithm_options(slint::ModelRc::new(slint::VecModel::from(
        algorithm_labels(),
    )));
    video.set_tone_mapping_enabled(settings.enabled);
    video.set_tone_mapping_target_peak_enabled(settings.target_peak_override_enabled);
    video.set_tone_mapping_target_peak_text(settings.target_peak_nits.to_string().into());
    video.set_tone_mapping_algorithm_index(settings.algorithm.index());
    video.set_tone_mapping_param_enabled(settings.param_override_enabled);
    video.set_tone_mapping_param_text(settings.param.to_string().into());
}

impl PreferenceSection for Settings {
    fn get(all: &Preferences) -> Self {
        all.hdr_tone_mapping.clone()
    }

    fn put(self, all: &mut Preferences) {
        all.hdr_tone_mapping = self;
    }

    fn read_window(window: &PreferencesWindow) -> Result<Self, SectionError> {
        current_window(window).map_err(SectionError::from)
    }

    fn write_window(&self, window: &PreferencesWindow) {
        install_window(window, self);
    }
}

pub(crate) fn current_window(window: &PreferencesWindow) -> Result<Settings, String> {
    let video = window.global::<VideoSettingsState>();
    let algorithm = Algorithm::from_index(video.get_tone_mapping_algorithm_index())
        .ok_or_else(|| "Invalid tone-mapping algorithm".to_owned())?;
    let target_peak_nits = video
        .get_tone_mapping_target_peak_text()
        .trim()
        .parse::<u32>()
        .map_err(|_| "Tone-mapping target peak must be a whole number".to_owned())?;
    let param = video
        .get_tone_mapping_param_text()
        .trim()
        .parse::<f64>()
        .map_err(|_| "Tone-mapping parameter must be a number".to_owned())?;
    let settings = Settings {
        enabled: video.get_tone_mapping_enabled(),
        target_peak_override_enabled: video.get_tone_mapping_target_peak_enabled(),
        target_peak_nits,
        algorithm,
        param_override_enabled: video.get_tone_mapping_param_enabled(),
        param,
    };
    settings.validate().map_err(|error| error.to_string())?;
    Ok(settings)
}

pub(crate) fn algorithm_labels() -> Vec<slint::SharedString> {
    [
        "Auto", "Clip", "Mobius", "Reinhard", "Hable", "BT.2390", "Gamma", "Linear",
    ]
    .into_iter()
    .map(Into::into)
    .collect()
}

#[cfg(test)]
mod tests {
    use super::algorithm_labels;

    #[test]
    fn labels_follow_the_pinned_codec_popup_order() {
        assert_eq!(
            algorithm_labels()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            [
                "Auto", "Clip", "Mobius", "Reinhard", "Hable", "BT.2390", "Gamma", "Linear"
            ]
        );
    }
}
