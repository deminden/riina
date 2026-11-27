use super::*;

#[cfg(target_os = "windows")]
#[test]
#[ignore = "requires the pinned Windows libmpv with its built-in Lua scripts"]
fn native_scripts_claim_keys_and_release_them_on_escape() {
    use riina_core::key_bindings::MpvInputOwner;

    let root = fixture_root();
    let harness = Harness::start(&root).unwrap();
    let send_keys = |owner, keys| {
        harness
            .send(Command::MpvScriptKeys { owner, keys })
            .unwrap()
    };
    harness.send(Command::SetPauseWhenOpen(true)).unwrap();
    harness
        .send(Command::Load(
            root.join("tests/fixtures/catalog.mkv")
                .to_string_lossy()
                .into_owned(),
        ))
        .unwrap();
    harness.wait(|state| state.mpv_console_available && state.paused && state.seekable);
    harness
        .send(Command::MpvInputAction(
            "script-binding console/enable".into(),
        ))
        .unwrap();
    harness.wait(|state| state.mpv_console_open && state.mpv_console_keys_active);

    let mut keys: Vec<_> = "set speed 1.26".chars().map(|ch| ch.to_string()).collect();
    keys.extend(["BS".into(), "5".into(), "ENTER".into()]);
    send_keys(MpvInputOwner::Console, keys);
    let changed = harness.wait(|state| (state.speed - 1.25).abs() < 0.001);
    assert!(changed.mpv_console_open);
    assert!(changed.paused);

    let mut keys: Vec<_> = "set force-media-title Héllo"
        .chars()
        .map(|ch| ch.to_string())
        .collect();
    keys.push("ENTER".into());
    send_keys(MpvInputOwner::Console, keys);
    harness.wait(|state| state.title == "Héllo");
    send_keys(MpvInputOwner::Console, vec!["ESC".into()]);
    harness.wait(|state| !state.mpv_console_open && !state.mpv_console_keys_active);

    harness
        .send(Command::MpvInputAction(
            "script-binding stats/display-stats-toggle".into(),
        ))
        .unwrap();
    harness.wait(|state| state.mpv_statistics_keys.iter().any(|key| key == "4"));
    send_keys(MpvInputOwner::Statistics, vec!["4".into(), "ESC".into()]);
    let closed = harness.wait(|state| state.mpv_statistics_keys.is_empty());
    assert!(closed.paused);
    assert_eq!(closed.error(), None);
}
use riina_mpv::{CommandSender, EngineError, MpvPropertyFormat};

#[test]
#[ignore = "requires provisioned libmpv and compares its playback clocks"]
fn remaining_clocks_follow_speed_while_paused_and_clear_on_stop() -> Result<(), EngineError> {
    let root = fixture_root();
    let harness = Harness::start(&root)?;
    harness.send(Command::Load(
        root.join("tests/fixtures/moving-square.avi")
            .to_string_lossy()
            .into_owned(),
    ))?;
    harness.wait(|state| state.phase == Phase::Playing && state.remaining_time.is_some());
    harness.send(Command::SetPaused(true))?;
    harness.wait(|state| state.paused && state.remaining_time.is_some_and(|seconds| seconds > 2.0));
    harness.send(Command::Speed(2.0))?;
    let paused = harness.wait(|state| {
        state.paused
            && (state.speed - 2.0).abs() < f64::EPSILON
            && state
                .remaining_time
                .zip(state.scaled_remaining_time)
                .is_some_and(|(remaining, scaled)| {
                    scaled > 1.0 && (remaining - scaled * 2.0).abs() < 0.1
                })
    });
    assert_eq!(paused.error(), None);

    harness.send(Command::Stop(None))?;
    harness.wait(|state| {
        state.phase == Phase::Stopped
            && state.remaining_time.is_none()
            && state.scaled_remaining_time.is_none()
    });
    Ok(())
}

