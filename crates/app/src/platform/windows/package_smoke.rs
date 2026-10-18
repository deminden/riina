//! Opt-in integration driver used only by the extracted-package release check.
use crate::{NativeVideoHost, PlayerWindow};
use slint::winit_030::winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::{ComponentHandle, Model, winit_030::WinitWindowAccessor};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use windows::Win32::{
    Foundation::{HWND, POINT, RECT},
    Graphics::Gdi::ClientToScreen,
    UI::WindowsAndMessaging::{
        GetClientRect, GetMenu, GetMenuItemCount, GetMenuState, GetMenuStringW, GetSubMenu,
        IsWindowVisible, MF_BYPOSITION, MF_DISABLED, MF_GRAYED,
    },
};

pub(super) struct SmokeRun {
    _timer: slint::Timer,
}

struct Runner {
    stage: u8,
    started: Instant,
    next: Instant,
    initial: Option<Surfaces>,
    volume: f32,
    position: f32,
    progressed: bool,
    checks: Vec<&'static str>,
}

impl Runner {
    fn new() -> Self {
        let started = Instant::now();
        Self {
            stage: 0,
            started,
            next: started + Duration::from_secs(2),
            initial: None,
            volume: 0.0,
            position: 0.0,
            progressed: false,
            checks: Vec::new(),
        }
    }

    fn tick(&mut self, app: &PlayerWindow, host: &NativeVideoHost) -> Result<bool, String> {
        if self.started.elapsed() > Duration::from_secs(40) {
            return Err(format!(
                "GUI smoke timed out at stage {}: {}",
                self.stage,
                app.get_status()
            ));
        }
        self.progressed |= self.stage > 0 && (app.get_position() - self.position).abs() > 0.1;
        match self.stage {
            0 => {
                if !app.get_loaded() || app.get_paused() || app.get_position() <= 0.1 {
                    return Ok(false);
                }
                self.initial = Some(surfaces(app, host)?);
                check_audio_menu(host)?;
                self.volume = app.get_volume();
                self.position = app.get_position();
                let path = crate::platform::preferences_path().map_err(|e| e.to_string())?;
                crate::preference_store::update(
                    &path,
                    &riina_core::Preferences::default(),
                    |prefs| {
                        prefs.video_threads = 7;
                        prefs.load_icc_profile = false;
                        prefs.audio.audio_threads = 3;
                        prefs.network.user_agent = "Riina GUI smoke".into();
                        prefs.hdr_tone_mapping.algorithm =
                            riina_core::hdr_preferences::Algorithm::Hable;
                        Ok(())
                    },
                )?;
                app.set_control_bar_position_horizontal(0.45);
                app.invoke_layout_settings_changed();
                app.set_trailing_sidebar_width(app.get_trailing_sidebar_width() + 8.0);
                app.invoke_sidebar_resize_finished();
                check_preferences()?;
                self.checks.push("preference_writers_preserve_settings");
                app.invoke_adjust_volume(-5.0);
                self.checks.push("playing_native_video_and_audio_menu");
            }
            1 => {
                if (app.get_volume() - (self.volume - 5.0).max(0.0)).abs() > 0.1 {
                    return Ok(false);
                }
                if self.initial.as_ref() != Some(&surfaces(app, host)?) {
                    return Err("Volume update changed client bounds or menu identity".into());
                }
                app.invoke_adjust_volume(5.0);
                self.checks.push("volume_preserves_surfaces");
            }
            2 => app.invoke_double_click_action(
                riina_core::control_preferences::MouseAction::Fullscreen.index(),
            ),
            3 => {
                if !app.get_fullscreen() {
                    return Ok(false);
                }
                let snapshot = surfaces(app, host)?;
                if snapshot.menu != 0
                    || snapshot.controls[2] != snapshot.video[2]
                    || snapshot.controls[3] + 1 != snapshot.video[3]
                {
                    return Err("Fullscreen overlay does not preserve the alpha workaround".into());
                }
                app.set_controls_visible(false);
                self.checks.push("double_click_fullscreen");
            }
            4 => {
                app.set_controls_visible(true);
                app.invoke_activity();
                app.window().request_redraw();
                eprintln!("riina-gui-smoke: fullscreen controller revealed");
            }
            5 => {
                if !app.get_controls_visible() || !app.get_controls_rendered() {
                    return Err("Controller did not reveal".into());
                }
                surfaces(app, host)?;
                self.checks.push("controller_reveal_keeps_native_surface");
                app.invoke_toggle_fullscreen();
            }
            6 => {
                if app.get_fullscreen() {
                    return Ok(false);
                }
                if self.initial.as_ref() != Some(&surfaces(app, host)?) {
                    return Err("Fullscreen exit did not restore bounds/menu".into());
                }
                if !self.progressed {
                    return Err("Playback stopped progressing during GUI smoke".into());
                }
                check_preferences()?;
                self.checks.push("fullscreen_exit_and_playback_progress");
                return Ok(true);
            }
            _ => return Err("Unexpected GUI smoke stage".into()),
        }
        self.stage += 1;
        self.next = Instant::now() + Duration::from_millis(600);
        Ok(false)
    }
}

