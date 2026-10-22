//! The rendering notifier owns GL resources until the backend tears down its context.
use super::materials::set_video_material_sample;
#[cfg(target_os = "macos")]
use crate::platform;
use crate::{PlayerWindow, media_open};
use riina_core::Command;
use riina_mpv::{Player, video::VideoSurface};
use slint::ComponentHandle;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

pub(super) fn install(
    app: &PlayerWindow,
    player: &Player,
    media: Vec<std::ffi::OsString>,
    startup_resume_position: Option<f64>,
    volume: f64,
    #[cfg(target_os = "macos")] picture_in_picture: Rc<
        RefCell<Option<platform::MacPictureInPicture>>,
    >,
) -> Result<(), Box<dyn std::error::Error>> {
    let pending_media = Rc::new(RefCell::new(media));
    let pending_resume_position = Rc::new(Cell::new(startup_resume_position));
    let render_weak = app.as_weak();
    let render_player = player.clone();
    let render_resume_position = pending_resume_position.clone();
    let mut surface: Option<VideoSurface> = None;
    #[cfg(target_os = "macos")]
    let mut icc_profile_display: Option<(usize, Option<Vec<u8>>, bool)> = None;
    #[cfg(target_os = "macos")]
    let mut icc_profile_auto_active: Option<bool> = None;
    let mut blur_surface: Option<crate::video_blur::VideoBlurSurface> = None;
    let render_generation = Rc::new(Cell::new(0_u64));
    let mut started = false;
    let repaint_pending = Arc::new(AtomicBool::new(false));
    #[cfg(target_os = "macos")]
    let render_picture_in_picture = picture_in_picture;
    app.window()
        .set_rendering_notifier(move |state, graphics_api| {
            match state {
                slint::RenderingState::RenderingSetup => {
                    render_generation.set(render_generation.get().wrapping_add(1));
                    if let Some(app) = render_weak.upgrade()
                        && app.get_native_video()
                    {
                        // Windows native playback is already presented by mpv's
                        // D3D11 child window. Creating libmpv's OpenGL render API
                        // context for that same player produces a second video
                        // feed above it and leaves doubled frames during handoff.
                        app.set_engine_ready(true);
                        if !started {
                            started = true;
                            let _ = render_player.send(Command::Volume(volume));
                            let paths = std::mem::take(&mut *pending_media.borrow_mut());
                            if !paths.is_empty() {
                                media_open::startup_at(
                                    &app,
                                    paths,
                                    render_resume_position.take(),
                                    render_player.command_sender(),
                                );
                            }
                        }
                    } else if let slint::GraphicsAPI::NativeOpenGL { get_proc_address } =
                        graphics_api
                    {
                        let weak = render_weak.clone();
                        let pending = repaint_pending.clone();
                        // SAFETY: notifier guarantees that this GL context is current.
                        match unsafe {
                            VideoSurface::new(
                                render_player.clone(),
                                &get_proc_address,
                                Box::new(move || {
                                    if pending.swap(true, Ordering::AcqRel) {
                                        return;
                                    }
                                    let weak = weak.clone();
                                    let pending = pending.clone();
                                    let _ = slint::invoke_from_event_loop(move || {
                                        pending.store(false, Ordering::Release);
                                        if let Some(app) = weak.upgrade() {
                                            app.window().request_redraw();
                                        }
                                    });
                                }),
                            )
                        } {
                            Ok(video) => {
                                // The video-backed material is an optional visual enhancement:
                                // failure to create it must not prevent playback.
                                match unsafe {
                                    crate::video_blur::VideoBlurSurface::new(&get_proc_address)
                                } {
                                    Ok(blur) => blur_surface = Some(blur),
                                    Err(error) => eprintln!("Could not initialize video blur: {error}"),
                                }
                                #[cfg(target_os = "macos")]
                                if let Some(manager) =
                                    render_picture_in_picture.borrow_mut().as_mut()
                                    && let Err(error) =
                                        unsafe { manager.configure_rendering(&get_proc_address) }
                                    && let Some(app) = render_weak.upgrade()
                                {
                                    app.set_status(
                                        format!("PiP renderer initialization failed: {error}")
                                            .into(),
                                    );
                                }
                                surface = Some(video);
                                if let Some(app) = render_weak.upgrade() {
                                    app.set_engine_ready(true);
                                }
                            }
                            Err(error) => {
                                if let Some(app) = render_weak.upgrade() {
                                    app.set_status(
                                        format!("Video initialization failed: {error}").into(),
                                    );
                                }
                            }
                        }
                    }
                }
                slint::RenderingState::BeforeRendering => {
                    if let (Some(video), Some(app)) = (surface.as_mut(), render_weak.upgrade()) {
                        #[cfg(target_os = "macos")]
                        if let Some(display) = platform::display_identity(app.window()) {
                            if !app.get_load_icc_profile()
                                && let Some((cached_display, profile, _)) =
                                    icc_profile_display.take()
                            {
                                icc_profile_display =
                                    Some((cached_display, profile, false));
                            }
                            let needs_profile = icc_profile_display
                                .as_ref()
                                .is_none_or(|(cached_display, _, attempted)| {
                                    *cached_display != display
                                        || (app.get_load_icc_profile() && !attempted)
                                });
                            if needs_profile {
                                let attempted = app.get_load_icc_profile();
                                let mut profile = attempted
                                    .then(|| platform::icc_profile_for_window(app.window()))
                                    .flatten();
                                if let Some(bytes) = profile.as_deref() {
                                    // SAFETY: this callback runs on Slint's current GL render context.
                                    if let Err(error) = unsafe { video.set_icc_profile(bytes) } {
                                        eprintln!("Could not install the macOS display ICC profile: {error}");
                                        profile = None;
                                    }
                                }
                                icc_profile_display = Some((display, profile, attempted));
                            }
                            let profile_available = icc_profile_display
                                .as_ref()
                                .is_some_and(|(_, profile, _)| profile.is_some());
                            let hdr_output_active = app.get_hdr_source_detected()
                                && app.get_hdr_enabled()
                                && app.get_hdr_display_active();
                            let should_use_icc = app.get_load_icc_profile()
                                && profile_available
                                && !hdr_output_active;
                            if icc_profile_auto_active != Some(should_use_icc) {
                                if let Err(error) = render_player
                                    .send(Command::SetIccProfileAuto(should_use_icc))
                                {
                                    eprintln!("Could not update macOS display ICC processing: {error}");
                                } else {
                                    icc_profile_auto_active = Some(should_use_icc);
                                }
                            }
                        }
                        let scale = app.window().scale_factor();
                        let width = (app.get_render_width() * scale).clamp(1.0, 8192.0) as u32;
                        let height = (app.get_render_height() * scale).clamp(1.0, 8192.0) as u32;
                        // SAFETY: the notifier uses the same context throughout its lifetime.
                        match unsafe { video.render(width, height) } {
                            Ok(()) => {
                                let sidebar_blur = app.get_liquid_glass_sidebar()
                                    && (app.get_sidebar() != 0 || app.get_leading_sidebar() != 0);
                                let controller_blur = app.get_liquid_glass_osc()
                                    && app.get_loaded()
                                    && (app.get_controls_visible() || app.get_music_mode());
                                let osd_blur = app.get_liquid_glass_osd()
                                    && app.get_loaded()
                                    && (app.get_video_osd_visible()
                                        || app.get_seek_osd_visible()
                                        || !app.get_plugin_osd_message().is_empty()
                                        || (app.get_fullscreen()
                                            && app.get_controls_visible()
                                            && app.get_display_time_and_battery_in_fullscreen()));
                                let blur_enabled = app.get_system_materials_enabled()
                                    && (sidebar_blur || controller_blur || osd_blur);
                                if blur_enabled {
                                    if let Some(blur) = blur_surface.as_mut() {
                                        // SAFETY: both textures belong to this window's current GL context.
                                        match unsafe {
                                            blur.render(
                                                glow::NativeTexture(video.texture_id()),
                                                width,
                                                height,
                                            )
                                        } {
                                            Ok(()) => {
                                                let image = unsafe {
                                                    slint::BorrowedOpenGLTextureBuilder::new_gl_2d_rgba_texture(
                                                        blur.texture_id(),
                                                        (width.div_ceil(4), height.div_ceil(4)).into(),
                                                    )
                                                    .build()
                                                };
                                                set_video_material_sample(&app, image, true);
                                            }
                                            Err(error) => eprintln!("Could not render video blur: {error}"),
                                        }
                                    }
                                } else {
                                    set_video_material_sample(&app, slint::Image::default(), false);
                                }
                                let image = unsafe {
                                    slint::BorrowedOpenGLTextureBuilder::new_gl_2d_rgba_texture(
                                        video.texture_id(),
                                        (width, height).into(),
                                    )
                                    .build()
                                };
                                app.set_video_frame(image);
                                #[cfg(target_os = "macos")]
                                if let Some(manager) =
                                    render_picture_in_picture.borrow_mut().as_mut()
                                    && let Err(error) = unsafe { manager.render_frame(video) }
                                {
                                    app.set_status(
                                        format!("PiP frame rendering failed: {error}").into(),
                                    );
                                }
                                if !started {
                                    started = true;
                                    let _ = render_player.send(Command::Volume(volume));
                                    let paths = std::mem::take(&mut *pending_media.borrow_mut());
                                    if !paths.is_empty() {
                                        media_open::startup_at(
                                            &app,
                                            paths,
                                            render_resume_position.take(),
                                            render_player.command_sender(),
                                        );
                                    }
                                }
                            }
                            Err(error) => {
                                app.set_status(format!("Video rendering failed: {error}").into())
                            }
                        }
                    }
                }
                slint::RenderingState::AfterRendering => {
                    if let Some(video) = surface.as_ref() {
                        // Slint presents immediately after this notifier; report at its
                        // submission boundary so libmpv can pace the next video frame.
                        // SAFETY: same live renderer and context as the composed frame.
                        unsafe { video.report_swap() };
                    }
                }
                slint::RenderingState::RenderingTeardown => {
                    #[cfg(target_os = "macos")]
                    if let Some(manager) = render_picture_in_picture.borrow_mut().as_mut() {
                        // SAFETY: RenderingTeardown runs before Slint releases its GL context.
                        unsafe { manager.teardown_rendering() };
                    }
                    drop(blur_surface.take());
                    drop(surface.take());
                    #[cfg(target_os = "macos")]
                    {
                        icc_profile_display = None;
                        icc_profile_auto_active = None;
                    }
                    // Context loss also needs UI invalidation, but only after
                    // the backend releases its mutable window borrow. A later
                    // setup must not have its new frame cleared by stale work.
                    let retired_generation = render_generation.get();
                    let generation = render_generation.clone();
                    let weak = render_weak.clone();
                    slint::Timer::single_shot(Duration::ZERO, move || {
                        if generation.get() == retired_generation
                            && let Some(app) = weak.upgrade()
                            && !app.get_native_video()
                        {
                            app.set_video_frame(slint::Image::default());
                            set_video_material_sample(&app, slint::Image::default(), false);
                            app.set_engine_ready(false);
                        }
                    });
                }
                _ => {}
            }
        })?;
    Ok(())
}
