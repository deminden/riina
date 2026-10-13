//! OpenSubtitles behavior preferences shared by the application frontends.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Provider {
    #[default]
    OpenSubtitles,
    Assrt,
    Shooter,
}

/// Stable multi-selection behavior for the online subtitle result list.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResultSelection {
    selected: BTreeSet<usize>,
    anchor: Option<usize>,
}

impl ResultSelection {
    pub fn click(&mut self, index: usize, count: usize, extend: bool, range: bool) {
        if index >= count {
            return;
        }
        if range {
            let anchor = *self.anchor.get_or_insert(index);
            if !extend {
                self.selected.clear();
            }
            for selected in anchor.min(index)..=anchor.max(index) {
                self.selected.insert(selected);
            }
        } else if extend {
            if !self.selected.remove(&index) {
                self.selected.insert(index);
            }
            self.anchor = Some(index);
        } else {
            self.selected.clear();
            self.selected.insert(index);
            self.anchor = Some(index);
        }
    }

    pub fn select_all(&mut self, count: usize) {
        self.selected = (0..count).collect();
        self.anchor = (count > 0).then_some(0);
    }

    pub fn clear(&mut self) {
        self.selected.clear();
        self.anchor = None;
    }

    pub fn indices(&self) -> Vec<usize> {
        self.selected.iter().copied().collect()
    }

    pub fn contains(&self, index: usize) -> bool {
        self.selected.contains(&index)
    }

    pub fn len(&self) -> usize {
        self.selected.len()
    }

    pub fn is_empty(&self) -> bool {
        self.selected.is_empty()
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Preferences {
    #[serde(default)]
    pub provider: Provider,
    /// Loaded from the platform credential store and never written to preferences JSON.
    #[serde(default, skip_serializing, deserialize_with = "discard_legacy_api_key")]
    pub api_key: String,
    /// Loaded from the platform credential store and never written to preferences JSON.
    #[serde(default, skip_serializing, deserialize_with = "discard_legacy_api_key")]
    pub assrt_token: String,
    pub auto_search: bool,
    pub auto_search_threshold_minutes: u16,
}

fn discard_legacy_api_key<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    serde::de::IgnoredAny::deserialize(deserializer).map(|_| String::new())
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            provider: Provider::OpenSubtitles,
            api_key: String::new(),
            assrt_token: String::new(),
            auto_search: false,
            auto_search_threshold_minutes: 20,
        }
    }
}

impl std::fmt::Debug for Preferences {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OnlineSubtitlePreferences")
            .field(
                "api_key",
                &if self.api_key.is_empty() {
                    "empty"
                } else {
                    "<redacted>"
                },
            )
            .field(
                "assrt_token",
                &if self.assrt_token.is_empty() {
                    "empty"
                } else {
                    "<redacted>"
                },
            )
            .field("provider", &self.provider)
            .field("auto_search", &self.auto_search)
            .field(
                "auto_search_threshold_minutes",
                &self.auto_search_threshold_minutes,
            )
            .finish()
    }
}

impl Preferences {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.api_key.len() > 512 || self.api_key.chars().any(char::is_control) {
            return Err("OpenSubtitles API key contains invalid text");
        }
        if self.assrt_token.len() > 512 || self.assrt_token.chars().any(char::is_control) {
            return Err("Assrt token contains invalid text");
        }
        if !(1..=1440).contains(&self.auto_search_threshold_minutes) {
            return Err("Online subtitle search threshold must be between 1 and 1440 minutes");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{Preferences, Provider, ResultSelection};

    #[test]
    fn result_selection_replaces_and_command_click_toggles_rows() {
        let mut selection = ResultSelection::default();
        selection.click(1, 5, false, false);
        assert_eq!(selection.indices(), vec![1]);
        selection.click(3, 5, true, false);
        assert_eq!(selection.indices(), vec![1, 3]);
        selection.click(1, 5, true, false);
        assert_eq!(selection.indices(), vec![3]);
        selection.click(4, 5, false, false);
        assert_eq!(selection.indices(), vec![4]);
    }

    #[test]
    fn shift_selection_uses_the_last_selection_anchor_and_can_extend() {
        let mut selection = ResultSelection::default();
        selection.click(2, 6, false, false);
        selection.click(5, 6, false, true);
        assert_eq!(selection.indices(), vec![2, 3, 4, 5]);
        selection.click(0, 6, true, true);
        assert_eq!(selection.indices(), vec![0, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn result_selection_select_all_and_clear_are_bounded() {
        let mut selection = ResultSelection::default();
        selection.select_all(4);
        assert_eq!(selection.indices(), vec![0, 1, 2, 3]);
        selection.click(8, 4, true, false);
        assert_eq!(selection.len(), 4);
        selection.clear();
        assert!(selection.is_empty());
    }

    #[test]
    fn api_key_is_never_serialized_with_preferences() {
        let preferences = Preferences {
            provider: Provider::OpenSubtitles,
            api_key: "private-test-key".into(),
            assrt_token: "assrt-private-test-token".into(),
            auto_search: true,
            auto_search_threshold_minutes: 25,
        };
        let value = serde_json::to_value(&preferences).unwrap();
        assert_eq!(value["api_key"], serde_json::Value::Null);
        assert_eq!(value["assrt_token"], serde_json::Value::Null);
        assert_eq!(value["provider"], "open-subtitles");
        assert_eq!(value["auto_search"], true);
        assert_eq!(value["auto_search_threshold_minutes"], 25);
        assert!(!format!("{preferences:?}").contains("private-test-key"));
        assert!(!format!("{preferences:?}").contains("assrt-private-test-token"));
    }

    #[test]
    fn provider_defaults_to_opensubtitles_and_round_trips() {
        let default = Preferences::default();
        assert_eq!(default.provider, Provider::OpenSubtitles);
        let preferences = Preferences {
            provider: Provider::Assrt,
            ..Preferences::default()
        };
        let encoded = serde_json::to_string(&preferences).unwrap();
        assert!(encoded.contains("\"provider\":\"assrt\""));
        assert_eq!(
            serde_json::from_str::<Preferences>(&encoded)
                .unwrap()
                .provider,
            Provider::Assrt
        );
        let preferences = Preferences {
            provider: Provider::Shooter,
            ..Preferences::default()
        };
        let encoded = serde_json::to_string(&preferences).unwrap();
        assert!(encoded.contains("\"provider\":\"shooter\""));
        assert_eq!(
            serde_json::from_str::<Preferences>(&encoded)
                .unwrap()
                .provider,
            Provider::Shooter
        );
    }

    #[test]
    fn legacy_plain_text_key_is_ignored_during_preferences_parse() {
        let preferences: crate::Preferences = serde_json::from_str(
            r#"{"online_subtitles":{"api_key":"legacy-secret","auto_search":true}}"#,
        )
        .unwrap();
        assert!(preferences.online_subtitles.api_key.is_empty());
        assert!(preferences.online_subtitles.auto_search);
    }
}
