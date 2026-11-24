//! One owned picker per player; native HWNDs outlive its dedicated COM apartment.
use super::file_dialog;
use crate::{NativeVideoHost, PlayerWindow, WelcomeWindow};
use riina_mpv::CommandSender;
use slint::{
    ComponentHandle,
    winit_030::{WinitWindowAccessor, winit},
};
use std::{path::PathBuf, sync::Arc};
use winit::{platform::windows::WindowExtWindows, window::Window};

use crate::ui_dialogs::{Purpose, dispatch_selection};

struct ModalWindows {
    app: slint::Weak<PlayerWindow>,
    welcome: Option<slint::Weak<WelcomeWindow>>,
    // The first HWND is the dialog owner (chrome); the optional second is its VO host.
    windows: Vec<Arc<Window>>,
}

impl ModalWindows {
    fn restore(&mut self, return_to_welcome: bool) {
        if let Some(app) = self.app.upgrade() {
            app.global::<riina_ui::PlayerDialogState>()
                .set_dialog_open(false);
            app.global::<riina_ui::PlayerControlActions>()
                .invoke_activity();
        }
        for window in &self.windows {
            window.set_enable(true);
        }
        if return_to_welcome && let Some(welcome) = self.welcome.as_ref().and_then(|w| w.upgrade())
        {
            // The optional second surface is the native video owner. It must
            // leave the z-order before the separate welcome window is restored.
            if let Some(video_host) = self.windows.get(1) {
                video_host.set_visible(false);
            }
            if let Some(app) = self.app.upgrade() {
                // Continue releasing session resources if the window is closing.
                let _ = app.hide();
            }
            if let Err(error) = welcome.show() {
                crate::diagnostics::warn!("Could not restore the welcome window: {error}");
            }
        } else if let Some(owner) = self.windows.first() {
            owner.focus_window();
        }
        self.windows.clear();
    }
}

impl Drop for ModalWindows {
    fn drop(&mut self) {
        if self.windows.is_empty() {
            return;
        }
        // Startup/thread failures still restore modality on the GUI thread. No raw
        // HWND is captured without the Arc that owns it, and no fake Send is needed.
        let app = self.app.clone();
        let windows = std::mem::take(&mut self.windows);
        // Capture plain fields: a rejected dispatch must not recursively drop
        // another guard and try to enqueue itself on a stopped event loop.
        // The window may close before deferred UI work can be delivered.
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(app) = app.upgrade() {
                app.global::<riina_ui::PlayerDialogState>()
                    .set_dialog_open(false);
                app.global::<riina_ui::PlayerControlActions>()
                    .invoke_activity();
            }
            for window in &windows {
                window.set_enable(true);
            }
            if let Some(owner) = windows.first() {
                owner.focus_window();
            }
        });
    }
}

pub fn open(
    app: &PlayerWindow,
    host: Option<slint::Weak<NativeVideoHost>>,
    purpose: Purpose,
    sender: CommandSender,
    welcome: Option<slint::Weak<WelcomeWindow>>,
) {
    start(
        app,
        host,
        welcome,
        move |owner| {
            let paths = file_dialog::open_files(owner, purpose.allows_multiple(), purpose.title())?;
            Ok((!paths.is_empty()).then_some(paths))
        },
        move |app, result| {
            let result = result.and_then(|paths| {
                dispatch_selection(app, purpose, paths.unwrap_or_default(), sender)
            });
            if let Err(error) = result {
                app.set_status(error.into());
            }
        },
    );
}

pub(crate) fn save_playlist_dialog(
    app: &PlayerWindow,
    host: Option<slint::Weak<NativeVideoHost>>,
    complete: impl FnOnce(&PlayerWindow, Result<Option<PathBuf>, String>) + Send + 'static,
) {
    start(app, host, None, file_dialog::playlist_destination, complete);
}

fn start<T: Send + 'static>(
    app: &PlayerWindow,
    host: Option<slint::Weak<NativeVideoHost>>,
    welcome: Option<slint::Weak<WelcomeWindow>>,
    pick: impl FnOnce(&file_dialog::DialogOwner) -> Result<Option<T>, String> + Send + 'static,
    complete: impl FnOnce(&PlayerWindow, Result<Option<T>, String>) + Send + 'static,
) {
    // The gate closes before the async HWND lookup: queued shortcuts cannot open
    // a second picker while the first is still being constructed.
    if app
        .global::<riina_ui::PlayerDialogState>()
        .get_dialog_open()
    {
        return;
    }
    app.global::<riina_ui::PlayerDialogState>()
        .set_dialog_open(true);
    app.global::<riina_ui::PlayerControlActions>()
        .invoke_activity();
    let weak = app.as_weak();
    let failure = weak.clone();
    let started = slint::spawn_local(async move {
        let Some(app) = weak.upgrade() else { return };
        let result = async {
            let owner = app
                .window()
                .winit_window()
                .await
                .map_err(|e| e.to_string())?;
            let mut windows = vec![owner.clone()];
            if let Some(host) = host {
                let host = host.upgrade().ok_or("Native video host closed")?;
                windows.push(
                    host.window()
                        .winit_window()
                        .await
                        .map_err(|e| e.to_string())?,
                );
            }
            let mut modality = ModalWindows {
                app: weak.clone(),
                welcome,
                windows,
            };
            // The Shell dialog runs on a separate STA. Explicitly disable every
            // player HWND; cross-thread Show alone did not reliably block its owner.
            // Restore only after Show returns, then focus the live chrome again.
            for window in &modality.windows {
                window.set_enable(false);
            }
            let owner = file_dialog::DialogOwner::new(owner)?;
            crate::background::spawn_named("file-picker", move || {
                let result = pick(&owner);
                // The window may close before deferred UI work can be delivered.
                let _ = slint::invoke_from_event_loop(move || {
                    modality.restore(
                        modality.welcome.is_some() && result.as_ref().is_ok_and(Option::is_none),
                    );
                    if let Some(app) = weak.upgrade() {
                        complete(&app, result);
                    }
                });
            })
            .map_err(|e| e.to_string())?;
            Ok::<(), String>(())
        }
        .await;
        if let Err(error) = result {
            app.global::<riina_ui::PlayerDialogState>()
                .set_dialog_open(false);
            app.set_status(error.into());
        }
    });
    if let Err(error) = started
        && let Some(app) = failure.upgrade()
    {
        app.global::<riina_ui::PlayerDialogState>()
            .set_dialog_open(false);
        app.set_status(error.to_string().into());
    }
}
