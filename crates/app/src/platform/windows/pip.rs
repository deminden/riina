//! DWM projects the existing mpv-owned video window into a separate PiP window.
use crate::{NativeVideoHost, PictureInPictureWindow, PlayerWindow};
use riina_core::{Command, pip_lifecycle, pip_preferences::WindowBehavior};
use riina_mpv::CommandSender;
use riina_ui::PlayerMenuActions;
use slint::winit_030::winit::{
    dpi::{PhysicalPosition, PhysicalSize},
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
    window::{Window, WindowLevel},
};
use slint::{ComponentHandle, Timer, TimerMode, winit_030::WinitWindowAccessor};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};
use windows::{
    Win32::{
        Foundation::{HWND, RECT, SIZE},
        Graphics::Dwm::{
            DWM_THUMBNAIL_PROPERTIES, DWM_TNP_RECTDESTINATION, DWM_TNP_SOURCECLIENTAREAONLY,
            DWM_TNP_VISIBLE, DwmQueryThumbnailSourceSize, DwmRegisterThumbnail,
            DwmUnregisterThumbnail, DwmUpdateThumbnailProperties,
        },
    },
    core::BOOL,
};

pub(crate) struct DwmThumbnail {
    handle: isize,
    source_size: SIZE,
    destination_rect: Option<RECT>,
}

impl DwmThumbnail {
    pub(crate) fn register(destination: HWND, source: HWND) -> Result<Self, String> {
        // DWM owns the live projection; libmpv keeps one decoder and one native
        // output window, while the PiP window supplies only its compact controls.
        // SAFETY: both HWNDs belong to the retained PiP and playback windows.
        let handle = unsafe { DwmRegisterThumbnail(destination, source) }
            .map_err(|error| format!("Could not attach the video to PiP: {error}"))?;
        let mut thumbnail = Self {
            handle,
            source_size: SIZE::default(),
            destination_rect: None,
        };
        // SAFETY: the thumbnail owns the registration until this query returns.
        thumbnail.source_size = unsafe { DwmQueryThumbnailSourceSize(thumbnail.handle) }
            .map_err(|error| format!("Could not inspect the video surface for PiP: {error}"))?;
        if thumbnail.source_size.cx <= 0 || thumbnail.source_size.cy <= 0 {
            return Err("The video window has no drawable size".into());
        }
        Ok(thumbnail)
    }

    pub(crate) fn source_size(&self) -> SIZE {
        self.source_size
    }

    pub(crate) fn set_visible(&self, destination: RECT, visible: bool) -> Result<(), String> {
        let properties = DWM_THUMBNAIL_PROPERTIES {
            dwFlags: DWM_TNP_RECTDESTINATION | DWM_TNP_VISIBLE | DWM_TNP_SOURCECLIENTAREAONLY,
            rcDestination: destination,
            opacity: u8::MAX,
            fVisible: BOOL(i32::from(visible)),
            fSourceClientAreaOnly: BOOL(1),
            ..Default::default()
        };
        // SAFETY: this owner retains the registration and the properties are initialized.
        unsafe { DwmUpdateThumbnailProperties(self.handle, &properties) }
            .map_err(|error| format!("Could not update the PiP video surface: {error}"))
    }

    fn refresh_layout(&mut self, width: u32, height: u32, scale_factor: f64) -> Result<(), String> {
        // SAFETY: the thumbnail handle is retained for this synchronous layout query.
        let source_size = unsafe { DwmQueryThumbnailSourceSize(self.handle) }
            .map_err(|error| format!("Could not inspect the PiP video surface: {error}"))?;
        if source_size.cx <= 0 || source_size.cy <= 0 {
            return Ok(());
        }
        let video_height = height.saturating_sub((54.0 * scale_factor).round() as u32);
        if video_height == 0 {
            return Ok(());
        }
        let destination = fit_source_in_rect(source_size, width, video_height);
        if same_rect(self.destination_rect, Some(destination)) && self.source_size == source_size {
            return Ok(());
        }
        self.set_visible(destination, true)?;
        self.source_size = source_size;
        self.destination_rect = Some(destination);
        Ok(())
    }
}

impl Drop for DwmThumbnail {
    fn drop(&mut self) {
        // SAFETY: this owner releases its registration once, before either HWND is destroyed.
        if let Err(error) = unsafe { DwmUnregisterThumbnail(self.handle) } {
            crate::diagnostics::warn!(
                "Could not release the Picture-in-Picture thumbnail: {error}"
            );
        }
    }
}

