mod credentials;
mod external;
mod request;

pub(crate) use request::PendingStream;

use crate::{PlayerWindow, WelcomeWindow, platform};
use credentials::{Account, Origin};
use request::{RunningStream, StreamMode};
use riina_core::stream_address::stream_address;
use riina_mpv::StreamStatus;
use riina_ui::{OpenUrlWindow, PlayerAppearance, PlayerMenuActions};
use slint::{
    CloseRequestResponse, ComponentHandle, Timer, TimerMode, winit_030::WinitWindowAccessor,
};
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
    time::Duration,
};

type Launch = dyn Fn(&PlayerWindow, PendingStream) -> Result<(), String>;
type Current = dyn Fn(&PlayerWindow, PendingStream);

struct Destination {
    player: slint::Weak<PlayerWindow>,
    welcome: Option<slint::Weak<WelcomeWindow>>,
    current: Rc<Current>,
    route: Route,
    mode: StreamMode,
}

enum Route {
    Menu { alternative: bool },
    Current,
}

#[derive(Default)]
pub(crate) struct ApplicationOpenUrl {
    window: RefCell<Option<OpenUrlWindow>>,
    destination: RefCell<Option<Destination>>,
    running: RefCell<Option<RunningStream>>,
    launch_new: RefCell<Option<Rc<Launch>>>,
    credential_origin: RefCell<Option<Origin>>,
    credential_timer: Timer,
    progress: Timer,
}

impl ApplicationOpenUrl {
    pub(crate) fn bind(
        &self,
        launch: impl Fn(&PlayerWindow, PendingStream) -> Result<(), String> + 'static,
    ) {
        self.launch_new.replace(Some(Rc::new(launch)));
    }

    pub(crate) fn connect(
        self: &Rc<Self>,
        app: &PlayerWindow,
        welcome: Option<&WelcomeWindow>,
        current: impl Fn(&PlayerWindow, PendingStream) + 'static,
    ) {
        let weak = app.as_weak();
        let welcome_handle = welcome.map(WelcomeWindow::as_weak);
        let current: Rc<Current> = Rc::new(current);
        let invoke = {
            let owner = self.clone();
            move |alternative| {
                if let Some(app) = weak.upgrade() {
                    let destination = Destination {
                        player: weak.clone(),
                        welcome: welcome_handle.clone(),
                        current: current.clone(),
                        route: Route::Menu { alternative },
                        mode: StreamMode::Manual,
                    };
                    if let Err(error) = owner.show(&app, destination) {
                        crate::diagnostics::report(
                            &app,
                            format!("Could not show Open URL: {error}"),
                        );
                    }
                }
            }
        };
        let normal = invoke.clone();
        let menu = app.global::<PlayerMenuActions>();
        menu.on_open_url_dialog(move || normal(false));
        let alternate = invoke.clone();
        menu.on_open_url_alternative(move || alternate(true));
        if let Some(welcome) = welcome {
            welcome.on_open_url(move || invoke(false));
        }
    }

    fn show(
        self: &Rc<Self>,
        app: &PlayerWindow,
        destination: Destination,
    ) -> Result<(), slint::PlatformError> {
        let window = self.window_for(app, destination.welcome.as_ref())?;
        if self.running.borrow().is_none() {
            self.destination.replace(Some(destination));
            self.reset_draft(&window);
        }
        Self::reveal(&window, app)
    }

    fn window_for(
        self: &Rc<Self>,
        app: &PlayerWindow,
        welcome: Option<&slint::Weak<WelcomeWindow>>,
    ) -> Result<OpenUrlWindow, slint::PlatformError> {
        let existing = self
            .window
            .borrow()
            .as_ref()
            .map(ComponentHandle::clone_strong);
        if let Some(window) = existing {
            return Ok(window);
        }
        let window = OpenUrlWindow::new()?;
        crate::appearance_preferences::install_system_typography(&window);
        self.connect_window(&window);
        if let Some(welcome) = welcome.and_then(slint::Weak::upgrade)
            && welcome.window().is_visible()
        {
            platform::center_auxiliary_window(&window, &welcome);
        } else {
            platform::center_auxiliary_window(&window, app);
        }
        self.window.replace(Some(window.clone_strong()));
        Ok(window)
    }

    fn reveal(window: &OpenUrlWindow, app: &PlayerWindow) -> Result<(), slint::PlatformError> {
        window.set_dark_theme(app.global::<PlayerAppearance>().get_dark_theme());
        platform::show_window(window)?;
        window.window().with_winit_window(|host| {
            host.set_minimized(false);
            host.focus_window();
        });
        if window.get_loading() {
            window.invoke_focus_stop();
        } else {
            window.invoke_focus_address();
        }
        Ok(())
    }

