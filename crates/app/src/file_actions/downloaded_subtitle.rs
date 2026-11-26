use crate::{
    NativeVideoHost, PlaybackStore, PlayerWindow, background, open_subtitles, platform,
    ui_dialogs::SaveFile,
};
use riina_core::{Playback, TrackKind, media_reference::same_native_path, sync::lock};
use slint::ComponentHandle;
use std::{
    io::{self, Read},
    path::{Path, PathBuf},
};

pub(crate) fn downloaded_subtitle_path(playback: &Playback) -> Option<PathBuf> {
    selected_path(playback, &open_subtitles::download_cache_root()?)
}

fn selected_path(playback: &Playback, root: &Path) -> Option<PathBuf> {
    let track = playback.tracks.iter().find(|track| {
        track.kind == TrackKind::Sub && track.is_primary_selected() && track.external
    })?;
    let path = PathBuf::from(track.external_filename.as_deref()?);
    let parent = path.parent()?;
    let in_cache = parent == root
        || parent
            .to_str()
            .zip(root.to_str())
            .is_some_and(|(parent, root)| same_native_path(parent, root));
    in_cache.then_some(path)
}

pub(crate) fn save_downloaded_subtitle(
    app: &PlayerWindow,
    host: Option<slint::Weak<NativeVideoHost>>,
    latest_playback: &PlaybackStore,
) {
    let playback = lock(latest_playback);
    let Some(path) = downloaded_subtitle_path(&playback) else {
        app.set_status("Select a subtitle downloaded by Riina before saving it".into());
        return;
    };
    let generation = playback.generation;
    drop(playback);
    let playback = latest_playback.clone();
    let name = suggested_name(&path).to_owned();
    let extension = Path::new(&name)
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or("srt")
        .to_owned();
    platform::save_file_dialog(
        app,
        host,
        SaveFile {
            title: "Save Downloaded Subtitle",
            name,
            filter: "Subtitle",
            extension,
        },
        move |app, chosen| match chosen {
            Ok(Some(destination)) => export(app, path, destination, playback, generation),
            Ok(None) => {}
            Err(error) => report(app, &error),
        },
    );
}

fn suggested_name(path: &Path) -> &str {
    let leaf = path
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or("subtitle.srt");
    // Cached files include a unique prefix. The export keeps the provider's name.
    if let Some((stamp, rest)) = leaf.split_once('-')
        && let Some((counter, name)) = rest.split_once('-')
        && stamp.parse::<u128>().is_ok()
        && counter.parse::<u64>().is_ok()
        && !name.is_empty()
    {
        return name;
    }
    leaf
}

fn export(
    app: &PlayerWindow,
    source: PathBuf,
    destination: PathBuf,
    playback: PlaybackStore,
    generation: u64,
) {
    let weak = app.as_weak();
    if let Err(error) = background::spawn_named("subtitle-export", move || {
        let result = open_subtitles::download_cache_root()
            .ok_or_else(|| io::Error::other("The user cache directory is unavailable"))
            .and_then(|root| copy_download(&root, &source, &destination));
        _ = background::to_ui(weak, move |app| match result {
            Ok(()) => {
                if lock(&playback).generation == generation {
                    app.set_status("Subtitle saved".into());
                    crate::player_ui::show_subtitle_saved(&app);
                }
            }
            Err(error) => report(&app, &error.to_string()),
        });
    }) {
        report(app, &error.to_string());
    }
}

fn copy_download(root: &Path, source: &Path, destination: &Path) -> io::Result<()> {
    let root = root.canonicalize()?;
    let source = source.canonicalize()?;
    if source.parent() != Some(root.as_path()) || !source.metadata()?.is_file() {
        return Err(io::Error::other(
            "The subtitle is outside the download cache",
        ));
    }
    let file = std::fs::File::open(source)?;
    let mut bytes = Vec::new();
    file.take(open_subtitles::MAX_SUBTITLE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.is_empty() || bytes.len() as u64 > open_subtitles::MAX_SUBTITLE_BYTES {
        return Err(io::Error::other(
            "The downloaded subtitle is empty or exceeds 16 MiB",
        ));
    }
    platform::settings_file::save_bytes(destination, &bytes)
}

fn report(app: &PlayerWindow, error: &str) {
    crate::diagnostics::report(app, format!("Could not save downloaded subtitle: {error}"));
}

#[cfg(test)]
mod tests {
    use super::*;
    use riina_test_support::TempDir;

    #[test]
    fn copies_original_subtitle_bytes_and_preserves_existing_file_on_failure() {
        let temp = TempDir::new("subtitle-export").unwrap();
        let cache = temp.path().join("cache");
        std::fs::create_dir(&cache).unwrap();
        let source = cache.join("1795716069000000000-2-L'été 02.ass");
        let outside = temp.path().join("Outside.ass");
        let destination = temp.path().join("Saved.ass");
        let bytes = b"\xef\xbb\xbf[Script Info]\r\nTitle: \xc3\xa9";
        std::fs::write(&source, bytes).unwrap();
        std::fs::write(&outside, b"outside the cache").unwrap();
        std::fs::write(&destination, b"previous").unwrap();

        assert_eq!(suggested_name(&source), "L'été 02.ass");
        assert_eq!(suggested_name(&outside), "Outside.ass");
        copy_download(&cache, &source, &destination).unwrap();
        assert_eq!(std::fs::read(&destination).unwrap(), bytes);
        assert!(copy_download(&cache, &outside, &destination).is_err());
        std::fs::write(&source, b"").unwrap();
        assert!(copy_download(&cache, &source, &destination).is_err());
        assert_eq!(std::fs::read(&destination).unwrap(), bytes);
    }
}
