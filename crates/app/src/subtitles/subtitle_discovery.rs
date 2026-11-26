//! Expand IINA-style subtitle search paths and find matching external files.
mod readiness;
pub(crate) use readiness::Readiness;

use riina_core::{
    Phase, Playback,
    subtitle_controls::{AutoLoadMode, AutoLoadPreferences},
};
use std::{
    cell::OnceCell,
    collections::{HashMap, HashSet},
    ffi::OsStr,
    path::{Path, PathBuf},
};
use unicode_segmentation::UnicodeSegmentation;

const MAX_SEARCH_DIRECTORIES: usize = 128;
const MAX_ENTRIES_PER_DIRECTORY: usize = 4096;
const MAX_CANDIDATE_SUBTITLES: usize = 8192;
const MAX_MATCHED_SUBTITLES: usize = 64;
const MAX_FUZZY_STEM_CHARACTERS: usize = 256;
const MAX_FUZZY_PAIRS: usize = 10_000;
const MEDIA_EXTENSIONS: &[&str] = &[
    "3g2", "3gp", "3gp2", "3gpp", "aac", "ac3", "a52", "aif", "aiff", "ape", "asf", "avi", "dff",
    "divx", "dv", "dsf", "f4a", "f4b", "f4p", "f4v", "flac", "flv", "m1v", "m2a", "m2p", "m2ts",
    "m2v", "m4a", "m4v", "mka", "mkv", "mov", "moov", "mp1", "mp2", "mp3", "mp4", "mpa", "mpeg",
    "mpeg1", "mpeg2", "mpeg4", "mpg", "mpg3", "mpv", "mts", "mt2s", "oga", "ogg", "opus", "ps",
    "qt", "ram", "rm", "rmvb", "ts", "wav", "webm", "wma", "wmv", "wv", "wtv", "xm",
];

#[derive(Default)]
pub(crate) struct AutoDiscovery {
    generation: Option<u64>,
    media_path: String,
    preferences: Option<AutoLoadPreferences>,
    explicit_playlist: bool,
}

impl AutoDiscovery {
    pub(crate) fn observe(
        &mut self,
        state: &Playback,
        preferences: &AutoLoadPreferences,
    ) -> Option<(u64, PathBuf)> {
        if preferences.mode == AutoLoadMode::Disabled {
            self.preferences = None;
            return None;
        }
        if !matches!(
            state.phase,
            Phase::Playing | Phase::Paused | Phase::Buffering | Phase::Seeking
        ) {
            return None;
        }
        let media_path = state
            .playlist
            .iter()
            .find(|entry| entry.current)
            .map(|entry| entry.filename.clone())?;
        let local_path = crate::file_actions::local_media_path(&media_path)?;
        if self.generation == Some(state.generation)
            && self.media_path == media_path
            && self.preferences.as_ref() == Some(preferences)
            && self.explicit_playlist == state.explicit_playlist
        {
            return None;
        }
        self.generation = Some(state.generation);
        self.media_path.clone_from(&media_path);
        self.preferences = Some(preferences.clone());
        self.explicit_playlist = state.explicit_playlist;
        Some((state.generation, local_path))
    }
}

pub(crate) type Matches = HashMap<PathBuf, Vec<PathBuf>>;

#[derive(Default)]
pub(crate) struct MenuFiles {
    pub(crate) related: Vec<PathBuf>,
    pub(crate) all: Vec<PathBuf>,
}

/// Browse the same search paths even when loading subtitles automatically is disabled.
pub(crate) fn files_for_menu(
    media_path: &Path,
    preferences: &AutoLoadPreferences,
    is_current: impl Fn() -> bool,
) -> MenuFiles {
    if !is_current() {
        return MenuFiles::default();
    }
    let Ok(media_path) = std::path::absolute(media_path) else {
        return MenuFiles::default();
    };
    let candidates = read_subtitle_candidates(&media_path, &preferences.search_paths, &is_current);
    let related = if preferences.mode == AutoLoadMode::Intelligent && is_current() {
        let files = media_files_with_stems(&media_path, true);
        files
            .iter()
            .find(|(path, _)| *path == media_path)
            .map_or_else(Vec::new, |(_, stem)| {
                let stems = unique_stems(&files);
                FolderMatcher::new(&stems, &candidates, preferences).matching_files(stem)
            })
    } else {
        Vec::new()
    };
    let mut all: Vec<_> = candidates.into_iter().map(|(path, _)| path).collect();
    all.sort_by(|left, right| {
        let left_name = left.file_name().unwrap_or_default().to_string_lossy();
        let right_name = right.file_name().unwrap_or_default().to_string_lossy();
        crate::platform::natural_compare(&left_name, &right_name).then_with(|| left.cmp(right))
    });
    MenuFiles { related, all }
}

