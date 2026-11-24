//! Discover playlist neighbors and filename series in the current media folder.

use crate::playlist_metadata::series::Series;
use riina_core::{Command, Phase, Playback, file_selection};
use std::{
    collections::HashSet,
    ffi::OsStr,
    path::{Path, PathBuf},
};

#[derive(Default)]
pub(crate) struct FolderDiscovery {
    last_media: Option<(u64, String, Options)>,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Options {
    pub(crate) append: bool,
    pub(crate) series: bool,
}

#[derive(Clone)]
pub(crate) struct Request {
    generation: u64,
    media_path: PathBuf,
    current_path: String,
    pub(crate) options: Options,
}

pub(crate) struct Prepared {
    pub(crate) insert: Option<Command>,
    pub(crate) series: Option<Series>,
}

impl FolderDiscovery {
    pub(crate) fn observe(&mut self, state: &Playback, options: Options) -> Option<Request> {
        if options == Options::default() || state.explicit_playlist || state.playlist.is_empty() {
            self.last_media = None;
            return None;
        }
        if !matches!(
            state.phase,
            Phase::Playing | Phase::Paused | Phase::Buffering
        ) {
            return None;
        }
        let current_path = state
            .playlist
            .iter()
            .find(|entry| entry.current)
            .map(|entry| entry.filename.clone())?;
        let media_path = crate::file_actions::local_media_path(&current_path)?;
        let media_path = std::path::absolute(media_path).ok()?;
        let identity = (state.generation, current_path.clone(), options);
        if self.last_media.as_ref() == Some(&identity) {
            return None;
        }
        self.last_media = Some(identity);
        Some(Request {
            generation: state.generation,
            media_path,
            current_path,
            options,
        })
    }
}

pub(crate) fn request_is_current(request: &Request, state: &Playback) -> bool {
    !state.explicit_playlist
        && state.generation == request.generation
        && state
            .playlist
            .iter()
            .any(|entry| entry.current && entry.filename == request.current_path)
}

pub(crate) fn prepare(request: &Request, state: &Playback) -> Option<Prepared> {
    if !request_is_current(request, state) {
        return None;
    }
    let folder = request.media_path.parent()?;
    let files = read_media_files(folder)?;
    Some(Prepared {
        insert: request
            .options
            .append
            .then(|| insertion(request, state, &files))
            .flatten(),
        series: request.options.series.then(|| Series::from_files(&files)),
    })
}

fn read_media_files(folder: &Path) -> Option<Vec<String>> {
    let mut videos = Vec::new();
    let mut audio = Vec::new();
    let mut seen = HashSet::new();
    for entry in std::fs::read_dir(folder).ok()?.flatten() {
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if !kind.is_file() || kind.is_symlink() {
            continue;
        }
        let Some(path) = entry
            .path()
            .into_os_string()
            .into_string()
            .ok()
            .map(|path| crate::platform::resolve_alias_path(&path))
        else {
            continue;
        };
        if !Path::new(&path).is_file() || !file_selection::is_supported_media(&path) {
            continue;
        }
        if !seen.insert(path.clone()) {
            continue;
        }
        if file_selection::is_supported_video(&path) {
            videos.push(path);
        } else {
            audio.push(path);
        }
    }
    sort_media(&mut videos);
    sort_media(&mut audio);
    Some(videos.into_iter().chain(audio).collect())
}

fn insertion(request: &Request, state: &Playback, files: &[String]) -> Option<Command> {
    let current = files
        .iter()
        .position(|path| crate::media_open::same_media(path, &request.current_path))?;
    let existing = state
        .playlist
        .iter()
        .map(|entry| entry.filename.as_str())
        .collect::<HashSet<_>>();
    let before = files[..current]
        .iter()
        .filter(|path| {
            !existing.contains(path.as_str())
                && !state
                    .playlist
                    .iter()
                    .any(|entry| crate::media_open::same_media(&entry.filename, path))
        })
        .cloned()
        .collect::<Vec<_>>();
    let after = files[current + 1..]
        .iter()
        .filter(|path| {
            !existing.contains(path.as_str())
                && !state
                    .playlist
                    .iter()
                    .any(|entry| crate::media_open::same_media(&entry.filename, path))
        })
        .cloned()
        .collect::<Vec<_>>();
    if before.is_empty() && after.is_empty() {
        None
    } else {
        Some(Command::InsertPlaylistFilesAroundCurrent { before, after })
    }
}

fn sort_media(paths: &mut [String]) {
    paths.sort_by(|left, right| {
        let left_name = Path::new(left).file_name().unwrap_or(OsStr::new(left));
        let right_name = Path::new(right).file_name().unwrap_or(OsStr::new(right));
        let left_name = left_name.to_string_lossy();
        let right_name = right_name.to_string_lossy();
        crate::platform::natural_compare(&left_name, &right_name).then_with(|| left.cmp(right))
    });
}

#[cfg(test)]
#[path = "auto_playlist_tests.rs"]
mod tests;
