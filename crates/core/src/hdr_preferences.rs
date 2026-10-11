//! Shared HDR tone-mapping preferences mapped to libmpv's documented options.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Algorithm {
    Auto,
    Clip,
    Mobius,
    Reinhard,
    Hable,
    Bt2390,
    Gamma,
    Linear,
}

impl Algorithm {
    pub const ALL: [Self; 8] = [
        Self::Auto,
        Self::Clip,
        Self::Mobius,
        Self::Reinhard,
        Self::Hable,
        Self::Bt2390,
        Self::Gamma,
        Self::Linear,
    ];

    pub const fn native_value(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Clip => "clip",
            Self::Mobius => "mobius",
            Self::Reinhard => "reinhard",
            Self::Hable => "hable",
            Self::Bt2390 => "bt.2390",
            Self::Gamma => "gamma",
            Self::Linear => "linear",
        }
    }

    pub const fn index(self) -> i32 {
        match self {
            Self::Auto => 0,
            Self::Clip => 1,
            Self::Mobius => 2,
            Self::Reinhard => 3,
            Self::Hable => 4,
            Self::Bt2390 => 5,
            Self::Gamma => 6,
            Self::Linear => 7,
        }
    }

    pub const fn from_index(index: i32) -> Option<Self> {
        match index {
            0 => Some(Self::Auto),
            1 => Some(Self::Clip),
            2 => Some(Self::Mobius),
            3 => Some(Self::Reinhard),
            4 => Some(Self::Hable),
            5 => Some(Self::Bt2390),
            6 => Some(Self::Gamma),
            7 => Some(Self::Linear),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub enabled: bool,
    pub target_peak_override_enabled: bool,
    pub target_peak_nits: u32,
    pub algorithm: Algorithm,
    pub param_override_enabled: bool,
    pub param: f64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: true,
            target_peak_override_enabled: false,
            target_peak_nits: 400,
            algorithm: Algorithm::Auto,
            param_override_enabled: false,
            param: 1.0,
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.target_peak_override_enabled && !(10..=10_000).contains(&self.target_peak_nits) {
            return Err("Tone-mapping target peak must be between 10 and 10,000 nits");
        }
        if !self.param.is_finite() {
            return Err("Tone-mapping parameter must be a finite number");
        }
        Ok(())
    }

    /// No explicit options lets libmpv keep its own defaults, matching IINA's reset path.
    pub fn engine_options(&self) -> Result<Vec<(String, String)>, String> {
        self.validate().map_err(str::to_owned)?;
        if !self.enabled {
            return Ok(Vec::new());
        }
        Ok(vec![
            (
                "target-peak".into(),
                if self.target_peak_override_enabled {
                    self.target_peak_nits.to_string()
                } else {
                    "auto".into()
                },
            ),
            ("tone-mapping".into(), self.algorithm.native_value().into()),
            (
                "tone-mapping-param".into(),
                if self.param_override_enabled {
                    self.param.to_string()
                } else {
                    "default".into()
                },
            ),
        ])
    }

    pub fn engine_commands(&self) -> Result<Vec<Vec<String>>, String> {
        let options = self.engine_options()?;
        if !self.enabled {
            // IINA resets renderer options when disabled; runtime changes must do
            // the same instead of leaving the previous video's override active.
            return Ok([
                vec!["set".into(), "target-peak".into(), "auto".into()],
                vec!["set".into(), "tone-mapping".into(), "auto".into()],
                vec!["set".into(), "tone-mapping-param".into(), "default".into()],
            ]
            .into());
        }
        Ok(options
            .into_iter()
            .map(|(name, value)| vec!["set".into(), name, value])
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::{Algorithm, Settings};

    #[test]
    fn defaults_match_the_pinned_codec_preferences() {
        let settings = Settings::default();
        assert!(settings.enabled);
        assert!(!settings.target_peak_override_enabled);
        assert_eq!(settings.target_peak_nits, 400);
        assert_eq!(settings.algorithm, Algorithm::Auto);
        assert!(!settings.param_override_enabled);
        assert_eq!(settings.param, 1.0);
        assert_eq!(
            settings.engine_options().unwrap(),
            [
                ("target-peak".into(), "auto".into()),
                ("tone-mapping".into(), "auto".into()),
                ("tone-mapping-param".into(), "default".into()),
            ]
        );
    }

    #[test]
    fn selected_overrides_map_to_native_options_and_commands() {
        let settings = Settings {
            target_peak_override_enabled: true,
            target_peak_nits: 800,
            algorithm: Algorithm::Bt2390,
            param_override_enabled: true,
            param: 0.75,
            ..Settings::default()
        };
        assert_eq!(
            settings.engine_options().unwrap(),
            [
                ("target-peak".into(), "800".into()),
                ("tone-mapping".into(), "bt.2390".into()),
                ("tone-mapping-param".into(), "0.75".into()),
            ]
        );
        assert_eq!(
            settings.engine_commands().unwrap()[1],
            ["set", "tone-mapping", "bt.2390"]
        );
    }

    #[test]
    fn disabling_tone_mapping_resets_each_live_renderer_option() {
        let settings = Settings {
            enabled: false,
            ..Settings::default()
        };
        assert!(settings.engine_options().unwrap().is_empty());
        assert_eq!(
            settings.engine_commands().unwrap(),
            [
                ["set", "target-peak", "auto"],
                ["set", "tone-mapping", "auto"],
                ["set", "tone-mapping-param", "default"],
            ]
        );
    }

    #[test]
    fn algorithm_indexes_and_native_names_are_stable() {
        for (index, algorithm) in Algorithm::ALL.into_iter().enumerate() {
            assert_eq!(Algorithm::from_index(index as i32), Some(algorithm));
            assert_eq!(algorithm.index(), index as i32);
        }
        assert_eq!(Algorithm::Bt2390.native_value(), "bt.2390");
        assert_eq!(Algorithm::from_index(-1), None);
        assert_eq!(Algorithm::from_index(8), None);
    }

    #[test]
    fn invalid_overrides_are_rejected_before_native_dispatch() {
        for peak in [9, 10_001] {
            assert!(
                Settings {
                    target_peak_override_enabled: true,
                    target_peak_nits: peak,
                    ..Settings::default()
                }
                .engine_options()
                .is_err()
            );
        }
        for param in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(
                Settings {
                    param_override_enabled: true,
                    param,
                    ..Settings::default()
                }
                .engine_options()
                .is_err()
            );
        }
    }

    #[test]
    fn finite_algorithm_parameters_are_not_artificially_range_limited() {
        for param in [-100.0, -0.25, 100.1, 1_000_000.0] {
            assert!(
                Settings {
                    param_override_enabled: true,
                    param,
                    ..Settings::default()
                }
                .engine_options()
                .is_ok()
            );
        }
    }
}
