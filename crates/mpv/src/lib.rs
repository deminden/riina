//! The event worker owns the player; the GUI owns only its OpenGL render context.
mod ffi;
pub mod video;

use ffi::{Api, EndFile, Property};
use riina_core::{Command, Phase, Playback};
use std::{
    collections::HashMap,
    ffi::{CStr, CString, c_void},
    path::Path,
    rc::Rc,
    sync::mpsc::{self, SyncSender},
    thread::JoinHandle,
};

enum Message {
    Intent(Command),
    Wake,
}

struct Owner {
    sender: SyncSender<Message>,
    thread: Option<JoinHandle<()>>,
}

impl Drop for Owner {
    fn drop(&mut self) {
        // Renderers retain the owner, so this runs only after render contexts are freed.
        let _ = self.sender.send(Message::Intent(Command::Shutdown));
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[derive(Clone)]
pub struct Player {
    owner: Rc<Owner>,
    api: Api,
    handle: usize,
}

#[derive(Clone)]
pub struct CommandSender(SyncSender<Message>);

impl CommandSender {
    pub fn send(&self, command: Command) -> Result<(), String> {
        self.0
            .try_send(Message::Intent(command))
            .map_err(|e| format!("Player command was not accepted: {e}"))
    }
}

unsafe extern "C" fn wakeup(context: *mut c_void) {
    // SAFETY: worker retains this boxed sender until the callback is unregistered.
    let sender = unsafe { &*(context.cast::<SyncSender<Message>>()) };
    // Wakeups coalesce: every intent also causes the worker to drain the entire event queue.
    let _ = sender.try_send(Message::Wake);
}

impl Player {
    pub fn start(
        path: &Path,
        headless: bool,
        notify: impl Fn(Playback) + Send + 'static,
    ) -> Result<Self, String> {
        Self::start_options(path, headless, Vec::new(), notify)
    }

    /// Experimental native VO. The caller must retain the HWND until Player drops.
    /// mpv creates its child video window; no render API context may use this player.
    #[cfg(target_os = "windows")]
    pub fn start_native_window(
        path: &Path,
        window: u32,
        log: &Path,
        notify: impl Fn(Playback) + Send + 'static,
    ) -> Result<Self, String> {
        if window == 0 || !log.is_absolute() {
            return Err("Native output requires a live HWND and absolute log path".into());
        }
        Self::start_options(
            path,
            false,
            vec![
                ("vo".into(), "gpu-next".into()),
                ("wid".into(), window.to_string()),
                ("gpu-api".into(), "d3d11".into()),
                ("gpu-context".into(), "d3d11".into()),
                ("d3d11-output-format".into(), "rgba16f".into()),
                ("target-colorspace-hint".into(), "auto".into()),
                ("log-file".into(), log.to_string_lossy().into_owned()),
                ("msg-level".into(), "all=v".into()),
            ],
            notify,
        )
    }

    fn start_options(
        path: &Path,
        headless: bool,
        options: Vec<(String, String)>,
        notify: impl Fn(Playback) + Send + 'static,
    ) -> Result<Self, String> {
        let api = Api::load(path)?;
        let worker_api = api.clone();
        let (sender, receiver) = mpsc::sync_channel(64);
        let callback_sender = sender.clone();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let thread = std::thread::Builder::new().name("mpv-events".into()).spawn(move || {
            // SAFETY: all ordinary client calls stay on this worker. Only the render API
            // sees the handle on the GUI thread, as permitted by render.h's threading contract.
            unsafe {
                let handle = (worker_api.create)();
                if handle.is_null() { let _ = ready_tx.send(Err("mpv_create failed".into())); return; }
                let initialized = (|| -> Result<(), String> {
                    for (key, value) in [("config", "no"), ("load-scripts", "no"), ("idle", "yes"),
                        ("vo", if headless { "null" } else { "libmpv" }),
                        ("volume-max", "100"), ("terminal", "no"), ("hwdec", "auto-safe"), ("keep-open", "yes"),
                        // Loop points belong to the current media, unlike volume
                        // and speed. Reset in mpv too, not just in our UI snapshot.
                        ("reset-on-next-file", "ab-loop-a,ab-loop-b")] {
                        let key = CString::new(key).unwrap();
                        let value = CString::new(value).unwrap();
                        worker_api.check((worker_api.option)(handle, key.as_ptr(), value.as_ptr()))?;
                    }
                    for (key, value) in &options {
                        let key = CString::new(key.as_str()).map_err(|e| e.to_string())?;
                        let value = CString::new(value.as_str()).map_err(|e| e.to_string())?;
                        worker_api.check((worker_api.option)(handle, key.as_ptr(), value.as_ptr()))?;
                    }
                    worker_api.check((worker_api.initialize)(handle))?;
                    for (name, format) in [("time-pos", 5), ("duration", 5), ("pause", 3),
                        ("paused-for-cache", 3), ("seeking", 3), ("seekable", 3),
                        ("volume", 5), ("mute", 3), ("speed", 5), ("media-title", 1), ("eof-reached", 3),
                        ("current-ao", 1), ("hwdec-current", 1),
                        ("track-list", 6), ("chapter-list", 6), ("playlist", 6), ("video-params", 6), ("video-target-params", 6),
                        ("audio-delay", 5), ("sub-delay", 5), ("sub-visibility", 3),
                        ("sub-text", 1), ("loop-file", 1), ("loop-playlist", 1),
                        ("ab-loop-a", 6), ("ab-loop-b", 6)] {
                        let name = CString::new(name).unwrap();
                        worker_api.check((worker_api.observe)(handle, 0, name.as_ptr(), format))?;
                    }
                    Ok(())
                })();
                if let Err(error) = initialized {
                    let _ = ready_tx.send(Err(error));
                    (worker_api.destroy)(handle);
                    return;
                }
                let mut callback = Box::new(callback_sender);
                (worker_api.wakeup)(handle, Some(wakeup), (&mut *callback as *mut SyncSender<Message>).cast());
                // The numeric token is opaque outside video.rs. No Rust object is given a
                // fabricated Send implementation; the worker owns and destroys this handle.
                let _ = ready_tx.send(Ok(handle as usize));
                let mut state = Playback { volume: 100.0, speed: 1.0, ..Default::default() };
                let mut next_id = 0u64;
                let mut pending = HashMap::new();
                let mut resume_on_load = false;
                let mut seek_pending = false;
                while let Ok(message) = receiver.recv() {
                    if let Message::Intent(command) = message {
                        if command == Command::Shutdown { break; }
                        match command.plan(&state) {
                            Ok(plan) => {
                                if matches!(command, Command::Load(_) | Command::OpenFiles(_) | Command::Stop) { resume_on_load = false; }
                                resume_on_load |= plan.resume_on_load;
                                // Validate the whole intent before submitting any part. Multi-file
                                // additions consume one queue slot and cannot be partially dropped.
                                for args in plan.commands {
                                    if let Err(error) = issue(&worker_api, handle, &args, &mut next_id, &mut pending, state.generation) {
                                        state.error = Some(error);
                                        resume_on_load = false;
                                        break;
                                    }
                                }
                            }
                            Err(error) => state.error = Some(error),
                        }
                    }
                    loop {
                        let event = &*(worker_api.wait)(handle, 0.0);
                        if event.id == 0 { break; }
                        match event.id {
                            5 => {
                                if let Some(generation) = pending.remove(&event.userdata)
                                    && generation == state.generation && event.error < 0 {
                                    state.error = worker_api.check(event.error).err();
                                    resume_on_load = false;
                                }
                            }
                            6 => { seek_pending = false; state.begin_load(); },
                            7 if !event.data.is_null() => {
                                let end = &*event.data.cast::<EndFile>();
                                seek_pending = false;
                                state.phase = match end.reason { 0 => Phase::Ended, 4 => Phase::Failed, _ => Phase::Stopped };
                                if matches!(state.phase, Phase::Stopped | Phase::Failed) {
                                    state.ab_loop_a = None;
                                    state.ab_loop_b = None;
                                }
                                if end.error < 0 { state.error = worker_api.check(end.error).err(); }
                                if end.reason == 4 { resume_on_load = false; }
                            }
                            8 => {
                                state.phase = Phase::Playing;
                                if resume_on_load {
                                    resume_on_load = false;
                                    let args = ["set".into(), "pause".into(), "no".into()];
                                    if let Err(error) = issue(&worker_api, handle, &args, &mut next_id, &mut pending, state.generation) { state.error = Some(error); }
                                }
                            },
                            20 => seek_pending = true,
                            21 if seek_pending => {
                                // Restart also occurs on initial load. Only a preceding
                                // native SEEK establishes a completed seek notification.
                                seek_pending = false;
                                state.completed_seeks = state.completed_seeks.wrapping_add(1);
                            }
                            22 if !event.data.is_null() => {
                                let property = &*event.data.cast::<Property>();
                                apply_property(&mut state, property);
                            }
                            24 => { state.error = Some("The libmpv event queue overflowed; playback state may be incomplete".into()); }
                            _ => {}
                        }
                    }
                    state.reconcile();
                    notify(state.clone());
                }
                (worker_api.wakeup)(handle, None, std::ptr::null_mut());
                drop(callback);
                (worker_api.destroy)(handle);
            }
        }).map_err(|e| e.to_string())?;
        let handle = ready_rx.recv().map_err(|e| e.to_string())??;
        Ok(Self {
            owner: Rc::new(Owner {
                sender,
                thread: Some(thread),
            }),
            api,
            handle,
        })
    }

    pub fn send(&self, command: Command) -> Result<(), String> {
        self.owner
            .sender
            .try_send(Message::Intent(command))
            .map_err(|e| format!("Player command was not accepted: {e}"))
    }

    pub fn command_sender(&self) -> CommandSender {
        CommandSender(self.owner.sender.clone())
    }
}

unsafe fn issue(
    api: &Api,
    handle: *mut c_void,
    args: &[String],
    next_id: &mut u64,
    pending: &mut HashMap<u64, u64>,
    generation: u64,
) -> Result<(), String> {
    let args: Vec<_> = args
        .iter()
        .map(|arg| CString::new(arg.as_str()))
        .collect::<Result<_, _>>()
        .map_err(|error| error.to_string())?;
    let mut pointers: Vec<_> = args.iter().map(|arg| arg.as_ptr()).collect();
    pointers.push(std::ptr::null());
    *next_id += 1;
    // SAFETY: worker owns the live handle; command_async copies these strings
    // before returning, as specified by client.h. Reply ownership stays with mpv.
    api.check(unsafe { (api.command)(handle, *next_id, pointers.as_ptr()) })?;
    pending.insert(*next_id, generation);
    Ok(())
}

unsafe fn apply_property(state: &mut Playback, property: &Property) {
    // SAFETY: caller processes the event before asking mpv for another one. Format
    // tags are checked before dereferencing; unavailable properties remain unknown.
    let name = unsafe { CStr::from_ptr(property.name) }.to_bytes();
    if property.data.is_null() {
        match name {
            b"time-pos" => state.position = None,
            b"duration" => state.duration = None,
            b"track-list" => state.tracks.clear(),
            b"chapter-list" => state.chapters.clear(),
            b"playlist" => state.playlist.clear(),
            b"sub-text" => state.subtitle_text = None,
            b"video-params" => state.video = None,
            b"video-target-params" => state.video_target = None,
            b"hwdec-current" => state.hardware_decoder = None,
            b"current-ao" => state.audio_output = None,
            b"loop-file" => state.loop_file = None,
            b"loop-playlist" => state.loop_playlist = None,
            b"ab-loop-a" => state.ab_loop_a = None,
            b"ab-loop-b" => state.ab_loop_b = None,
            _ => {}
        }
        return;
    }
    if property.format == 6 {
        let node = unsafe { &*property.data.cast::<ffi::Node>() };
        let result = unsafe { node.copy_json() }.and_then(|value| {
            match name {
                b"ab-loop-a" | b"ab-loop-b" => {
                    // Native values are either a numeric timestamp or "no".
                    // Unknown/unavailable points must not become a zero marker.
                    let point = value
                        .as_f64()
                        .filter(|point| point.is_finite() && *point >= 0.0);
                    if name == b"ab-loop-a" {
                        state.ab_loop_a = point;
                    } else {
                        state.ab_loop_b = point;
                    }
                }
                b"track-list" => {
                    state.tracks = serde_json::from_value(value).map_err(|e| e.to_string())?
                }
                b"chapter-list" => {
                    state.chapters = serde_json::from_value(value).map_err(|e| e.to_string())?
                }
                b"playlist" => {
                    state.playlist = serde_json::from_value(value).map_err(|e| e.to_string())?
                }
                b"video-params" | b"video-target-params" => {
                    // The native VO target describes presentation, while source
                    // parameters can still carry a PQ/HLG transfer function.
                    let destination = if name == b"video-params" {
                        &mut state.video
                    } else {
                        &mut state.video_target
                    };
                    *destination =
                        if value.is_null() || value.as_object().is_some_and(|map| map.is_empty()) {
                            None
                        } else {
                            Some(serde_json::from_value(value).map_err(|e| e.to_string())?)
                        };
                }
                _ => {}
            }
            Ok(())
        });
        if let Err(error) = result {
            state.error = Some(format!("Cannot decode mpv property: {error}"));
        }
    } else if property.format == 5 {
        let value = unsafe { *property.data.cast::<f64>() };
        if !value.is_finite() {
            return;
        }
        match name {
            b"time-pos" => state.position = (value >= 0.0).then_some(value),
            b"duration" => state.duration = (value >= 0.0).then_some(value),
            b"volume" => state.volume = value,
            b"speed" => state.speed = value,
            b"audio-delay" => state.audio_delay = value,
            b"sub-delay" => state.subtitle_delay = value,
            _ => {}
        }
    } else if property.format == 3 {
        let flag = unsafe { *property.data.cast::<i32>() } != 0;
        match name {
            b"pause" => state.paused = flag,
            b"paused-for-cache" => state.buffering = flag,
            b"seeking" => state.seeking = flag,
            b"seekable" => state.seekable = flag,
            b"mute" => state.muted = flag,
            b"sub-visibility" => state.subtitle_visible = flag,
            b"eof-reached" => {
                if flag {
                    state.phase = Phase::Ended;
                } else if state.phase == Phase::Ended {
                    state.phase = Phase::Playing;
                }
            }
            _ => {}
        }
    } else if property.format == 1 {
        let text = unsafe { *property.data.cast::<*const std::ffi::c_char>() };
        if !text.is_null() {
            let value = unsafe { CStr::from_ptr(text) }
                .to_string_lossy()
                .into_owned();
            match name {
                b"media-title" => state.title = value,
                b"current-ao" => state.audio_output = Some(value),
                b"hwdec-current" => state.hardware_decoder = Some(value),
                b"sub-text" => state.subtitle_text = Some(value),
                b"loop-file" => state.loop_file = Some(value),
                b"loop-playlist" => state.loop_playlist = Some(value),
                _ => {}
            }
        }
    }
}