    fn connect_window(self: &Rc<Self>, window: &OpenUrlWindow) {
        window.on_address_edited(window_action(self, window, Self::address_changed));
        window.on_submit(window_action(self, window, Self::submit));
        window.on_stop(window_action(self, window, |owner, window| {
            owner.stop(window)
        }));
        window.on_cancel(window_action(self, window, |owner, window| {
            owner.hide(window)
        }));
        let owner = Rc::downgrade(self);
        let component = window.as_weak();
        window.window().on_close_requested(move || {
            if let Some((owner, window)) = reconnect(&owner, &component) {
                owner.stop(&window);
                owner.reset_draft(&window);
                owner.destination.take();
            }
            CloseRequestResponse::HideWindow
        });
    }

    fn reset_draft(&self, window: &OpenUrlWindow) {
        self.credential_timer.stop();
        self.credential_origin.take();
        window.set_address("".into());
        window.set_username("".into());
        window.set_password("".into());
        window.set_remember(false);
        window.set_message("".into());
        window.set_show_default_scheme(false);
        window.set_authentication_available(false);
        window.set_valid_address(false);
    }

    fn stop(&self, window: &OpenUrlWindow) {
        self.progress.stop();
        if let Some(running) = self.running.take() {
            running.cancel();
        }
        window.set_loading(false);
        window.invoke_focus_address();
    }

    fn hide(&self, window: &OpenUrlWindow) {
        self.stop(window);
        self.reset_draft(window);
        self.destination.take();
        let _ = window.hide();
    }

    fn address_changed(self: &Rc<Self>, window: &OpenUrlWindow) {
        let draft = window.get_address();
        let address = stream_address(draft.as_str());
        window.set_message(if address.is_err() && !draft.trim().is_empty() {
            "Enter a valid stream URL".into()
        } else {
            "".into()
        });
        window.set_valid_address(address.is_ok());
        window.set_show_default_scheme(address.as_ref().is_ok_and(|url| url.uses_default_scheme));
        let origin = address.ok().and_then(|url| Origin::from_address(&url.url));
        window.set_authentication_available(origin.is_some());
        if *self.credential_origin.borrow() == origin {
            return;
        }
        self.credential_origin.replace(origin.clone());
        self.credential_timer.stop();
        window.set_username("".into());
        window.set_password("".into());
        window.set_remember(false);
        if let Some(origin) = origin {
            credentials::restore(&self.credential_timer, window, origin);
        }
    }

    fn submit(self: &Rc<Self>, window: &OpenUrlWindow) {
        if self.running.borrow().is_some() {
            return;
        }
        let draft = window.get_address();
        let Ok(address) = stream_address(draft.as_str()) else {
            return;
        };
        let destination = self.destination.borrow();
        let Some(target) = destination.as_ref() else {
            return;
        };
        let Some(player) = target.player.upgrade() else {
            window.set_message(
                "The player window closed. Open this dialog from another player.".into(),
            );
            return;
        };

        let account = Account {
            username: window.get_username().to_string(),
            password: window.get_password().to_string(),
        };
        let source = credentials::source(&address.url, &account);
        if window.get_remember()
            && !account.username.is_empty()
            && let Some(origin) = Origin::from_address(&source.address)
        {
            credentials::remember(window, origin, account);
        }
        window.set_password("".into());
        self.credential_timer.stop();

        let (request, running) = PendingStream::new(source, target.mode.clone());
        if let Err(error) = self.launch(target, &player, request) {
            window.set_message(error.into());
            return;
        }
        self.show_progress(window, running);
    }

    fn show_progress(self: &Rc<Self>, window: &OpenUrlWindow, running: RunningStream) {
        self.running.replace(Some(running));
        window.set_loading(true);
        window.invoke_focus_stop();
        let weak = window.as_weak();
        let owner = Rc::downgrade(self);
        self.progress
            .start(TimerMode::Repeated, Duration::from_millis(60), move || {
                if let Some((owner, window)) = reconnect(&owner, &weak) {
                    owner.observe_progress(&window);
                }
            });
    }

    fn launch(
        &self,
        target: &Destination,
        player: &PlayerWindow,
        request: PendingStream,
    ) -> Result<(), String> {
        let launch = self.launch_new.borrow().clone();
        if let Route::Menu { alternative } = target.route
            && let Some(launch) = launch
            && crate::media_open::stream_in_new_player(player, alternative)?
        {
            launch(player, request)
        } else {
            (target.current)(player, request);
            Ok(())
        }
    }

    fn observe_progress(&self, window: &OpenUrlWindow) {
        let status = self.running.borrow().as_ref().map(RunningStream::status);
        match status {
            Some(StreamStatus::Opened) => self.hide(window),
            Some(StreamStatus::Failed | StreamStatus::Cancelled) => {
                self.stop(window);
                window.set_message("Cannot open this file or stream.".into());
            }
            _ => {}
        }
    }
}

