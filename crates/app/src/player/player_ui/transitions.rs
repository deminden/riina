//! Collect outgoing and incoming transition frames around playlist navigation.
use super::PendingNavigation;
use crate::{NativeVideoHost, platform, transition_frame};
use riina_core::{Command, Phase, Playback, sync::lock};
use riina_mpv::CommandSender;
use slint::ComponentHandle;
use std::{
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

const INCOMING_SURFACE_STABLE_FOR: Duration = Duration::from_millis(300);
const MANUAL_TRANSITION_CAPTURE_TIMEOUT: Duration = Duration::from_millis(500);

pub(super) fn install(
    capture_transition_first: bool,
    native: Option<&platform::NativePresentation>,
    command_sender: &CommandSender,
    latest_playback: &crate::PlaybackStore,
    pending_navigation: &PendingNavigation,
    transition_release_generation: &Arc<AtomicU64>,
) -> Option<Rc<slint::Timer>> {
    if !capture_transition_first {
        return None;
    }
    let playback = latest_playback.clone();
    let mut sampler = Sampler {
        sender: command_sender.clone(),
        navigation: pending_navigation.clone(),
        host: native.map(platform::NativePresentation::host),
        released_generation: transition_release_generation.clone(),
        outgoing_generation: None,
        outgoing_attempt: None,
        incoming_attempt: None,
        incoming_frame: None,
        incoming_surface: None,
    };
    let timer = Rc::new(slint::Timer::default());
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(80),
        move || {
            let state = lock(&playback).clone();
            sampler.sample(&state);
        },
    );
    Some(timer)
}

struct Sampler {
    sender: CommandSender,
    navigation: PendingNavigation,
    host: Option<slint::Weak<NativeVideoHost>>,
    released_generation: Arc<AtomicU64>,
    outgoing_generation: Option<u64>,
    outgoing_attempt: Option<(u64, Instant)>,
    incoming_attempt: Option<(u64, Instant)>,
    incoming_frame: Option<IncomingFrame>,
    incoming_surface: Option<IncomingSurface>,
}

struct IncomingFrame {
    generation: u64,
    revision: u64,
    count: u8,
    since: Instant,
}

struct IncomingSurface {
    generation: u64,
    size: slint::PhysicalSize,
    signature: u64,
    since: Instant,
}

impl Sampler {
    fn sample(&mut self, state: &Playback) {
        self.release_navigation(state);
        self.capture_outgoing(state);
        if state
            .transition_frame
            .generation
            .is_some_and(|generation| generation < state.generation)
            && transition_frame::incoming_has_video(state)
            && !matches!(state.phase, Phase::Loading | Phase::Stopped | Phase::Failed)
            && transition_frame::incoming_capture_window_active(state, Instant::now())
            && transition_frame::incoming_capture_ready(state)
        {
            self.capture_incoming(state, Instant::now());
        }
    }

    fn release_navigation(&self, state: &Playback) {
        let mut pending = self.navigation.borrow_mut();
        let ready = pending.as_ref().is_some_and(|(_, requested_at)| {
            state.transition_frame.generation == Some(state.generation)
                && state
                    .transition_frame
                    .captured_at
                    .is_some_and(|at| at >= *requested_at)
                || requested_at.elapsed() >= MANUAL_TRANSITION_CAPTURE_TIMEOUT
        });
        if !ready {
            return;
        }
        let navigation = pending.take();
        drop(pending);
        if let Some((commands, _)) = navigation {
            for command in commands {
                if let Err(error) = self.sender.send(command) {
                    crate::diagnostics::warn!("Could not navigate the playlist: {error}");
                }
            }
        }
    }

    fn capture_outgoing(&mut self, state: &Playback) {
        if state.transition_frame.generation == Some(state.generation) {
            // A successful reply confirms that the frame survived D3D11 reconfiguration.
            self.outgoing_generation = Some(state.generation);
            self.outgoing_attempt = None;
        }
        let now = Instant::now();
        if !transition_frame::outgoing_capture_due(
            state,
            self.outgoing_generation,
            self.outgoing_attempt,
            now,
        ) {
            return;
        }
        // Retry while time remains before the playlist advances.
        match self.sender.send(Command::CaptureTransitionFrame) {
            Ok(()) => self.outgoing_attempt = Some((state.generation, now)),
            Err(error) => {
                self.outgoing_attempt = None;
                crate::diagnostics::warn!("Could not retain the outgoing video frame: {error}");
            }
        }
    }

    fn capture_incoming(&mut self, state: &Playback, now: Instant) {
        // A decoded frame can arrive while the D3D11 child is hidden or resizing.
        // Both the actual surface and the decoded pictures must settle before release.
        let surface_stable = self.surface_is_stable(state.generation, now);
        if let Some(frame) = self.stable_frame(state, now)
            && transition_frame::incoming_cover_release_ready(
                frame.count,
                frame.since,
                now,
                surface_stable,
            )
        {
            self.released_generation
                .store(state.generation, Ordering::Release);
            return;
        }
        let retry_wait_elapsed = self
            .incoming_attempt
            .is_none_or(|(generation, attempted_at)| {
                generation != state.generation
                    || now.duration_since(attempted_at) >= Duration::from_millis(240)
            });
        if !retry_wait_elapsed || !surface_stable {
            return;
        }
        if let Err(error) = self.sender.send(Command::CaptureIncomingTransitionFrame) {
            crate::diagnostics::warn!("Could not capture the incoming video frame: {error}");
        } else {
            self.incoming_attempt = Some((state.generation, now));
        }
    }

    fn surface_is_stable(&mut self, generation: u64, now: Instant) -> bool {
        let Some(host) = self.host.as_ref().and_then(slint::Weak::upgrade) else {
            return false;
        };
        let size = host.window().size();
        let Some(signature) = platform::native_video_surface_signature(host.window()) else {
            self.incoming_surface = None;
            return false;
        };
        if let Some(previous) = &self.incoming_surface
            && previous.generation == generation
            && previous.size == size
            && previous.signature == signature
        {
            return now.duration_since(previous.since) >= INCOMING_SURFACE_STABLE_FOR;
        }
        self.incoming_surface = Some(IncomingSurface {
            generation,
            size,
            signature,
            since: now,
        });
        false
    }

    fn stable_frame(&mut self, state: &Playback, now: Instant) -> Option<&IncomingFrame> {
        let capture = &state.incoming_transition_frame;
        if capture.generation != Some(state.generation)
            || !transition_frame::incoming_frame_is_usable(state)
        {
            self.incoming_frame = None;
            return None;
        }
        if let Some(frame) = &mut self.incoming_frame
            && frame.generation == state.generation
        {
            if frame.revision != capture.revision {
                frame.count = frame.count.saturating_add(1);
                frame.revision = capture.revision;
            }
        } else {
            self.incoming_frame = Some(IncomingFrame {
                generation: state.generation,
                revision: capture.revision,
                count: 1,
                since: now,
            });
        }
        self.incoming_frame.as_ref()
    }
}
