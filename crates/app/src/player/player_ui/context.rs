//! Shared UI-thread handles used by player callbacks.
use super::show_osd_message;
use crate::{
    PlayerWindow,
    plugin_host::PluginHostHandle,
    preferences::model::PreferenceModel,
    video_osd::{ActionFeedback, action_message},
};
use riina_core::Command;
use riina_mpv::CommandSender;
use slint::ComponentHandle;
use std::{
    path::{Path, PathBuf},
    rc::Rc,
};

#[derive(Clone)]
pub(crate) struct PlayerContext(Rc<Handles>);

struct Handles {
    app: slint::Weak<PlayerWindow>,
    commands: CommandSender,
    playback: crate::PlaybackStore,
    preferences: PreferenceModel,
    plugin_host: Option<PluginHostHandle>,
    preferences_path: PathBuf,
}

impl PlayerContext {
    pub(super) fn new(
        app: &PlayerWindow,
        commands: CommandSender,
        playback: crate::PlaybackStore,
        preferences: PreferenceModel,
        plugin_host: Option<PluginHostHandle>,
        preferences_path: PathBuf,
    ) -> Self {
        Self(Rc::new(Handles {
            app: app.as_weak(),
            commands,
            playback,
            preferences,
            plugin_host,
            preferences_path,
        }))
    }

    pub(crate) fn app(&self) -> Option<PlayerWindow> {
        self.0.app.upgrade()
    }

    pub(crate) fn commands(&self) -> &CommandSender {
        &self.0.commands
    }

    pub(crate) fn playback(&self) -> &crate::PlaybackStore {
        &self.0.playback
    }

    pub(crate) fn preferences(&self) -> &PreferenceModel {
        &self.0.preferences
    }

    pub(super) fn plugin_host(&self) -> Option<&PluginHostHandle> {
        self.0.plugin_host.as_ref()
    }

    pub(crate) fn preferences_path(&self) -> &Path {
        &self.0.preferences_path
    }

    pub(crate) fn activity(&self) {
        if let Some(app) = self.app() {
            app.global::<riina_ui::PlayerControlActions>()
                .invoke_activity();
        }
    }

    pub(crate) fn report(&self, error: impl Into<crate::diagnostics::UserMessage>) {
        if let Some(app) = self.app() {
            crate::diagnostics::report(&app, error);
        }
    }

    pub(super) fn send(&self, command: Command) {
        if let Err(error) = self.0.commands.send(command) {
            self.report(error);
        }
    }

    pub(super) fn send_with_osd(
        &self,
        command: Command,
        feedback: ActionFeedback,
        persistent: bool,
    ) {
        match self.0.commands.send(command) {
            Err(error) => self.report(error),
            Ok(()) => {
                if let Some(app) = self.app() {
                    let message = action_message(feedback);
                    show_osd_message(&app, message.text, persistent);
                }
            }
        }
    }
}

macro_rules! bind {
    ($context:expr, $app:expr, $callback:ident, |$ctx:ident $(, $argument:ident)*| $body:expr) => {{
        let $ctx = $context.clone();
        ($app).$callback(move |$($argument),*| $body);
    }};
    ($context:expr, $app:expr, $callback:ident, $command:expr) => {
        bind!($context, $app, $callback, |ctx| {
            ctx.activity();
            ctx.send($command);
        })
    };
}

pub(super) use bind;
