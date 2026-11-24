//! Own the WinRT transport object, event registrations, and last published playback state.
use super::{
    PlaybackStore, PlayerWindow,
    actions::{Action, Destination},
};
use ::windows::{
    Foundation::{TimeSpan, TypedEventHandler},
    Media::{
        MediaPlaybackStatus, MediaPlaybackType, PlaybackPositionChangeRequestedEventArgs,
        SystemMediaTransportControls as Controls,
        SystemMediaTransportControlsButtonPressedEventArgs,
        SystemMediaTransportControlsTimelineProperties,
    },
    Win32::{
        Foundation::HWND,
        System::WinRT::{
            ISystemMediaTransportControlsInterop, RO_INIT_SINGLETHREADED, RoInitialize,
            RoUninitialize,
        },
    },
    core::{HSTRING, factory},
};
use riina_core::{Phase, Playback, TrackKind};
use slint::Weak;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

struct Apartment;

impl Apartment {
    fn current_ui_thread() -> ::windows::core::Result<Self> {
        // SAFETY: this UI thread owns the native controls and presents a Win32 toplevel.
        unsafe { RoInitialize(RO_INIT_SINGLETHREADED) }?;
        Ok(Self)
    }
}

impl Drop for Apartment {
    fn drop(&mut self) {
        // SAFETY: the registration keeps this object on the initializing UI thread.
        unsafe { RoUninitialize() };
    }
}

pub(super) struct Registration {
    controls: Controls,
    pressed: Option<i64>,
    seek: Option<i64>,
    enabled: Arc<AtomicBool>,
    generation: Arc<AtomicU64>,
    last: Option<Projection>,
    _apartment: Apartment,
}

impl Registration {
    pub(super) fn open(
        window: HWND,
        app: Weak<PlayerWindow>,
        latest: PlaybackStore,
    ) -> ::windows::core::Result<Self> {
        let apartment = Apartment::current_ui_thread()?;
        let interop = factory::<Controls, ISystemMediaTransportControlsInterop>()?;
        // SAFETY: window is the live foreground-capable top-level Slint/Winit window on this UI thread.
        let controls = unsafe { interop.GetForWindow::<Controls>(window) }?;
        let enabled = Arc::new(AtomicBool::new(false));
        let generation = Arc::new(AtomicU64::new(0));
        let destination = Destination::new(app, latest, enabled.clone(), generation.clone());
        let mut state = Self {
            controls,
            pressed: None,
            seek: None,
            enabled,
            generation,
            last: None,
            _apartment: apartment,
        };
        let button = destination.clone();
        state.pressed = Some(state.controls.ButtonPressed(&TypedEventHandler::<
            Controls,
            SystemMediaTransportControlsButtonPressedEventArgs,
        >::new(move |_, event| {
            if let Some(event) = event.as_ref() {
                button.post(Action::Button(event.Button()?));
            }
            Ok(())
        }))?);
        state.seek = Some(
            state
                .controls
                .PlaybackPositionChangeRequested(&TypedEventHandler::<
                    Controls,
                    PlaybackPositionChangeRequestedEventArgs,
                >::new(move |_, event| {
                    if let Some(event) = event.as_ref() {
                        let ticks = event.RequestedPlaybackPosition()?.Duration;
                        if ticks >= 0 {
                            destination.post(Action::Position(ticks as f64 / 10_000_000.0));
                        }
                    }
                    Ok(())
                }))?,
        );
        state.controls.SetIsPlayEnabled(true)?;
        state.controls.SetIsPauseEnabled(true)?;
        state.controls.SetIsStopEnabled(true)?;
        Ok(state)
    }

    pub(super) fn synchronize(&mut self, playback: &Playback) -> ::windows::core::Result<()> {
        let now = Projection::read(playback);
        let last = self.last.as_ref();
        if last.is_none_or(|last| last.media != now.media) {
            now.media.publish(&self.controls)?;
        }
        if last.is_none_or(|last| last.status != now.status) {
            self.controls.SetPlaybackStatus(now.status)?;
        }
        if last.is_none_or(|last| (last.next, last.previous) != (now.next, now.previous)) {
            self.controls.SetIsNextEnabled(now.next)?;
            self.controls.SetIsPreviousEnabled(now.previous)?;
        }
        if last.is_none_or(|last| last.seekable != now.seekable) {
            self.controls.SetIsFastForwardEnabled(now.seekable)?;
            self.controls.SetIsRewindEnabled(now.seekable)?;
        }
        if last.is_none_or(|last| {
            last.media.generation != now.media.generation
                || (last.position, last.duration, last.seekable)
                    != (now.position, now.duration, now.seekable)
        }) {
            now.update_timeline(&self.controls)?;
        }
        self.generation
            .store(now.media.generation, Ordering::Release);
        if self.last.is_none() {
            self.controls.SetIsEnabled(true)?;
            self.enabled.store(true, Ordering::Release);
        }
        self.last = Some(now);
        Ok(())
    }
}

impl Drop for Registration {
    fn drop(&mut self) {
        self.enabled.store(false, Ordering::Release);
        let _ = self.controls.SetIsEnabled(false);
        if let Some(token) = self.seek.take() {
            let _ = self.controls.RemovePlaybackPositionChangeRequested(token);
        }
        if let Some(token) = self.pressed.take() {
            let _ = self.controls.RemoveButtonPressed(token);
        }
        let _ = self
            .controls
            .DisplayUpdater()
            .and_then(|updater| updater.ClearAll());
    }
}

#[derive(PartialEq)]
struct Metadata {
    generation: u64,
    title: String,
    artist: String,
    album: String,
    audio_only: bool,
}

