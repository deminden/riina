use super::*;
use riina_mpv::{CommandSender, EngineError, MpvPropertyFormat};

#[test]
#[ignore = "requires provisioned libmpv; exercises generic property observation"]
fn dynamic_mpv_property_observation_publishes_generic_node_values() -> Result<(), EngineError> {
    let root = fixture_root();
    let harness = Harness::start(&root).unwrap();
    let commands = harness.player.command_sender();
    assert!(read(&commands, "mpv-version", MpvPropertyFormat::Text)?.is_string());
    let startup_versions = harness.wait(|state| {
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
        startup_versions.error(),
        None,
        "unexpected native error while observing engine versions before media: {:?}",
        startup_versions.error()
    );
    harness
        .send(Command::Load(
            root.join("tests/fixtures/moving-square.avi")
                .to_string_lossy()
                .into_owned(),
        ))
        .unwrap();
    harness.wait(|state| {
        state.phase == Phase::Playing && state.position.is_some_and(|position| position > 0.2)
    });
    let versions = harness.wait(|state| {
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
        versions.error(),
        None,
        "unexpected native error while observing engine versions: {:?}",
        versions.error()
    );
    harness
        .send(Command::ObserveMpvProperty("container-fps".into()))
        .unwrap();
    let observed = harness.wait(|state| {
        state
            .observed_mpv_properties
            .get("container-fps")
            .is_some_and(|value| value.as_f64().is_some_and(|fps| fps > 0.0))
    });
    assert_eq!(
        observed.error(),
        None,
        "unexpected native error: {:?}",
        observed.error()
    );
    harness.send(Command::SetPaused(true)).unwrap();
    harness.wait(|state| state.paused);
    verify_direct_reads(&commands)?;
    drop(harness);
    Ok(())
}

fn read(
    commands: &CommandSender,
    name: &str,
    format: MpvPropertyFormat,
) -> Result<serde_json::Value, EngineError> {
    commands.read_mpv_property(name, format, Instant::now() + Duration::from_secs(2))
}

fn verify_direct_reads(commands: &CommandSender) -> Result<(), EngineError> {
    assert_eq!(read(commands, "pause", MpvPropertyFormat::Flag)?, true);
    let duration = read(commands, "duration", MpvPropertyFormat::Number)?;
    assert!(
        duration
            .as_f64()
            .is_some_and(|seconds| (seconds - 5.0).abs() < 0.1)
    );
    let path = read(commands, "path", MpvPropertyFormat::Text)?;
    assert!(
        path.as_str()
            .is_some_and(|path| path.ends_with("moving-square.avi"))
    );
    let tracks = read(commands, "track-list", MpvPropertyFormat::Node)?;
    assert!(
        tracks
            .as_array()
            .is_some_and(|tracks| tracks.iter().any(|track| track["type"] == "video"))
    );
    assert!(
        read(
            commands,
            "riina-missing-test-property",
            MpvPropertyFormat::Node
        )?
        .is_null()
    );
    Ok(())
}

#[test]
#[ignore = "requires provisioned libmpv; verifies native event names reach playback snapshots"]
fn generic_mpv_events_are_retained_for_plugin_listeners() {
    let root = fixture_root();
    let harness = Harness::start(&root).unwrap();
    harness
        .send(Command::Load(
            root.join("tests/fixtures/moving-square.avi")
                .to_string_lossy()
                .into_owned(),
        ))
        .unwrap();
    let playback = harness.wait(|state| {
        state
            .mpv_events
            .iter()
            .any(|event| event.name.as_ref() == "file-loaded")
    });
    assert!(
        playback
            .mpv_events
            .iter()
            .any(|event| event.name.as_ref() == "start-file")
    );
    assert!(playback.mpv_event_sequence >= 2);
}
