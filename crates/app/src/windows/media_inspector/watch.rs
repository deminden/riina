//! Persist watch names independently of the current media or the selected tab.
use riina_core::Preferences;
use std::{collections::BTreeSet, path::Path};

pub(super) fn load(path: &Path) -> Result<Vec<String>, String> {
    Ok(crate::preference_load::read_valid(path)
        .map_err(|error| error.to_string())?
        .unwrap_or_default()
        .inspector_watch_properties)
}

pub(super) fn add(path: &Path, name: &str) -> Result<Vec<String>, String> {
    let name = name.trim();
    if name.is_empty() || name.contains('\0') {
        return Err("Enter an mpv property name".into());
    }
    update(path, |current| {
        current.push(name.into());
        Ok(())
    })
}

pub(super) fn remove(
    path: &Path,
    displayed: &[String],
    selected: &[usize],
) -> Result<Vec<String>, String> {
    update(path, |current| {
        if *current != displayed {
            return Err("The watch list changed. Refresh it before removing properties.".into());
        }
        let indexes = selected.iter().copied().collect::<BTreeSet<_>>();
        for index in indexes.into_iter().rev() {
            if index < current.len() {
                current.remove(index);
            }
        }
        Ok(())
    })
}

fn update(
    path: &Path,
    edit: impl FnOnce(&mut Vec<String>) -> Result<(), String>,
) -> Result<Vec<String>, String> {
    let mut result = Vec::new();
    crate::preference_store::update(path, &Preferences::default(), |preferences| {
        edit(&mut preferences.inspector_watch_properties)?;
        result.clone_from(&preferences.inspector_watch_properties);
        Ok(())
    })?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use riina_test_support::TempDir;

    #[test]
    fn watch_edits_keep_saved_settings_and_reject_stale_deletions()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = TempDir::new("inspector-watch")?;
        let path = directory.path().join("preferences.json");
        std::fs::write(&path, r#"{"volume":73.0}"#)?;
        assert!(load(&path)?.is_empty());

        let first = add(&path, " video-bitrate ")?;
        assert_eq!(first, vec!["video-bitrate"]);
        let second = add(&path, "avsync")?;
        assert!(remove(&path, &first, &[0]).is_err());
        assert_eq!(load(&path)?, second);
        let edited = add(&path, "video-bitrate")?;
        assert_eq!(remove(&path, &edited, &[2, 0, 2])?, vec!["avsync"]);
        assert!(add(&path, "").is_err());
        let saved = crate::preference_load::read_valid(&path)?.ok_or("Missing settings")?;
        assert_eq!(saved.volume, 73.0);
        assert_eq!(saved.inspector_watch_properties, vec!["avsync"]);
        Ok(())
    }
}