pub(crate) fn find_matches(
    media_path: &Path,
    preferences: &AutoLoadPreferences,
    include_siblings: bool,
    is_current: impl Fn() -> bool,
) -> Matches {
    if preferences.mode == AutoLoadMode::Disabled || !is_current() {
        return Matches::new();
    }
    let Ok(media_path) = std::path::absolute(media_path) else {
        return Matches::new();
    };
    let files = media_files_with_stems(
        &media_path,
        include_siblings || preferences.mode == AutoLoadMode::Intelligent,
    );
    let stems = unique_stems(&files);
    let candidates = read_subtitle_candidates(&media_path, &preferences.search_paths, &is_current);
    let matcher = FolderMatcher::new(&stems, &candidates, preferences);
    files
        .into_iter()
        .filter(|(file, _)| include_siblings || *file == media_path)
        .take_while(|_| is_current())
        .map(|(file, stem)| (file, matcher.matching_files(&stem)))
        .collect()
}

fn media_files_with_stems(media_path: &Path, include_siblings: bool) -> Vec<(PathBuf, String)> {
    let mut files = if include_siblings {
        sibling_media_files(media_path)
    } else {
        Vec::new()
    };
    if !files.iter().any(|file| file == media_path) {
        files.push(media_path.into());
    }
    files
        .into_iter()
        .filter_map(|file| {
            let stem = file.file_stem()?.to_str()?.to_lowercase();
            (!stem.is_empty()).then_some((file, stem))
        })
        .collect()
}

fn unique_stems(files: &[(PathBuf, String)]) -> Vec<String> {
    let mut stems: Vec<_> = files.iter().map(|(_, stem)| stem.clone()).collect();
    stems.sort();
    stems.dedup();
    stems
}

#[cfg(test)]
#[test]
fn manual_browsing_includes_unmatched_files_even_when_auto_loading_is_disabled() {
    let temp = riina_test_support::TempDir::new("subtitle-menu-search").unwrap();
    let root = temp.path();
    std::fs::create_dir(root.join("subs")).unwrap();
    for file in [
        "episode.mkv",
        "episode.en.srt",
        "subs/extra10.srt",
        "subs/extra2.srt",
        "subs/.hidden.srt",
    ] {
        std::fs::write(root.join(file), b"").unwrap();
    }
    let path = root.join("episode.mkv");
    let mut prefs = AutoLoadPreferences {
        mode: AutoLoadMode::Disabled,
        search_paths: "./subs".into(),
        ..AutoLoadPreferences::default()
    };
    let files = files_for_menu(&path, &prefs, || true);
    assert!(files.related.is_empty());
    assert_eq!(
        files.all,
        [
            root.join("episode.en.srt"),
            root.join("subs/extra2.srt"),
            root.join("subs/extra10.srt")
        ]
    );
    prefs.mode = AutoLoadMode::Intelligent;
    let files = files_for_menu(&path, &prefs, || true);
    assert_eq!(files.related, [root.join("episode.en.srt")]);
    assert!(files_for_menu(&path, &prefs, || false).all.is_empty());
}

fn read_subtitle_candidates(
    media_path: &Path,
    search_paths: &str,
    is_current: impl Fn() -> bool,
) -> Vec<(PathBuf, String)> {
    let directories = expand_search_paths(media_path, search_paths);
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();
    for directory in directories.into_iter().take_while(|_| is_current()) {
        let Ok(entries) = std::fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.take(MAX_ENTRIES_PER_DIRECTORY).flatten() {
            let path = entry.path();
            if entry.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if !kind.is_file() || kind.is_symlink() || !is_subtitle_path(&path) {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(OsStr::to_str) else {
                continue;
            };
            if seen.insert(path.clone()) {
                let stem = stem.to_lowercase();
                candidates.push((path, stem));
                if candidates.len() >= MAX_CANDIDATE_SUBTITLES {
                    break;
                }
            }
        }
        if candidates.len() >= MAX_CANDIDATE_SUBTITLES {
            break;
        }
    }
    candidates
}

struct FolderMatcher<'a> {
    stems: &'a [String],
    candidates: &'a [(PathBuf, String)],
    mode: AutoLoadMode,
    priority: Vec<String>,
    series: OnceCell<crate::subtitle_series::SeriesMatches<'a>>,
    episodes: OnceCell<EpisodeMatches<'a>>,
    fuzzy: OnceCell<HashMap<&'a str, Vec<PathBuf>>>,
}

