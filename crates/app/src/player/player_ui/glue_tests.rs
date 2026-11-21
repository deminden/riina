//! Characterise the real player callbacks without opening a native window or libmpv.

use super::*;
use riina_mpv::{CommandSender, RecordingEngine};
use std::{fs, io, sync::mpsc::TryRecvError};

struct TestProfile(PathBuf);

impl TestProfile {
    fn new() -> io::Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        loop {
            let id = NEXT.fetch_add(1, Ordering::Relaxed);
            let path =
                std::env::temp_dir().join(format!("riina-player-glue-{}-{id}", std::process::id()));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
    }
}

impl Drop for TestProfile {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub(crate) struct Harness {
    pub(crate) app: PlayerWindow,
    pub(crate) session: UiSession,
    pub(crate) engine: Rc<RecordingEngine>,
    pub(crate) sender: CommandSender,
}

impl Harness {
    pub(crate) fn start(
        path: PathBuf,
        capture_before_navigation: bool,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let loaded = crate::preference_load::load(&path)?;
        let app = PlayerWindow::new()?;
        app.set_native_video(capture_before_navigation);
        let (commands, engine) = CommandSender::recording(64);
        let engine = Rc::new(engine);
        let session = configure_with_engine(
            &app,
            None,
            Vec::new(),
            None,
            None,
            loaded,
            EngineStart::Recording {
                engine: engine.clone(),
                commands: commands.clone(),
                profile: path,
                capture_before_navigation,
            },
        )?;
        // These tests exercise subsequent application intents independently of startup.
        assert!(engine.commands.try_iter().count() > 0);
        Ok(Self {
            app,
            session,
            engine,
            sender: commands,
        })
    }

    #[track_caller]
    fn expect(&self, command: Command) {
        assert_eq!(self.engine.commands.try_recv(), Ok(command));
        self.expect_idle();
    }

    #[track_caller]
    fn expect_idle(&self) {
        assert_eq!(self.engine.commands.try_recv(), Err(TryRecvError::Empty));
    }

    fn publish(&self, state: Playback) -> Result<(), Box<dyn std::error::Error>> {
        self.engine.publish(state);
        // The observer posts to the real Slint event queue. Quit after it drains
        // without advancing clocks, opening a native surface, or sleeping.
        slint::invoke_from_event_loop(|| {
            let _ = slint::quit_event_loop();
        })?;
        slint::run_event_loop_until_quit()?;
        Ok(())
    }
}

#[test]
fn player_callbacks_and_playback_projection_work_headlessly() -> io::Result<()> {
    // Slint registers a process-wide proxy for a backend with an event loop.
    // Keep it out of the process running the other widget and app tests.
    let module = module_path!()
        .split_once("::")
        .map_or(module_path!(), |(_, path)| path);
    let worker = format!("{module}::player_glue_worker");
    let profile = TestProfile::new()?;
    let mut child = std::process::Command::new(std::env::current_exe()?)
        .args(["--ignored", "--exact", &worker])
        .env("RIINA_TEST_GLUE_PROFILE", &profile.0)
        .spawn()?;
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Some(status) = child.try_wait()? {
            assert!(
                status.success(),
                "Headless player process exited with {status}"
            );
            return Ok(());
        }
        if Instant::now() >= deadline {
            child.kill()?;
            child.wait()?;
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Headless player test timed out",
            ));
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
#[ignore = "Uses Slint's process-wide event loop in an isolated test process"]
fn player_glue_worker() -> Result<(), Box<dyn std::error::Error>> {
    i_slint_backend_testing::init_integration_test_with_mock_time();
    let profile = PathBuf::from(
        std::env::var_os("RIINA_TEST_GLUE_PROFILE").ok_or("Expected an isolated test profile")?,
    );
    let harness = Harness::start(profile.join("shared/preferences.json"), false)?;
    exercise_transport_and_tracks(&harness);
    exercise_dialogs_and_relative_controls(&harness);
    exercise_observation_and_volume(&harness)?;
    drop(harness);

    let capture = Harness::start(profile.join("capture/preferences.json"), true)?;
    capture.app.invoke_playlist_next();
    capture.expect(Command::CaptureTransitionFrame);
    capture.publish(Playback {
        generation: 1,
        transition_frame_generation: Some(1),
        transition_frame_captured_at: Some(Instant::now()),
        ..Playback::default()
    })?;
    capture.expect(Command::VideoViewport(None));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(80));
    capture.expect(Command::PlaylistNext);
    capture.app.invoke_close_player();
    capture.expect(Command::Stop);
    Ok(())
}

