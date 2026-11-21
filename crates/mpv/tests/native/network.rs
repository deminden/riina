use super::*;

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

    let harness = Harness::start(&root).unwrap();
    harness
        .send(Command::Load(format!("http://{address}/moving-square.avi")))
        .unwrap();
    let loaded = harness.wait(|state| {
        state.error.is_some()
            || (state.phase == Phase::Playing
                && state.position.is_some_and(|position| position > 0.1))
    });
    assert_eq!(loaded.error, None);
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
    let mut range = None;
    loop {
        line.clear();
        if request.read_line(&mut line)? == 0 || line == "\r\n" {
            break;
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