impl<'a> FolderMatcher<'a> {
    fn new(
        stems: &'a [String],
        candidates: &'a [(PathBuf, String)],
        preferences: &AutoLoadPreferences,
    ) -> Self {
        Self {
            stems,
            candidates,
            mode: preferences.mode,
            priority: preferences
                .priority_keywords
                .split(',')
                .map(str::trim)
                .filter(|keyword| !keyword.is_empty())
                .map(str::to_owned)
                .collect(),
            series: OnceCell::new(),
            episodes: OnceCell::new(),
            fuzzy: OnceCell::new(),
        }
    }

    fn matching_files(&self, media_stem: &str) -> Vec<PathBuf> {
        let mut found = self
            .candidates
            .iter()
            .filter(|(_, stem)| matches_stem(media_stem, stem, self.mode))
            .map(|(path, _)| path.clone())
            .collect::<Vec<_>>();
        if self.mode == AutoLoadMode::Intelligent && !self.candidates.is_empty() {
            let episodes = self
                .episodes
                .get_or_init(|| EpisodeMatches::new(self.stems, self.candidates));
            found.extend(episodes.for_media(media_stem));
            let series = self.series.get_or_init(|| {
                crate::subtitle_series::SeriesMatches::new(self.stems, self.candidates)
            });
            found.extend(series.for_media(media_stem));
            found.sort();
            found.dedup();
            if found.is_empty() {
                let fuzzy = self
                    .fuzzy
                    .get_or_init(|| fuzzy_matches(self.stems, self.candidates));
                found = fuzzy.get(media_stem).cloned().unwrap_or_default();
            }
        }
        prioritize(&mut found, &self.priority);
        found
    }
}

fn prioritize(found: &mut Vec<PathBuf>, priority_keywords: &[String]) {
    found.sort_by(|left, right| {
        crate::subtitle_priority::priority_occurrences(right, priority_keywords)
            .cmp(&crate::subtitle_priority::priority_occurrences(
                left,
                priority_keywords,
            ))
            .then_with(|| {
                left.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_lowercase()
                    .cmp(
                        &right
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_lowercase(),
                    )
            })
            .then_with(|| left.cmp(right))
    });
    found.truncate(MAX_MATCHED_SUBTITLES);
}

fn expand_search_paths(media_path: &Path, configured: &str) -> Vec<PathBuf> {
    let media_directory = media_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut directories = vec![media_directory.to_path_buf()];
    let mut seen = HashSet::from([media_directory.to_path_buf()]);
    for configured_path in
        std::env::split_paths(configured).filter(|path| !path.as_os_str().is_empty())
    {
        if directories.len() >= MAX_SEARCH_DIRECTORIES {
            break;
        }
        let Some(path) = expand_home(configured_path) else {
            continue;
        };
        let wildcard = path.file_name() == Some(OsStr::new("*"));
        if path
            .components()
            .any(|component| component.as_os_str().to_string_lossy().contains(['*', '?']))
            && !wildcard
        {
            continue;
        }
        let base = if wildcard {
            path.parent().unwrap_or_else(|| Path::new("."))
        } else {
            &path
        };
        let base = if base.is_absolute() {
            base.to_path_buf()
        } else {
            media_directory.join(base)
        };
        if wildcard {
            let Ok(entries) = std::fs::read_dir(base) else {
                continue;
            };
            let mut children = entries
                .take(MAX_ENTRIES_PER_DIRECTORY)
                .flatten()
                .filter_map(|entry| {
                    let name = entry.file_name();
                    if name.to_string_lossy().starts_with('.') {
                        return None;
                    }
                    let kind = entry.file_type().ok()?;
                    (kind.is_dir() && !kind.is_symlink()).then_some(entry.path())
                })
                .collect::<Vec<_>>();
            children.sort();
            for child in children {
                if directories.len() >= MAX_SEARCH_DIRECTORIES {
                    break;
                }
                if seen.insert(child.clone()) {
                    directories.push(child);
                }
            }
        } else if base.is_dir() && seen.insert(base.clone()) {
            directories.push(base);
        }
    }
    directories
}

