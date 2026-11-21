//! Public AVKit sample-buffer PiP ownership for the macOS player.

use objc2::{
    AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send,
    rc::Retained, runtime::ProtocolObject,
};
use objc2_av_foundation::{
    AVQueuedSampleBufferRendering, AVSampleBufferDisplayLayer, AVSampleBufferVideoRenderer,
};
use objc2_av_kit::{
    AVPictureInPictureController, AVPictureInPictureControllerContentSource,
    AVPictureInPictureSampleBufferPlaybackDelegate,
};
use objc2_core_media::{
    CMClock, CMTime, CMTimeRange, CMTimebase, CMVideoDimensions, kCMTimePositiveInfinity,
};
use objc2_foundation::{NSObject, NSObjectProtocol};
use riina_core::{Command, Playback, VideoParameters};
use riina_mpv::CommandSender;
use std::{ffi::CStr, ptr::NonNull};

use super::pip_frame::PipFrameSource;

struct PlaybackDelegateIvars {
    commands: CommandSender,
    playback: crate::PlaybackStore,
}

define_class!(
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = PlaybackDelegateIvars]
    struct PlaybackDelegate;

    // SAFETY: the class inherits NSObject and uses its protocol implementation.
    unsafe impl NSObjectProtocol for PlaybackDelegate {}

    #[allow(non_snake_case)]
    // SAFETY: the selectors below implement the AVKit protocol with its declared ABI.
    unsafe impl AVPictureInPictureSampleBufferPlaybackDelegate for PlaybackDelegate {
        #[unsafe(method(pictureInPictureController:setPlaying:))]
        unsafe fn pictureInPictureController_setPlaying(
            &self,
            _controller: &AVPictureInPictureController,
            playing: bool,
        ) {
            let _ = self.ivars().commands.send(Command::SetPaused(!playing));
        }

        #[unsafe(method(pictureInPictureControllerTimeRangeForPlayback:))]
        unsafe fn pictureInPictureControllerTimeRangeForPlayback(
            &self,
            _controller: &AVPictureInPictureController,
        ) -> CMTimeRange {
            let playback = riina_core::sync::lock(&self.ivars().playback);
            let duration = playback
                .duration
                .filter(|value| value.is_finite() && *value >= 0.0);
            // The sample renderer's control timebase starts at the first media
            // timestamp; unknown duration denotes live media to AVKit.
            let duration = duration.map_or_else(
                || {
                    // SAFETY: this immutable time constant is exported by Core Media.
                    unsafe { kCMTimePositiveInfinity }
                },
                |duration| {
                    // SAFETY: duration was checked to be finite and the timescale is positive.
                    unsafe { CMTime::with_seconds(duration, 1_000) }
                },
            );
            // SAFETY: both times are initialized Core Media values passed by value.
            unsafe { CMTimeRange::new(CMTime::new(0, 1), duration) }
        }

        #[unsafe(method(pictureInPictureControllerIsPlaybackPaused:))]
        unsafe fn pictureInPictureControllerIsPlaybackPaused(
            &self,
            _controller: &AVPictureInPictureController,
        ) -> bool {
            self.ivars()
                .playback
                .lock()
                .map_or(true, |playback| playback.paused)
        }

        #[unsafe(method(pictureInPictureController:didTransitionToRenderSize:))]
        unsafe fn pictureInPictureController_didTransitionToRenderSize(
            &self,
            _controller: &AVPictureInPictureController,
            _new_render_size: CMVideoDimensions,
        ) {
            // The source dimensions follow the decoded video; PiP size is fitted
            // by AVKit and does not reconfigure the single libmpv render context.
        }

        #[unsafe(method(pictureInPictureController:skipByInterval:completionHandler:))]
        unsafe fn pictureInPictureController_skipByInterval_completionHandler(
            &self,
            _controller: &AVPictureInPictureController,
            interval: CMTime,
            completion_handler: &block2::DynBlock<dyn Fn()>,
        ) {
            // SAFETY: AVKit supplies a CMTime by value for this callback.
            let seconds = unsafe { interval.seconds() };
            if seconds.is_finite() && seconds != 0.0 {
                let _ = self.ivars().commands.send(Command::Seek {
                    seconds,
                    absolute: false,
                });
            }
            completion_handler.call(());
        }
    }
);

impl PlaybackDelegate {
    fn new(
        mtm: MainThreadMarker,
        commands: CommandSender,
        playback: crate::PlaybackStore,
    ) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(PlaybackDelegateIvars { commands, playback });
        // SAFETY: NSObject initializes the allocated, ivar-initialized object on the main thread.
        unsafe { msg_send![super(this), init] }
    }
}

/// Owns the AVKit source and delegate for the full player-session lifetime.
/// Frames are accepted only through `enqueue_sample`; the controller cannot
/// start until the same mpv render context has supplied a real video sample.
pub(crate) struct MacPictureInPicture {
    _layer: Retained<AVSampleBufferDisplayLayer>,
    renderer: Retained<AVSampleBufferVideoRenderer>,
    controller: Retained<AVPictureInPictureController>,
    _delegate: Retained<PlaybackDelegate>,
    playback: crate::PlaybackStore,
    clock: objc2_core_foundation::CFRetained<CMClock>,
    timebase: objc2_core_foundation::CFRetained<CMTimebase>,
    frame_source: Option<PipFrameSource>,
    last_seek_generation: u64,
    pending_start: bool,
    has_sample: bool,
}

