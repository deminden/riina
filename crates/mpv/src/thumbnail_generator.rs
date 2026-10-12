//! Independent, paused libmpv decoder used for timeline thumbnails.
//! It never seeks or changes state on the player's audible playback instance.

use crate::Player;
use riina_core::{CapturedFrame, Command, Phase, Playback};
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

pub const SAMPLE_COUNT: u32 = 100;
const EVENT_QUEUE_CAPACITY: usize = 4;
const MAX_THUMBNAIL_DIMENSION: u32 = 8192;

#[derive(Debug, Clone)]
pub struct Sample {
    pub index: u32,
    pub timestamp: f64,
    pub frame: Arc<CapturedFrame>,
}

#[derive(Debug)]
pub enum Event {
    Sample(Sample),
    Finished(Result<(), String>),
}

struct CapturePlan {
    maximum_width: u32,
    video_track_id: Option<i64>,
    sample_intervals: u32,
    single_frame_time: Option<f64>,
}

/// Owns a separate silent decoder and its bounded progress stream.
pub struct Generator {
    cancel: Arc<AtomicBool>,
    events: Option<Receiver<Event>>,
    thread: Option<JoinHandle<()>>,
}

impl Generator {
    /// Begins extracting IINA's 101 evenly spaced preview frames without
    /// interrupting the player that is currently rendering and playing audio.
    pub fn start(library: &Path, media: &str, maximum_width: u32) -> Result<Self, String> {
        Self::start_inner(library, media, maximum_width, None, SAMPLE_COUNT, None)
    }

    /// Captures one frame at the start of a media item without seeking the player.
    pub fn start_first_frame(
        library: &Path,
        media: &str,
        maximum_width: u32,
    ) -> Result<Self, String> {
        Self::start_inner(library, media, maximum_width, None, 0, None)
    }

    /// Captures a single preview frame at the requested media time.
    pub fn start_at_time(
        library: &Path,
        media: &str,
        maximum_width: u32,
        seconds: f64,
    ) -> Result<Self, String> {
        if !seconds.is_finite() || seconds < 0.0 {
            return Err("Thumbnail time must be a finite non-negative value".into());
        }
        Self::start_inner(library, media, maximum_width, None, 0, Some(seconds))
    }

    /// Begins a one-media decode with the requested video track selected before capture.
    pub fn start_for_video_track(
        library: &Path,
        media: &str,
        maximum_width: u32,
        track_id: i64,
    ) -> Result<Self, String> {
        if track_id <= 0 {
            return Err("Video track IDs must be positive".into());
        }
        Self::start_inner(library, media, maximum_width, Some(track_id), 0, None)
    }

    fn start_inner(
        library: &Path,
        media: &str,
        maximum_width: u32,
        video_track_id: Option<i64>,
        sample_intervals: u32,
        single_frame_time: Option<f64>,
    ) -> Result<Self, String> {
        if !Path::new(media).is_absolute() && !media.contains("://") {
            return Err("Thumbnail media paths must be absolute".into());
        }
        if maximum_width == 0 || maximum_width > MAX_THUMBNAIL_DIMENSION {
            return Err("Thumbnail width is outside the supported range".into());
        }
        let library = library.to_path_buf();
        let media = media.to_owned();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let (sender, events) = mpsc::sync_channel(EVENT_QUEUE_CAPACITY);
        let thread = thread::Builder::new()
            .name("riina-thumbnails".into())
            .spawn(move || {
                let result = extract(
                    &library,
                    &media,
                    CapturePlan {
                        maximum_width,
                        video_track_id,
                        sample_intervals,
                        single_frame_time,
                    },
                    &sender,
                    &worker_cancel,
                );
                send_terminal_event(&sender, Event::Finished(result));
            })
            .map_err(|error| format!("Could not start thumbnail extraction: {error}"))?;
        Ok(Self {
            cancel,
            events: Some(events),
            thread: Some(thread),
        })
    }

    /// Reads a generated frame or completion result without blocking the UI.
    pub fn try_recv(&self) -> Result<Event, TryRecvError> {
        self.events
            .as_ref()
            .expect("thumbnail event stream remains available until drop")
            .try_recv()
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Release);
    }
}

