use block2::RcBlock;
use objc2::{
    AnyThread,
    rc::Retained,
    runtime::{AnyObject, NSObject, ProtocolObject},
};
use objc2_app_kit::{
    NSApplicationDidBecomeActiveNotification, NSApplicationDidResignActiveNotification, NSImage,
    NSWorkspaceWillSleepNotification,
};
use objc2_core_foundation::{
    CFBoolean, CFData, CFDictionary, CFMutableData, CFNumber, CFRetained, CFString, CFType, CFURL,
    CGSize,
};
use objc2_core_graphics::CGImage;
use objc2_foundation::{
    NSArray, NSData, NSDictionary, NSError, NSNotificationCenter, NSNumber, NSObjectProtocol,
    NSOperationQueue, NSString, NSURL, NSUserDefaults,
};
use objc2_image_io::{CGImageDestination, CGImageSource};
use objc2_media_player::{
    MPChangePlaybackPositionCommandEvent, MPChangePlaybackRateCommandEvent,
    MPChangeRepeatModeCommandEvent, MPMediaItemArtwork, MPMediaItemPropertyAlbumTitle,
    MPMediaItemPropertyArtist, MPMediaItemPropertyArtwork, MPMediaItemPropertyPlaybackDuration,
    MPMediaItemPropertyTitle, MPNowPlayingInfoCenter, MPNowPlayingInfoMediaType,
    MPNowPlayingInfoPropertyAssetURL, MPNowPlayingInfoPropertyChapterCount,
    MPNowPlayingInfoPropertyChapterNumber, MPNowPlayingInfoPropertyDefaultPlaybackRate,
    MPNowPlayingInfoPropertyElapsedPlaybackTime, MPNowPlayingInfoPropertyMediaType,
    MPNowPlayingInfoPropertyPlaybackQueueCount, MPNowPlayingInfoPropertyPlaybackQueueIndex,
    MPNowPlayingInfoPropertyPlaybackRate, MPNowPlayingPlaybackState, MPRemoteCommand,
    MPRemoteCommandCenter, MPRemoteCommandEvent, MPRemoteCommandHandlerStatus, MPRepeatType,
    MPSkipIntervalCommandEvent,
};
use objc2_quick_look_thumbnailing::{
    QLThumbnailGenerationRequest, QLThumbnailGenerationRequestRepresentationTypes,
    QLThumbnailGenerator, QLThumbnailRepresentation,
};
use riina_core::{LoopMode, Phase, Playback};
use std::{
    path::PathBuf,
    ptr::NonNull,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

#[derive(Clone, Debug)]
pub(crate) enum RemoteMediaAction {
    Play,
    Pause,
    Toggle,
    Stop,
    Next,
    Previous,
    Forward(f64),
    Backward(f64),
    Position(f64),
    Rate(f32),
    Repeat,
    Sleep,
    Inactive,
    Active,
}

type HandlerBlock = dyn Fn(NonNull<MPRemoteCommandEvent>) -> MPRemoteCommandHandlerStatus;

struct RegisteredTarget {
    command: Retained<MPRemoteCommand>,
    target: Retained<AnyObject>,
}

struct GeneratedArtworkTask {
    generation: u64,
    cancel: Arc<AtomicBool>,
    receiver: Receiver<Result<Vec<u8>, String>>,
    thread: Option<JoinHandle<()>>,
}

impl GeneratedArtworkTask {
    fn start(
        library: PathBuf,
        media: String,
        track_id: Option<i64>,
        fallback_time: Option<f64>,
        generation: u64,
    ) -> Result<Self, String> {
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let (sender, receiver) = mpsc::channel();
        let thread = thread::Builder::new()
            .name("riina-now-playing-artwork".into())
            .spawn(move || {
                let result = capture_now_playing_artwork(
                    &library,
                    &media,
                    track_id,
                    fallback_time,
                    &worker_cancel,
                );
                let _ = sender.send(result);
            })
            .map_err(|error| format!("Could not start Now Playing artwork extraction: {error}"))?;
        Ok(Self {
            generation,
            cancel,
            receiver,
            thread: Some(thread),
        })
    }

    fn cancel(&self) {
        self.cancel.store(true, Ordering::Release);
    }

    fn try_recv(&self) -> Result<Result<Vec<u8>, String>, mpsc::TryRecvError> {
        self.receiver.try_recv()
    }
}

impl Drop for GeneratedArtworkTask {
    fn drop(&mut self) {
        self.cancel();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub(crate) struct SystemMediaControls {
    library: PathBuf,
    center: Retained<MPRemoteCommandCenter>,
    sender: SyncSender<RemoteMediaAction>,
    receiver: Receiver<RemoteMediaAction>,
    targets: Vec<RegisteredTarget>,
    now_playing: Retained<MPNowPlayingInfoCenter>,
    lifecycle_notification_center: Retained<NSNotificationCenter>,
    lifecycle_observers: Vec<Retained<ProtocolObject<dyn NSObjectProtocol>>>,
    last_published: Option<NowPlayingSnapshot>,
    artwork_task: Option<GeneratedArtworkTask>,
    artwork_attempt: Option<(u64, String)>,
    generated_artwork: Option<(u64, Vec<u8>)>,
    configuration: Option<(String, Option<f64>, Option<f64>)>,
    pending_install_at: Option<Instant>,
}

#[derive(PartialEq)]
struct NowPlayingSnapshot {
    generation: u64,
    title: String,
    artist: Option<String>,
    album: Option<String>,
    duration: Option<f64>,
    position: Option<f64>,
    speed: f64,
    paused: bool,
    phase: Phase,
    loop_mode: LoopMode,
    audio_media: bool,
    queue_count: usize,
    queue_index: Option<usize>,
    chapter_count: usize,
    chapter_index: Option<usize>,
    artwork_enabled: bool,
    asset_url: Option<String>,
    artwork_path: Option<String>,
    embedded_artwork_track: Option<i64>,
    fallback_video_track: Option<i64>,
    fallback_video_time: Option<f64>,
}

impl SystemMediaControls {
    pub(crate) fn new(library: PathBuf) -> Self {
        let (sender, receiver) = mpsc::sync_channel(64);
        // SAFETY: construction and all subsequent MediaPlayer access stay on
        // the macOS UI thread that owns this controller.
        let center = unsafe { MPRemoteCommandCenter::sharedCommandCenter() };
        // SAFETY: the default MediaPlayer info center is process-global and is
        // accessed on the same UI thread as its remote command center.
        let now_playing = unsafe { MPNowPlayingInfoCenter::defaultCenter() };
        let lifecycle_notification_center = NSNotificationCenter::defaultCenter();
        let sleep_sender = sender.clone();
        let sleep_block = RcBlock::new(move |_| {
            let _ = sleep_sender.try_send(RemoteMediaAction::Sleep);
        });
        let inactive_sender = sender.clone();
        let inactive_block = RcBlock::new(move |_| {
            let _ = inactive_sender.try_send(RemoteMediaAction::Inactive);
        });
        let active_sender = sender.clone();
        let active_block = RcBlock::new(move |_| {
            let _ = active_sender.try_send(RemoteMediaAction::Active);
        });
        // SAFETY: AppKit owns this notification and dispatches its observer on
        // the main operation queue; these blocks only send owned actions.
        let lifecycle_observers = unsafe {
            vec![
                lifecycle_notification_center.addObserverForName_object_queue_usingBlock(
                    Some(NSWorkspaceWillSleepNotification),
                    None,
                    Some(&NSOperationQueue::mainQueue()),
                    &sleep_block,
                ),
                lifecycle_notification_center.addObserverForName_object_queue_usingBlock(
                    Some(NSApplicationDidResignActiveNotification),
                    None,
                    Some(&NSOperationQueue::mainQueue()),
                    &inactive_block,
                ),
                lifecycle_notification_center.addObserverForName_object_queue_usingBlock(
                    Some(NSApplicationDidBecomeActiveNotification),
                    None,
                    Some(&NSOperationQueue::mainQueue()),
                    &active_block,
                ),
            ]
        };
        Self {
            library,
            center,
            sender,
            receiver,
            targets: Vec::new(),
            now_playing,
            lifecycle_notification_center,
            lifecycle_observers,
            last_published: None,
            artwork_task: None,
            artwork_attempt: None,
            generated_artwork: None,
            configuration: None,
            pending_install_at: None,
        }
    }

    pub(crate) fn synchronize(
        &mut self,
        active: bool,
        profile: &str,
        forward_interval: Option<f64>,
        backward_interval: Option<f64>,
    ) {
        let requested = active.then(|| (profile.to_owned(), forward_interval, backward_interval));
        if self.configuration == requested {
            if self
                .pending_install_at
                .is_some_and(|deadline| Instant::now() >= deadline)
            {
                self.pending_install_at = None;
                if let Some((_, forward_interval, backward_interval)) = requested {
                    self.install_targets(forward_interval, backward_interval);
                }
            }
            return;
        }
        let delay_reinstall = self.configuration.is_some() && requested.is_some();
        self.remove_targets();
        self.configuration = requested.clone();
        let Some((_, forward_interval, backward_interval)) = requested else {
            self.pending_install_at = None;
            self.last_published = None;
            self.clear_now_playing();
            return;
        };
        if delay_reinstall {
            self.pending_install_at = Some(Instant::now() + Duration::from_secs(1));
            return;
        }
        self.install_targets(forward_interval, backward_interval);
    }

    pub(crate) fn publish_playback(&mut self, state: &Playback, thumbnail_preview_enabled: bool) {
        let active = !matches!(state.phase, Phase::Stopped | Phase::Loading | Phase::Failed);
        if !active {
            if self.last_published.take().is_some() {
                self.clear_now_playing();
            }
            return;
        }

        let audio_media = state.audio_status() == riina_core::music_mode::AudioStatus::Audio;
        let artwork_enabled = now_playing_artwork_enabled();
        let album_art_track = if artwork_enabled {
            state
                .tracks
                .iter()
                .find(|track| track.kind == riina_core::TrackKind::Video && track.albumart)
        } else {
            None
        };
        let snapshot = NowPlayingSnapshot {
            generation: state.generation,
            title: state.title.clone(),
            artist: if audio_media {
                state.artist.clone()
            } else {
                None
            },
            album: if audio_media {
                state.album.clone()
            } else {
                None
            },
            duration: state
                .duration
                .filter(|value| value.is_finite() && *value >= 0.0),
            position: state
                .position
                .filter(|value| value.is_finite() && *value >= 0.0),
            speed: if state.speed.is_finite() {
                state.speed
            } else {
                1.0
            },
            paused: state.paused || state.phase != Phase::Playing,
            phase: state.phase,
            loop_mode: state.loop_mode(),
            audio_media,
            queue_count: state.playlist.len(),
            queue_index: state.playlist.iter().position(|entry| entry.current),
            chapter_count: state.chapters.len(),
            chapter_index: state.position.and_then(|position| {
                state
                    .chapters
                    .iter()
                    .enumerate()
                    .filter(|(_, chapter)| chapter.time.is_finite() && chapter.time <= position)
                    .map(|(index, _)| index)
                    .next_back()
            }),
            artwork_enabled,
            asset_url: state
                .playlist
                .iter()
                .find(|entry| entry.current)
                .map(|entry| entry.filename.clone()),
            artwork_path: album_art_track.and_then(|track| track.external_filename.clone()),
            embedded_artwork_track: album_art_track
                .filter(|track| track.external_filename.is_none())
                .map(|track| track.id),
            fallback_video_track: if artwork_enabled && thumbnail_preview_enabled && !audio_media {
                state
                    .tracks
                    .iter()
                    .find(|track| {
                        track.kind == riina_core::TrackKind::Video
                            && !track.albumart
                            && track.selected
                    })
                    .map(|track| track.id)
            } else {
                None
            },
            fallback_video_time: if artwork_enabled && thumbnail_preview_enabled && !audio_media {
                state
                    .duration
                    .filter(|duration| duration.is_finite() && *duration > 0.0)
                    .map(|duration| {
                        let requested = if duration < 120.0 {
                            duration / 2.0
                        } else {
                            60.0
                        };
                        let sample = (requested / duration
                            * f64::from(riina_mpv::thumbnail_generator::SAMPLE_COUNT))
                        .ceil()
                            - 1.0;
                        duration * sample / f64::from(riina_mpv::thumbnail_generator::SAMPLE_COUNT)
                    })
            } else {
                None
            },
        };
        let artwork_source_changed = self.last_published.as_ref().is_none_or(|previous| {
            previous.generation != snapshot.generation
                || previous.artwork_path != snapshot.artwork_path
                || previous.artwork_enabled != snapshot.artwork_enabled
                || previous.embedded_artwork_track != snapshot.embedded_artwork_track
                || previous.fallback_video_track != snapshot.fallback_video_track
                || previous.fallback_video_time != snapshot.fallback_video_time
                || previous.asset_url != snapshot.asset_url
        });
        if artwork_source_changed {
            self.artwork_task.take();
            self.artwork_attempt = None;
            self.generated_artwork = None;
        }
        if snapshot.artwork_enabled
            && let Some(media) = snapshot.asset_url.clone()
            && !riina_core::media_reference::is_network_url_candidate(&media)
            && self.artwork_attempt.as_ref() != Some(&(snapshot.generation, media.clone()))
        {
            self.artwork_attempt = Some((snapshot.generation, media.clone()));
            let selected_track = snapshot.embedded_artwork_track;
            match GeneratedArtworkTask::start(
                self.library.clone(),
                media,
                selected_track,
                snapshot.fallback_video_time,
                snapshot.generation,
            ) {
                Ok(task) => self.artwork_task = Some(task),
                Err(error) => eprintln!("{error}"),
            }
        }
        let artwork_finished = self
            .artwork_task
            .as_ref()
            .filter(|task| task.generation == snapshot.generation)
            .and_then(|task| task.try_recv().ok());
        let artwork_changed = if let Some(result) = artwork_finished {
            self.artwork_task.take();
            match result {
                Ok(image) => {
                    self.generated_artwork = Some((snapshot.generation, image));
                    true
                }
                Err(error) if error != "Now Playing artwork extraction was cancelled" => {
                    eprintln!("Could not extract Now Playing artwork: {error}");
                    false
                }
                Err(_) => false,
            }
        } else {
            false
        };
        let position_changed = self.last_published.as_ref().is_some_and(|previous| {
            match (previous.position, snapshot.position) {
                (Some(old), Some(new)) => (new - old).abs() >= 1.0,
                (old, new) => old != new,
            }
        });
        let metadata_changed = self.last_published.as_ref().is_none_or(|previous| {
            previous.generation != snapshot.generation
                || previous.title != snapshot.title
                || previous.artist != snapshot.artist
                || previous.album != snapshot.album
                || previous.duration != snapshot.duration
                || previous.speed != snapshot.speed
                || previous.paused != snapshot.paused
                || previous.phase != snapshot.phase
                || previous.loop_mode != snapshot.loop_mode
                || previous.audio_media != snapshot.audio_media
                || previous.queue_count != snapshot.queue_count
                || previous.queue_index != snapshot.queue_index
                || previous.chapter_count != snapshot.chapter_count
                || previous.chapter_index != snapshot.chapter_index
                || previous.artwork_enabled != snapshot.artwork_enabled
                || previous.asset_url != snapshot.asset_url
                || previous.artwork_path != snapshot.artwork_path
                || previous.embedded_artwork_track != snapshot.embedded_artwork_track
                || previous.fallback_video_track != snapshot.fallback_video_track
                || previous.fallback_video_time != snapshot.fallback_video_time
        });
        if metadata_changed || position_changed || artwork_changed || self.last_published.is_none()
        {
            self.write_now_playing(&snapshot);
            self.last_published = Some(snapshot);
        }
    }

    fn write_now_playing(&self, state: &NowPlayingSnapshot) {
        // SAFETY: MediaPlayer exports immutable NSString keys for its public
        // metadata dictionary, and this is called only on the UI thread.
        let (keys, values) = unsafe {
            let mut keys = vec![MPMediaItemPropertyTitle];
            let mut values = vec![NSString::from_str(&state.title).into_super()];
            if let Some(artist) = &state.artist {
                keys.push(MPMediaItemPropertyArtist);
                values.push(NSString::from_str(artist).into_super());
            }
            if let Some(album) = &state.album {
                keys.push(MPMediaItemPropertyAlbumTitle);
                values.push(NSString::from_str(album).into_super());
            }
            if let Some(asset_url) = state.asset_url.as_deref().and_then(now_playing_asset_url) {
                keys.push(MPNowPlayingInfoPropertyAssetURL);
                values.push(asset_url.into_super());
            }
            if let Some(artwork) = state.artwork_path.as_deref().and_then(now_playing_artwork) {
                keys.push(MPMediaItemPropertyArtwork);
                values.push(artwork.into_super());
            } else if let Some(artwork) = self
                .generated_artwork
                .as_ref()
                .filter(|(generation, _)| *generation == state.generation)
                .and_then(|(_, bytes)| now_playing_artwork_data(bytes))
            {
                keys.push(MPMediaItemPropertyArtwork);
                values.push(artwork.into_super());
            }
            if let Some(duration) = state.duration {
                keys.push(MPMediaItemPropertyPlaybackDuration);
                values.push(NSNumber::new_f64(duration).into_super().into_super());
            }
            keys.extend([
                MPNowPlayingInfoPropertyMediaType,
                MPNowPlayingInfoPropertyPlaybackQueueCount,
            ]);
            values.extend([
                NSNumber::new_i64(if state.audio_media {
                    MPNowPlayingInfoMediaType::Audio.0 as i64
                } else {
                    MPNowPlayingInfoMediaType::Video.0 as i64
                })
                .into_super()
                .into_super(),
                NSNumber::new_usize(state.queue_count)
                    .into_super()
                    .into_super(),
            ]);
            if let Some(index) = state.queue_index {
                keys.push(MPNowPlayingInfoPropertyPlaybackQueueIndex);
                values.push(NSNumber::new_usize(index).into_super().into_super());
            }
            if state.chapter_count > 0 {
                keys.push(MPNowPlayingInfoPropertyChapterCount);
                values.push(
                    NSNumber::new_usize(state.chapter_count)
                        .into_super()
                        .into_super(),
                );
                if let Some(index) = state.chapter_index {
                    keys.push(MPNowPlayingInfoPropertyChapterNumber);
                    values.push(NSNumber::new_usize(index).into_super().into_super());
                }
            }
            keys.extend([
                MPNowPlayingInfoPropertyElapsedPlaybackTime,
                MPNowPlayingInfoPropertyPlaybackRate,
                MPNowPlayingInfoPropertyDefaultPlaybackRate,
            ]);
            values.extend([
                NSNumber::new_f64(state.position.unwrap_or_default())
                    .into_super()
                    .into_super(),
                NSNumber::new_f64(if state.paused { 0.0 } else { state.speed })
                    .into_super()
                    .into_super(),
                NSNumber::new_f64(1.0).into_super().into_super(),
            ]);
            (keys, values)
        };
        let info: Retained<NSDictionary<NSString, NSObject>> =
            NSDictionary::from_retained_objects(&keys, &values);
        // SAFETY: its NSString, NSNumber, and NSURL values are all NSObjects and
        // therefore valid as AnyObject values.
        let info = unsafe { info.cast_unchecked::<NSString, objc2::runtime::AnyObject>() };
        // SAFETY: MediaPlayer copies this immutable dictionary on the UI thread.
        unsafe { self.now_playing.setNowPlayingInfo(Some(info)) };
        // SAFETY: playback state is owned by this app and updated on its UI thread.
        unsafe {
            self.now_playing.setPlaybackState(if state.paused {
                MPNowPlayingPlaybackState::Paused
            } else {
                MPNowPlayingPlaybackState::Playing
            });
        }
        let repeat_command = unsafe { self.center.changeRepeatModeCommand() };
        let repeat_type = match state.loop_mode {
            LoopMode::Off => MPRepeatType::Off,
            LoopMode::File => MPRepeatType::One,
            LoopMode::Playlist => MPRepeatType::All,
        };
        // SAFETY: current repeat state mirrors the app's observed loop mode.
        unsafe { repeat_command.setCurrentRepeatType(repeat_type) };
    }

    fn clear_now_playing(&mut self) {
        self.artwork_task.take();
        self.artwork_attempt = None;
        self.generated_artwork = None;
        // SAFETY: this app owns the info-center entry while media controls are active.
        unsafe {
            self.now_playing.setNowPlayingInfo(None);
            self.now_playing
                .setPlaybackState(MPNowPlayingPlaybackState::Stopped);
        }
    }

    pub(crate) fn drain_actions(&self) -> impl Iterator<Item = RemoteMediaAction> + '_ {
        self.receiver.try_iter()
    }

    fn install_targets(&mut self, forward_interval: Option<f64>, backward_interval: Option<f64>) {
        // SAFETY: MPRemoteCommandCenter objects are accessed only from the
        // main UI thread; the handler blocks enqueue owned actions and never
        // call into AppKit from MediaPlayer's callback.
        unsafe {
            self.add_simple(self.center.playCommand(), RemoteMediaAction::Play);
            self.add_simple(self.center.pauseCommand(), RemoteMediaAction::Pause);
            self.add_simple(
                self.center.togglePlayPauseCommand(),
                RemoteMediaAction::Toggle,
            );
            self.add_simple(self.center.stopCommand(), RemoteMediaAction::Stop);
            self.add_simple(self.center.nextTrackCommand(), RemoteMediaAction::Next);
            self.add_simple(
                self.center.previousTrackCommand(),
                RemoteMediaAction::Previous,
            );
            let repeat = self.center.changeRepeatModeCommand();
            let sender = self.sender.clone();
            self.add_handler(repeat.into_super(), repeat_handler(sender));

            let forward = self.center.skipForwardCommand();
            self.set_interval(&forward, forward_interval);
            let sender = self.sender.clone();
            self.add_handler(forward.into_super(), skip_handler(sender, true));

            let backward = self.center.skipBackwardCommand();
            self.set_interval(&backward, backward_interval);
            let sender = self.sender.clone();
            self.add_handler(backward.into_super(), skip_handler(sender, false));

            let position = self.center.changePlaybackPositionCommand();
            let sender = self.sender.clone();
            self.add_handler(position.into_super(), position_handler(sender));

            let rate = self.center.changePlaybackRateCommand();
            let supported = [
                NSNumber::new_f64(0.5),
                NSNumber::new_f64(1.0),
                NSNumber::new_f64(1.5),
                NSNumber::new_f64(2.0),
            ];
            let rates = NSArray::from_retained_slice(&supported);
            rate.setSupportedPlaybackRates(&rates);
            let sender = self.sender.clone();
            self.add_handler(rate.into_super(), rate_handler(sender));
        }
    }

    unsafe fn add_simple(&mut self, command: Retained<MPRemoteCommand>, action: RemoteMediaAction) {
        let sender = self.sender.clone();
        unsafe { self.add_handler(command, move |_| enqueue(&sender, action.clone())) };
    }

    unsafe fn add_handler(
        &mut self,
        command: Retained<MPRemoteCommand>,
        handler: impl Fn(NonNull<MPRemoteCommandEvent>) -> MPRemoteCommandHandlerStatus + 'static,
    ) {
        let block: RcBlock<HandlerBlock> = RcBlock::new(handler);
        let target = unsafe { command.addTargetWithHandler(&block) };
        unsafe { command.setEnabled(true) };
        self.targets.push(RegisteredTarget { command, target });
    }

    unsafe fn set_interval(
        &self,
        command: &objc2_media_player::MPSkipIntervalCommand,
        interval: Option<f64>,
    ) {
        let interval = interval
            .filter(|value| value.is_finite() && *value > 0.0)
            .map(NSNumber::new_f64);
        let values = match interval {
            Some(interval) => NSArray::from_retained_slice(&[interval]),
            None => NSArray::<NSNumber>::from_slice(&[]),
        };
        unsafe { command.setPreferredIntervals(&values) };
    }

    fn remove_targets(&mut self) {
        for target in self.targets.drain(..) {
            // SAFETY: each token is returned by addTargetWithHandler and is
            // retained until it is removed from the same command object.
            unsafe {
                target.command.removeTarget(Some(&target.target));
            }
        }
    }
}

fn now_playing_asset_url(value: &str) -> Option<Retained<NSURL>> {
    if riina_core::media_reference::is_network_url_candidate(value) || value.starts_with("file://")
    {
        NSURL::URLWithString(&NSString::from_str(value))
    } else {
        Some(NSURL::fileURLWithPath(&NSString::from_str(value)))
    }
}

fn now_playing_artwork_enabled() -> bool {
    let defaults = NSUserDefaults::standardUserDefaults();
    let key = NSString::from_str("enableNowPlayingArtwork");
    if defaults.objectForKey(&key).is_none() {
        return true;
    }
    defaults.boolForKey(&key)
}

fn now_playing_artwork(path: &str) -> Option<Retained<MPMediaItemArtwork>> {
    let url = CFURL::from_file_path(path)?;
    // ImageIO creates an orientation-correct thumbnail directly from the file,
    // avoiding a full-resolution decode for large external cover images.
    let source = unsafe { CGImageSource::with_url(&url, None) }?;
    let thumbnail = image_source_thumbnail_options();
    let image = unsafe { source.thumbnail_at_index(0, Some(thumbnail.as_opaque())) }?;
    let size = CGSize::new(
        CGImage::width(Some(&image)) as f64,
        CGImage::height(Some(&image)) as f64,
    );
    let image = NSImage::initWithCGImage_size(NSImage::alloc(), &image, size);
    now_playing_artwork_image(image)
}

fn image_source_thumbnail_options() -> CFRetained<CFDictionary<CFType, CFType>> {
    let create_key = CFString::from_str("kCGImageSourceCreateThumbnailFromImageAlways");
    let transform_key = CFString::from_str("kCGImageSourceCreateThumbnailWithTransform");
    let size_key = CFString::from_str("kCGImageSourceThumbnailMaxPixelSize");
    let maximum_size = CFNumber::new_isize(768);
    CFDictionary::from_slices(
        &[
            create_key.as_ref(),
            transform_key.as_ref(),
            size_key.as_ref(),
        ],
        &[
            CFBoolean::new(true).as_ref(),
            CFBoolean::new(true).as_ref(),
            maximum_size.as_ref(),
        ],
    )
}

fn now_playing_artwork_data(bytes: &[u8]) -> Option<Retained<MPMediaItemArtwork>> {
    // SAFETY: the byte slice is valid for the duration of this copying initializer.
    let data = unsafe { NSData::dataWithBytes_length(bytes.as_ptr().cast(), bytes.len()) };
    let image = NSImage::initWithData(NSImage::alloc(), &data)?;
    now_playing_artwork_image(image)
}

fn now_playing_artwork_image(image: Retained<NSImage>) -> Option<Retained<MPMediaItemArtwork>> {
    let bounds_size = image.size();
    if !bounds_size.width.is_finite()
        || !bounds_size.height.is_finite()
        || bounds_size.width <= 0.0
        || bounds_size.height <= 0.0
    {
        return None;
    }
    let request_handler: RcBlock<dyn Fn(CGSize) -> NonNull<NSImage>> =
        RcBlock::new(move |_| NonNull::from(image.as_ref()));
    // SAFETY: the block captures the retained image for its full lifetime and
    // returns that live object whenever MediaPlayer requests artwork pixels.
    Some(unsafe {
        MPMediaItemArtwork::initWithBoundsSize_requestHandler(
            MPMediaItemArtwork::alloc(),
            bounds_size,
            &request_handler,
        )
    })
}

fn capture_now_playing_artwork(
    library: &std::path::Path,
    media: &str,
    track_id: Option<i64>,
    fallback_time: Option<f64>,
    cancel: &AtomicBool,
) -> Result<Vec<u8>, String> {
    if let Some(track_id) = track_id
        && let Ok(bytes) = capture_video_artwork(library, media, track_id, cancel)
    {
        return Ok(bytes);
    }
    if cancel.load(Ordering::Acquire) {
        return Err("Now Playing artwork extraction was cancelled".into());
    }
    if let Ok(bytes) = capture_quick_look_artwork(media, cancel) {
        return Ok(bytes);
    }
    if cancel.load(Ordering::Acquire) {
        return Err("Now Playing artwork extraction was cancelled".into());
    }
    if let Some(seconds) = fallback_time
        && let Ok(bytes) = capture_timeline_artwork(library, media, seconds, cancel)
    {
        return Ok(bytes);
    }
    Err("No Now Playing artwork could be generated".into())
}

fn capture_video_artwork(
    library: &std::path::Path,
    media: &str,
    track_id: i64,
    cancel: &AtomicBool,
) -> Result<Vec<u8>, String> {
    let mut generator = riina_mpv::thumbnail_generator::Generator::start_for_video_track(
        library, media, 768, track_id,
    )?;
    receive_video_artwork(&mut generator, cancel)
}

fn capture_timeline_artwork(
    library: &std::path::Path,
    media: &str,
    seconds: f64,
    cancel: &AtomicBool,
) -> Result<Vec<u8>, String> {
    let mut generator =
        riina_mpv::thumbnail_generator::Generator::start_at_time(library, media, 768, seconds)?;
    receive_video_artwork(&mut generator, cancel)
}

fn receive_video_artwork(
    generator: &mut riina_mpv::thumbnail_generator::Generator,
    cancel: &AtomicBool,
) -> Result<Vec<u8>, String> {
    loop {
        if cancel.load(Ordering::Acquire) {
            generator.cancel();
            return Err("Now Playing artwork extraction was cancelled".into());
        }
        match generator.try_recv() {
            Ok(riina_mpv::thumbnail_generator::Event::Sample(sample)) => {
                generator.cancel();
                return encode_artwork_jpeg(&sample.frame);
            }
            Ok(riina_mpv::thumbnail_generator::Event::Finished(Ok(()))) => {
                return Err("The artwork track produced no video frame".into());
            }
            Ok(riina_mpv::thumbnail_generator::Event::Finished(Err(error))) => return Err(error),
            Err(mpsc::TryRecvError::Empty) => thread::sleep(Duration::from_millis(20)),
            Err(mpsc::TryRecvError::Disconnected) => {
                return Err("The artwork decoder stopped unexpectedly".into());
            }
        }
    }
}

fn capture_quick_look_artwork(media: &str, cancel: &AtomicBool) -> Result<Vec<u8>, String> {
    if cancel.load(Ordering::Acquire) {
        return Err("Now Playing artwork extraction was cancelled".into());
    }
    let url = if media.starts_with("file://") {
        NSURL::URLWithString(&NSString::from_str(media))
    } else {
        Some(NSURL::fileURLWithPath(&NSString::from_str(media)))
    }
    .ok_or("The media path is not a valid file URL")?;
    let mut last_error = "Quick Look did not produce a thumbnail".to_owned();
    for _ in 0..3 {
        match capture_quick_look_request(&url, cancel) {
            Ok(bytes) => return Ok(bytes),
            Err(error) if error == "Now Playing artwork extraction was cancelled" => {
                return Err(error);
            }
            Err(error) => last_error = error,
        }
    }
    Err(last_error)
}

fn capture_quick_look_request(url: &NSURL, cancel: &AtomicBool) -> Result<Vec<u8>, String> {
    if cancel.load(Ordering::Acquire) {
        return Err("Now Playing artwork extraction was cancelled".into());
    }
    let request = unsafe {
        QLThumbnailGenerationRequest::initWithFileAtURL_size_scale_representationTypes(
            QLThumbnailGenerationRequest::alloc(),
            url,
            CGSize::new(2000.0, 2000.0),
            1.0,
            QLThumbnailGenerationRequestRepresentationTypes::LowQualityThumbnail,
        )
    };
    let generator = unsafe { QLThumbnailGenerator::sharedGenerator() };
    let (sender, receiver) = mpsc::sync_channel(1);
    let completion: RcBlock<dyn Fn(*mut QLThumbnailRepresentation, *mut NSError)> = RcBlock::new(
        move |representation: *mut QLThumbnailRepresentation, _error: *mut NSError| {
            let result = if representation.is_null() {
                Err("Quick Look did not produce a thumbnail".to_owned())
            } else {
                // SAFETY: Quick Look supplies a live representation for this callback.
                let representation = unsafe { &*representation };
                // SAFETY: the callback keeps the representation alive until encoding finishes.
                let image = unsafe { representation.CGImage() };
                encode_cgimage_jpeg(&image)
            };
            let _ = sender.send(result);
        },
    );
    unsafe {
        generator.generateBestRepresentationForRequest_completionHandler(&request, &completion)
    };
    let mut cancellation_requested = false;
    loop {
        if cancel.load(Ordering::Acquire) && !cancellation_requested {
            // The completion block remains alive until Quick Look confirms cancellation.
            unsafe { generator.cancelRequest(&request) };
            cancellation_requested = true;
        }
        match receiver.recv_timeout(Duration::from_millis(50)) {
            Ok(result) => {
                if cancel.load(Ordering::Acquire) {
                    return Err("Now Playing artwork extraction was cancelled".into());
                }
                return result;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err("Quick Look stopped before returning a thumbnail".into());
            }
        }
    }
}

fn encode_cgimage_jpeg(image: &CGImage) -> Result<Vec<u8>, String> {
    let output = CFMutableData::new(None, 0).ok_or("Could not allocate Quick Look artwork data")?;
    let jpeg_type = CFString::from_str("public.jpeg");
    // SAFETY: the destination writes one JPEG image into the retained mutable data.
    let destination = unsafe { CGImageDestination::with_data(&output, &jpeg_type, 1, None) }
        .ok_or("Could not create a JPEG encoder for Quick Look artwork")?;
    // SAFETY: image and destination remain alive for the duration of the encoding call.
    unsafe { destination.add_image(image, None) };
    // SAFETY: finalization completes the single-image destination before reading its data.
    if !unsafe { destination.finalize() } {
        return Err("Could not encode Quick Look artwork".into());
    }
    let data: &CFData = output.as_ref();
    let length = usize::try_from(data.length())
        .map_err(|_| "Quick Look produced artwork with an invalid length")?;
    if length == 0 {
        return Err("Quick Look produced empty artwork data".into());
    }
    // SAFETY: CFData owns this pointer and it remains valid while output is retained.
    let bytes = unsafe { std::slice::from_raw_parts(data.byte_ptr(), length) };
    Ok(bytes.to_vec())
}

fn encode_artwork_jpeg(frame: &riina_core::CapturedFrame) -> Result<Vec<u8>, String> {
    let expected = (frame.width as usize)
        .checked_mul(frame.height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or("The artwork frame is too large")?;
    if frame.rgba.len() != expected {
        return Err("The artwork frame has incomplete RGBA pixels".into());
    }
    let mut rgb = Vec::with_capacity(expected / 4 * 3);
    for pixel in frame.rgba.as_chunks::<4>().0 {
        rgb.extend_from_slice(&pixel[..3]);
    }
    let mut jpeg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 85)
        .encode(
            &rgb,
            frame.width,
            frame.height,
            image::ExtendedColorType::Rgb8,
        )
        .map_err(|error| format!("Could not encode embedded artwork: {error}"))?;
    Ok(jpeg)
}

impl Drop for SystemMediaControls {
    fn drop(&mut self) {
        self.remove_targets();
        self.clear_now_playing();
        // SAFETY: these are the retained observer tokens returned by this center.
        unsafe {
            for observer in &self.lifecycle_observers {
                self.lifecycle_notification_center
                    .removeObserver(observer.as_ref());
            }
        }
    }
}

fn repeat_handler(
    sender: SyncSender<RemoteMediaAction>,
) -> impl Fn(NonNull<MPRemoteCommandEvent>) -> MPRemoteCommandHandlerStatus + 'static {
    move |event| {
        // SAFETY: MediaPlayer passes a live command-event object for this callback.
        let event = unsafe { event.as_ref() };
        if event
            .downcast_ref::<MPChangeRepeatModeCommandEvent>()
            .is_none()
        {
            return MPRemoteCommandHandlerStatus::CommandFailed;
        }
        enqueue(&sender, RemoteMediaAction::Repeat)
    }
}

fn skip_handler(
    sender: SyncSender<RemoteMediaAction>,
    forward: bool,
) -> impl Fn(NonNull<MPRemoteCommandEvent>) -> MPRemoteCommandHandlerStatus + 'static {
    move |event| {
        // SAFETY: MediaPlayer passes a live command-event object for this callback.
        let event = unsafe { event.as_ref() };
        let Some(event) = event.downcast_ref::<MPSkipIntervalCommandEvent>() else {
            return MPRemoteCommandHandlerStatus::CommandFailed;
        };
        // SAFETY: the typed event was checked immediately above.
        let interval = unsafe { event.interval() };
        enqueue(
            &sender,
            if forward {
                RemoteMediaAction::Forward(interval)
            } else {
                RemoteMediaAction::Backward(interval)
            },
        )
    }
}

