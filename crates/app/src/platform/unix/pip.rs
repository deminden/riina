//! Present a Linux preview of the same decoder and restore its owning player.

mod context;
mod frames;
mod owner;

use self::{
    context::PlayerBinding,
    frames::{FrameKey, FrameTransfer},
    owner::{OwnerAction, OwnerState},
};
use crate::{PictureInPictureWindow, PlaybackStore, PlayerWindow};
use riina_core::{Command, Phase, Playback, music_mode::AudioStatus, pip_lifecycle, sync::lock};
use riina_mpv::{CommandSender, video::VideoSurface};
use slint::{ComponentHandle, Image, Timer, TimerMode};
use std::{
    cell::{Cell, RefCell},
    ffi::{CStr, c_void},
    rc::{Rc, Weak},
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};

const FRAME_TICK: Duration = Duration::from_millis(16);
const OWNER_TICK: Duration = Duration::from_millis(160);
const OPEN_TIMEOUT: Duration = Duration::from_secs(4);

pub(crate) struct PictureInPictureSession(Rc<LinuxPictureInPicture>);

impl PictureInPictureSession {
    pub(crate) fn new(app: &PlayerWindow) -> Self {
        Self(Rc::new(LinuxPictureInPicture {
            player: app.as_weak(),
            playback: RefCell::new(None),
            preview: RefCell::new(None),
            graphics: RefCell::new(None),
            frames: Timer::default(),
            owner_watch: Timer::default(),
            minimized: Cell::new(None),
            next_preview: Cell::new(0),
            closing: Cell::new(false),
        }))
    }

    pub(crate) fn bind(
        &self,
        app: &PlayerWindow,
        commands: &CommandSender,
        playback: &PlaybackStore,
    ) {
        self.0.playback.replace(Some(PlaybackLink {
            state: playback.clone(),
            commands: commands.clone(),
        }));
        let state = Rc::downgrade(&self.0);
        app.global::<riina_ui::PlayerMenuActions>()
            .on_toggle_picture_in_picture(move || {
                if let Some(state) = state.upgrade() {
                    state.toggle();
                }
            });
        let state = Rc::downgrade(&self.0);
        app.on_pip_owner_minimization_observation_changed(move |observe| {
            if let Some(state) = state.upgrade() {
                state.watch_owner(observe);
            }
        });
        self.0.watch_owner(app.get_observe_pip_owner_minimization());
    }

    pub(crate) fn render_extras(&self) -> crate::platform::SharedVideoExtras {
        crate::platform::SharedVideoExtras::with_linux_picture_in_picture(self.0.clone())
    }

    pub(crate) fn notification_flag(&self) -> Option<Arc<AtomicBool>> {
        None
    }

    pub(crate) fn prepare_to_close(&self) {
        self.0.closing.set(true);
        self.0.owner_watch.stop();
        self.0.close_preview();
    }
}

struct PlaybackLink {
    state: PlaybackStore,
    commands: CommandSender,
}

enum Presentation {
    Opening {
        deadline: Instant,
        from_minimize: bool,
    },
    Open(OwnerAction),
}

struct Preview {
    id: u64,
    window: PictureInPictureWindow,
    presentation: Presentation,
    displayed: Option<FrameKey>,
}

struct Graphics {
    capture: FrameTransfer,
    binding: Option<PlayerBinding>,
    video: Weak<RefCell<Option<VideoSurface>>>,
    newest: Option<(FrameKey, Image)>,
    last_foreground: Option<Instant>,
}

pub(crate) struct LinuxPictureInPicture {
    player: slint::Weak<PlayerWindow>,
    playback: RefCell<Option<PlaybackLink>>,
    preview: RefCell<Option<Preview>>,
    graphics: RefCell<Option<Graphics>>,
    frames: Timer,
    owner_watch: Timer,
    minimized: Cell<Option<bool>>,
    next_preview: Cell<u64>,
    closing: Cell<bool>,
}

impl LinuxPictureInPicture {
    fn toggle(self: &Rc<Self>) {
        if self.preview.borrow().is_some() {
            self.close_preview();
        } else if let Err(error) = self.open_preview(false) {
            self.report(error);
        }
    }