#[test]
#[ignore = "requires provisioned libmpv; exercises generic property observation"]
fn dynamic_mpv_property_observation_publishes_generic_node_values() -> Result<(), EngineError> {
    let root = fixture_root();
    let quiet_client_versions = riina_mpv::read_engine_versions(&test_library(&root))?;
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
    assert_eq!(
        startup_versions.mpv_version.as_deref(),
        Some(quiet_client_versions.mpv.as_str())
    );
    assert_eq!(
        startup_versions.ffmpeg_version.as_deref(),
        Some(quiet_client_versions.ffmpeg.as_str())
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
    harness
        .send(Command::ObserveMpvProperty("pause".into()))
        .unwrap();
    let initial_pause = harness.wait(|state| {
        state.observed_mpv_properties.get("pause") == Some(&serde_json::json!(false))
    });
    let mut after = initial_pause.mpv_event_sequence;
    for paused in [true, false, true] {
        let expected = serde_json::json!(paused);
        harness.send(Command::SetPaused(paused)).unwrap();
        let state = harness.wait(|state| {
            state.paused == paused
                && state.mpv_events.iter().any(|event| {
                    event.sequence > after
                        && event.name.as_ref() == "pause.changed"
                        && event.value.as_deref() == Some(&expected)
                })
        });
        let mut notifications = state.mpv_events.iter().skip_while(|event| {
            event.sequence <= after
                || event.name.as_ref() != "pause.changed"
                || event.value.as_deref() != Some(&expected)
        });
        let sequence = notifications.next().unwrap().sequence;
        assert_eq!(
            notifications.next().unwrap().name.as_ref(),
            "property-change"
        );
        after = sequence;
    }
    verify_property_requests(&commands)?;
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

fn verify_property_requests(commands: &CommandSender) -> Result<(), EngineError> {
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
    let write = |name, value| {
        commands.write_mpv_property(name, value, Instant::now() + Duration::from_secs(2))
    };
    for (name, format, value) in [
        ("speed", MpvPropertyFormat::Number, serde_json::json!(1.25)),
        ("mute", MpvPropertyFormat::Flag, serde_json::json!(true)),
        (
            "loop-file",
            MpvPropertyFormat::Text,
            serde_json::json!("inf"),
        ),
        (
            "alang",
            MpvPropertyFormat::Node,
            serde_json::json!(["eng", "fra"]),
        ),
    ] {
        write(name, value.clone())?;
        assert_eq!(
            read(commands, name, format)?,
            value,
            "Native property {name}"
        );
    }
    assert!(matches!(
        write("riina-missing-test-property", serde_json::json!(1)),
        Err(EngineError::Native {
            call: "mpv_set_property_async",
            ..
        })
    ));
    for (boost, input, expected) in [
        (false, serde_json::json!(150), 100.0),
        (true, serde_json::json!("175"), 175.0),
        (false, serde_json::json!(150), 100.0),
    ] {
        commands.send(Command::SetVolumeBoost(boost))?;
        write("volume", input)?;
        assert_eq!(
            read(commands, "volume", MpvPropertyFormat::Number)?.as_f64(),
            Some(expected)
        );
    }
    let deadline = || Instant::now() + Duration::from_secs(2);
    commands.run_plugin_command(Command::Volume(180.0), deadline())?;
    assert_eq!(
        read(commands, "volume", MpvPropertyFormat::Number)?.as_f64(),
        Some(100.0)
    );
    commands.run_plugin_command(Command::SetMuted(false), deadline())?;
    assert_eq!(read(commands, "mute", MpvPropertyFormat::Flag)?, false);
    let message = riina_core::PluginMpvCommand::new("show-text", vec!["literal; stop".into()])
        .map_err(|_| EngineError::InvalidArgument {
            reason: "Valid native test command rejected",
        })?;
    commands.run_plugin_command(Command::PluginMpv(message), deadline())?;
    assert!(read(commands, "path", MpvPropertyFormat::Text)?.is_string());
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

#[test]
#[ignore = "requires provisioned libmpv; exercises hook ordering and unload recovery"]
fn hooks_release_loading_in_priority_and_registration_order() -> Result<(), EngineError> {
    let root = fixture_root();
    let harness = Harness::start(&root)?;
    let commands = harness.player.command_sender();
    let (send, receive) = mpsc::channel();
    let register = |label, priority| {
        let send = send.clone();
        commands.register_mpv_hook(
            "on_load",
            priority,
            Instant::now() + Duration::from_secs(3),
            move |next| {
                _ = send.send((label, next));
            },
        )
    };
    let _later = register("later", 0)?;
    let cancelled = register("cancelled", -10)?;
    let _peer = register("peer", -10)?;
    let source = root.join("tests/fixtures/moving-square.avi");
    harness.send(Command::Load(source.to_string_lossy().into_owned()))?;

    let take_hook = |expected| {
        let (label, next) = receive.recv_timeout(Duration::from_secs(3)).unwrap();
        assert_eq!(label, expected);
        assert!(next.is_pending());
        assert!(receive.try_recv().is_err());
        next
    };
    let abandoned = take_hook("cancelled");
    assert!(
        read(&commands, "path", MpvPropertyFormat::Text)?
            .as_str()
            .is_some_and(|path| path.ends_with("moving-square.avi"))
    );
    drop(cancelled);
    let peer = take_hook("peer");
    assert!(!abandoned.is_pending());
    abandoned.finish();
    peer.finish();
    peer.finish();
    drop(take_hook("later"));
    let first = harness.wait(|state| state.phase == Phase::Playing);

    let _replacement = register("replacement", -10)?;
    harness.send(Command::Load(source.to_string_lossy().into_owned()))?;
    drop(take_hook("peer"));
    drop(take_hook("replacement"));
    drop(take_hook("later"));
    harness.wait(|state| state.generation > first.generation && state.phase == Phase::Playing);
    Ok(())
}
