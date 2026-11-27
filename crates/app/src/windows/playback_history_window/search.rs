use crate::PlaybackHistoryWindow;
use icu_casemap::CaseMapper;
use icu_normalizer::DecomposingNormalizerBorrowed;
use icu_properties::{
    CodePointMapData, CodePointSetData,
    props::{Diacritic, GeneralCategory, GeneralCategoryGroup},
};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
};

pub(super) struct Query {
    key: String,
}

impl Query {
    pub(super) fn new(text: &str) -> Self {
        Self { key: fold(text) }
    }

    pub(super) fn matches(&self, text: &str) -> bool {
        self.key.is_empty() || fold(text).contains(&self.key)
    }

    pub(super) fn matches_path(&self, reference: &str, local: Option<&Path>) -> bool {
        if self.key.is_empty() {
            return true;
        }
        if let Some(local) = local {
            return self.matches(&local.to_string_lossy());
        }
        url::Url::parse(reference).is_ok_and(|url| {
            self.matches(&percent_encoding::percent_decode_str(url.path()).decode_utf8_lossy())
        })
    }
}

fn fold(text: &str) -> String {
    if text.is_ascii() {
        return text.to_ascii_lowercase();
    }
    let case = CaseMapper::new().fold_string(text);
    let diacritics = CodePointSetData::new::<Diacritic>();
    let categories = CodePointMapData::<GeneralCategory>::new();
    DecomposingNormalizerBorrowed::new_nfd()
        .normalize_iter(case.chars())
        .filter(|character| {
            !diacritics.contains(*character)
                || !GeneralCategoryGroup::Mark.contains(categories.get(*character))
        })
        .collect()
}

const RECENT_LIMIT: usize = 10;

struct Recents {
    path: PathBuf,
    queries: Vec<String>,
}

impl Recents {
    fn load(path: PathBuf) -> Self {
        let mut recents = Self {
            path,
            queries: Vec::new(),
        };
        let loaded = std::fs::read_to_string(&recents.path)
            .map_err(|error| error.to_string())
            .and_then(|contents| {
                serde_json::from_str::<Vec<String>>(&contents).map_err(|error| error.to_string())
            });
        if let Ok(queries) = loaded {
            for query in queries.into_iter().rev() {
                recents.remember(&query);
            }
        }
        recents
    }

    fn remember(&mut self, query: &str) -> bool {
        if query.is_empty() || self.queries.first().is_some_and(|first| first == query) {
            return false;
        }
        self.queries.retain(|previous| previous != query);
        self.queries.insert(0, query.to_owned());
        self.queries.truncate(RECENT_LIMIT);
        true
    }

    fn update(&self, window: &PlaybackHistoryWindow) {
        self.show_in(window);
        let result = serde_json::to_string(&self.queries)
            .map_err(std::io::Error::other)
            .and_then(|json| crate::platform::save_preferences(&self.path, &json));
        if let Err(error) = result {
            super::report(
                window,
                format!("Could not save recent History searches: {error}"),
            );
        }
    }

    fn show_in(&self, window: &PlaybackHistoryWindow) {
        let mut labels = vec![window.get_recent_searches_title()];
        if !self.queries.is_empty() {
            labels.extend(
                self.queries
                    .iter()
                    .map(|query| SharedString::from(query.as_str())),
            );
            labels.push(window.get_clear_recents_title());
        }
        window.set_recent_searches(ModelRc::new(VecModel::from(labels)));
    }
}

pub(super) fn install_recents(window: &PlaybackHistoryWindow, preferences: &Path) {
    let recents = Rc::new(RefCell::new(Recents::load(
        preferences.with_file_name("playback-history-search.json"),
    )));
    recents.borrow().show_in(window);
    let accepted = recents.clone();
    let weak = window.as_weak();
    window.on_search_accepted(move || {
        let Some(window) = weak.upgrade() else { return };
        let mut recents = accepted.borrow_mut();
        if recents.remember(window.get_search_text().as_str()) {
            recents.update(&window);
        }
    });
    let weak = window.as_weak();
    window.on_recent_search_selected(move |index| {
        let (Some(window), Ok(index)) = (weak.upgrade(), usize::try_from(index - 1)) else {
            return;
        };
        let mut recents = recents.borrow_mut();
        if let Some(query) = recents.queries.get(index).cloned() {
            window.set_search_text(query.as_str().into());
            if recents.remember(&query) {
                recents.update(&window);
            }
        } else if index == recents.queries.len() && !recents.queries.is_empty() {
            recents.queries.clear();
            recents.update(&window);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn searches_filenames_and_decoded_paths_case_and_accent_insensitively() {
        assert!(Query::new("CAFE").matches("Cafe\u{301}.mkv"));
        assert!(Query::new("STRASSE").matches("Straße.mp4"));
        assert!(!Query::new(" CAFE").matches("Cafe\u{301}.mkv"));
        assert!(Query::new("series d'ete").matches_path(
            "https://example.com/S%C3%A9ries%20d%27%C3%A9t%C3%A9/1.mkv",
            None
        ));
        assert!(Query::new("cafeteria").matches_path(
            "file:///Caf%C3%A9teria/one.mkv",
            Some(Path::new("/Caféteria/one.mkv"))
        ));
        for term in ["example.com", "token", "fragment", "%C3%A9"] {
            assert!(
                !Query::new(term)
                    .matches_path("https://example.com/Caf%C3%A9.mkv?token=abc#fragment", None)
            );
        }
        assert!(!Query::new("episode 2").matches("Episode 12"));
    }

    #[test]
    fn recent_searches_stay_bounded_and_promote_a_repeated_query() {
        let mut recents = Recents {
            path: PathBuf::new(),
            queries: Vec::new(),
        };
        for index in 0..12 {
            assert!(recents.remember(&format!("Title {index}")));
        }
        assert_eq!(recents.queries.len(), RECENT_LIMIT);
        assert_eq!(recents.queries.last().map(String::as_str), Some("Title 2"));
        assert!(recents.remember("Title 3"));
        assert_eq!(recents.queries[0], "Title 3");
        assert_eq!(recents.queries[1], "Title 11");
        assert!(!recents.remember("Title 3"));
        assert!(!recents.remember(""));
        assert!(recents.remember(" Title 3 "));
        assert_eq!(recents.queries[0], " Title 3 ");
    }
}
