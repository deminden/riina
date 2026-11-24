//! External opens share the same progress and retry window as menu requests.
use super::request::StreamRequest;
use super::{ApplicationOpenUrl, Destination, PendingStream, Route, StreamMode, credentials};
use crate::PlayerWindow;
use riina_core::MediaLink;
use riina_mpv::CommandSender;
use slint::ComponentHandle;
use std::rc::Rc;

impl ApplicationOpenUrl {
    pub(crate) fn open_initial_stream(
        self: &Rc<Self>,
        app: &PlayerWindow,
        sender: &CommandSender,
        address: &str,
        start: Option<f64>,
    ) -> Result<(), String> {
        self.open_external(
            app,
            sender,
            address,
            StreamMode::Startup(start),
            StreamRequest::New,
        )
    }

    pub(crate) fn open_browser_stream(
        self: &Rc<Self>,
        app: &PlayerWindow,
        sender: &CommandSender,
        link: MediaLink,
    ) -> Result<(), String> {
        let address = link.path.clone();
        self.open_external(
            app,
            sender,
            &address,
            StreamMode::Browser(link),
            StreamRequest::New,
        )
    }

    pub(crate) fn open_initial_playlist(
        self: &Rc<Self>,
        app: &PlayerWindow,
        sender: &CommandSender,
        address: &str,
        following: &[String],
        explicit: bool,
        generation: i32,
    ) -> Result<(), String> {
        let mode = StreamMode::Playlist {
            following: following.into(),
            explicit,
        };
        self.open_external(
            app,
            sender,
            address,
            mode,
            StreamRequest::Prepared(generation),
        )
    }

    fn open_external(
        self: &Rc<Self>,
        app: &PlayerWindow,
        sender: &CommandSender,
        address: &str,
        mode: StreamMode,
        request: StreamRequest,
    ) -> Result<(), String> {
        let source = credentials::source(address, &credentials::Account::default());
        let visible = credentials::visible_address(&source.address);
        let (pending, running) = PendingStream::new(source, mode.clone());
        if !pending.dispatch(app, sender, request)? {
            return Ok(());
        }
        let presented = (|| -> Result<(), slint::PlatformError> {
            let window = self.window_for(app, None)?;
            self.reset_draft(&window);
            let sender = sender.clone();
            self.destination.replace(Some(Destination {
                player: app.as_weak(),
                welcome: None,
                current: Rc::new(move |app, stream| stream.start(app, &sender)),
                route: Route::Current,
                mode,
            }));
            window.set_address(visible.into());
            self.credential_timer.stop();
            // Only the foreground request owns Stop Loading. Earlier loads on
            // other players continue when a newer external request is shown.
            self.show_progress(&window, running);
            Self::reveal(&window, app)
        })();
        if let Err(error) = presented {
            crate::diagnostics::report(
                app,
                format!("Could not show stream loading progress: {error}"),
            );
        }
        Ok(())
    }
}
