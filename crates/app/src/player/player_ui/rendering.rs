//! The rendering notifier owns GL resources until the backend tears down its context.
use super::startup::InitialOpen;
use crate::PlayerWindow;
use crate::background::CoalescedUiCallback;
use crate::platform::{SharedVideoExtras, video_materials::set_video_material_sample};
use crate::video_blur::VideoBlurSurface;
use riina_core::Command;
use riina_mpv::{Player, video::VideoSurface};
use slint::{ComponentHandle, GraphicsAPI, RenderingState, Weak};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

pub(super) fn install(
    app: &PlayerWindow,
    player: &Player,
    media: InitialOpen,
    volume: f64,
    extras: SharedVideoExtras,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut session = RenderSession {
        app: app.as_weak(),
        player: player.clone(),
        opening: Some(InitialPlayback { media, volume }),
        surface: Rc::default(),
        blur_surface: None,
        generation: Rc::new(Cell::new(0)),
        repaint: CoalescedUiCallback::default(),
        extras,
    };
    app.window()
        .set_rendering_notifier(move |state, api| match state {
            RenderingState::RenderingSetup => session.setup(api),
            RenderingState::BeforeRendering => session.draw(),
            RenderingState::AfterRendering => session.submitted(),
            RenderingState::RenderingTeardown => session.teardown(),
            _ => {}
        })?;
    Ok(())
}

struct InitialPlayback {
    media: InitialOpen,
    volume: f64,
}

struct RenderSession {
    app: Weak<PlayerWindow>,
    player: Player,
    opening: Option<InitialPlayback>,
    surface: Rc<RefCell<Option<VideoSurface>>>,
    blur_surface: Option<VideoBlurSurface>,
    generation: Rc<Cell<u64>>,
    repaint: CoalescedUiCallback,
    extras: SharedVideoExtras,
}

impl RenderSession {
    fn setup(&mut self, api: &GraphicsAPI<'_>) {
        self.generation.set(self.generation.get().wrapping_add(1));
        if let Some(app) = self.app.upgrade()
            && app.get_native_video()
        {
            // The D3D11 child already presents this player. An additional GL
            // context creates a second video feed during playback handoff.
            app.set_engine_ready(true);
            self.start_playback(&app);
            return;
        }
        let GraphicsAPI::NativeOpenGL { get_proc_address } = api else {
            return;
        };
        let weak = self.app.clone();
        let repaint = self.repaint.clone();
        // SAFETY: RenderingSetup has this window's GL context current.
        let video = unsafe {
            VideoSurface::new(
                self.player.clone(),
                get_proc_address,
                Box::new(move || {
                    repaint.post(|| {
                        let weak = weak.clone();
                        move || {
                            if let Some(app) = weak.upgrade() {
                                app.window().request_redraw();
                            }
                        }
                    });
                }),
            )
        };
        match video {
            Ok(video) => {
                // The video-backed material is optional and can fail independently.
                // SAFETY: RenderingSetup has this window's GL context current.
                match unsafe { VideoBlurSurface::new(get_proc_address) } {
                    Ok(blur) => self.blur_surface = Some(blur),
                    Err(error) => {
                        crate::diagnostics::warn!("Could not initialize video blur: {error}")
                    }
                }
                // SAFETY: rendering setup holds the GL context for all player surfaces.
                if let Err(error) = unsafe {
                    self.extras
                        .configure_rendering(get_proc_address, &self.surface)
                } && let Some(app) = self.app.upgrade()
                {
                    crate::diagnostics::report(
                        &app,
                        format!("PiP renderer initialization failed: {error}"),
                    );
                }
                self.surface.replace(Some(video));
                if let Some(app) = self.app.upgrade() {
                    app.set_engine_ready(true);
                }
            }
            Err(error) => {
                if let Some(app) = self.app.upgrade() {
                    crate::diagnostics::report(
                        &app,
                        format!("Video initialization failed: {error}"),
                    );
                }
            }
        }
    }

