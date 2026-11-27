use super::*;

#[test]
#[ignore = "requires provisioned libmpv; verifies cancellation before a stream reaches the engine"]
fn cancelled_stream_before_native_load_retires_the_empty_player() {
    use riina_mpv::{StreamAttempt, StreamSource, StreamStatus};

    let harness = Harness::with_options(&fixture_root(), vec![]).unwrap();
    let sender = harness.player.command_sender();
    let attempt = StreamAttempt::default();
    sender.cancel_stream(&attempt);
    sender
        .open_stream(
            StreamSource {
                address: "http://127.0.0.1:9/unused.avi".into(),
                authenticated_address: None,
            },
            attempt.clone(),
        )
        .unwrap();
    let cancelled = harness.wait(Playback::idle_for_finalization);
    assert_eq!(attempt.status(), StreamStatus::Cancelled);
    assert_eq!(cancelled.generation, 0);
    assert_eq!(cancelled.error(), None);
    assert_eq!(
        cancelled.file_feedback,
        riina_core::FileFeedback::OpenCancelled {
            stream_attempt: attempt.id(),
        }
    );
    sender.finalize_idle_media(cancelled.generation, 1).unwrap();
    harness.wait(|state| state.stopped_by == Some(1));
}

#[test]
#[ignore = "requires provisioned libmpv; verifies HTTP media playback from a local server"]
fn http_url_opens_and_plays_a_local_media_fixture() {
    let root = fixture_root();
    let fixture = std::fs::read(root.join("tests/fixtures/moving-square.avi")).unwrap();
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let (stop, stopping) = mpsc::channel();
    let server = std::thread::spawn(move || serve_http_fixture(listener, fixture, stopping));

    let harness = Harness::with_options(&root, vec![("ao".into(), "null".into())]).unwrap();
    harness
        .send(Command::Load(format!("http://{address}/moving-square.avi")))
        .unwrap();
    let loaded = harness.wait(|state| {
        state.error().is_some()
            || (state.phase == Phase::Playing
                && state.position.is_some_and(|position| position > 0.1)
                && state.cache.forward_bytes > 0
                && state
                    .tracks
                    .iter()
                    .any(|track| track.kind == TrackKind::Audio && track.selected))
    });
    assert_eq!(loaded.error(), None);
    assert!(
        loaded
            .duration
            .is_some_and(|duration| (duration - 5.0).abs() < 0.1)
    );
    assert!(
        loaded
            .tracks
            .iter()
            .any(|track| track.kind == TrackKind::Audio && track.selected)
    );
    drop(harness);
    stop.send(()).unwrap();
    server.join().unwrap().unwrap();
}

#[test]
#[ignore = "requires provisioned libmpv; exercises tracked HTTP loads against a local server"]
fn stream_open_tracks_authentication_failure_and_cancellation() {
    use riina_mpv::{MpvPropertyFormat, StreamAttempt, StreamSource, StreamStatus};
    let root = fixture_root();
    let fixture = std::fs::read(root.join("tests/fixtures/moving-square.avi")).unwrap();
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let (stop, stopping) = mpsc::channel();
    let server = std::thread::spawn(move || serve_http_fixture(listener, fixture, stopping));
    let harness = Harness::with_options(&root, vec![("ao".into(), "null".into())]).unwrap();
    let sender = harness.player.command_sender();
    let open = |path: &str, authenticate: bool| {
        let attempt = StreamAttempt::default();
        let source = StreamSource {
            address: format!("http://{address}/{path}"),
            authenticated_address: authenticate
                .then(|| format!("http://stream:secret@{address}/{path}")),
        };
        sender.open_stream(source, attempt.clone()).unwrap();
        attempt
    };
    let rejected = open("auth.avi", false);
    let failure = harness
        .wait(|state| state.idle_for_finalization() && rejected.status() == StreamStatus::Failed);
    assert_eq!(
        failure.file_feedback,
        riina_core::FileFeedback::OpenFailed {
            stream_attempt: Some(rejected.id()),
        }
    );
    let accepted = open("auth.avi", true);
    let playing = harness.wait(|state| {
        accepted.status() == StreamStatus::Opened && state.position.is_some_and(|p| p >= 0.1)
    });
    assert!(
        playing
            .playlist
            .iter()
            .all(|entry| !entry.filename.contains('@'))
    );
    let public = format!("http://{address}/auth.avi");
    for property in ["stream-open-filename", "stream-path"] {
        let visible = sender
            .read_mpv_property(
                property,
                MpvPropertyFormat::Text,
                Instant::now() + Duration::from_secs(2),
            )
            .unwrap();
        assert_eq!(visible.as_str(), Some(public.as_str()));
    }
    let slow = open("stall.avi", false);
    harness.wait(|state| {
        state.generation > playing.generation
            && state.phase == Phase::Loading
            && state.playlist.iter().any(|entry| {
                Some(entry.id) == state.active_playlist_entry
                    && entry.filename.ends_with("/stall.avi")
            })
    });
    sender.cancel_stream(&slow);
    let cancelled = harness.wait(Playback::idle_for_finalization);
    assert_eq!(slow.status(), StreamStatus::Cancelled);
    assert_eq!(
        cancelled.file_feedback,
        riina_core::FileFeedback::OpenCancelled {
            stream_attempt: slow.id(),
        }
    );
    assert_eq!(cancelled.error(), None);
    let next = StreamAttempt::default();
    sender
        .open_stream_with_options(
            StreamSource {
                address: format!("http://{address}/moving-square.avi"),
                authenticated_address: None,
            },
            Some(riina_core::MediaLink {
                start: Some(1.25),
                pause: Some(true),
                speed: Some(1.25),
                volume: Some(31.0),
                mute: Some(true),
                ..Default::default()
            }),
            next.clone(),
        )
        .unwrap();
    harness.wait(|state| {
        next.status() == StreamStatus::Opened
            && state.phase == Phase::Paused
            && state
                .position
                .is_some_and(|position| (position - 1.25).abs() < 0.15)
            && (state.speed - 1.25).abs() < 0.01
            && (state.volume - 31.0).abs() < 0.01
            && state.muted
    });

    let grouped = StreamAttempt::default();
    let lead = format!("http://{address}/opening.avi");
    let remote_following = format!("http://{address}/closing.avi");
    let local_following = root.join("tests/fixtures/moving-square.avi");
    let local_following = local_following.to_str().unwrap();
    let following = [local_following.to_owned(), remote_following.clone()];
    sender.send(Command::SetPauseWhenOpen(true)).unwrap();
    sender
        .open_stream_playlist(
            StreamSource {
                address: lead.clone(),
                authenticated_address: None,
            },
            &following,
            true,
            grouped.clone(),
        )
        .unwrap();
    let group = harness.wait(|state| {
        grouped.status() == StreamStatus::Opened
            && state.phase == Phase::Paused
            && state.playlist.len() == 3
            && state.explicit_playlist && state.playlist_feedback.is_some_and(|feedback| {
            matches!(feedback.change, riina_core::PlaylistChange::Added(count) if count.get() == 3)
                && feedback.generation == state.generation
        })
    });
    assert!(group.playlist[0].playing || group.playlist[0].current);
    assert_eq!(group.playlist[0].filename, lead);
    assert_eq!(
        std::fs::canonicalize(&group.playlist[1].filename).unwrap(),
        std::fs::canonicalize(local_following).unwrap()
    );
    assert_eq!(group.playlist[2].filename, remote_following);
    drop(harness);
    stop.send(()).unwrap();
    server.join().unwrap().unwrap();
}