    fn open_preview(self: &Rc<Self>, from_minimize: bool) -> Result<(), String> {
        let Some(app) = self.player.upgrade().filter(|_| !self.closing.get()) else {
            return Ok(());
        };
        if self.preview.borrow().is_some()
            || app
                .global::<riina_ui::PlayerDialogState>()
                .get_dialog_open()
            || app.global::<riina_ui::PlayerCropState>().get_crop_editing()
        {
            return Ok(());
        }
        let playback = self.playback().ok_or("Playback is still starting")?;
        if dimensions(&playback).is_none() {
            return Err("Picture-in-Picture requires a loaded video".into());
        }
        if self.graphics.borrow().is_none() {
            return Err("Picture-in-Picture needs the player's OpenGL renderer".into());
        }
        let window = PictureInPictureWindow::new().map_err(|error| error.to_string())?;
        self.bind_controls(&window);
        let id = self.next_preview.get().wrapping_add(1);
        self.next_preview.set(id);
        self.preview.replace(Some(Preview {
            id,
            window,
            presentation: Presentation::Opening {
                deadline: Instant::now() + OPEN_TIMEOUT,
                from_minimize,
            },
            displayed: None,
        }));
        let weak = Rc::downgrade(self);
        self.frames.start(TimerMode::Repeated, FRAME_TICK, move || {
            if let Some(state) = weak.upgrade()
                && let Err(error) = state.frame_tick()
            {
                state.close_preview();
                state.report(error);
            }
        });
        app.window().request_redraw();
        Ok(())
    }

    fn bind_controls(self: &Rc<Self>, window: &PictureInPictureWindow) {
        let weak = Rc::downgrade(self);
        window.on_play_pause(move || {
            if let Some(state) = weak.upgrade()
                && let Some(playback) = state.playback()
            {
                state.command(Command::SetPaused(!playback.paused));
            }
        });
        let weak = Rc::downgrade(self);
        window.on_stop(move || {
            if let Some(state) = weak.upgrade() {
                state.command(Command::SetPaused(true));
            }
        });
        let weak = Rc::downgrade(self);
        window.on_skip(move |seconds| {
            if let Some(state) = weak.upgrade() {
                state.command(Command::Seek {
                    seconds: f64::from(seconds),
                    absolute: false,
                });
            }
        });
        let weak = Rc::downgrade(self);
        window.on_exit_picture_in_picture(move || {
            if let Some(state) = weak.upgrade() {
                state.close_preview();
            }
        });
        let weak = Rc::downgrade(self);
        window.window().on_close_requested(move || {
            let weak = weak.clone();
            // Release the preview outside Slint's native close callback.
            Timer::single_shot(Duration::ZERO, move || {
                if let Some(state) = weak.upgrade() {
                    state.close_preview();
                }
            });
            slint::CloseRequestResponse::KeepWindowShown
        });
    }

    fn frame_tick(&self) -> Result<(), String> {
        let Some(app) = self.player.upgrade() else {
            self.close_preview();
            return Ok(());
        };
        let Some(playback) = self.playback() else {
            return Ok(());
        };
        if matches!(playback.phase, Phase::Stopped | Phase::Failed) {
            self.close_preview();
            return Ok(());
        }
        let mut preview = self.preview.borrow_mut();
        let Some(preview) = preview.as_mut() else {
            return Ok(());
        };
        if matches!(preview.presentation, Presentation::Opening { deadline, .. } if Instant::now() >= deadline)
        {
            return Err("Picture-in-Picture did not receive a video frame".into());
        }
        preview.window.set_media_title(app.get_media_title());
        preview.window.set_paused(playback.paused);
        let key = frame_key(preview.id, &playback);
        if preview
            .displayed
            .is_some_and(|previous| previous.media != key.media)
        {
            preview.window.set_video_frame(Image::default());
            preview.displayed = None;
        }
        let (frame, can_draw_offscreen) = {
            let mut graphics = self.graphics.borrow_mut();
            let Some(graphics) = graphics.as_mut() else {
                return Ok(());
            };
            if graphics
                .last_foreground
                .is_none_or(|last| last.elapsed() >= FRAME_TICK * 3)
            {
                graphics.draw_offscreen(SampleRequest::new(preview, &playback))?;
            }
            (graphics.newest.take(), graphics.binding.is_some())
        };
        let Some((revision, image)) = frame.filter(|(revision, _)| *revision == key) else {
            if preview.displayed != Some(key) {
                app.window().request_redraw();
            }
            return Ok(());
        };
        preview.window.set_video_frame(image);
        preview.displayed = Some(revision);
        if matches!(preview.presentation, Presentation::Opening { .. }) {
            self.show_preview(&app, preview, can_draw_offscreen)?;
        }
        Ok(())
    }

