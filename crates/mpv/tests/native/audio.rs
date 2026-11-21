use super::*;

#[test]
#[ignore = "requires provisioned libmpv; verifies audio-device selection through observed property state"]
fn audio_device_selection_updates_the_live_engine() {
    let root = fixture_root();
    let harness = Harness::with_options(&root, vec![("ao".into(), "null".into())]).unwrap();
    let initial = harness.wait(|state| state.audio_device.is_some());
    assert_eq!(initial.error, None);
    eprintln!(
        "audio output driver: {:?}; observed devices: {}",
        initial.audio_output,
        initial.audio_devices.len()
    );
    if initial.audio_devices.is_empty() {
        // Some headless Windows sessions expose the AO but no endpoint list.
        drop(harness);
        return;
    }
    assert!(
        initial
            .audio_devices
            .iter()
            .any(|device| { Some(device.name.as_str()) == initial.audio_device.as_deref() })
    );
    let alternative = initial
        .audio_devices
        .iter()
        .find(|device| Some(device.name.as_str()) != initial.audio_device.as_deref());
    eprintln!(
        "observed {} audio device(s); alternate selection available: {}",
        initial.audio_devices.len(),
        alternative.is_some()
    );
    if let Some(alternative) = alternative {
        let selected = alternative.name.clone();
        harness
            .send(Command::AudioDevice(selected.clone()))
            .unwrap();
        let changed =
            harness.wait(|state| state.audio_device.as_deref() == Some(selected.as_str()));
        assert_eq!(changed.error, None);
    }
    drop(harness);
}

#[test]
#[ignore = "requires provisioned libmpv; verifies live equalizer filter replacement and observation"]
fn audio_equalizer_applies_updates_and_returns_to_flat() {
    let root = fixture_root();
    let harness = Harness::with_options(&root, vec![("ao".into(), "null".into())]).unwrap();
    harness
        .send(Command::Load(
            root.join("tests/fixtures/external-tone.wav")
                .to_string_lossy()
                .into_owned(),
        ))
        .unwrap();
    let loaded = harness.wait(|state| {
        state.error.is_some() || (state.phase == Phase::Playing && state.audio_equalizer.is_some())
    });
    assert_eq!(loaded.error, None);
    assert_eq!(loaded.audio_equalizer, Some([0.; 10]));

    let mut gains = [0.; 10];
    gains[4] = 4.5;
    harness.send(Command::AudioEqualizer(gains)).unwrap();
    let applied = harness.wait(|state| {
        state.error.is_some()
            || state.audio_equalizer.is_some_and(|observed| {
                observed
                    .iter()
                    .zip(gains)
                    .all(|(actual, expected)| (actual - expected).abs() < 0.01)
            })
    });
    assert_eq!(applied.error, None);

    harness.send(Command::AudioEqualizer([0.; 10])).unwrap();
    let reset =
        harness.wait(|state| state.error.is_some() || state.audio_equalizer == Some([0.; 10]));
    assert_eq!(reset.error, None);
    drop(harness);
}
