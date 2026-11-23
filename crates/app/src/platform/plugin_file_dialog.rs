//! A native selection belongs to the live player that presented it.
use crate::{
    NativeVideoHost, PlayerWindow,
    plugin_runtime::utilities::{FileDialogs, FileRequest},
};
use riina_ui::{PlayerControlActions, PlayerDialogState};
use slint::{ComponentHandle, winit_030::WinitWindowAccessor};

pub(crate) fn install(
    app: &PlayerWindow,
    native_host: Option<slint::Weak<NativeVideoHost>>,
    dialogs: &FileDialogs,
) {
    let player = app.as_weak();
    dialogs.install(move |request| {
        let player = player.clone();
        let native_host = native_host.clone();
        slint::invoke_from_event_loop(move || {
            let Some(app) = player.upgrade() else {
                request.finish(Err("The player has closed".into()));
                return;
            };
            if !request.is_cancelled() {
                present(&app, native_host, request);
            }
        })
        .map_err(|error| error.to_string())
    });
}

fn present(
    app: &PlayerWindow,
    native_host: Option<slint::Weak<NativeVideoHost>>,
    request: FileRequest,
) {
    if app.global::<PlayerDialogState>().get_dialog_open() {
        request.finish(Err("The player already has an open dialog".into()));
        return;
    }
    set_active(app, true);
    let app = app.as_weak();
    let failure = app.clone();
    if slint::spawn_local(async move {
        let Some(app) = app.upgrade() else {
            request.finish(Err("The player has closed".into()));
            return;
        };
        let result = pick(&app, native_host, &request).await;
        set_active(&app, false);
        request.finish(result);
    })
    .is_err()
        && let Some(app) = failure.upgrade()
    {
        set_active(&app, false);
    }
}

fn set_active(app: &PlayerWindow, active: bool) {
    app.global::<PlayerDialogState>().set_dialog_open(active);
    app.global::<PlayerControlActions>().invoke_activity();
}

async fn pick(
    app: &PlayerWindow,
    native_host: Option<slint::Weak<NativeVideoHost>>,
    request: &FileRequest,
) -> Result<Option<String>, String> {
    let owner = app
        .window()
        .winit_window()
        .await
        .map_err(|error| error.to_string())?;
    if request.is_cancelled() {
        return Ok(None);
    }
    #[cfg(windows)]
    let _modal = {
        let mut disabled = DisabledWindows(Vec::new());
        if let Some(host) = native_host.and_then(|host| host.upgrade()) {
            let host = host
                .window()
                .winit_window()
                .await
                .map_err(|error| error.to_string())?;
            disabled.include(host);
        }
        disabled.include(owner.clone());
        disabled
    };
    #[cfg(not(windows))]
    let _ = native_host;

    let mut dialog = rfd::AsyncFileDialog::new()
        .set_title(&request.title)
        .set_parent(owner.as_ref());
    if !request.extensions.is_empty() && !request.folder {
        dialog = dialog.add_filter("Allowed files", &request.extensions);
    }
    let selected = if request.folder {
        dialog.pick_folder().await
    } else {
        dialog.pick_file().await
    };
    selected
        .map(|entry| {
            entry
                .path()
                .to_str()
                .map(str::to_owned)
                .ok_or_else(|| "The selected path contains invalid Unicode".into())
        })
        .transpose()
}

#[cfg(windows)]
struct DisabledWindows(Vec<std::sync::Arc<slint::winit_030::winit::window::Window>>);

#[cfg(windows)]
impl DisabledWindows {
    fn include(&mut self, window: std::sync::Arc<slint::winit_030::winit::window::Window>) {
        use slint::winit_030::winit::platform::windows::WindowExtWindows;
        window.set_enable(false);
        self.0.push(window);
    }
}

#[cfg(windows)]
impl Drop for DisabledWindows {
    fn drop(&mut self) {
        use slint::winit_030::winit::platform::windows::WindowExtWindows;
        for window in &self.0 {
            window.set_enable(true);
        }
        if let Some(owner) = self.0.last() {
            owner.focus_window();
        }
    }
}