fn expand_home(path: PathBuf) -> Option<PathBuf> {
    let text = path.to_str()?;
    let Some(rest) = text.strip_prefix('~') else {
        return Some(path);
    };
    if !rest.is_empty() && !rest.starts_with(['/', '\\']) {
        return Some(path);
    }
    let home = dirs::home_dir()?;
    Some(if rest.is_empty() {
        home
    } else {
        home.join(rest.trim_start_matches(['/', '\\']))
    })
}

fn is_subtitle_path(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| {
            riina_core::file_selection::SUPPORTED_SUBTITLE_EXTENSIONS
                .contains(&extension.to_lowercase().as_str())
        })
}

fn matches_stem(media: &str, subtitle: &str, mode: AutoLoadMode) -> bool {
    match mode {
        AutoLoadMode::Disabled => false,
        AutoLoadMode::Filename => subtitle.contains(media),
        AutoLoadMode::Intelligent => {
            subtitle == media
                || subtitle.strip_prefix(media).is_some_and(|suffix| {
                    suffix.chars().next().is_none_or(|character| {
                        matches!(character, '.' | '_' | '-' | ' ' | '[' | '(')
                    })
                })
        }
    }
}

#[derive(PartialEq, Eq)]
enum SeriesToken {
    Number(u64),
    Text(String),
}

impl From<String> for SeriesToken {
    fn from(token: String) -> Self {
        token
            .parse::<u64>()
            .map_or_else(|_| Self::Text(token), Self::Number)
    }
}

#[derive(Hash, PartialEq, Eq)]
enum SeriesPatternToken {
    Number,
    Text(String),
}

fn sibling_media_files(media_path: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Some(directory) = media_path.parent()
        && let Ok(entries) = std::fs::read_dir(directory)
    {
        for entry in entries.take(MAX_ENTRIES_PER_DIRECTORY).flatten() {
            let path = entry.path();
            if entry.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if !kind.is_file() || kind.is_symlink() || !is_media_path(&path) {
                continue;
            }
            files.push(path);
        }
    }
    files
}

struct EpisodeMatches<'a> {
    identities: HashMap<&'a str, Vec<SeriesToken>>,
    subtitles: Vec<(&'a PathBuf, Vec<SeriesToken>)>,
}

impl<'a> EpisodeMatches<'a> {
    fn new(stems: &'a [String], subtitles: &'a [(PathBuf, String)]) -> Self {
        let tokens: Vec<_> = stems
            .iter()
            .map(|stem| filename_token_ranges(stem))
            .collect();
        let mut identities = HashMap::new();
        for (pattern, members) in series_patterns(&tokens) {
            let Some(last) = varying_numbers(&pattern, &members, &tokens).next_back() else {
                continue;
            };
            for member in members {
                let identity = tokens[member][..=last]
                    .iter()
                    .map(|(token, _)| token.clone().into())
                    .collect();
                identities.insert(stems[member].as_str(), identity);
            }
        }
        let subtitles = if identities.is_empty() {
            Vec::new()
        } else {
            subtitles
                .iter()
                .map(|(path, stem)| {
                    let tokens = filename_tokens(stem)
                        .into_iter()
                        .map(SeriesToken::from)
                        .collect();
                    (path, tokens)
                })
                .collect()
        };
        Self {
            identities,
            subtitles,
        }
    }

    fn for_media(&self, media_stem: &str) -> impl Iterator<Item = PathBuf> {
        let identity = self.identities.get(media_stem);
        self.subtitles.iter().filter_map(move |(path, tokens)| {
            identity
                .filter(|identity| tokens.starts_with(identity))
                .map(|_| (*path).clone())
        })
    }
}

fn is_media_path(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| {
            let extension = extension.to_ascii_lowercase();
            MEDIA_EXTENSIONS.contains(&extension.as_str())
                || riina_core::file_selection::SUPPORTED_VIDEO_EXTENSIONS
                    .contains(&extension.as_str())
                || riina_core::file_selection::SUPPORTED_AUDIO_EXTENSIONS
                    .contains(&extension.as_str())
        })
}

fn filename_tokens(filename: &str) -> Vec<String> {
    filename_token_ranges(filename)
        .into_iter()
        .map(|(token, _)| token)
        .collect()
}

