//! Characterise the real player callbacks without opening a native window or libmpv.

use super::{engine_start::EngineOptions, *};
use crate::CatalogSelection;
use riina_core::{Command, Preferences, TrackKind};
use riina_mpv::{CommandSender, RecordingEngine};
use riina_test_support::TempDir;
use riina_ui::PlayerMenuActions;
use slint::ComponentHandle;
use std::{
    cell::RefCell,
    io,
    path::PathBuf,
    rc::Rc,
    sync::mpsc::TryRecvError,
    time::{Duration, Instant},
};

pub(crate) struct Harness {
    pub(crate) app: PlayerWindow,
    pub(crate) session: UiSession,
    pub(crate) engine: Rc<RecordingEngine>,
    pub(crate) sender: CommandSender,
    startup_options: Rc<RefCell<EngineOptions>>,
    startup_commands: Vec<Command>,
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
        let startup_options = Rc::new(RefCell::new(Vec::new()));
        let session = configure_with_engine(
            &app,
            None,
            startup::InitialMedia::default(),
            None,
            loaded,
            SessionStartup {
                media_geometry: Default::default(),
                engine: EngineStart::Recording {
                    engine: engine.clone(),
                    commands: commands.clone(),
                    profile: path,
                    capture_before_navigation,
                    startup_options: startup_options.clone(),
                },
            },
        )?;
        // These tests exercise subsequent application intents independently of startup.
        let startup_commands: Vec<_> = engine.commands.try_iter().collect();
        assert!(!startup_commands.is_empty());
        Ok(Self {
            app,
            session,
            engine,
            sender: commands,
            startup_options,
            startup_commands,
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

pub(super) struct DropProbe(&'static str, Rc<RefCell<Vec<&'static str>>>);

impl Drop for DropProbe {
    fn drop(&mut self) {
        self.1.borrow_mut().push(self.0);
    }
}

fn exercise_teardown(profile: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let order = Rc::new(RefCell::new(Vec::new()));
    let Harness { mut session, .. } =
        Harness::start(profile.join("teardown/preferences.json"), false)?;
    session.player = Some(SessionResource::Recorded(DropProbe(
        "player",
        order.clone(),
    )));
    session._presentation = Some(SessionResource::Recorded(DropProbe(
        "presentation",
        order.clone(),
    )));
    drop(session);
    assert_eq!(&*order.borrow(), &["player", "presentation"]);
    Ok(())
}

#[test]
fn player_callbacks_and_playback_projection_work_headlessly() -> io::Result<()> {
    // Slint registers a process-wide proxy for a backend with an event loop.
    // Keep it out of the process running the other widget and app tests.
    let module = module_path!()
        .split_once("::")
        .map_or(module_path!(), |(_, path)| path);
    let worker = format!("{module}::player_glue_worker");
    let profile = TempDir::new("player-glue")?;
    let mut child = std::process::Command::new(std::env::current_exe()?)
        .args(["--ignored", "--exact", &worker])
        .env("RIINA_TEST_GLUE_PROFILE", profile.path())
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
    exercise_startup(&profile)?;
    exercise_teardown(&profile)?;
    let harness = Harness::start(profile.join("shared/preferences.json"), false)?;
    exercise_transport_and_tracks(&harness);
    exercise_transport_settings(&harness);
    exercise_dialogs_and_relative_controls(&harness);
    exercise_observation_and_volume(&harness)?;
    drop(harness);

    let capture = Harness::start(profile.join("capture/preferences.json"), true)?;
    capture
        .app
        .global::<PlayerMenuActions>()
        .invoke_playlist_next();
    capture.expect(Command::CaptureTransitionFrame);
    capture.publish(Playback {
        generation: 1,
        transition_frame: riina_core::FrameSlot {
            generation: Some(1),
            captured_at: Some(Instant::now()),
            ..Default::default()
        },
        ..Playback::default()
    })?;
    capture.expect(Command::VideoViewport(None));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(80));
    capture.expect(Command::PlaylistNext);
    capture
        .app
        .global::<PlayerMenuActions>()
        .invoke_close_player();
    capture.expect(Command::Stop);
    Ok(())
}

fn exercise_startup(profile: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let path = profile.join("configured/preferences.json");
    let prefs = Preferences {
        resume_last_position: false,
        pause_when_open: true,
        video_threads: 4,
        volume_boost: true,
        hdr_enabled: false,
        advanced: riina_core::advanced_preferences::Settings {
            enabled: true,
            use_mpv_osd: true,
            user_options: vec![riina_core::advanced_preferences::MpvOption {
                name: "vd-lavc-threads".into(),
                value: "2".into(),
            }],
            ..Default::default()
        },
        ..Default::default()
    };
    crate::preference_store::Writer::new(path.clone(), Preferences::default()).save(&prefs)?;
    let harness = Harness::start(path.clone(), false)?;
    let options = harness.startup_options.borrow();
    assert_eq!(
        &options[..3],
        &[
            (
                "watch-later-directory".into(),
                path.with_file_name("watch-later")
                    .to_string_lossy()
                    .into_owned()
            ),
            ("save-position-on-quit".into(), "no".into()),
            ("resume-playback".into(), "no".into()),
        ]
    );
    let decoder_options: Vec<_> = options
        .iter()
        .filter(|(option, _)| option == "vd-lavc-threads")
        .map(|(_, value)| value.as_str())
        .collect();
    assert_eq!(decoder_options, ["4", "2"]);
    assert_eq!(
        &options[options.len() - 2..],
        &[
            ("osd-level".into(), "1".into()),
            ("vd-lavc-threads".into(), "2".into())
        ]
    );
    let controls = &harness.startup_commands;
    assert_eq!(
        &controls[..3],
        &[
            Command::SetVolumeBoost(true),
            Command::SetPauseWhenOpen(true),
            Command::HardwareDecoding(prefs.hardware_decoder),
        ]
    );
    Ok(())
}

fn exercise_transport_settings(harness: &Harness) {
    let app = &harness.app;
    app.global::<riina_ui::PlayerControlActions>()
        .invoke_set_paused(false);
    harness.expect(Command::SetPaused(false));
    app.global::<PlayerMenuActions>().invoke_frame_step();
    harness.expect(Command::FrameStep);
    app.global::<PlayerMenuActions>().invoke_frame_back_step();
    harness.expect(Command::FrameBackStep);
    app.global::<PlayerMenuActions>().invoke_adjust_chapter(-1);
    harness.expect(Command::AdjustChapter(-1));
    app.global::<riina_ui::PlayerControlActions>()
        .invoke_subtitle_seek(1);
    harness.expect(Command::SubtitleSeek(1));
    app.global::<PlayerMenuActions>().invoke_reset_speed();
    harness.expect(Command::Speed(1.0));
    app.global::<riina_ui::PlayerSidebarActions>()
        .invoke_set_speed_slider(16.0);
    harness.expect(Command::Speed(4.0));
    assert!(
        app.global::<riina_ui::PlayerSidebarActions>()
            .invoke_set_speed_text("1.2345675".into())
    );
    harness.expect(Command::Speed(1.234567));
    assert!(
        !app.global::<riina_ui::PlayerSidebarActions>()
            .invoke_set_speed_text("invalid".into())
    );
    assert_eq!(app.get_status(), "Enter a numeric playback speed");
    harness.expect_idle();
    app.global::<PlayerMenuActions>().invoke_cycle_ab_loop();
    harness.expect(Command::CycleAbLoop);
    app.global::<PlayerMenuActions>().invoke_toggle_file_loop();
    harness.expect(Command::ToggleFileLoop);
    app.global::<riina_ui::PlayerControlActions>()
        .invoke_execute_mpv_input_action("cycle pause".into());
    harness.expect(Command::MpvInputAction("cycle pause".into()));
}

fn exercise_transport_and_tracks(harness: &Harness) {
    let app = &harness.app;
    app.global::<PlayerMenuActions>().invoke_toggle_pause();
    harness.expect(Command::TogglePause);
    app.global::<PlayerMenuActions>().invoke_stop();
    harness.expect(Command::Stop);
    app.global::<PlayerMenuActions>().invoke_seek(17.5);
    harness.expect(Command::Seek {
        seconds: 17.5,
        absolute: true,
    });
    app.global::<PlayerMenuActions>().invoke_seek_relative(-5.0);
    harness.expect(Command::Seek {
        seconds: -5.0,
        absolute: false,
    });
    for (category, kind) in [
        (CatalogSelection::AudioTrack, TrackKind::Audio),
        (CatalogSelection::PrimarySubtitle, TrackKind::Sub),
        (CatalogSelection::VideoTrack, TrackKind::Video),
    ] {
        app.global::<PlayerMenuActions>()
            .invoke_select_item(category, "9007199254740993".into());
        harness.expect(Command::SelectTrack {
            kind,
            id: Some(9007199254740993),
        });
    }
    app.global::<PlayerMenuActions>()
        .invoke_select_item(CatalogSelection::SecondarySubtitle, "no".into());
    harness.expect(Command::SelectSecondarySubtitle(None));
    app.global::<PlayerMenuActions>().invoke_playlist_next();
    harness.expect(Command::PlaylistNext);
    app.global::<riina_ui::PlayerControlActions>()
        .invoke_set_ab_loop_point(0, 1.25);
    harness.expect_idle();
}

fn exercise_dialogs_and_relative_controls(harness: &Harness) {
    let app = &harness.app;
    app.global::<riina_ui::PlaybackTimelineState>()
        .set_position_label("2:05".into());
    app.global::<PlayerMenuActions>()
        .invoke_open_jump_time_dialog();
    assert!(
        app.global::<riina_ui::PlayerDialogState>()
            .get_jump_time_dialog_visible()
    );
    assert_eq!(
        app.global::<riina_ui::PlayerDialogState>()
            .get_jump_time_draft(),
        "2:05"
    );
    app.global::<riina_ui::PlayerDialogState>()
        .invoke_jump_to_time("invalid".into());
    assert!(
        app.global::<riina_ui::PlayerDialogState>()
            .get_jump_time_dialog_visible()
    );
    assert_eq!(
        app.global::<riina_ui::PlayerDialogState>()
            .get_jump_time_error(),
        "Enter a valid time such as 20:35 or 1:02:03.500."
    );
    harness.expect_idle();
    app.global::<riina_ui::PlayerDialogState>()
        .invoke_jump_to_time("2:05.500".into());
    harness.expect(Command::Seek {
        seconds: 125.5,
        absolute: true,
    });
    assert!(
        !app.global::<riina_ui::PlayerDialogState>()
            .get_jump_time_dialog_visible()
    );
    assert_eq!(
        app.global::<riina_ui::PlayerDialogState>()
            .get_jump_time_error(),
        ""
    );

    app.global::<riina_ui::PlayerDialogState>()
        .set_url_dialog_draft("previous".into());
    app.global::<PlayerMenuActions>().invoke_open_url_dialog();
    assert!(
        app.global::<riina_ui::PlayerDialogState>()
            .get_url_dialog_visible()
    );
    assert_eq!(
        app.global::<riina_ui::PlayerDialogState>()
            .get_url_dialog_draft(),
        ""
    );
    let dialog = app.global::<riina_ui::PlayerDialogState>();
    app.global::<PlayerMenuActions>()
        .invoke_open_playlist_url_dialog();
    assert!(dialog.get_url_dialog_appends_playlist());
    assert!(!dialog.invoke_append_playlist_url("bad url".into()));
    assert!(!dialog.get_url_dialog_error().is_empty());
    harness.expect_idle();
    assert!(dialog.invoke_append_playlist_url(" https://example.com/video.mp4 ".into()));
    harness.expect(Command::AppendFiles(vec![
        "https://example.com/video.mp4".into(),
    ]));
    app.global::<PlayerMenuActions>().invoke_open_url_dialog();
    assert!(!dialog.get_url_dialog_appends_playlist());
    assert!(dialog.get_url_dialog_error().is_empty());
    dialog.set_url_dialog_visible(false);
    app.global::<PlayerMenuActions>().invoke_browse_files(-1);
    harness.expect_idle();

    app.global::<PlayerMenuActions>().invoke_speed(1.5);
    harness.expect(Command::MultiplySpeed(1.5));
    app.global::<PlayerMenuActions>().invoke_adjust_volume(-3.0);
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
    assert_eq!(
        app.global::<riina_ui::PlaybackTimelineState>()
            .get_position(),
        1.25
    );
    assert_eq!(
        app.global::<riina_ui::PlaybackTimelineState>()
            .get_duration(),
        10.0
    );
    assert_eq!(
        app.global::<riina_ui::PlaybackAudioState>().get_volume(),
        45.0
    );
    assert_eq!(app.get_media_title(), "Headless fixture");

    app.global::<riina_ui::PlayerControlActions>()
        .invoke_set_ab_loop_point(0, 1.25);
    harness.expect(Command::SetAbLoopPoint {
        point: riina_core::AbLoopPoint::A,
        value: Some(1.25),
    });
    app.global::<riina_ui::PlayerControlActions>()
        .invoke_set_volume(140.0);
    harness.expect(Command::Volume(100.0));
    app.global::<riina_ui::PlayerSidebarActions>()
        .invoke_set_volume_boost(true);
    harness.expect(Command::SetVolumeBoost(true));
    app.global::<riina_ui::PlayerControlActions>()
        .invoke_set_volume(140.0);
    harness.expect(Command::Volume(140.0));
    Ok(())
}
