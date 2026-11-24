//! Own one release checker and window for the application lifetime.
mod check;
#[cfg(target_os = "windows")]
mod install;
#[cfg(target_os = "windows")]
mod package;
mod releases;
mod schedule;
mod settings;

use crate::{
    NativeVideoHost, PlayerWindow, PreferencesWindow, UpdateWindow, platform::NativePresentation,
};
use riina_ui::{PlayerAppearance, PlayerMenuActions};
use schedule::{Permission, State, Store};
use slint::{ComponentHandle, Timer, TimerMode, Weak, winit_030::WinitWindowAccessor};
use std::{
    cell::{Cell, OnceCell, RefCell},
    io,
    path::Path,
    rc::Rc,
    time::Duration,
};

const STARTUP_DELAY: Duration = Duration::from_secs(3);

#[derive(Default)]
pub(crate) struct ApplicationUpdates {
    window: RefCell<Option<UpdateWindow>>,
    settings: RefCell<Vec<Weak<PreferencesWindow>>>,
    owners: RefCell<Vec<Owner>>,
    store: OnceCell<Store>,
    timer: Timer,
    consent_due: Cell<bool>,
    running: Cell<Option<check::Running>>,
    pub(crate) closed: crate::windows::close_observer::CloseObserver,
    application_theme: Cell<Option<(riina_core::ThemeMaterial, bool)>>,
    #[cfg(target_os = "windows")]
    install: install::InstallFlow,
}

#[derive(Clone)]
struct Owner {
    player: Weak<PlayerWindow>,
    host: Option<Weak<NativeVideoHost>>,
}

impl ApplicationUpdates {
    pub(crate) fn connect(
        self: &Rc<Self>,
        app: &PlayerWindow,
        presentation: Option<&NativePresentation>,
        preferences_path: &Path,
    ) {
        let owner = Owner {
            player: app.as_weak(),
            host: presentation.map(NativePresentation::host),
        };
        self.owners
            .borrow_mut()
            .retain(|owner| owner.player.upgrade().is_some());
        self.owners.borrow_mut().push(owner.clone());
        self.initialize_schedule(preferences_path);
        let updates = Rc::downgrade(self);
        app.global::<PlayerMenuActions>()
            .on_check_for_updates(move || {
                if let (Some(updates), Some(app)) = (updates.upgrade(), owner.player.upgrade())
                    && let Err(error) = updates.show_manual(Some(&owner))
                {
                    crate::diagnostics::report(
                        &app,
                        format!("Could not open Software Update: {error}"),
                    );
                }
            });
    }

    fn initialize_schedule(self: &Rc<Self>, preferences_path: &Path) {
        if self
            .store
            .set(Store::for_preferences(preferences_path))
            .is_ok()
        {
            match self.change(State::should_ask) {
                Ok(ask) => self.consent_due.set(ask),
                Err(error) => crate::diagnostics::warn!("Could not load update schedule: {error}"),
            }
            self.refresh_schedule();
        }
    }

    #[cfg(any(test, target_os = "macos"))]
    pub(crate) fn configure_application(
        self: &Rc<Self>,
        preferences_path: &Path,
        preferences: &riina_core::Preferences,
    ) {
        self.application_theme.set(Some((
            preferences.effective_theme_material(),
            preferences.dark,
        )));
        self.initialize_schedule(preferences_path);
    }

    #[cfg(any(test, target_os = "macos"))]
    pub(crate) fn show_from_application(self: &Rc<Self>) -> Result<(), slint::PlatformError> {
        self.show_manual(None)
    }

    fn show_manual(self: &Rc<Self>, owner: Option<&Owner>) -> Result<(), slint::PlatformError> {
        let window = self.window()?;
        let start = !window.window().is_visible() || window.get_asking_permission();
        self.consent_due.set(false);
        if start && !self.install_in_progress() {
            self.request_manual(&window);
        }
        self.reveal(&window, owner)?;
        window
            .window()
            .with_winit_window(|native| native.focus_window());
        window.invoke_focus_dismiss();
        Ok(())
    }