impl Drop for Generator {
    fn drop(&mut self) {
        self.cancel();
        // Dropping the receiver wakes a worker blocked by backpressure before joining it.
        drop(self.events.take());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn extract(
    library: &Path,
    media: &str,
    plan: CapturePlan,
    events: &SyncSender<Event>,
    cancel: &AtomicBool,
) -> Result<(), String> {
    let CapturePlan {
        maximum_width,
        video_track_id,
        sample_intervals,
        single_frame_time,
    } = plan;
    let (state_sender, state_receiver) = mpsc::channel();
    let player = Player::start_with_options(
        library,
        true,
        vec![("audio".into(), "no".into()), ("hwdec".into(), "no".into())],
        move |state| {
            let _ = state_sender.send(state);
        },
    )?;
    let commands = player.command_sender();
    commands.send(Command::SetPauseWhenOpen(true))?;
    commands.send(Command::Load(media.to_owned()))?;
    let mut state = wait_for(&state_receiver, cancel, |state| {
        state.phase == Phase::Paused && state.duration.is_some()
    })?;
    if let Some(track_id) = video_track_id {
        commands.send(Command::SelectTrack {
            kind: riina_core::TrackKind::Video,
            id: Some(track_id),
        })?;
        state = wait_for(&state_receiver, cancel, |state| {
            state.tracks.iter().any(|track| {
                track.kind == riina_core::TrackKind::Video && track.id == track_id && track.selected
            })
        })?;
    }
    let duration = state
        .duration
        .filter(|duration| duration.is_finite() && *duration > 0.0)
        .ok_or("The media duration is unavailable for thumbnail extraction")?;
    if state.video.is_none() && state.video_target.is_none() && state.video_output.is_none() {
        return Err("The media has no video frames to preview".into());
    }

    let mut previous_seek = state.completed_seeks;
    let first_timestamp = single_frame_time
        .unwrap_or(0.0)
        .min((duration - 0.001).max(0.0));
    for index in 0..=sample_intervals {
        if cancel.load(Ordering::Acquire) {
            return Err("Thumbnail extraction was cancelled".into());
        }
        let timestamp = if index == 0 {
            first_timestamp
        } else {
            (duration * f64::from(index) / f64::from(sample_intervals))
                .min((duration - 0.001).max(0.0))
        };
        if timestamp > 0.0 {
            commands.send(Command::Seek {
                seconds: timestamp,
                absolute: true,
            })?;
            state = wait_for(&state_receiver, cancel, |state| {
                state.completed_seeks > previous_seek && !state.seeking && state.paused
            })?;
            previous_seek = state.completed_seeks;
        }
        let captures = state.completed_frame_captures;
        commands.send(Command::CaptureFrame)?;
        state = wait_for(&state_receiver, cancel, |state| {
            state.completed_frame_captures > captures
        })?;
        let frame = state
            .captured_frame
            .as_ref()
            .ok_or("libmpv completed a frame capture without returning pixels")?;
        let frame = Arc::new(resize_frame(frame, maximum_width)?);
        if !send_event(
            events,
            Event::Sample(Sample {
                index,
                timestamp,
                frame,
            }),
            cancel,
        ) {
            return Err("Thumbnail extraction was cancelled".into());
        }
    }
    Ok(())
}

fn wait_for(
    receiver: &Receiver<Playback>,
    cancel: &AtomicBool,
    predicate: impl Fn(&Playback) -> bool,
) -> Result<Playback, String> {
    loop {
        if cancel.load(Ordering::Acquire) {
            return Err("Thumbnail extraction was cancelled".into());
        }
        match receiver.recv_timeout(Duration::from_millis(100)) {
            Ok(state) => {
                if let Some(error) = state.error {
                    return Err(error);
                }
                if predicate(&state) {
                    return Ok(state);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err("The thumbnail decoder stopped unexpectedly".into());
            }
        }
    }
}

fn send_event(sender: &SyncSender<Event>, mut event: Event, cancel: &AtomicBool) -> bool {
    loop {
        if cancel.load(Ordering::Acquire) {
            return false;
        }
        match sender.try_send(event) {
            Ok(()) => return true,
            Err(TrySendError::Disconnected(_)) => return false,
            Err(TrySendError::Full(returned)) => {
                event = returned;
                thread::sleep(Duration::from_millis(10));
            }
        }
    }
}

fn send_terminal_event(sender: &SyncSender<Event>, mut event: Event) -> bool {
    loop {
        match sender.try_send(event) {
            Ok(()) => return true,
            Err(TrySendError::Disconnected(_)) => return false,
            Err(TrySendError::Full(returned)) => {
                event = returned;
                thread::sleep(Duration::from_millis(10));
            }
        }
    }
}

fn resize_frame(frame: &CapturedFrame, maximum_width: u32) -> Result<CapturedFrame, String> {
    let scale = (f64::from(maximum_width) / f64::from(frame.width))
        .min(f64::from(MAX_THUMBNAIL_DIMENSION) / f64::from(frame.height))
        .min(1.0);
    let width = (f64::from(frame.width) * scale).round().max(1.0) as u32;
    let height = (f64::from(frame.height) * scale).round().max(1.0) as u32;
    if (width, height) == (frame.width, frame.height) {
        return Ok(frame.clone());
    }

    let mut pixels = vec![0u8; width as usize * height as usize * 4];
    for y in 0..height {
        let source_y = (f64::from(y) + 0.5) * f64::from(frame.height) / f64::from(height) - 0.5;
        let y0 = source_y.floor().max(0.0) as u32;
        let y1 = (y0 + 1).min(frame.height - 1);
        let fy = (source_y - f64::from(y0)).clamp(0.0, 1.0);
        for x in 0..width {
            let source_x = (f64::from(x) + 0.5) * f64::from(frame.width) / f64::from(width) - 0.5;
            let x0 = source_x.floor().max(0.0) as u32;
            let x1 = (x0 + 1).min(frame.width - 1);
            let fx = (source_x - f64::from(x0)).clamp(0.0, 1.0);
            let destination = ((y * width + x) * 4) as usize;
            for channel in 0..4 {
                let at = |sx: u32, sy: u32| {
                    frame.rgba[((sy * frame.width + sx) * 4) as usize + channel] as f64
                };
                let top = at(x0, y0) * (1.0 - fx) + at(x1, y0) * fx;
                let bottom = at(x0, y1) * (1.0 - fx) + at(x1, y1) * fx;
                pixels[destination + channel] = (top * (1.0 - fy) + bottom * fy).round() as u8;
            }
        }
    }
    CapturedFrame::new(width, height, width as usize * 4, &pixels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downscaling_preserves_aspect_and_exact_rgba_storage() {
        let frame = CapturedFrame::new(4, 2, 16, &(0..32).collect::<Vec<u8>>()).unwrap();
        let scaled = resize_frame(&frame, 2).unwrap();
        assert_eq!((scaled.width, scaled.height), (2, 1));
        assert_eq!(scaled.rgba.len(), 8);
    }

    #[test]
    fn upscaling_is_skipped_and_tall_frames_stay_bounded() {
        let frame = CapturedFrame::new(2, 4, 8, &(0..32).collect::<Vec<u8>>()).unwrap();
        let unchanged = resize_frame(&frame, 4).unwrap();
        assert_eq!((unchanged.width, unchanged.height), (2, 4));
        let tall = CapturedFrame::new(1, 16_000, 4, &vec![0; 64_000]).unwrap();
        let bounded = resize_frame(&tall, 2).unwrap();
        assert_eq!(bounded.height, MAX_THUMBNAIL_DIMENSION);
    }

    #[test]
    fn cancellation_keeps_the_terminal_event_available() {
        let (sender, receiver) = mpsc::sync_channel(1);
        assert!(send_terminal_event(
            &sender,
            Event::Finished(Err("Thumbnail extraction was cancelled".into()))
        ));
        assert!(matches!(
            receiver.try_recv(),
            Ok(Event::Finished(Err(error))) if error == "Thumbnail extraction was cancelled"
        ));
    }
}
