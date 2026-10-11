//! Native video owns a host HWND; the shared player UI owns its transparent popup.
use crate::{
    NativeVideoHost, PlayerWindow,
    player_ui::{UiSession, configure_player},
};
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
};
use windows::{
    Win32::{
        Foundation::HWND,
        Graphics::{
            Dwm::{DWM_BB_BLURREGION, DWM_BB_ENABLE, DWM_BLURBEHIND, DwmEnableBlurBehindWindow},
            Gdi::{CreateRectRgn, DeleteObject, HGDIOBJ},
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
            chrome.set_size(slint::PhysicalSize::new(
                window.inner_size().width,
                window.inner_size().height,
            ));
            chrome.set_position(slint::PhysicalPosition::new(origin.x, origin.y));
        }
    });
}

pub fn run(media: Option<OsString>) -> Result<(), Box<dyn std::error::Error>> {
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
    host.set_display_title("riina".into());
    host.window().with_winit_window(|window| {
        window.set_title("riina");
    });
    let chrome: Rc<RefCell<Option<PlayerWindow>>> = Rc::new(RefCell::new(None));
    let welcome_window: Rc<RefCell<Option<crate::WelcomeWindow>>> = Rc::new(RefCell::new(None));
    let session: Rc<RefCell<Option<UiSession>>> = Rc::new(RefCell::new(None));
    let weak_host = host.as_weak();
    let startup_chrome = chrome.clone();
    let startup_welcome = welcome_window.clone();
    let startup_session = session.clone();
    let resize_filter = Rc::new(RefCell::new(
        None::<crate::window_aspect::WindowEventFilter>,
    ));
    let startup_resize_filter = resize_filter.clone();
    let menu_lifecycle = Rc::new(RefCell::new(None));
    let startup_menu_lifecycle = menu_lifecycle.clone();
    let menu_focus = Rc::new(Cell::new(false));
    let startup_menu_focus = menu_focus.clone();
    host.show()?;
    slint::spawn_local(async move {
        let Some(host) = weak_host.upgrade() else {
            return;
        };
        let result = async {
            let window = host.window().winit_window().await?;
            window.set_min_inner_size(Some(winit::dpi::LogicalSize::new(520.0, 300.0)));
            let presentation = super::NativePresentation::new(window, host.as_weak())
                .map_err(std::io::Error::other)?;
            owner.store(presentation.owner_handle(), Ordering::Relaxed);
            // Slint captures the attributes hook when creating the component adapter.
            // Construct only after its owner HWND exists, as in the verified probe.
            let app = PlayerWindow::new()?;
            app.set_native_video(true);
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
            let welcome = if media.is_none() {
                Some(crate::WelcomeWindow::new()?)
            } else {
                None
            };
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
            // The framed owner hosts the native menu; its transparent popup
            // retains controller state. Bridge observations without a second
            // playback model or sending window handles across threads.
            let menu_host = host.as_weak();
            let menu_app = app.as_weak();
            app.on_video_menu_changed(move || {
                if let (Some(host), Some(app)) = (menu_host.upgrade(), menu_app.upgrade()) {
                    host.set_reflection_menu_enabled(
                        app.get_loaded() && app.get_reflection_known() && !app.get_crop_editing(),
                    );
                    host.set_video_mirrored(app.get_video_mirrored());
                    host.set_video_flipped(app.get_video_flipped());
                    host.set_crop_menu_enabled(
                        app.get_loaded() && app.get_video_crop_known() && !app.get_crop_editing(),
                    );
                    host.set_crop_menu_labels(app.get_video_crop_menu_labels());
                    host.set_crop_menu_index(app.get_video_crop_menu_index());
                    host.set_aspect_menu_enabled(
                        app.get_loaded() && app.get_video_aspect_known() && !app.get_crop_editing(),
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
                        app.get_loaded() && app.get_deinterlace_known() && !app.get_crop_editing(),
                    );
                    host.set_deinterlace_enabled(app.get_deinterlace_enabled());
                    host.set_screenshot_available(
                        app.get_screenshot_available() && !app.get_crop_editing(),
                    );
                    host.set_screenshot_folder_enabled(!app.get_crop_editing());
                }
            });
            let menu_app = app.as_weak();
            host.on_toggle_video_reflection(move |horizontal| {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_toggle_video_reflection(horizontal);
                }
            });
            let configured = configure_player(&app, Some(presentation), media, welcome.as_ref())?;
            let menu_app = app.as_weak();
            host.on_set_menu_video_crop(move |index| {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_set_menu_video_crop(index);
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
            let close_host = host.as_weak();
            let weak_app = app.as_weak();
            app.window().on_close_requested(move || {
                if weak_app.upgrade().is_some_and(|app| app.get_dialog_open()) {
                    return slint::CloseRequestResponse::KeepWindowShown;
                }
                close(&close_session, weak_app.upgrade().as_ref());
                if let Some(host) = close_host.upgrade() {
                    let _ = host.hide();
                }
                slint::CloseRequestResponse::HideWindow
            });
            app.show()?;
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
            close(&startup_session, startup_chrome.borrow().as_ref());
            let _ = host.hide();
        }
    })?;
    let event_chrome = chrome.clone();
    host.window().on_winit_window_event(move |host, event| {
        let result = resize_filter
            .borrow()
            .as_ref()
            .map_or(EventResult::Propagate, |filter| filter(host, event));
        if matches!(event, winit::event::WindowEvent::Focused(true))
            && !menu_focus.get()
            && let Some(app) = event_chrome.borrow().as_ref()
            && !app.get_dialog_open()
        {
            // Fullscreen and taskbar activation focus the owner. Keyboard commands
            // belong to the shared chrome, not mpv's otherwise inactive host.
            super::menu_focus::focus_chrome(app.window());
        }
        if matches!(
            event,
            winit::event::WindowEvent::Moved(_)
                | winit::event::WindowEvent::Resized(_)
                | winit::event::WindowEvent::ScaleFactorChanged { .. }
        ) && let Some(app) = event_chrome.borrow().as_ref()
        {
            align(host, app.window());
        }
        result
    });
    let close_session = session.clone();
    let close_chrome = chrome.clone();
    host.window().on_close_requested(move || {
        let chrome = close_chrome.borrow();
        if chrome.as_ref().is_some_and(|app| app.get_dialog_open()) {
            return slint::CloseRequestResponse::KeepWindowShown;
        }
        close(&close_session, chrome.as_ref());
        if let Some(app) = chrome.as_ref() {
            let _ = app.hide();
        }
        slint::CloseRequestResponse::HideWindow
    });
    slint::run_event_loop()?;
    close(&session, chrome.borrow().as_ref());
    Ok(())
}

fn close(session: &RefCell<Option<UiSession>>, app: Option<&PlayerWindow>) {
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
