#[cfg(target_os = "windows")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    windows_probe::run()
}

#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("The D3D11 presentation probe requires Windows.");
}

#[cfg(target_os = "windows")]
mod windows_probe {
    //! Bounded experiment: mpv owns native video; an owned Slint popup tests airspace.
    use riina_core::{Command, time_label};
    use riina_mpv::Player;
    use slint::{
        ComponentHandle,
        winit_030::{EventResult, WinitWindowAccessor, winit},
    };
    use std::{
        cell::RefCell,
        path::PathBuf,
        rc::Rc,
        sync::{
            Arc, Mutex,
            atomic::{AtomicIsize, Ordering},
        },
    };
    use winit::{
        platform::windows::WindowAttributesExtWindows,
        raw_window_handle::{HasWindowHandle, RawWindowHandle},
        window::Fullscreen,
    };
    slint::include_modules!();

    fn position_controls(host: &slint::Window, overlay: &slint::Window) {
        host.with_winit_window(|window| {
            if let Ok(origin) = window.inner_position() {
                let size = window.inner_size();
                let scale = window.scale_factor();
                overlay.set_position(slint::PhysicalPosition::new(
                    origin.x + (size.width as i32 - (460.0 * scale) as i32) / 2,
                    origin.y + size.height as i32 - (230.0 * scale) as i32,
                ));
            }
        });
    }

    pub(super) fn run() -> Result<(), Box<dyn std::error::Error>> {
        let files: Rc<Vec<String>> = Rc::new(std::env::args().skip(1).collect());
        let media = files
            .first()
            .ok_or("Pass one or more local media files to the experiment")?
            .clone();
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()?;
        let log = root.join("tools/d3d11-probe.log");
        let owner = Arc::new(AtomicIsize::new(0));
        let hook_owner = owner.clone();
        slint::BackendSelector::new()
            .backend_name("winit".into())
            .with_winit_window_attributes_hook(move |attributes| {
                let handle = hook_owner.load(Ordering::Relaxed);
                if handle == 0 {
                    attributes
                } else {
                    // An owned popup stays above its video host and follows minimization.
                    attributes
                        .with_owner_window(handle)
                        .with_transparent(true)
                        .with_decorations(false)
                        .with_resizable(false)
                }
            })
            .select()?;
        let host = NativeVideoHost::new()?;
        let controls: Rc<RefCell<Option<NativeProbeOverlay>>> = Rc::new(RefCell::new(None));
        let player: Rc<RefCell<Option<Player>>> = Rc::new(RefCell::new(None));
        let weak_host = host.as_weak();
        let startup_controls = controls.clone();
        let startup_player = player.clone();
        host.show()?;
        slint::spawn_local(async move {
            let Some(host) = weak_host.upgrade() else {
                return;
            };
            // show() schedules creation; wait for the actual winit window rather than
            // treating a zero-duration timer as proof that its HWND already exists.
            if let Err(error) = host.window().winit_window().await {
                eprintln!("{error}");
                return;
            }
            let result = (|| -> Result<Player, String> {
                let hwnd = host
                    .window()
                    .with_winit_window(|window| -> Result<isize, String> {
                        eprintln!(
                            "Native host: scale={}, client={}x{} physical pixels",
                            window.scale_factor(),
                            window.inner_size().width,
                            window.inner_size().height
                        );
                        match window.window_handle().map_err(|e| e.to_string())?.as_raw() {
                            RawWindowHandle::Win32(handle) => Ok(handle.hwnd.get()),
                            _ => Err("The experiment requires a Windows HWND".into()),
                        }
                    })
                    .ok_or("Native host was not realized")??;
                // Windows HWNDs use 32 significant bits, including on 64-bit Windows.
                let wid = u32::try_from(hwnd as usize)
                    .map_err(|_| "HWND exceeds mpv's Windows wid range")?;
                owner.store(hwnd, Ordering::Relaxed);
                let overlay = NativeProbeOverlay::new().map_err(|e| e.to_string())?;
                wire_controls(&host, &overlay, &startup_player, files);
                overlay.show().map_err(|e| e.to_string())?;
                position_controls(host.window(), overlay.window());
                let weak = overlay.as_weak();
                *startup_controls.borrow_mut() = Some(overlay);
                let last_metadata = Mutex::new(None);
                let player = Player::start_native_window(
                    &root.join("tools/mpv/libmpv-2.dll"),
                    wid,
                    &log,
                    Vec::new(),
                    move |state| {
                        let metadata = (
                            state.generation,
                            state.video.clone(),
                            state.video_target.clone(),
                        );
                        let mut last = last_metadata.lock().unwrap();
                        if last.as_ref() != Some(&metadata) {
                            eprintln!(
                                "Color metadata: generation={}, source={:?}, target={:?}",
                                state.generation, state.video, state.video_target
                            );
                            *last = Some(metadata);
                        }
                        let source = state
                            .video
                            .as_ref()
                            .and_then(|video| video.gamma.as_deref())
                            .unwrap_or("Unknown transfer");
                        let status = state.error.unwrap_or_else(|| {
                            format!(
                                "{} · {:?} · {} · {}",
                                time_label(state.position),
                                state.phase,
                                source,
                                state
                                    .hardware_decoder
                                    .as_deref()
                                    .unwrap_or("No decoder observed")
                            )
                        });
                        let _ = weak.upgrade_in_event_loop(move |overlay| {
                            overlay.set_status(status.into())
                        });
                    },
                )?;
                player.send(Command::Load(media))?;
                Ok(player)
            })();
            match result {
                Ok(value) => *startup_player.borrow_mut() = Some(value),
                Err(error) => {
                    eprintln!("{error}");
                    if let Some(overlay) = startup_controls.borrow().as_ref() {
                        overlay.set_status(error.into());
                    }
                }
            }
        })?;
        let event_controls = controls.clone();
        host.window().on_winit_window_event(move |host, event| {
            if matches!(
                event,
                winit::event::WindowEvent::Moved(_)
                    | winit::event::WindowEvent::Resized(_)
                    | winit::event::WindowEvent::ScaleFactorChanged { .. }
            ) && let Some(overlay) = event_controls.borrow().as_ref()
            {
                position_controls(host, overlay.window());
            }
            EventResult::Propagate
        });
        let shutdown_player = player.clone();
        let close_controls = controls.clone();
        host.window().on_close_requested(move || {
            // Stop the VO before the host HWND can disappear. The popup owns no player.
            shutdown_player.borrow_mut().take();
            if let Some(overlay) = close_controls.borrow().as_ref() {
                let _ = overlay.hide();
            }
            slint::CloseRequestResponse::HideWindow
        });
        slint::run_event_loop()?;
        player.borrow_mut().take();
        Ok(())
    }

