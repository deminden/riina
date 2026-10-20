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
    time::{Duration, Instant},
};
use windows::Win32::{
    Foundation::{HWND, LPARAM, RECT},
    Graphics::{
        Dwm::{
            DWM_BB_BLURREGION, DWM_BB_ENABLE, DWM_BB_TRANSITIONONMAXIMIZED, DWM_BLURBEHIND,
            DwmEnableBlurBehindWindow,
        },
        Gdi::{CreateRectRgn, DeleteObject},
    },
    UI::WindowsAndMessaging::{
        GWL_EXSTYLE, GetWindowLongPtrW, GetWindowRect, SWP_FRAMECHANGED, SWP_NOACTIVATE,
        SWP_NOMOVE, SWP_NOOWNERZORDER, SWP_NOSIZE, SWP_NOZORDER, SendMessageW, SetWindowLongPtrW,
        SetWindowPos, WS_EX_LAYERED, WS_EX_TRANSPARENT,
    },
};
use windows::core::BOOL;
use winit::platform::windows::WindowAttributesExtWindows;
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

thread_local! {
    // Surface alignment retries run every 16 ms. Report each new failure once
    // so a persistent native-window mismatch stays diagnosable without flooding logs.
    static LAST_OVERLAY_ALIGNMENT_ERROR: RefCell<Option<String>> = const { RefCell::new(None) };
    #[cfg(debug_assertions)]
    static ALIGNMENT_WINDOWS_REPORTED: Cell<bool> = const { Cell::new(false) };
}

// Slint can change the popup's native styles while creating its shared menu.
// Preserve Winit's layered composition surface, but restore interactive hit
// testing after the shared menu has made the popup click-through.
fn enable_video_transparency(window: &winit::window::Window) -> Result<(), String> {
    let handle = window.window_handle().map_err(|error| error.to_string())?;
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return Err("Native video transparency requires a Windows window".into());
    };
    let hwnd = HWND(handle.hwnd.get() as *mut _);
    // Winit may provide WS_EX_LAYERED for per-pixel alpha composition. Preserve
    // its style and remove only the click-through flag that blocks control input.
    let extended_style = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) };
    let interactive_style = extended_style & !(WS_EX_TRANSPARENT.0 as isize);
    if interactive_style != extended_style {
        // SAFETY: hwnd is the retained, realized Slint chrome window. The style
        // change leaves its owner and taskbar attributes intact.
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
    if (effective_style ^ extended_style) & (WS_EX_LAYERED.0 as isize) != 0 {
        return Err("The native video overlay lost its layered composition style".into());
    }
    // An empty blur region enables pixel alpha without requesting an opaque
    // whole-window backdrop. A null region instead covers the entire client,
    // including the otherwise clear fullscreen area around the controller.
    let region = unsafe { CreateRectRgn(0, 0, 0, 0) };
    if region.0.is_null() {
        return Err("Could not create the controls' transparent composition region".into());
    }
    let blur_behind = DWM_BLURBEHIND {
        dwFlags: DWM_BB_ENABLE | DWM_BB_BLURREGION | DWM_BB_TRANSITIONONMAXIMIZED,
        fEnable: BOOL(1),
        hRgnBlur: region,
        fTransitionOnMaximized: BOOL(0),
    };
    // Windows 8+ no longer blurs with this legacy API, but Microsoft documents
    // that it still enables DWM to honor this window's per-pixel alpha values.
    // The actual Windows material comes from the sampled-video Acrylic layer.
    let result = unsafe { DwmEnableBlurBehindWindow(hwnd, &blur_behind) };
    // DWM copies the region; release our GDI object even if composition failed.
    let _ = unsafe { DeleteObject(region.into()) };
    result
        .map_err(|error| format!("Could not restore video-overlay alpha composition: {error}"))?;
    Ok(())
}

fn dismiss_unaligned_transition_cover(app: &PlayerWindow) {
    if app.get_native_transition_frame() {
        // A retained frame is opaque by design. If the controls HWND cannot be
        // verified over its video host, discard it before it can remain painted
        // at the overlay's previous desktop coordinates.
        app.set_native_transition_frame(false);
        app.set_video_frame(slint::Image::default());
        app.window().request_redraw();
    }
}

