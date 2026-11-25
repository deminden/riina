//! Own the native Picture-in-Picture controller and its renderer lifetime.

mod avkit;

use self::avkit::{PlaybackDelegate, SampleQueue};
use super::{pip_frame::PipFrameSource, pip_window::window_can_draw};
use objc2::{
    AnyThread, DefinedClass, MainThreadMarker, msg_send, rc::Retained, runtime::ProtocolObject,
};
use objc2_av_foundation::AVSampleBufferDisplayLayer;
use objc2_av_kit::{AVPictureInPictureController, AVPictureInPictureControllerContentSource};
use objc2_core_media::{CMClock, CMTime, CMTimebase};
use objc2_foundation::NSProcessInfo;
use riina_core::{Phase, Playback, VideoParameters, pip_lifecycle};
use riina_mpv::{CommandSender, video::VideoSurface};
use slint::ComponentHandle;
use std::{
    cell::{Cell, RefCell},
    ffi::CStr,
    ptr::NonNull,
    rc::Weak,
    time::{Duration, Instant},
};

const START_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Transition {
    Idle,
    WaitingForSource(Instant),
    Starting,
    CancellingStart,
    Stopping,
}

/// Owns the AVKit source and delegate for the full player-session lifetime.
/// Frames are accepted only through `enqueue_sample`; the controller cannot
/// start until the same mpv render context has supplied a real video sample.
pub(crate) struct MacPictureInPicture {
    surface: SampleQueue,
    controller: Retained<AVPictureInPictureController>,
    delegate: Retained<PlaybackDelegate>,
    playback: crate::PlaybackStore,
    clock: objc2_core_foundation::CFRetained<CMClock>,
    timebase: objc2_core_foundation::CFRetained<CMTimebase>,
    frame_source: Option<PipFrameSource>,
    video_surface: Weak<RefCell<Option<VideoSurface>>>,
    frame_revision: Option<(u64, u64)>,
    has_sample: bool,
    minimize_requested_start: Cell<bool>,
}

