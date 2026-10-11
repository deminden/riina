//! DWM projects the existing mpv-owned video window into a separate PiP window.
use crate::{NativeVideoHost, PictureInPictureWindow, PlayerWindow};
use riina_core::{Command, pip_lifecycle, pip_preferences::WindowBehavior};
use riina_mpv::CommandSender;
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
}

impl DwmThumbnail {
    pub(crate) fn register(destination: HWND, source: HWND) -> Result<Self, String> {
        // DWM owns the live projection; libmpv keeps one decoder and one native
        // output window, while the PiP window supplies only its compact controls.
        let handle = unsafe { DwmRegisterThumbnail(destination, source) }
            .map_err(|error| format!("Could not attach the video to PiP: {error}"))?;
        let source_size = match unsafe { DwmQueryThumbnailSourceSize(handle) } {
            Ok(size) if size.cx > 0 && size.cy > 0 => size,
            Ok(_) => {
                let _ = unsafe { DwmUnregisterThumbnail(handle) };
                return Err("The video window has no drawable size".into());
            }
            Err(error) => {
                let _ = unsafe { DwmUnregisterThumbnail(handle) };
                return Err(format!(
                    "Could not inspect the video surface for PiP: {error}"
                ));
            }
        };
        Ok(Self {
            handle,
            source_size,
        })
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
        unsafe { DwmUpdateThumbnailProperties(self.handle, &properties) }
            .map_err(|error| format!("Could not update the PiP video surface: {error}"))
    }
}

impl Drop for DwmThumbnail {
    fn drop(&mut self) {
        // Registration is process-local and must be released before either HWND.
        let _ = unsafe { DwmUnregisterThumbnail(self.handle) };
    }
}

#[derive(Clone, Copy)]
struct Geometry {
    position: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
}

struct ActiveSession {
    _thumbnail: DwmThumbnail,
    host_geometry: Geometry,
    player_geometry: Geometry,
    policy: WindowBehavior,
    lifecycle: pip_lifecycle::Session,
}

pub(crate) struct Controller {
    player: slint::Weak<PlayerWindow>,
    host: slint::Weak<NativeVideoHost>,
    pip: PictureInPictureWindow,
    source: HWND,
    commands: CommandSender,
    active: Rc<RefCell<Option<ActiveSession>>>,
    entering: Rc<Cell<bool>>,
    closed: Rc<Cell<bool>>,
    _playback_timer: Timer,
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
        let entering = Rc::new(Cell::new(false));
        let closed = Rc::new(Cell::new(false));
        let player_weak = player.as_weak();
        let host_weak = host.as_weak();
        let pip_weak = pip.as_weak();
        let controller = Rc::new(RefCell::new(Self {
            player: player_weak.clone(),
            host: host_weak.clone(),
            pip,
            source: HWND(source as *mut _),
            commands: commands.clone(),
            active: active.clone(),
            entering: entering.clone(),
            closed: closed.clone(),
            _playback_timer: Timer::default(),
        }));

        let toggle_controller = controller.clone();
        player.on_toggle_picture_in_picture(move || {
            toggle(toggle_controller.clone());
        });
        let weak_player = player_weak.clone();
        host.on_toggle_picture_in_picture(move || {
            if let Some(player) = weak_player.upgrade() {
                player.invoke_toggle_picture_in_picture();
            }
        });