fn set_overlay_visibility(app: &PlayerWindow, visible: bool) {
    app.window().with_winit_window(|overlay| {
        overlay.set_visible(visible);
        if visible {
            // A move/fullscreen transition may hide the popup while its
            // previous redraw is queued. Re-arm the native paint event so
            // the restored surface cannot retain that frame indefinitely.
            overlay.request_redraw();
        }
    });
}

fn align(host: &slint::Window, chrome: &PlayerWindow) -> bool {
    let mut aligned = true;
    host.with_winit_window(|window| {
        chrome.window().with_winit_window(|overlay| {
            #[cfg(debug_assertions)]
            ALIGNMENT_WINDOWS_REPORTED.with(|reported| {
                if !reported.replace(true) {
                    let hwnd = |window: &winit::window::Window| {
                        window.window_handle().ok().and_then(|handle| match handle.as_raw() {
                            RawWindowHandle::Win32(handle) => Some(HWND(handle.hwnd.get() as *mut _)),
                            _ => None,
                        })
                    };
                    let host_hwnd = hwnd(window);
                    let overlay_hwnd = hwnd(overlay);
                    let bounds = |hwnd: HWND| {
                        let mut rect = RECT::default();
                        // SAFETY: Each handle was read from its live Winit window.
                        unsafe { GetWindowRect(hwnd, &mut rect) }
                            .ok()
                            .map(|_| (rect.left, rect.top, rect.right, rect.bottom))
                    };
                    eprintln!(
                        "Native video alignment HWNDs: host={host_hwnd:?} bounds={:?}; controls={overlay_hwnd:?} bounds={:?}",
                        host_hwnd.and_then(bounds),
                        overlay_hwnd.and_then(bounds),
                    );
                }
            });
            if let Err(error) = super::window_geometry::align_surfaces(window, overlay) {
                aligned = false;
                dismiss_unaligned_transition_cover(chrome);
                // Winit can still report pre-move coordinates while Windows is
                // reconciling an owned popup. Writing those stale coordinates
                // back here can detach the black-backed transparent surface
                // from playback. Keep the Win32 client bounds authoritative;
                // the next alignment tick retries after the move settles.
                let should_report = LAST_OVERLAY_ALIGNMENT_ERROR.with(|previous| {
                    let mut previous = previous.borrow_mut();
                    if previous.as_deref() == Some(error.as_str()) {
                        false
                    } else {
                        *previous = Some(error.clone());
                        true
                    }
                });
                if should_report {
                    eprintln!("Could not align native video controls atomically: {error}");
                }
            }
        });
    });
    if aligned {
        LAST_OVERLAY_ALIGNMENT_ERROR.with(|previous| *previous.borrow_mut() = None);
    }
    aligned
}

fn sync_native_theme(
    host: &winit::window::Window,
    dark_theme: bool,
    last_native_theme: &Cell<Option<bool>>,
) {
    // Native-video playback uses separate HWNDs for the framed host and
    // transparent controls. Keep the Windows caption/menu and Mica tone in the
    // same appearance as the controls when the user's theme changes.
    if last_native_theme.get() == Some(dark_theme) {
        return;
    }
    let Ok(handle) = host.window_handle() else {
        return;
    };
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return;
    };
    // Slint 1.18 does not expose WindowAdapter::set_color_scheme publicly, so
    // its Muda menubar misses app-driven theme changes. Muda 0.19.3's public
    // Menu::set_theme_for_hwnd sends this private update message; mirror that
    // call for the known live HWND and keep the crate version pinned above.
    const MUDA_MENU_UPDATE_THEME: u32 = 201;
    let menu_theme = if dark_theme {
        muda::MenuTheme::Dark
    } else {
        muda::MenuTheme::Light
    };
    let hwnd = HWND(handle.hwnd.get() as *mut _);
    // SAFETY: this HWND is the live Winit host whose Slint Muda menubar is
    // attached before this synchronization timer starts.
    unsafe {
        SendMessageW(
            hwnd,
            MUDA_MENU_UPDATE_THEME,
            None,
            Some(LPARAM(menu_theme as isize)),
        );
    }
    // Winit's `theme()` reports the cached system scheme, not the explicit
    // app-selected override. Apply only on transitions, after Muda's redraw
    // request so non-client activation cannot leave the caption in the old tone.
    host.set_theme(Some(if dark_theme {
        winit::window::Theme::Dark
    } else {
        winit::window::Theme::Light
    }));
    last_native_theme.set(Some(dark_theme));
}

