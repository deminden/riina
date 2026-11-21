use super::*;

#[test]
#[ignore = "requires provisioned libmpv; exercises generic property observation"]
fn dynamic_mpv_property_observation_publishes_generic_node_values() {
    let root = fixture_root();
    let (sender, receiver) = mpsc::channel();
    let player = start_test_player(&test_library(&root), true, move |state| {
        let _ = sender.send(state);
    })
    .unwrap();
    let startup_versions = wait_for(&receiver, |state| {
        state.phase == Phase::Stopped
            && state
                .mpv_version
                .as_deref()
                .is_some_and(|version| !version.trim().is_empty())
            && state
                .ffmpeg_version
                .as_deref()
                .is_some_and(|version| !version.trim().is_empty())
    });
    assert_eq!(
        startup_versions.error, None,
        "unexpected native error while observing engine versions before media: {:?}",
        startup_versions.error
    );
    player
        .send(Command::Load(
            root.join("tests/fixtures/moving-square.avi")
                .to_string_lossy()
                .into_owned(),
        ))
        .unwrap();
    wait_for(&receiver, |state| {
        state.phase == Phase::Playing && state.position.is_some_and(|position| position > 0.2)
    });
    let versions = wait_for(&receiver, |state| {
        state
            .mpv_version
            .as_deref()
            .is_some_and(|version| !version.trim().is_empty())
            && state
                .ffmpeg_version
                .as_deref()
                .is_some_and(|version| !version.trim().is_empty())
    });
    assert_eq!(
        versions.error, None,
        "unexpected native error while observing engine versions: {:?}",
        versions.error
    );
    player
        .send(Command::ObserveMpvProperty("container-fps".into()))
        .unwrap();
    let observed = wait_for(&receiver, |state| {
        state
            .observed_mpv_properties
            .get("container-fps")
            .is_some_and(|value| value.as_f64().is_some_and(|fps| fps > 0.0))
    });
    assert_eq!(
        observed.error, None,
        "unexpected native error: {:?}",
        observed.error
    );
    drop(player);
}

#[test]
#[ignore = "requires provisioned libmpv; verifies native event names reach playback snapshots"]
fn generic_mpv_events_are_retained_for_plugin_listeners() {
    let root = fixture_root();
    let (sender, receiver) = mpsc::channel();
    let player = start_test_player(&test_library(&root), true, move |state| {
        let _ = sender.send(state);
    })
    .unwrap();
    player
        .send(Command::Load(
            root.join("tests/fixtures/moving-square.avi")
                .to_string_lossy()
                .into_owned(),
        ))
        .unwrap();
    let playback = wait_for(&receiver, |state| {
        state
            .mpv_events
            .iter()
            .any(|event| event.name == "file-loaded")
    });
    assert!(
        playback
            .mpv_events
            .iter()
            .any(|event| event.name == "start-file")
    );
    assert!(playback.mpv_event_sequence >= 2);
}