    fn draw(&mut self) {
        let mut surface = self.surface.borrow_mut();
        let (Some(video), Some(app)) = (surface.as_mut(), self.app.upgrade()) else {
            return;
        };
        self.extras.update_display_color(&app, video, &self.player);
        let scale = app.window().scale_factor();
        let width = (app.get_render_width() * scale).clamp(1.0, 8192.0) as u32;
        let height = (app.get_render_height() * scale).clamp(1.0, 8192.0) as u32;
        // SAFETY: BeforeRendering holds this surface's GL context.
        if let Err(error) = unsafe { video.render(width, height) } {
            crate::diagnostics::report(&app, format!("Video rendering failed: {error}"));
            return;
        }
        project_blur(&app, self.blur_surface.as_mut(), video, width, height);
        // SAFETY: the video surface owns this texture from the current
        // context, and Slint samples it in the same renderer.
        let image = unsafe {
            slint::BorrowedOpenGLTextureBuilder::new_gl_2d_rgba_texture(
                video.texture_id(),
                (width, height).into(),
            )
            .build()
        };
        app.global::<riina_ui::PlayerRenderedVideo>()
            .set_video_frame(image);
        // SAFETY: all presentation work uses this window's current GL context.
        if let Err(error) = unsafe { self.extras.render_pip(video) } {
            crate::diagnostics::report(&app, format!("PiP frame rendering failed: {error}"));
        }
        drop(surface);
        self.start_playback(&app);
    }

    fn submitted(&self) {
        if let Some(video) = self.surface.borrow().as_ref() {
            // Slint presents immediately after this notifier. Report at its
            // submission boundary so libmpv can pace the next frame.
            // SAFETY: the same renderer and context composed this frame.
            unsafe { video.report_swap() };
        }
    }

    fn teardown(&mut self) {
        // SAFETY: RenderingTeardown precedes release of this player's GL context.
        unsafe { self.extras.teardown_rendering() };
        drop(self.blur_surface.take());
        drop(self.surface.replace(None));
        self.extras.reset_display_color();
        // Wait for Slint to release its mutable window borrow. A subsequent
        // setup owns a new frame and invalidates this cleanup request.
        let retired_generation = self.generation.get();
        let generation = self.generation.clone();
        let weak = self.app.clone();
        slint::Timer::single_shot(Duration::ZERO, move || {
            if generation.get() == retired_generation
                && let Some(app) = weak.upgrade()
                && !app.get_native_video()
            {
                app.global::<riina_ui::PlayerRenderedVideo>()
                    .set_video_frame(slint::Image::default());
                set_video_material_sample(&app, slint::Image::default(), false);
                app.set_engine_ready(false);
            }
        });
    }

    fn start_playback(&mut self, app: &PlayerWindow) {
        let Some(opening) = self.opening.take() else {
            return;
        };
        if let Err(error) = self.player.send(Command::Volume(opening.volume)) {
            crate::diagnostics::warn!("Could not set the initial playback volume: {error}");
        }
        opening.media.open(app, self.player.command_sender());
    }
}

fn project_blur(
    app: &PlayerWindow,
    blur_surface: Option<&mut VideoBlurSurface>,
    video: &VideoSurface,
    width: u32,
    height: u32,
) {
    let appearance = app.global::<riina_ui::PlayerAppearance>();
    let sidebar_blur = appearance.get_liquid_glass_sidebar()
        && (app.global::<riina_ui::PlayerSidebarState>().get_sidebar() != 0
            || app
                .global::<riina_ui::PlayerSidebarState>()
                .get_leading_sidebar()
                != 0);
    let controller_blur = appearance.get_liquid_glass_osc()
        && app.get_loaded()
        && (app
            .global::<riina_ui::PlayerControlsState>()
            .get_controls_visible()
            || app
                .global::<riina_ui::PlayerMusicModeState>()
                .get_music_mode());
    let osd = app.global::<riina_ui::PlayerOsdState>();
    let osd_blur = appearance.get_liquid_glass_osd()
        && app.get_loaded()
        && (osd.get_video_osd_visible()
            || osd.get_seek_osd_visible()
            || !osd.get_plugin_osd_message().is_empty()
            || (app.get_fullscreen()
                && app
                    .global::<riina_ui::PlayerControlsState>()
                    .get_controls_visible()
                && app
                    .global::<riina_ui::PlayerPreferences>()
                    .get_display_time_and_battery_in_fullscreen()));
    let blur_enabled =
        appearance.get_system_materials_enabled() && (sidebar_blur || controller_blur || osd_blur);
    if blur_enabled {
        if let Some(blur) = blur_surface {
            // SAFETY: both textures belong to this window's current GL context.
            match unsafe { blur.render(glow::NativeTexture(video.texture_id()), width, height) } {
                Ok(()) => {
                    // SAFETY: the blur surface owns this texture from
                    // the current context, and it is used by this window.
                    let image = unsafe {
                        slint::BorrowedOpenGLTextureBuilder::new_gl_2d_rgba_texture(
                            blur.texture_id(),
                            (width.div_ceil(4), height.div_ceil(4)).into(),
                        )
                        .build()
                    };
                    set_video_material_sample(app, image, true);
                }
                Err(error) => crate::diagnostics::warn!("Could not render video blur: {error}"),
            }
        }
    } else {
        set_video_material_sample(app, slint::Image::default(), false);
    }
}