    fn wire_controls(
        host: &NativeVideoHost,
        overlay: &NativeProbeOverlay,
        player: &Rc<RefCell<Option<Player>>>,
        files: Rc<Vec<String>>,
    ) {
        let close_player = player.clone();
        let close_host = host.as_weak();
        overlay.window().on_close_requested(move || {
            // The controls belong to the player, not an independent document.
            // Alt+F4 while they own focus must close both windows and the native VO.
            close_player.borrow_mut().take();
            if let Some(host) = close_host.upgrade() {
                let _ = host.hide();
            }
            slint::CloseRequestResponse::HideWindow
        });
        let commands = player.clone();
        overlay.on_stop(move || {
            if let Some(player) = commands.borrow().as_ref()
                && let Err(error) = player.send(Command::Stop)
            {
                eprintln!("{error}");
            }
        });
        let commands = player.clone();
        let index = Rc::new(RefCell::new(0usize));
        overlay.on_next_file(move || {
            if let Some(player) = commands.borrow().as_ref() {
                let next = (*index.borrow() + 1) % files.len();
                match player.send(Command::Load(files[next].clone())) {
                    Ok(()) => *index.borrow_mut() = next,
                    Err(error) => eprintln!("{error}"),
                }
            }
        });
        let commands = player.clone();
        overlay.on_pause(move || {
            if let Some(p) = commands.borrow().as_ref() {
                let _ = p.send(Command::TogglePause);
            }
        });
        let commands = player.clone();
        overlay.on_seek(move |seconds| {
            if let Some(p) = commands.borrow().as_ref() {
                let _ = p.send(Command::Seek {
                    seconds: seconds.into(),
                    absolute: false,
                });
            }
        });
        let weak = host.as_weak();
        overlay.on_fullscreen(move || {
            if let Some(host) = weak.upgrade() {
                host.window().with_winit_window(|window| {
                    window.set_fullscreen(if window.fullscreen().is_some() {
                        None
                    } else {
                        Some(Fullscreen::Borderless(None))
                    })
                });
            }
        });
    }
}