fn filename_token_ranges(filename: &str) -> Vec<(String, usize)> {
    let mut tokens = Vec::new();
    let mut token = String::new();
    let mut token_start = 0;
    let mut previous_kind = None;
    for (offset, character) in filename.char_indices() {
        let kind = if character.is_ascii_digit() {
            Some(true)
        } else if character.is_alphanumeric() {
            Some(false)
        } else {
            None
        };
        if (kind.is_none() || previous_kind.is_some_and(|previous| Some(previous) != kind))
            && !token.is_empty()
        {
            tokens.push((std::mem::take(&mut token), token_start));
        }
        if kind.is_some() {
            if token.is_empty() {
                token_start = offset;
            }
            token.extend(character.to_lowercase());
        }
        previous_kind = kind;
    }
    if !token.is_empty() {
        tokens.push((token, token_start));
    }
    tokens
}

fn series_patterns(
    tokens: &[Vec<(String, usize)>],
) -> impl Iterator<Item = (Vec<SeriesPatternToken>, Vec<usize>)> {
    let mut groups = HashMap::<Vec<SeriesPatternToken>, Vec<usize>>::new();
    for (index, filename_tokens) in tokens.iter().enumerate() {
        let pattern = filename_tokens
            .iter()
            .map(|(token, _)| {
                token.parse::<u64>().map_or_else(
                    |_| SeriesPatternToken::Text(token.clone()),
                    |_| SeriesPatternToken::Number,
                )
            })
            .collect();
        groups.entry(pattern).or_default().push(index);
    }
    groups.into_iter().filter(|(_, members)| members.len() >= 3)
}

fn varying_numbers<'a>(
    pattern: &'a [SeriesPatternToken],
    members: &'a [usize],
    tokens: &'a [Vec<(String, usize)>],
) -> impl DoubleEndedIterator<Item = usize> + 'a {
    (0..pattern.len()).filter(|token_index| {
        matches!(pattern[*token_index], SeriesPatternToken::Number)
            && members.iter().any(|member| {
                tokens[*member][*token_index].0.parse::<u64>()
                    != tokens[members[0]][*token_index].0.parse::<u64>()
            })
    })
}

fn series_split_offsets(stems: &[String]) -> Vec<usize> {
    let tokens = stems
        .iter()
        .map(|stem| filename_token_ranges(stem))
        .collect::<Vec<_>>();
    let mut offsets = vec![0; stems.len()];
    for (pattern, members) in series_patterns(&tokens) {
        let Some(first_varying) = varying_numbers(&pattern, &members, &tokens).next() else {
            continue;
        };
        for member in members {
            offsets[member] = tokens[member][first_varying].1;
        }
    }
    offsets
}

