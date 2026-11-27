//! Persistent playback-history entries, separate from the bounded resume list.

use serde::{Deserialize, Deserializer, Serialize, de::IgnoredAny};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// The resume filename chosen by mpv's policy when this media was played.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub watch_later_key: Option<WatchLaterKey>,
    /// Unix epoch milliseconds, matching the JavaScript `Date` value exposed to plugins.
    pub date_ms: u64,
    /// Live position read from mpv's watch-later file.
    #[serde(default, skip_serializing, deserialize_with = "discard_saved_progress")]
    pub progress: Option<f64>,
    pub duration: f64,
}

fn discard_saved_progress<'de, D: Deserializer<'de>>(input: D) -> Result<Option<f64>, D::Error> {
    // Keep the history rows when an archive contains cached positions.
    IgnoredAny::deserialize(input)?;
    Ok(None)
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String")]
pub struct WatchLaterKey(String);

impl WatchLaterKey {
    pub fn from_digest(digest: [u8; 16]) -> Self {
        const HEX: &[u8; 16] = b"0123456789ABCDEF";
        let mut key = String::with_capacity(32);
        for byte in digest {
            key.push(char::from(HEX[usize::from(byte >> 4)]));
            key.push(char::from(HEX[usize::from(byte & 0xf)]));
        }
        Self(key)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for WatchLaterKey {
    type Error = &'static str;

    fn try_from(mut value: String) -> Result<Self, Self::Error> {
        if value.len() != 32 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("invalid watch-later key");
        }
        value.make_ascii_uppercase();
        Ok(Self(value))
    }
}

impl Entry {
    pub fn is_valid(&self) -> bool {
        !self.path.trim().is_empty()
            && !self.path.contains('\0')
            && self.date_ms <= 8_640_000_000_000_000
            && self.duration.is_finite()
            && self.duration >= 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_entries_save_metadata_and_resume_keys_without_caching_positions() {
        let first = Entry {
            path: "clip.mkv".into(),
            title: Some("A Clip".into()),
            watch_later_key: Some(WatchLaterKey::from_digest([0xab; 16])),
            date_ms: 10,
            progress: Some(3.5),
            duration: 12.0,
        };
        let second = Entry {
            path: "other.mkv".into(),
            title: None,
            watch_later_key: None,
            date_ms: 11,
            progress: None,
            duration: 0.0,
        };
        let mut history = vec![first, second];
        let encoded = serde_json::to_string(&history).unwrap();
        assert!(!encoded.contains("\"progress\""));
        history[0].progress = None;
        assert_eq!(
            serde_json::from_str::<Vec<Entry>>(&encoded).unwrap(),
            history
        );
    }

    #[test]
    fn invalid_history_values_are_rejected() {
        for (path, duration) in [("", 1.0), ("clip.mkv", f64::INFINITY), ("clip.mkv", -1.0)] {
            let entry = Entry {
                path: path.into(),
                title: None,
                watch_later_key: None,
                date_ms: 1,
                progress: None,
                duration,
            };
            assert!(!entry.is_valid());
        }
    }
}