#[derive(Clone, Copy)]
struct Geometry {
    position: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
}

struct ActiveSession {
    thumbnail: DwmThumbnail,
    restore: Option<(Geometry, Geometry)>,
    policy: WindowBehavior,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PendingEntry {
    Manual,
    Minimize,
    Cancelled,
}

pub(crate) struct Controller {
    player: slint::Weak<PlayerWindow>,
    host: slint::Weak<NativeVideoHost>,
    pip: PictureInPictureWindow,
    source: HWND,
    commands: CommandSender,
    active: Rc<RefCell<Option<ActiveSession>>>,
    entering: Rc<Cell<Option<PendingEntry>>>,
    closed: Rc<Cell<bool>>,
    acrylic_monitor_started: Rc<Cell<bool>>,
    playback_timer: Timer,
}

impl Controller {
    pub(crate) fn install(
        player: &PlayerWindow,
        host: &NativeVideoHost,
        source: isize,
        commands: CommandSender,
    ) -> Result<Rc<RefCell<Self>>, String> {
        let pip = PictureInPictureWindow::new().map_err(|error| error.to_string())?;
        let active = Rc::new(RefCell::new(None));
        let entering = Rc::new(Cell::new(None));
        let closed = Rc::new(Cell::new(false));
        let acrylic_monitor_started = Rc::new(Cell::new(false));
        let player_weak = player.as_weak();
        let host_weak = host.as_weak();
        let controller = Rc::new(RefCell::new(Self {
            player: player_weak.clone(),
            host: host_weak.clone(),
            pip,
            source: HWND(source as *mut _),
            commands: commands.clone(),
            active: active.clone(),
            entering: entering.clone(),
            closed: closed.clone(),
            acrylic_monitor_started: acrylic_monitor_started.clone(),
            playback_timer: Timer::default(),
        }));

        let toggle_controller = controller.clone();
        player
            .global::<PlayerMenuActions>()
            .on_toggle_picture_in_picture(move || {
                toggle(toggle_controller.clone());
            });
        let weak_player = player_weak.clone();
        host.on_toggle_picture_in_picture(move || {
            if let Some(player) = weak_player.upgrade() {
                player
                    .global::<PlayerMenuActions>()
                    .invoke_toggle_picture_in_picture();
            }
        });

        let weak_player = player_weak.clone();
        let play_commands = controller.borrow().commands.clone();
        controller.borrow().pip.on_play_pause(move || {
            if let Some(player) = weak_player.upgrade()
                && let Err(error) = play_commands.send(Command::SetPaused(!player.get_paused()))
            {
                crate::diagnostics::report(&player, error);
            }
        });
        let stop_commands = controller.borrow().commands.clone();
        controller.borrow().pip.on_stop(move || {
            if let Err(error) = stop_commands.send(Command::SetPaused(true)) {
                crate::diagnostics::warn!("Could not pause Picture-in-Picture playback: {error}");
            }
        });
        let skip_commands = controller.borrow().commands.clone();
        controller.borrow().pip.on_skip(move |seconds| {
            if let Err(error) = skip_commands.send(Command::Seek {
                seconds: f64::from(seconds),
                absolute: false,
            }) {
                crate::diagnostics::warn!("Could not seek in Picture-in-Picture: {error}");
            }
        });
        let weak_controller = Rc::downgrade(&controller);
        controller.borrow().pip.on_exit_picture_in_picture(move || {
            if let Some(controller) = weak_controller.upgrade() {
                exit(controller);
            }
        });
        let weak_controller = Rc::downgrade(&controller);
        controller
            .borrow()
            .pip
            .window()
            .on_close_requested(move || {
                if let Some(controller) = weak_controller.upgrade() {
                    exit(controller);
                }
                slint::CloseRequestResponse::HideWindow
            });

        Self::install_playback_watch(&controller, player);
        Ok(controller)
    }