fn reconnect(
    owner: &Weak<ApplicationOpenUrl>,
    window: &slint::Weak<OpenUrlWindow>,
) -> Option<(Rc<ApplicationOpenUrl>, OpenUrlWindow)> {
    Some((owner.upgrade()?, window.upgrade()?))
}

fn window_action(
    owner: &Rc<ApplicationOpenUrl>,
    window: &OpenUrlWindow,
    action: fn(&Rc<ApplicationOpenUrl>, &OpenUrlWindow),
) -> impl Fn() + 'static {
    let weak = window.as_weak();
    let owner = Rc::downgrade(owner);
    move || {
        if let Some((owner, window)) = reconnect(&owner, &weak) {
            action(&owner, &window);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn welcome_and_player_menus_share_the_validated_stream_dialog()
    -> Result<(), Box<dyn std::error::Error>> {
        i_slint_backend_testing::init_no_event_loop();
        let open_url = Rc::new(ApplicationOpenUrl::default());
        let first = PlayerWindow::new()?;
        let second = PlayerWindow::new()?;
        let welcome = WelcomeWindow::new()?;
        welcome.show()?;
        let selected = Rc::new(Cell::new(0));
        let observed = selected.clone();
        open_url.connect(&first, Some(&welcome), move |_, _| observed.set(1));
        let observed = selected.clone();
        open_url.connect(&second, None, move |_, _| observed.set(2));

        welcome.invoke_open_url();
        let window = open_url.window.borrow().as_ref().unwrap().clone_strong();
        assert!(window.window().is_visible());
        window.set_address("file:///local/movie.avi".into());
        window.invoke_address_edited();
        window.invoke_submit();
        assert!(!window.get_valid_address());
        assert_eq!(selected.get(), 0);

        second
            .global::<PlayerMenuActions>()
            .invoke_open_url_dialog();
        assert!(window.window().is_visible());
        assert!(window.get_address().is_empty());
        window.set_address("media.example:8000/movie.avi".into());
        window.invoke_address_edited();
        assert!(window.get_show_default_scheme());
        window.invoke_submit();
        assert_eq!(selected.get(), 2);
        assert!(window.get_loading());
        // The fake receiver drops the request, like a window closing before launch.
        open_url.observe_progress(&window);
        assert!(!window.get_loading());
        assert_eq!(window.get_address(), "media.example:8000/movie.avi");
        assert!(!window.get_message().is_empty());
        window.invoke_cancel();
        assert!(!window.window().is_visible());
        welcome.hide()?;
        Ok(())
    }

    #[test]
    fn overlapping_external_loads_keep_independent_cancellation_and_retry_options()
    -> Result<(), Box<dyn std::error::Error>> {
        use riina_core::{Command, MediaLink};
        use riina_mpv::CommandSender;

        i_slint_backend_testing::init_no_event_loop();
        let owner = Rc::new(ApplicationOpenUrl::default());
        let first = PlayerWindow::new()?;
        let second = PlayerWindow::new()?;
        let (initial, initial_engine) = CommandSender::recording(8);
        let (browser, browser_engine) = CommandSender::recording(8);
        let first_address = "https://example.test/first";
        owner.open_initial_stream(&first, &initial, first_address, Some(3.0))?;
        assert_eq!(
            initial_engine.commands.try_recv()?,
            Command::OpenMediaLink(MediaLink {
                path: first_address.into(),
                start: Some(3.0),
                ..MediaLink::default()
            })
        );
        let first_load = owner.running.borrow().as_ref().unwrap().clone();
        let window = owner.window.borrow().as_ref().unwrap().clone_strong();

        let mut link = MediaLink {
            path: "https://example.test/second".into(),
            pause: Some(true),
            speed: Some(1.25),
            ..MediaLink::default()
        };
        owner.open_browser_stream(&second, &browser, link.clone())?;
        assert_eq!(
            browser_engine.commands.try_recv()?,
            Command::OpenMediaLink(link.clone())
        );
        assert_eq!(window.get_address(), link.path.as_str());
        let second_load = owner.running.borrow().as_ref().unwrap().clone();
        window.invoke_stop();
        assert_eq!(first_load.status(), StreamStatus::Loading);
        assert_eq!(second_load.status(), StreamStatus::Cancelled);

        link.path = "https://example.test/retry".into();
        window.set_address(link.path.as_str().into());
        window.invoke_address_edited();
        window.invoke_submit();
        assert_eq!(
            browser_engine.commands.try_recv()?,
            Command::OpenMediaLink(link)
        );
        window.invoke_cancel();
        assert!(!window.window().is_visible());
        Ok(())
    }
}