    fn window(self: &Rc<Self>) -> Result<UpdateWindow, slint::PlatformError> {
        let mut slot = self.window.borrow_mut();
        if let Some(window) = slot.as_ref() {
            return Ok(window.clone_strong());
        }
        let window = UpdateWindow::new()?;
        crate::appearance_preferences::install_system_typography(&window);
        let weak = window.as_weak();
        let updates = Rc::downgrade(self);
        window.on_retry(move || {
            if let (Some(window), Some(updates)) = (weak.upgrade(), updates.upgrade()) {
                updates.request_manual(&window);
            }
        });
        #[cfg(target_os = "windows")]
        {
            let weak = window.as_weak();
            let updates = Rc::downgrade(self);
            window.on_installer_action(move || {
                if let (Some(window), Some(updates)) = (weak.upgrade(), updates.upgrade()) {
                    updates.install_action(&window);
                }
            });
        }
        let weak = window.as_weak();
        let updates = Rc::downgrade(self);
        window.on_dismiss(move || {
            if let (Some(window), Some(updates)) = (weak.upgrade(), updates.upgrade())
                && updates.can_dismiss(&window)
            {
                let _ = window.hide();
                updates.closed.notify();
            }
        });
        let weak = window.as_weak();
        let updates = Rc::downgrade(self);
        window.on_set_permission(move |enabled| {
            if let (Some(window), Some(updates)) = (weak.upgrade(), updates.upgrade())
                && updates.answer_consent(&window, enabled)
            {
                let _ = window.hide();
                updates.closed.notify();
            }
        });
        let weak = window.as_weak();
        let updates = Rc::downgrade(self);
        window.window().on_close_requested(move || {
            if let (Some(window), Some(updates)) = (weak.upgrade(), updates.upgrade()) {
                if !updates.can_dismiss(&window) {
                    return slint::CloseRequestResponse::KeepWindowShown;
                }
                updates.closed.notify();
            }
            slint::CloseRequestResponse::HideWindow
        });
        *slot = Some(window.clone_strong());
        Ok(window)
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn is_visible(&self) -> bool {
        self.window
            .borrow()
            .as_ref()
            .is_some_and(|window| window.window().is_visible())
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn hide_for_quit(&self) {
        self.timer.stop();
        self.running.take();
        if let Some(window) = self.window.borrow().as_ref() {
            let _ = window.hide();
        }
    }

    fn can_dismiss(self: &Rc<Self>, window: &UpdateWindow) -> bool {
        #[cfg(target_os = "windows")]
        self.install.dismiss(window);
        !window.get_asking_permission() || self.answer_consent(window, false)
    }

    fn answer_consent(self: &Rc<Self>, window: &UpdateWindow, enabled: bool) -> bool {
        if let Err(error) = self.set_permission(enabled) {
            window.set_detail(format!("Could not save the update preference: {error}").into());
            return false;
        }
        window.set_asking_permission(false);
        true
    }

    fn refresh_schedule(self: &Rc<Self>) {
        self.timer.stop();
        if self.running.get().is_some() {
            return;
        }
        let delay = if self.consent_due.get() {
            Some(STARTUP_DELAY)
        } else {
            self.state()
                .ok()
                .and_then(|state| state.wait(schedule::now()))
                .map(|wait| wait.clamp(STARTUP_DELAY, Duration::from_secs(3_600)))
        };
        if let Some(delay) = delay {
            let weak = Rc::downgrade(self);
            self.timer.start(TimerMode::SingleShot, delay, move || {
                if let Some(updates) = weak.upgrade() {
                    updates.run_scheduled();
                }
            });
        }
    }

    fn run_scheduled(self: &Rc<Self>) {
        if self.consent_due.replace(false)
            && self
                .state()
                .is_ok_and(|state| state.permission == Permission::Undecided)
        {
            if let Err(error) = self.show_consent() {
                crate::diagnostics::warn!("Could not open update consent window: {error}");
            }
        } else {
            match self.change(|state| state.claim_due(schedule::now())) {
                Ok(Some(channel)) => self.start(check::Running::automatic(channel)),
                Ok(None) => self.refresh_schedule(),
                Err(error) => {
                    crate::diagnostics::warn!("Could not claim scheduled update: {error}")
                }
            }
        }
    }

    fn show_consent(self: &Rc<Self>) -> Result<(), slint::PlatformError> {
        let window = self.window()?;
        window.set_asking_permission(true);
        window.set_headline("Check for updates automatically?".into());
        window.set_detail("Riina can check for new releases in the background.".into());
        self.reveal(&window, self.owner().as_ref())?;
        window.invoke_focus_dismiss();
        Ok(())
    }

    fn owner(&self) -> Option<Owner> {
        let owners = self.owners.borrow();
        owners
            .iter()
            .rev()
            .find(|owner| {
                owner
                    .player
                    .upgrade()
                    .is_some_and(|player| player.window().is_visible())
                    || owner
                        .host
                        .as_ref()
                        .and_then(Weak::upgrade)
                        .is_some_and(|host| host.window().is_visible())
            })
            .or_else(|| {
                owners
                    .iter()
                    .rev()
                    .find(|owner| owner.player.upgrade().is_some())
            })
            .cloned()
    }

    fn state(&self) -> io::Result<State> {
        self.store
            .get()
            .map_or_else(|| Ok(State::default()), Store::read)
    }

    fn change<T>(&self, edit: impl FnOnce(&mut State) -> T) -> io::Result<T> {
        self.store
            .get()
            .ok_or_else(|| io::Error::other("Software Update was not initialized"))?
            .change(edit)
    }

    fn install_in_progress(&self) -> bool {
        #[cfg(target_os = "windows")]
        return self.install.active();
        #[cfg(not(target_os = "windows"))]
        false
    }
    fn reveal(
        &self,
        window: &UpdateWindow,
        owner: Option<&Owner>,
    ) -> Result<(), slint::PlatformError> {
        let app = owner.and_then(|owner| owner.player.upgrade());
        if let Some(app) = app.as_ref() {
            window.set_dark_theme(app.global::<PlayerAppearance>().get_dark_theme());
            if !window.window().is_visible() {
                if let Some(host) = owner
                    .and_then(|owner| owner.host.as_ref())
                    .and_then(Weak::upgrade)
                {
                    crate::platform::center_auxiliary_window(window, &host);
                } else {
                    crate::platform::center_auxiliary_window(window, app);
                }
            }
        }
        let application_theme = app
            .is_none()
            .then_some(self.application_theme.get())
            .flatten();
        if let Some((theme, dark)) = application_theme {
            window.set_dark_theme(theme.dark_for(dark));
        }
        if !window.window().is_visible() {
            crate::platform::show_window(window)?;
        }
        if let Some((theme, fallback_dark)) = application_theme {
            window.window().with_winit_window(|host| {
                let dark = host.theme().map_or(fallback_dark, |theme| {
                    theme == slint::winit_030::winit::window::Theme::Dark
                });
                window.set_dark_theme(theme.dark_for(dark));
            });
        }
        Ok(())
    }
}

fn present(
    updates: &ApplicationUpdates,
    window: &UpdateWindow,
    result: Result<releases::Outcome, String>,
) {
    #[cfg(target_os = "windows")]
    updates.install.dismiss(window);
    #[cfg(not(target_os = "windows"))]
    let _ = updates;
    window.set_asking_permission(false);
    window.set_checking(false);
    window.set_release_available(false);
    match result {
        Ok(releases::Outcome::Available(release)) => {
            #[cfg(target_os = "windows")]
            updates.install.offer(window, release.installer);
            window.set_headline(format!("Riina {} is available", release.version).into());
            window.set_detail(format!("Installed version: {}", env!("CARGO_PKG_VERSION")).into());
            let weak = window.as_weak();
            window.on_open_release(move || {
                if let Some(window) = weak.upgrade()
                    && window.get_release_available()
                    && let Err(error) = crate::platform::open_http_url_in_browser(&release.url)
                {
                    window.set_detail(format!("Could not open the release page: {error}").into());
                }
            });
            window.set_release_available(true);
        }
        Ok(releases::Outcome::Current(latest)) => {
            window.set_headline("No newer Riina release was found".into());
            window.set_detail(
                format!(
                    "Installed: {}. Latest release: {latest}.",
                    env!("CARGO_PKG_VERSION")
                )
                .into(),
            );
        }
        Err(error) => {
            window.set_headline("Could not check for updates".into());
            window.set_detail(error.into());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use semver::Version;
    use slint::platform::{Key, WindowEvent};

    #[test]
    fn update_window_reflects_each_result_and_escape_dismisses_it() {
        i_slint_backend_testing::init_no_event_loop();
        let updates = Rc::new(ApplicationUpdates::default());
        let window = updates.window().unwrap();
        window.show().unwrap();
        window.invoke_focus_dismiss();

        present(
            &updates,
            &window,
            Ok(releases::Outcome::Available(releases::Release {
                version: Version::new(5, 0, 0),
                url: "https://github.com/deminden/riina/releases/tag/v5.0.0".into(),
                #[cfg(target_os = "windows")]
                installer: None,
            })),
        );
        assert!(window.get_release_available());
        assert_eq!(window.get_headline(), "Riina 5.0.0 is available");

        present(
            &updates,
            &window,
            Ok(releases::Outcome::Current(Version::new(4, 2, 1))),
        );
        assert!(!window.get_release_available());
        assert!(window.get_detail().contains("Latest release: 4.2.1"));

        window.set_checking(true);
        present(&updates, &window, Err("Test feed offline".into()));
        assert!(!window.get_checking());
        assert_eq!(window.get_detail(), "Test feed offline");
        window.window().dispatch_event(WindowEvent::KeyPressed {
            text: Key::Escape.into(),
        });
        assert!(!window.window().is_visible());
    }
}
