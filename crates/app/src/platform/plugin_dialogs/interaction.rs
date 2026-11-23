use super::{NativeVideoHost, PlayerWindow, set_active};
use crate::plugin_runtime::utilities::{InteractionKind, InteractionRequest};
use riina_ui::{PlayerAppearance, PlayerDialogState, PluginInteractionWindow};
use slint::{CloseRequestResponse, ComponentHandle, Timer, TimerMode};
use std::{cell::Cell, rc::Rc, time::Duration};
use tokio::sync::oneshot;

pub(super) fn present(
    app: &PlayerWindow,
    native_host: Option<slint::Weak<NativeVideoHost>>,
    request: InteractionRequest,
) {
    set_interaction_active(app, true);
    let player = app.as_weak();
    let fallback = player.clone();
    if slint::spawn_local(async move {
        let Some(app) = player.upgrade() else {
            request.finish(Err("The player has closed".into()));
            return;
        };
        let response = interact(&app, native_host, &request).await;
        set_interaction_active(&app, false);
        request.finish(response);
    })
    .is_err()
        && let Some(app) = fallback.upgrade()
    {
        set_interaction_active(&app, false);
    }
}

fn set_interaction_active(app: &PlayerWindow, active: bool) {
    app.global::<PlayerDialogState>()
        .set_plugin_interaction_visible(active);
    set_active(app, active);
}

async fn interact(
    app: &PlayerWindow,
    native_host: Option<slint::Weak<NativeVideoHost>>,
    request: &InteractionRequest,
) -> Result<Option<String>, String> {
    if request.is_cancelled() {
        return Ok(None);
    }
    let window = PluginInteractionWindow::new().map_err(|error| error.to_string())?;
    crate::appearance_preferences::install_system_typography(&window);
    window.set_dark_theme(app.global::<PlayerAppearance>().get_dark_theme());
    window.set_asks_for_text(request.kind == InteractionKind::Prompt);
    window.set_message(request.title.as_str().into());
    let (answer, response) = oneshot::channel();
    let answer = Rc::new(Cell::new(Some(answer)));
    let button_answer = answer.clone();
    window.on_response(move |accepted, text| {
        complete(&button_answer, accepted.then(|| text.to_string()));
    });
    let close_answer = answer.clone();
    window.window().on_close_requested(move || {
        complete(&close_answer, None);
        CloseRequestResponse::KeepWindowShown
    });
    crate::platform::center_auxiliary_window(&window, app);
    crate::platform::show_window(&window).map_err(|error| error.to_string())?;
    #[cfg(windows)]
    let _modal = windows_modal(&window, app, native_host).await;
    #[cfg(not(windows))]
    let _ = native_host;
    window.invoke_focus_default();

    let timer = Timer::default();
    let cancelled = request.cancellation();
    timer.start(TimerMode::Repeated, Duration::from_millis(50), move || {
        if cancelled() {
            complete(&answer, None);
        }
    });
    let decision = response.await.ok().flatten();
    timer.stop();
    let _ = window.hide();
    Ok(decision)
}

fn complete(answer: &Cell<Option<oneshot::Sender<Option<String>>>>, value: Option<String>) {
    if let Some(answer) = answer.take() {
        let _ = answer.send(value);
    }
}

#[cfg(windows)]
async fn windows_modal(
    dialog: &PluginInteractionWindow,
    app: &PlayerWindow,
    native_host: Option<slint::Weak<NativeVideoHost>>,
) -> super::DisabledWindows {
    use slint::winit_030::WinitWindowAccessor;
    let mut disabled = super::DisabledWindows(Vec::new());
    if let Some(host) = native_host.and_then(|host| host.upgrade())
        && host.window().has_winit_window()
        && let Ok(window) = host.window().winit_window().await
    {
        disabled.include(window);
    }
    if app.window().has_winit_window()
        && let Ok(window) = app.window().winit_window().await
    {
        disabled.include(window);
    }
    dialog.window().with_winit_window(|window| {
        if let Some(owner) = disabled
            .0
            .iter()
            .find(|window| window.is_visible() == Some(true))
        {
            attach_owner(window, owner);
        }
        window.focus_window();
    });
    disabled
}

#[cfg(windows)]
fn attach_owner(
    dialog: &slint::winit_030::winit::window::Window,
    owner: &slint::winit_030::winit::window::Window,
) {
    use slint::winit_030::winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows::Win32::{
        Foundation::HWND,
        UI::WindowsAndMessaging::{GWLP_HWNDPARENT, SetWindowLongPtrW},
    };
    let (Ok(dialog), Ok(owner)) = (dialog.window_handle(), owner.window_handle()) else {
        return;
    };
    if let (RawWindowHandle::Win32(dialog), RawWindowHandle::Win32(owner)) =
        (dialog.as_raw(), owner.as_raw())
    {
        // SAFETY: both handles are live top-level windows on this UI thread.
        // GWLP_HWNDPARENT changes the owner of a top-level window.
        unsafe {
            SetWindowLongPtrW(
                HWND(dialog.hwnd.get() as _),
                GWLP_HWNDPARENT,
                owner.hwnd.get(),
            )
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slint::platform::{Key, WindowEvent};

    #[test]
    fn interaction_keyboard_preserves_text_and_delivers_escape_as_cancel() {
        i_slint_backend_testing::init_no_event_loop();
        let window = PluginInteractionWindow::new().unwrap();
        window.set_asks_for_text(true);
        window.set_message("Enter a name".into());
        let answer = Rc::new(std::cell::RefCell::new(Vec::new()));
        let result = answer.clone();
        window.on_response(move |accepted, text| result.borrow_mut().push((accepted, text)));
        window.show().unwrap();
        window.invoke_focus_default();
        let press = |text: slint::SharedString| {
            window
                .window()
                .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
            window
                .window()
                .dispatch_event(WindowEvent::KeyReleased { text });
        };
        press("雪".into());
        press(Key::Return.into());
        assert_eq!(answer.take(), [(true, "雪".into())]);
        window.set_asks_for_text(false);
        window.set_draft("".into());
        window.invoke_focus_default();
        press(Key::Escape.into());
        press(Key::Return.into());
        assert_eq!(answer.take(), [(false, "".into()), (true, "".into())]);
        window.hide().unwrap();
    }
}
