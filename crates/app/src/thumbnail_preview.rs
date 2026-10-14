use crate::PlayerWindow;
use riina_core::{Phase, Playback, music_mode::AudioStatus, thumbnail_preferences::Settings};
use slint::{Image, Rgba8Pixel, SharedPixelBuffer};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

const THUMBNAIL_COUNT: usize = riina_mpv::thumbnail_generator::SAMPLE_COUNT as usize + 1;
const EVENT_CAPACITY: usize = 4;

enum PreviewEvent {
    Frame {
        index: usize,
        timestamp: f64,
        jpeg: Vec<u8>,
    },
    Finished(Result<(), String>),
}

/// Owns a cache lookup/extraction worker; UI callbacks only drain bounded JPEG events.
struct PreviewTask {
    cancel: Arc<AtomicBool>,
    events: Option<Receiver<PreviewEvent>>,
    thread: Option<JoinHandle<()>>,
}

impl PreviewTask {
    fn start(
        library: PathBuf,
        source: PathBuf,
        settings: Settings,
        cache_root: Option<PathBuf>,
    ) -> Result<Self, String> {
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let (sender, events) = mpsc::sync_channel(EVENT_CAPACITY);
        let thread = thread::Builder::new()
            .name("riina-thumbnail-preview".into())
            .spawn(move || {
                run_preview_task(
                    &library,
                    &source,
                    &settings,
                    cache_root.as_deref(),
                    &sender,
                    &worker_cancel,
                )
            })
            .map_err(|error| format!("Could not start thumbnail preview: {error}"))?;
        Ok(Self {
            cancel,
            events: Some(events),
            thread: Some(thread),
        })
    }

    fn try_recv(&self) -> Result<PreviewEvent, mpsc::TryRecvError> {
        self.events
            .as_ref()
            .expect("thumbnail event receiver remains available until task drop")
            .try_recv()
    }

    fn cancel(&self) {
        self.cancel.store(true, Ordering::Release);
    }
}