    fn install_playback_watch(controller: &Rc<RefCell<Self>>, player: &PlayerWindow) {
        let weak_controller = Rc::downgrade(controller);
        let weak_pip = controller.borrow().pip.as_weak();
        let playback_timer = Timer::default();
        playback_timer.start(TimerMode::Repeated, Duration::from_millis(250), move || {
            let Some(controller) = weak_controller.upgrade() else {
                return;
            };
            let (player_weak, is_active) = {
                let controller = controller.borrow();
                (
                    controller.player.clone(),
                    controller.active.borrow().is_some(),
                )
            };
            let Some(player) = player_weak.upgrade() else {
                return;
            };
            if is_active && let Some(pip) = weak_pip.upgrade() {
                pip.set_media_title(player.get_media_title());
                pip.set_paused(player.get_paused());
                let layout = pip
                    .window()
                    .with_winit_window(|window| (window.inner_size(), window.scale_factor()));
                let refresh_error = if let Some((size, scale_factor)) = layout {
                    let controller = controller.borrow();
                    let mut active = controller.active.borrow_mut();
                    active.as_mut().and_then(|session| {
                        session
                            .thumbnail
                            .refresh_layout(size.width, size.height, scale_factor)
                            .err()
                    })
                } else {
                    None
                };
                if let Some(error) = refresh_error
                    && let Some(player) = player_weak.upgrade()
                {
                    player.set_status(error.into());
                }
            }
        });
        controller.borrow_mut().playback_timer = playback_timer;
        controller
            .borrow()
            .update_playback_timer(player.get_picture_in_picture_active());
        let weak_controller = Rc::downgrade(controller);
        player.on_picture_in_picture_monitor_changed(move |needed| {
            if let Some(controller) = weak_controller.upgrade() {
                controller.borrow().update_playback_timer(needed);
            }
        });
    }

    pub(super) fn owner_minimize_handler(
        controller: &Rc<RefCell<Self>>,
    ) -> impl Fn(bool) + 'static {
        let weak = Rc::downgrade(controller);
        move |minimized| {
            let weak = weak.clone();
            // Placement notifications run inside Win32's window procedure.
            if let Err(error) = slint::spawn_local(async move {
                if let Some(controller) = weak.upgrade() {
                    owner_minimized(controller, minimized);
                }
            }) {
                crate::diagnostics::warn!("Could not handle player minimization: {error}");
            }
        }
    }

    fn update_playback_timer(&self, needed: bool) {
        match (needed && !self.closed.get(), self.playback_timer.running()) {
            (true, false) => self.playback_timer.restart(),
            (false, true) => self.playback_timer.stop(),
            _ => {}
        }
    }

    pub(crate) fn shutdown(controller: Rc<RefCell<Self>>) {
        {
            let state = controller.borrow();
            state.closed.set(true);
            state.playback_timer.stop();
        }
        exit(controller);
    }
}

fn toggle(controller: Rc<RefCell<Controller>>) {
    if controller.borrow().active.borrow().is_some() {
        exit(controller);
        return;
    }
    if controller.borrow().entering.get().is_some() {
        controller
            .borrow()
            .entering
            .set(Some(PendingEntry::Cancelled));
        return;
    }
    enter(controller, PendingEntry::Manual);
}

fn owner_minimized(controller: Rc<RefCell<Controller>>, minimized: bool) {
    let target = {
        let controller = controller.borrow();
        let (Some(player), Some(host)) = (controller.player.upgrade(), controller.host.upgrade())
        else {
            return;
        };
        if controller.closed.get() || owner_is_minimized(&host) != minimized {
            return;
        }
        if !minimized && controller.entering.get() == Some(PendingEntry::Minimize) {
            controller.entering.set(Some(PendingEntry::Cancelled));
            return;
        }
        let phase = if controller.entering.get().is_some() {
            pip_lifecycle::Phase::Entering
        } else if controller.active.borrow().is_some() {
            pip_lifecycle::Phase::Active
        } else {
            pip_lifecycle::Phase::Off
        };
        let Ok(settings) = crate::pip_preferences::current_player(&player) else {
            return;
        };
        pip_lifecycle::minimize_toggle_target(
            settings,
            player.get_video_width() > 0.0,
            minimized,
            phase,
        )
    };
    match target {
        Some(true) => enter(controller, PendingEntry::Minimize),
        Some(false) => exit(controller),
        None => {}
    }
}

fn owner_is_minimized(host: &NativeVideoHost) -> bool {
    host.window()
        .with_winit_window(|window| window.is_minimized().unwrap_or(false))
        .unwrap_or(false)
}

