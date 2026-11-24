//! Review and install packages selected in Settings or handed over by the desktop.
mod transaction;

use super::{
    Playback, PluginSettings, current_runtime_supports,
    inventory::{load_enabled, refresh},
    permissions::describe_permissions,
};
use crate::{PreferencesWindow, plugin_host::GlobalHandle, plugin_packages::StagedPackage};
use rfd::{AsyncFileDialog, AsyncMessageDialog, MessageButtons, MessageDialogResult};
use slint::{ComponentHandle, Weak, winit_030::WinitWindowAccessor};
use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    rc::Rc,
};
use transaction::{Installed, Profile, Review};

enum Source {
    ChooseArchive,
    Archive(PathBuf),
    Repository(String),
}

impl Source {
    fn stage(self, profile: &Profile) -> Result<StagedPackage, String> {
        match self {
            Self::Archive(path) => crate::plugin_packages::stage_archive(&path, profile.plugins())
                .map_err(|error| format!("Package rejected: {error}")),
            Self::Repository(source) => {
                crate::plugin_github::stage_repository(&source, profile.plugins())
                    .map_err(|error| format!("GitHub plugin could not be installed: {error}"))
            }
            Self::ChooseArchive => unreachable!("The file dialog resolves the archive first"),
        }
    }
}

struct Installer {
    window: Weak<PreferencesWindow>,
    settings: PluginSettings,
    profile: Profile,
    playback: Option<Playback>,
    active: RefCell<Option<Rc<Cell<Phase>>>>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Reviewing,
    Installing,
    Closed,
}

struct Request {
    installer: Rc<Installer>,
    phase: Rc<Cell<Phase>>,
}

impl Drop for Request {
    fn drop(&mut self) {
        if self.phase.get() != Phase::Closed {
            self.installer.finish_request();
        }
    }
}

pub(super) fn connect(
    window: &PreferencesWindow,
    settings: &PluginSettings,
    host: Option<&GlobalHandle>,
    playback: Option<Playback>,
) {
    let installer = Rc::new(Installer {
        window: window.as_weak(),
        settings: settings.clone(),
        profile: Profile::new(
            settings.plugins.clone(),
            settings.preferences_path.clone(),
            settings.enabled_path.clone(),
            host.cloned(),
        ),
        playback,
        active: RefCell::default(),
    });
    let picker = installer.clone();
    window.on_plugin_request_local_install(move || picker.start(Source::ChooseArchive));
    let archive = installer.clone();
    window.on_plugin_install_archive(move |path| {
        archive.start(Source::Archive(PathBuf::from(path.as_str())));
    });
    let weak = window.as_weak();
    window.on_plugin_request_github_install(move || {
        if let Some(window) = weak.upgrade()
            && !window.get_plugin_github_installing()
        {
            window.set_plugin_github_repository("".into());
            window.set_plugin_github_sheet_open(true);
        }
    });
    let github = installer.clone();
    window.on_plugin_github_install_requested(move |source| {
        github.start(Source::Repository(source.to_string()));
    });
    window.on_plugin_cancel_import(move || installer.cancel_review());
}

impl Installer {
    fn start(self: &Rc<Self>, source: Source) {
        let Some(window) = self.window.upgrade() else {
            return;
        };
        if self.active.borrow().is_some() || window.get_plugin_github_installing() {
            window.set_plugin_update_status(
                "Finish the current plugin operation before opening another package.".into(),
            );
            return;
        }
        window.set_plugin_github_installing(true);
        let phase = Rc::new(Cell::new(Phase::Reviewing));
        self.active.replace(Some(phase.clone()));
        let request = Request {
            installer: self.clone(),
            phase,
        };
        if let Err(error) = slint::spawn_local(async move {
            let result = request.run(source).await;
            if let Some(window) = request.settings() {
                match result {
                    Ok(Some(installed)) => request.installer.finish(&window, installed),
                    Ok(None) => {
                        window.set_plugin_update_status("Plugin installation cancelled.".into())
                    }
                    Err(error) => window.set_plugin_update_status(error.into()),
                }
            }
        }) {
            window
                .set_plugin_update_status(format!("Could not start plugin import: {error}").into());
        }
    }

    fn cancel_review(&self) {
        let reviewing = self
            .active
            .borrow()
            .as_ref()
            .is_some_and(|phase| phase.get() == Phase::Reviewing);
        if reviewing {
            self.finish_request();
            if let Some(window) = self.window.upgrade() {
                window.set_plugin_github_sheet_open(false);
            }
        }
    }

    fn finish_request(&self) {
        let phase = self.active.borrow_mut().take();
        if let Some(phase) = phase {
            phase.set(Phase::Closed);
            if let Some(window) = self.window.upgrade() {
                window.set_plugin_github_installing(false);
            }
        }
    }