pub(super) fn start(app: &PlayerWindow, host: &NativeVideoHost) -> Option<SmokeRun> {
    let output = PathBuf::from(std::env::var_os("RIINA_GUI_SMOKE_REPORT")?);
    if !output.is_absolute() {
        eprintln!("GUI smoke report must be absolute");
        let _ = slint::quit_event_loop();
        return None;
    }
    let weak = app.as_weak();
    let owner = host.as_weak();
    let timer = slint::Timer::default();
    let mut run = Runner::new();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(100),
        move || {
            let (Some(app), Some(host)) = (weak.upgrade(), owner.upgrade()) else {
                return;
            };
            if Instant::now() < run.next {
                return;
            }
            let result = match run.tick(&app, &host) {
                Ok(false) => return,
                Ok(true) => Ok(()),
                Err(error) => Err(error),
            };
            let report = serde_json::json!({
                "success": result.is_ok(), "error": result.err(), "checks": run.checks,
                "executable": std::env::current_exe().ok(), "pid": std::process::id(),
                "title": app.get_window_title().as_str(), "physical_display_verified": false,
                "elapsed_ms": run.started.elapsed().as_millis(),
            });
            if let Err(error) = std::fs::write(&output, serde_json::to_vec_pretty(&report).unwrap())
            {
                eprintln!("Could not save GUI smoke evidence: {error}");
            }
            // The runner checks process exit separately: a stalled decoder shutdown
            // must fail even when all playback and controls checks completed.
            app.invoke_quit_riina();
        },
    );
    Some(SmokeRun { _timer: timer })
}

fn check_preferences() -> Result<(), String> {
    let path = crate::platform::preferences_path().map_err(|e| e.to_string())?;
    let saved =
        riina_core::Preferences::parse(&std::fs::read_to_string(path).map_err(|e| e.to_string())?)?;
    if saved.video_threads != 7
        || saved.load_icc_profile
        || saved.audio.audio_threads != 3
        || saved.network.user_agent != "Riina GUI smoke"
        || saved.hdr_tone_mapping.algorithm != riina_core::hdr_preferences::Algorithm::Hable
    {
        return Err("Layout or sidebar persistence overwrote other preferences".into());
    }
    Ok(())
}

#[derive(Debug, PartialEq)]
struct Surfaces {
    video: [i32; 4],
    controls: [i32; 4],
    menu: isize,
}
fn hwnd(window: &slint::Window) -> Result<HWND, String> {
    window
        .with_winit_window(|w| {
            w.window_handle().ok().and_then(|h| match h.as_raw() {
                RawWindowHandle::Win32(h) => Some(HWND(h.hwnd.get() as *mut _)),
                _ => None,
            })
        })
        .flatten()
        .ok_or("No realized native smoke window".into())
}
fn bounds(hwnd: HWND) -> Result<[i32; 4], String> {
    let mut rect = RECT::default();
    let mut point = POINT::default();
    // SAFETY: handles belong to retained live test windows; outputs are writable.
    unsafe {
        GetClientRect(hwnd, &mut rect).map_err(|e| e.to_string())?;
        if !ClientToScreen(hwnd, &mut point).as_bool() || !IsWindowVisible(hwnd).as_bool() {
            return Err("Native playback window is hidden".into());
        }
    }
    Ok([
        point.x,
        point.y,
        rect.right - rect.left,
        rect.bottom - rect.top,
    ])
}
fn surfaces(app: &PlayerWindow, host: &NativeVideoHost) -> Result<Surfaces, String> {
    let host_hwnd = hwnd(host.window())?;
    let controls = hwnd(app.window())?;
    let signature = host
        .window()
        .with_winit_window(super::window_geometry::native_video_surface_signature)
        .flatten();
    if signature.is_none() {
        return Err("No visible aligned mpv child".into());
    }
    let video = bounds(host_hwnd)?;
    let controls = bounds(controls)?;
    if video[..2] != controls[..2] {
        return Err("Controls detached from video".into());
    }
    // SAFETY: host remains retained on this UI thread.
    Ok(Surfaces {
        video,
        controls,
        menu: unsafe { GetMenu(host_hwnd) }.0 as isize,
    })
}
fn check_audio_menu(host: &NativeVideoHost) -> Result<(), String> {
    let window = hwnd(host.window())?;
    // SAFETY: menu handles are read from the retained realized host.
    unsafe {
        let menu = GetMenu(window);
        let mut found = false;
        for index in 0..GetMenuItemCount(Some(menu)) {
            let mut text = [0u16; 128];
            let n = GetMenuStringW(menu, index as u32, Some(&mut text), MF_BYPOSITION);
            if String::from_utf16_lossy(&text[..n.max(0) as usize]) != "Audio" {
                continue;
            }
            let audio = GetSubMenu(menu, index);
            let mut items = 0;
            for item in 0..GetMenuItemCount(Some(audio)) {
                let n = GetMenuStringW(audio, item as u32, Some(&mut text), MF_BYPOSITION);
                let label = String::from_utf16_lossy(&text[..n.max(0) as usize]);
                if label == "Audio Track" || label == "Load External Audio…" {
                    if GetMenuState(audio, item as u32, MF_BYPOSITION)
                        & (MF_DISABLED.0 | MF_GRAYED.0)
                        != 0
                    {
                        return Err(format!("Audio action remains disabled: {label}"));
                    }
                    if label == "Audio Track" && GetMenuItemCount(Some(GetSubMenu(audio, item))) < 1
                    {
                        return Err("Audio tracks submenu is empty".into());
                    }
                    items += 1;
                }
            }
            found = items == 2;
        }
        if !found || host.get_audio_track_menu_rows().row_count() == 0 {
            return Err("Audio menu contract is missing".into());
        }
    }
    Ok(())
}
