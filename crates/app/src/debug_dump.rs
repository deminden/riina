use crate::{PlaybackStore, PlayerWindow};
use riina_core::{Playback, sync::lock};
use riina_ui::PlayerMenuActions;
use slint::ComponentHandle;
use std::{
    cell::RefCell,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    rc::{Rc, Weak},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_COLLECTION_ITEMS: usize = 512;
const MAX_FIELD_CHARS: usize = 1024;

#[derive(Default)]
pub(crate) struct ApplicationDebugInformation {
    players: RefCell<Vec<Weak<PlaybackStore>>>,
}

pub(crate) struct Registration {
    source: RefCell<Option<Rc<PlaybackStore>>>,
}

impl Registration {
    pub(crate) fn retire(&self) {
        self.source.take();
    }
}

impl ApplicationDebugInformation {
    pub(crate) fn connect(
        self: &Rc<Self>,
        app: &PlayerWindow,
        playback: &PlaybackStore,
    ) -> Registration {
        // The session can close while Slint still retains its component.
        let source = Rc::new(playback.clone());
        let mut players = self.players.borrow_mut();
        players.retain(|player| player.strong_count() != 0);
        players.push(Rc::downgrade(&source));
        drop(players);

        let application = self.clone();
        let player = app.as_weak();
        app.global::<PlayerMenuActions>()
            .on_dump_debug_info(move || application.dump(Some(&player)));
        Registration {
            source: RefCell::new(Some(source)),
        }
    }

    pub(crate) fn dump(&self, owner: Option<&slint::Weak<PlayerWindow>>) {
        let snapshots = self.snapshots();
        let recipient = owner.cloned();
        let started = crate::background::spawn_named("debug-information", move || {
            let result = write_download(&snapshots);
            let _ = slint::invoke_from_event_loop(move || report(recipient.as_ref(), result));
        });
        if let Err(error) = started {
            report(owner, Err(error));
        }
    }

    fn snapshots(&self) -> Vec<Arc<Playback>> {
        self.players
            .borrow()
            .iter()
            .filter_map(Weak::upgrade)
            .map(|player| lock(&player).clone())
            .collect()
    }
}

fn report(owner: Option<&slint::Weak<PlayerWindow>>, result: io::Result<PathBuf>) {
    let app = owner.and_then(slint::Weak::upgrade);
    if owner.is_some() && app.is_none() {
        return;
    }
    match result {
        Ok(path) => {
            let _ = rfd::MessageDialog::new()
                .set_title("Debug Info Saved")
                .set_description(format!(
                    "Debug information was saved to {}. It may contain media filenames and URLs. Review it before sharing.",
                    path.display()
                ))
                .set_buttons(rfd::MessageButtons::Ok)
                .show();
        }
        Err(error) => {
            let message = format!("Could not save debug information: {error}");
            if let Some(app) = app.as_ref() {
                crate::diagnostics::report(app, message);
            } else {
                crate::diagnostics::warn!("{message}");
                let _ = rfd::MessageDialog::new()
                    .set_title("Debug Info")
                    .set_description(message)
                    .set_buttons(rfd::MessageButtons::Ok)
                    .show();
            }
        }
    }
}

fn write_download(snapshots: &[Arc<Playback>]) -> io::Result<PathBuf> {
    let directory = dirs::download_dir().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "the system Downloads folder could not be located",
        )
    })?;
    write_to(&directory, snapshots)
}

fn write_to(directory: &Path, snapshots: &[Arc<Playback>]) -> io::Result<PathBuf> {
    fs::create_dir_all(directory)?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = directory.join(format!("riina-debug-dump-{stamp}.txt"));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;

    let result = write_report(&mut file, snapshots);
    drop(file);
    if let Err(error) = result {
        // A failed write must not look like a completed report in Downloads.
        let _ = fs::remove_file(&path);
        return Err(error);
    }
    Ok(path)
}