fn enter(controller: Rc<RefCell<Controller>>, origin: PendingEntry) {
    if controller.borrow().closed.get() || controller.borrow().entering.get().is_some() {
        return;
    }
    controller.borrow().entering.set(Some(origin));

    let (
        player_weak,
        host_weak,
        pip_weak,
        source,
        active,
        entering,
        closed,
        acrylic_monitor_started,
        commands,
    ) = {
        let controller = controller.borrow();
        (
            controller.player.clone(),
            controller.host.clone(),
            controller.pip.as_weak(),
            controller.source,
            controller.active.clone(),
            controller.entering.clone(),
            controller.closed.clone(),
            controller.acrylic_monitor_started.clone(),
            controller.commands.clone(),
        )
    };
    let scheduled = slint::spawn_local(async move {
        let result = async {
            let cancelled = || closed.get() || entering.get() == Some(PendingEntry::Cancelled);
            if cancelled() {
                return Ok(None);
            }
            let player = player_weak
                .upgrade()
                .ok_or_else(|| "The player window has closed".to_owned())?;
            let host = host_weak
                .upgrade()
                .ok_or_else(|| "The video window has closed".to_owned())?;
            if !player.get_loaded() || player.get_video_width() <= 0.0 {
                return Err("Picture in Picture requires a loaded video".to_owned());
            }
            if player
                .global::<riina_ui::PlayerDialogState>()
                .get_dialog_open()
            {
                return Ok(None);
            }
            let settings = crate::pip_preferences::current_player(&player)?;
            let (lifecycle, effects) = pip_lifecycle::Session::enter(
                settings,
                pip_lifecycle::MainWindowState {
                    fullscreen: player.get_fullscreen() || player.get_entering_fullscreen(),
                    minimized: owner_is_minimized(&host),
                },
            );
            let behavior = effects.window_behavior;
            let restore = lifecycle
                .should_restore_main_window()
                .then(|| capture_geometry(&host, &player))
                .transpose()?;
            let pip = pip_weak
                .upgrade()
                .ok_or_else(|| "The PiP window has closed".to_owned())?;
            pip.set_media_title(player.get_media_title());
            pip.set_paused(player.get_paused());
            pip.show().map_err(|error| error.to_string())?;
            if !acrylic_monitor_started.replace(true) {
                crate::platform::enable_pip_acrylic(&pip);
            }
            let destination = pip
                .window()
                .winit_window()
                .await
                .map_err(|error| error.to_string())?;
            if cancelled()
                || player
                    .global::<riina_ui::PlayerDialogState>()
                    .get_dialog_open()
                || (origin == PendingEntry::Minimize && !owner_is_minimized(&host))
            {
                return Ok(None);
            }
            destination.set_window_level(WindowLevel::AlwaysOnTop);
            let destination_hwnd = raw_hwnd(&destination)?;
            let mut thumbnail = DwmThumbnail::register(destination_hwnd, source)?;
            let client = destination.inner_size();
            let video_bottom = client
                .height
                .saturating_sub((54.0 * destination.scale_factor()).round() as u32);
            if video_bottom == 0 {
                return Err("The PiP window is too small to show video".to_owned());
            }
            let destination_rect =
                fit_source_in_rect(thumbnail.source_size(), client.width, video_bottom);
            thumbnail.set_visible(destination_rect, true)?;
            thumbnail.destination_rect = Some(destination_rect);

            apply_entry_policy(&player, &host, &commands, behavior, effects.pause_playback)?;
            Ok(Some(ActiveSession {
                thumbnail,
                restore,
                policy: behavior,
            }))
        }
        .await;
        entering.set(None);
        match result {
            Ok(Some(session)) => {
                if let Some(host) = host_weak.upgrade() {
                    host.set_picture_in_picture_active(true);
                }
                *active.borrow_mut() = Some(session);
                if let Some(player) = player_weak.upgrade() {
                    crate::player::presentation::set_picture_in_picture(&player, true);
                }
            }
            result => {
                if let Err(error) = result
                    && !closed.get()
                    && let Some(player) = player_weak.upgrade()
                {
                    crate::diagnostics::report(&player, error);
                }
                if let Some(pip) = pip_weak.upgrade() {
                    // Continue releasing session resources if the window is closing.
                    let _ = pip.hide();
                }
            }
        }
    });
    if let Err(error) = scheduled {
        controller.borrow().entering.set(None);
        crate::diagnostics::warn!("Could not schedule Picture-in-Picture entry: {error}");
    }
}

