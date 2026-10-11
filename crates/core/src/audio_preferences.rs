//! Shared audio preferences that map to documented libmpv options.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub audio_threads: u8,
    pub driver: Driver,
    pub device: String,
    pub device_description: String,
    pub spdif: Vec<SpdifCodec>,
    pub initial_volume_enabled: bool,
    pub initial_volume: u16,
    pub replay_gain: ReplayGain,
    pub replay_gain_preamp: f64,
    pub replay_gain_clip: bool,
    pub replay_gain_fallback: f64,
    pub gapless: Gapless,
    pub preferred_languages: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            audio_threads: 0,
            driver: Driver::platform_default(),
            device: "auto".into(),
            device_description: "Autoselect device".into(),
            spdif: Vec::new(),
            initial_volume_enabled: false,
            initial_volume: 100,
            replay_gain: ReplayGain::default(),
            replay_gain_preamp: 0.0,
            replay_gain_clip: false,
            replay_gain_fallback: 0.0,
            gapless: Gapless::default(),
            preferred_languages: String::new(),
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.audio_threads > 16 {
            return Err("Audio threads must be between 0 and 16");
        }
        if self.initial_volume > 1000 {
            return Err("Initial volume must be between 0 and 1000");
        }
        if !self.replay_gain_preamp.is_finite()
            || !(-15.0..=15.0).contains(&self.replay_gain_preamp)
        {
            return Err("ReplayGain preamp must be between -15 and 15 dB");
        }
        if !self.replay_gain_fallback.is_finite()
            || !(-20.0..=6.0).contains(&self.replay_gain_fallback)
        {
            return Err("ReplayGain fallback must be between -20 and 6 dB");
        }
        if self.device.contains('\0')
            || self.device_description.contains('\0')
            || self.preferred_languages.contains('\0')
        {
            return Err("Audio preferences cannot contain a NUL character");
        }
        Ok(())
    }

    /// Options are validated before reaching libmpv so invalid fields cannot
    /// leave a partially configured player during startup.
    pub fn engine_options(&self) -> Result<Vec<(String, String)>, String> {
        self.validate().map_err(str::to_owned)?;
        let mut options = vec![
            ("ad-lavc-threads".into(), self.audio_threads.to_string()),
            ("alang".into(), self.preferred_languages.clone()),
            ("audio-spdif".into(), self.spdif_value()),
            ("replaygain".into(), self.replay_gain.native_value().into()),
            (
                "replaygain-preamp".into(),
                self.replay_gain_preamp.to_string(),
            ),
            (
                "replaygain-clip".into(),
                if self.replay_gain_clip { "yes" } else { "no" }.into(),
            ),
            (
                "replaygain-fallback".into(),
                self.replay_gain_fallback.to_string(),
            ),
            ("gapless-audio".into(), self.gapless.native_value().into()),
        ];
        if self.driver != Driver::Default {
            options.push(("ao".into(), self.driver.native_value().into()));
        }
        if self.device != "auto" {
            options.push(("audio-device".into(), self.device.clone()));
        }
        if self.initial_volume_enabled {
            options.push(("volume".into(), self.initial_volume.to_string()));
        }
        Ok(options)
    }

    pub fn spdif_value(&self) -> String {
        SpdifCodec::ALL
            .into_iter()
            .filter(|codec| self.spdif.contains(codec))
            .map(SpdifCodec::native_value)
            .collect::<Vec<_>>()
            .join(",")
    }
}

/// A playback-engine output device, keyed by the native ID mpv accepts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputDevice {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Driver {
    #[default]
    Default,
    CoreAudio,
    AvFoundation,
    Wasapi,
    DirectSound,
    Winmm,
    PipeWire,
    PulseAudio,
    Alsa,
    Jack,
    Sdl,
}

impl Driver {
    pub const fn platform_default() -> Self {
        #[cfg(target_os = "macos")]
        {
            Self::CoreAudio
        }
        #[cfg(not(target_os = "macos"))]
        {
            Self::Default
        }
    }