fn center_host_in_work_area(window: &winit::window::Window, owner_center: Option<(i32, i32)>) {
    // Startup media may enter fullscreen before the delayed placement runs.
    // Work-area clamping must never shrink a fullscreen or maximized surface.
    if window.fullscreen().is_some() || window.is_maximized() {
        return;
    }
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
            if app.get_initial_window_position_enabled() || app.get_fullscreen() {
                pending.set(false);
                return;
            }
            host.window()
                .with_winit_window(|window| center_host_in_work_area(window, owner_center));
            align(host.window(), &app);
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
    let initial_native_theme = crate::player_ui::windows_initial_theme()?;
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
                // The first window is the persistent host. Its transparent
                // client reveals the requested backdrop only after DWM accepts
                // its attribute; actual pixels remain subject to OS policy.
                // Initialize the native menu in the saved app theme before Slint
                // creates its Muda menu; later frame-theme changes cannot restyle
                // a menu that was born with the OS's initial color scheme.
                attributes
                    .with_theme(initial_native_theme)
                    .with_transparent(true)
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
    let overlay_hidden_for_alignment = Rc::new(Cell::new(false));
    let startup_overlay_hidden = overlay_hidden_for_alignment.clone();
    let material_policy_timer = Rc::new(slint::Timer::default());
    let startup_material_policy_timer = material_policy_timer.clone();
    let welcome_window: Rc<RefCell<Option<crate::WelcomeWindow>>> = Rc::new(RefCell::new(None));
    let smoke = Rc::new(RefCell::new(None));
    let startup_smoke = smoke.clone();
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
            let initial_material_policy = super::materials::native_mica_enabled();
            let mica_attribute_applied = match initial_material_policy
                .then(|| super::materials::apply_main_window_mica(&window))
            {
                Some(Ok(())) => true,
                Some(Err(error)) => {
                    eprintln!(
                        "Windows system backdrop unavailable; keeping the renderer fallback: {error}"
                    );
                    false
                }
                None => false,
            };
            host.set_mica_backdrop_enabled(mica_attribute_applied);
            let material_host = host.as_weak();
            let last_material_policy = Rc::new(Cell::new(initial_material_policy));
            let applied_material_policy = Rc::new(Cell::new(if initial_material_policy {
                mica_attribute_applied.then_some(true)
            } else {
                Some(false)
            }));
            let material_retry_delay = Rc::new(Cell::new(Duration::from_millis(500)));
            let material_retry_after = Rc::new(Cell::new(Instant::now()));
            let material_last_error = Rc::new(RefCell::new(None::<String>));
            let timer_material_host = material_host.clone();
            let timer_last_material_policy = last_material_policy.clone();
            let timer_applied_material_policy = applied_material_policy.clone();
            let timer_material_retry_delay = material_retry_delay.clone();
            let timer_material_retry_after = material_retry_after.clone();
            let timer_material_last_error = material_last_error.clone();
            startup_material_policy_timer.start(
                slint::TimerMode::Repeated,
                Duration::from_millis(500),
                move || {
                    let policy_enabled = super::materials::native_mica_enabled();
                    let policy_changed = timer_last_material_policy.get() != policy_enabled;
                    if policy_changed {
                        timer_last_material_policy.set(policy_enabled);
                        timer_material_retry_delay.set(Duration::from_millis(500));
                        timer_material_retry_after.set(Instant::now());
                        *timer_material_last_error.borrow_mut() = None;
                    }
                    let Some(host) = timer_material_host.upgrade() else {
                        return;
                    };
                    if timer_applied_material_policy.get() == Some(policy_enabled)
                        && host.get_mica_backdrop_enabled() == policy_enabled
                    {
                        return;
                    }
                    if Instant::now() < timer_material_retry_after.get() {
                        return;
                    }
                    let mut update_result = None;
                    host.window().with_winit_window(|window| {
                        update_result = Some(if policy_enabled {
                            super::materials::apply_main_window_mica(window)
                        } else {
                            super::materials::clear_main_window_backdrop(window)
                        });
                    });
                    match update_result {
                        Some(Ok(())) => {
                            timer_applied_material_policy.set(Some(policy_enabled));
                            host.set_mica_backdrop_enabled(policy_enabled);
                            timer_material_retry_delay.set(Duration::from_millis(500));
                            *timer_material_last_error.borrow_mut() = None;
                        }
                        Some(Err(error)) => {
                            timer_applied_material_policy.set(None);
                            if !policy_enabled {
                                host.set_mica_backdrop_enabled(false);
                            }
                            let mut last_error = timer_material_last_error.borrow_mut();
                            if last_error.as_deref() != Some(error.as_str()) {
                                eprintln!("Could not update the Windows system backdrop policy: {error}");
                                *last_error = Some(error);
                            }
                            let retry_delay = timer_material_retry_delay.get();
                            timer_material_retry_after.set(Instant::now() + retry_delay);
                            timer_material_retry_delay.set(
                                retry_delay.saturating_mul(2).min(Duration::from_secs(30)),
                            );
                        }
                        None => {
                            timer_applied_material_policy.set(None);
                            if !policy_enabled {
                                host.set_mica_backdrop_enabled(false);
                            }
                            let retry_delay = timer_material_retry_delay.get();
                            timer_material_retry_after.set(Instant::now() + retry_delay);
                            timer_material_retry_delay.set(
                                retry_delay.saturating_mul(2).min(Duration::from_secs(30)),
                            );
                        }
                    }
                },
            );
            let presentation = super::NativePresentation::new(window, host.as_weak())
                .map_err(std::io::Error::other)?;
            let video_hwnd = presentation.owner_handle();
            owner.store(presentation.owner_handle(), Ordering::Relaxed);
            // Slint captures the attributes hook when creating the component adapter.
            // Construct only after its owner HWND exists, as in the verified probe.
            let app = PlayerWindow::new()?;
            app.set_native_video(true);
            app.set_system_materials_enabled(super::materials::native_materials_enabled());
            let alignment_host = host.as_weak();
            let alignment_chrome = app.as_weak();
            let timer_overlay_hidden = startup_overlay_hidden.clone();
            let last_native_theme = Rc::new(Cell::new(None));
            let alignment_native_theme = last_native_theme.clone();
            startup_surface_alignment_timer.start(
                slint::TimerMode::Repeated,
                std::time::Duration::from_millis(16),
                move || {
                    if let (Some(host), Some(chrome)) =
                        (alignment_host.upgrade(), alignment_chrome.upgrade())
                    {
                        host.window().with_winit_window(|window| {
                            sync_native_theme(
                                window,
                                chrome.get_dark_theme(),
                                &alignment_native_theme,
                            );
                        });
                        // mpv can resize or hide its child HWND during a
                        // playlist handoff without resizing the host. Repair
                        // that transient independently of host WM_SIZE events.
                        let aligned = align(host.window(), &chrome);
                        if aligned && timer_overlay_hidden.get() {
                            set_overlay_visibility(&chrome, true);
                            timer_overlay_hidden.set(false);
                        } else if !aligned && !timer_overlay_hidden.get() {
                            // The retry loop is also the fallback when Winit
                            // misses a host move notification. Never leave a
                            // detached transparent surface visible at its old
                            // desktop coordinates while geometry catches up.
                            set_overlay_visibility(&chrome, false);
                            timer_overlay_hidden.set(true);
                        }
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
            let music_menu = startup_menu_lifecycle.clone();
            app.on_music_mode_changed(move || {
                let (Some(host), Some(app)) = (music_host.upgrade(), music_app.upgrade()) else {
                    return;
                };
                if let Some(menu) = music_menu.borrow().as_ref()
                    && let Err(error) = menu.set_visible(!app.get_music_mode() && !app.get_fullscreen())
                {
                    app.set_status(error.into());
                    return;
                }
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
                        window.set_min_inner_size(Some(winit::dpi::LogicalSize::new(260.0, 90.0)));
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
            let mut alpha_trace = super::overlay_trace::OverlayTrace::new();
            app.window()
                .set_rendering_notifier(move |state, graphics_api| {
                    if let Some(app) = recreated_app.upgrade() {
                        alpha_trace.observe(&state, graphics_api, &app);
                    }
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
                                align(host.window(), &app);
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
                        align(host.window(), &app);
                        host.window()
                            .with_winit_window(|window| window.set_visible(true));
                        let _ = app.show();
                        host.window().with_winit_window(|window| {
                            center_host_in_work_area(window, owner_center)
                        });
                        align(host.window(), &app);
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
            if let Some(menu) = startup_menu_lifecycle.borrow().as_ref() {
                menu.set_visible(!app.get_fullscreen() && !app.get_music_mode())
                    .map_err(std::io::Error::other)?;
            }
            let fullscreen_menu = startup_menu_lifecycle.clone();
            let fullscreen_host = host.as_weak();
            let fullscreen_app = app.as_weak();
            host.on_set_fullscreen_window(move |fullscreen| {
                let (Some(host), Some(app)) = (fullscreen_host.upgrade(), fullscreen_app.upgrade()) else {
                    return false;
                };
                if let Some(menu) = fullscreen_menu.borrow().as_ref()
                    && let Err(error) = menu.set_visible(!fullscreen && !app.get_music_mode())
                {
                    app.set_status(error.into());
                    return false;
                }
                host.set_fullscreen_mode(fullscreen);
                host.window().set_fullscreen(fullscreen);
                true
            });
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
                    host.set_playback_muted(app.get_muted());
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
                        host.set_playback_muted(app.get_muted());
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
                        let audio_rows = app.get_audio_tracks();
                        crate::ui_models::set_rows_if_changed(
                            &host.get_audio_track_menu_rows(),
                            (0..audio_rows.row_count()).filter_map(|index| audio_rows.row_data(index))
                                .map(|row| crate::MenuCatalogRow { token: row.token, label: row.label, selected: row.selected }).collect(),
                            |rows| host.set_audio_track_menu_rows(rows),
                        );
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
            let menu_app = app.as_weak();
            host.on_hide_riina(move || {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_hide_riina();
                }
            });
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
            host.on_select_menu_audio_track(move |token| {
                if let Some(app) = menu_app.upgrade() { app.invoke_select_item(1, token); }
            });
            let menu_app = app.as_weak();
            host.on_load_external_audio(move || {
                if let Some(app) = menu_app.upgrade() { app.invoke_browse_files(1); }
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
            host.on_multiply_subtitle_scale(move |factor| {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_multiply_subtitle_scale(factor);
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
            align(host.window(), &app);
            app.show()?;
            // The controls HWND is created lazily. Attach routing only after
            // realization, while the framed owner's menu guard retains it.
            let _controls = app.window().winit_window().await?;
            if let Some(menu) = startup_menu_lifecycle.borrow_mut().as_mut() {
                menu.attach_controls(app.window()).map_err(std::io::Error::other)?;
            }
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
            align(host.window(), &app);
            if let Some(welcome) = &welcome {
                app.hide()?;
                // A visible native host owns the menu/video surface and would
                // cover the independent welcome window with its black idle frame.
                crate::platform::center_auxiliary_window(welcome, &app);
                welcome.show()?;
                crate::platform::enable_welcome_mica(welcome);
                // NativePresentation retains the Winit handle for mpv's HWND
                // lifetime, so hide the OS surface directly instead of asking
                // Slint to destroy a still-referenced adapter window.
                host.window()
                    .with_winit_window(|window| window.set_visible(false));
            }
            *startup_welcome.borrow_mut() = welcome;
            app.window().request_redraw();
            *startup_smoke.borrow_mut() = super::package_smoke::start(&app, &host);
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
    let event_overlay_hidden = overlay_hidden_for_alignment.clone();
    let scroll_transition = Rc::new(Cell::new(0_u64));
    let event_scroll_transition = scroll_transition.clone();
    let host_input = super::host_input::HostInput::default();
    host.window().on_winit_window_event(move |host, event| {
        if let Some(app) = event_chrome.borrow().as_ref()
            && host_input.handle(app, event) == EventResult::PreventDefault
        {
            return EventResult::PreventDefault;
        }
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
            let host_moved = matches!(event, winit::event::WindowEvent::Moved(_));
            if host_moved {
                // Retained frames belong to the old client geometry. Keep the
                // live popup visible while aligning it: fullscreen also emits
                // Moved, and hiding/showing a surface during that transition can
                // strand its compositor image even while UI state advances.
                dismiss_unaligned_transition_cover(app);
            }
            app.set_hdr_display_active(super::presentation::hdr_display_active_for_slint(host));
            let aligned = align(host, app);
            if !aligned {
                // Hide only a proven mismatch; the retry loop restores it once
                // Win32 confirms matching host, child and popup client bounds.
                if !event_overlay_hidden.replace(true) {
                    set_overlay_visibility(app, false);
                }
            } else if aligned && event_overlay_hidden.get() {
                set_overlay_visibility(app, true);
                event_overlay_hidden.set(false);
            }
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
    drop(smoke.borrow_mut().take());
    surface_alignment_timer.stop();
    material_policy_timer.stop();
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
