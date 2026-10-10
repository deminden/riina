//! Same original fields through an alternative demuxer, without changing defaults.
use super::*;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

fn wait(receiver: &mpsc::Receiver<Playback>, predicate: impl Fn(&Playback) -> bool) -> Playback {
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        let state = receiver
            .recv_timeout(until.saturating_duration_since(Instant::now()))
            .unwrap();
        assert_eq!(state.error, None);
        if predicate(&state) {
            return state;
        }
    }
}

#[test]
#[ignore = "requires provisioned libmpv; compares lavf container field metadata"]
fn lavf_woven_field_captures() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let library = std::env::var_os("RIINA_TEST_MPV_LIBRARY")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("tools/mpv/libmpv-2.dll"));
    let output = root.join("tools/interlaced-lavf-test").join(format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&output).unwrap();
    let mut captures = Vec::new();
    for name in ["interlaced-tff.mkv", "interlaced-bff.mkv"] {
        let (sender, receiver) = mpsc::channel();
        let player = Player::start_options(
            &library,
            true,
            vec![
                ("demuxer".into(), "lavf".into()),
                ("hwdec".into(), "no".into()),
            ],
            move |state| {
                let _ = sender.send(state);
            },
        )
        .unwrap();
        player
            .send(Command::Load(
                root.join("tests/fixtures")
                    .join(name)
                    .to_string_lossy()
                    .into_owned(),
            ))
            .unwrap();
        wait(&receiver, |state| {
            state.phase == Phase::Playing && state.position.is_some()
        });
        player.send(Command::TogglePause).unwrap();
        let mut previous = wait(&receiver, |state| state.paused && state.position.is_some());
        assert!(
            previous
                .video_frame_info
                .as_ref()
                .is_some_and(|frame| frame.interlaced == Some(true)
                    && frame.top_field_first == Some(name.ends_with("-tff.mkv")))
        );
        for enabled in [false, true, false] {
            player.send(Command::Deinterlace(enabled)).unwrap();
            previous = wait(&receiver, |state| {
                state.paused
                    && !state.seeking
                    && state.deinterlace == Some(enabled)
                    && state.deinterlace_active == Some(enabled)
            });
            for _ in 0..6 {
                let from = previous.position.unwrap();
                player.send(Command::FrameStep).unwrap();
                let next = wait(&receiver, |state| {
                    state.paused
                        && !state.seeking
                        && state
                            .position
                            .is_some_and(|position| position > from + 0.005)
                });
                let expected = if enabled { 0.020 } else { 0.040 };
                assert!((next.position.unwrap() - from - expected).abs() < 0.002);
                let path = output.join(format!("capture-{}.png", captures.len()));
                player
                    .send(Command::ScreenshotToFile {
                        path: path.to_string_lossy().into_owned(),
                        content: riina_core::screenshot::Content::Video,
                    })
                    .unwrap();
                let captured = wait(&receiver, |state| {
                    state.completed_screenshots > next.completed_screenshots
                });
                assert_eq!(captured.position, next.position);
                captures.push(serde_json::json!({"fixture": name, "deinterlace": enabled,
                    "position": captured.position, "path": path}));
                previous = captured;
            }
        }
    }
    let manifest = output.join("captures.json");
    std::fs::write(&manifest, serde_json::to_vec_pretty(&captures).unwrap()).unwrap();
    println!("Forced lavf captures: {}", manifest.display());
}
