//! Run explicitly: cargo test -p riina-mpv --test native --locked -- --ignored --nocapture
use riina_core::{Command, Phase, Playback, TrackKind};
use riina_mpv::Player;
use std::{
    fs::File,
    io::{BufRead, BufReader, BufWriter, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    sync::mpsc,
    time::{Duration, Instant},
};

mod audio;
mod crop;
mod demuxer_tests;
mod filters;
mod harness;
mod lifecycle;
mod loading;
mod network;
mod observations;
mod playlist;
mod screenshots;
mod seeking;
mod subtitles;
mod tracks;
mod video;

use harness::*;

// Keep this test at the crate root for the exact filter used by the package workflow.
#[test]
#[ignore = "requires provisioned libmpv; exercises real native playback"]
#[expect(
    clippy::too_many_lines,
    reason = "decomposition tracked in docs/review"
)]
fn playback_pause_seek_stop_failure_and_shutdown() {
    let root = fixture_root();
    let Harness { player, receiver } = Harness::start(&root)
    .expect("Prerequisite failure: provision libmpv and select its absolute path with RIINA_TEST_MPV_LIBRARY");
    player
        .send(Command::Load(
            root.join("tests/fixtures/moving-square.avi")
                .to_str()
                .unwrap()
                .into(),
        ))
        .unwrap();
    let loaded = wait_for(&receiver, |state| {
        state.phase == Phase::Playing && state.position.is_some_and(|p| p > 0.2)
    });
    assert!(loaded.duration.is_some_and(|d| (d - 5.0).abs() < 0.1));
    assert!(
        loaded.audio_output.is_some(),
        "No active native audio output was observed"
    );
    // Volume and speed controls take longer than this five-second fixture on
    // some software audio backends. Keep it alive until the explicit EOF check.
    player.send(Command::CycleLoop).unwrap();
    wait_for(&receiver, |state| {
        state.loop_mode() == riina_core::LoopMode::File
    });
    let catalog = wait_for(&receiver, |state| {
        state
            .tracks
            .iter()
            .any(|track| track.kind == TrackKind::Audio && track.selected)
            && state
                .tracks
                .iter()
                .any(|track| track.kind == TrackKind::Video)
            && state.playlist.len() == 1
    });
    // Freeze the short clip while the test changes its selected audio track.
    player.send(Command::SetPaused(true)).unwrap();
    wait_for(&receiver, |state| state.paused);
    let audio_id = catalog
        .tracks
        .iter()
        .find(|track| track.kind == TrackKind::Audio)
        .unwrap()
        .id;
    player
        .send(Command::SelectTrack {
            kind: TrackKind::Audio,
            id: None,
        })
        .unwrap();
    wait_for(&receiver, |state| {
        !state
            .tracks
            .iter()
            .any(|track| track.kind == TrackKind::Audio && track.selected)
    });
    player
        .send(Command::SelectTrack {
            kind: TrackKind::Audio,
            id: Some(audio_id),
        })
        .unwrap();
    wait_for(&receiver, |state| {
        state
            .tracks
            .iter()
            .any(|track| track.kind == TrackKind::Audio && track.id == audio_id && track.selected)
    });
    // The transport button dispatches TogglePause, so cover that exact native
    // command path before exercising the idempotent preference-oriented setter.
    player.send(Command::SetPaused(false)).unwrap();
    wait_for(&receiver, |state| {
        state.phase == Phase::Playing && !state.paused
    });
    player.send(Command::TogglePause).unwrap();
    let paused = wait_for(&receiver, |state| state.phase == Phase::Paused);
    let paused_position = paused.position;
    let stable_until = Instant::now() + Duration::from_millis(250);
    while Instant::now() < stable_until {
        let remaining = stable_until.saturating_duration_since(Instant::now());
        match receiver.recv_timeout(remaining) {
            Ok(state) => {
                assert!(state.paused, "Playback resumed without a command");
                assert!(match (state.position, paused_position) {
                    (Some(current), Some(at_pause)) => (current - at_pause).abs() < 0.05,
                    _ => false,
                });
            }
            Err(mpsc::RecvTimeoutError::Timeout) => break,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                panic!("Playback state stream ended while the player was paused")
            }
        }
    }
    player.send(Command::TogglePause).unwrap();
    wait_for(&receiver, |state| {
        state.phase == Phase::Playing && !state.paused
    });
    player.send(Command::SetPaused(true)).unwrap();
    wait_for(&receiver, |state| state.paused);
    player
        .send(Command::Seek {
            seconds: 2.0,
            absolute: true,
        })
        .unwrap();
    let seeked = wait_for(&receiver, |state| {
        state.paused
            && state.position.is_some_and(|p| (p - 2.0).abs() < 0.15)
            && state.completed_seeks > paused.completed_seeks
    });
    assert_eq!(paused.generation, seeked.generation);
    player.send(Command::SetPaused(false)).unwrap();
    wait_for(&receiver, |state| {
        state.phase == Phase::Playing && !state.paused
    });
    player.send(Command::SetPaused(true)).unwrap();
    wait_for(&receiver, |state| {
        state.phase == Phase::Paused && state.paused
    });
    player.send(Command::Volume(50.0)).unwrap();
    wait_for(&receiver, |state| (state.volume - 50.0).abs() < 0.1);
    for _ in 0..12 {
        player
            .send(Command::AdjustVolume {
                delta: 5.0,
                boost: false,
            })
            .unwrap();
    }
    let capped = wait_for(&receiver, |state| (state.volume - 100.0).abs() < 0.1);
    assert!(capped.paused);
    assert_eq!(capped.error(), None);
    for _ in 0..22 {
        player
            .send(Command::AdjustVolume {
                delta: -5.0,
                boost: false,
            })
            .unwrap();
    }
    wait_for(&receiver, |state| state.volume.abs() < 0.1);
    player.send(Command::SetVolumeBoost(true)).unwrap();
    for volume in [150.0, 200.0] {
        player.send(Command::Volume(volume)).unwrap();
        wait_for(&receiver, |state| (state.volume - volume).abs() < 0.1);
    }
    player.send(Command::Volume(150.0)).unwrap();
    wait_for(&receiver, |state| (state.volume - 150.0).abs() < 0.1);
    for _ in 0..12 {
        player
            .send(Command::AdjustVolume {
                delta: 5.0,
                boost: true,
            })
            .unwrap();
    }
    wait_for(&receiver, |state| (state.volume - 200.0).abs() < 0.1);
    player.send(Command::SetVolumeBoost(false)).unwrap();
    wait_for(&receiver, |state| (state.volume - 100.0).abs() < 0.1);
    player.send(Command::Speed(1.0)).unwrap();
    wait_for(&receiver, |state| (state.speed - 1.0).abs() < 0.001);
    for _ in 0..3 {
        player.send(Command::MultiplySpeed(2.0)).unwrap();
    }
    wait_for(&receiver, |state| (state.speed - 8.0).abs() < 0.001);
    for _ in 0..3 {
        player.send(Command::MultiplySpeed(0.5)).unwrap();
    }
    wait_for(&receiver, |state| (state.speed - 1.0).abs() < 0.001);
    for _ in 0..10 {
        player.send(Command::MultiplySpeed(1.1)).unwrap();
    }
    wait_for(&receiver, |state| {
        (state.speed - 1.1_f64.powi(10)).abs() < 0.001
    });
    player.send(Command::Speed(1.0)).unwrap();
    let normal = wait_for(&receiver, |state| (state.speed - 1.0).abs() < 0.001);
    assert!(normal.paused);
    assert_eq!(normal.error(), None);
    player.send(Command::Volume(25.0)).unwrap();
    wait_for(&receiver, |state| (state.volume - 25.0).abs() < 0.1);
    player.send(Command::ToggleMute).unwrap();
    wait_for(&receiver, |state| state.muted);
    player.send(Command::CycleLoop).unwrap();
    wait_for(&receiver, |state| {
        state.loop_mode() == riina_core::LoopMode::Playlist
    });
    player.send(Command::CycleLoop).unwrap();
    wait_for(&receiver, |state| {
        state.loop_mode() == riina_core::LoopMode::Off
    });
    player.send(Command::TogglePause).unwrap();
    player
        .send(Command::Seek {
            seconds: 4.8,
            absolute: true,
        })
        .unwrap();
    let ended = wait_for(&receiver, |state| state.phase == Phase::Ended);
    assert_eq!(ended.stopped_by, None);
    player.send(Command::Stop(Some(24))).unwrap();
    wait_for(&receiver, |state| state.confirmed_stop(24));
    player.send(Command::Stop(Some(25))).unwrap();
    wait_for(&receiver, |state| state.confirmed_stop(25));
    player
        .send(Command::Load(
            root.join("tests/fixtures/does-not-exist.mkv")
                .to_str()
                .unwrap()
                .into(),
        ))
        .unwrap();
    let failed = wait_for(&receiver, |state| {
        state.phase == Phase::Failed && state.error().is_some()
    });
    assert_eq!(failed.stopped_by, None);
    player
        .send(Command::Load(
            root.join("tests/fixtures/moving-square.avi")
                .to_str()
                .unwrap()
                .into(),
        ))
        .unwrap();
    let reloaded = wait_for(&receiver, |state| {
        state.generation > loaded.generation
            && state.phase == Phase::Playing
            && !state.paused
            && state.position.is_some_and(|p| p > 0.2)
    });
    assert!(reloaded.error().is_none());
    drop(player);
}
