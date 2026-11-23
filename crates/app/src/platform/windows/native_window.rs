//! Native video owns a host HWND; the shared player UI owns its transparent popup.
mod events;
mod material_watch;
mod menu_actions;
mod menus;
mod placement;
mod surfaces;
mod visibility;
use crate::{
    NativeVideoHost, PlayerWindow,
    player_ui::{UiSession, configure_player},
    preference_load::LoadedPreferences,
};
use placement::{StartupCenter, center_host_in_work_area};
use riina_ui::PlayerMenuActions;
use slint::{
    ComponentHandle,
    winit_030::{WinitWindowAccessor, winit},
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
use surfaces::{align, enable_video_transparency};
use winit::platform::windows::WindowAttributesExtWindows;
pub fn run(
    media: Vec<OsString>,
    resume_position: Option<f64>,
    owner_center: Option<(i32, i32)>,
    loaded: LoadedPreferences,
) -> Result<(), Box<dyn std::error::Error>> {
    let app = NativeApp::create_host(&loaded)?;
    app.start(NativeLaunch {
        media,
        resume_position,
        owner_center,
        loaded,
    })?;
    app.run()
}

struct NativeLaunch {
    media: Vec<OsString>,
    resume_position: Option<f64>,
    owner_center: Option<(i32, i32)>,
    loaded: LoadedPreferences,
}

type Slot<T> = Rc<RefCell<Option<T>>>;

struct NativeApp {
    startup_failure: Slot<Box<dyn std::error::Error>>,
    placement: StartupCenter,
    menu_focus: Rc<Cell<bool>>,
    menu_lifecycle: Slot<super::menu_focus::MenuFocus>,
    resize_filter: Slot<crate::window_aspect::WindowEventFilter>,
    pip_controller: Slot<Rc<RefCell<super::pip::Controller>>>,
    session: Slot<UiSession>,
    smoke: Slot<super::package_smoke::SmokeRun>,
    welcome_window: Slot<crate::WelcomeWindow>,
    visibility_monitor: Slot<visibility::VisibilityMonitor>,
    material_policy_timer: Rc<slint::Timer>,
    hdr_display_timer: Rc<slint::Timer>,
    overlay_hidden_for_alignment: Rc<Cell<bool>>,
    surface_alignment_timer: Rc<slint::Timer>,
    chrome: Slot<PlayerWindow>,
    // Keep the window alive through the cleanup of earlier fields on an error.
    host: NativeVideoHost,
    next_owner: Arc<AtomicIsize>,
}

impl NativeApp {
    fn create_host(loaded: &LoadedPreferences) -> Result<Self, Box<dyn std::error::Error>> {
        let initial_native_theme = match loaded.preferences.effective_theme_material() {
            riina_core::ThemeMaterial::Dark => Some(winit::window::Theme::Dark),
            riina_core::ThemeMaterial::Light => Some(winit::window::Theme::Light),
            riina_core::ThemeMaterial::System => None,
        };
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
        Ok(Self {
            host,
            next_owner: owner,
            chrome: Default::default(),
            welcome_window: Default::default(),
            visibility_monitor: Default::default(),
            session: Default::default(),
            pip_controller: Default::default(),
            smoke: Default::default(),
            resize_filter: Default::default(),
            surface_alignment_timer: Default::default(),
            overlay_hidden_for_alignment: Default::default(),
            material_policy_timer: Default::default(),
            hdr_display_timer: Default::default(),
            menu_lifecycle: Default::default(),
            menu_focus: Default::default(),
            placement: Default::default(),
            startup_failure: Default::default(),
        })
    }

    fn start(&self, launch: NativeLaunch) -> Result<(), Box<dyn std::error::Error>> {
        let NativeLaunch {
            media,
            resume_position,
            owner_center,
            loaded,
        } = launch;
        let action_after_launch = loaded.preferences.action_after_launch;
        let supplied_media_argument = !media.is_empty();
        let weak_host = self.host.as_weak();
        let owner = self.next_owner.clone();
        let startup_chrome = self.chrome.clone();
        let startup_welcome = self.welcome_window.clone();
        let startup_session = self.session.clone();
        let startup_pip = self.pip_controller.clone();
        let startup_smoke = self.smoke.clone();
        let startup_resize_filter = self.resize_filter.clone();
        let startup_surface_alignment_timer = self.surface_alignment_timer.clone();
        let startup_overlay_hidden = self.overlay_hidden_for_alignment.clone();
        let startup_material_policy_timer = self.material_policy_timer.clone();
        let startup_hdr_display_timer = self.hdr_display_timer.clone();
        let startup_visibility_monitor = self.visibility_monitor.clone();
        let startup_menu_lifecycle = self.menu_lifecycle.clone();
        let startup_menu_focus = self.menu_focus.clone();
        let placement = self.placement.clone();
        let initialization_failure = self.startup_failure.clone();
        self.host.show()?;
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
                material_watch::install(&host, &window, &startup_material_policy_timer);
                let presentation = super::NativePresentation::new(window, host.as_weak())
                    .map_err(std::io::Error::other)?;
                let video_hwnd = presentation.owner_handle();
                owner.store(presentation.owner_handle(), Ordering::Relaxed);
                // Slint captures the attributes hook when creating the component adapter.
                // Construct only after its owner HWND exists, as in the verified probe.
                let app = PlayerWindow::new()?;
                app.set_native_video(true);
                app.global::<riina_ui::PlayerAppearance>()
                    .set_system_materials_enabled(super::materials::native_materials_enabled());
                surfaces::watch_alignment(
                    &host,
                    &app,
                    &startup_surface_alignment_timer,
                    &startup_overlay_hidden,
                );
                super::presentation::watch_hdr_display(&host, &app, &startup_hdr_display_timer);
                match visibility::VisibilityMonitor::install(
                    video_hwnd,
                    [
                        &startup_surface_alignment_timer,
                        &startup_material_policy_timer,
                        &startup_hdr_display_timer,
                    ],
                ) {
                    Ok(monitor) => *startup_visibility_monitor.borrow_mut() = Some(monitor),
                    Err(error) => crate::diagnostics::warn!("{error}"),
                }
                let title_host = host.as_weak();
                app.on_window_title_changed(move |title| {
                    if let Some(host) = title_host.upgrade() {
                        host.set_display_title(title);
                    }
                });
                placement.watch_initial_media(&host, &app, owner_center);
                install_music_mode_window(&host, &app, &startup_menu_lifecycle);
                restore_recreated_chrome(&host, &app)?;
                let welcome = if action_after_launch.show_welcome(supplied_media_argument) {
                    Some(crate::WelcomeWindow::new()?)
                } else {
                    None
                };
                if let Some(welcome) = &welcome {
                    keep_player_after_welcome_close(welcome, &host, &app, owner_center);
                }
                // Slint stores one event hook per window. Return the aspect filter
                // to our owner router so it cannot replace alignment/focus handling.
                let media_geometry = crate::window_aspect::MediaGeometry::default();
                *startup_resize_filter.borrow_mut() = Some(
                    crate::window_aspect::connect_to_native_host(&app, &host, &media_geometry),
                );
                install_menu_lifecycle(
                    &host,
                    &app,
                    presentation.owner_handle(),
                    &startup_menu_lifecycle,
                    startup_menu_focus,
                )?;
                let configured = configure_player(
                    &app,
                    Some(presentation),
                    media,
                    welcome.as_ref(),
                    resume_position,
                    loaded,
                    media_geometry,
                )?;
                placement.refresh_position_policy(&app);
                let commands = configured
                    .player()
                    .ok_or("The player engine was not initialized")?
                    .command_sender();
                *startup_pip.borrow_mut() = Some(
                    super::pip::Controller::install(&app, &host, video_hwnd, commands)
                        .map_err(std::io::Error::other)?,
                );
                menu_actions::install_playback(&host, &app);
                *startup_session.borrow_mut() = Some(configured);
                install_chrome_close_handlers(&host, &app, &startup_session, &startup_pip);
                let _controls = show_chrome(&host, &app, &startup_menu_lifecycle).await?;
                // The native host otherwise keeps Winit's default placement until
                // playback loads. That leaves idle launches and cancelled open
                // dialogs stranded at the OS-selected (often top-left) position.
                placement.schedule(host.as_weak(), app.as_weak(), owner_center);
                if action_after_launch.open_file_panel(supplied_media_argument) {
                    app.global::<PlayerMenuActions>().invoke_open_file();
                }
                // Its initial DPI and frameless client geometry are not settled until
                // the popup exists. Sizing a queued adapter can be overwritten at creation.
                {
                    let chrome_window = app.window().winit_window().await?;
                    enable_video_transparency(&chrome_window).map_err(std::io::Error::other)?;
                }
                align(host.window(), &app);
                if let Some(welcome) = &welcome {
                    show_welcome(&host, &app, welcome)?;
                }
                *startup_welcome.borrow_mut() = welcome;
                app.window().request_redraw();
                *startup_smoke.borrow_mut() = super::package_smoke::start(&app, &host);
                *startup_chrome.borrow_mut() = Some(app);
                Ok::<(), Box<dyn std::error::Error>>(())
            }
            .await;
            if let Err(error) = result {
                close(
                    &startup_session,
                    &startup_pip,
                    startup_chrome.borrow().as_ref(),
                );
                // Continue releasing session resources if the window is closing.
                let _ = host.hide();
                *initialization_failure.borrow_mut() = Some(error);
                // Shutdown may already have ended the event loop.
                let _ = slint::quit_event_loop();
            }
        })?;
        self.install_event_filter(owner_center);
        self.install_close_handler();
        Ok(())
    }

    fn install_event_filter(&self, owner_center: Option<(i32, i32)>) {
        events::install_filter(
            &self.host,
            events::FilterInputs {
                event_chrome: self.chrome.clone(),
                event_overlay_hidden: self.overlay_hidden_for_alignment.clone(),
                event_placement: self.placement.clone(),
                event_center_host: self.host.as_weak(),
                owner_center,
                resize_filter: self.resize_filter.clone(),
            },
        );
    }

    fn install_close_handler(&self) {
        let close_session = self.session.clone();
        let close_pip = self.pip_controller.clone();
        let close_chrome = self.chrome.clone();
        self.host.window().on_close_requested(move || {
            let chrome = close_chrome.borrow();
            if chrome.as_ref().is_some_and(|app| {
                app.global::<riina_ui::PlayerDialogState>()
                    .get_dialog_open()
            }) {
                return slint::CloseRequestResponse::KeepWindowShown;
            }
            let should_quit = chrome.as_ref().is_some_and(|app| {
                crate::player_manager::should_quit_after_last_window_closed(
                    app.global::<riina_ui::PlayerPreferences>()
                        .get_quit_when_no_opened_window(),
                    0,
                )
            });
            close(&close_session, &close_pip, chrome.as_ref());
            if let Some(app) = chrome.as_ref() {
                // Continue releasing session resources if the window is closing.
                let _ = app.hide();
            }
            if should_quit {
                // Shutdown may already have ended the event loop.
                let _ = slint::quit_event_loop();
            }
            slint::CloseRequestResponse::HideWindow
        });
    }

    fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        slint::run_event_loop()?;
        drop(self.smoke.borrow_mut().take());
        drop(self.visibility_monitor.borrow_mut().take());
        self.surface_alignment_timer.stop();
        self.material_policy_timer.stop();
        close(
            &self.session,
            &self.pip_controller,
            self.chrome.borrow().as_ref(),
        );
        if let Some(error) = self.startup_failure.borrow_mut().take() {
            return Err(error);
        }
        Ok(())
    }
}

