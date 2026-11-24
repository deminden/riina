//! Native video owns a host HWND; the shared player UI owns its transparent popup.
mod application;
mod events;
mod material_watch;
mod menu_actions;
mod menus;
mod placement;
mod surfaces;
mod visibility;
use crate::{
    NativeVideoHost, PlayerWindow,
    player_ui::{ApplicationServices, InitialMedia, UiSession, configure_player},
    preference_load::LoadedPreferences,
};
use application::{Backend, WindowActions};
use placement::{StartupCenter, center_host_in_work_area};
use riina_ui::PlayerMenuActions;
use slint::{
    ComponentHandle,
    winit_030::{WinitWindowAccessor, winit},
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};
use surfaces::{align, enable_video_transparency};
pub(crate) fn run(
    players: impl Iterator<Item = InitialMedia>,
    owner_center: Option<(i32, i32)>,
    loaded: LoadedPreferences,
    browser: &crate::browser_links::Inbox,
) -> Result<(), Box<dyn std::error::Error>> {
    let launches = players.map(|media| NativeLaunch {
        media,
        owner_center,
        loaded: loaded.clone(),
    });
    application::run(launches, browser)
}

struct NativeLaunch {
    media: InitialMedia,
    owner_center: Option<(i32, i32)>,
    loaded: LoadedPreferences,
}

type Slot<T> = Rc<RefCell<Option<T>>>;

struct NativePlayer {
    startup_task: Slot<slint::JoinHandle<()>>,
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
}

impl NativePlayer {
    fn create_host() -> Result<Self, slint::PlatformError> {
        let host = NativeVideoHost::new()?;
        host.set_bring_all_to_front_available(true);
        host.set_display_title("riina".into());
        Ok(Self {
            host,
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
            startup_task: Default::default(),
        })
    }

