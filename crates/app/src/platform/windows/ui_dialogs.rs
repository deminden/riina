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

use crate::ui_dialogs::{Purpose, SaveFile, dispatch_selection};

struct ModalWindows {
    app: slint::Weak<PlayerWindow>,
    welcome: Option<slint::Weak<WelcomeWindow>>,
    framed_owner: Option<Arc<Window>>,
    // Only windows disabled by this picker are reenabled at completion.
    disabled: Option<Vec<Arc<Window>>>,
}

impl ModalWindows {
    fn restore(&mut self, return_to_welcome: bool) {
        let Some(disabled) = self.disabled.take() else {
            return;
        };
        for window in &disabled {
            window.set_enable(true);
        }
        let app = self.app.upgrade();
        if return_to_welcome && let Some(welcome) = self.welcome.as_ref().and_then(|w| w.upgrade())
        {
            if let Some(owner) = &self.framed_owner {
                owner.set_visible(false);
            }
            if let Some(app) = &app {
                // Continue releasing session resources if the window is closing.
                let _ = app.hide();
            }
            if let Err(error) = welcome.show() {
                crate::diagnostics::warn!("Could not restore the welcome window: {error}");
            }
        } else if let Some(app) = &app {
            app.window()
                .with_winit_window(super::menu_focus::activate_without_menu_key);
        }
        if let Some(app) = app {
            app.global::<riina_ui::PlayerDialogState>()
                .set_dialog_open(false);
            app.global::<riina_ui::PlayerControlActions>()
                .invoke_activity();
        }
    }
}

impl Drop for ModalWindows {
    fn drop(&mut self) {
        let Some(disabled) = self.disabled.take() else {
            return;
        };
        // Startup/thread failures still restore modality on the GUI thread. No raw
        // HWND is captured without the Arc that owns it, and no fake Send is needed.
        let app = self.app.clone();
        let framed_owner = self.framed_owner.take();
        // Capture plain fields: a rejected dispatch must not recursively drop
        // another guard and try to enqueue itself on a stopped event loop.
        // The window may close before deferred UI work can be delivered.
        let _ = slint::invoke_from_event_loop(move || {
            ModalWindows {
                app,
                welcome: None,
                framed_owner,
                disabled: Some(disabled),
            }
            .restore(false);
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
            let paths = if matches!(purpose, Purpose::PlaylistFolders) {
                file_dialog::open_folders(owner, purpose.title())?
            } else {
                file_dialog::open_files(owner, purpose.allows_multiple(), purpose.title())?
            };
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

pub(crate) fn save_file_dialog(
    app: &PlayerWindow,
    host: Option<slint::Weak<NativeVideoHost>>,
    file: SaveFile,
    complete: impl FnOnce(&PlayerWindow, Result<Option<PathBuf>, String>) + Send + 'static,
) {
    start(
        app,
        host,
        None,
        move |owner| file_dialog::save_destination(owner, &file),
        complete,
    );
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
        let (owner, mut modality) = match prepare(&app, host, welcome).await {
            Ok(prepared) => prepared,
            Err(error) => {
                app.global::<riina_ui::PlayerDialogState>()
                    .set_dialog_open(false);
                complete(&app, Err(error));
                return;
            }
        };
        if let Err(error) = crate::background::spawn_named("file-picker", move || {
            let result = pick(&owner);
            // The window may close before deferred UI work can be delivered.
            let _ = slint::invoke_from_event_loop(move || {
                let return_to_welcome =
                    modality.welcome.is_some() && result.as_ref().is_ok_and(Option::is_none);
                if let Some(app) = weak.upgrade() {
                    // Finish the caller's reservation before native input is reenabled.
                    complete(&app, result);
                }
                modality.restore(return_to_welcome);
            });
        }) {
            // The guard's cleanup releases the window and the gate together.
            crate::diagnostics::report(&app, format!("Could not start file chooser: {error}"));
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

async fn prepare(
    app: &PlayerWindow,
    host: Option<slint::Weak<NativeVideoHost>>,
    welcome: Option<slint::Weak<WelcomeWindow>>,
) -> Result<(file_dialog::DialogOwner, ModalWindows), String> {
    let controls = app
        .window()
        .winit_window()
        .await
        .map_err(|e| e.to_string())?;
    let framed_owner = if let Some(host) = host {
        let host = host.upgrade().ok_or("Native video host closed")?;
        Some(
            host.window()
                .winit_window()
                .await
                .map_err(|e| e.to_string())?,
        )
    } else {
        None
    };
    // The framed HWND survives changes to the transparent controls during playback.
    let owner = file_dialog::DialogOwner::new(framed_owner.as_ref().unwrap_or(&controls).clone())?;
    let disabled = std::iter::once(controls)
        .chain(framed_owner.iter().cloned())
        .filter(|window| file_dialog::window_enabled(window))
        .collect::<Vec<_>>();
    // Cross-thread IFileDialog::Show alone does not block both player surfaces.
    for window in &disabled {
        window.set_enable(false);
    }
    Ok((
        owner,
        ModalWindows {
            app: app.as_weak(),
            welcome,
            framed_owner,
            disabled: Some(disabled),
        },
    ))
}
