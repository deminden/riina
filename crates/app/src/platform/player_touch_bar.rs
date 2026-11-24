//! Bridge the player lifecycle and timeline samples to the native Touch Bar.
use crate::{PlayerWindow, player_ui::PlayerContext, thumbnail_preview::Controller};
use riina_core::Playback;
use std::{cell::RefCell, rc::Rc};

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

#[cfg(target_os = "macos")]
type Redraw = Box<dyn Fn()>;

#[derive(Clone)]
pub(crate) struct Samples {
    #[cfg(target_os = "macos")]
    pub(super) controller: Rc<RefCell<Controller>>,
    #[cfg(target_os = "macos")]
    redraw: Rc<RefCell<Option<Redraw>>>,
}

impl Samples {
    pub(crate) fn new(controller: &Rc<RefCell<Controller>>) -> Self {
        #[cfg(not(target_os = "macos"))]
        let _ = controller;
        Self {
            #[cfg(target_os = "macos")]
            controller: controller.clone(),
            #[cfg(target_os = "macos")]
            redraw: Rc::default(),
        }
    }

    #[cfg(target_os = "macos")]
    pub(super) fn on_redraw(&self, redraw: impl Fn() + 'static) {
        self.redraw.replace(Some(Box::new(redraw)));
    }

    pub(crate) fn changed(&self) {
        #[cfg(target_os = "macos")]
        if let Some(redraw) = self.redraw.borrow().as_ref() {
            redraw();
        }
    }
}

pub(crate) fn bind_escape(app: &PlayerWindow, exits_fullscreen: impl FnOnce() -> bool) {
    #[cfg(target_os = "macos")]
    app.set_escape_exits_fullscreen(exits_fullscreen());
    #[cfg(not(target_os = "macos"))]
    let _ = (app, exits_fullscreen);
}