    fn start(
        &self,
        launch: NativeLaunch,
        backend: Backend,
        actions: WindowActions,
        application_services: ApplicationServices,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let NativeLaunch {
            media,
            owner_center,
            loaded,
        } = launch;
        let action_after_launch = loaded.preferences.action_after_launch;
        let supplied_media_argument = media.requests_playback();
        let managed_ticket = media.managed.clone();
        let disable_animation = media
            .managed
            .as_ref()
            .is_some_and(|ticket| ticket.options.disable_window_animation);
        let weak_host = self.host.as_weak();
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
        self.install_close_handler(actions.clone());
        self.host.show()?;
        let startup = slint::spawn_local(async move {
            let Some(host) = weak_host.upgrade() else {
                return;
            };
            let result = async {
                let window = host.window().winit_window().await?;
                if disable_animation {
                    crate::platform::window_sizing::disable_plugin_window_animation(host.window());
                }
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
                // Slint captures the attributes hook when creating the component adapter.
                // Construct only after its owner HWND exists, as in the verified probe.
                let app = backend.create_controls(video_hwnd)?;
                app.set_native_video(true);
                app.global::<riina_ui::PlayerAppearance>()
                    .set_system_materials_enabled(super::materials::native_materials_enabled());
                connect_video_surfaces(
                    &host,
                    &app,
                    video_hwnd,
                    [
                        &startup_surface_alignment_timer,
                        &startup_material_policy_timer,
                        &startup_hdr_display_timer,
                    ],
                    &startup_overlay_hidden,
                    &startup_visibility_monitor,
                );
                placement.watch_initial_media(&host, &app, owner_center);
                install_music_mode_window(&host, &app, &startup_menu_lifecycle);
                restore_recreated_chrome(&host, &app)?;
                let welcome = (managed_ticket.is_none()
                    && (actions.opens_welcome()
                        || action_after_launch.show_welcome(supplied_media_argument)))
                .then(crate::WelcomeWindow::new)
                .transpose()?;
                if let Some(welcome) = &welcome {
                    keep_player_after_welcome_close(
                        welcome,
                        &host,
                        &app,
                        owner_center,
                        actions.clone(),
                    );
                }
                // Slint stores one event hook per window. Return the aspect filter
                // to our owner router so it cannot replace alignment/focus handling.
                let media_geometry = crate::window_aspect::MediaGeometry::default();
                *startup_resize_filter.borrow_mut() = Some(
                    crate::window_aspect::connect_with_target(&app, Some(&host), &media_geometry),
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
                    loaded,
                    media_geometry,
                    application_services.clone(),
                )?;
                placement.refresh_position_policy(&app);
                let commands = configured
                    .player()
                    .ok_or("The player engine was not initialized")?
                    .command_sender();
                let pip = super::pip::Controller::install(&app, &host, video_hwnd, commands)
                    .map_err(std::io::Error::other)?;
                if let Some(monitor) = startup_visibility_monitor.borrow().as_ref() {
                    monitor.on_minimized(super::pip::Controller::owner_minimize_handler(&pip));
                }
                *startup_pip.borrow_mut() = Some(pip);
                menu_actions::install_playback(&host, &app);
                *startup_session.borrow_mut() = Some(configured);
                actions.bind(&app);
                *startup_chrome.borrow_mut() = Some(app.clone_strong());
                let _controls = show_chrome(&host, &app, &startup_menu_lifecycle).await?;
                // The native host otherwise keeps Winit's default placement until
                // playback loads. That leaves idle launches and cancelled open
                // dialogs stranded at the OS-selected (often top-left) position.
                placement.schedule(host.as_weak(), app.as_weak(), owner_center);
                if welcome.is_none()
                    && managed_ticket.is_none()
                    && action_after_launch.open_file_panel(supplied_media_argument)
                {
                    app.global::<PlayerMenuActions>().invoke_open_file();
                }
                surfaces::align_realized_overlay(&host, &app).await?;
                if let Some(welcome) = &welcome {
                    show_welcome(&host, &app, welcome)?;
                }
                if managed_ticket
                    .as_ref()
                    .is_some_and(|ticket| ticket.is_waiting_for_media())
                {
                    app.hide()?;
                    // mpv retains the realized HWND while the managed player is idle.
                    host.window()
                        .with_winit_window(|window| window.set_visible(false));
                }
                if actions.is_initial_window() {
                    application_services.present_release_highlights(&app, welcome.as_ref());
                    *startup_smoke.borrow_mut() = super::package_smoke::start(&app, &host);
                }
                *startup_welcome.borrow_mut() = welcome;
                app.window().request_redraw();
                Ok::<(), Box<dyn std::error::Error>>(())
            }
            .await;
            if let Err(error) = result {
                actions.startup_failed(error);
            }
        })?;
        *self.startup_task.borrow_mut() = Some(startup);
        self.install_event_filter(owner_center);
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

    fn install_close_handler(&self, actions: WindowActions) {
        let chrome = self.chrome.clone();
        self.host.window().on_close_requested(move || {
            if !chrome.borrow().as_ref().is_some_and(|app| {
                app.global::<riina_ui::PlayerDialogState>()
                    .get_dialog_open()
            }) {
                actions.close();
            }
            slint::CloseRequestResponse::KeepWindowShown
        });
    }

    fn finish(&self) {
        if let Some(startup) = self.startup_task.borrow_mut().take() {
            startup.abort();
        }
        drop(self.smoke.borrow_mut().take());
        drop(self.visibility_monitor.borrow_mut().take());
        self.surface_alignment_timer.stop();
        self.material_policy_timer.stop();
        self.hdr_display_timer.stop();
        drop(self.menu_lifecycle.borrow_mut().take());
        let app = self.chrome.borrow_mut().take();
        close(&self.session, &self.pip_controller, app.as_ref());
        if let Some(welcome) = self.welcome_window.borrow_mut().take() {
            let _ = welcome.hide();
        }
        if let Some(app) = app {
            let _ = app.hide();
        }
        let _ = self.host.hide();
    }
}

fn install_music_mode_window(
    host: &NativeVideoHost,
    app: &PlayerWindow,
    menu: &Slot<super::menu_focus::MenuFocus>,
) {
    let saved_frame = Cell::new(None::<crate::music_mode_layout::MainWindowFrame>);
    let music_host = host.as_weak();
    let music_app = app.as_weak();
    let music_menu = menu.clone();
    app.on_music_mode_changed(move || {
        let (Some(host), Some(app)) = (music_host.upgrade(), music_app.upgrade()) else {
            return;
        };
        let window = host.window();
        let active = app
            .global::<riina_ui::PlayerMusicModeState>()
            .get_music_mode();
        // Detaching the menu enlarges the client without resizing the frame.
        // Remember the normal client before the menu changes its height.
        let entering = (active && saved_frame.get().is_none())
            .then(|| crate::music_mode_layout::MainWindowFrame::leave(window));
        if let Some(menu) = music_menu.borrow().as_ref()
            && let Err(error) = menu.set_visible(!active && !app.get_fullscreen())
        {
            if let Some(frame) = entering {
                frame.restore_placement(window);
            }
            app.set_status(error.into());
            return;
        }
        let (target, restore) = if active {
            if let Some(frame) = entering {
                saved_frame.set(Some(frame));
            }
            (
                crate::music_mode_layout::compact_size(&app, f64::from(window.scale_factor())),
                None,
            )
        } else if let Some(frame) = saved_frame.take() {
            (frame.size, Some(frame))
        } else {
            return;
        };
        window.with_winit_window(|native| {
            let (width, height) = if active {
                (260.0, 90.0)
            } else {
                (285.0, 120.0)
            };
            native.set_min_inner_size(Some(winit::dpi::LogicalSize::new(width, height)));
        });
        let current = crate::music_mode_layout::client_size(window);
        crate::window_aspect::resize_centered(
            window,
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
        if let Some(frame) = restore {
            frame.restore_placement(window);
        }
        align(window, &app);
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
    actions: WindowActions,
) {
    let quit_preference = app.as_weak();
    let close_welcome_app = app.as_weak();
    let close_welcome_host = host.as_weak();
    welcome.window().on_close_requested(move || {
        let setting = quit_preference.upgrade().is_some_and(|app| {
            app.global::<riina_ui::PlayerPreferences>()
                .get_quit_when_no_opened_window()
        });
        if actions.opens_welcome() || setting {
            actions.close();
            return slint::CloseRequestResponse::KeepWindowShown;
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

fn connect_video_surfaces(
    host: &NativeVideoHost,
    app: &PlayerWindow,
    video_hwnd: isize,
    timers: [&Rc<slint::Timer>; 3],
    overlay_hidden: &Rc<Cell<bool>>,
    monitor: &Slot<visibility::VisibilityMonitor>,
) {
    let [alignment, _, hdr_display] = timers;
    surfaces::watch_alignment(host, app, alignment, overlay_hidden);
    super::presentation::watch_hdr_display(host, app, hdr_display);
    match visibility::VisibilityMonitor::install(video_hwnd, timers) {
        Ok(watcher) => *monitor.borrow_mut() = Some(watcher),
        Err(error) => crate::diagnostics::warn!("{error}"),
    }
    let title_host = host.as_weak();
    app.on_window_title_changed(move |title| {
        if let Some(host) = title_host.upgrade() {
            host.set_display_title(title);
        }
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
