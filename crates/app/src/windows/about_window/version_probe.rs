use super::{ApplicationAbout, refresh_engine_versions, reveal};
use crate::{AboutWindow, background};
use riina_core::{Playback, Preferences};
use riina_mpv::EngineVersions;
use slint::{ComponentHandle, winit_030::WinitWindowAccessor};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[derive(Default)]
pub(super) struct Probe {
    pub(super) application_view: Cell<bool>,
    pending: Cell<bool>,
    versions: RefCell<Option<EngineVersions>>,
}

impl ApplicationAbout {
    pub(crate) fn show_from_application(
        self: &Rc<Self>,
        preferences: &Preferences,
    ) -> Result<(), slint::PlatformError> {
        let window = self.window_for(None)?;
        self.timer.stop();
        self.probe.application_view.set(true);
        let theme = preferences.effective_theme_material();
        window.set_dark_theme(theme.dark_for(preferences.dark));
        reveal(&window)?;
        window.window().with_winit_window(|host| {
            let dark = host.theme().map_or(preferences.dark, |theme| {
                theme == slint::winit_030::winit::window::Theme::Dark
            });
            window.set_dark_theme(theme.dark_for(dark));
        });
        if let Some(versions) = self.probe.versions.borrow().as_ref() {
            show_versions(&window, versions);
        } else {
            refresh_engine_versions(&window, &Playback::default());
            self.start_probe();
        }
        Ok(())
    }

    fn start_probe(self: &Rc<Self>) {
        if self.probe.pending.replace(true) {
            return;
        }
        let owner = Rc::downgrade(self);
        let started = slint::spawn_local(async move {
            let result = read_versions().await;
            if let Some(owner) = owner.upgrade() {
                owner.complete_probe(result);
            }
        });
        if let Err(error) = started {
            self.complete_probe(Err(error.to_string()));
        }
    }

    fn complete_probe(&self, result: Result<EngineVersions, String>) {
        self.probe.pending.set(false);
        let window = self.window.borrow();
        let application_view = window
            .as_ref()
            .filter(|_| self.probe.application_view.get());
        match result {
            Ok(versions) => {
                if let Some(window) = application_view {
                    show_versions(window, &versions);
                }
                self.probe.versions.replace(Some(versions));
            }
            Err(error) => {
                crate::diagnostics::warn!("Could not read engine versions: {error}");
                if let Some(window) = application_view {
                    window.set_engine_label("libmpv version unavailable".into());
                    window.set_ffmpeg_label("FFmpeg version unavailable".into());
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
    window.set_engine_label(format!("libmpv {}", versions.mpv).into());
    window.set_ffmpeg_label(format!("FFmpeg {}", versions.ffmpeg).into());
}
