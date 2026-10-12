//! Persistent playback-history entries, separate from the bounded resume list.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub path: String,
    /// Unix epoch milliseconds, matching the JavaScript `Date` value exposed to plugins.
    pub date_ms: u64,
    pub progress: Option<f64>,
    pub duration: f64,
}

impl Entry {
    pub fn is_valid(&self) -> bool {
        !self.path.trim().is_empty()
            && !self.path.contains('\0')
            && self.date_ms <= 8_640_000_000_000_000
            && self.duration.is_finite()
            && self.duration >= 0.0
            && self
                .progress
                .is_none_or(|progress| progress.is_finite() && progress >= 0.0)
    }
}

pub fn move_to_front(entries: &mut Vec<Entry>, entry: Entry) {
    entries.retain(|existing| existing.path != entry.path);
    entries.insert(0, entry);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_is_persistent_and_keeps_one_recent_entry_per_media_path() {
        let first = Entry {
            path: "clip.mkv".into(),
            date_ms: 10,
            progress: Some(3.5),
            duration: 12.0,
        };
        let second = Entry {
            path: "other.mkv".into(),
            date_ms: 11,
            progress: None,
            duration: 0.0,
        };
        let reopened = Entry {
            date_ms: 20,
            progress: Some(6.0),
            ..first.clone()
        };
        let mut history = vec![first, second.clone()];
        move_to_front(&mut history, reopened.clone());

        assert_eq!(history, [reopened, second]);
        let encoded = serde_json::to_string(&history).unwrap();
        assert_eq!(
            serde_json::from_str::<Vec<Entry>>(&encoded).unwrap(),
            history
        );
    }

    #[test]
    fn invalid_history_values_are_rejected() {
        for (path, progress, duration) in [
            ("", None, 1.0),
            ("clip.mkv", Some(f64::NAN), 1.0),
            ("clip.mkv", Some(-1.0), 1.0),
            ("clip.mkv", None, f64::INFINITY),
            ("clip.mkv", None, -1.0),
        ] {
            let entry = Entry {
                path: path.into(),
                date_ms: 1,
                progress,
                duration,
            };
            assert!(!entry.is_valid());
        }
    }
}
