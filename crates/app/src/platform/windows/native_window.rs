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
    cell::RefCell,
    ffi::OsString,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicIsize, Ordering},
    },
};
use winit::platform::windows::WindowAttributesExtWindows;

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
    host.set_display_title("riina — native video experiment".into());
    host.window().with_winit_window(|window| {
        window.set_title("riina — native video experiment");
    });
    let chrome: Rc<RefCell<Option<PlayerWindow>>> = Rc::new(RefCell::new(None));
    let session: Rc<RefCell<Option<UiSession>>> = Rc::new(RefCell::new(None));
    let weak_host = host.as_weak();
    let startup_chrome = chrome.clone();
    let startup_session = session.clone();
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
            // The framed owner hosts the native menu; its transparent popup
            // retains controller state. Bridge observations without a second
            // playback model or sending window handles across threads.
            let menu_host = host.as_weak();
            let menu_app = app.as_weak();
            app.on_reflection_menu_changed(move || {
                if let (Some(host), Some(app)) = (menu_host.upgrade(), menu_app.upgrade()) {
                    host.set_reflection_menu_enabled(
                        app.get_loaded() && app.get_reflection_known() && !app.get_crop_editing(),
                    );
                    host.set_video_mirrored(app.get_video_mirrored());
                    host.set_video_flipped(app.get_video_flipped());
                }
            });
            let menu_app = app.as_weak();
            host.on_toggle_video_reflection(move |horizontal| {
                if let Some(app) = menu_app.upgrade() {
                    app.invoke_toggle_video_reflection(horizontal);
                }
            });
            let configured = configure_player(&app, Some(presentation), media)?;
            *startup_session.borrow_mut() = Some(configured);
            let close_session = startup_session.clone();
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
            app.window().winit_window().await?;
            align(host.window(), app.window());
            eprintln!(
                "Native chrome: scale={}, client={}x{} physical, layout={}x{} logical",
                app.window().scale_factor(),
                app.window().size().width,
                app.window().size().height,
                app.get_video_width(),
                app.get_video_height(),
            );
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
        if matches!(event, winit::event::WindowEvent::Focused(true))
            && let Some(app) = event_chrome.borrow().as_ref()
            && !app.get_dialog_open()
        {
            // Fullscreen and taskbar activation focus the owner. Keyboard commands
            // belong to the shared chrome, not mpv's otherwise inactive host.
            app.window()
                .with_winit_window(|window| window.focus_window());
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
        EventResult::Propagate
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
