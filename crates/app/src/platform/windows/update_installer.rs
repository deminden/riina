//! Drive the explicit download and setup actions for an installed Windows copy.
mod package;

use super::update_setup::{self, Setup};
use crate::UpdateWindow;
use package::Downloaded;
pub(crate) use package::{Asset, Package};
use slint::ComponentHandle;
use std::{
    cell::RefCell,
    path::PathBuf,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::sync::oneshot;

#[derive(Clone)]
struct Offer {
    package: Package,
    destination: PathBuf,
}

#[derive(Default)]
enum Stage {
    #[default]
    None,
    Offered(Offer),
    Downloading(Offer, Arc<AtomicBool>),
    Ready(Offer, Downloaded),
    Setup,
}

#[derive(Default)]
pub(crate) struct InstallFlow {
    installation: Rc<Installation>,
}

#[derive(Default)]
struct Installation {
    stage: RefCell<Stage>,
}

impl InstallFlow {
    pub(crate) fn bind(&self, window: &UpdateWindow) {
        let installation = Rc::downgrade(&self.installation);
        let window_handle = window.as_weak();
        window.on_installer_action(move || {
            if let (Some(window), Some(installation)) =
                (window_handle.upgrade(), installation.upgrade())
            {
                installation.install_action(&window);
            }
        });
    }

    pub(crate) fn active(&self) -> bool {
        matches!(
            *self.installation.stage.borrow(),
            Stage::Downloading(..) | Stage::Ready(..) | Stage::Setup
        )
    }

    pub(crate) fn offer(&self, window: &UpdateWindow, package: Option<Package>) {
        let stage = match (package, update_setup::installed_destination()) {
            (Some(package), Some(destination)) => Stage::Offered(Offer {
                package,
                destination,
            }),
            _ => Stage::None,
        };
        self.installation.set(window, stage);
    }

    pub(crate) fn dismiss(&self, window: &UpdateWindow) {
        match self.installation.stage.take() {
            Stage::Downloading(offer, cancelled) => {
                self.installation.cancel(window, offer, cancelled)
            }
            Stage::Setup => self.installation.set(window, Stage::Setup),
            _ => self.installation.set(window, Stage::None),
        }
    }
}

impl Installation {
    fn cancel(&self, window: &UpdateWindow, offer: Offer, cancelled: Arc<AtomicBool>) {
        cancelled.store(true, Ordering::Relaxed);
        window.set_detail("Cancelling the installer download…".into());
        self.set(window, Stage::Downloading(offer, cancelled));
    }

    fn set(&self, window: &UpdateWindow, stage: Stage) {
        let label = match &stage {
            Stage::Offered(_) => "Download Update",
            Stage::Downloading(_, cancelled) if !cancelled.load(Ordering::Relaxed) => {
                "Cancel Download"
            }
            Stage::Ready(..) => "Open Setup",
            Stage::Downloading(..) | Stage::None | Stage::Setup => "",
        };
        self.stage.replace(stage);
        window.set_installer_action_label(label.into());
    }
}

impl Drop for Installation {
    fn drop(&mut self) {
        if let Stage::Downloading(_, cancelled) = self.stage.get_mut() {
            cancelled.store(true, Ordering::Relaxed);
        }
    }
}

impl Installation {
    fn install_action(self: &Rc<Self>, window: &UpdateWindow) {
        let stage = self.stage.take();
        match stage {
            Stage::Offered(offer) => self.download_installer(window, offer),
            Stage::Downloading(offer, cancelled) => self.cancel(window, offer, cancelled),
            Stage::Ready(offer, installer) => self.open_setup(window, offer, installer),
            stage => self.set(window, stage),
        }
    }

    fn download_installer(self: &Rc<Self>, window: &UpdateWindow, offer: Offer) {
        let cancelled = Arc::new(AtomicBool::new(false));
        let flag = cancelled.clone();
        let package = offer.package.clone();
        self.set(window, Stage::Downloading(offer, cancelled.clone()));
        window.set_detail("Downloading and verifying the Windows installer…".into());

        let (send, response) = oneshot::channel();
        if let Err(error) = crate::background::spawn_named("update-download", move || {
            let _ = send.send(package.download(&flag));
        }) {
            self.finish_download(window, &cancelled, Err(error.to_string()));
            return;
        }
        let installation = Rc::downgrade(self);
        let weak = window.as_weak();
        let token = cancelled.clone();
        if let Err(error) = slint::spawn_local(async move {
            let result = response
                .await
                .unwrap_or_else(|_| Err("The download ended early.".into()));
            if let (Some(window), Some(installation)) = (weak.upgrade(), installation.upgrade()) {
                installation.finish_download(&window, &token, result);
            }
        }) {
            cancelled.store(true, Ordering::Relaxed);
            self.finish_download(window, &cancelled, Err(error.to_string()));
        }
    }

    fn finish_download(
        &self,
        window: &UpdateWindow,
        token: &Arc<AtomicBool>,
        result: Result<Downloaded, String>,
    ) {
        let stage = match self.stage.take() {
            Stage::Downloading(offer, current)
                if Arc::ptr_eq(token, &current) && current.load(Ordering::Relaxed) =>
            {
                window.set_detail("Download cancelled.".into());
                Stage::Offered(offer)
            }
            Stage::Downloading(offer, current) if Arc::ptr_eq(token, &current) => match result {
                Ok(installer) => {
                    window.set_detail(
                        "Installer verified. Open Windows Setup to install the update.".into(),
                    );
                    Stage::Ready(offer, installer)
                }
                Err(error) => {
                    window.set_detail(format!("Could not download the update. {error}").into());
                    Stage::Offered(offer)
                }
            },
            stage => stage,
        };
        self.set(window, stage);
    }

    fn open_setup(self: &Rc<Self>, window: &UpdateWindow, offer: Offer, installer: Downloaded) {
        self.set(window, Stage::Setup);
        window.set_detail("Verifying the installer before opening Windows Setup…".into());
        let destination = offer.destination.clone();
        let (opened, opening) = oneshot::channel();
        let (exited, exit) = oneshot::channel();
        if let Err(error) = crate::background::spawn_named("update-setup", move || {
            let setup = launch_installer(&installer, destination);
            let _ = opened.send(setup.is_ok());
            let result = setup.and_then(|setup| setup.wait().map_err(|error| error.to_string()));
            let _ = exited.send((installer, result));
        }) {
            self.report_setup_error(window, offer, error.to_string());
            return;
        }
        let installation = Rc::downgrade(self);
        let weak = window.as_weak();
        let retry_offer = offer.clone();
        if let Err(error) = slint::spawn_local(async move {
            if matches!(opening.await, Ok(true))
                && let Some(window) = weak.upgrade()
            {
                window.set_detail("Follow the prompts in Windows Setup.".into());
            }
            let result = exit.await;
            if let (Some(window), Some(installation)) = (weak.upgrade(), installation.upgrade()) {
                match result {
                    Ok((_, Ok(0))) => {
                        installation.set(&window, Stage::None);
                        window.set_detail("Windows Setup completed.".into());
                    }
                    Ok((installer, Ok(code))) => {
                        installation.set(&window, Stage::Ready(offer, installer));
                        window.set_detail(
                            format!("Windows Setup closed with exit code {code}.").into(),
                        );
                    }
                    Ok((_, Err(error))) => installation.report_setup_error(&window, offer, error),
                    Err(error) => {
                        installation.report_setup_error(&window, offer, error.to_string())
                    }
                }
            }
        }) {
            self.report_setup_error(window, retry_offer, error.to_string());
        }
    }

    fn report_setup_error(&self, window: &UpdateWindow, offer: Offer, error: String) {
        self.set(window, Stage::Offered(offer));
        window.set_detail(format!("Could not open Windows Setup. {error}").into());
    }
}

fn launch_installer(installer: &Downloaded, destination: PathBuf) -> Result<Setup, String> {
    if update_setup::installed_destination().as_ref() != Some(&destination) {
        return Err("Riina's registered installation has changed.".into());
    }
    let _verified = installer
        .lock_verified()
        .map_err(|error| error.to_string())?;
    Setup::open(installer.path(), &destination).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use semver::Version;

    #[test]
    fn cancelling_waits_for_the_matching_worker_without_showing_its_error() {
        i_slint_backend_testing::init_no_event_loop();
        let install = InstallFlow::default();
        let window = UpdateWindow::new().unwrap();
        install.bind(&window);
        let assets = serde_json::from_value(serde_json::json!([{
            "name": "riina-5.0.0-windows-x86_64-setup.exe",
            "state": "uploaded",
            "size": 1,
            "digest": format!("sha256:{}", "0".repeat(64)),
        }]))
        .unwrap();
        let package = Package::select(assets, "v5.0.0", &Version::new(5, 0, 0)).unwrap();
        let token = Arc::new(AtomicBool::new(false));
        let offer = Offer {
            package,
            destination: PathBuf::new(),
        };
        install
            .installation
            .set(&window, Stage::Downloading(offer, token.clone()));

        assert_eq!(window.get_installer_action_label(), "Cancel Download");
        window.invoke_installer_action();
        assert!(token.load(Ordering::Relaxed));
        assert!(install.active());
        assert!(window.get_installer_action_label().is_empty());

        let stale = Arc::new(AtomicBool::new(false));
        install.installation.finish_download(
            &window,
            &stale,
            Err("Previous network request failed".into()),
        );
        assert!(install.active());
        install
            .installation
            .finish_download(&window, &token, Err("Interrupted".into()));
        assert!(!install.active());
        assert_eq!(window.get_installer_action_label(), "Download Update");
        assert_eq!(window.get_detail(), "Download cancelled.");
    }
}