fn install_music_mode_window(
    host: &NativeVideoHost,
    app: &PlayerWindow,
    menu: &Slot<super::menu_focus::MenuFocus>,
) {
    let saved_music_size = Cell::new(None);
    let music_host = host.as_weak();
    let music_app = app.as_weak();
    let music_menu = menu.clone();
    app.on_music_mode_changed(move || {
        let (Some(host), Some(app)) = (music_host.upgrade(), music_app.upgrade()) else {
            return;
        };
        if let Some(menu) = music_menu.borrow().as_ref()
            && let Err(error) = menu.set_visible(
                !app.global::<riina_ui::PlayerMusicModeState>()
                    .get_music_mode()
                    && !app.get_fullscreen(),
            )
        {
            app.set_status(error.into());
            return;
        }
        let mut resized = None;
        host.window().with_winit_window(|window| {
            let scale = window.scale_factor();
            let target = if app
                .global::<riina_ui::PlayerMusicModeState>()
                .get_music_mode()
            {
                if saved_music_size.get().is_none() {
                    saved_music_size.set(Some(window.inner_size()));
                }
                let height = crate::music_mode_layout::window_height(
                    app.global::<riina_ui::PlayerMusicModeState>()
                        .get_music_mode_show_album_art(),
                    app.global::<riina_ui::PlayerMusicModeState>()
                        .get_music_mode_show_playlist(),
                    f64::from(
                        app.global::<riina_ui::PlayerMusicModeState>()
                            .get_music_mode_artwork_aspect_ratio(),
                    ),
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
}

fn restore_recreated_chrome(
    host: &NativeVideoHost,
    app: &PlayerWindow,
) -> Result<(), std::io::Error> {
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
                // The window may close before deferred UI work can be delivered.
                let _ = slint::invoke_from_event_loop(move || {
                    if let (Some(app), Some(host)) = (weak_app.upgrade(), weak_host.upgrade()) {
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
        .map_err(std::io::Error::other)
}

fn keep_player_after_welcome_close(
    welcome: &crate::WelcomeWindow,
    host: &NativeVideoHost,
    app: &PlayerWindow,
    owner_center: Option<(i32, i32)>,
) {
    let quit_preference = app.as_weak();
    let close_welcome_app = app.as_weak();
    let close_welcome_host = host.as_weak();
    welcome.window().on_close_requested(move || {
        let setting = quit_preference.upgrade().is_some_and(|app| {
            app.global::<riina_ui::PlayerPreferences>()
                .get_quit_when_no_opened_window()
        });
        let should_quit = crate::player_manager::should_quit_after_last_window_closed(setting, 0);
        if should_quit {
            // Shutdown may already have ended the event loop.
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
            if let Err(error) = app.show() {
                crate::diagnostics::warn!(
                    "Could not show the player after closing welcome: {error}"
                );
            }
            host.window()
                .with_winit_window(|window| center_host_in_work_area(window, owner_center));
            align(host.window(), &app);
        }
        slint::CloseRequestResponse::HideWindow
    });
}

fn install_menu_lifecycle(
    host: &NativeVideoHost,
    app: &PlayerWindow,
    menu_handle: isize,
    lifecycle: &Slot<super::menu_focus::MenuFocus>,
    focus: Rc<Cell<bool>>,
) -> Result<(), std::io::Error> {
    *lifecycle.borrow_mut() = Some(
        super::menu_focus::MenuFocus::install(menu_handle, app.as_weak(), focus)
            .map_err(std::io::Error::other)?,
    );
    if let Some(menu) = lifecycle.borrow().as_ref() {
        menu.set_visible(
            !app.get_fullscreen()
                && !app
                    .global::<riina_ui::PlayerMusicModeState>()
                    .get_music_mode(),
        )
        .map_err(std::io::Error::other)?;
    }
    let fullscreen_menu = lifecycle.clone();
    let fullscreen_host = host.as_weak();
    let fullscreen_app = app.as_weak();
    host.on_set_fullscreen_window(move |fullscreen| {
        let (Some(host), Some(app)) = (fullscreen_host.upgrade(), fullscreen_app.upgrade()) else {
            return false;
        };
        if let Some(menu) = fullscreen_menu.borrow().as_ref()
            && let Err(error) = menu.set_visible(
                !fullscreen
                    && !app
                        .global::<riina_ui::PlayerMusicModeState>()
                        .get_music_mode(),
            )
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
    app.global::<riina_ui::PlayerControlActions>()
        .on_open_application_menu(move || {
            if let Some(_host) = keyboard_host.upgrade()
                && let Err(error) = super::menu_focus::request(menu_handle)
                && let Some(app) = keyboard_app.upgrade()
            {
                app.set_status(error.to_string().into());
            }
        });
    menus::install(host, app);
    menu_actions::install(host, app);
    Ok(())
}

fn install_chrome_close_handlers(
    host: &NativeVideoHost,
    app: &PlayerWindow,
    session: &Slot<UiSession>,
    pip: &Slot<Rc<RefCell<super::pip::Controller>>>,
) {
    let close_session = session.clone();
    let close_pip = pip.clone();
    let close_host = host.as_weak();
    let weak_app = app.as_weak();
    app.window().on_close_requested(move || {
        let app = weak_app.upgrade();
        if app.as_ref().is_some_and(|app| {
            app.global::<riina_ui::PlayerDialogState>()
                .get_dialog_open()
        }) {
            return slint::CloseRequestResponse::KeepWindowShown;
        }
        let should_quit = crate::player_manager::should_quit_after_last_window_closed(
            app.as_ref().is_some_and(|app| {
                app.global::<riina_ui::PlayerPreferences>()
                    .get_quit_when_no_opened_window()
            }),
            0,
        );
        close(&close_session, &close_pip, app.as_ref());
        if let Some(host) = close_host.upgrade() {
            // Continue releasing session resources if the window is closing.
            let _ = host.hide();
        }
        if should_quit {
            // Shutdown may already have ended the event loop.
            let _ = slint::quit_event_loop();
        }
        slint::CloseRequestResponse::HideWindow
    });
    let close_button_chrome = app.as_weak();
    let close_button_host = host.as_weak();
    let close_button_session = session.clone();
    let close_button_pip = pip.clone();
    app.global::<PlayerMenuActions>().on_close_player(move || {
        let chrome = close_button_chrome.upgrade();
        let should_quit = chrome.as_ref().is_some_and(|app| {
            crate::player_manager::should_quit_after_last_window_closed(
                app.global::<riina_ui::PlayerPreferences>()
                    .get_quit_when_no_opened_window(),
                0,
            )
        });
        close(&close_button_session, &close_button_pip, chrome.as_ref());
        if let Some(host) = close_button_host.upgrade() {
            // Continue releasing session resources if the window is closing.
            let _ = host.hide();
        }
        if should_quit {
            // Shutdown may already have ended the event loop.
            let _ = slint::quit_event_loop();
        }
    });
    let quit_app = app.as_weak();
    app.global::<PlayerMenuActions>().on_quit_riina(move || {
        if let Some(app) = quit_app.upgrade() {
            app.global::<PlayerMenuActions>().invoke_close_player();
        }
        // Shutdown may already have ended the event loop.
        let _ = slint::quit_event_loop();
    });
}

async fn show_chrome(
    host: &NativeVideoHost,
    app: &PlayerWindow,
    lifecycle: &Slot<super::menu_focus::MenuFocus>,
) -> Result<Arc<winit::window::Window>, Box<dyn std::error::Error>> {
    // The shared controls are a separate transparent HWND layered over
    // the native video host. Give it the host's realized bounds before
    // its first show so Winit never exposes it at its default position.
    align(host.window(), app);
    app.show()?;
    // The controls HWND is created lazily. Attach routing only after
    // realization, while the framed owner's menu guard retains it.
    let controls = app.window().winit_window().await?;
    if let Some(menu) = lifecycle.borrow_mut().as_mut() {
        menu.attach_controls(app.window())
            .map_err(std::io::Error::other)?;
    }
    Ok(controls)
}

fn show_welcome(
    host: &NativeVideoHost,
    app: &PlayerWindow,
    welcome: &crate::WelcomeWindow,
) -> Result<(), slint::PlatformError> {
    app.hide()?;
    // A visible native host owns the menu/video surface and would
    // cover the independent welcome window with its black idle frame.
    crate::platform::center_auxiliary_window(welcome, app);
    welcome.show()?;
    crate::platform::enable_welcome_mica(welcome);
    // NativePresentation retains the Winit handle for mpv's HWND
    // lifetime, so hide the OS surface directly instead of asking
    // Slint to destroy a still-referenced adapter window.
    host.window()
        .with_winit_window(|window| window.set_visible(false));
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
            crate::diagnostics::warn!("Preferences were not saved: {error}");
        }
        // Callbacks retain only senders. Dropping this sole owner joins the VO worker
        // before either HWND disappears; no orphan decoder can use a destroyed host.
        session.release_player();
    }
}