#[expect(
    clippy::expect_used,
    reason = "Failed fixture responses must be visible to the native test"
)]
fn serve_http_fixture(
    listener: TcpListener,
    fixture: Vec<u8>,
    stopping: mpsc::Receiver<()>,
) -> std::io::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(12);
    let fixture = std::sync::Arc::new(fixture);
    let mut connections = Vec::new();
    loop {
        if !matches!(stopping.try_recv(), Err(mpsc::TryRecvError::Empty)) {
            break;
        }
        match listener.accept() {
            Ok((stream, _)) => {
                // On Windows an accepted socket can retain the listener's
                // nonblocking mode. The request worker uses blocking reads with
                // a timeout, so explicitly restore that contract here.
                stream.set_nonblocking(false)?;
                // A decoder may open its next range while an earlier response
                // is still buffered. A blocked writer must not block acceptance.
                let fixture = fixture.clone();
                connections.push(std::thread::spawn(move || {
                    write_http_fixture(stream, &fixture)
                }));
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "libmpv did not request the HTTP fixture",
                    ));
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(error) => return Err(error),
        }
    }
    for connection in connections {
        let result = connection
            .join()
            .expect("HTTP fixture connection worker panicked");
        if let Err(error) = result
            && !matches!(
                error.kind(),
                std::io::ErrorKind::BrokenPipe
                    | std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::ConnectionAborted
            )
        {
            return Err(error);
        }
    }
    Ok(())
}

fn write_http_fixture(stream: TcpStream, fixture: &[u8]) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    let mut request = BufReader::new(stream);
    let mut line = String::new();
    request.read_line(&mut line)?;
    let path = line
        .split_ascii_whitespace()
        .nth(1)
        .unwrap_or_default()
        .to_owned();
    let mut range = None;
    let mut authorized = false;
    loop {
        line.clear();
        if request.read_line(&mut line)? == 0 || line == "\r\n" {
            break;
        }
        if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("authorization")
        {
            authorized = value.trim() == "Basic c3RyZWFtOnNlY3JldA==";
        }
        if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("range")
            && let Some(value) = value.trim().strip_prefix("bytes=")
            && let Some((start, end)) = value.split_once('-')
            && let Ok(start) = start.parse::<usize>()
        {
            let end = end.parse::<usize>().unwrap_or(fixture.len() - 1);
            range = Some((start, end.min(fixture.len() - 1)));
        }
    }
    if path == "/auth.avi" && !authorized {
        return request.get_mut().write_all(
            b"HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm=\"Riina test\"\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        );
    }
    if path == "/stall.avi" {
        std::thread::sleep(Duration::from_secs(2));
    }
    let (status, body, content_range) = match range {
        Some((start, end)) if start <= end && start < fixture.len() => (
            "206 Partial Content",
            &fixture[start..=end],
            format!("Content-Range: bytes {start}-{end}/{}\r\n", fixture.len()),
        ),
        Some(_) => (
            "416 Range Not Satisfiable",
            &[][..],
            format!("Content-Range: bytes */{}\r\n", fixture.len()),
        ),
        None => ("200 OK", fixture, String::new()),
    };
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: video/x-msvideo\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\n{content_range}Connection: close\r\n\r\n",
        body.len()
    );
    let stream = request.get_mut();
    stream.write_all(response.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()
}
