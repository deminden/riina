//! Translate editable Audio controls into validated, portable mpv preferences.

use riina_core::audio_preferences::{
    Driver, Gapless, OutputDevice, ReplayGain, Settings, SpdifCodec,
};
use slint::Model;

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
    let drivers = Driver::choices_for_current_platform();
    let replay_gain = [ReplayGain::Off, ReplayGain::Track, ReplayGain::Album];
    let gapless = [Gapless::Disabled, Gapless::Weak, Gapless::Strong];
    let device_index = usize::try_from(window.get_audio_device_index())
        .map_err(|_| "Invalid audio device selection".to_owned())?;
    let device = window
        .get_audio_device_names()
        .row_data(device_index)
        .ok_or_else(|| "Invalid audio device selection".to_owned())?;
    let device_description = window
        .get_audio_device_descriptions()
        .row_data(device_index)
        .ok_or_else(|| "Invalid audio device description".to_owned())?;
    let driver = selected(drivers, window.get_audio_driver_index(), "audio driver")?;
    let mut preferences = Settings {
        audio_threads: number(window.get_audio_threads_text().as_str(), "Audio threads")?,
        driver,
        device: device.to_string(),
        device_description: device_description.to_string(),
        spdif: [
            (window.get_audio_spdif_ac3(), SpdifCodec::Ac3),
            (window.get_audio_spdif_dts(), SpdifCodec::Dts),
            (window.get_audio_spdif_dts_hd(), SpdifCodec::DtsHd),
            (window.get_audio_spdif_eac3(), SpdifCodec::Eac3),
            (window.get_audio_spdif_truehd(), SpdifCodec::TrueHd),
        ]
        .into_iter()
        .filter_map(|(enabled, codec)| enabled.then_some(codec))
        .collect(),
        initial_volume_enabled: window.get_audio_initial_volume_enabled(),
        initial_volume: number(
            window.get_audio_initial_volume_text().as_str(),
            "Initial volume",
        )?,
        replay_gain: selected(
            &replay_gain,
            window.get_audio_replay_gain_index(),
            "ReplayGain",
        )?,
        replay_gain_preamp: number(
            window.get_audio_replay_gain_preamp_text().as_str(),
            "ReplayGain preamp",
        )?,
        replay_gain_clip: window.get_audio_replay_gain_clip(),
        replay_gain_fallback: number(
            window.get_audio_replay_gain_fallback_text().as_str(),
            "ReplayGain fallback",
        )?,
        gapless: selected(
            &gapless,
            window.get_audio_gapless_index(),
            "gapless playback",
        )?,
        preferred_languages: window.get_audio_preferred_languages().to_string(),
    };
    // A device ID is tied to its output driver. If the selected driver changed
    // while Settings was open, don't persist a stale ID that can prevent audio
    // initialization on the next launch; the user can choose its device after
    // the newly selected driver has started.
    if !device_matches_driver(driver, &preferences.device) {
        preferences.device = "auto".into();
        preferences.device_description = "Autoselect device".into();
        window.set_audio_device_index(0);
    }
    preferences.validate().map_err(str::to_owned)?;
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
    let drivers = Driver::choices_for_current_platform();
    let replay_gain = [ReplayGain::Off, ReplayGain::Track, ReplayGain::Album];
    let gapless = [Gapless::Disabled, Gapless::Weak, Gapless::Strong];
    window.set_audio_threads_text(preferences.audio_threads.to_string().into());
    window.set_audio_driver_index(
        drivers
            .iter()
            .position(|driver| *driver == preferences.driver)
            .unwrap_or(0) as i32,
    );
    window.set_audio_driver_options(slint::ModelRc::new(slint::VecModel::from(
        drivers
            .iter()
            .map(|driver| driver.label().into())
            .collect::<Vec<slint::SharedString>>(),
    )));
    install_devices(window, preferences, available_devices);
    window.set_audio_spdif_ac3(preferences.spdif.contains(&SpdifCodec::Ac3));
    window.set_audio_spdif_dts(preferences.spdif.contains(&SpdifCodec::Dts));
    window.set_audio_spdif_dts_hd(preferences.spdif.contains(&SpdifCodec::DtsHd));
    window.set_audio_spdif_eac3(preferences.spdif.contains(&SpdifCodec::Eac3));
    window.set_audio_spdif_truehd(preferences.spdif.contains(&SpdifCodec::TrueHd));
    window.set_audio_initial_volume_enabled(preferences.initial_volume_enabled);
    window.set_audio_initial_volume_text(preferences.initial_volume.to_string().into());
    window.set_audio_replay_gain_index(
        replay_gain
            .iter()
            .position(|value| *value == preferences.replay_gain)
            .unwrap_or(0) as i32,
    );
    window.set_audio_replay_gain_options(slint::ModelRc::new(slint::VecModel::from(
        ["Off", "Track", "Album"]
            .into_iter()
            .map(Into::into)
            .collect::<Vec<slint::SharedString>>(),
    )));
    window.set_audio_replay_gain_preamp_text(preferences.replay_gain_preamp.to_string().into());
    window.set_audio_replay_gain_clip(preferences.replay_gain_clip);
    window.set_audio_replay_gain_fallback_text(preferences.replay_gain_fallback.to_string().into());
    window.set_audio_gapless_index(
        gapless
            .iter()
            .position(|value| *value == preferences.gapless)
            .unwrap_or(1) as i32,
    );
    window.set_audio_gapless_options(slint::ModelRc::new(slint::VecModel::from(
        ["Disabled", "Weak", "Strong"]
            .into_iter()
            .map(Into::into)
            .collect::<Vec<slint::SharedString>>(),
    )));
    window.set_audio_preferred_languages(preferences.preferred_languages.clone().into());
}

pub(crate) fn install_devices(
    window: &crate::PreferencesWindow,
    preferences: &Settings,
    available_devices: &[OutputDevice],
) {
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
    window.set_audio_device_index(selected_device as i32);
    window.set_audio_device_names(slint::ModelRc::new(slint::VecModel::from(
        devices
            .iter()
            .map(|(name, _)| name.clone().into())
            .collect::<Vec<slint::SharedString>>(),
    )));
    window.set_audio_device_descriptions(slint::ModelRc::new(slint::VecModel::from(
        devices
            .iter()
            .map(|(_, description)| description.clone().into())
            .collect::<Vec<slint::SharedString>>(),
    )));
    window.set_audio_device_options(slint::ModelRc::new(slint::VecModel::from(
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
