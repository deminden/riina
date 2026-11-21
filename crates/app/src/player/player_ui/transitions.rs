//! Collect outgoing and incoming transition frames around playlist navigation.
use super::PendingNavigation;
use crate::platform;
use riina_core::{Command, Phase, sync::lock};
use riina_mpv::CommandSender;
use slint::ComponentHandle;
use std::{
    cell::Cell,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

const INCOMING_SURFACE_STABLE_FOR: Duration = Duration::from_millis(300);
const MANUAL_TRANSITION_CAPTURE_TIMEOUT: Duration = Duration::from_millis(500);

#[expect(
    clippy::too_many_lines,
    reason = "The transition timer still contains the original capture state machine"
)]
pub(super) fn install(
    capture_transition_first: bool,
    native: Option<&platform::NativePresentation>,
    command_sender: &CommandSender,
    latest_playback: &crate::PlaybackStore,
    pending_navigation: &PendingNavigation,
    transition_release_generation: &Arc<AtomicU64>,
) -> Option<Rc<slint::Timer>> {
    if capture_transition_first {
        let timer = Rc::new(slint::Timer::default());
        let playback = latest_playback.clone();
        let sender = command_sender.clone();
        let captured_generation = Rc::new(Cell::new(None::<u64>));
        let outgoing_capture_attempt = Rc::new(Cell::new(None::<(u64, Instant)>));
        let incoming_capture_attempt = Rc::new(Cell::new(None::<(u64, Instant)>));
        let incoming_capture_stability = Rc::new(Cell::new(None::<(u64, u64, u8, Instant)>));
        let surface_host = native.as_ref().map(|presentation| presentation.host());
        let surface_geometry = Rc::new(Cell::new(None::<(u64, slint::PhysicalSize, u64, Instant)>));
        let timer_generation = captured_generation.clone();
        let timer_outgoing_attempt = outgoing_capture_attempt.clone();
        let timer_incoming_attempt = incoming_capture_attempt.clone();
        let timer_incoming_stability = incoming_capture_stability.clone();
        let timer_transition_release_generation = transition_release_generation.clone();
        let timer_pending_navigation = pending_navigation.clone();
        let timer_surface_host = surface_host.clone();
        let timer_surface_geometry = surface_geometry.clone();
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(80),
            move || {
                let state = lock(&playback).clone();
                let navigation = {
                    let mut pending = timer_pending_navigation.borrow_mut();
                    pending
                        .as_ref()
                        .is_some_and(|(_, requested_at)| {
                            state.transition_frame.generation == Some(state.generation)
                                && state
                                    .transition_frame
                                    .captured_at
                                    .is_some_and(|captured_at| captured_at >= *requested_at)
                                || requested_at.elapsed() >= MANUAL_TRANSITION_CAPTURE_TIMEOUT
                        })
                        .then(|| pending.take().map(|(commands, _)| commands))
                        .flatten()
                };
                if let Some(commands) = navigation {
                    for command in commands {
                        if let Err(error) = sender.send(command) {
                            crate::diagnostics::warn!("Could not navigate the playlist: {error}");
                        }
                    }
                }
                if state.transition_frame.generation == Some(state.generation) {
                    // Only a successful asynchronous reply confirms that mpv
                    // retained a frame. Queue acceptance alone is not enough:
                    // the D3D11 output can reject a capture while reconfiguring.
                    timer_generation.set(Some(state.generation));
                    timer_outgoing_attempt.set(None);
                }
                let now = Instant::now();
                if crate::transition_frame::outgoing_capture_due(
                    &state,
                    timer_generation.get(),
                    timer_outgoing_attempt.get(),
                    now,
                ) {
                    // Capture before mpv tears down and reconfigures its
                    // D3D11 output. Retry rejected or failed captures while
                    // there is still time before the playlist advances.
                    match sender.send(Command::CaptureTransitionFrame) {
                        Ok(()) => timer_outgoing_attempt.set(Some((state.generation, now))),
                        Err(error) => {
                            timer_outgoing_attempt.set(None);
                            crate::diagnostics::warn!(
                                "Could not retain the outgoing video frame: {error}"
                            );
                        }
                    }
                }
                if state
                    .transition_frame
                    .generation
                    .is_some_and(|generation| generation < state.generation)
                    && crate::transition_frame::incoming_has_video(&state)
                    && !matches!(state.phase, Phase::Loading | Phase::Stopped | Phase::Failed)
                    && crate::transition_frame::incoming_capture_window_active(
                        &state,
                        Instant::now(),
                    )
                    && crate::transition_frame::incoming_capture_ready(&state)
                {
                    let now = Instant::now();
                    // Decoder captures can remain healthy while mpv's native
                    // child HWND is hidden or changing size. Check the actual
                    // presentation surface before considering cover release;
                    // doing this after the capture-stability branch let a
                    // stale but valid decoder frame uncover a blank swap chain.
                    let surface_stable = timer_surface_host
                        .as_ref()
                        .and_then(slint::Weak::upgrade)
                        .is_some_and(|host| {
                            let size = host.window().size();
                            let Some(surface_signature) =
                                crate::platform::native_video_surface_signature(host.window())
                            else {
                                timer_surface_geometry.set(None);
                                return false;
                            };
                            match timer_surface_geometry.get() {
                                Some((generation, previous_size, previous_surface, since))
                                    if generation == state.generation
                                        && previous_size == size
                                        && previous_surface == surface_signature =>
                                {
                                    now.duration_since(since) >= INCOMING_SURFACE_STABLE_FOR
                                }
                                _ => {
                                    timer_surface_geometry.set(Some((
                                        state.generation,
                                        size,
                                        surface_signature,
                                        now,
                                    )));
                                    false
                                }
                            }
                        });
                    let capture_is_usable = state.incoming_transition_frame.generation
                        == Some(state.generation)
                        && crate::transition_frame::incoming_frame_is_usable(&state);
                    if capture_is_usable {
                        let revision = state.incoming_transition_frame.revision;
                        let stable_capture = match timer_incoming_stability.get() {
                            Some((generation, last_revision, count, since))
                                if generation == state.generation && last_revision == revision =>
                            {
                                (generation, last_revision, count, since)
                            }
                            Some((generation, _, count, since))
                                if generation == state.generation =>
                            {
                                (generation, revision, count.saturating_add(1), since)
                            }
                            _ => (state.generation, revision, 1, now),
                        };
                        timer_incoming_stability.set(Some(stable_capture));
                        let (_, _, count, since) = stable_capture;
                        if crate::transition_frame::incoming_cover_release_ready(
                            count,
                            since,
                            now,
                            surface_stable,
                        ) {
                            timer_transition_release_generation
                                .store(state.generation, Ordering::Release);
                            return;
                        }
                    } else {
                        timer_incoming_stability.set(None);
                    }
                    let retry_wait_elapsed =
                        timer_incoming_attempt
                            .get()
                            .is_none_or(|(generation, attempted_at)| {
                                generation != state.generation
                                    || now.duration_since(attempted_at)
                                        >= Duration::from_millis(240)
                            });
                    if !retry_wait_elapsed {
                        return;
                    }
                    if !surface_stable {
                        return;
                    }
                    // Do not uncover the native child just because time-pos or
                    // track metadata appeared. Wait for host geometry to settle,
                    // then require a successful raw capture before uncovering.
                    if let Err(error) = sender.send(Command::CaptureIncomingTransitionFrame) {
                        crate::diagnostics::warn!(
                            "Could not capture the incoming video frame: {error}"
                        );
                    } else {
                        timer_incoming_attempt.set(Some((state.generation, now)));
                    }
                }
            },
        );
        Some(timer)
    } else {
        None
    }
}
