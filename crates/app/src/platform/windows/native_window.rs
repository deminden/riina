//! Native video owns a host HWND; the shared player UI owns its transparent popup.
use crate::{
    NativeVideoHost, PlayerWindow,
    player_ui::{UiSession, configure_player},
};
use slint::{
    ComponentHandle, Model,
    winit_030::{EventResult, WinitWindowAccessor, winit},
};
use std::{
    cell::{Cell, RefCell},
    ffi::OsString,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicIsize, Ordering},
    },
};
use windows::{
    Win32::{
        Foundation::{HWND, RECT},
        Graphics::{
            Dwm::{DWM_BB_BLURREGION, DWM_BB_ENABLE, DWM_BLURBEHIND, DwmEnableBlurBehindWindow},
            Gdi::{CreateRectRgn, DeleteObject, HGDIOBJ},
        },
        UI::WindowsAndMessaging::{
            GWL_EXSTYLE, GetWindowLongPtrW, GetWindowRect, SWP_FRAMECHANGED, SWP_NOACTIVATE,
            SWP_NOMOVE, SWP_NOOWNERZORDER, SWP_NOSIZE, SWP_NOZORDER, SetWindowLongPtrW,
            SetWindowPos, WS_EX_TRANSPARENT,
        },
    },
    core::BOOL,
};
use winit::platform::windows::WindowAttributesExtWindows;
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

// Slint registers even the hidden shared-renderer menu, then forces its HWND
// opaque. The native owner already hosts the menu, so restore the chrome's alpha
// after realization using the same DWM region as Winit's transparent windows.
fn enable_video_transparency(window: &winit::window::Window) -> Result<(), String> {
    let handle = window.window_handle().map_err(|error| error.to_string())?;
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return Err("Native video transparency requires a Windows window".into());
    };
    let hwnd = HWND(handle.hwnd.get() as *mut _);
    // Keep Winit's layered alpha for video composition, but remove its input
    // transparency so the controls remain part of the hit-test surface.
    let extended_style = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) };
    let interactive_style = extended_style & !(WS_EX_TRANSPARENT.0 as isize);
    if interactive_style != extended_style {
        // SAFETY: hwnd is the retained, realized Slint chrome window. The style
        // is updated without replacing its owner, alpha, or taskbar attributes.
        unsafe {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, interactive_style);
            SetWindowPos(
                hwnd,
                None,
                0,
                0,
                0,
                0,
                SWP_FRAMECHANGED
                    | SWP_NOMOVE
                    | SWP_NOSIZE
                    | SWP_NOACTIVATE
                    | SWP_NOOWNERZORDER
                    | SWP_NOZORDER,
            )
            .map_err(|error| format!("Could not enable native video control input: {error}"))?;
        }
    }
    let effective_style = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) };
    if effective_style & (WS_EX_TRANSPARENT.0 as isize) != 0 {
        return Err("The native video overlay is still mouse-transparent".into());
    }
    // SAFETY: This creates a locally owned GDI region with no borrowed pointers.
    let region = unsafe { CreateRectRgn(0, 0, -1, -1) };
    if region.0.is_null() {
        return Err("Could not create the transparent video region".into());
    }
    let blur = DWM_BLURBEHIND {
        dwFlags: DWM_BB_ENABLE | DWM_BB_BLURREGION,
        fEnable: BOOL(1),
        hRgnBlur: region,
        fTransitionOnMaximized: BOOL(0),
    };
    // SAFETY: The retained Winit window owns this live HWND. DWM reads the
    // descriptor synchronously and does not take ownership of the GDI region.
    let result = unsafe { DwmEnableBlurBehindWindow(hwnd, &blur) };
    // SAFETY: Release our region after DWM has consumed it, including on failure.
    unsafe {
        let _ = DeleteObject(HGDIOBJ(region.0));
    }
    result.map_err(|error| format!("Could not make the native video overlay transparent: {error}"))
}

fn align(host: &slint::Window, chrome: &slint::Window) {
    host.with_winit_window(|window| {
        if let Ok(origin) = window.inner_position() {
            // Both values are physical: never multiply a physical size by DPI again.
            let size =
                slint::PhysicalSize::new(window.inner_size().width, window.inner_size().height);
            let position = slint::PhysicalPosition::new(origin.x, origin.y);
            chrome.with_winit_window(|overlay| {
                if let Err(error) =
                    super::window_geometry::align_surfaces(window, overlay, position, size)
                {
                    // The overlay adapter may not have an HWND during its
                    // initial construction. Keep queued Slint geometry there;
                    // realized windows use one native move/resize operation.
                    if chrome.size() != size {
                        chrome.set_size(size);
                    }
                    if chrome.position() != position {
                        chrome.set_position(position);
                    }
                    if !error.contains("not initialized") {
                        eprintln!("Could not align native video controls atomically: {error}");
                    }
                }
            });
        }
    });
}

fn center_host_in_work_area(window: &winit::window::Window, owner_center: Option<(i32, i32)>) {
    let Ok(raw_window) = window.window_handle() else {
        return;
    };
    let RawWindowHandle::Win32(raw_window) = raw_window.as_raw() else {
        return;
    };
    let hwnd = HWND(raw_window.hwnd.get() as *mut _);
    let mut frame = RECT::default();
    // SAFETY: frame is a valid writable RECT and hwnd belongs to this live window.
    if unsafe { GetWindowRect(hwnd, &mut frame) }.is_err() {
        return;
    }
    // Fit oversized initial frames inside the taskbar-free area before
    // centering. A position-only center clamps an overheight window to (0, 0).
    let work_area = owner_center
        .and_then(|(x, y)| super::window_position::work_area_near_point(x, y))
        .or_else(|| super::window_position::primary_work_area(window));
    let Some(work_area) = work_area else {
        return;
    };
    let width = frame.right.saturating_sub(frame.left).max(0) as u32;
    let height = frame.bottom.saturating_sub(frame.top).max(0) as u32;
    let bounds = riina_core::window_resize::ScreenRect {
        x: work_area.x,
        y: work_area.y,
        size: riina_core::window_aspect::PixelSize {
            width: work_area.size.width,
            height: work_area.size.height,
        },
    };
    let margin = (24.0 * window.scale_factor()).round().max(0.0) as u32;
    let Some(placement) = riina_core::window_resize::centered_window_placement(
        riina_core::window_aspect::PixelSize { width, height },
        bounds,
        margin,
    ) else {
        return;
    };
    let mut flags = SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_NOZORDER;
    if placement.size.width == width && placement.size.height == height {
        flags |= SWP_NOSIZE;
    }
    // Position by the measured outer frame so DPI conversion cannot shift the
    // caption edge; shrink only when that frame exceeds the usable area.
    let _ = unsafe {
        SetWindowPos(
            hwnd,
            None,
            placement.x,
            placement.y,
            i32::try_from(placement.size.width).unwrap_or(i32::MAX),
            i32::try_from(placement.size.height).unwrap_or(i32::MAX),
            flags,
        )
    };
}