    fn show_preview(
        &self,
        app: &PlayerWindow,
        preview: &mut Preview,
        can_draw_offscreen: bool,
    ) -> Result<(), String> {
        let settings = crate::pip_preferences::current_player(app)?;
        let owner = OwnerState::read(app);
        let (_, entry) = pip_lifecycle::Session::enter(
            settings,
            pip_lifecycle::MainWindowState {
                fullscreen: app.get_fullscreen() || app.get_entering_fullscreen(),
                minimized: owner.minimized.unwrap_or(false),
            },
        );
        owner::show_preview(&preview.window)?;
        let restore = owner.apply(app, entry.window_behavior, can_draw_offscreen);
        preview.presentation = Presentation::Open(restore);
        if entry.pause_playback {
            self.command(Command::SetPaused(true));
        }
        crate::player::presentation::set_picture_in_picture(app, true);
        Ok(())
    }

    fn close_preview(&self) {
        self.frames.stop();
        let Some(preview) = self.preview.borrow_mut().take() else {
            return;
        };
        if let Presentation::Open(action) = preview.presentation
            && let Some(player) = self.player.upgrade()
        {
            if !self.closing.get() {
                action.restore(&player);
            }
            crate::player::presentation::set_picture_in_picture(&player, false);
        }
        if let Err(error) = preview.window.hide() {
            self.report(error.to_string());
        }
    }

    fn playback(&self) -> Option<Arc<Playback>> {
        self.playback
            .borrow()
            .as_ref()
            .map(|link| lock(&link.state).clone())
    }

    fn command(&self, command: Command) {
        if let Some(link) = self.playback.borrow().as_ref()
            && let Err(error) = link.commands.send(command)
        {
            self.report(format!(
                "Could not control Picture-in-Picture playback: {error}"
            ));
        }
    }

    fn report(&self, error: String) {
        if let Some(app) = self.player.upgrade() {
            crate::diagnostics::report(&app, error);
        }
    }
}

impl LinuxPictureInPicture {
    fn watch_owner(self: &Rc<Self>, observe: bool) {
        self.owner_watch.stop();
        self.minimized.set(None);
        if !observe || self.closing.get() {
            return;
        }
        let weak = Rc::downgrade(self);
        self.owner_watch
            .start(TimerMode::Repeated, OWNER_TICK, move || {
                if let Some(state) = weak.upgrade() {
                    state.observe_owner();
                }
            });
    }

    fn observe_owner(self: &Rc<Self>) {
        let Some(app) = self.player.upgrade() else {
            return;
        };
        let owner = OwnerState::read(&app);
        if owner.wayland {
            // Stable xdg-shell exposes no acknowledged minimized state.
            self.owner_watch.stop();
            return;
        }
        let Some(minimized) = owner.minimized else {
            return;
        };
        if self
            .minimized
            .replace(Some(minimized))
            .is_none_or(|prior| prior == minimized)
        {
            return;
        }
        let phase = {
            let preview = self.preview.borrow();
            match preview.as_ref().map(|preview| &preview.presentation) {
                Some(Presentation::Open(OwnerAction::Minimized)) => return,
                Some(Presentation::Open(_)) => pip_lifecycle::Phase::Active,
                Some(Presentation::Opening {
                    from_minimize: true,
                    ..
                }) if !minimized => {
                    drop(preview);
                    self.close_preview();
                    return;
                }
                Some(Presentation::Opening { .. }) => pip_lifecycle::Phase::Entering,
                None => pip_lifecycle::Phase::Off,
            }
        };
        let Ok(settings) = crate::pip_preferences::current_player(&app) else {
            return;
        };
        let is_video = self
            .playback()
            .is_some_and(|state| state.audio_status() == AudioStatus::Video);
        match pip_lifecycle::minimize_toggle_target(settings, is_video, minimized, phase) {
            Some(true) => {
                if let Err(error) = self.open_preview(true) {
                    self.report(error);
                }
            }
            Some(false) => self.close_preview(),
            None => {}
        }
    }