    fn finish(&self, window: &PreferencesWindow, installed: Installed) {
        let settings = &self.settings;
        *settings.enabled.borrow_mut() = load_enabled(&settings.enabled_path);
        refresh(
            window,
            &settings.plugins,
            &settings.inventory,
            &settings.enabled.borrow(),
            settings.runtime_available,
        );
        let action = if installed.replaced {
            "Reinstalled"
        } else {
            "Installed"
        };
        let name = &installed.manifest.name;
        let status = if installed.enabled {
            format!("{action} and enabled {name}.")
        } else {
            format!(
                "{action} {name}. The plugin remains disabled because its runtime is unavailable."
            )
        };
        window.set_plugin_update_status(status.clone().into());
        if let Some(playback) = &self.playback {
            if installed.manifest.identifier == "io.iina.ytdl" {
                playback.ytdl_changed(installed.enabled);
            }
            if let Some(player) = playback.window.upgrade() {
                crate::diagnostics::report(&player, crate::diagnostics::UserMessage::info(status));
            }
        }
    }
}

impl Request {
    fn settings(&self) -> Option<PreferencesWindow> {
        if self.phase.get() == Phase::Closed {
            return None;
        }
        self.installer.window.upgrade()
    }

    fn window(&self) -> Option<PreferencesWindow> {
        self.settings()
            .filter(|window| window.window().is_visible())
    }

    async fn run(&self, source: Source) -> Result<Option<Installed>, String> {
        let source = match source {
            Source::ChooseArchive => {
                let Some(window) = self.window() else {
                    return Ok(None);
                };
                let owner = window
                    .window()
                    .winit_window()
                    .await
                    .map_err(|error| error.to_string())?;
                drop(window);
                let file = AsyncFileDialog::new()
                    .set_title("Install local plugin package")
                    .set_parent(owner.as_ref())
                    .add_filter("Riina plugin packages", &["iinaplgz"])
                    .pick_file()
                    .await;
                let Some(file) = file else { return Ok(None) };
                Source::Archive(file.path().to_owned())
            }
            source => source,
        };
        let Some(window) = self.window() else {
            return Ok(None);
        };
        match &source {
            Source::Repository(source) => {
                window.set_plugin_update_status(format!("Downloading {source}…").into());
            }
            _ => window.set_plugin_update_status("Reading plugin package…".into()),
        }
        drop(window);
        let profile = self.installer.profile.clone();
        let review = run_worker("plugin-import-stage", move || {
            profile.review(source.stage(&profile)?)
        })
        .await?;
        if !self.confirm(&review).await? {
            return Ok(None);
        }
        let Some(window) = self.window() else {
            return Ok(None);
        };
        self.phase.set(Phase::Installing);
        window.set_plugin_update_status(format!("Installing {}…", review.manifest().name).into());
        drop(window);
        let profile = self.installer.profile.clone();
        run_worker("plugin-import-commit", move || profile.install(review))
            .await
            .map(Some)
    }

    async fn confirm(&self, review: &Review) -> Result<bool, String> {
        let Some(window) = self.window() else {
            return Ok(false);
        };
        window.set_plugin_github_sheet_open(false);
        let owner = window
            .window()
            .winit_window()
            .await
            .map_err(|error| error.to_string())?;
        let manifest = review.manifest();
        let text = window.global::<riina_ui::PluginReviewText>();
        let title = text.invoke_install_title(manifest.name.as_str().into(), review.replaces());
        let can_enable =
            self.installer.settings.runtime_available && current_runtime_supports(manifest);
        let effect = if can_enable {
            text.get_will_enable()
        } else {
            text.get_runtime_unavailable()
        };
        let permissions = describe_permissions(&window, manifest);
        let permissions = if permissions.is_empty() {
            text.get_no_additional_permissions().to_string()
        } else {
            permissions
        };
        drop(window);
        let result = AsyncMessageDialog::new()
            .set_parent(owner.as_ref())
            .set_title(title.as_str())
            .set_description(format!("{effect}\n\n{permissions}"))
            .set_buttons(MessageButtons::YesNo)
            .show()
            .await;
        Ok(result == MessageDialogResult::Yes)
    }
}

async fn run_worker<T: Send + 'static>(
    title: &'static str,
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    let (complete, result) = tokio::sync::oneshot::channel();
    crate::background::spawn_named(title, move || {
        let _ = complete.send(work());
    })
    .map_err(|error| error.to_string())?;
    result.await.map_err(|error| error.to_string())?
}

#[cfg(test)]
mod tests;