    pub const fn native_value(self) -> &'static str {
        match self {
            Self::Default => "auto",
            Self::CoreAudio => "coreaudio",
            Self::AvFoundation => "avfoundation",
            Self::Wasapi => "wasapi",
            Self::DirectSound => "dsound",
            Self::Winmm => "winmm",
            Self::PipeWire => "pipewire",
            Self::PulseAudio => "pulse",
            Self::Alsa => "alsa",
            Self::Jack => "jack",
            Self::Sdl => "sdl",
        }
    }

    pub const fn choices_for_current_platform() -> &'static [Self] {
        #[cfg(target_os = "windows")]
        {
            &[Self::Default, Self::Wasapi, Self::DirectSound, Self::Winmm]
        }
        #[cfg(target_os = "macos")]
        {
            &[Self::Default, Self::CoreAudio, Self::AvFoundation]
        }
        #[cfg(target_os = "linux")]
        {
            &[
                Self::Default,
                Self::PipeWire,
                Self::PulseAudio,
                Self::Alsa,
                Self::Jack,
                Self::Sdl,
            ]
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        {
            &[Self::Default]
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Default => "System Default",
            Self::CoreAudio => "Core Audio",
            Self::AvFoundation => "AVFoundation",
            Self::Wasapi => "WASAPI",
            Self::DirectSound => "DirectSound",
            Self::Winmm => "WinMM",
            Self::PipeWire => "PipeWire",
            Self::PulseAudio => "PulseAudio",
            Self::Alsa => "ALSA",
            Self::Jack => "JACK",
            Self::Sdl => "SDL",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpdifCodec {
    Ac3,
    Dts,
    DtsHd,
    Eac3,
    TrueHd,
}

impl SpdifCodec {
    const ALL: [Self; 5] = [Self::Ac3, Self::Dts, Self::DtsHd, Self::Eac3, Self::TrueHd];

    pub const fn native_value(self) -> &'static str {
        match self {
            Self::Ac3 => "ac3",
            Self::Dts => "dts",
            Self::DtsHd => "dts-hd",
            Self::Eac3 => "eac3",
            Self::TrueHd => "truehd",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayGain {
    #[default]
    Off,
    Track,
    Album,
}

impl ReplayGain {
    pub const fn native_value(self) -> &'static str {
        match self {
            Self::Off => "no",
            Self::Track => "track",
            Self::Album => "album",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Gapless {
    Disabled,
    #[default]
    Weak,
    Strong,
}

impl Gapless {
    pub const fn native_value(self) -> &'static str {
        match self {
            Self::Disabled => "no",
            Self::Weak => "weak",
            Self::Strong => "yes",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_pinned_audio_behavior() {
        let settings = Settings::default();
        assert_eq!(settings.audio_threads, 0);
        assert_eq!(settings.driver, Driver::platform_default());
        assert_eq!(settings.device, "auto");
        assert_eq!(settings.device_description, "Autoselect device");
        assert_eq!(settings.initial_volume, 100);
        assert!(!settings.initial_volume_enabled);
        assert_eq!(settings.replay_gain, ReplayGain::Off);
        assert_eq!(settings.gapless, Gapless::Weak);
        assert_eq!(settings.spdif_value(), "");
    }

    #[test]
    fn options_preserve_source_order_and_typed_values() {
        let settings = Settings {
            audio_threads: 4,
            driver: Driver::Wasapi,
            device: "wasapi/{0.0.0.00000000}.{device}".into(),
            spdif: vec![SpdifCodec::Dts, SpdifCodec::Ac3, SpdifCodec::TrueHd],
            replay_gain: ReplayGain::Album,
            replay_gain_preamp: -1.5,
            replay_gain_clip: true,
            replay_gain_fallback: -6.0,
            gapless: Gapless::Strong,
            preferred_languages: "eng,en".into(),
            ..Settings::default()
        };
        let options = settings.engine_options().unwrap();
        assert_eq!(options[0], ("ad-lavc-threads".into(), "4".into()));
        assert_eq!(options[1], ("alang".into(), "eng,en".into()));
        assert_eq!(options[2], ("audio-spdif".into(), "ac3,dts,truehd".into()));
        assert_eq!(options[3], ("replaygain".into(), "album".into()));
        assert_eq!(
            options.last().unwrap(),
            &("audio-device".into(), settings.device)
        );
    }

    #[test]
    fn initial_volume_is_only_sent_when_enabled() {
        let defaults = Settings::default().engine_options().unwrap();
        assert!(!defaults.iter().any(|(name, _)| name == "volume"));

        let enabled = Settings {
            initial_volume_enabled: true,
            initial_volume: 125,
            ..Settings::default()
        }
        .engine_options()
        .unwrap();
        assert!(enabled.contains(&("volume".into(), "125".into())));
    }

    #[test]
    fn selected_device_identity_and_description_survive_settings_round_trip() {
        let selected = Settings {
            device: "wasapi/{device-id}".into(),
            device_description: "Living room speakers".into(),
            ..Settings::default()
        };
        let restored: Settings =
            serde_json::from_str(&serde_json::to_string(&selected).unwrap()).unwrap();
        assert_eq!(restored.device, selected.device);
        assert_eq!(restored.device_description, selected.device_description);
    }

    #[test]
    fn invalid_audio_preferences_are_rejected_before_startup() {
        for invalid in [
            Settings {
                audio_threads: 17,
                ..Settings::default()
            },
            Settings {
                initial_volume: 1001,
                ..Settings::default()
            },
            Settings {
                replay_gain_preamp: 15.01,
                ..Settings::default()
            },
            Settings {
                replay_gain_fallback: f64::NAN,
                ..Settings::default()
            },
            Settings {
                device: "bad\0device".into(),
                ..Settings::default()
            },
        ] {
            assert!(invalid.engine_options().is_err());
        }
    }
}
