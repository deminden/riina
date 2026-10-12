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
    kCMTimeRangeInvalid,
};
use objc2_foundation::{NSObject, NSObjectProtocol};
use riina_core::{Command, Playback, VideoParameters};
use riina_mpv::CommandSender;
use std::{
    ffi::CStr,
    ptr::NonNull,
    sync::{Arc, Mutex},
};

use super::pip_frame::PipFrameSource;

struct PlaybackDelegateIvars {
    commands: CommandSender,
    playback: Arc<Mutex<Playback>>,
}

define_class!(
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = PlaybackDelegateIvars]
    struct PlaybackDelegate;

    unsafe impl NSObjectProtocol for PlaybackDelegate {}

    #[allow(non_snake_case)]
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
            let Ok(playback) = self.ivars().playback.lock() else {
                return unsafe { kCMTimeRangeInvalid };
            };
            let duration = playback
                .duration
                .filter(|value| value.is_finite() && *value >= 0.0);
            // The sample renderer's control timebase starts at the first media
            // timestamp; unknown duration denotes live media to AVKit.
            let duration = duration.map_or_else(
                || unsafe { kCMTimePositiveInfinity },
                |duration| unsafe { CMTime::with_seconds(duration, 1_000) },
            );
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
        playback: Arc<Mutex<Playback>>,
    ) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(PlaybackDelegateIvars { commands, playback });
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
    playback: Arc<Mutex<Playback>>,
    clock: objc2_core_foundation::CFRetained<CMClock>,
    timebase: objc2_core_foundation::CFRetained<CMTimebase>,
    frame_source: Option<PipFrameSource>,
    last_seek_generation: u64,
    pending_start: bool,
    has_sample: bool,
}

impl MacPictureInPicture {
    pub(crate) fn new(
        commands: CommandSender,
        playback: Arc<Mutex<Playback>>,
        mtm: MainThreadMarker,
    ) -> Result<Self, String> {
        if !unsafe { AVPictureInPictureController::isPictureInPictureSupported() } {
            return Err("Picture-in-Picture is not supported on this Mac".into());
        }
        let layer = unsafe { AVSampleBufferDisplayLayer::new() };
        let renderer = unsafe { layer.sampleBufferRenderer() };
        let delegate = PlaybackDelegate::new(mtm, commands, playback.clone());
        let protocol = ProtocolObject::from_ref(&*delegate);
        let source = unsafe {
            AVPictureInPictureControllerContentSource::initWithSampleBufferDisplayLayer_playbackDelegate(
                AVPictureInPictureControllerContentSource::alloc(),
                &layer,
                protocol,
            )
        };
        let controller = unsafe {
            AVPictureInPictureController::initWithContentSource(
                AVPictureInPictureController::alloc(),
                &source,
            )
        };
        let clock = unsafe { CMClock::host_time_clock() };
        let mut timebase = std::ptr::null_mut();
        #[allow(deprecated)]
        let status = unsafe {
            CMTimebase::create_with_master_clock(None, &clock, NonNull::from(&mut timebase))
        };
        if status != 0 || timebase.is_null() {
            return Err(format!(
                "Could not create the PiP playback clock ({status})"
            ));
        }
        let timebase = unsafe {
            objc2_core_foundation::CFRetained::from_raw(
                NonNull::new(timebase).expect("checked non-null"),
            )
        };
        let initial_time = unsafe { CMTime::new(0, 1_000) };
        let anchor = unsafe { clock.time() };
        let status = unsafe { timebase.set_rate_and_anchor_time(0.0, initial_time, anchor) };
        if status != 0 {
            return Err(format!(
                "Could not initialize the PiP playback clock ({status})"
            ));
        }
        unsafe { layer.setControlTimebase(Some(&timebase)) };
        unsafe {
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
        self.frame_source = Some(unsafe { PipFrameSource::new(loader)? });
        Ok(())
    }

    pub(crate) fn toggle(&mut self) -> Result<(), String> {
        if unsafe { self.controller.isPictureInPictureActive() } {
            self.pending_start = false;
            unsafe { self.controller.stopPictureInPicture() };
            return Ok(());
        }
        if !self.has_sample {
            self.pending_start = true;
            return Ok(());
        }
        if !unsafe { self.controller.isPictureInPicturePossible() } {
            self.pending_start = true;
            return Ok(());
        }
        unsafe { self.controller.startPictureInPicture() };
        Ok(())
    }

    pub(crate) fn is_active(&self) -> bool {
        unsafe { self.controller.isPictureInPictureActive() }
    }

    pub(crate) fn synchronize_playback(&mut self) {
        let Ok(playback) = self.playback.lock() else {
            return;
        };
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
        if playback.completed_seeks != self.last_seek_generation {
            self.last_seek_generation = playback.completed_seeks;
            self.has_sample = false;
            if let Some(source) = self.frame_source.as_mut() {
                source.reset_after_seek();
            }
            unsafe {
                self.renderer
                    .flushWithRemovalOfDisplayedImage_completionHandler(true, None);
            }
        }
        let media_time = unsafe { CMTime::with_seconds(position, 1_000) };
        let host_time = unsafe { self.clock.time() };
        let _ = unsafe {
            self.timebase
                .set_rate_and_anchor_time(rate, media_time, host_time)
        };
        unsafe { self.controller.invalidatePlaybackState() };
    }

    /// # Safety
    /// Must be called while the CGL context used by the renderer is current.
    pub(crate) unsafe fn teardown_rendering(&mut self) {
        self.has_sample = false;
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
        let Ok(playback) = self.playback.lock() else {
            return Ok(());
        };
        let size = video_dimensions(&playback);
        let active = unsafe { self.controller.isPictureInPictureActive() };
        let is_paused = playback.paused || playback.buffering || playback.seeking;
        drop(playback);
        if let Some(size) = size
            && (!is_paused || !self.has_sample)
            && let Some(source) = self.frame_source.as_mut()
        {
            // SAFETY: this method is called only from the Slint render notifier.
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
        if self.pending_start
            && self.has_sample
            && unsafe { self.controller.isPictureInPicturePossible() }
        {
            self.pending_start = false;
            unsafe { self.controller.startPictureInPicture() };
        }
        Ok(())
    }

    fn enqueue_sample(&mut self, sample: &objc2_core_media::CMSampleBuffer) {
        if !unsafe { self.renderer.isReadyForMoreMediaData() } {
            return;
        }
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