impl Drop for PreviewTask {
    fn drop(&mut self) {
        self.cancel();
        drop(self.events.take());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Identity {
    generation: u64,
    media_path: String,
    settings: Settings,
}

/// Binds the independent decoder to the current loaded video and supplies the
/// nearest generated frame as the timeline pointer moves.
#[derive(Default)]
pub(crate) struct Controller {
    identity: Option<Identity>,
    task: Option<PreviewTask>,
    frames: Vec<Option<(f64, Vec<u8>)>>,
    hover_time: Option<f64>,
    displayed_index: Option<usize>,
}

impl Controller {
    pub(crate) fn observe(
        &mut self,
        app: &PlayerWindow,
        state: &Playback,
        settings: &Settings,
        library: &Path,
    ) -> bool {
        app.set_thumbnail_preview_width(settings.width as f32);
        let next = active_identity(state, settings);
        if next != self.identity {
            self.cancel_current();
            self.identity = next.clone();
            self.frames.clear();
            self.displayed_index = None;
            app.set_timeline_thumbnail_image(Image::default());
            app.set_timeline_preview_label("".into());
            if let Some(identity) = next {
                self.frames.resize_with(THUMBNAIL_COUNT, || None);
                let source = PathBuf::from(&identity.media_path);
                let cache_root = crate::utilities_preferences::thumbnail_cache_directory().ok();
                match PreviewTask::start(
                    library.to_path_buf(),
                    source,
                    settings.clone(),
                    cache_root,
                ) {
                    Ok(task) => self.task = Some(task),
                    Err(error) => {
                        app.set_status(format!("Thumbnail preview unavailable: {error}").into());
                    }
                }
            }
        }

        let mut events = Vec::with_capacity(4);
        if let Some(task) = self.task.as_ref() {
            // Bounded batches prevent thumbnail delivery from monopolizing a UI tick.
            for _ in 0..4 {
                match task.try_recv() {
                    Ok(event) => {
                        let finished = matches!(event, PreviewEvent::Finished(_));
                        events.push(event);
                        if finished {
                            break;
                        }
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        break;
                    }
                }
            }
        }
        let mut thumbnails_ready = false;
        for event in events {
            match event {
                PreviewEvent::Frame {
                    index,
                    timestamp,
                    jpeg,
                } => {
                    if let Some(slot) = self.frames.get_mut(index) {
                        *slot = Some((timestamp, jpeg));
                    }
                }
                PreviewEvent::Finished(result) => {
                    self.task.take();
                    thumbnails_ready = result.is_ok() && complete_frame_set(&self.frames);
                    if let Err(error) = result
                        && error != "Thumbnail extraction was cancelled"
                    {
                        app.set_status(format!("Thumbnail extraction failed: {error}").into());
                    }
                }
            }
        }
        self.refresh_preview(app);
        thumbnails_ready
    }

    pub(crate) fn hover(&mut self, app: &PlayerWindow, seconds: f64) {
        if !seconds.is_finite() || seconds < 0.0 {
            self.hover_time = None;
            self.displayed_index = None;
            app.set_timeline_preview_label("".into());
            app.set_timeline_thumbnail_image(Image::default());
            return;
        }
        self.hover_time = Some(seconds);
        app.set_timeline_preview_label(riina_core::time_label(Some(seconds)).into());
        self.refresh_preview(app);
    }

    fn refresh_preview(&mut self, app: &PlayerWindow) {
        let Some(seconds) = self.hover_time else {
            return;
        };
        let index = nearest_sample_index(&self.frames, seconds);
        if index == self.displayed_index {
            return;
        }
        let Some((_, jpeg)) = index.and_then(|index| self.frames[index].as_ref()) else {
            self.displayed_index = None;
            app.set_timeline_thumbnail_image(Image::default());
            return;
        };
        match decode_image(jpeg) {
            Ok(image) => {
                self.displayed_index = index;
                app.set_timeline_thumbnail_image(image);
            }
            Err(error) => {
                self.displayed_index = index;
                app.set_timeline_thumbnail_image(Image::default());
                app.set_status(format!("Could not display a thumbnail: {error}").into());
            }
        }
    }

    fn cancel_current(&mut self) {
        let Some(task) = self.task.take() else {
            return;
        };
        task.cancel();
        let _ = thread::Builder::new()
            .name("riina-thumbnail-cleanup".into())
            .spawn(move || drop(task));
    }
}

impl Drop for Controller {
    fn drop(&mut self) {
        self.cancel_current();
    }
}

fn active_identity(state: &Playback, settings: &Settings) -> Option<Identity> {
    if !settings.enabled
        || !matches!(state.phase, Phase::Playing | Phase::Paused)
        || state.audio_status() != AudioStatus::Video
        || !state
            .duration
            .is_some_and(|duration| duration.is_finite() && duration > 0.0)
    {
        return None;
    }
    let source = state
        .playlist
        .iter()
        .find(|entry| entry.current)?
        .filename
        .as_str();
    let path = media_path(source)?;
    Some(Identity {
        generation: state.generation,
        media_path: path.to_string_lossy().into_owned(),
        settings: settings.clone(),
    })
}

fn media_path(value: &str) -> Option<PathBuf> {
    if value.contains("://") {
        return None;
    }
    let path = Path::new(value);
    path.is_absolute().then(|| path.to_path_buf())
}

fn volume_is_allowed(settings: &Settings, is_remote: bool) -> bool {
    settings.enable_for_remote_files || !is_remote
}

fn nearest_sample_index(frames: &[Option<(f64, Vec<u8>)>], seconds: f64) -> Option<usize> {
    frames.iter().rposition(|frame| {
        frame
            .as_ref()
            .is_some_and(|(timestamp, _)| *timestamp <= seconds)
    })
}

fn complete_frame_set(frames: &[Option<(f64, Vec<u8>)>]) -> bool {
    !frames.is_empty() && frames.iter().all(Option::is_some)
}

fn run_preview_task(
    library: &Path,
    source: &Path,
    settings: &Settings,
    cache_root: Option<&Path>,
    events: &SyncSender<PreviewEvent>,
    cancel: &AtomicBool,
) {
    let width = settings.width.saturating_mul(2);
    // Mount inspection may touch an unavailable network drive, so keep it off
    // the UI thread alongside cache reads and media decoding.
    if !volume_is_allowed(settings, crate::platform::is_remote_volume(source)) {
        let _ = send_preview_event(events, PreviewEvent::Finished(Ok(())), cancel);
        return;
    }
    if let Some(root) = cache_root
        && settings.maximum_cache_size_mb > 0
        && let Ok(Some(frames)) =
            crate::thumbnail_cache::load(root, source, width, settings.maximum_cache_size_mb)
    {
        for (index, (timestamp, jpeg)) in frames.into_iter().enumerate() {
            if !send_preview_event(
                events,
                PreviewEvent::Frame {
                    index,
                    timestamp,
                    jpeg,
                },
                cancel,
            ) {
                return;
            }
        }
        let _ = send_preview_event(events, PreviewEvent::Finished(Ok(())), cancel);
        return;
    }

    let generator = match riina_mpv::thumbnail_generator::Generator::start(
        library,
        &source.to_string_lossy(),
        width,
    ) {
        Ok(generator) => generator,
        Err(error) => {
            let _ = send_preview_event(events, PreviewEvent::Finished(Err(error)), cancel);
            return;
        }
    };
    let mut frames = vec![None; THUMBNAIL_COUNT];
    let outcome = loop {
        if cancel.load(Ordering::Acquire) {
            generator.cancel();
            break Err("Thumbnail extraction was cancelled".to_owned());
        }
        match generator.try_recv() {
            Ok(riina_mpv::thumbnail_generator::Event::Sample(sample)) => {
                match encode_jpeg(sample.frame.width, sample.frame.height, &sample.frame.rgba) {
                    Ok(jpeg) => {
                        let index = sample.index as usize;
                        if let Some(slot) = frames.get_mut(index) {
                            *slot = Some((sample.timestamp, jpeg.clone()));
                        }
                        if !send_preview_event(
                            events,
                            PreviewEvent::Frame {
                                index,
                                timestamp: sample.timestamp,
                                jpeg,
                            },
                            cancel,
                        ) {
                            generator.cancel();
                            break Err("Thumbnail extraction was cancelled".to_owned());
                        }
                    }
                    Err(error) => {
                        generator.cancel();
                        break Err(error);
                    }
                }
            }
            Ok(riina_mpv::thumbnail_generator::Event::Finished(result)) => break result,
            Err(mpsc::TryRecvError::Empty) => thread::sleep(Duration::from_millis(10)),
            Err(mpsc::TryRecvError::Disconnected) => {
                break Err("Thumbnail extraction stopped unexpectedly".to_owned());
            }
        }
    };
    drop(generator);
    if outcome.is_ok()
        && let Some(root) = cache_root
    {
        let complete = frames
            .iter()
            .map(|frame| {
                frame
                    .clone()
                    .ok_or("Thumbnail extraction returned an incomplete frame set")
            })
            .collect::<Result<Vec<_>, _>>();
        if let Ok(complete) = complete
            && let Err(error) = crate::thumbnail_cache::store(
                root,
                source,
                width,
                settings.maximum_cache_size_mb,
                &complete,
            )
        {
            eprintln!("Could not store thumbnail cache entry: {error}");
        }
    }
    let _ = send_preview_event(events, PreviewEvent::Finished(outcome), cancel);
}

fn send_preview_event(
    events: &SyncSender<PreviewEvent>,
    mut event: PreviewEvent,
    cancel: &AtomicBool,
) -> bool {
    loop {
        if cancel.load(Ordering::Acquire) {
            return false;
        }
        match events.try_send(event) {
            Ok(()) => return true,
            Err(TrySendError::Disconnected(_)) => return false,
            Err(TrySendError::Full(returned)) => {
                event = returned;
                thread::sleep(Duration::from_millis(10));
            }
        }
    }
}

fn encode_jpeg(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, String> {
    let mut rgb = Vec::with_capacity(width as usize * height as usize * 3);
    let (pixels, trailing) = rgba.as_chunks::<4>();
    if !trailing.is_empty() {
        return Err("The thumbnail frame has incomplete RGBA pixels".into());
    }
    for pixel in pixels {
        rgb.extend_from_slice(&pixel[..3]);
    }
    let mut encoded = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut encoded, 75)
        .encode(&rgb, width, height, image::ExtendedColorType::Rgb8)
        .map_err(|error| error.to_string())?;
    Ok(encoded)
}

fn decode_image(data: &[u8]) -> Result<Image, String> {
    let pixels = image::load_from_memory(data)
        .map_err(|error| error.to_string())?
        .to_rgba8();
    let (width, height) = pixels.dimensions();
    let buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(pixels.as_raw(), width, height);
    Ok(Image::from_rgba8(buffer))
}

#[cfg(test)]
mod tests {
    use super::{
        EVENT_CAPACITY, PreviewEvent, THUMBNAIL_COUNT, complete_frame_set, media_path,
        nearest_sample_index, run_preview_task, volume_is_allowed,
    };
    use riina_core::thumbnail_preferences::Settings;
    use std::{
        fs,
        path::PathBuf,
        sync::{Arc, atomic::AtomicBool, mpsc},
        time::SystemTime,
    };

    struct TempTree(PathBuf);

    impl TempTree {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "riina-thumbnail-preview-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir_all(&root).unwrap();
            Self(root)
        }
    }

    impl Drop for TempTree {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn only_absolute_local_files_are_started_by_the_player() {
        let absolute_path = std::env::temp_dir().join("clip.mkv");
        assert!(media_path(absolute_path.to_str().unwrap()).is_some());
        #[cfg(windows)]
        assert!(media_path("C:\\Videos\\clip.mkv").is_some());
        #[cfg(not(windows))]
        assert!(media_path("C:\\Videos\\clip.mkv").is_none());
        assert!(media_path("relative\\clip.mkv").is_none());
        assert!(media_path("https://example.com/clip.mkv").is_none());
    }

    #[test]
    fn remote_volumes_follow_the_opt_in_preference() {
        assert!(!volume_is_allowed(&Settings::default(), true));
        assert!(volume_is_allowed(&Settings::default(), false));
        assert!(volume_is_allowed(
            &Settings {
                enable_for_remote_files: true,
                ..Settings::default()
            },
            true
        ));
    }

    #[test]
    fn hover_uses_the_nearest_available_frame_at_or_before_pointer_time() {
        let frames = vec![
            Some((0.0, vec![])),
            None,
            Some((2.0, vec![])),
            Some((3.0, vec![])),
        ];
        assert_eq!(nearest_sample_index(&frames, 1.9), Some(0));
        assert_eq!(nearest_sample_index(&frames, 2.8), Some(2));
        assert_eq!(nearest_sample_index(&frames, 3.1), Some(3));
        assert_eq!(nearest_sample_index(&frames, -0.1), None);
    }

    #[test]
    fn thumbnail_completion_requires_every_sample() {
        assert!(complete_frame_set(&[
            Some((0.0, vec![])),
            Some((1.0, vec![]))
        ]));
        assert!(!complete_frame_set(&[Some((0.0, vec![])), None]));
        assert!(!complete_frame_set(&[]));
    }

    #[test]
    fn preview_worker_reuses_cached_frames_without_opening_libmpv() {
        let temp = TempTree::new();
        let source = temp.0.join("clip.mkv");
        let cache = temp.0.join("cache");
        fs::write(&source, b"fixture media identity").unwrap();
        let cached = (0..THUMBNAIL_COUNT)
            .map(|index| (index as f64, vec![index as u8, 7, 8]))
            .collect::<Vec<_>>();
        crate::thumbnail_cache::store(&cache, &source, 240, 500, &cached).unwrap();

        let settings = Settings::default();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let (sender, receiver) = mpsc::sync_channel(EVENT_CAPACITY);
        let missing_library = temp.0.join("libmpv-must-not-open-on-cache-hit.dll");
        let worker_source = source.clone();
        let worker_cache = cache.clone();
        let worker = std::thread::spawn(move || {
            run_preview_task(
                &missing_library,
                &worker_source,
                &settings,
                Some(&worker_cache),
                &sender,
                &worker_cancel,
            );
        });
        for (index, (timestamp, jpeg)) in cached.iter().enumerate() {
            match receiver.recv().unwrap() {
                PreviewEvent::Frame {
                    index: actual_index,
                    timestamp: actual_timestamp,
                    jpeg: actual_jpeg,
                } => {
                    assert_eq!(actual_index, index);
                    assert_eq!(actual_timestamp, *timestamp);
                    assert_eq!(actual_jpeg, *jpeg);
                }
                PreviewEvent::Finished(result) => panic!("cache finished early: {result:?}"),
            }
        }
        assert!(matches!(
            receiver.recv().unwrap(),
            PreviewEvent::Finished(Ok(()))
        ));
        worker.join().unwrap();
    }
}
