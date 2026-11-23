//! Translate editable Audio controls into validated, portable mpv preferences.

use super::model::{PreferenceSection, SectionError};
use crate::{AudioSettingsState, PreferencesWindow};
use riina_core::audio_preferences::{
    Driver, Gapless, OutputDevice, ReplayGain, Settings, SpdifCodec,
};
use slint::{ComponentHandle, Model};

impl PreferenceSection for Settings {
    fn get(all: &riina_core::Preferences) -> Self {
        all.audio.clone()
    }

    fn put(self, all: &mut riina_core::Preferences) {
        all.audio = self;
    }

    fn read_window(window: &PreferencesWindow) -> Result<Self, SectionError> {
        current_window(window).map_err(SectionError::from)
    }

    fn write_window(&self, window: &PreferencesWindow) {
        let audio = window.global::<AudioSettingsState>();
        install_driver(window, self);
        // Device choices come from the active engine. Restore the selection
        // without replacing that observed list with stored descriptions.
        let index = audio
            .get_audio_device_names()
            .iter()
            .position(|device| device.as_str() == self.device)
            .unwrap_or(0);
        audio.set_audio_device_index(index as i32);
        install_output_options(window, self);
    }

    fn needs_restart(&self, previous: &Self) -> bool {
        self != previous
    }
}

fn number<T: std::str::FromStr>(text: &str, label: &str) -> Result<T, String> {
    text.trim()
        .parse()
        .map_err(|_| format!("{label} must be a number"))
}

fn selected<T: Copy>(items: &[T], index: i32, label: &str) -> Result<T, String> {
    usize::try_from(index)
        .ok()
        .and_then(|index| items.get(index))
        .copied()
        .ok_or_else(|| format!("Invalid {label} selection"))
}

pub(crate) fn current_window(window: &crate::PreferencesWindow) -> Result<Settings, String> {
    let audio = window.global::<AudioSettingsState>();
    let drivers = Driver::choices_for_current_platform();
    let replay_gain = [ReplayGain::Off, ReplayGain::Track, ReplayGain::Album];
    let gapless = [Gapless::Disabled, Gapless::Weak, Gapless::Strong];
    let device_index = usize::try_from(audio.get_audio_device_index())
        .map_err(|_| "Invalid audio device selection".to_owned())?;
    let device = audio
        .get_audio_device_names()
        .row_data(device_index)
        .ok_or_else(|| "Invalid audio device selection".to_owned())?;
    let device_description = audio
        .get_audio_device_descriptions()
        .row_data(device_index)
        .ok_or_else(|| "Invalid audio device description".to_owned())?;
    let driver = selected(drivers, audio.get_audio_driver_index(), "audio driver")?;
    let mut preferences = Settings {
        audio_threads: number(audio.get_audio_threads_text().as_str(), "Audio threads")?,
        driver,
        device: device.to_string(),
        device_description: device_description.to_string(),
        spdif: [
            (audio.get_audio_spdif_ac3(), SpdifCodec::Ac3),
            (audio.get_audio_spdif_dts(), SpdifCodec::Dts),
            (audio.get_audio_spdif_dts_hd(), SpdifCodec::DtsHd),
            (audio.get_audio_spdif_eac3(), SpdifCodec::Eac3),
            (audio.get_audio_spdif_truehd(), SpdifCodec::TrueHd),
        ]
        .into_iter()
        .filter_map(|(enabled, codec)| enabled.then_some(codec))
        .collect(),
        initial_volume_enabled: audio.get_audio_initial_volume_enabled(),
        initial_volume: number(
            audio.get_audio_initial_volume_text().as_str(),
            "Initial volume",
        )?,
        replay_gain: selected(
            &replay_gain,
            audio.get_audio_replay_gain_index(),
            "ReplayGain",
        )?,
        replay_gain_preamp: number(
            audio.get_audio_replay_gain_preamp_text().as_str(),
            "ReplayGain preamp",
        )?,
        replay_gain_clip: audio.get_audio_replay_gain_clip(),
        replay_gain_fallback: number(
            audio.get_audio_replay_gain_fallback_text().as_str(),
            "ReplayGain fallback",
        )?,
        gapless: selected(
            &gapless,
            audio.get_audio_gapless_index(),
            "gapless playback",
        )?,
        preferred_languages: audio.get_audio_preferred_languages().to_string(),
    };
    // A device ID is tied to its output driver. If the selected driver changed
    // while Settings was open, don't persist a stale ID that can prevent audio
    // initialization on the next launch; the user can choose its device after
    // the newly selected driver has started.
    if !device_matches_driver(driver, &preferences.device) {
        preferences.device = "auto".into();
        preferences.device_description = "Autoselect device".into();
        audio.set_audio_device_index(0);
    }
    preferences.validate().map_err(|error| error.to_string())?;
    Ok(preferences)
}

fn device_matches_driver(driver: Driver, name: &str) -> bool {
    if name == "auto" {
        return true;
    }
    let Some((prefix, _)) = name.split_once('/') else {
        return true;
    };
    match driver {
        Driver::Default => true,
        Driver::CoreAudio => prefix == "coreaudio",
        Driver::AvFoundation => prefix == "avfoundation",
        Driver::Wasapi => prefix == "wasapi",
        Driver::DirectSound => prefix == "dsound",
        Driver::Winmm => prefix == "winmm",
        Driver::PipeWire => prefix == "pipewire",
        Driver::PulseAudio => prefix == "pulse",
        Driver::Alsa => prefix == "alsa",
        Driver::Jack => prefix == "jack",
        Driver::Sdl => prefix == "sdl",
    }
}

