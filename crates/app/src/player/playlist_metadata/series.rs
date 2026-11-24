//! Fold the common filename prefix of each series discovered in a media folder.

use crate::series_names::group_series_names;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use unicode_segmentation::UnicodeSegmentation;

pub(crate) type Cache = Arc<Mutex<Option<Arc<Series>>>>;

pub(crate) struct Series {
    prefixes: HashMap<PathBuf, String>,
}

impl Series {
    pub(crate) fn from_files(files: &[String]) -> Self {
        let candidates: Vec<_> = files
            .iter()
            .filter_map(|file| {
                let path = Path::new(file);
                let stem = path.file_stem()?.to_str()?;
                Some((std::path::absolute(path).ok()?, stem.to_owned()))
            })
            .collect();
        let names: Vec<_> = candidates.iter().map(|(_, stem)| stem.clone()).collect();
        let mut prefixes = HashMap::new();
        for group in group_series_names(&names) {
            if group.members.len() < 3 || group.prefix.graphemes(true).count() < 7 {
                continue;
            }
            for index in group.members {
                let (path, stem) = &candidates[index];
                if stem.len() > group.prefix.len() {
                    prefixes.insert(path.clone(), group.prefix.clone());
                }
            }
        }
        Self { prefixes }
    }

    pub(crate) fn split_title<'a>(
        &'a self,
        filename: &str,
        label: &'a str,
    ) -> Option<(&'a str, &'a str)> {
        let path = crate::file_actions::local_media_path(filename)?;
        let prefix = self
            .prefixes
            .get(&std::path::absolute(path).ok()?)?
            .as_str();
        let title = label.strip_prefix(prefix)?;
        let stem = Path::new(label).file_stem()?.to_str()?;
        (label.graphemes(true).count() > 12 && stem.len() > prefix.len()).then_some((prefix, title))
    }
}