    /// # Safety
    /// The player's Slint renderer holds its GL context current.
    pub(crate) unsafe fn configure_rendering(
        &self,
        loader: &dyn Fn(&CStr) -> *const c_void,
        video: Weak<RefCell<Option<VideoSurface>>>,
    ) -> Result<(), String> {
        // SAFETY: the caller retains the context that supplies this loader.
        let capture = unsafe { FrameTransfer::new(loader)? };
        self.graphics.replace(Some(Graphics {
            capture,
            binding: PlayerBinding::capture(),
            video,
            newest: None,
            last_foreground: None,
        }));
        Ok(())
    }

    /// # Safety
    /// The source context must stay current through this draw.
    pub(crate) unsafe fn render_frame(&self, video: &mut VideoSurface) -> Result<(), String> {
        let preview = self.preview.borrow();
        let (Some(preview), Some(playback)) = (preview.as_ref(), self.playback()) else {
            return Ok(());
        };
        let mut graphics = self.graphics.borrow_mut();
        let Some(graphics) = graphics.as_mut() else {
            return Ok(());
        };
        graphics.last_foreground = Some(Instant::now());
        // SAFETY: the rendering hook holds the source context current.
        unsafe { graphics.step(video, SampleRequest::new(preview, &playback)) }.map(|_| ())
    }

    /// # Safety
    /// Called before the player's context disappears, with that context current.
    pub(crate) unsafe fn teardown_rendering(&self) {
        self.graphics.replace(None);
    }
}

struct SampleRequest {
    key: FrameKey,
    dimensions: Option<(u32, u32)>,
    submit: bool,
}

impl SampleRequest {
    fn new(preview: &Preview, playback: &Playback) -> Self {
        let key = frame_key(preview.id, playback);
        Self {
            key,
            dimensions: dimensions(playback),
            submit: !playback.seeking
                && !playback.buffering
                && (!playback.paused || preview.displayed != Some(key)),
        }
    }
}

impl Graphics {
    fn draw_offscreen(&mut self, request: SampleRequest) -> Result<(), String> {
        let (Some(binding), Some(video)) = (&self.binding, self.video.upgrade()) else {
            return Ok(());
        };
        let Ok(mut video) = video.try_borrow_mut() else {
            return Ok(());
        };
        let Some(video) = video.as_mut() else {
            return Ok(());
        };
        // SAFETY: the weak source upgraded and remains borrowed on the renderer's UI thread.
        unsafe {
            binding.draw(|| {
                let submitted =
                    Self::transfer(&mut self.capture, &mut self.newest, video, &request)?;
                if submitted {
                    video.report_swap();
                }
                Ok(())
            })
        }
    }

    unsafe fn step(
        &mut self,
        video: &mut VideoSurface,
        request: SampleRequest,
    ) -> Result<bool, String> {
        // SAFETY: the rendering hook keeps the creating context current.
        unsafe { Self::transfer(&mut self.capture, &mut self.newest, video, &request) }
    }

    unsafe fn transfer(
        capture: &mut FrameTransfer,
        newest: &mut Option<(FrameKey, Image)>,
        video: &mut VideoSurface,
        request: &SampleRequest,
    ) -> Result<bool, String> {
        // SAFETY: the native framebuffer and each pending fence belong to the current renderer.
        if let Some(frame) = unsafe { capture.collect()? }
            && frame.0 == request.key
        {
            *newest = Some(frame);
        }
        if request.submit
            && let Some(dimensions) = request.dimensions
        {
            // SAFETY: capture owns the complete target in the same context as video.
            return unsafe { capture.submit(video, request.key, dimensions) };
        }
        Ok(false)
    }
}

fn frame_key(preview: u64, playback: &Playback) -> FrameKey {
    FrameKey {
        preview,
        media: playback.generation,
        seek: playback.completed_seeks,
    }
}

fn dimensions(playback: &Playback) -> Option<(u32, u32)> {
    if matches!(
        playback.phase,
        Phase::Loading | Phase::Stopped | Phase::Failed
    ) {
        return None;
    }
    let video = playback.display_video_parameters()?;
    let (mut width, mut height) = (video.dw.unwrap_or(video.w), video.dh.unwrap_or(video.h));
    if matches!(video.rotate.unwrap_or(0).rem_euclid(360), 90 | 270) {
        std::mem::swap(&mut width, &mut height);
    }
    (width > 0 && height > 0).then_some((width, height))
}