fn apply_entry_policy(
    player: &PlayerWindow,
    host: &NativeVideoHost,
    commands: &CommandSender,
    behavior: WindowBehavior,
    pause_playback: bool,
) -> Result<(), String> {
    let was_paused = player.get_paused();
    if pause_playback {
        commands
            .send(Command::SetPaused(true))
            .map_err(|error| error.to_string())?;
    }
    // Do the fallible player-window request before parking the renderer HWND,
    // so a rejected PiP entry cannot strand video off-screen.
    match behavior {
        WindowBehavior::Hide => {
            if let Err(error) = player.window().hide() {
                // A rejected hide may leave the window state uncertain. Restore
                // the visible entry surface and the pause state before aborting.
                if let Err(restore_error) = player.window().show() {
                    crate::diagnostics::warn!(
                        "Could not restore the player after rejected Picture-in-Picture entry: {restore_error}"
                    );
                }
                if pause_playback
                    && !was_paused
                    && let Err(restore_error) = commands.send(Command::SetPaused(false))
                {
                    crate::diagnostics::warn!(
                        "Could not restore playback after rejected Picture-in-Picture entry: {restore_error}"
                    );
                }
                return Err(error.to_string());
            }
        }
        WindowBehavior::Minimize => {
            player
                .window()
                .with_winit_window(|window| window.set_minimized(true));
        }
        WindowBehavior::DoNothing => return Ok(()),
    }
    // Keep the renderer HWND alive and drawable while its pixels are
    // projected; the player-level policy only affects its chrome.
    host.window().with_winit_window(|window| {
        window.set_outer_position(PhysicalPosition::new(-32_000, -32_000));
    });
    Ok(())
}

fn exit(controller: Rc<RefCell<Controller>>) {
    let (pip, player_weak, host_weak, active, closing) = {
        let controller = controller.borrow();
        (
            controller.pip.as_weak(),
            controller.player.clone(),
            controller.host.clone(),
            controller.active.clone(),
            controller.closed.get(),
        )
    };
    let Some(session) = active.borrow_mut().take() else {
        return;
    };
    let ActiveSession {
        thumbnail,
        restore,
        policy,
    } = session;
    drop(thumbnail);
    if let Some(pip) = pip.upgrade() {
        // Continue releasing session resources if the window is closing.
        let _ = pip.hide();
    }
    if let Some(host) = host_weak.upgrade() {
        host.set_picture_in_picture_active(false);
        if !closing && let Some((geometry, _)) = restore {
            host.window()
                .with_winit_window(|window| restore_geometry(window, geometry));
        }
    }
    if let Some(player) = player_weak.upgrade() {
        if !closing {
            if let Some((_, geometry)) = restore {
                player.window().with_winit_window(|window| {
                    restore_geometry(window, geometry);
                    if policy == WindowBehavior::Minimize {
                        window.set_minimized(false);
                    }
                });
                if policy == WindowBehavior::Hide
                    && let Err(error) = player.window().show()
                {
                    crate::diagnostics::warn!(
                        "Could not restore the player after closing Picture-in-Picture: {error}"
                    );
                }
            }
            player
                .window()
                .with_winit_window(super::menu_focus::activate_without_menu_key);
        }
        crate::player::presentation::set_picture_in_picture(&player, false);
    }
}

fn restore_geometry(window: &Window, geometry: Geometry) {
    // Native geometry notifications confirm the restored size.
    let _ = window.request_inner_size(geometry.size);
    window.set_outer_position(geometry.position);
}

fn capture_geometry(
    host: &NativeVideoHost,
    player: &PlayerWindow,
) -> Result<(Geometry, Geometry), String> {
    fn capture(window: &slint::Window) -> Result<Geometry, String> {
        window
            .with_winit_window(|window| {
                Ok(Geometry {
                    position: window.outer_position().map_err(|error| error.to_string())?,
                    size: window.inner_size(),
                })
            })
            .ok_or_else(|| "A player window handle is unavailable".to_owned())?
    }
    Ok((capture(host.window())?, capture(player.window())?))
}

fn raw_hwnd(window: &Window) -> Result<HWND, String> {
    let handle = window.window_handle().map_err(|error| error.to_string())?;
    match handle.as_raw() {
        RawWindowHandle::Win32(handle) => Ok(HWND(handle.hwnd.get() as *mut _)),
        _ => Err("The PiP window requires a native Windows handle".into()),
    }
}