fn schedule_startup_center(
    host: slint::Weak<NativeVideoHost>,
    app: slint::Weak<PlayerWindow>,
    pending: Rc<Cell<bool>>,
    generation: Rc<Cell<u64>>,
    owner_center: Option<(i32, i32)>,
) {
    if !pending.get() {
        return;
    }
    let next = generation.get().wrapping_add(1);
    generation.set(next);
    slint::Timer::single_shot(std::time::Duration::from_millis(250), move || {
        if generation.get() != next || !pending.get() {
            return;
        }
        if let (Some(host), Some(app)) = (host.upgrade(), app.upgrade()) {
            if app.get_initial_window_position_enabled() {
                pending.set(false);
                return;
            }
            host.window()
                .with_winit_window(|window| center_host_in_work_area(window, owner_center));
            align(host.window(), app.window());
            pending.set(false);
        }
    });
}

pub fn run(
    media: Vec<OsString>,
    resume_position: Option<f64>,
    owner_center: Option<(i32, i32)>,
) -> Result<(), Box<dyn std::error::Error>> {
    let action_after_launch = crate::player_ui::action_after_launch()?;
    let supplied_media_argument = !media.is_empty();
    let owner = Arc::new(AtomicIsize::new(0));
    let hook_owner = owner.clone();
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("femtovg".into())
        .with_winit_window_attributes_hook(move |attributes| {
            // Consume the next adapter's owner once. Leaving it installed also
            // styles auxiliary windows as transparent, taskbar-hidden overlays.
            let handle = hook_owner.swap(0, Ordering::Relaxed);
            if handle == 0 {
                attributes
            } else {
                attributes
                    .with_owner_window(handle)
                    .with_transparent(true)
                    .with_decorations(false)
                    .with_resizable(false)
                    .with_skip_taskbar(true)
            }
        })
        .select()?;
    let host = NativeVideoHost::new()?;
    host.set_bring_all_to_front_available(true);
    host.set_display_title("riina".into());
    let chrome: Rc<RefCell<Option<PlayerWindow>>> = Rc::new(RefCell::new(None));
    let surface_alignment_timer = Rc::new(slint::Timer::default());
    let startup_surface_alignment_timer = surface_alignment_timer.clone();
    let welcome_window: Rc<RefCell<Option<crate::WelcomeWindow>>> = Rc::new(RefCell::new(None));
    let session: Rc<RefCell<Option<UiSession>>> = Rc::new(RefCell::new(None));
    let pip_controller: Rc<RefCell<Option<Rc<RefCell<super::pip::Controller>>>>> =
        Rc::new(RefCell::new(None));
    let weak_host = host.as_weak();
    let startup_chrome = chrome.clone();
    let startup_welcome = welcome_window.clone();
    let startup_session = session.clone();
    let startup_pip = pip_controller.clone();
    let resize_filter = Rc::new(RefCell::new(
        None::<crate::window_aspect::WindowEventFilter>,
    ));
    let startup_resize_filter = resize_filter.clone();
    let menu_lifecycle = Rc::new(RefCell::new(None::<super::menu_focus::MenuFocus>));
    let startup_menu_lifecycle = menu_lifecycle.clone();
    let menu_focus = Rc::new(Cell::new(false));
    let startup_menu_focus = menu_focus.clone();
    let startup_center_pending = Rc::new(Cell::new(false));
    let startup_media_loaded = Rc::new(Cell::new(false));
    let startup_center_generation = Rc::new(Cell::new(0_u64));
    let event_center_host = host.as_weak();
    let event_startup_center_pending = startup_center_pending.clone();
    let event_startup_media_loaded = startup_media_loaded.clone();
    let event_startup_center_generation = startup_center_generation.clone();
    host.show()?;
    slint::spawn_local(async move {
        let Some(host) = weak_host.upgrade() else {
            return;
        };
        let result = async {
            let window = host.window().winit_window().await?;
            // Winit's first placement is an implementation default, often near
            // the upper-left corner. Correct it as soon as the native frame is
            // realized so the user never sees the temporary location while the
            // player and playback session are still being initialized.
            center_host_in_work_area(&window, owner_center);
            window.set_min_inner_size(Some(winit::dpi::LogicalSize::new(285.0, 120.0)));
            if let Err(error) = super::materials::apply_main_window_mica(&window) {
                eprintln!(
                    "Windows system backdrop unavailable; keeping the renderer fallback: {error}"
                );
            }
            let presentation = super::NativePresentation::new(window, host.as_weak())
                .map_err(std::io::Error::other)?;
            let video_hwnd = presentation.owner_handle();
            owner.store(presentation.owner_handle(), Ordering::Relaxed);
            // Slint captures the attributes hook when creating the component adapter.
            // Construct only after its owner HWND exists, as in the verified probe.
            let app = PlayerWindow::new()?;
            app.set_native_video(true);
            let alignment_host = host.as_weak();
            let alignment_chrome = app.as_weak();
            startup_surface_alignment_timer.start(
                slint::TimerMode::Repeated,
                std::time::Duration::from_millis(16),
                move || {
                    if let (Some(host), Some(chrome)) =
                        (alignment_host.upgrade(), alignment_chrome.upgrade())
                    {
                        // mpv can resize or hide its child HWND during a
                        // playlist handoff without resizing the host. Repair
                        // that transient independently of host WM_SIZE events.
                        align(host.window(), chrome.window());
                    }
                },
            );
            let title_host = host.as_weak();
            app.on_window_title_changed(move |title| {
                if let Some(host) = title_host.upgrade() {
                    host.set_display_title(title);
                }
            });
            let music_original_size = Rc::new(Cell::new(None));
            startup_center_pending.set(!app.get_initial_window_position_enabled());
            let loaded_center_host = host.as_weak();
            let loaded_center_app = app.as_weak();
            let loaded_center_pending = startup_center_pending.clone();
            let loaded_media_state = startup_media_loaded.clone();
            let loaded_center_generation = startup_center_generation.clone();
            let loaded_owner_center = owner_center;
            app.on_initial_media_loaded(move || {
                if !loaded_center_pending.get() {
                    return;
                }
                loaded_media_state.set(true);
                schedule_startup_center(
                    loaded_center_host.clone(),
                    loaded_center_app.clone(),
                    loaded_center_pending.clone(),
                    loaded_center_generation.clone(),
                    loaded_owner_center,
                );
            });
            let saved_music_size = music_original_size.clone();
            let music_host = host.as_weak();
            let music_app = app.as_weak();
            app.on_music_mode_changed(move || {
                let (Some(host), Some(app)) = (music_host.upgrade(), music_app.upgrade()) else {
                    return;
                };
                let mut resized = None;
                host.window().with_winit_window(|window| {
                    let scale = window.scale_factor();
                    let target = if app.get_music_mode() {
                        if saved_music_size.get().is_none() {
                            saved_music_size.set(Some(window.inner_size()));
                        }
                        let height = crate::music_mode_layout::window_height(
                            app.get_music_mode_show_album_art(),
                            app.get_music_mode_show_playlist(),
                            f64::from(app.get_music_mode_artwork_aspect_ratio()),
                        );
                        window.set_min_inner_size(Some(winit::dpi::LogicalSize::new(260.0, 72.0)));
                        winit::dpi::PhysicalSize::new(
                            (crate::music_mode_layout::WINDOW_WIDTH * scale).round() as u32,
                            (height * scale).round() as u32,
                        )
                    } else if let Some(size) = saved_music_size.take() {
                        window.set_min_inner_size(Some(winit::dpi::LogicalSize::new(285.0, 120.0)));
                        size
                    } else {
                        return;
                    };
                    resized = Some((window.inner_size(), target));
                });
                if let Some((current, target)) = resized {
                    crate::window_aspect::resize_centered(
                        host.window(),
                        riina_core::window_aspect::PixelSize {
                            width: current.width,
                            height: current.height,
                        },
                        riina_core::window_aspect::PixelSize {
                            width: target.width,
                            height: target.height,
                        },
                        Some(app.window()),
                    );
                }
            });
            let recreated_app = app.as_weak();
            let recreated_host = host.as_weak();
            app.window()
                .set_rendering_notifier(move |state, _| {
                    if matches!(state, slint::RenderingState::RenderingSetup) {
                        let weak_app = recreated_app.clone();
                        let weak_host = recreated_host.clone();
                        // Welcome/dialog recovery can destroy and recreate the
                        // chrome HWND. Restore each realized surface, after renderer
                        // setup has released its internal borrows.
                        let _ = slint::invoke_from_event_loop(move || {
                            if let (Some(app), Some(host)) =
                                (weak_app.upgrade(), weak_host.upgrade())
                            {
                                app.window().with_winit_window(|window| {
                                    if let Err(error) = enable_video_transparency(window) {
                                        app.set_status(error.into());
                                    }
                                });
                                align(host.window(), app.window());
                            }
                        });
                    }
                })
                .map_err(std::io::Error::other)?;
            let welcome = if action_after_launch.show_welcome(supplied_media_argument) {
                Some(crate::WelcomeWindow::new()?)
            } else {
                None
            };
            if let Some(welcome) = &welcome {
                let quit_preference = app.as_weak();
                let close_welcome_app = app.as_weak();
                let close_welcome_host = host.as_weak();
                welcome.window().on_close_requested(move || {
                    let setting = quit_preference
                        .upgrade()
                        .is_some_and(|app| app.get_quit_when_no_opened_window());
                    let should_quit =
                        crate::player_manager::should_quit_after_last_window_closed(setting, 0);
                    if should_quit {
                        let _ = slint::quit_event_loop();
                    } else if let (Some(app), Some(host)) =
                        (close_welcome_app.upgrade(), close_welcome_host.upgrade())
                    {
                        // The welcome surface is the only visible native window at
                        // startup. Keep a usable player window when the user closes
                        // it but has asked the application to remain available.
                        align(host.window(), app.window());
                        host.window()
                            .with_winit_window(|window| window.set_visible(true));
                        let _ = app.show();
                        host.window().with_winit_window(|window| {
                            center_host_in_work_area(window, owner_center)
                        });
                        align(host.window(), app.window());
                    }
                    slint::CloseRequestResponse::HideWindow
                });
            }
            // Slint stores one event hook per window. Return the aspect filter
            // to our owner router so it cannot replace alignment/focus handling.
            *startup_resize_filter.borrow_mut() =
                Some(crate::window_aspect::connect_to_native_host(&app, &host));
            let menu_handle = presentation.owner_handle();
            *startup_menu_lifecycle.borrow_mut() = Some(
                super::menu_focus::MenuFocus::install(
                    menu_handle,
                    app.as_weak(),
                    startup_menu_focus.clone(),
                )
                .map_err(std::io::Error::other)?,
            );
            let keyboard_host = host.as_weak();
            let keyboard_app = app.as_weak();
            app.on_open_application_menu(move || {
                if let Some(_host) = keyboard_host.upgrade()
                    && let Err(error) = super::menu_focus::request(menu_handle)
                    && let Some(app) = keyboard_app.upgrade()
                {
                    app.set_status(error.to_string().into());
                }
            });
            let menu_state_host = host.as_weak();
            let menu_state_app = app.as_weak();
            app.on_native_menu_will_open(move || {
                if let (Some(host), Some(app)) =
                    (menu_state_host.upgrade(), menu_state_app.upgrade())
                {
                    let settings_pane = if app.get_settings_at_leading() {
                        app.get_leading_sidebar()
                    } else {
                        app.get_sidebar()
                    };
                    host.set_video_panel_open(settings_pane == 6);
                    host.set_audio_panel_open(settings_pane == 1);
                    host.set_subtitles_panel_open(settings_pane == 2);
                    host.set_plugins_panel_open(
                        app.get_leading_sidebar() == 7 || app.get_sidebar() == 7,
                    );
                    host.set_playback_paused(app.get_paused());
                    host.set_playback_speed_value(app.get_speed_edit_value());
                    host.set_volume_menu_title(app.get_volume_menu_title());
                    host.set_playback_muted(app.get_muted());
                    host.set_audio_delay_text(app.get_audio_delay_text());
                }
            });
            // The framed owner hosts the native menu; its transparent popup
            // retains controller state. Bridge observations without a second
            // playback model or sending window handles across threads.
            let menu_host = host.as_weak();
            let menu_app = app.as_weak();
            let menu_pending = Rc::new(Cell::new(false));
            app.on_video_menu_changed(move || {
                if menu_pending.replace(true) {
                    return;
                }
                let menu_host = menu_host.clone();
                let menu_app = menu_app.clone();
                let menu_pending = menu_pending.clone();
                slint::Timer::single_shot(std::time::Duration::from_millis(80), move || {
                    menu_pending.set(false);
                    if let (Some(host), Some(app)) = (menu_host.upgrade(), menu_app.upgrade()) {
                        let settings_pane = if app.get_settings_at_leading() {
                            app.get_leading_sidebar()
                        } else {
                            app.get_sidebar()
                        };
                        host.set_video_panel_open(settings_pane == 6);
                        host.set_audio_panel_open(settings_pane == 1);
                        host.set_subtitles_panel_open(settings_pane == 2);
                        host.set_plugins_panel_open(
                            app.get_leading_sidebar() == 7 || app.get_sidebar() == 7,
                        );
                        host.set_reflection_menu_enabled(
                            app.get_loaded()
                                && app.get_reflection_known()
                                && !app.get_crop_editing(),
                        );
                        host.set_video_mirrored(app.get_video_mirrored());
                        host.set_video_flipped(app.get_video_flipped());
                        host.set_delogo_menu_enabled(
                            app.get_delogo_enabled() && !app.get_crop_editing(),
                        );
                        host.set_delogo_active(app.get_delogo_active());
                        host.set_saved_video_filter_rows(app.get_saved_video_filter_rows());
                        host.set_saved_audio_filter_rows(app.get_saved_audio_filter_rows());
                        host.set_crop_menu_enabled(
                            app.get_loaded()
                                && app.get_video_crop_known()
                                && !app.get_crop_editing(),
                        );
                        host.set_crop_menu_labels(app.get_video_crop_menu_labels());
                        host.set_crop_menu_index(app.get_video_crop_menu_index());
                        host.set_aspect_menu_enabled(
                            app.get_loaded()
                                && app.get_video_aspect_known()
                                && !app.get_crop_editing(),
                        );
                        host.set_aspect_menu_labels(app.get_video_aspect_menu_labels());
                        host.set_video_aspect_menu_index(app.get_video_aspect_menu_index());
                        host.set_rotation_menu_enabled(
                            app.get_loaded()
                                && app.get_video_rotation_known()
                                && !app.get_crop_editing(),
                        );
                        host.set_video_rotation_index(app.get_video_rotation_index());
                        host.set_deinterlace_menu_enabled(
                            app.get_loaded()
                                && app.get_deinterlace_known()
                                && !app.get_crop_editing(),
                        );
                        host.set_deinterlace_enabled(app.get_deinterlace_enabled());
                        host.set_screenshot_available(
                            app.get_screenshot_available() && !app.get_crop_editing(),
                        );
                        host.set_screenshot_folder_enabled(!app.get_crop_editing());
                        host.set_fullscreen_mode(app.get_fullscreen());
                        host.set_always_float_on_top(app.get_always_float_on_top());
                        host.set_menu_shortcuts(app.get_menu_shortcuts());
                        host.set_unlock_window_aspect_ratio(app.get_unlock_window_aspect_ratio());
                        host.set_live_text_available(app.get_live_text_available());
                        host.set_live_text_enabled(app.get_live_text_enabled());
                        host.set_record_recent_files(app.get_record_recent_files());
                        host.set_recent_open_menu_rows(app.get_recent_open_menu_rows());
                        host.set_plugin_menu_rows(app.get_plugin_menu_rows());
                        host.set_playback_loaded(app.get_loaded() && !app.get_crop_editing());
                        host.set_reveal_current_media_available(
                            app.get_reveal_current_media_available(),
                        );
                        host.set_delete_current_media_available(
                            app.get_delete_current_media_available(),
                        );
                        host.set_save_current_playlist_available(
                            app.get_save_current_playlist_available(),
                        );
                        host.set_downloaded_subtitle_save_available(
                            app.get_downloaded_subtitle_save_available(),
                        );
                        host.set_open_subtitles_provider_index(
                            app.get_open_subtitles_provider_index(),
                        );
                        host.set_current_file_manager_name(app.get_current_file_manager_name());
                        host.set_playback_paused(app.get_paused());
                        host.set_menu_alternates_visible(app.get_menu_alternates_visible());
                        host.set_playback_speed_value(app.get_speed_edit_value());
                        host.set_volume_menu_title(app.get_volume_menu_title());
                        host.set_playback_muted(app.get_muted());
                        host.set_audio_delay_text(app.get_audio_delay_text());
                        host.set_loop_label(app.get_loop_label());
                        let playlist_rows = app.get_playlist();
                        crate::ui_models::set_rows_if_changed(
                            &host.get_playlist_menu_rows(),
                            (0..playlist_rows.row_count())
                                .filter_map(|index| playlist_rows.row_data(index))
                                .map(|row| crate::MenuCatalogRow {
                                    token: row.token,
                                    label: row.label,
                                    selected: row.playing,
                                })
                                .collect::<Vec<_>>(),
                            |rows| host.set_playlist_menu_rows(rows),
                        );
                        host.set_audio_device_menu_rows(app.get_audio_device_menu_rows());
                        let chapter_rows = app.get_chapters();
                        crate::ui_models::set_rows_if_changed(
                            &host.get_chapter_menu_rows(),
                            (0..chapter_rows.row_count())
                                .filter_map(|index| chapter_rows.row_data(index))
                                .map(|row| crate::MenuCatalogRow {
                                    token: row.token,
                                    label: row.label,
                                    selected: row.selected,
                                })
                                .collect::<Vec<_>>(),
                            |rows| host.set_chapter_menu_rows(rows),
                        );
                        let video_rows = app.get_video_tracks();
                        crate::ui_models::set_rows_if_changed(
                            &host.get_video_track_menu_rows(),
                            (0..video_rows.row_count())
                                .filter_map(|index| video_rows.row_data(index))
                                .map(|row| crate::MenuCatalogRow {
                                    token: row.token,
                                    label: row.label,
                                    selected: row.selected,
                                })
                                .collect::<Vec<_>>(),
                            |rows| host.set_video_track_menu_rows(rows),
                        );
                        host.set_subtitle_visible(app.get_subtitle_visible());
                        host.set_secondary_subtitle_visible(app.get_secondary_subtitle_visible());
                        host.set_secondary_subtitle_visibility_known(
                            app.get_secondary_subtitle_visibility_known(),
                        );
                        host.set_subtitle_scale_known(app.get_subtitle_scale_known());
                        host.set_subtitle_default_encoding(app.get_subtitle_default_encoding());
                        let subtitle_rows = app.get_subtitle_tracks();
                        crate::ui_models::set_rows_if_changed(
                            &host.get_subtitle_track_menu_rows(),
                            (0..subtitle_rows.row_count())
                                .filter_map(|index| subtitle_rows.row_data(index))
                                .map(|row| crate::MenuCatalogRow {
                                    token: row.token,
                                    label: row.label,
                                    selected: row.selected,
                                })
                                .collect::<Vec<_>>(),
                            |rows| host.set_subtitle_track_menu_rows(rows),
                        );
                        let secondary_subtitle_rows = app.get_secondary_subtitle_tracks();
                        crate::ui_models::set_rows_if_changed(
                            &host.get_secondary_subtitle_menu_rows(),
                            (0..secondary_subtitle_rows.row_count())
                                .filter_map(|index| secondary_subtitle_rows.row_data(index))
                                .map(|row| crate::MenuCatalogRow {
                                    token: row.token,
                                    label: row.label,
                                    selected: row.selected,
                                })
                                .collect::<Vec<_>>(),
                            |rows| host.set_secondary_subtitle_menu_rows(rows),
                        );
                        let subtitle_encodings = app.get_subtitle_encoding_options();
                        crate::ui_models::set_rows_if_changed(
                            &host.get_subtitle_encoding_menu_rows(),
                            (0..subtitle_encodings.row_count())
                                .filter_map(|index| subtitle_encodings.row_data(index))
                                .map(|row| crate::MenuCatalogRow {
                                    token: row.token,
                                    label: row.label,
                                    selected: row.selected,
                                })
                                .collect::<Vec<_>>(),
                            |rows| host.set_subtitle_encoding_menu_rows(rows),
                        );
                        host.set_music_mode_available(app.get_loaded() && !app.get_crop_editing());
                        host.set_music_mode(app.get_music_mode());
                        host.set_video_resize_available(
                            app.get_video_window_width() > 0.0
                                && app.get_video_window_height() > 0.0
                                && !app.get_crop_editing(),
                        );
                        host.set_picture_in_picture_available(
                            app.get_loaded()
                                && app.get_video_width() > 0.0
                                && !app.get_crop_editing(),
                        );
                    }
                });
            });
            app.invoke_video_menu_changed();
            let menu_app = app.as_weak();
            host.on_open_media_file(move || {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_open_file();
                }
            });
            let menu_app = app.as_weak();
            host.on_open_in_new_window(move || {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_open_in_new_window();
                }
            });
            let menu_app = app.as_weak();
            host.on_reveal_current_media(move || {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_reveal_current_media();
                }
            });
            let menu_app = app.as_weak();
            host.on_delete_current_media(move || {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_delete_current_media();
                }
            });
            let menu_app = app.as_weak();
            host.on_show_playback_history(move || {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_show_playback_history();
                }
            });
            let menu_app = app.as_weak();
            host.on_save_current_playlist(move || {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_save_current_playlist();
                }
            });
            let menu_app = app.as_weak();
            host.on_save_downloaded_subtitle(move || {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_save_downloaded_subtitle();
                }
            });
            let menu_app = app.as_weak();
            host.on_show_about(move || {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_show_about();
                }
            });
            let menu_app = app.as_weak();
            host.on_open_help_link(move |destination| {
                let Some(app) = menu_app.upgrade() else {
                    return;
                };
                let Some(url) = crate::player_ui::help_menu_url(destination.as_str()) else {
                    return;
                };
                if let Err(error) = crate::platform::open_http_url_in_browser(url) {
                    app.set_status(format!("Could not open the Help link: {error}").into());
                }
            });
            let debug_app = app.as_weak();
            host.on_dump_debug_info(move || {
                if let Some(app) = debug_app.upgrade() {
                    app.invoke_dump_debug_info();
                }
            });
            let menu_app = app.as_weak();
            host.on_quit_riina(move || {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_quit_riina();
                }
            });
            let menu_host = host.as_weak();
            host.on_minimize_window(move || {
                if let Some(host) = menu_host.upgrade() {
                    crate::platform::minimize_window(host.window());
                }
            });
            let menu_host = host.as_weak();
            host.on_zoom_window(move || {
                if let Some(host) = menu_host.upgrade() {
                    crate::platform::toggle_zoom_window(host.window());
                }
            });
            let menu_host = host.as_weak();
            host.on_bring_all_to_front(move || {
                if let Some(host) = menu_host.upgrade()
                    && let Err(error) =
                        super::bring_to_front::bring_all_riina_windows_to_front(host.window())
                {
                    eprintln!("Could not bring all Riina windows forward: {error}");
                }
            });
            let menu_app = app.as_weak();
            host.on_close_player(move || {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_close_player();
                }
            });
            let menu_app = app.as_weak();
            host.on_open_url_dialog(move || {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_open_url_dialog();
                }
            });
            let menu_app = app.as_weak();
            host.on_open_recent_path(move |path| {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_open_recent_path(path);
                }
            });
            let menu_app = app.as_weak();
            host.on_clear_open_recent(move || {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_clear_open_recent();
                }
            });
            let menu_app = app.as_weak();
            host.on_toggle_music_mode(move || {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_toggle_music_mode();
                }
            });
            let menu_app = app.as_weak();
            host.on_toggle_video_reflection(move |horizontal| {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_toggle_video_reflection(horizontal);
                }
            });
            let menu_app = app.as_weak();
            host.on_select_menu_playlist_item(move |token| {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_playlist_action(12, token);
                }
            });
            let menu_app = app.as_weak();
            host.on_select_menu_chapter_item(move |token| {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_select_item(3, token);
                }
            });
            let menu_app = app.as_weak();
            host.on_show_video_panel(move || {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_toggle_settings_pane(6);
                }
            });
            let menu_app = app.as_weak();
            host.on_show_plugins_panel(move || {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_toggle_plugins_pane();
                }
            });
            let menu_app = app.as_weak();
            host.on_cycle_audio_tracks(move || {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_cycle_audio_track();
                }
            });
            let menu_app = app.as_weak();
            host.on_select_audio_device(move |device| {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_set_audio_device(device);
                }
            });
            let menu_app = app.as_weak();
            host.on_adjust_volume(move |delta| {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_adjust_volume(delta);
                }
            });
            let menu_app = app.as_weak();
            host.on_toggle_mute(move || {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_toggle_mute();
                }
            });
            let menu_app = app.as_weak();
            host.on_show_audio_panel(move || {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_toggle_settings_pane(1);
                }
            });
            let menu_app = app.as_weak();
            host.on_adjust_audio_delay(move |delta| {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_adjust_audio_delay(delta);
                }
            });
            let menu_app = app.as_weak();
            host.on_reset_audio_delay(move || {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_reset_audio_delay();
                }
            });
            let menu_app = app.as_weak();
            host.on_cycle_subtitles(move || {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_cycle_subtitle_track();
                }
            });
            let menu_app = app.as_weak();
            host.on_toggle_subtitle_visibility(move || {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_toggle_subtitle_visibility();
                }
            });
            let menu_app = app.as_weak();
            host.on_show_subtitles_panel(move || {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_toggle_settings_pane(2);
                }
            });
            let menu_app = app.as_weak();
            host.on_select_subtitle_track(move |token| {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_select_item(2, token);
                }
            });
            let menu_app = app.as_weak();
            host.on_select_secondary_subtitle(move |token| {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_select_item(5, token);
                }
            });
            let menu_app = app.as_weak();
            host.on_toggle_secondary_subtitle_visibility(move || {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_toggle_secondary_subtitle_visibility();
                }
            });
            let menu_app = app.as_weak();
            host.on_load_external_subtitle(move || {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_browse_files(2);
                }
            });
            let menu_app = app.as_weak();
            host.on_find_online_subtitles(move || {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_find_online_subtitles();
                }
            });
            let menu_app = app.as_weak();
            host.on_find_online_subtitles_with_provider(move |index| {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_find_online_subtitles_with_provider(index);
                }
            });
            let menu_app = app.as_weak();
            host.on_set_subtitle_encoding(move |encoding| {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_set_subtitle_encoding(encoding);
                }
            });
            let menu_app = app.as_weak();
            host.on_adjust_subtitle_scale(move |delta| {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_adjust_subtitle_scale(delta);
                }
            });
            let menu_app = app.as_weak();
            host.on_reset_subtitle_scale(move || {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_set_subtitle_scale_slider(0.0);
                }
            });
            let menu_app = app.as_weak();
            host.on_adjust_subtitle_delay(move |delta| {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_adjust_subtitle_delay(delta);
                }
            });
            let menu_app = app.as_weak();
            host.on_reset_subtitle_delay(move || {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_reset_subtitle_delay();
                }
            });
            let menu_app = app.as_weak();
            host.on_choose_subtitle_font(move || {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_choose_subtitle_font();
                }
            });
            let menu_app = app.as_weak();
            host.on_cycle_video_track(move || {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_cycle_video_track();
                }
            });
            let menu_app = app.as_weak();
            host.on_select_video_track(move |token| {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_select_item(6, token);
                }
            });
            let menu_app = app.as_weak();
            host.on_fit_window_to_video(move || {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_fit_window_to_video();
                }
            });
            let menu_app = app.as_weak();
            host.on_resize_window_command(move |command| {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_resize_window_command(command);
                }
            });
            let menu_app = app.as_weak();
            host.on_toggle_fullscreen(move || {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_toggle_fullscreen();
                }
            });
            let menu_app = app.as_weak();
            host.on_toggle_float_on_top(move || {
                if let Some(app) = menu_app.upgrade() {
                    app.set_always_float_on_top(!app.get_always_float_on_top());
                }
            });
            let menu_app = app.as_weak();
            host.on_toggle_window_aspect_lock(move || {
                if let Some(app) = menu_app.upgrade() {
                    app.set_unlock_window_aspect_ratio(!app.get_unlock_window_aspect_ratio());
                }
            });
            let menu_app = app.as_weak();
            host.on_toggle_live_text(move || {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_toggle_live_text();
                }
            });
            let configured = configure_player(
                &app,
                Some(presentation),
                media,
                welcome.as_ref(),
                resume_position,
            )?;
            startup_center_pending.set(!app.get_initial_window_position_enabled());
            let commands = configured
                .player
                .as_ref()
                .ok_or("The player engine was not initialized")?
                .command_sender();
            *startup_pip.borrow_mut() = Some(
                super::pip::Controller::install(&app, &host, video_hwnd, commands)
                    .map_err(std::io::Error::other)?,
            );
            let menu_app = app.as_weak();
            host.on_set_menu_video_crop(move |index| {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_set_menu_video_crop(index);
                }
            });
            let menu_app = app.as_weak();
            host.on_toggle_delogo(move || {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_toggle_delogo();
                }
            });
            let menu_app = app.as_weak();
            host.on_toggle_saved_video_filter(move |filter| {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_toggle_saved_video_filter(filter);
                }
            });
            let menu_app = app.as_weak();
            host.on_open_video_filters(move || {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_open_video_filters();
                }
            });
            let menu_app = app.as_weak();
            host.on_toggle_saved_audio_filter(move |filter| {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_toggle_saved_audio_filter(filter);
                }
            });
            let menu_app = app.as_weak();
            host.on_open_audio_filters(move || {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_open_audio_filters();
                }
            });
            let menu_app = app.as_weak();
            host.on_set_menu_video_rotation(move |degrees| {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_set_menu_video_rotation(degrees);
                }
            });
            *startup_session.borrow_mut() = Some(configured);
            let menu_app = app.as_weak();
            host.on_set_menu_video_aspect(move |index| {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_set_menu_video_aspect(index);
                }
            });
            let close_session = startup_session.clone();
            let close_pip = startup_pip.clone();
            let menu_app = app.as_weak();
            host.on_toggle_deinterlace(move || {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_toggle_deinterlace();
                }
            });
            let menu_app = app.as_weak();
            host.on_take_screenshot(move || {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_take_screenshot();
                }
            });
            let menu_app = app.as_weak();
            host.on_open_screenshot_folder(move || {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_open_screenshot_folder();
                }
            });
            let preferences_app = app.as_weak();
            host.on_open_preferences(move || {
                if let Some(app) = preferences_app.upgrade() {
                    app.invoke_open_preferences();
                }
            });
            let plugin_manager_app = app.as_weak();
            host.on_manage_plugins(move || {
                if let Some(app) = plugin_manager_app.upgrade() {
                    app.set_preferences_target_pane(9);
                    app.invoke_open_preferences();
                }
            });
            let reload_plugins_app = app.as_weak();
            host.on_reload_plugins(move || {
                if let Some(app) = reload_plugins_app.upgrade() {
                    app.invoke_reload_plugins();
                }
            });
            let menu_action_app = app.as_weak();
            host.on_activate_plugin_menu_item(move |token| {
                if let Some(app) = menu_action_app.upgrade() {
                    app.invoke_activate_plugin_menu_item(token);
                }
            });
            let jump_app = app.as_weak();
            host.on_open_jump_time_dialog(move || {
                if let Some(app) = jump_app.upgrade() {
                    app.invoke_open_jump_time_dialog();
                }
            });
            let menu_app = app.as_weak();
            host.on_toggle_pause(move || {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_toggle_pause();
                }
            });
            let menu_app = app.as_weak();
            host.on_stop(move || {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_stop();
                }
            });
            let menu_app = app.as_weak();
            host.on_seek_relative(move |seconds| {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_seek_relative(seconds);
                }
            });
            let menu_app = app.as_weak();
            host.on_seek(move |seconds| {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_seek(seconds);
                }
            });
            let menu_app = app.as_weak();
            host.on_frame_step(move || {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_frame_step();
                }
            });
            let menu_app = app.as_weak();
            host.on_frame_back_step(move || {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_frame_back_step();
                }
            });
            let menu_app = app.as_weak();
            host.on_set_speed(move |factor| {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_speed(factor);
                }
            });
            let menu_app = app.as_weak();
            host.on_reset_speed(move || {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_reset_speed();
                }
            });
            let menu_app = app.as_weak();
            host.on_cycle_ab_loop(move || {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_cycle_ab_loop();
                }
            });
            let menu_app = app.as_weak();
            host.on_toggle_file_loop(move || {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_toggle_file_loop();
                }
            });
            let menu_app = app.as_weak();
            host.on_cycle_playlist_loop(move || {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_playlist_action(5, "".into());
                }
            });
            let menu_app = app.as_weak();
            host.on_playlist_next(move || {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_playlist_next();
                }
            });
            let menu_app = app.as_weak();
            host.on_playlist_previous(move || {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_playlist_previous();
                }
            });
            let menu_app = app.as_weak();
            host.on_show_playlist_panel(move || {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_toggle_playlist_pane(4);
                }
            });
            let menu_app = app.as_weak();
            host.on_show_chapters_panel(move || {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_toggle_playlist_pane(3);
                }
            });
            let menu_app = app.as_weak();
            host.on_adjust_chapter(move |delta| {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_adjust_chapter(delta);
                }
            });
            let close_host = host.as_weak();
            let weak_app = app.as_weak();
            app.window().on_close_requested(move || {
                let app = weak_app.upgrade();
                if app.as_ref().is_some_and(|app| app.get_dialog_open()) {
                    return slint::CloseRequestResponse::KeepWindowShown;
                }
                let should_quit = crate::player_manager::should_quit_after_last_window_closed(
                    app.as_ref()
                        .is_some_and(PlayerWindow::get_quit_when_no_opened_window),
                    0,
                );
                close(&close_session, &close_pip, app.as_ref());
                if let Some(host) = close_host.upgrade() {
                    let _ = host.hide();
                }
                if should_quit {
                    let _ = slint::quit_event_loop();
                }
                slint::CloseRequestResponse::HideWindow
            });
            let close_button_chrome = app.as_weak();
            let close_button_host = host.as_weak();
            let close_button_session = startup_session.clone();
            let close_button_pip = startup_pip.clone();
            app.on_close_player(move || {
                let chrome = close_button_chrome.upgrade();
                let should_quit = chrome.as_ref().is_some_and(|app| {
                    crate::player_manager::should_quit_after_last_window_closed(
                        app.get_quit_when_no_opened_window(),
                        0,
                    )
                });
                close(&close_button_session, &close_button_pip, chrome.as_ref());
                if let Some(host) = close_button_host.upgrade() {
                    let _ = host.hide();
                }
                if should_quit {
                    let _ = slint::quit_event_loop();
                }
            });
            let quit_app = app.as_weak();
            app.on_quit_riina(move || {
                if let Some(app) = quit_app.upgrade() {
                    app.invoke_close_player();
                }
                let _ = slint::quit_event_loop();
            });
            // The shared controls are a separate transparent HWND layered over
            // the native video host. Give it the host's realized bounds before
            // its first show so Winit never exposes it at its default position.
            align(host.window(), app.window());
            app.show()?;
            // The native host otherwise keeps Winit's default placement until
            // playback loads. That leaves idle launches and cancelled open
            // dialogs stranded at the OS-selected (often top-left) position.
            schedule_startup_center(
                host.as_weak(),
                app.as_weak(),
                startup_center_pending.clone(),
                startup_center_generation.clone(),
                owner_center,
            );
            if action_after_launch.open_file_panel(supplied_media_argument) {
                app.invoke_open_file();
            }
            // Its initial DPI and frameless client geometry are not settled until
            // the popup exists. Sizing a queued adapter can be overwritten at creation.
            {
                let chrome_window = app.window().winit_window().await?;
                enable_video_transparency(&chrome_window).map_err(std::io::Error::other)?;
            }
            align(host.window(), app.window());
            if let Some(welcome) = &welcome {
                app.hide()?;
                // A visible native host owns the menu/video surface and would
                // cover the independent welcome window with its black idle frame.
                crate::platform::center_auxiliary_window(welcome, &app);
                welcome.show()?;
                // NativePresentation retains the Winit handle for mpv's HWND
                // lifetime, so hide the OS surface directly instead of asking
                // Slint to destroy a still-referenced adapter window.
                host.window()
                    .with_winit_window(|window| window.set_visible(false));
            }
            *startup_welcome.borrow_mut() = welcome;
            app.window().request_redraw();
            *startup_chrome.borrow_mut() = Some(app);
            Ok::<(), Box<dyn std::error::Error>>(())
        }
        .await;
        if let Err(error) = result {
            eprintln!("Native player startup failed: {error}");
            close(
                &startup_session,
                &startup_pip,
                startup_chrome.borrow().as_ref(),
            );
            let _ = host.hide();
        }
    })?;
    let event_chrome = chrome.clone();
    let scroll_transition = Rc::new(Cell::new(0_u64));
    let event_scroll_transition = scroll_transition.clone();
    host.window().on_winit_window_event(move |host, event| {
        if event_startup_media_loaded.get()
            && matches!(
                event,
                winit::event::WindowEvent::Resized(_)
                    | winit::event::WindowEvent::ScaleFactorChanged { .. }
            )
            && let Some(app) = event_chrome.borrow().as_ref()
        {
            schedule_startup_center(
                event_center_host.clone(),
                app.as_weak(),
                event_startup_center_pending.clone(),
                event_startup_center_generation.clone(),
                owner_center,
            );
        }
        let result = resize_filter
            .borrow()
            .as_ref()
            .map_or(EventResult::Propagate, |filter| filter(host, event));
        if matches!(
            event,
            winit::event::WindowEvent::Focused(_)
                | winit::event::WindowEvent::CursorEntered { .. }
                | winit::event::WindowEvent::CursorLeft { .. }
        ) && let Some(app) = event_chrome.borrow().as_ref()
        {
            // Switching between the aligned video and chrome HWNDs can produce
            // a stale wheel delta. Keep that transition from becoming a seek.
            app.set_scroll_input_suppressed(true);
            let generation = event_scroll_transition.get().wrapping_add(1);
            event_scroll_transition.set(generation);
            let scroll_transition = event_scroll_transition.clone();
            let weak_app = app.as_weak();
            slint::Timer::single_shot(std::time::Duration::from_millis(220), move || {
                if scroll_transition.get() == generation
                    && let Some(app) = weak_app.upgrade()
                {
                    app.set_scroll_input_suppressed(false);
                }
            });
        }
        if matches!(
            event,
            winit::event::WindowEvent::Moved(_)
                | winit::event::WindowEvent::Resized(_)
                | winit::event::WindowEvent::ScaleFactorChanged { .. }
        ) && let Some(app) = event_chrome.borrow().as_ref()
        {
            app.set_hdr_display_active(super::presentation::hdr_display_active_for_slint(host));
            align(host, app.window());
        }
        result
    });
    let close_session = session.clone();
    let close_pip = pip_controller.clone();
    let close_chrome = chrome.clone();
    host.window().on_close_requested(move || {
        let chrome = close_chrome.borrow();
        if chrome.as_ref().is_some_and(|app| app.get_dialog_open()) {
            return slint::CloseRequestResponse::KeepWindowShown;
        }
        let should_quit = chrome.as_ref().is_some_and(|app| {
            crate::player_manager::should_quit_after_last_window_closed(
                app.get_quit_when_no_opened_window(),
                0,
            )
        });
        close(&close_session, &close_pip, chrome.as_ref());
        if let Some(app) = chrome.as_ref() {
            let _ = app.hide();
        }
        if should_quit {
            let _ = slint::quit_event_loop();
        }
        slint::CloseRequestResponse::HideWindow
    });
    slint::run_event_loop()?;
    surface_alignment_timer.stop();
    close(&session, &pip_controller, chrome.borrow().as_ref());
    Ok(())
}

fn close(
    session: &RefCell<Option<UiSession>>,
    pip: &RefCell<Option<Rc<RefCell<super::pip::Controller>>>>,
    app: Option<&PlayerWindow>,
) {
    if let Some(app) = app {
        app.invoke_window_will_close();
    }
    if let Some(controller) = pip.borrow_mut().take() {
        super::pip::Controller::shutdown(controller);
    }
    if let Some(app) = app {
        app.invoke_dismiss_font_picker();
    }
    if let Some(mut session) = session.borrow_mut().take() {
        if let Some(app) = app
            && let Err(error) = session.save(app)
        {
            eprintln!("Preferences were not saved: {error}");
        }
        // Callbacks retain only senders. Dropping this sole owner joins the VO worker
        // before either HWND disappears; no orphan decoder can use a destroyed host.
        session.player.take();
    }
}
