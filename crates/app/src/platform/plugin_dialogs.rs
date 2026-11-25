//! Plugin dialogs use the player's existing modal exclusion.
mod application;
mod interaction;

use crate::{
    NativeVideoHost, PlayerWindow,
    plugin_runtime::utilities::{DialogRequest, FileRequest, PluginDialogs},
};
use riina_ui::{PlayerControlActions, PlayerDialogState};
use slint::{ComponentHandle, winit_030::WinitWindowAccessor};
use std::sync::{Arc, Mutex, Weak};

struct ApplicationOwner {
    player: slint::Weak<PlayerWindow>,
    native: Option<slint::Weak<NativeVideoHost>>,
}

pub(crate) struct ApplicationOwners {
    players: Arc<Mutex<Vec<Weak<ApplicationOwner>>>>,
    application: Option<std::rc::Rc<application::Dialogs>>,
}

impl Default for ApplicationOwners {
    fn default() -> Self {
        Self {
            players: Default::default(),
            application: super::plugin_application::SUPPORTS_WINDOWLESS.then(Default::default),
        }
    }
}

pub(crate) struct OwnerRegistration {
    _owner: Arc<ApplicationOwner>,
}

impl ApplicationOwners {
    pub(crate) fn install(&self, dialogs: &PluginDialogs) {
        let owners = self.players.clone();
        let application = self
            .application
            .as_ref()
            .map(application::Dialogs::bind_to_thread);
        dialogs.install(move |request| {
            let owners = owners.clone();
            slint::invoke_from_event_loop(move || {
                if matches!(request, DialogRequest::Notice(_)) {
                    present_without_player(application, request);
                    return;
                }
                let owners = {
                    let mut registrations = riina_core::sync::lock(&owners);
                    let mut owners = Vec::new();
                    registrations.retain(|registration| {
                        let Some(owner) = registration.upgrade() else {
                            return false;
                        };
                        owners.push(owner);
                        true
                    });
                    owners
                };
                let target = owners
                    .iter()
                    .filter_map(|owner| {
                        let app = owner.player.upgrade()?;
                        let rank = |window: &slint::Window| {
                            if window.with_winit_window(|window| window.has_focus()) == Some(true) {
                                2
                            } else {
                                u8::from(window.is_visible())
                            }
                        };
                        let rank = rank(app.window()).max(
                            owner
                                .native
                                .as_ref()
                                .and_then(slint::Weak::upgrade)
                                .map_or(0, |host| rank(host.window())),
                        );
                        if rank == 0 && application.is_some() {
                            return None;
                        }
                        Some((rank, app, &owner.native))
                    })
                    .max_by_key(|(rank, _, _)| *rank);
                if let Some((_, app, native)) = target {
                    present(&app, native.clone(), request);
                } else {
                    present_without_player(application, request);
                }
            })
            .map_err(|error| error.to_string())
        });
    }

    pub(crate) fn register(
        &self,
        app: &PlayerWindow,
        native: Option<slint::Weak<NativeVideoHost>>,
    ) -> OwnerRegistration {
        let owner = Arc::new(ApplicationOwner {
            player: app.as_weak(),
            native,
        });
        let mut registrations = riina_core::sync::lock(&self.players);
        registrations.retain(|entry| entry.strong_count() != 0);
        registrations.push(Arc::downgrade(&owner));
        OwnerRegistration { _owner: owner }
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn bind_application_close(
        &self,
        callback: std::rc::Rc<crate::windows::close_observer::CloseObserver>,
    ) {
        if let Some(application) = &self.application {
            application.bind_close(callback);
        }
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn application_dialog_visible(&self) -> bool {
        self.application
            .as_deref()
            .is_some_and(application::Dialogs::is_visible)
    }
}

fn present_without_player(application: Option<u64>, request: DialogRequest) {
    if let Some(application) = application {
        application::present(application, request);
    } else {
        request.fail("No player window is available for the plugin dialog".into());
    }
}

pub(crate) fn install(
    app: &PlayerWindow,
    native_host: Option<slint::Weak<NativeVideoHost>>,
    dialogs: &PluginDialogs,
) {
    let player = app.as_weak();
    dialogs.install(move |request| {
        let player = player.clone();
        let native_host = native_host.clone();
        slint::invoke_from_event_loop(move || {
            let Some(app) = player.upgrade() else {
                request.fail("The player has closed".into());
                return;
            };
            present(&app, native_host, request);
        })
        .map_err(|error| error.to_string())
    });
}

fn present(
    app: &PlayerWindow,
    native_host: Option<slint::Weak<NativeVideoHost>>,
    request: DialogRequest,
) {
    if app.global::<PlayerDialogState>().get_dialog_open() {
        request.fail("The player already has an open dialog".into());
        return;
    }
    match request {
        DialogRequest::File(request) if !request.is_cancelled() => {
            pick_file(app, native_host, request)
        }
        DialogRequest::Interaction(request) if !request.is_cancelled() => {
            interaction::present(app, native_host, request)
        }
        _ => {}
    }
}

fn pick_file(
    app: &PlayerWindow,
    native_host: Option<slint::Weak<NativeVideoHost>>,
    request: FileRequest,
) {
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

    pick_request(Some(owner.as_ref()), request).await
}

async fn pick_request(
    owner: Option<&slint::winit_030::winit::window::Window>,
    request: &FileRequest,
) -> Result<Option<String>, String> {
    if request.is_cancelled() {
        return Ok(None);
    }
    let mut dialog = rfd::AsyncFileDialog::new().set_title(&request.title);
    if let Some(owner) = owner {
        dialog = dialog.set_parent(owner);
    }
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