fn exercise_transport_and_tracks(harness: &Harness) {
    let app = &harness.app;
    app.invoke_toggle_pause();
    harness.expect(Command::TogglePause);
    app.invoke_stop();
    harness.expect(Command::Stop);
    app.invoke_seek(17.5);
    harness.expect(Command::Seek {
        seconds: 17.5,
        absolute: true,
    });
    app.invoke_seek_relative(-5.0);
    harness.expect(Command::Seek {
        seconds: -5.0,
        absolute: false,
    });
    for (category, kind) in [
        (1, TrackKind::Audio),
        (2, TrackKind::Sub),
        (6, TrackKind::Video),
    ] {
        app.invoke_select_item(category, "9007199254740993".into());
        harness.expect(Command::SelectTrack {
            kind,
            id: Some(9007199254740993),
        });
    }
    app.invoke_select_item(5, "no".into());
    harness.expect(Command::SelectSecondarySubtitle(None));
    app.invoke_playlist_next();
    harness.expect(Command::PlaylistNext);
    app.invoke_set_ab_loop_point(0, 1.25);
    harness.expect_idle();
}

fn exercise_dialogs_and_relative_controls(harness: &Harness) {
    let app = &harness.app;
    app.set_position_label("2:05".into());
    app.invoke_open_jump_time_dialog();
    assert!(app.get_jump_time_dialog_visible());
    assert_eq!(app.get_jump_time_draft(), "2:05");
    app.invoke_jump_to_time("invalid".into());
    assert!(app.get_jump_time_dialog_visible());
    assert_eq!(
        app.get_jump_time_error(),
        "Enter a valid time such as 20:35 or 1:02:03.500."
    );
    harness.expect_idle();
    app.invoke_jump_to_time("2:05.500".into());
    harness.expect(Command::Seek {
        seconds: 125.5,
        absolute: true,
    });
    assert!(!app.get_jump_time_dialog_visible());
    assert_eq!(app.get_jump_time_error(), "");

    app.set_url_dialog_draft("previous".into());
    app.invoke_open_url_dialog();
    assert!(app.get_url_dialog_visible());
    assert_eq!(app.get_url_dialog_draft(), "");
    app.set_url_dialog_visible(false);
    app.invoke_browse_files(-1);
    harness.expect_idle();

    app.invoke_speed(1.5);
    harness.expect(Command::MultiplySpeed(1.5));
    app.invoke_adjust_volume(-3.0);
    harness.expect(Command::AdjustVolume {
        delta: -3.0,
        boost: false,
    });
}

fn exercise_observation_and_volume(harness: &Harness) -> Result<(), Box<dyn std::error::Error>> {
    let app = &harness.app;
    harness.publish(Playback {
        generation: 1,
        phase: Phase::Paused,
        paused: true,
        position: Some(1.25),
        duration: Some(10.0),
        volume: 45.0,
        title: "Headless fixture".into(),
        ..Playback::default()
    })?;
    harness.expect(Command::SetHdrEnabledWithIcc {
        enabled: false,
        load_icc_profile: true,
    });
    assert!(app.get_loaded());
    assert!(app.get_paused());
    assert_eq!(app.get_position(), 1.25);
    assert_eq!(app.get_duration(), 10.0);
    assert_eq!(app.get_volume(), 45.0);
    assert_eq!(app.get_media_title(), "Headless fixture");

    app.invoke_set_ab_loop_point(0, 1.25);
    harness.expect(Command::SetAbLoopPoint {
        point: riina_core::AbLoopPoint::A,
        value: Some(1.25),
    });
    app.invoke_set_volume(140.0);
    harness.expect(Command::Volume(100.0));
    app.invoke_set_volume_boost(true);
    harness.expect(Command::SetVolumeBoost(true));
    app.invoke_set_volume(140.0);
    harness.expect(Command::Volume(140.0));
    Ok(())
}