impl MacPictureInPicture {
    #[expect(
        clippy::expect_used,
        reason = "The Core Media clock output is checked for null after creation"
    )]
    pub(crate) fn new(
        commands: CommandSender,
        playback: crate::PlaybackStore,
        mtm: MainThreadMarker,
    ) -> Result<Self, String> {
        // SAFETY: the main-thread marker establishes the caller's AVKit thread.
        if !unsafe { AVPictureInPictureController::isPictureInPictureSupported() } {
            return Err("Picture-in-Picture is not supported on this Mac".into());
        }
        // SAFETY: construction and the retained layer access occur on the main thread.
        let layer = unsafe { AVSampleBufferDisplayLayer::new() };
        // SAFETY: the macOS 14+ renderer accessor receives a live sample-buffer layer.
        let renderer = unsafe { layer.sampleBufferRenderer() };
        let delegate = PlaybackDelegate::new(mtm, commands, playback.clone());
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
        // SAFETY: the source is retained for controller initialization on the main thread.
        let controller = unsafe {
            AVPictureInPictureController::initWithContentSource(
                AVPictureInPictureController::alloc(),
                &source,
            )
        };
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
            controller.setCanStartPictureInPictureAutomaticallyFromInline(false);
        }
        Ok(Self {
            _layer: layer,
            renderer,
            controller,
            _delegate: delegate,
            playback,
            clock,
            timebase,
            frame_source: None,
            last_seek_generation: 0,
            pending_start: false,
            has_sample: false,
        })
    }

    /// # Safety
    /// Must be called during Slint rendering setup while its GL context is current.
    pub(crate) unsafe fn configure_rendering(
        &mut self,
        loader: &dyn Fn(&CStr) -> *const std::ffi::c_void,
    ) -> Result<(), String> {
        // SAFETY: the caller keeps Slint's CGL context current throughout setup.
        self.frame_source = Some(unsafe { PipFrameSource::new(loader)? });
        Ok(())
    }

    pub(crate) fn toggle(&mut self) -> Result<(), String> {
        // SAFETY: the session retains the main-thread AVKit controller.
        if unsafe { self.controller.isPictureInPictureActive() } {
            self.pending_start = false;
            // SAFETY: the live controller is stopped on its owning thread.
            unsafe { self.controller.stopPictureInPicture() };
            return Ok(());
        }
        if !self.has_sample {
            self.pending_start = true;
            return Ok(());
        }
        // SAFETY: the retained controller belongs to this UI session.
        if !unsafe { self.controller.isPictureInPicturePossible() } {
            self.pending_start = true;
            return Ok(());
        }
        // SAFETY: this live main-thread controller has a sample and reports availability.
        unsafe { self.controller.startPictureInPicture() };
        Ok(())
    }

    pub(crate) fn is_active(&self) -> bool {
        // SAFETY: the session keeps the controller alive on its owning thread.
        unsafe { self.controller.isPictureInPictureActive() }
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
        let completed_seeks = playback.completed_seeks;
        drop(playback);
        if completed_seeks != self.last_seek_generation {
            self.last_seek_generation = completed_seeks;
            self.has_sample = false;
            if let Some(source) = self.frame_source.as_mut() {
                source.reset_after_seek();
            }
            // SAFETY: the retained renderer may flush without a completion block.
            unsafe {
                self.renderer
                    .flushWithRemovalOfDisplayedImage_completionHandler(true, None);
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

    /// # Safety
    /// Must be called while the CGL context used by the renderer is current.
    pub(crate) unsafe fn teardown_rendering(&mut self) {
        self.has_sample = false;
        // SAFETY: the renderer remains live through teardown and accepts a null completion block.
        unsafe {
            self.renderer
                .flushWithRemovalOfDisplayedImage_completionHandler(true, None);
        }
        self.frame_source.take();
    }

    /// # Safety
    /// Must be called from the renderer notifier with its CGL context current.
    pub(crate) unsafe fn render_frame(
        &mut self,
        video: &mut riina_mpv::video::VideoSurface,
    ) -> Result<(), String> {
        let playback = riina_core::sync::lock(&self.playback);
        let size = video_dimensions(&playback);
        // SAFETY: Slint's render notifier runs on this session's UI thread.
        let active = unsafe { self.controller.isPictureInPictureActive() };
        let is_paused = playback.paused || playback.buffering || playback.seeking;
        drop(playback);
        if let Some(size) = size
            && (!is_paused || !self.has_sample)
            && let Some(source) = self.frame_source.as_mut()
        {
            // SAFETY: the notifier holds the current CGL context, the render
            // source and the control timebase for the synchronous frame copy.
            let sample = match unsafe { source.sample(video, size, self.timebase.time(), active) } {
                Ok(sample) => sample,
                Err(error) => {
                    self.frame_source = None;
                    return Err(error);
                }
            };
            if let Some(sample) = sample {
                self.enqueue_sample(&sample);
            }
        }
        if self.pending_start && self.has_sample {
            // SAFETY: the main-thread render notifier retains this controller
            // and its ready media source throughout the availability check.
            unsafe {
                if self.controller.isPictureInPicturePossible() {
                    self.pending_start = false;
                    self.controller.startPictureInPicture();
                }
            }
        }
        Ok(())
    }

    fn enqueue_sample(&mut self, sample: &objc2_core_media::CMSampleBuffer) {
        // SAFETY: the retained renderer is still attached to this session's layer.
        if !unsafe { self.renderer.isReadyForMoreMediaData() } {
            return;
        }
        // SAFETY: sample is an initialized buffer borrowed for the enqueue call.
        unsafe { self.renderer.enqueueSampleBuffer(sample) };
        self.has_sample = true;
    }
}

fn video_dimensions(playback: &Playback) -> Option<(u32, u32)> {
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