impl MacPictureInPicture {
    #[expect(
        clippy::expect_used,
        reason = "The Core Media clock output is checked for null after creation"
    )]
    pub(crate) fn new(
        player: slint::Weak<crate::PlayerWindow>,
        commands: CommandSender,
        playback: crate::PlaybackStore,
        mtm: MainThreadMarker,
    ) -> Result<Self, String> {
        let major_version = NSProcessInfo::processInfo()
            .operatingSystemVersion()
            .majorVersion;
        if major_version < 12 {
            return Err("Sample-buffer Picture-in-Picture requires macOS 12 or later".into());
        }
        // SAFETY: the main-thread marker establishes the caller's AVKit thread.
        if !unsafe { AVPictureInPictureController::isPictureInPictureSupported() } {
            return Err("Picture-in-Picture is not supported on this Mac".into());
        }
        // SAFETY: construction and the retained layer access occur on the main thread.
        let layer = unsafe { AVSampleBufferDisplayLayer::new() };
        // SAFETY: the macOS 14+ renderer accessor receives a live sample-buffer layer.
        let renderer = (major_version >= 14).then(|| unsafe { layer.sampleBufferRenderer() });
        let delegate = PlaybackDelegate::new(mtm, player, commands, playback.clone());
        let protocol = ProtocolObject::from_ref(&*delegate);
        // SAFETY: the main-thread owner retains the layer and typed delegate for the
        // lifetime of the AVKit controller.
        let source = unsafe {
            AVPictureInPictureControllerContentSource::initWithSampleBufferDisplayLayer_playbackDelegate(
                AVPictureInPictureControllerContentSource::alloc(),
                &layer,
                protocol,
            )
        };
        // AVKit can reject a content source even if the device supports PiP.
        // SAFETY: this is the documented initializer, and the source is live on the main thread.
        let controller: Option<Retained<AVPictureInPictureController>> = unsafe {
            msg_send![AVPictureInPictureController::alloc(), initWithContentSource: &*source]
        };
        let controller = controller
            .ok_or_else(|| "AVKit rejected the Picture-in-Picture video source".to_owned())?;
        // SAFETY: Core Media provides the host clock for the lifetime of this timebase.
        let clock = unsafe { CMClock::host_time_clock() };
        let mut timebase = std::ptr::null_mut();
        #[allow(deprecated)]
        // SAFETY: the retained clock and writable out pointer are live for the call.
        let status = unsafe {
            CMTimebase::create_with_master_clock(None, &clock, NonNull::from(&mut timebase))
        };
        if status != 0 || timebase.is_null() {
            return Err(format!(
                "Could not create the PiP playback clock ({status})"
            ));
        }
        // SAFETY: the successful Create call transferred one retained, non-null timebase.
        let timebase = unsafe {
            objc2_core_foundation::CFRetained::from_raw(
                NonNull::new(timebase).expect("checked non-null"),
            )
        };
        // SAFETY: the timescale is positive and the retained clock is this timebase's source.
        let status = unsafe {
            let initial_time = CMTime::new(0, 1_000);
            let anchor = clock.time();
            timebase.set_rate_and_anchor_time(0.0, initial_time, anchor)
        };
        if status != 0 {
            return Err(format!(
                "Could not initialize the PiP playback clock ({status})"
            ));
        }
        // SAFETY: the main-thread owner retains all AVKit objects and the control timebase.
        unsafe {
            layer.setControlTimebase(Some(&timebase));
            controller.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
            controller.setCanStartPictureInPictureAutomaticallyFromInline(false);
        }
        Ok(Self {
            surface: SampleQueue { layer, renderer },
            controller,
            delegate,
            playback,
            clock,
            timebase,
            frame_source: None,
            video_surface: Weak::new(),
            frame_revision: None,
            has_sample: false,
            minimize_requested_start: Cell::new(false),
        })
    }

    /// # Safety
    /// Must be called during Slint rendering setup while its GL context is current.
    pub(crate) unsafe fn configure_rendering(
        &mut self,
        loader: &dyn Fn(&CStr) -> *const std::ffi::c_void,
        video_surface: Weak<RefCell<Option<VideoSurface>>>,
    ) -> Result<(), String> {
        // SAFETY: the caller keeps Slint's CGL context current throughout setup.
        self.frame_source = Some(unsafe { PipFrameSource::new(loader)? });
        self.video_surface = video_surface;
        Ok(())
    }

    pub(crate) fn toggle(&mut self) -> Result<(), String> {
        let transition = &self.delegate.ivars().transition;
        match transition.get() {
            Transition::Stopping | Transition::CancellingStart => return Ok(()),
            Transition::Starting => {
                transition.set(Transition::CancellingStart);
                return Ok(());
            }
            Transition::WaitingForSource(_) => {
                transition.set(Transition::Idle);
                return Ok(());
            }
            Transition::Idle => {}
        }
        self.minimize_requested_start.set(false);
        if self.is_active() {
            self.request_stop();
            return Ok(());
        }
        self.request_start()
    }

    fn request_start(&mut self) -> Result<(), String> {
        if self.frame_source.is_none()
            || video_dimensions(&riina_core::sync::lock(&self.playback)).is_none()
        {
            return Err("Picture-in-Picture requires a loaded video".into());
        }
        self.clear_samples();
        if let Some(source) = self.frame_source.as_mut() {
            source.reset_after_seek();
        }
        self.delegate
            .ivars()
            .transition
            .set(Transition::WaitingForSource(Instant::now() + START_TIMEOUT));
        self.advance_start();
        Ok(())
    }

    fn is_active(&self) -> bool {
        // SAFETY: the retained controller belongs to this UI thread.
        unsafe { self.controller.isPictureInPictureActive() }
    }

    pub(crate) fn needs_presentation_poll(&self) -> bool {
        let ivars = self.delegate.ivars();
        !ivars.window.is_closing()
            && (ivars.transition.get() != Transition::Idle || self.has_sample || self.is_active())
    }

    fn request_stop(&self) {
        self.delegate.ivars().transition.set(Transition::Stopping);
        // SAFETY: the retained controller is queried and stopped on its owning thread.
        unsafe { self.controller.stopPictureInPicture() };
    }

    fn advance_start(&self) {
        let ivars = self.delegate.ivars();
        if ivars.window.is_closing() {
            return;
        }
        let transition = &ivars.transition;
        let Transition::WaitingForSource(deadline) = transition.get() else {
            return;
        };
        if Instant::now() >= deadline {
            transition.set(Transition::Idle);
            if let Some(player) = self.delegate.ivars().player.upgrade() {
                crate::diagnostics::report(
                    &player,
                    "macOS did not make the Picture-in-Picture video source available".to_owned(),
                );
            }
        } else if self.has_sample {
            // SAFETY: the controller owns a sample source and remains on its owning thread.
            unsafe {
                if self.controller.isPictureInPicturePossible() {
                    transition.set(Transition::Starting);
                    self.controller.startPictureInPicture();
                }
            }
        }
    }

    pub(crate) fn minimized(&mut self, player: &crate::PlayerWindow, minimized: bool) {
        let ivars = self.delegate.ivars();
        if ivars.window.owns_minimization() || ivars.window.is_closing() {
            return;
        }
        let Ok(settings) = crate::pip_preferences::current_player(player) else {
            return;
        };
        if settings.toggle_by_minimizing
            && !minimized
            && self.minimize_requested_start.replace(false)
        {
            match ivars.transition.get() {
                Transition::WaitingForSource(_) => {
                    ivars.transition.set(Transition::Idle);
                    return;
                }
                Transition::Starting => {
                    ivars.transition.set(Transition::CancellingStart);
                    return;
                }
                _ => {}
            }
        }
        let phase = match ivars.transition.get() {
            Transition::WaitingForSource(_) | Transition::Starting => {
                pip_lifecycle::Phase::Entering
            }
            Transition::Stopping | Transition::CancellingStart => pip_lifecycle::Phase::Exiting,
            Transition::Idle if self.is_active() => pip_lifecycle::Phase::Active,
            Transition::Idle => pip_lifecycle::Phase::Off,
        };
        let is_video = matches!(
            riina_core::sync::lock(&self.playback).audio_status(),
            riina_core::music_mode::AudioStatus::Video
        );
        match pip_lifecycle::minimize_toggle_target(settings, is_video, minimized, phase) {
            Some(true) => {
                if self.request_start().is_ok() {
                    self.minimize_requested_start.set(true);
                }
            }
            Some(false) => self.request_stop(),
            None => {}
        }
    }

    pub(crate) fn close(&self) {
        let ivars = self.delegate.ivars();
        if ivars.window.is_closing() {
            return;
        }
        ivars.window.close();
        if self.is_active() && ivars.transition.get() != Transition::Stopping {
            self.request_stop();
        }
    }

    pub(crate) fn synchronize_playback(&mut self) {
        let playback = riina_core::sync::lock(&self.playback);
        let position = playback
            .position
            .filter(|value| value.is_finite() && *value >= 0.0)
            .unwrap_or(0.0);
        let rate = if playback.paused || playback.buffering || playback.seeking {
            0.0
        } else if playback.speed.is_finite() && playback.speed > 0.0 {
            playback.speed
        } else {
            1.0
        };
        let revision = (playback.generation, playback.completed_seeks);
        let closed_media = matches!(playback.phase, Phase::Stopped | Phase::Failed);
        drop(playback);
        self.refresh_frame_revision(revision);
        if closed_media {
            let transition = &self.delegate.ivars().transition;
            match transition.get() {
                Transition::WaitingForSource(_) => transition.set(Transition::Idle),
                Transition::Starting => transition.set(Transition::CancellingStart),
                Transition::Idle if self.is_active() => self.request_stop(),
                _ => {}
            }
        }
        // SAFETY: the position and rate are finite, the timescale is positive,
        // and the clock and timebase are retained by this session.
        let _ = unsafe {
            let media_time = CMTime::with_seconds(position, 1_000);
            let host_time = self.clock.time();
            self.timebase
                .set_rate_and_anchor_time(rate, media_time, host_time)
        };
        // SAFETY: the main-thread controller and its delegate are both retained.
        unsafe { self.controller.invalidatePlaybackState() };
    }

    fn refresh_frame_revision(&mut self, revision: (u64, u64)) {
        if self.frame_revision != Some(revision) {
            self.frame_revision = Some(revision);
            self.clear_samples();
            if let Some(source) = self.frame_source.as_mut() {
                source.reset_after_seek();
            }
        }
    }

    fn clear_samples(&mut self) {
        self.has_sample = false;
        self.surface.flush();
    }

    /// # Safety
    /// Must be called while the CGL context used by the renderer is current.
    pub(crate) unsafe fn teardown_rendering(&mut self) {
        self.clear_samples();
        self.frame_revision = None;
        self.frame_source.take();
        self.video_surface = Weak::new();
    }

    pub(crate) fn poll_source(&mut self, player: &crate::PlayerWindow) -> Result<bool, String> {
        let active = self.is_active();
        let transition = self.delegate.ivars().transition.get();
        let waiting = matches!(
            transition,
            Transition::WaitingForSource(_) | Transition::Starting
        );
        if active && self.frame_source.is_none() && transition == Transition::Idle {
            self.request_stop();
            return Ok(false);
        }
        let offscreen = (active || waiting) && !window_can_draw(player);
        if offscreen {
            if let Err(error) = self.render_offscreen() {
                self.frame_source.take();
                self.clear_samples();
                if active && transition != Transition::Stopping {
                    self.request_stop();
                }
                return Err(error);
            }
        } else if waiting && !self.has_sample {
            player.window().request_redraw();
        }
        if transition == Transition::Idle && !active && self.has_sample {
            self.clear_samples();
        }
        // AVKit availability can change without triggering a new video frame.
        self.advance_start();
        Ok(offscreen && active)
    }

    fn render_offscreen(&mut self) -> Result<(), String> {
        let (Some(surface), Some(source)) =
            (self.video_surface.upgrade(), self.frame_source.as_ref())
        else {
            return Ok(());
        };
        let Ok(mut surface) = surface.try_borrow_mut() else {
            return Ok(());
        };
        let Some(video) = surface.as_mut() else {
            return Ok(());
        };
        // SAFETY: the rendering notifier and timer are serialized on the main
        // thread. The borrowed video still belongs to the source's CGL context.
        let _current = unsafe { source.bind_context()? };
        // SAFETY: the guard keeps the creating CGL context current.
        if unsafe { self.render_frame(video)? } {
            // SAFETY: the IOSurface sample is this frame's sole presentation.
            unsafe { video.report_swap() };
        }
        Ok(())
    }

    /// # Safety
    /// Must be called with the retained player's CGL context current.
    pub(crate) unsafe fn render_frame(&mut self, video: &mut VideoSurface) -> Result<bool, String> {
        let active = self.is_active();
        let ivars = self.delegate.ivars();
        if ivars.window.is_closing()
            || (!active
                && !matches!(
                    ivars.transition.get(),
                    Transition::WaitingForSource(_) | Transition::Starting
                ))
        {
            return Ok(false);
        }
        let playback = riina_core::sync::lock(&self.playback);
        let size = video_dimensions(&playback);
        let revision = (playback.generation, playback.completed_seeks);
        let can_render = !playback.seeking && !playback.buffering;
        let is_paused = playback.paused;
        drop(playback);
        if self.frame_revision != Some(revision) {
            self.synchronize_playback();
        }
        let mut enqueued = false;
        if let Some(size) = size
            && can_render
            && (!is_paused || !self.has_sample)
            && self.surface.ready()
            && let Some(source) = self.frame_source.as_mut()
        {
            // SAFETY: the notifier holds the current CGL context, the render
            // source and the control timebase for the synchronous frame copy.
            let sample = match unsafe { source.sample(video, size, self.timebase.time()) } {
                Ok(sample) => sample,
                Err(error) => {
                    self.frame_source = None;
                    return Err(error);
                }
            };
            if let Some(sample) = sample {
                self.surface.enqueue(&sample);
                self.has_sample = true;
                enqueued = true;
            }
        }
        self.advance_start();
        Ok(enqueued)
    }
}

fn video_dimensions(playback: &Playback) -> Option<(u32, u32)> {
    if matches!(
        playback.phase,
        Phase::Stopped | Phase::Loading | Phase::Failed
    ) {
        return None;
    }
    let parameters: &VideoParameters =
        playback.video_target.as_ref().or(playback.video.as_ref())?;
    let (mut width, mut height) = (
        parameters.dw.unwrap_or(parameters.w),
        parameters.dh.unwrap_or(parameters.h),
    );
    if matches!(parameters.rotate.unwrap_or(0).rem_euclid(360), 90 | 270) {
        std::mem::swap(&mut width, &mut height);
    }
    (width > 0 && height > 0).then_some((width, height))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pip_source_rejects_stale_dimensions_until_a_replacement_is_ready() {
        let mut playback = Playback {
            phase: Phase::Loading,
            video: Some(serde_json::from_str(r#"{"w":1920,"h":1080,"rotate":90}"#).unwrap()),
            ..Playback::default()
        };
        assert_eq!(video_dimensions(&playback), None);

        playback.phase = Phase::Paused;
        assert_eq!(video_dimensions(&playback), Some((1080, 1920)));

        playback.phase = Phase::Failed;
        assert_eq!(video_dimensions(&playback), None);
    }
}
