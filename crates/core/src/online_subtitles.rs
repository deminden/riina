//! OpenSubtitles behavior preferences shared by the application frontends.

use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Preferences {
    /// Loaded from the platform credential store and never written to preferences JSON.
    #[serde(default, skip_serializing, deserialize_with = "discard_legacy_api_key")]
    pub api_key: String,
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
            api_key: String::new(),
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
        if !(1..=1440).contains(&self.auto_search_threshold_minutes) {
            return Err("Online subtitle search threshold must be between 1 and 1440 minutes");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Preferences;

    #[test]
    fn api_key_is_never_serialized_with_preferences() {
        let preferences = Preferences {
            api_key: "private-test-key".into(),
            auto_search: true,
            auto_search_threshold_minutes: 25,
        };
        let value = serde_json::to_value(&preferences).unwrap();
        assert_eq!(value["api_key"], serde_json::Value::Null);
        assert_eq!(value["auto_search"], true);
        assert_eq!(value["auto_search_threshold_minutes"], 25);
        assert!(!format!("{preferences:?}").contains("private-test-key"));
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
