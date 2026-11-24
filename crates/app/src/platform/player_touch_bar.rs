//! Bridge the player lifecycle and timeline samples to the native Touch Bar.
use super::player_thumbnails::Samples;
use crate::{PlayerWindow, player_ui::PlayerContext};
use riina_core::Playback;
#[cfg(target_os = "macos")]
use std::rc::Rc;

#[cfg(target_os = "macos")]
use super::macos::touch_bar::{ApplicationTouchBars, Session};

#[derive(Clone, Default)]
pub(crate) struct Application {
    #[cfg(target_os = "macos")]
    inner: Rc<ApplicationTouchBars>,
}

impl Application {
    pub(crate) fn install(
        &self,
        app: &PlayerWindow,
        context: &PlayerContext,
        samples: &Samples,
    ) -> Registration {
        #[cfg(target_os = "macos")]
        {
            Registration(Some(self.inner.install(app, context.clone(), samples)))
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (self, app, context, samples);
            Registration::default()
        }
    }
}

#[derive(Clone, Default)]
pub(crate) struct Registration(#[cfg(target_os = "macos")] Option<Rc<Session>>);

impl Registration {
    pub(crate) fn observe(&self, app: &PlayerWindow, playback: &Playback) {
        #[cfg(target_os = "macos")]
        if let Some(session) = &self.0 {
            session.observe(app, playback);
        }
        #[cfg(not(target_os = "macos"))]
        let _ = (app, playback);
    }

    pub(crate) fn retire(self) {
        #[cfg(target_os = "macos")]
        drop(self.0);
    }
}

pub(crate) fn bind_escape(app: &PlayerWindow, exits_fullscreen: impl FnOnce() -> bool) {
    #[cfg(target_os = "macos")]
    app.set_escape_exits_fullscreen(exits_fullscreen());
    #[cfg(not(target_os = "macos"))]
    let _ = (app, exits_fullscreen);
}
