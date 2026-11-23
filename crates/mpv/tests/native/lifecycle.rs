use super::*;

#[test]
#[ignore = "requires provisioned libmpv; verifies worker panic reporting and final-owner teardown"]
fn a_panicking_observer_reports_failure_then_joins_on_final_owner_drop() {
    let root = fixture_root();
    let (sender, receiver) = mpsc::channel();
    let did_panic = std::sync::atomic::AtomicBool::new(false);
    let player = start_test_player(&test_library(&root), true, move |state| {
        if state.phase == Phase::Playing
            && !did_panic.swap(true, std::sync::atomic::Ordering::Relaxed)
        {
            panic!("deliberate observer failure");
        }
        let _ = sender.send(state);
    })
    .unwrap();
    let peer = player.clone();
    let commands = player.command_sender();
    player
        .send(Command::Load(
            root.join("tests/fixtures/moving-square.avi")
                .to_string_lossy()
                .into_owned(),
        ))
        .unwrap();
    let failed = wait_for(&receiver, |state| state.phase == Phase::Failed);
    assert_eq!(
        failed.error(),
        Some("The playback engine stopped unexpectedly; reopen the file")
    );
    let error = commands.send(Command::Stop).unwrap_err();
    assert!(matches!(error, riina_mpv::EngineError::Stopped));
    assert_eq!(error.to_string(), "The playback engine has stopped");
    drop(player);
    assert!(matches!(
        receiver.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    drop(peer);
    assert!(matches!(
        receiver.recv_timeout(Duration::from_secs(1)),
        Err(mpsc::RecvTimeoutError::Disconnected)
    ));
}

#[test]
#[ignore = "requires provisioned libmpv; verifies close with queued paused-frame work"]
fn closing_with_queued_frame_work_terminates_and_reopens() {
    let root = fixture_root();
    let library = test_library(&root);
    let media = root.join("tests/fixtures/interlaced-tff.mkv");
    for _ in 0..3 {
        let Harness { player, receiver } = Harness::at(&library, Vec::new()).unwrap();
        player.send(Command::SetPauseWhenOpen(true)).unwrap();
        player
            .send(Command::Load(media.to_str().unwrap().into()))
            .unwrap();
        let loaded = wait_for(&receiver, |state| {
            state.error().is_some() || (state.paused && state.position.is_some())
        });
        assert_eq!(loaded.error(), None);
        // Close while frame work is queued rather than waiting for every reply.
        // This is a normal window-close path, including a close during capture.
        for _ in 0..12 {
            player.send(Command::FrameStep).unwrap();
            player.send(Command::CaptureFrame).unwrap();
        }
        let closing = Instant::now();
        drop(player);
        assert!(closing.elapsed() < Duration::from_secs(12));
    }
}

#[test]
#[ignore = "requires provisioned libmpv; verifies independent cores and reopening"]
fn independent_players_survive_peer_shutdown_and_reopen() {
    let root = fixture_root();
    let library = test_library(&root);
    let media = root.join("tests/fixtures/moving-square.avi");
    let open = || {
        let Harness { player, receiver } = Harness::at(&library, Vec::new()).unwrap();
        player
            .send(Command::Load(media.to_str().unwrap().into()))
            .unwrap();
        let loaded = wait_for(&receiver, |state| {
            state.error().is_some()
                || (state.phase == Phase::Playing
                    && state.position.is_some_and(|position| position > 0.1))
        });
        assert_eq!(loaded.error(), None);
        player.send(Command::SetPaused(true)).unwrap();
        wait_for(&receiver, |state| state.paused);
        (player, receiver)
    };

    let (first, _) = open();
    let (second, receiver) = open();
    drop(first);
    // Closing one window must destroy its core without invalidating another
    // window's native bindings or changing that window's paused playback.
    second
        .send(Command::Seek {
            seconds: 2.0,
            absolute: true,
        })
        .unwrap();
    let sought = wait_for(&receiver, |state| {
        state.error().is_some() || state.position.is_some_and(|position| position >= 1.9)
    });
    assert_eq!(sought.error(), None);
    assert!(sought.paused);
    drop(second);

    // A new core must remain usable after every earlier core has been destroyed.
    let (reopened, receiver) = open();
    reopened.send(Command::SetPaused(false)).unwrap();
    let playing = wait_for(&receiver, |state| {
        state.phase == Phase::Playing && !state.paused
    });
    assert_eq!(playing.error(), None);
    drop(reopened);
}

#[test]
#[ignore = "requires provisioned libmpv; verifies shutdown is exclusive to the final owner"]
fn public_shutdown_cannot_invalidate_a_retained_player() {
    let root = fixture_root();
    let Harness { player, receiver } = Harness::start(&root).unwrap();
    let retained = player.clone();
    let commands = player.command_sender();
    assert!(player.send(Command::Shutdown).is_err());
    assert!(commands.send(Command::Shutdown).is_err());
    drop(player);

    commands.send(Command::SetPauseWhenOpen(true)).unwrap();
    commands
        .send(Command::Load(
            root.join("tests/fixtures/moving-square.avi")
                .to_str()
                .unwrap()
                .into(),
        ))
        .unwrap();
    let loaded = wait_for(&receiver, |state| {
        state.error().is_some() || (state.phase == Phase::Paused && state.position.is_some())
    });
    assert_eq!(loaded.error(), None);
    drop(retained);
    assert!(commands.send(Command::SetPaused(false)).is_err());
}
