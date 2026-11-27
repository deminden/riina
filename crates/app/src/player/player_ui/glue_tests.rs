//! Characterise the real player callbacks without opening a native window or libmpv.

use super::*;
use crate::CatalogSelection;
use riina_core::{Command, Preferences, TrackKind};
use riina_mpv::{CommandSender, RecordingEngine, StartupOptions};
use riina_test_support::TempDir;
use riina_ui::PlayerMenuActions;
use slint::{ComponentHandle, Model};
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
    startup_options: Rc<RefCell<StartupOptions>>,
    startup_commands: Vec<Command>,
}

impl Harness {
    pub(crate) fn start(
        path: PathBuf,
        capture_before_navigation: bool,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Self::start_with_media(path, capture_before_navigation, InitialMedia::default())
    }

    fn start_with_media(
        path: PathBuf,
        capture_before_navigation: bool,
        media: InitialMedia,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Self::start_in_application(
            path,
            capture_before_navigation,
            media,
            ApplicationServices::default(),
            None,
        )
    }

    fn start_in_application(
        path: PathBuf,
        capture_before_navigation: bool,
        media: InitialMedia,
        application: ApplicationServices,
        welcome: Option<&WelcomeWindow>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let loaded = crate::preference_load::load(&path)?;
        let app = PlayerWindow::new()?;
        app.set_native_video(capture_before_navigation);
        let (commands, engine) = CommandSender::recording(64);
        let engine = Rc::new(engine);
        let startup_options = Rc::new(RefCell::new(StartupOptions::default()));
        let session = configure_with_engine(
            &app,
            None,
            media,
            welcome,
            loaded,
            SessionStartup {
                media_geometry: Default::default(),
                application,
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

fn exercise_script_key_ownership(
    profile: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    use riina_core::key_bindings::MpvInputOwner;
    use riina_ui::PlayerControlActions;

    let player = Harness::start(profile.join("mpv-console-routing/preferences.json"), false)?;
    crate::key_binding_preferences::install_player(
        &player.app,
        &riina_core::key_bindings::Settings {
            current_profile: "mpv Default".into(),
            ..Default::default()
        },
        crate::PlaybackStore::default(),
    );
    let controls = player.app.global::<PlayerControlActions>();
    let route = |key: &str| {
        controls.invoke_forward_mpv_script_key(key.into(), false, false, false, false, false)
    };
    assert!(!route("s"));
    controls.invoke_open_mpv_console();
    player.expect_idle();
    assert!(!route("s"));

    let mut state = Playback {
        mpv_console_available: true,
        ..Playback::default()
    };
    player.publish(state.clone())?;
    assert!(controls.invoke_dispatch_key_binding("`".into(), false, false, false, false, false));
    player.expect(Command::MpvInputAction(
        "script-binding console/enable".into(),
    ));
    assert!(route("H"));
    assert!(route("é"));
    assert!(controls.invoke_forward_mpv_script_key("é".into(), false, false, false, false, true));
    player.expect_idle();
    state.mpv_console_open = true;
    player.publish(state.clone())?;
    player.expect_idle();
    state.mpv_console_keys_active = true;
    player.publish(state.clone())?;
    player.expect(Command::MpvScriptKeys {
        owner: MpvInputOwner::Console,
        keys: vec!["H".into(), "é".into()],
    });

    assert!(route("Escape"));
    player.expect(Command::MpvScriptKeys {
        owner: MpvInputOwner::Console,
        keys: vec!["ESC".into()],
    });
    state.mpv_console_open = false;
    state.mpv_console_keys_active = false;
    state.mpv_statistics_keys = std::sync::Arc::new(vec!["2".into(), "ESC".into()]);
    player.publish(state)?;
    assert!(!route("s"));
    assert!(route("2"));
    player.expect(Command::MpvScriptKeys {
        owner: MpvInputOwner::Statistics,
        keys: vec!["2".into()],
    });
    Ok(())
}

#[test]
fn screenshot_bindings_use_the_players_capture_settings_and_ignore_key_repeat()
-> Result<(), Box<dyn std::error::Error>> {
    use riina_core::screenshot::{Binding, Content, Encoding, Format, Settings};
    use riina_ui::{PlayerControlActions, PlayerPreferences};

    i_slint_backend_testing::init_no_event_loop();
    let scratch = TempDir::new("screenshot-bindings")?;
    let player = Harness::start(scratch.path().join("preferences.json"), false)?;
    let directory = scratch.path().join("captures");
    let settings = Settings {
        directory: Some(directory.to_string_lossy().into_owned()),
        include_subtitles: false,
        ..Settings::default()
    };
    crate::screenshot_preferences::install(&player.app, &settings);
    crate::key_binding_preferences::install_player(
        &player.app,
        &riina_core::key_bindings::Settings {
            current_profile: "mpv Default".into(),
            ..Default::default()
        },
        crate::PlaybackStore::default(),
    );
    player.app.set_screenshot_available(true);
    let controls = player.app.global::<PlayerControlActions>();
    let dispatch = |ctrl, alt, shift, repeat| {
        assert!(controls.invoke_dispatch_key_binding("s".into(), ctrl, alt, shift, false, repeat));
    };
    let directory = crate::platform::screenshot_directory(settings.directory.as_deref())?
        .to_string_lossy()
        .into_owned();
    dispatch(false, false, false, false);
    player.expect(Command::Screenshot {
        directory: directory.clone(),
        content: Content::Video,
        encoding: Encoding::default(),
    });
    for (ctrl, alt, shift, action) in [
        (true, false, false, "screenshot window"),
        (false, false, true, "screenshot video"),
        (false, true, false, "screenshot each-frame"),
    ] {
        dispatch(ctrl, alt, shift, false);
        player.expect(Command::ScreenshotBinding {
            directory: directory.clone(),
            default_content: Content::Video,
            encoding: Encoding::default(),
            binding: Binding::parse(action).unwrap(),
        });
        dispatch(ctrl, alt, shift, true);
        player.expect_idle();
    }
    player
        .app
        .global::<PlayerPreferences>()
        .set_screenshot_save_to_file(false);
    dispatch(true, false, false, false);
    player.expect_idle();
    let preferences = player.app.global::<PlayerPreferences>();
    preferences.set_screenshot_copy_to_clipboard(true);
    preferences.set_screenshot_format("avif".into());
    preferences.set_screenshot_template(
        scratch
            .path()
            .join("an absolute template %n")
            .to_string_lossy()
            .as_ref()
            .into(),
    );
    dispatch(true, false, false, false);
    let Command::ScreenshotBinding {
        directory: private_path,
        default_content,
        encoding,
        binding,
    } = player.engine.commands.try_recv()?
    else {
        panic!("Expected the clipboard-only screenshot command")
    };
    assert_ne!(private_path, directory);
    assert!(std::path::Path::new(&private_path).is_dir());
    assert_eq!(default_content, Content::Video);
    assert_eq!(encoding.format, Format::Avif);
    assert_eq!(encoding.template, "%n");
    assert_eq!(binding, Binding::parse("screenshot window").unwrap());
    player.expect_idle();
    Ok(())
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

fn exercise_shared_volume(profile: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    use riina_core::RememberedVolume;

    let path = profile.join("shared-volume/preferences.json");
    let application = ApplicationServices::default();
    let start = || {
        Harness::start_in_application(
            path.clone(),
            false,
            InitialMedia::default(),
            application.clone(),
            None,
        )
    };
    let first = start()?;
    let second = start()?;
    let accepted = RememberedVolume {
        revision: 2,
        level: 68.0,
    };
    first.publish(Playback {
        volume: accepted.level,
        remembered_volume: Some(accepted),
        ..Default::default()
    })?;
    assert!(!path.exists());
    assert_eq!(
        second.session.preference_state.volume.pending_level(),
        Some(68.0)
    );
    second.publish(Playback {
        volume: 37.0,
        remembered_volume: Some(RememberedVolume {
            revision: 1,
            level: 37.0,
        }),
        ..Default::default()
    })?;
    assert_eq!(
        second.session.preference_state.volume.pending_level(),
        Some(68.0)
    );
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(200));
    assert_eq!(second.session.preference_state.volume.pending_level(), None);
    assert_eq!(
        crate::preference_load::load(&path)?.preferences.volume,
        68.0
    );

    first.engine.publish(Playback {
        volume: 42.0,
        remembered_volume: Some(RememberedVolume {
            revision: 3,
            level: 42.0,
        }),
        ..Default::default()
    });
    first.session.save(&first.app)?;
    second.session.save(&second.app)?;
    assert_eq!(
        crate::preference_load::load(&path)?.preferences.volume,
        42.0
    );
    Ok(())
}

fn exercise_idle_managed_player(
    profile: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::plugins::plugin_players::{Options, Ticket};
    use riina_core::Phase;

    let ticket = Ticket::new(
        "org.example.visibility".into(),
        1,
        Options {
            source: None,
            label: None,
            enable_plugins: false,
            disable_ui: true,
            disable_window_animation: false,
        },
    );
    let harness = Harness::start_with_media(
        profile.join("managed/preferences.json"),
        false,
        InitialMedia::for_plugin(ticket),
    )?;
    harness.publish(Playback::default())?;
    assert!(!harness.app.window().is_visible());
    assert!(
        harness
            .app
            .global::<riina_ui::PlayerControlsState>()
            .get_plugin_ui_disabled()
    );
    harness.publish(Playback {
        generation: 1,
        phase: Phase::Loading,
        ..Playback::default()
    })?;
    assert!(harness.app.window().is_visible());
    harness.app.hide()?;
    harness.publish(Playback {
        generation: 2,
        phase: Phase::Paused,
        ..Playback::default()
    })?;
    assert!(!harness.app.window().is_visible());
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
    exercise_script_key_ownership(&profile)?;
    exercise_teardown(&profile)?;
    exercise_idle_managed_player(&profile)?;
    exercise_shared_recents(&profile)?;
    exercise_shared_volume(&profile)?;
    let harness = Harness::start(profile.join("shared/preferences.json"), false)?;
    exercise_transport_and_tracks(&harness);
    exercise_transport_settings(&harness);
    exercise_dialogs_and_relative_controls(&harness);
    exercise_observation_and_volume(&harness)?;
    exercise_chapter_metadata_and_hover(&harness)?;
    exercise_controller_arrows(&harness);
    exercise_timeline_generation(&profile)?;
    exercise_playlist_feedback(&profile)?;
    super::super::playlist_edits::tests::exercise_playlist_edit_order(&profile)?;
    exercise_playlist_footer_menus(&profile)?;
    exercise_playlist_footer_selection(&profile)?;
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
    capture.expect(Command::Stop(Some(
        capture.app.get_media_request_generation().cast_unsigned(),
    )));
    Ok(())
}

fn exercise_shared_recents(profile: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let directory = profile.join("shared-recents");
    std::fs::create_dir_all(&directory)?;
    let path = directory.join("preferences.json");
    let old_media = directory.join("first.mkv");
    let new_media = directory.join("second.mkv");
    std::fs::write(&old_media, b"first")?;
    std::fs::write(&new_media, b"second")?;
    let old_media = old_media.to_string_lossy().into_owned();
    let new_media = new_media.to_string_lossy().into_owned();
    let original = riina_core::RecentMedia {
        path: old_media.clone(),
        title: "first.mkv".into(),
        position: 18.0,
    };
    let alias = riina_core::RecentMedia {
        path: url::Url::from_file_path(&old_media)
            .map_err(|()| "Could not make a file URL for the recent item")?
            .into(),
        ..original.clone()
    };
    let prefs = Preferences {
        resume_last_position: true,
        recent_media: vec![original.clone()],
        recent_open_documents: vec![original, alias],
        ..Preferences::default()
    };
    std::fs::write(&path, serde_json::to_vec(&prefs)?)?;
    let application = ApplicationServices::default();
    let welcome = WelcomeWindow::new()?;
    let first = Harness::start_in_application(
        path.clone(),
        false,
        InitialMedia::default(),
        application.clone(),
        Some(&welcome),
    )?;
    let second = Harness::start_in_application(
        path.clone(),
        false,
        InitialMedia::default(),
        application.clone(),
        None,
    )?;
    let menu_rows = |app: &PlayerWindow| {
        app.global::<riina_ui::PlayerCatalogState>()
            .get_recent_open_menu_rows()
    };
    assert_eq!(welcome.get_recent_media().row_count(), 1);
    assert_eq!(welcome.get_recent_documents().row_count(), 0);
    assert_eq!(menu_rows(&first.app).row_count(), 1);
    crate::ui_dialogs::remember_selected_paths(&second.app, std::slice::from_ref(&new_media));
    assert_eq!(menu_rows(&first.app).row_count(), 2);
    assert_eq!(
        welcome.get_recent_documents().row_data(0).unwrap().title,
        "second.mkv"
    );
    let third = directory.join("third.mkv");
    std::fs::write(&third, b"third")?;
    welcome.set_selection_index(0);
    crate::ui_dialogs::remember_selected_paths(
        &second.app,
        &[third.to_string_lossy().into_owned()],
    );
    assert_eq!(welcome.get_selection_index(), 1);
    assert_eq!(
        welcome
            .get_recent_documents()
            .row_data(1)
            .unwrap()
            .token
            .as_str(),
        new_media
    );
    let alias: String = url::Url::from_file_path(&new_media)
        .map_err(|()| "Could not make a file URL for the selected item")?
        .into();
    crate::ui_dialogs::remember_selected_paths(&second.app, std::slice::from_ref(&alias));
    assert_eq!(welcome.get_selection_index(), 0);
    assert_eq!(menu_rows(&second.app).row_count(), 3);
    assert_eq!(
        welcome.get_recent_documents().row_data(0).unwrap().token,
        alias.as_str()
    );
    let saved = crate::preference_load::read_valid(&path)?.ok_or("Missing recent preferences")?;
    assert_eq!(saved.recent_open_documents.len(), 3);
    assert_eq!(saved.recent_open_documents[0].path, alias);
    first
        .app
        .global::<PlayerMenuActions>()
        .invoke_clear_open_recent();
    assert_eq!(menu_rows(&second.app).row_count(), 0);
    assert_eq!(welcome.get_recent_media().row_count(), 1);
    first
        .app
        .invoke_open_recent(new_media.as_str().into(), false);
    first.expect_idle();

    let paused = |id, generation, filename, position| Playback {
        generation,
        phase: riina_core::Phase::Paused,
        position: Some(position),
        playlist: vec![riina_core::PlaylistEntry {
            id,
            filename,
            current: true,
            playing: true,
            title: None,
        }]
        .into(),
        ..Default::default()
    };
    second.publish(paused(1, 1, old_media, 12.0))?;
    let state = &first.session.preference_state;
    let clear = state.history.clear_hook(
        path.clone(),
        state.recent.activity.clone(),
        application.history.clone(),
    );
    assert_eq!(clear()?, 1);
    assert_eq!(welcome.get_recent_media().row_count(), 0);
    assert_eq!(menu_rows(&second.app).row_count(), 0);
    second.session.save(&second.app)?;
    let saved = Preferences::parse(&std::fs::read_to_string(&path)?)?;
    assert_eq!(saved.recent_resume_revision, 1);
    assert!(saved.recent_media.is_empty());
    assert!(saved.recent_open_documents.is_empty());

    second.publish(Playback {
        generation: 2,
        phase: riina_core::Phase::Paused,
        ..Default::default()
    })?;
    second.publish(paused(2, 2, new_media.clone(), 4.0))?;
    assert_eq!(menu_rows(&first.app).row_count(), 1);
    second.session.save(&second.app)?;
    let saved = Preferences::parse(&std::fs::read_to_string(&path)?)?;
    assert_eq!(
        saved.recent_media.first().map(|media| media.path.as_str()),
        Some(new_media.as_str())
    );
    assert_eq!(welcome.get_recent_media().row_count(), 1);
    assert_eq!(welcome.get_recent_documents().row_count(), 0);
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
        audio: riina_core::audio_preferences::Settings {
            device: "wasapi/detached".into(),
            ..Default::default()
        },
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
    let startup = harness.startup_options.borrow();
    assert_eq!(
        startup.preferred_audio_device.as_deref(),
        Some("wasapi/detached")
    );
    let options = &startup.mpv;
    assert!(options.iter().all(|(name, _)| name != "audio-device"));
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
        &options[options.len() - 3..],
        &[
            ("osd-level".into(), "1".into()),
            ("vd-lavc-threads".into(), "2".into()),
            ("stop-screensaver".into(), "no".into()),
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
    app.global::<PlayerMenuActions>()
        .invoke_toggle_file_repeat();
    harness.expect(Command::ToggleLoopMode(riina_core::LoopMode::File));
    app.global::<PlayerMenuActions>()
        .invoke_toggle_playlist_repeat();
    harness.expect(Command::ToggleLoopMode(riina_core::LoopMode::Playlist));
    app.global::<riina_ui::PlayerControlActions>()
        .invoke_execute_mpv_input_action("cycle pause".into());
    harness.expect(Command::MpvInputAction("cycle pause".into()));
}

fn exercise_transport_and_tracks(harness: &Harness) {
    let app = &harness.app;
    app.global::<PlayerMenuActions>().invoke_toggle_pause();
    harness.expect(Command::TogglePause);
    app.global::<PlayerMenuActions>().invoke_stop();
    harness.expect(Command::Stop(Some(
        app.get_media_request_generation().cast_unsigned(),
    )));
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

    let dialog = app.global::<riina_ui::PlayerDialogState>();
    app.global::<PlayerMenuActions>()
        .invoke_open_playlist_url_dialog();
    assert!(dialog.get_url_dialog_visible());
    assert!(!dialog.invoke_append_playlist_url("bad url".into()));
    assert!(!dialog.get_url_dialog_error().is_empty());
    harness.expect_idle();
    assert!(dialog.invoke_append_playlist_url(" https://example.com/video.mp4 ".into()));
    harness.expect(Command::AppendFiles(vec![
        "https://example.com/video.mp4".into(),
    ]));
    app.global::<PlayerMenuActions>()
        .invoke_open_playlist_url_dialog();
    assert!(dialog.get_url_dialog_draft().is_empty());
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

fn exercise_playlist_feedback(profile: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    use riina_core::{PlaylistChange, PlaylistFeedback};
    use riina_ui::PlayerOsdState;
    use std::num::NonZeroUsize;

    let harness = Harness::start(profile.join("playlist-feedback/preferences.json"), false)?;
    let app = &harness.app;
    let osd = app.global::<PlayerOsdState>();
    let mut state = Playback {
        generation: 1,
        phase: Phase::Paused,
        paused: true,
        ..Default::default()
    };
    harness.publish(state.clone())?;
    harness.expect(Command::SetHdrEnabledWithIcc {
        enabled: false,
        load_icc_profile: true,
    });
    let dialog = app.global::<riina_ui::PlayerDialogState>();
    assert!(dialog.invoke_append_playlist_url("https://example.com/episode.avi".into()));
    harness.expect(Command::AppendFiles(vec![
        "https://example.com/episode.avi".into(),
    ]));
    assert!(!osd.get_video_osd_text().contains("Added"));

    let first = PlaylistFeedback {
        sequence: 1,
        generation: 1,
        change: PlaylistChange::Added(NonZeroUsize::MIN),
    };
    state.playlist_feedback = Some(first);
    harness.publish(state.clone())?;
    assert_eq!(osd.get_video_osd_text(), "Added 1 Files to Playlist");

    app.global::<PlayerMenuActions>()
        .invoke_playlist_action(4, "".into());
    harness.expect(Command::ClearPlaylist);
    assert_eq!(osd.get_video_osd_text(), "Added 1 Files to Playlist");
    state.playlist_feedback = Some(PlaylistFeedback {
        sequence: 2,
        change: PlaylistChange::Cleared,
        ..first
    });
    harness.publish(state.clone())?;
    assert_eq!(osd.get_video_osd_text(), "Cleared Playlist");

    osd.set_video_osd_text("later feedback".into());
    harness.publish(state.clone())?;
    assert_eq!(osd.get_video_osd_text(), "later feedback");
    state.generation += 1;
    state.playlist_feedback = Some(PlaylistFeedback {
        sequence: 3,
        ..first
    });
    harness.publish(state)?;
    assert_ne!(osd.get_video_osd_text(), "Added 1 Files to Playlist");
    Ok(())
}

fn exercise_playlist_footer_menus(
    profile: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    use i_slint_backend_testing::ElementHandle;
    use riina_ui::{
        PlaylistFooterMenu, PlaylistInteraction, PlaylistMenuOrigin, PlaylistPluginPopupState,
        PlaylistPopupInput,
    };
    use slint::platform::{Key, PointerEventButton, WindowEvent};
    let harness = Harness::start(profile.join("playlist-footer/preferences.json"), false)?;
    let app = &harness.app;
    let playlist = app.global::<PlaylistInteraction>();
    let sidebar = app.global::<riina_ui::PlayerSidebarState>();
    let popup = app.global::<PlaylistPluginPopupState>();
    let dialog = app.global::<riina_ui::PlayerDialogState>();
    let menu = app.global::<PlayerMenuActions>();
    let picked = std::rc::Rc::new(std::cell::Cell::new(None));
    let report = picked.clone();
    menu.on_browse_files(move |category| report.set(Some(category)));
    app.window().set_size(slint::PhysicalSize::new(600, 450));
    app.set_loaded(true);
    sidebar.set_sidebar(4);
    app.show()?;

    let flush = || -> Result<(), Box<dyn std::error::Error>> {
        slint::invoke_from_event_loop(|| {
            let _ = slint::quit_event_loop();
        })?;
        slint::run_event_loop_until_quit()?;
        slint::platform::update_timers_and_animations();
        Ok(())
    };
    let show = |origin, menu| -> Result<(), Box<dyn std::error::Error>> {
        playlist.invoke_request_playlist_footer_menu(origin, menu, 80.0, 300.0);
        flush()?;
        assert!(popup.get_shown(), "Could not open {menu:?} in {origin:?}");
        Ok(())
    };
    let choose = |title: &str| {
        let panel = popup.get_panels().row_data(0).unwrap();
        let index = panel
            .rows
            .iter()
            .position(|row| row.title == title)
            .unwrap();
        popup.invoke_input(
            PlaylistPopupInput::Choose,
            0,
            i32::try_from(index).unwrap(),
            0.0,
        );
        assert!(!popup.get_shown());
    };

    let sort = ElementHandle::find_by_accessible_label(app, "Sort playlist")
        .next()
        .expect("The playlist footer must be visible");
    sort.mock_single_click(PointerEventButton::Left);
    flush()?;
    assert!(popup.get_shown());
    let press = |key: Key| {
        let text: slint::SharedString = key.into();
        app.window()
            .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
        app.window()
            .dispatch_event(WindowEvent::KeyReleased { text });
    };
    press(Key::Escape);
    flush()?;
    assert!(!popup.get_shown());
    press(Key::Return);
    flush()?;
    assert!(
        popup.get_shown(),
        "Escape must restore the Sort button's focus"
    );
    press(Key::End);
    app.window().dispatch_event(WindowEvent::KeyPressRepeated {
        text: Key::Return.into(),
    });
    assert!(popup.get_shown());
    harness.expect_idle();
    press(Key::Return);
    harness.expect(Command::SortPlaylist(
        riina_core::playlist::PlaylistSort::PathDescending,
    ));

    show(PlaylistMenuOrigin::Sidebar, PlaylistFooterMenu::Add)?;
    choose("Add File…");
    assert_eq!(picked.get(), Some(4));
    if !crate::platform::playlist_menu::MIXED_PLAYLIST_PICKER {
        show(PlaylistMenuOrigin::Sidebar, PlaylistFooterMenu::Add)?;
        choose("Add Folder…");
        assert_eq!(picked.get(), Some(5));
    }

    sidebar.set_sidebar(0);
    let music = app.global::<riina_ui::PlayerMusicModeState>();
    music.set_music_mode(true);
    music.set_music_mode_show_playlist(true);
    flush()?;
    show(PlaylistMenuOrigin::MusicMode, PlaylistFooterMenu::Sort)?;
    choose("File Path Descending");
    harness.expect(Command::SortPlaylist(
        riina_core::playlist::PlaylistSort::PathDescending,
    ));
    show(PlaylistMenuOrigin::MusicMode, PlaylistFooterMenu::Add)?;
    choose("Add URL…");
    assert!(dialog.get_url_dialog_visible());

    dialog.set_url_dialog_visible(false);
    flush()?;
    show(PlaylistMenuOrigin::MusicMode, PlaylistFooterMenu::Add)?;
    playlist.set_playlist_menu_generation(playlist.get_playlist_menu_generation().wrapping_add(1));
    choose("Add URL…");
    assert!(!dialog.get_url_dialog_visible());
    app.hide()?;
    Ok(())
}

fn exercise_playlist_footer_selection(
    profile: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    use i_slint_backend_testing::ElementHandle;
    use riina_ui::{PlayerCatalogState, PlayerSidebarState, PlaylistInteraction};
    let harness = Harness::start(profile.join("playlist-selection/preferences.json"), false)?;
    let app = &harness.app;
    let playlist = app.global::<PlaylistInteraction>();
    app.global::<PlayerSidebarState>().set_sidebar(4);
    app.window().set_size(slint::PhysicalSize::new(760, 510));
    app.show()?;
    harness.publish(Playback {
        generation: 1,
        phase: Phase::Paused,
        paused: true,
        playlist: ["First", "Second"]
            .into_iter()
            .enumerate()
            .map(|(index, title)| riina_core::PlaylistEntry {
                id: i64::try_from(index).unwrap() + 1,
                filename: format!("{title}.mkv"),
                title: Some(title.into()),
                playing: index == 0,
                current: index == 0,
            })
            .collect::<Vec<_>>()
            .into(),
        ..Playback::default()
    })?;
    harness.expect(Command::SetHdrEnabledWithIcc {
        enabled: false,
        load_icc_profile: true,
    });
    let control = |label| {
        ElementHandle::find_by_accessible_label(app, label)
            .next()
            .unwrap()
    };
    let remove = control("Remove selected playlist entries");
    assert_eq!(remove.accessible_enabled(), Some(false));

    let first = control("First, First.mkv");
    first.invoke_accessible_default_action();
    assert_eq!(first.accessible_item_selected(), Some(true));
    assert_eq!(playlist.get_focused_playlist_token(), "1");
    harness.expect_idle();

    playlist.invoke_select_row(1, true, false);
    playlist.invoke_select_row(0, true, false);
    assert_eq!(first.accessible_item_selected(), Some(false));
    assert_eq!(remove.accessible_enabled(), Some(true));
    remove.invoke_accessible_default_action();
    harness.expect(Command::RemovePlaylistIds(vec![2]));
    assert_eq!(remove.accessible_enabled(), Some(false));
    remove.invoke_accessible_default_action();
    harness.expect_idle();

    let shuffle = control("Shuffle playlist");
    assert_eq!(shuffle.accessible_checkable(), Some(true));
    assert_eq!(shuffle.accessible_checked(), Some(false));
    app.global::<PlayerCatalogState>()
        .set_playlist_shuffled(true);
    assert_eq!(shuffle.accessible_checked(), Some(true));
    app.hide()?;
    Ok(())
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
        .invoke_seek_timeline(0.25);
    harness.expect(Command::SeekTimeline(0.25));

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

fn exercise_timeline_generation(
    profile: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let harness = Harness::start(profile.join("timeline-gesture/preferences.json"), false)?;
    let snapshots = |generation| Playback {
        generation,
        phase: Phase::Paused,
        paused: true,
        seekable: true,
        position: Some(2.0),
        duration: Some(8.0),
        ..Playback::default()
    };
    let hdr = Command::SetHdrEnabledWithIcc {
        enabled: false,
        load_icc_profile: true,
    };
    let actions = harness.app.global::<riina_ui::PlayerControlActions>();
    harness.publish(snapshots(1))?;
    harness.expect(hdr.clone());
    actions.invoke_timeline_gesture(true);
    actions.invoke_seek_timeline(0.5);
    harness.expect(Command::SeekTimeline(0.5));
    harness.publish(snapshots(2))?;
    harness.expect(hdr);
    actions.invoke_seek_timeline(0.75);
    actions.invoke_set_ab_loop_point(0, 1.0);
    harness.expect_idle();
    actions.invoke_timeline_gesture(false);
    actions.invoke_seek_timeline(0.25);
    harness.expect(Command::SeekTimeline(0.25));
    Ok(())
}

fn exercise_chapter_metadata_and_hover(
    harness: &Harness,
) -> Result<(), Box<dyn std::error::Error>> {
    use riina_core::Chapter;
    use riina_ui::{
        PlaybackTimelineState, PlayerCatalogState, PlayerControlActions, PlayerMenuActions,
        PlayerMusicModeState, PlayerSidebarState,
    };

    let app = &harness.app;
    let music = app.global::<PlayerMusicModeState>();
    let timeline = app.global::<PlaybackTimelineState>();
    let catalog = app.global::<PlayerCatalogState>();
    let actions = app.global::<PlayerControlActions>();
    app.global::<riina_ui::PlayerPreferences>()
        .set_thumbnail_preview_enabled(false);
    let mut playback = Playback {
        generation: 1,
        phase: Phase::Paused,
        paused: true,
        seekable: true,
        position: Some(1.25),
        duration: Some(10.0),
        title: "Recorded session".into(),
        artist: Some("File artist".into()),
        album: Some("Session album".into()),
        chapter_index: Some(0),
        chapter_performer: Some("Guest artist".into()),
        chapters: vec![
            Chapter {
                time: 1.0,
                title: Some("Opening".into()),
            },
            Chapter {
                time: 3.0,
                title: None,
            },
        ]
        .into(),
        ..Default::default()
    };
    harness.publish(playback.clone())?;
    assert_eq!(app.get_media_title(), "Recorded session");
    assert_eq!(music.get_music_title(), "Opening");
    assert_eq!(music.get_music_artist(), "Guest artist");
    assert_eq!(music.get_music_album(), "Session album");
    let menu_actions = app.global::<PlayerMenuActions>();
    let sidebar = app.global::<PlayerSidebarState>();
    let menu = catalog.get_chapter_menu_rows();
    let first = menu.row_data(0).unwrap();
    assert_eq!(first.label, "00:01 – Opening");
    assert!(first.selected);
    let rows = catalog.get_chapters();
    music.set_music_mode(true);
    assert_eq!(app.get_window_title(), "Opening");
    assert!(
        catalog
            .get_chapters()
            .row_data(0)
            .is_some_and(|row| row.selected)
    );

    actions.invoke_timeline_hovered(0.5);
    assert!(timeline.get_timeline_preview_chapter().is_empty());
    actions.invoke_timeline_hovered(3.5);
    assert_eq!(timeline.get_timeline_preview_chapter(), "Chapter 1");
    assert_eq!(
        timeline.get_timeline_preview_label(),
        riina_core::time_label(Some(3.5))
    );
    actions.invoke_timeline_hovered(-1.0);
    assert!(timeline.get_timeline_preview_label().is_empty());
    harness.expect_idle();

    menu_actions.invoke_select_item(crate::CatalogSelection::Chapter, first.token.clone());
    harness.expect(Command::ActivateChapter {
        index: 0,
        start: 1.0,
        notify: false,
    });
    menu_actions.invoke_select_chapter_menu(first.token.clone());
    harness.expect(Command::ActivateChapter {
        index: 0,
        start: 1.0,
        notify: true,
    });

    playback.chapter_index = Some(1);
    playback.chapter_performer = None;
    sidebar.set_chapter_cursor(1);
    harness.publish(playback.clone())?;
    assert!(
        catalog
            .get_chapters()
            .row_data(1)
            .is_some_and(|row| row.selected)
    );
    assert!(rows.row_data(1).unwrap().selected);
    assert!(
        catalog
            .get_chapter_menu_rows()
            .row_data(0)
            .unwrap()
            .selected
    );
    assert_eq!(sidebar.get_chapter_cursor(), 1);
    assert_eq!(music.get_music_title(), "Recorded session");
    assert_eq!(music.get_music_artist(), "File artist");
    playback.position = Some(3.0);
    harness.publish(playback.clone())?;
    assert!(
        catalog
            .get_chapter_menu_rows()
            .row_data(1)
            .unwrap()
            .selected
    );

    playback.generation += 1;
    std::sync::Arc::make_mut(&mut playback.chapters)[1].time = 3661.0;
    harness.publish(playback)?;
    harness.expect(Command::SetHdrEnabledWithIcc {
        enabled: false,
        load_icc_profile: true,
    });
    assert_eq!(sidebar.get_chapter_cursor(), -1);
    assert_eq!(
        catalog.get_chapter_menu_rows().row_data(0).unwrap().label,
        "0:00:01 – Opening"
    );
    menu_actions.invoke_select_chapter_menu(first.token);
    harness.expect_idle();
    music.set_music_mode(false);
    Ok(())
}

fn exercise_controller_arrows(harness: &Harness) {
    use riina_core::osc_preferences::ArrowAction;
    use riina_ui::{
        ControllerArrowPhase, PlayerControlActions, PlayerControlsState, PlayerPreferences,
    };

    let preferences = harness.app.global::<PlayerPreferences>();
    let controls = harness.app.global::<PlayerControlsState>();
    let actions = harness.app.global::<PlayerControlActions>();
    assert_eq!(
        preferences.get_control_arrow_action(),
        ArrowAction::Speed.index()
    );
    actions.invoke_controller_arrow(true, ControllerArrowPhase::Pressed);
    assert_eq!(harness.engine.commands.try_recv(), Ok(Command::Speed(0.5)));
    harness.expect(Command::SetPaused(false));
    assert!(controls.get_control_bar_interaction_held());
    actions.invoke_controller_arrow(true, ControllerArrowPhase::Released);
    harness.expect_idle();
    assert!(!controls.get_control_bar_interaction_held());

    preferences.set_control_arrow_action(ArrowAction::Seek.index());
    actions.invoke_controller_arrow(true, ControllerArrowPhase::Activated);
    harness.expect(Command::Seek {
        seconds: -10.0,
        absolute: false,
    });
    actions.invoke_controller_arrow(false, ControllerArrowPhase::Activated);
    harness.expect(Command::Seek {
        seconds: 10.0,
        absolute: false,
    });

    preferences.set_control_arrow_action(ArrowAction::Playlist.index());
    actions.invoke_controller_arrow(true, ControllerArrowPhase::Activated);
    harness.expect(Command::PlaylistPrevious);
    actions.invoke_controller_arrow(false, ControllerArrowPhase::Activated);
    harness.expect(Command::PlaylistNext);
}
