use crate::PreferencesWindow;
use riina_core::hdr_preferences::{Algorithm, Settings};
pub(crate) fn install_window(window: &PreferencesWindow, settings: &Settings) {
    window.set_tone_mapping_algorithm_options(slint::ModelRc::new(slint::VecModel::from(
        algorithm_labels(),
    )));
    window.set_tone_mapping_enabled(settings.enabled);
    window.set_tone_mapping_target_peak_enabled(settings.target_peak_override_enabled);
    window.set_tone_mapping_target_peak_text(settings.target_peak_nits.to_string().into());
    window.set_tone_mapping_algorithm_index(settings.algorithm.index());
    window.set_tone_mapping_param_enabled(settings.param_override_enabled);
    window.set_tone_mapping_param_text(settings.param.to_string().into());
}

pub(crate) fn current_window(window: &PreferencesWindow) -> Result<Settings, String> {
    let algorithm = Algorithm::from_index(window.get_tone_mapping_algorithm_index())
        .ok_or_else(|| "Invalid tone-mapping algorithm".to_owned())?;
    let target_peak_nits = window
        .get_tone_mapping_target_peak_text()
        .trim()
        .parse::<u32>()
        .map_err(|_| "Tone-mapping target peak must be a whole number".to_owned())?;
    let param = window
        .get_tone_mapping_param_text()
        .trim()
        .parse::<f64>()
        .map_err(|_| "Tone-mapping parameter must be a number".to_owned())?;
    let settings = Settings {
        enabled: window.get_tone_mapping_enabled(),
        target_peak_override_enabled: window.get_tone_mapping_target_peak_enabled(),
        target_peak_nits,
        algorithm,
        param_override_enabled: window.get_tone_mapping_param_enabled(),
        param,
    };
    settings.validate().map_err(str::to_owned)?;
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