fn write_report(file: &mut impl Write, snapshots: &[Arc<Playback>]) -> io::Result<()> {
    writeln!(
        file,
        "Riina {} debug information",
        env!("CARGO_PKG_VERSION")
    )?;
    writeln!(
        file,
        "OS: {} {}",
        std::env::consts::OS,
        std::env::consts::ARCH
    )?;
    writeln!(file, "Players: {}", snapshots.len())?;
    for (index, snapshot) in snapshots.iter().enumerate() {
        writeln!(file, "\nPlayer {} snapshot:", index + 1)?;
        write_player(file, snapshot)?;
    }
    file.flush()?;
    Ok(())
}

fn write_player(file: &mut impl Write, playback: &Playback) -> io::Result<()> {
    writeln!(file, "Generation: {}", playback.generation)?;
    writeln!(file, "Phase: {:?}", playback.phase)?;
    writeln!(
        file,
        "libmpv version: {:?}",
        clipped_option(&playback.mpv_version)
    )?;
    writeln!(
        file,
        "FFmpeg version: {:?}",
        clipped_option(&playback.ffmpeg_version)
    )?;
    writeln!(file, "Title: {:?}", media_reference(&playback.title))?;
    writeln!(file, "Artist: {:?}", clipped_option(&playback.artist))?;
    writeln!(file, "Album: {:?}", clipped_option(&playback.album))?;
    writeln!(file, "Position: {:?}", playback.position)?;
    writeln!(file, "Duration: {:?}", playback.duration)?;
    writeln!(file, "Paused: {}", playback.paused)?;
    writeln!(file, "Buffering: {}", playback.buffering)?;
    writeln!(file, "Volume: {}", playback.volume)?;
    writeln!(file, "Muted: {}", playback.muted)?;
    writeln!(file, "Speed: {}", playback.speed)?;
    writeln!(
        file,
        "Audio output: {:?}",
        clipped_option(&playback.audio_output)
    )?;
    writeln!(
        file,
        "Audio device: {:?}",
        clipped_option(&playback.audio_device)
    )?;
    writeln!(
        file,
        "Hardware decoder: {:?}",
        clipped_option(&playback.hardware_decoder)
    )?;
    writeln!(file, "Playback error: {:?}", playback.error().map(clipped))?;
    if let Some(video) = &playback.video {
        writeln!(
            file,
            "\nVideo: {}×{}, display {:?}×{:?}, rotate {:?}, pixel format {:?}",
            video.w,
            video.h,
            video.dw,
            video.dh,
            video.rotate,
            clipped_option(&video.pixelformat)
        )?;
    } else {
        writeln!(file, "\nVideo: none")?;
    }
    writeln!(
        file,
        "\nTracks: {} total; showing at most {MAX_COLLECTION_ITEMS}",
        playback.tracks.len()
    )?;
    for track in playback.tracks.iter().take(MAX_COLLECTION_ITEMS) {
        writeln!(
            file,
            "  id={} type={:?} selected={} external={} title={:?} language={:?} codec={:?} external_file={:?}",
            track.id,
            track.kind,
            track.selected,
            track.external,
            clipped_option(&track.title),
            clipped_option(&track.lang),
            clipped_option(&track.codec),
            track.external_filename.as_deref().map(media_reference)
        )?;
    }
    writeln!(
        file,
        "\nChapters: {} total; showing at most {MAX_COLLECTION_ITEMS}",
        playback.chapters.len()
    )?;
    for chapter in playback.chapters.iter().take(MAX_COLLECTION_ITEMS) {
        writeln!(
            file,
            "  time={} title={:?}",
            chapter.time,
            chapter.title.as_deref().map(clipped)
        )?;
    }
    writeln!(
        file,
        "\nPlaylist: {} total; showing at most {MAX_COLLECTION_ITEMS}; filenames may be private",
        playback.playlist.len()
    )?;
    for entry in playback.playlist.iter().take(MAX_COLLECTION_ITEMS) {
        writeln!(
            file,
            "  id={} current={} playing={} filename={:?} title={:?}",
            entry.id,
            entry.current,
            entry.playing,
            media_reference(&entry.filename),
            entry.title.as_deref().map(clipped)
        )?;
    }
    Ok(())
}

fn media_reference(value: &str) -> String {
    if let Ok(mut url) = url::Url::parse(value)
        && (!url.username().is_empty() || url.password().is_some())
    {
        let _ = url.set_username("");
        let _ = url.set_password(None);
        return clipped(url.as_str());
    }
    clipped(value)
}

