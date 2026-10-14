use riina_core::Playback;
use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_COLLECTION_ITEMS: usize = 512;
const MAX_FIELD_CHARS: usize = 1024;

pub(crate) fn write_download(playback: &Playback) -> io::Result<PathBuf> {
    let directory = dirs::download_dir().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "the system Downloads folder could not be located",
        )
    })?;
    write_to(&directory, playback)
}

fn write_to(directory: &Path, playback: &Playback) -> io::Result<PathBuf> {
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
    writeln!(file, "\nPlayer snapshot:")?;
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
    writeln!(file, "Title: {:?}", clipped(&playback.title))?;
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
    writeln!(
        file,
        "Playback error: {:?}",
        clipped_option(&playback.error)
    )?;
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
            clipped_option(&track.external_filename)
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
            clipped(&entry.filename),
            entry.title.as_deref().map(clipped)
        )?;
    }
    file.flush()?;
    Ok(path)
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
    use super::{MAX_FIELD_CHARS, write_to};
    use riina_core::{Phase, Playback, PlaylistEntry};
    use std::{fs, time::SystemTime};

    #[test]
    fn debug_dump_saves_useful_player_state_and_warnable_media_paths() {
        let directory = std::env::temp_dir().join(format!(
            "riina-debug-dump-test-{}",
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
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
            playlist,
            ..Playback::default()
        };

        let path = write_to(&directory, &playback).unwrap();
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

        fs::remove_dir_all(directory).unwrap();
    }
}