        let weak_player = player_weak.clone();
        let play_commands = controller.borrow().commands.clone();
        controller.borrow().pip.on_play_pause(move || {
            if let Some(player) = weak_player.upgrade() {
                let _ = play_commands.send(Command::SetPaused(!player.get_paused()));
            }
        });
        let skip_commands = controller.borrow().commands.clone();
        controller.borrow().pip.on_skip(move |seconds| {
            let _ = skip_commands.send(Command::Seek {
                seconds: f64::from(seconds),
                absolute: false,
            });
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

        let weak_controller = Rc::downgrade(&controller);
        let weak_pip = pip_weak;
        let playback_timer = Timer::default();
        playback_timer.start(TimerMode::Repeated, Duration::from_millis(250), move || {
            let Some(controller) = weak_controller.upgrade() else {
                return;
            };
            let (player_weak, is_active, is_entering, is_closed) = {
                let controller = controller.borrow();
                (
                    controller.player.clone(),
                    controller.active.borrow().is_some(),
                    controller.entering.get(),
                    controller.closed.get(),
                )
            };
            if is_closed {
                return;
            }
            let Some(player) = player_weak.upgrade() else {
                return;
            };
            let minimized = player
                .window()
                .with_winit_window(|window| window.is_minimized().unwrap_or(false))
                .unwrap_or(false);
            let phase = if is_entering {
                pip_lifecycle::Phase::Entering
            } else if is_active {
                pip_lifecycle::Phase::Active
            } else {
                pip_lifecycle::Phase::Off
            };
            if let Ok(settings) = crate::pip_preferences::current_player(&player)
                && let Some(target) = pip_lifecycle::minimize_toggle_target(
                    settings,
                    player.get_video_width() > 0.0,
                    minimized,
                    phase,
                )
            {
                if target {
                    toggle(controller);
                } else {
                    exit(controller);
                }
                return;
            }
            if is_active && let Some(pip) = weak_pip.upgrade() {
                pip.set_media_title(player.get_media_title());
                pip.set_paused(player.get_paused());
            }
        });
        controller.borrow_mut()._playback_timer = playback_timer;
        Ok(controller)
    }

    pub(crate) fn shutdown(controller: Rc<RefCell<Self>>) {
        controller.borrow().closed.set(true);
        exit(controller);
    }
}

fn toggle(controller: Rc<RefCell<Controller>>) {
    if controller.borrow().active.borrow().is_some() {
        exit(controller);
        return;
    }
    if controller.borrow().closed.get() || controller.borrow().entering.replace(true) {
        return;
    }

    let (player_weak, host_weak, pip_weak, source, active, entering, closed, commands) = {
        let controller = controller.borrow();
        (
            controller.player.clone(),
            controller.host.clone(),
            controller.pip.as_weak(),
            controller.source,
            controller.active.clone(),
            controller.entering.clone(),
            controller.closed.clone(),
            controller.commands.clone(),
        )
    };
    let _ = slint::spawn_local(async move {
        let result = async {
            let player = player_weak
                .upgrade()
                .ok_or_else(|| "The player window has closed".to_owned())?;
            let host = host_weak
                .upgrade()
                .ok_or_else(|| "The video window has closed".to_owned())?;
            if !player.get_loaded() || player.get_video_width() <= 0.0 {
                return Err("Picture in Picture requires a loaded video".to_owned());
            }
            let settings = crate::pip_preferences::current_player(&player)?;
            let (host_geometry, player_geometry) = capture_geometry(&host, &player)?;
            let minimized = player
                .window()
                .with_winit_window(|window| window.is_minimized().unwrap_or(false))
                .unwrap_or(false);
            let (lifecycle, effects) = pip_lifecycle::Session::enter(
                settings,
                pip_lifecycle::MainWindowState {
                    fullscreen: player.get_fullscreen(),
                    minimized,
                },
            );
            let behavior = effects.window_behavior;
            let pip = pip_weak
                .upgrade()
                .ok_or_else(|| "The PiP window has closed".to_owned())?;
            pip.set_media_title(player.get_media_title());
            pip.set_paused(player.get_paused());
            pip.show().map_err(|error| error.to_string())?;
            let destination = pip
                .window()
                .winit_window()
                .await
                .map_err(|error| error.to_string())?;
            if closed.get() {
                return Err("The player is closing".to_owned());
            }
            destination.set_window_level(WindowLevel::AlwaysOnTop);
            let destination_hwnd = raw_hwnd(&destination)?;
            let thumbnail = DwmThumbnail::register(destination_hwnd, source)?;
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

            if behavior != WindowBehavior::DoNothing {
                // Keep the renderer HWND alive and drawable while its pixels are
                // projected; the player-level policy only affects its chrome.
                host.window().with_winit_window(|window| {
                    window.set_outer_position(PhysicalPosition::new(-32_000, -32_000));
                });
                match behavior {
                    WindowBehavior::Hide => {
                        player.window().hide().map_err(|error| error.to_string())?
                    }
                    WindowBehavior::Minimize => {
                        player
                            .window()
                            .with_winit_window(|window| window.set_minimized(true));
                    }
                    WindowBehavior::DoNothing => {}
                }
            }
            if effects.pause_playback {
                let _ = commands.send(Command::SetPaused(true));
            }
            Ok(ActiveSession {
                _thumbnail: thumbnail,
                host_geometry,
                player_geometry,
                policy: behavior,
                lifecycle,
            })
        }
        .await;
        entering.set(false);
        match result {
            Ok(session) => {
                if let Some(host) = host_weak.upgrade() {
                    host.set_picture_in_picture_active(true);
                }
                *active.borrow_mut() = Some(session);
            }
            Err(error) => {
                if let Some(player) = player_weak.upgrade() {
                    player.set_status(error.into());
                }
                if let Some(pip) = pip_weak.upgrade() {
                    let _ = pip.hide();
                }
            }
        }
    });
}

fn exit(controller: Rc<RefCell<Controller>>) {
    let (pip, player_weak, host_weak, active, commands) = {
        let controller = controller.borrow();
        (
            controller.pip.as_weak(),
            controller.player.clone(),
            controller.host.clone(),
            controller.active.clone(),
            controller.commands.clone(),
        )
    };
    let Some(session) = active.borrow_mut().take() else {
        return;
    };
    let effects = session.lifecycle.exit();
    if let Some(pip) = pip.upgrade() {
        let _ = pip.hide();
    }
    if let Some(host) = host_weak.upgrade() {
        host.set_picture_in_picture_active(false);
        host.window().with_winit_window(|window| {
            let _ = window.request_inner_size(session.host_geometry.size);
            window.set_outer_position(session.host_geometry.position);
        });
    }
    if let Some(player) = player_weak.upgrade() {
        player.window().with_winit_window(|window| {
            let _ = window.request_inner_size(session.player_geometry.size);
            window.set_outer_position(session.player_geometry.position);
            if session.policy == WindowBehavior::Minimize {
                window.set_minimized(false);
            }
        });
        if effects.restore_main_window {
            let _ = player.window().show();
        }
        player
            .window()
            .with_winit_window(|window| window.focus_window());
    }
    if effects.pause_playback {
        let _ = commands.send(Command::SetPaused(true));
    }
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

#[cfg(test)]
mod tests {
    use super::fit_source_in_rect;
    use windows::Win32::Foundation::{RECT, SIZE};

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
}
