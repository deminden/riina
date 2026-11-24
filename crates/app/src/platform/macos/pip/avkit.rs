//! Bind the AVKit delegate and preserve the renderer API for each macOS version.
use super::Transition;
use crate::platform::macos::pip_window::WindowPolicy;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained};
use objc2_av_foundation::{
    AVQueuedSampleBufferRendering, AVSampleBufferDisplayLayer, AVSampleBufferVideoRenderer,
};
use objc2_av_kit::{
    AVPictureInPictureController, AVPictureInPictureControllerDelegate,
    AVPictureInPictureSampleBufferPlaybackDelegate,
};
use objc2_core_media::{
    CMSampleBuffer, CMTime, CMTimeRange, CMVideoDimensions, kCMTimePositiveInfinity,
};
use objc2_foundation::{NSError, NSObject, NSObjectProtocol};
use riina_core::Command;
use riina_mpv::CommandSender;
use std::{cell::Cell, rc::Rc, time::Duration};

pub(super) struct PlaybackDelegateIvars {
    pub(super) player: slint::Weak<crate::PlayerWindow>,
    commands: CommandSender,
    playback: crate::PlaybackStore,
    pub(super) transition: Cell<Transition>,
    pub(super) window: Rc<WindowPolicy>,
}

define_class!(
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = PlaybackDelegateIvars]
    pub(super) struct PlaybackDelegate;

    // SAFETY: the class inherits NSObject and uses its protocol implementation.
    unsafe impl NSObjectProtocol for PlaybackDelegate {}

    #[allow(non_snake_case)]
    // SAFETY: these selectors match AVKit's main-thread lifecycle protocol.
    unsafe impl AVPictureInPictureControllerDelegate for PlaybackDelegate {
        #[unsafe(method(pictureInPictureControllerDidStartPictureInPicture:))]
        unsafe fn pictureInPictureControllerDidStartPictureInPicture(
            &self,
            controller: &AVPictureInPictureController,
        ) {
            let ivars = self.ivars();
            if ivars.window.is_closing() || ivars.transition.get() == Transition::CancellingStart {
                ivars.transition.set(Transition::Stopping);
                // SAFETY: AVKit has just confirmed this controller's live session.
                unsafe { controller.stopPictureInPicture() };
                return;
            }
            self.confirm(true);
        }

        #[unsafe(method(pictureInPictureControllerDidStopPictureInPicture:))]
        unsafe fn pictureInPictureControllerDidStopPictureInPicture(
            &self,
            _controller: &AVPictureInPictureController,
        ) {
            self.confirm(false);
        }

        #[unsafe(method(pictureInPictureController:failedToStartPictureInPictureWithError:))]
        unsafe fn pictureInPictureController_failedToStartPictureInPictureWithError(
            &self,
            _controller: &AVPictureInPictureController,
            error: &NSError,
        ) {
            self.confirm(false);
            if let Some(player) = self.ivars().player.upgrade()
                && !self.ivars().window.is_closing()
            {
                crate::diagnostics::report(
                    &player,
                    format!(
                        "Could not start Picture-in-Picture: {}",
                        error.localizedDescription()
                    ),
                );
            }
        }

        #[unsafe(method(pictureInPictureController:restoreUserInterfaceForPictureInPictureStopWithCompletionHandler:))]
        unsafe fn pictureInPictureController_restoreUserInterfaceForPictureInPictureStopWithCompletionHandler(
            &self,
            _controller: &AVPictureInPictureController,
            complete: &block2::DynBlock<dyn Fn(objc2::runtime::Bool)>,
        ) {
            complete.call((self.ivars().window.restore(true).into(),));
        }
    }

    #[allow(non_snake_case)]
    // SAFETY: the selectors below implement the AVKit protocol with its declared ABI.
    unsafe impl AVPictureInPictureSampleBufferPlaybackDelegate for PlaybackDelegate {
        #[unsafe(method(pictureInPictureController:setPlaying:))]
        unsafe fn pictureInPictureController_setPlaying(
            &self,
            _controller: &AVPictureInPictureController,
            playing: bool,
        ) {
            if let Err(error) = self.ivars().commands.send(Command::SetPaused(!playing)) {
                crate::diagnostics::warn!("Could not change Picture-in-Picture playback: {error}");
            }
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
            if seconds.is_finite()
                && seconds != 0.0
                && let Err(error) = self.ivars().commands.send(Command::Seek {
                    seconds,
                    absolute: false,
                })
            {
                crate::diagnostics::warn!("Could not seek in Picture-in-Picture: {error}");
            }
            completion_handler.call(());
        }
    }
);

impl PlaybackDelegate {
    pub(super) fn new(
        mtm: MainThreadMarker,
        player: slint::Weak<crate::PlayerWindow>,
        commands: CommandSender,
        playback: crate::PlaybackStore,
    ) -> Retained<Self> {
        let window = Rc::new(WindowPolicy::new(player.clone()));
        let this = Self::alloc(mtm).set_ivars(PlaybackDelegateIvars {
            player,
            commands,
            playback,
            transition: Cell::new(Transition::Idle),
            window,
        });
        // SAFETY: NSObject initializes the allocated, ivar-initialized object on the main thread.
        unsafe { msg_send![super(this), init] }
    }

    fn confirm(&self, active: bool) {
        let ivars = self.ivars();
        ivars.transition.set(Transition::Idle);
        let window = ivars.window.clone();
        let player = ivars.player.clone();
        let commands = ivars.commands.clone();
        // AVKit can complete a transition while the shared renderer is borrowed.
        slint::Timer::single_shot(Duration::ZERO, move || {
            let Some(player) = player.upgrade().filter(|_| !window.is_closing()) else {
                return;
            };
            if active {
                window.entered(&commands);
            } else {
                window.restore(false);
            }
            crate::player::presentation::set_picture_in_picture(&player, active);
        });
    }
}

pub(super) struct SampleQueue {
    pub(super) layer: Retained<AVSampleBufferDisplayLayer>,
    pub(super) renderer: Option<Retained<AVSampleBufferVideoRenderer>>,
}

#[allow(
    deprecated,
    reason = "macOS 12 and 13 expose the sample queue directly on the layer"
)]
impl SampleQueue {
    pub(super) fn flush(&self) {
        // SAFETY: the retained queue belongs to the player on the main thread.
        unsafe {
            if let Some(renderer) = &self.renderer {
                renderer.flushWithRemovalOfDisplayedImage_completionHandler(true, None);
            } else {
                self.layer.flushAndRemoveImage();
            }
        }
    }

    pub(super) fn ready(&self) -> bool {
        // SAFETY: the retained queue belongs to the player on the main thread.
        unsafe {
            self.renderer.as_ref().map_or_else(
                || self.layer.isReadyForMoreMediaData(),
                |renderer| renderer.isReadyForMoreMediaData(),
            )
        }
    }

    pub(super) fn enqueue(&self, sample: &CMSampleBuffer) {
        // SAFETY: the initialized buffer remains live through the main-thread enqueue.
        unsafe {
            if let Some(renderer) = &self.renderer {
                renderer.enqueueSampleBuffer(sample);
            } else {
                self.layer.enqueueSampleBuffer(sample);
            }
        }
    }
}
