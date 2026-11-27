//! External track pickers own one media generation and its starting folder.
use super::{OpenFiles, Purpose, SelectionMode};
use crate::{PlaybackStore, PlayerWindow, file_actions::local_media_path};
use riina_core::{Command, Playback, file_selection, sync::lock};
use riina_mpv::CommandSender;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy)]
pub(crate) enum TrackFile {
    Audio,
    Subtitle,
}

impl TrackFile {
    pub(crate) fn request(self, app: &PlayerWindow, playback: &PlaybackStore) -> Option<Purpose> {
        let state = lock(playback).clone();
        active(&state).then(|| {
            Purpose::ExternalTrack(TrackRequest {
                kind: self,
                generation: state.generation,
                open_request: app.get_media_request_generation(),
                directory: media_folder(&state),
                playback: playback.clone(),
            })
        })
    }

    pub(crate) fn filter_name(self) -> &'static str {
        match self {
            Self::Audio => "Audio and video files",
            Self::Subtitle => "Subtitle and video files",
        }
    }

    pub(crate) fn extensions(self) -> impl Iterator<Item = &'static str> {
        let formats = match self {
            Self::Audio => file_selection::SUPPORTED_AUDIO_EXTENSIONS,
            Self::Subtitle => file_selection::SUPPORTED_SUBTITLE_EXTENSIONS,
        };
        formats
            .iter()
            .chain(file_selection::SUPPORTED_VIDEO_EXTENSIONS)
            .copied()
    }

    fn command(self, path: String) -> Result<Command, String> {
        let supported = Path::new(&path)
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| self.extensions().any(|item| item.eq_ignore_ascii_case(ext)));
        if !supported {
            return Err(match self {
                Self::Audio => "Choose a supported audio or video file",
                Self::Subtitle => "Choose a supported subtitle or video file",
            }
            .into());
        }
        Ok(match self {
            Self::Audio => Command::AddAudio(path),
            Self::Subtitle => Command::AddSubtitle(path),
        })
    }
}

pub(crate) struct TrackRequest {
    kind: TrackFile,
    generation: u64,
    open_request: i32,
    directory: Option<PathBuf>,
    playback: PlaybackStore,
}

impl TrackRequest {
    pub(super) fn picker(&self) -> OpenFiles {
        OpenFiles {
            title: match self.kind {
                TrackFile::Audio => "Load External Audio",
                TrackFile::Subtitle => "Load External Subtitle",
            },
            mode: SelectionMode::Track {
                kind: self.kind,
                directory: self.directory.clone(),
            },
        }
    }

    pub(super) fn attach(
        self,
        app: &PlayerWindow,
        paths: Vec<String>,
        sender: &CommandSender,
    ) -> Result<(), String> {
        let Some(path) = paths.into_iter().next() else {
            return Ok(());
        };
        let state = lock(&self.playback).clone();
        if app.get_media_request_generation() != self.open_request
            || state.generation != self.generation
            || !active(&state)
        {
            return Ok(());
        }
        sender
            .send_for_media(self.generation, self.kind.command(path)?)
            .map_err(String::from)
    }
}

fn active(state: &Playback) -> bool {
    state.phase.has_loaded_file() && !state.idle_active && !state.engine_shutdown
}

fn media_folder(state: &Playback) -> Option<PathBuf> {
    let entry = state.playlist.iter().find(|entry| {
        state
            .active_playlist_entry
            .map_or(entry.current, |current| current == entry.id)
    })?;
    let path = local_media_path(&entry.filename)?;
    if path == Path::new("-") {
        return None;
    }
    std::path::absolute(path).ok()?.parent().map(Path::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_dialogs::dispatch_selection;
    use riina_core::{Phase, PlaylistEntry};
    use std::{sync::Arc, sync::mpsc::TryRecvError};

    #[test]
    fn track_picker_uses_the_active_folder_and_retires_stale_selections()
    -> Result<(), Box<dyn std::error::Error>> {
        i_slint_backend_testing::init_no_event_loop();
        let app = PlayerWindow::new()?;
        let playback = PlaybackStore::default();
        let (commands, engine) = CommandSender::recording(4);
        assert!(TrackFile::Audio.request(&app, &playback).is_none());

        let folder = std::env::temp_dir().join("live-片段");
        let media = folder.join("movie.mkv").to_string_lossy().into_owned();
        let mut state = Playback {
            generation: 2,
            phase: Phase::Paused,
            active_playlist_entry: Some(15),
            playlist: Arc::new(vec![
                PlaylistEntry {
                    id: 12,
                    filename: "old/movie.mkv".into(),
                    title: None,
                    current: true,
                    playing: true,
                },
                PlaylistEntry {
                    id: 15,
                    filename: media,
                    title: None,
                    current: false,
                    playing: false,
                },
            ]),
            ..Default::default()
        };
        *lock(&playback) = Arc::new(state.clone());
        let open = |kind: TrackFile| {
            kind.request(&app, &playback)
                .ok_or("Expected an active file")
        };
        let dispatch = |purpose, file: &str| {
            dispatch_selection(&app, purpose, vec![file.into()], commands.clone())
        };

        let audio = open(TrackFile::Audio)?;
        let SelectionMode::Track { kind, directory } = audio.picker().mode else {
            return Err("Expected a single external track picker".into());
        };
        assert_eq!(directory.as_deref(), Some(folder.as_path()));
        assert!(kind.extensions().any(|ext| ext == "wav"));
        assert!(dispatch(open(TrackFile::Audio)?, "subtitle.srt").is_err());
        dispatch(audio, "external-audio.MKV")?;
        assert_eq!(
            engine.commands.try_recv()?,
            Command::AddAudio("external-audio.MKV".into())
        );

        let subtitles = open(TrackFile::Subtitle)?;
        assert!(dispatch(open(TrackFile::Subtitle)?, "audio.wav").is_err());
        dispatch(subtitles, "embedded-subtitles.mkv")?;
        assert_eq!(
            engine.commands.try_recv()?,
            Command::AddSubtitle("embedded-subtitles.mkv".into())
        );

        let superseded_open = open(TrackFile::Audio)?;
        app.set_media_request_generation(1);
        dispatch(superseded_open, "external-audio.wav")?;
        let superseded_media = open(TrackFile::Subtitle)?;
        state.generation += 1;
        Arc::make_mut(&mut state.playlist)[1].filename = "https://example.test/movie.mkv".into();
        *lock(&playback) = Arc::new(state);
        dispatch(superseded_media, "subtitle.srt")?;
        assert_eq!(engine.commands.try_recv(), Err(TryRecvError::Empty));
        assert!(matches!(
            open(TrackFile::Subtitle)?.picker().mode,
            SelectionMode::Track {
                directory: None,
                ..
            }
        ));
        Ok(())
    }
}