fn clipped_option(value: &Option<String>) -> Option<String> {
    value.as_deref().map(clipped)
}

fn clipped(value: &str) -> String {
    let mut characters = value.chars();
    let output = characters
        .by_ref()
        .take(MAX_FIELD_CHARS)
        .collect::<String>();
    if characters.next().is_some() {
        format!("{output}… [truncated]")
    } else {
        output
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_FIELD_CHARS, media_reference, write_to};
    use riina_core::{Phase, Playback, PlaylistEntry};
    use riina_test_support::TempDir;
    use std::fs;

    #[test]
    fn debug_dump_saves_useful_player_state_and_warnable_media_paths() {
        let directory = TempDir::new("debug-dump").unwrap();
        let mut playlist = vec![PlaylistEntry {
            id: 1,
            filename: "C:\\Private\\family-video.mkv".into(),
            title: Some("Family video".into()),
            current: true,
            playing: true,
        }];
        playlist.extend((2..=513).map(|id| PlaylistEntry {
            id,
            filename: if id == 513 {
                "private-tail.mkv".into()
            } else {
                format!("clip-{id}.mkv")
            },
            title: None,
            current: false,
            playing: false,
        }));
        let playback = Playback {
            generation: 7,
            phase: Phase::Paused,
            mpv_version: Some("mpv test".into()),
            title: "A".repeat(MAX_FIELD_CHARS + 32),
            playlist: playlist.into(),
            ..Playback::default()
        };

        let path = write_to(directory.path(), &[std::sync::Arc::new(playback)]).unwrap();
        assert!(
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("riina-debug-dump-")
        );
        let contents = fs::read_to_string(&path).unwrap();
        assert!(contents.contains("mpv test"));
        assert!(contents.contains("Paused"));
        assert!(contents.contains("family-video.mkv"));
        assert!(contents.contains("513 total; showing at most 512"));
        assert!(!contents.contains("private-tail.mkv"));
        assert!(contents.contains("[truncated]"));
    }
    #[test]
    fn one_report_keeps_player_order_and_removes_url_credentials() {
        let directory = TempDir::new("multiple-player-dump").unwrap();
        let snapshots = [
            std::sync::Arc::new(Playback {
                title: "First player".into(),
                ..Playback::default()
            }),
            std::sync::Arc::new(Playback {
                title: "https://viewer:password@media.example/play.mkv".into(),
                ..Playback::default()
            }),
        ];
        let path = write_to(directory.path(), &snapshots).unwrap();
        let result = fs::read_to_string(path).unwrap();
        assert!(result.contains("Players: 2"));
        assert!(
            result.find("First player").unwrap()
                < result.find("https://media.example/play.mkv").unwrap()
        );
        assert!(!result.contains("password"));
        assert_eq!(
            media_reference("C:\\Local\\file.mkv"),
            "C:\\Local\\file.mkv"
        );
    }
    #[test]
    fn closed_player_windows_are_excluded_even_with_retained_observations()
    -> Result<(), Box<dyn std::error::Error>> {
        use super::{ApplicationDebugInformation, PlayerWindow};
        use std::{
            rc::Rc,
            sync::{Arc, Mutex},
        };

        i_slint_backend_testing::init_no_event_loop();
        let first = PlayerWindow::new()?;
        let second = PlayerWindow::new()?;
        let information = Rc::new(ApplicationDebugInformation::default());
        let mut startup_report = Vec::new();
        super::write_report(&mut startup_report, &information.snapshots())?;
        assert!(String::from_utf8(startup_report)?.contains("Players: 0"));
        let observation = |title: &str| {
            Arc::new(Mutex::new(Arc::new(Playback {
                title: title.into(),
                ..Playback::default()
            })))
        };
        let first_observation = observation("First");
        let second_observation = observation("Second");
        let first_session = information.connect(&first, &first_observation);
        let _second_session = information.connect(&second, &second_observation);
        assert_eq!(information.snapshots().len(), 2);
        first_session.retire();
        let live = information.snapshots();
        assert_eq!(live.len(), 1);
        assert_eq!(live[0].title, "Second");
        assert_eq!(riina_core::sync::lock(&first_observation).title, "First");
        Ok(())
    }
}