fn fuzzy_matches<'a>(
    sibling_stems: &'a [String],
    subtitles: &[(PathBuf, String)],
) -> HashMap<&'a str, Vec<PathBuf>> {
    let unmatched_subtitles = subtitles
        .iter()
        .enumerate()
        .filter(|(_, (_, subtitle))| {
            !sibling_stems
                .iter()
                .any(|media| matches_stem(media, subtitle, AutoLoadMode::Intelligent))
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let unmatched_media = sibling_stems
        .iter()
        .enumerate()
        .filter(|(_, media)| {
            !subtitles
                .iter()
                .any(|(_, subtitle)| matches_stem(media, subtitle, AutoLoadMode::Intelligent))
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    if unmatched_media.is_empty()
        || unmatched_subtitles.is_empty()
        || unmatched_media
            .len()
            .saturating_mul(unmatched_subtitles.len())
            >= MAX_FUZZY_PAIRS
    {
        return HashMap::new();
    }
    let media_split_offsets = series_split_offsets(sibling_stems);
    let subtitle_stems = subtitles
        .iter()
        .map(|(_, stem)| stem.clone())
        .collect::<Vec<_>>();
    let subtitle_split_offsets = series_split_offsets(&subtitle_stems);
    let mut distances = vec![vec![None; unmatched_subtitles.len()]; unmatched_media.len()];
    for (media_index, video_index) in unmatched_media.iter().enumerate() {
        let video = &sibling_stems[*video_index];
        if video.graphemes(true).count() > MAX_FUZZY_STEM_CHARACTERS {
            continue;
        }
        for (subtitle_index, subtitle_index_in_all) in unmatched_subtitles.iter().enumerate() {
            let subtitle = &subtitles[*subtitle_index_in_all].1;
            if subtitle.graphemes(true).count() > MAX_FUZZY_STEM_CHARACTERS
                || episode_number_conflicts(video, subtitle)
            {
                continue;
            }
            let threshold =
                (video.graphemes(true).count() + subtitle.graphemes(true).count()) * 3 / 5;
            let limit = threshold.saturating_sub(1);
            let video_split = media_split_offsets[*video_index];
            let subtitle_split = subtitle_split_offsets[*subtitle_index_in_all];
            let (video_prefix, video_suffix) = video.split_at(video_split);
            let (subtitle_prefix, subtitle_suffix) = subtitle.split_at(subtitle_split);
            let Some(prefix_distance) = edit_distance_bounded(video_prefix, subtitle_prefix, limit)
            else {
                continue;
            };
            let Some(suffix_distance) =
                edit_distance_bounded(video_suffix, subtitle_suffix, limit - prefix_distance)
            else {
                continue;
            };
            let distance = prefix_distance + suffix_distance;
            distances[media_index][subtitle_index] = Some(distance);
        }
    }

    let nearest_video_distance = (0..unmatched_subtitles.len())
        .map(|subtitle_index| {
            distances
                .iter()
                .filter_map(|video| video[subtitle_index])
                .min()
        })
        .collect::<Vec<_>>();
    distances
        .into_iter()
        .zip(unmatched_media)
        .filter_map(|(row, media_index)| {
            let nearest = row.iter().flatten().min().copied()?;
            let found = row
                .into_iter()
                .enumerate()
                .filter(|(index, distance)| {
                    *distance == Some(nearest) && *distance == nearest_video_distance[*index]
                })
                .map(|(index, _)| subtitles[unmatched_subtitles[index]].0.clone())
                .collect();
            Some((sibling_stems[media_index].as_str(), found))
        })
        .collect()
}

fn episode_number_conflicts(media_stem: &str, subtitle_stem: &str) -> bool {
    let media = filename_tokens(media_stem);
    let subtitle = filename_tokens(subtitle_stem);
    media
        .iter()
        .zip(&subtitle)
        .enumerate()
        .any(|(index, (left, right))| {
            left.parse::<u64>()
                .ok()
                .zip(right.parse::<u64>().ok())
                .is_some_and(|(left_number, right_number)| {
                    left_number != right_number && media[..index] == subtitle[..index] && index > 0
                })
        })
}

pub(crate) fn edit_distance_bounded(left: &str, right: &str, limit: usize) -> Option<usize> {
    let left = left.graphemes(true).collect::<Vec<_>>();
    let right = right.graphemes(true).collect::<Vec<_>>();
    if left.len().abs_diff(right.len()) > limit {
        return None;
    }
    let unreachable = limit.saturating_add(1);
    let mut previous = vec![unreachable; right.len() + 1];
    for (index, value) in previous
        .iter_mut()
        .enumerate()
        .take(limit.min(right.len()) + 1)
    {
        *value = index;
    }
    for (left_index, left_grapheme) in left.iter().enumerate() {
        let row = left_index + 1;
        let start = row.saturating_sub(limit).max(1);
        let end = (row + limit).min(right.len());
        let mut current = vec![unreachable; right.len() + 1];
        if row <= limit {
            current[0] = row;
        }
        for column in start..=end {
            current[column] = (previous[column].saturating_add(1))
                .min(current[column - 1].saturating_add(1))
                .min(
                    previous[column - 1]
                        .saturating_add(usize::from(left_grapheme != &right[column - 1])),
                );
        }
        if current.iter().copied().min().unwrap_or(unreachable) > limit {
            return None;
        }
        previous = current;
    }
    let distance = previous[right.len()];
    (distance <= limit).then_some(distance)
}

#[cfg(test)]
mod tests {
    use super::find_matches;
    use riina_core::subtitle_controls::{AutoLoadMode, AutoLoadPreferences};
    use std::path::{Path, PathBuf};

    struct TestTree(riina_test_support::TempDir);

    impl TestTree {
        fn new() -> Self {
            Self(riina_test_support::TempDir::new("subtitle-discovery").unwrap())
        }

        fn file(&self, path: &str) -> PathBuf {
            let path = self.0.path().join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, []).unwrap();
            path
        }

        fn media(&self) -> PathBuf {
            self.file("Movie.mkv")
        }
    }

    fn preferences(mode: AutoLoadMode, search_paths: &str) -> AutoLoadPreferences {
        AutoLoadPreferences {
            mode,
            priority_keywords: String::new(),
            search_paths: search_paths.into(),
        }
    }

    fn find_matching_files(media: &Path, preferences: &AutoLoadPreferences) -> Vec<PathBuf> {
        let absolute = std::path::absolute(media).unwrap();
        find_matches(media, preferences, false, || true)
            .remove(&absolute)
            .unwrap_or_default()
    }

    #[test]
    fn folder_scan_keeps_matches_for_each_episode_and_limits_explicit_open_to_current_file() {
        let tree = TestTree::new();
        let first = tree.file("Café Stories S01 E01.mkv");
        let second = tree.file("Café Stories S01 E02.mkv");
        let third = tree.file("Café Stories S01 E03.mxf");
        let first_sub = tree.file("captions/Café Stories S01 E01.en.srt");
        let second_sub = tree.file("captions/Café Stories S01 E02.en.srt");
        let second_preferred = tree.file("captions/Café Stories S01 E02.fr.srt");
        let mut settings = preferences(AutoLoadMode::Intelligent, "captions");
        settings.priority_keywords = "fr".into();

        let all = find_matches(&first, &settings, true, || true);
        assert_eq!(all[&first], std::slice::from_ref(&first_sub));
        assert_eq!(all[&second], [second_preferred, second_sub]);
        assert!(all[&third].is_empty());
        assert_eq!(
            find_matches(&first, &settings, false, || true),
            [(first, vec![first_sub])].into()
        );
    }

    #[test]
    fn expands_default_child_directory_glob_and_finds_supported_matching_subtitles() {
        let tree = TestTree::new();
        let adjacent = tree.file("Movie.aa.srt");
        let english = tree.file("English/Movie.en.srt");
        tree.file("English/Other.en.srt");
        tree.file("English/Movie.en.txt");
        tree.file(".hidden/Movie.srt");
        let loose = tree.file("Loose/Movie.ass");
        let found = find_matching_files(
            &tree.media(),
            &preferences(AutoLoadMode::Intelligent, "./*"),
        );
        assert_eq!(found, [adjacent, loose, english]);
    }

    #[test]
    fn priority_sort_ignores_search_folder_and_subtitle_extension() {
        let tree = TestTree::new();
        let filename_order = tree.file("srt/Movie.aaa.en.vtt");
        let extension_only = tree.file("captions/Movie.zzz.en.srt");
        let paths = std::env::join_paths([Path::new("captions"), Path::new("srt")])
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let mut settings = preferences(AutoLoadMode::Intelligent, &paths);
        settings.priority_keywords = "srt".into();

        assert_eq!(
            find_matching_files(&tree.media(), &settings),
            [filename_order, extension_only]
        );
    }

    #[test]
    fn explicit_search_paths_are_relative_to_media_and_deduplicated() {
        let tree = TestTree::new();
        let expected = tree.file("captions/Movie.forced.vtt");
        let search_paths = std::env::join_paths([Path::new("captions"), Path::new("captions")])
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert_eq!(
            find_matching_files(
                &tree.media(),
                &preferences(AutoLoadMode::Intelligent, &search_paths)
            ),
            [expected]
        );
    }

    #[test]
    fn filename_mode_uses_containment_but_intelligent_mode_requires_a_media_name_prefix() {
        let tree = TestTree::new();
        let media = tree.file("My Movie.mkv");
        let prefix = tree.file("subs/MY MOVIE.en.srt");
        tree.file("subs/alt.My Movie.srt");
        assert_eq!(
            find_matching_files(&media, &preferences(AutoLoadMode::Intelligent, "subs")),
            [prefix]
        );
        assert_eq!(
            find_matching_files(&media, &preferences(AutoLoadMode::Filename, "subs")),
            [
                tree.0.path().join("subs/alt.My Movie.srt"),
                tree.0.path().join("subs/MY MOVIE.en.srt")
            ]
        );
    }

    #[test]
    fn intelligent_mode_matches_same_episode_across_series_filename_styles() {
        let tree = TestTree::new();
        let media = tree.file("Show.S01E02.mkv");
        tree.file("Show.S01E01.mkv");
        tree.file("Show.S01E03.mkv");
        tree.file("Unrelated.Movie.mkv");
        let same_episode = tree.file("Show S01 E02 - French.srt");
        tree.file("Show S01 E01 - French.srt");

        assert_eq!(
            find_matching_files(&media, &preferences(AutoLoadMode::Intelligent, ".")),
            [same_episode]
        );
    }

    #[test]
    fn direct_match_survives_competing_subtitle_series_prefixes() {
        let tree = TestTree::new();
        let media = tree.file("The Other Side - Part 002.mkv");
        tree.file("The Other Side - Part 001.mkv");
        tree.file("The Other Side - Part 003.mkv");
        tree.file("subs/The Other Side - English 001.en.srt");
        tree.file("subs/The Other Side - English 002.en.srt");
        tree.file("subs/The Other Side - English 003.en.srt");
        let direct_match = tree.file("subs/The Other Side - Part 002.en.srt");

        assert_eq!(
            find_matching_files(&media, &preferences(AutoLoadMode::Intelligent, "subs")),
            [direct_match]
        );
    }

    #[test]
    fn intelligent_mode_uses_closest_fuzzy_name_when_no_exact_match_exists() {
        let tree = TestTree::new();
        let expected = tree.file("subs/Mvoie.en.srt");
        tree.file("subs/Other.en.srt");

        assert_eq!(
            find_matching_files(
                &tree.media(),
                &preferences(AutoLoadMode::Intelligent, "subs")
            ),
            [expected]
        );
    }

    #[test]
    fn fuzzy_fallback_does_not_steal_a_neighboring_videos_closer_subtitle() {
        let tree = TestTree::new();
        let expected_for_neighbor = tree.file("subs/Mvoie.en.srt");
        tree.file("Mvoie.mkv");

        assert!(
            find_matching_files(
                &tree.media(),
                &preferences(AutoLoadMode::Intelligent, "subs")
            )
            .is_empty()
        );
        assert!(expected_for_neighbor.exists());
    }

    #[test]
    fn fuzzy_fallback_requires_a_reciprocal_nearest_match() {
        let tree = TestTree::new();
        let media = tree.file("Abcd.mkv");
        tree.file("Abce.mkv");
        tree.file("subs/Abcex.srt");

        assert!(
            find_matching_files(&media, &preferences(AutoLoadMode::Intelligent, "subs")).is_empty()
        );
    }

    #[test]
    fn fuzzy_fallback_is_skipped_when_the_pairing_work_bound_is_reached() {
        let tree = TestTree::new();
        let media = tree.media();
        for index in 0..100 {
            tree.file(&format!("Series.{index:03}.mkv"));
            tree.file(&format!("subs/Captions.{index:03}.srt"));
        }

        assert!(
            find_matching_files(&media, &preferences(AutoLoadMode::Intelligent, "subs")).is_empty()
        );
    }

    #[test]
    fn fuzzy_fallback_does_not_cross_an_episode_number() {
        let tree = TestTree::new();
        let media = tree.file("Show.S01E02.mkv");
        let other_episode = tree.file("subs/Show.S01E01.en.srt");
        tree.file("Show.S01E01.mkv");
        tree.file("Show.S01E03.mkv");

        assert!(
            find_matching_files(&media, &preferences(AutoLoadMode::Intelligent, "subs")).is_empty()
        );
        assert!(other_episode.exists());
    }

    #[test]
    fn disabled_mode_does_not_scan_or_load_candidate_files() {
        let tree = TestTree::new();
        tree.file("subs/Movie.srt");
        assert!(
            find_matching_files(&tree.media(), &preferences(AutoLoadMode::Disabled, "subs"))
                .is_empty()
        );
    }

    #[test]
    fn discovery_runs_once_for_each_active_file_and_retries_after_preference_changes() {
        let mut controller = super::AutoDiscovery::default();
        let mut state = riina_core::Playback {
            generation: 4,
            phase: riina_core::Phase::Playing,
            playlist: vec![riina_core::PlaylistEntry {
                id: 1,
                filename: "movie.mkv".into(),
                title: None,
                current: true,
                playing: true,
            }]
            .into(),
            ..riina_core::Playback::default()
        };
        let mut preferences = preferences(AutoLoadMode::Intelligent, "subs");
        assert_eq!(
            controller.observe(&state, &preferences),
            Some((4, PathBuf::from("movie.mkv")))
        );
        assert_eq!(controller.observe(&state, &preferences), None);
        preferences.search_paths = "captions".into();
        assert_eq!(
            controller.observe(&state, &preferences),
            Some((4, PathBuf::from("movie.mkv")))
        );
        preferences.mode = AutoLoadMode::Disabled;
        assert_eq!(controller.observe(&state, &preferences), None);
        preferences.mode = AutoLoadMode::Intelligent;
        assert_eq!(
            controller.observe(&state, &preferences),
            Some((4, PathBuf::from("movie.mkv")))
        );
        assert_eq!(controller.observe(&state, &preferences), None);
        state.generation = 5;
        assert_eq!(
            controller.observe(&state, &preferences),
            Some((5, PathBuf::from("movie.mkv")))
        );
        std::sync::Arc::make_mut(&mut state.playlist)[0].filename =
            "https://example.invalid/movie.mkv".into();
        state.generation = 6;
        assert_eq!(controller.observe(&state, &preferences), None);
    }
}