fn fit_source_in_rect(source: SIZE, width: u32, height: u32) -> RECT {
    let source_ratio = source.cx as f64 / source.cy as f64;
    let canvas_ratio = width as f64 / height as f64;
    let (fitted_width, fitted_height) = if canvas_ratio > source_ratio {
        ((height as f64 * source_ratio).round() as u32, height)
    } else {
        (width, (width as f64 / source_ratio).round() as u32)
    };
    let left = (width - fitted_width) / 2;
    let top = (height - fitted_height) / 2;
    RECT {
        left: left as i32,
        top: top as i32,
        right: (left + fitted_width) as i32,
        bottom: (top + fitted_height) as i32,
    }
}

fn same_rect(left: Option<RECT>, right: Option<RECT>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => {
            left.left == right.left
                && left.top == right.top
                && left.right == right.right
                && left.bottom == right.bottom
        }
        (None, None) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{Controller, NativeVideoHost, PlayerWindow, fit_source_in_rect, same_rect, toggle};
    use windows::Win32::Foundation::{RECT, SIZE};

    #[test]
    fn picture_in_picture_watch_runs_only_for_an_active_preview()
    -> Result<(), Box<dyn std::error::Error>> {
        use slint::ComponentHandle;
        use std::time::Duration;

        i_slint_backend_testing::init_no_event_loop();
        let player = PlayerWindow::new()?;
        let host = NativeVideoHost::new()?;
        let (commands, _) = riina_mpv::CommandSender::recording(1);
        let controller = Controller::install(&player, &host, 0, commands)?;
        assert!(!controller.borrow().playback_timer.running());

        let preferences = player.global::<riina_ui::PlayerPreferences>();
        preferences.set_pip_toggle_by_minimizing(true);
        i_slint_backend_testing::mock_elapsed_time(Duration::ZERO);
        assert!(!controller.borrow().playback_timer.running());

        preferences.set_pip_toggle_by_minimizing(false);
        i_slint_backend_testing::mock_elapsed_time(Duration::ZERO);
        assert!(!controller.borrow().playback_timer.running());

        player.set_picture_in_picture_active(true);
        i_slint_backend_testing::mock_elapsed_time(Duration::ZERO);
        assert!(controller.borrow().playback_timer.running());

        Controller::shutdown(controller.clone());
        assert!(!controller.borrow().playback_timer.running());
        Ok(())
    }

    #[test]
    fn rejected_picture_in_picture_entry_releases_the_pending_state()
    -> Result<(), Box<dyn std::error::Error>> {
        i_slint_backend_testing::init_no_event_loop();
        let player = PlayerWindow::new()?;
        let host = NativeVideoHost::new()?;
        let (commands, _) = riina_mpv::CommandSender::recording(1);
        let controller = Controller::install(&player, &host, 0, commands)?;

        toggle(controller.clone());

        assert!(controller.borrow().entering.get().is_none());
        assert!(controller.borrow().active.borrow().is_none());
        Ok(())
    }

    #[test]
    fn picture_in_picture_video_is_aspect_fitted_inside_controls_area() {
        let rect = fit_source_in_rect(SIZE { cx: 1920, cy: 1080 }, 800, 400);

        assert_eq!(
            rect,
            RECT {
                left: 44,
                top: 0,
                right: 755,
                bottom: 400
            }
        );
    }

    #[test]
    fn picture_in_picture_video_is_centered_with_vertical_letterboxing() {
        let rect = fit_source_in_rect(SIZE { cx: 4, cy: 3 }, 400, 400);

        assert_eq!(
            rect,
            RECT {
                left: 0,
                top: 50,
                right: 400,
                bottom: 350
            }
        );
    }

    #[test]
    fn destination_rect_comparison_detects_resize_and_source_layout_changes() {
        let initial = fit_source_in_rect(SIZE { cx: 16, cy: 9 }, 800, 400);
        let resized = fit_source_in_rect(SIZE { cx: 16, cy: 9 }, 600, 400);
        let source_changed = fit_source_in_rect(SIZE { cx: 4, cy: 3 }, 800, 400);

        assert!(same_rect(Some(initial), Some(initial)));
        assert!(!same_rect(Some(initial), Some(resized)));
        assert!(!same_rect(Some(initial), Some(source_changed)));
    }
}