pub(crate) fn install_window(
    window: &crate::PreferencesWindow,
    preferences: &Settings,
    available_devices: &[OutputDevice],
) {
    install_driver(window, preferences);
    install_devices(window, preferences, available_devices);
    install_output_options(window, preferences);
}

fn install_driver(window: &PreferencesWindow, preferences: &Settings) {
    let audio = window.global::<AudioSettingsState>();
    let drivers = Driver::choices_for_current_platform();
    audio.set_audio_threads_text(preferences.audio_threads.to_string().into());
    audio.set_audio_driver_index(
        drivers
            .iter()
            .position(|driver| *driver == preferences.driver)
            .unwrap_or(0) as i32,
    );
    audio.set_audio_driver_options(slint::ModelRc::new(slint::VecModel::from(
        drivers
            .iter()
            .map(|driver| driver.label().into())
            .collect::<Vec<slint::SharedString>>(),
    )));
}

fn install_output_options(window: &PreferencesWindow, preferences: &Settings) {
    let audio = window.global::<AudioSettingsState>();
    let replay_gain = [ReplayGain::Off, ReplayGain::Track, ReplayGain::Album];
    let gapless = [Gapless::Disabled, Gapless::Weak, Gapless::Strong];
    audio.set_audio_spdif_ac3(preferences.spdif.contains(&SpdifCodec::Ac3));
    audio.set_audio_spdif_dts(preferences.spdif.contains(&SpdifCodec::Dts));
    audio.set_audio_spdif_dts_hd(preferences.spdif.contains(&SpdifCodec::DtsHd));
    audio.set_audio_spdif_eac3(preferences.spdif.contains(&SpdifCodec::Eac3));
    audio.set_audio_spdif_truehd(preferences.spdif.contains(&SpdifCodec::TrueHd));
    audio.set_audio_initial_volume_enabled(preferences.initial_volume_enabled);
    audio.set_audio_initial_volume_text(preferences.initial_volume.to_string().into());
    audio.set_audio_replay_gain_index(
        replay_gain
            .iter()
            .position(|value| *value == preferences.replay_gain)
            .unwrap_or(0) as i32,
    );
    audio.set_audio_replay_gain_options(slint::ModelRc::new(slint::VecModel::from(
        ["Off", "Track", "Album"]
            .into_iter()
            .map(Into::into)
            .collect::<Vec<slint::SharedString>>(),
    )));
    audio.set_audio_replay_gain_preamp_text(preferences.replay_gain_preamp.to_string().into());
    audio.set_audio_replay_gain_clip(preferences.replay_gain_clip);
    audio.set_audio_replay_gain_fallback_text(preferences.replay_gain_fallback.to_string().into());
    audio.set_audio_gapless_index(
        gapless
            .iter()
            .position(|value| *value == preferences.gapless)
            .unwrap_or(1) as i32,
    );
    audio.set_audio_gapless_options(slint::ModelRc::new(slint::VecModel::from(
        ["Disabled", "Weak", "Strong"]
            .into_iter()
            .map(Into::into)
            .collect::<Vec<slint::SharedString>>(),
    )));
    audio.set_audio_preferred_languages(preferences.preferred_languages.clone().into());
    crate::language_token_preferences::install_audio(window, &preferences.preferred_languages);
}

pub(crate) fn install_devices(
    window: &crate::PreferencesWindow,
    preferences: &Settings,
    available_devices: &[OutputDevice],
) {
    let audio = window.global::<AudioSettingsState>();
    let mut devices = vec![("auto".to_owned(), "Autoselect device".to_owned())];
    devices.extend(
        available_devices
            .iter()
            .filter(|device| device.name != "auto")
            .map(|device| (device.name.clone(), device.description.clone())),
    );
    if !devices.iter().any(|(name, _)| name == &preferences.device) {
        devices.push((
            preferences.device.clone(),
            format!("{} (missing)", preferences.device_description),
        ));
    }
    let selected_device = devices
        .iter()
        .position(|(name, _)| name == &preferences.device)
        .unwrap_or(0);
    audio.set_audio_device_index(selected_device as i32);
    audio.set_audio_device_names(slint::ModelRc::new(slint::VecModel::from(
        devices
            .iter()
            .map(|(name, _)| name.clone().into())
            .collect::<Vec<slint::SharedString>>(),
    )));
    audio.set_audio_device_descriptions(slint::ModelRc::new(slint::VecModel::from(
        devices
            .iter()
            .map(|(_, description)| description.clone().into())
            .collect::<Vec<slint::SharedString>>(),
    )));
    audio.set_audio_device_options(slint::ModelRc::new(slint::VecModel::from(
        devices
            .into_iter()
            .map(|(name, description)| {
                let label = if name == "auto" {
                    "[Autoselect device] auto".to_owned()
                } else {
                    format!("[{description}] {name}")
                };
                label.into()
            })
            .collect::<Vec<slint::SharedString>>(),
    )));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_device_ids_stay_bound_to_their_selected_driver() {
        assert!(device_matches_driver(Driver::Wasapi, "auto"));
        assert!(device_matches_driver(Driver::Wasapi, "wasapi/{endpoint}"));
        assert!(!device_matches_driver(Driver::Wasapi, "dsound/{endpoint}"));
        assert!(device_matches_driver(
            Driver::Default,
            "coreaudio/{endpoint}"
        ));
    }
}