impl Metadata {
    fn publish(&self, controls: &Controls) -> ::windows::core::Result<()> {
        let updater = controls.DisplayUpdater()?;
        updater.ClearAll()?;
        let title = HSTRING::from(&self.title);
        if self.audio_only {
            updater.SetType(MediaPlaybackType::Music)?;
            let music = updater.MusicProperties()?;
            music.SetTitle(&title)?;
            music.SetArtist(&HSTRING::from(&self.artist))?;
            music.SetAlbumTitle(&HSTRING::from(&self.album))?;
        } else {
            updater.SetType(MediaPlaybackType::Video)?;
            updater.VideoProperties()?.SetTitle(&title)?;
        }
        updater.Update()
    }
}

struct Projection {
    media: Metadata,
    status: MediaPlaybackStatus,
    next: bool,
    previous: bool,
    seekable: bool,
    position: i64,
    duration: i64,
}

impl Projection {
    fn read(playback: &Playback) -> Self {
        let current = playback.playlist.iter().position(|entry| entry.current);
        let position = ticks(playback.position);
        let duration = ticks(playback.duration).max(position);
        Self {
            media: Metadata {
                generation: playback.generation,
                title: playback.title.clone(),
                artist: playback.artist.clone().unwrap_or_default(),
                album: playback.album.clone().unwrap_or_default(),
                audio_only: !playback
                    .tracks
                    .iter()
                    .any(|track| track.kind == TrackKind::Video && !track.albumart),
            },
            status: if matches!(
                playback.phase,
                Phase::Ended | Phase::Stopped | Phase::Failed
            ) {
                MediaPlaybackStatus::Stopped
            } else if playback.paused {
                MediaPlaybackStatus::Paused
            } else if matches!(playback.phase, Phase::Loading | Phase::Buffering) {
                MediaPlaybackStatus::Changing
            } else {
                MediaPlaybackStatus::Playing
            },
            next: current.is_some_and(|index| index + 1 < playback.playlist.len()),
            previous: current.is_some_and(|index| index > 0),
            seekable: playback.seekable,
            position,
            duration,
        }
    }

    fn update_timeline(&self, controls: &Controls) -> ::windows::core::Result<()> {
        let timeline = SystemMediaTransportControlsTimelineProperties::new()?;
        timeline.SetEndTime(TimeSpan {
            Duration: self.duration,
        })?;
        timeline.SetPosition(TimeSpan {
            Duration: self.position,
        })?;
        timeline.SetMaxSeekTime(TimeSpan {
            Duration: if self.seekable { self.duration } else { 0 },
        })?;
        controls.UpdateTimelineProperties(&timeline)
    }
}

fn ticks(seconds: Option<f64>) -> i64 {
    (seconds
        .filter(|value| value.is_finite())
        .unwrap_or_default()
        .max(0.0)
        * 10_000_000.0) as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires the native Windows system media service and an isolated desktop"]
    fn owned_registration_publishes_and_clears_native_metadata() {
        use ::windows::{
            Win32::UI::WindowsAndMessaging::{CreateWindowExW, DestroyWindow, WS_OVERLAPPEDWINDOW},
            core::w,
        };

        struct TestWindow(HWND);
        impl Drop for TestWindow {
            fn drop(&mut self) {
                // SAFETY: this test created the window on the current thread.
                let _ = unsafe { DestroyWindow(self.0) };
            }
        }

        let _apartment = Apartment::current_ui_thread().unwrap();
        // SAFETY: STATIC is a predefined class and each optional pointer is absent.
        let window = TestWindow(
            unsafe {
                CreateWindowExW(
                    Default::default(),
                    w!("STATIC"),
                    w!("Riina media test"),
                    WS_OVERLAPPEDWINDOW,
                    0,
                    0,
                    320,
                    180,
                    None,
                    None,
                    None,
                    None,
                )
            }
            .unwrap(),
        );
        let mut registration =
            Registration::open(window.0, Weak::default(), PlaybackStore::default()).unwrap();
        let playback = Playback {
            generation: 17,
            title: "Riina native isolated fixture".into(),
            phase: Phase::Paused,
            paused: true,
            seekable: true,
            position: Some(0.5),
            duration: Some(5.0),
            ..Playback::default()
        };
        registration.synchronize(&playback).unwrap();
        let controls = registration.controls.clone();
        assert!(controls.IsEnabled().unwrap());
        assert_eq!(
            controls.PlaybackStatus().unwrap(),
            MediaPlaybackStatus::Paused
        );
        assert_eq!(
            controls
                .DisplayUpdater()
                .unwrap()
                .MusicProperties()
                .unwrap()
                .Title()
                .unwrap()
                .to_string_lossy(),
            playback.title,
        );

        drop(registration);
        assert!(!controls.IsEnabled().unwrap());
        assert_eq!(
            controls.DisplayUpdater().unwrap().Type().unwrap(),
            MediaPlaybackType::Unknown
        );
    }

    #[test]
    fn shell_projection_reports_pause_and_end_with_nonnegative_timeline() {
        let mut playback = Playback {
            generation: 3,
            title: "Fixture".into(),
            phase: Phase::Paused,
            paused: true,
            seekable: true,
            position: Some(2.5),
            duration: Some(5.0),
            ..Playback::default()
        };
        let paused = Projection::read(&playback);
        assert_eq!(paused.status, MediaPlaybackStatus::Paused);
        assert_eq!(paused.position, 25_000_000);
        assert_eq!(paused.duration, 50_000_000);
        assert_eq!(paused.media.generation, 3);

        playback.phase = Phase::Ended;
        playback.paused = false;
        playback.position = Some(-1.0);
        playback.duration = Some(f64::NAN);
        let ended = Projection::read(&playback);
        assert_eq!(ended.status, MediaPlaybackStatus::Stopped);
        assert_eq!((ended.position, ended.duration), (0, 0));
    }
}
