use crate::{AboutWindow, about_window::refresh_engine_versions, background};
use riina_core::Playback;
use riina_mpv::EngineVersions;
use riina_ui::EngineVersionStatus;
use slint::ComponentHandle;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[derive(Default)]
pub(crate) struct Probe(Rc<State>);

#[derive(Default)]
struct State {
    application_view: RefCell<Option<slint::Weak<AboutWindow>>>,
    pending: Cell<bool>,
    versions: RefCell<Option<EngineVersions>>,
}

impl Probe {
    pub(crate) fn use_player(&self) {
        self.0.application_view.take();
    }

    pub(crate) fn use_application(&self, window: &AboutWindow) {
        self.0.application_view.replace(Some(window.as_weak()));
    }

    pub(crate) fn request(&self, window: &AboutWindow) {
        if let Some(versions) = self.0.versions.borrow().as_ref() {
            show_versions(window, versions);
        } else {
            refresh_engine_versions(window, &Playback::default());
            self.0.start();
        }
    }
}

impl State {
    fn start(self: &Rc<Self>) {
        if self.pending.replace(true) {
            return;
        }
        let owner = Rc::downgrade(self);
        let started = slint::spawn_local(async move {
            let result = read_versions().await;
            if let Some(owner) = owner.upgrade() {
                owner.complete(result);
            }
        });
        if let Err(error) = started {
            self.complete(Err(error.to_string()));
        }
    }

    fn complete(&self, result: Result<EngineVersions, String>) {
        self.pending.set(false);
        let application_view = self
            .application_view
            .borrow()
            .as_ref()
            .and_then(slint::Weak::upgrade);
        match result {
            Ok(versions) => {
                if let Some(window) = application_view {
                    show_versions(&window, &versions);
                }
                self.versions.replace(Some(versions));
            }
            Err(error) => {
                crate::diagnostics::warn!("Could not read engine versions: {error}");
                if let Some(window) = application_view {
                    window.set_engine_version_status(EngineVersionStatus::Unavailable);
                }
            }
        }
    }
}

async fn read_versions() -> Result<EngineVersions, String> {
    let (ready, result) = tokio::sync::oneshot::channel();
    background::spawn_named("about-engine-versions", move || {
        let _ = ready.send(riina_mpv::read_engine_versions(
            &crate::platform::playback_library(),
        ));
    })
    .map_err(|error| error.to_string())?;
    result
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())
}

fn show_versions(window: &AboutWindow, versions: &EngineVersions) {
    window.set_engine_version(versions.mpv.as_str().into());
    window.set_ffmpeg_version(versions.ffmpeg.as_str().into());
}
