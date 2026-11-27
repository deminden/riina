//! Classify a drop before changing the receiving player's playback request.
use super::{ObserverId, open_prepared_command};
use crate::{PlayerWindow, background, diagnostics, file_selection, platform};
use riina_core::{Command, media_reference::is_network_url_candidate};
use riina_mpv::CommandSender;
use slint::ComponentHandle;
use std::{ffi::OsString, path::Path, sync::atomic::Ordering};

pub(crate) fn dropped(
    app: &PlayerWindow,
    paths: Vec<OsString>,
    media: Option<u64>,
    sender: CommandSender,
) {
    if let Some(filter) = dropped_lut(&paths) {
        if let Err(error) = filter.and_then(|filter| {
            sender
                .send(Command::AddVideoFilter(filter))
                .map_err(|error| error.to_string())
        }) {
            diagnostics::report(app, error);
        }
        return;
    }

    let request = app.get_media_request_generation();
    let observer = ObserverId::for_window(app);
    let cancelled = observer
        .map(ObserverId::begin_drop_preparation)
        .unwrap_or_default();
    let player = app.as_weak();
    if let Err(error) = background::spawn_named("media-drop-preparation", move || {
        let result = prepare_drop(paths, || cancelled.load(Ordering::Relaxed));
        // A closed destination has no recipient for the prepared drop.
        let _ = background::to_ui(player, move |app| {
            if cancelled.load(Ordering::Relaxed) || app.get_media_request_generation() != request {
                return;
            }
            let result = result.and_then(|drop| drop.apply(&app, &sender, media, observer));
            if let Err(error) = result {
                diagnostics::report(&app, error);
            }
        });
    }) {
        diagnostics::report(app, format!("Could not prepare the drop: {error}"));
    }
}

#[derive(Debug, PartialEq)]
enum Drop {
    Play(Vec<String>),
    Subtitles(Vec<String>),
}

impl Drop {
    fn apply(
        self,
        app: &PlayerWindow,
        sender: &CommandSender,
        media: Option<u64>,
        observer: Option<ObserverId>,
    ) -> Result<(), String> {
        match self {
            Self::Play(paths) => {
                let generation = super::invalidate(app);
                app.set_status("Opening…".into());
                open_prepared_command(app, sender, Command::OpenFiles(paths), observer, generation)
            }
            Self::Subtitles(paths) => {
                let generation = media.ok_or("Open a media file before dropping subtitles")?;
                sender
                    .send_for_media(
                        generation,
                        Command::AddSubtitlesForMedia { generation, paths },
                    )
                    .map_err(|error| error.to_string())
            }
        }
    }
}

fn prepare_drop(paths: Vec<OsString>, cancelled: impl Fn() -> bool) -> Result<Drop, String> {
    let paths = paths
        .into_iter()
        .map(OsString::into_string)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "A media path contains invalid Unicode")?;
    let playable = file_selection::prepare_cancellable(paths.clone(), true, &cancelled)?;
    if !playable.is_empty() {
        return Ok(Drop::Play(playable));
    }
    let subtitles = paths
        .into_iter()
        .take_while(|_| !cancelled())
        .filter(|path| !is_network_url_candidate(path))
        .map(|path| platform::resolve_alias_path(&path))
        .filter(|path| riina_core::file_selection::is_supported_subtitle(path))
        .collect::<Vec<_>>();
    if cancelled() {
        return Err("Media selection was cancelled".into());
    }
    if subtitles.is_empty() {
        return Err("No playable files were found".into());
    }
    Ok(Drop::Subtitles(subtitles))
}

fn dropped_lut(paths: &[OsString]) -> Option<Result<String, String>> {
    let [path] = paths else { return None };
    if path.to_str().is_some_and(is_network_url_candidate) {
        return None;
    }
    let extension = Path::new(path).extension()?;
    if !["3dl", "cube", "dat", "m3d"]
        .iter()
        .any(|expected| extension.eq_ignore_ascii_case(expected))
    {
        return None;
    }
    Some(
        path.to_str()
            .ok_or_else(|| "The 3D LUT path contains invalid Unicode".into())
            .and_then(|path| {
                riina_core::video_filters::quick_lut_filter(path).map_err(|e| e.to_string())
            }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playable_drops_open_their_media_and_subtitle_only_drops_keep_the_current_file() {
        for paths in [
            vec!["Café 2.mkv"],
            vec!["https://example.test/live"],
            vec!["Café 2.mkv", "Café 10.mkv"],
        ] {
            let expected = paths.iter().map(|path| (*path).to_owned()).collect();
            let paths = paths
                .into_iter()
                .map(OsString::from)
                .chain([OsString::from("captions.srt")]);
            assert_eq!(
                prepare_drop(paths.collect(), || false),
                Ok(Drop::Play(expected))
            );
        }
        assert_eq!(
            prepare_drop(vec!["captions.srt".into(), "second.ass".into()], || false),
            Ok(Drop::Subtitles(vec![
                "captions.srt".into(),
                "second.ass".into()
            ]))
        );
    }

    #[test]
    fn subtitle_attachment_preserves_the_open_request_and_uses_the_captured_media() {
        i_slint_backend_testing::init_no_event_loop();
        let app = PlayerWindow::new().unwrap();
        app.set_media_request_generation(31);
        let (sender, recording) = CommandSender::recording(2);
        let subtitles = vec!["captions.srt".into(), "dialogue.ass".into()];
        assert!(
            Drop::Subtitles(subtitles.clone())
                .apply(&app, &sender, None, None)
                .is_err()
        );
        Drop::Subtitles(subtitles.clone())
            .apply(&app, &sender, Some(42), None)
            .unwrap();
        assert_eq!(app.get_media_request_generation(), 31);
        assert_eq!(
            recording.commands.try_recv(),
            Ok(Command::AddSubtitlesForMedia {
                generation: 42,
                paths: subtitles
            })
        );
        assert!(recording.commands.try_recv().is_err());
    }

    #[test]
    fn a_single_lut_drop_builds_a_filter_without_replacing_media() {
        i_slint_backend_testing::init_no_event_loop();
        let app = PlayerWindow::new().unwrap();
        let (sender, commands) = CommandSender::recording(2);
        let drop = OsString::from("Filter [café], tint.CUBE");
        let before = app.get_media_request_generation();
        dropped(&app, vec![drop.clone()], None, sender);
        assert_eq!(app.get_media_request_generation(), before);
        assert_eq!(
            commands.commands.try_recv(),
            Ok(Command::AddVideoFilter(
                riina_core::video_filters::quick_lut_filter(drop.to_str().unwrap()).unwrap()
            ))
        );
        assert!(commands.commands.try_recv().is_err());
        assert!(dropped_lut(&[drop, "video.mkv".into()]).is_none());
        for path in ["look.3dl", "look.dat", "look.m3d"] {
            assert!(dropped_lut(&[path.into()]).is_some_and(|filter| filter.is_ok()));
        }
    }
}