fn position_handler(
    sender: SyncSender<RemoteMediaAction>,
) -> impl Fn(NonNull<MPRemoteCommandEvent>) -> MPRemoteCommandHandlerStatus + 'static {
    move |event| {
        // SAFETY: MediaPlayer passes a live command-event object for this callback.
        let event = unsafe { event.as_ref() };
        let Some(event) = event.downcast_ref::<MPChangePlaybackPositionCommandEvent>() else {
            return MPRemoteCommandHandlerStatus::CommandFailed;
        };
        // SAFETY: the typed event was checked immediately above.
        enqueue(
            &sender,
            RemoteMediaAction::Position(unsafe { event.positionTime() }),
        )
    }
}

fn rate_handler(
    sender: SyncSender<RemoteMediaAction>,
) -> impl Fn(NonNull<MPRemoteCommandEvent>) -> MPRemoteCommandHandlerStatus + 'static {
    move |event| {
        // SAFETY: MediaPlayer passes a live command-event object for this callback.
        let event = unsafe { event.as_ref() };
        let Some(event) = event.downcast_ref::<MPChangePlaybackRateCommandEvent>() else {
            return MPRemoteCommandHandlerStatus::CommandFailed;
        };
        // SAFETY: the typed event was checked immediately above.
        enqueue(
            &sender,
            RemoteMediaAction::Rate(unsafe { event.playbackRate() }),
        )
    }
}

fn enqueue(
    sender: &SyncSender<RemoteMediaAction>,
    action: RemoteMediaAction,
) -> MPRemoteCommandHandlerStatus {
    match sender.try_send(action) {
        Ok(()) => MPRemoteCommandHandlerStatus::Success,
        Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => {
            MPRemoteCommandHandlerStatus::CommandFailed
        }
    }
}
