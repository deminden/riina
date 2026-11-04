//! Native video owns a host HWND; the shared player UI owns its transparent popup.
mod menu_actions;
mod menus;
mod placement;
mod surfaces;
use crate::{
    NativeVideoHost, PlayerWindow,
    player_ui::{UiSession, configure_player},
};
use placement::{center_host_in_work_area, schedule_startup_center};
use slint::{
    ComponentHandle,
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
use surfaces::{
    align, dismiss_unaligned_transition_cover, enable_video_transparency, set_overlay_visibility,
    sync_native_theme,
};
use winit::platform::windows::WindowAttributesExtWindows;
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
                    align(host.window(), &app);
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
            menus::install(&host, &app);
            menu_actions::install(&host, &app);
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
            menu_actions::install_playback(&host, &app);
            *startup_session.borrow_mut() = Some(configured);
            let close_session = startup_session.clone();
            let close_pip = startup_pip.clone();
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
