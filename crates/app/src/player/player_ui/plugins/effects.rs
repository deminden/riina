//! Project typed plugin messages onto the player or the native presenters.
use super::super::plugin_menus::{remove_plugin_menu_rows, replace_plugin_menu_rows};
use super::{
    plugin_sidebar_tab_added, plugin_sidebar_tab_removed, select_plugin_sidebar_tab, window,
};
use crate::{
    NativeVideoHost, PlayerWindow, platform,
    plugin_host::{PluginHostHandle, PluginHostMessage},
};
use riina_core::{Command, plugin_sidebar::PluginSidebarEffect, sync::lock};
use riina_mpv::CommandSender;
use slint::{ComponentHandle, Timer, TimerMode, winit_030::WinitWindowAccessor};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
    sync::{Arc, Mutex},
    time::Duration,
};

pub(super) struct PluginEffects {
    pub(super) commands: Arc<Mutex<Option<CommandSender>>>,
    pub(super) window_host: Option<slint::Weak<NativeVideoHost>>,
    pub(super) osd_timer: Option<Weak<Timer>>,
    pub(super) osd_generation: Rc<Cell<u64>>,
    pub(super) overlay: platform::PluginOverlayDelivery,
    pub(super) sidebar: Rc<RefCell<platform::PluginSidebarPresenter>>,
    pub(super) sidebar_host: PluginHostHandle,
}

impl PluginEffects {
    pub(super) fn apply(&self, app: &PlayerWindow, message: PluginHostMessage) {
        match message {
            PluginHostMessage::Log(text) | PluginHostMessage::Status(text) => {
                app.set_status(text.into());
            }
            PluginHostMessage::PlayerCommand(command) => {
                if crate::player::surface::is_parked(app)
                    && (command.replaces_media() || matches!(command, Command::AppendFiles(_)))
                {
                    // This window no longer accepts playback requests from its old realm.
                    return;
                }
                let command = if matches!(command, Command::Stop(_)) {
                    crate::media_open::stop_command(app)
                } else if command.replaces_media() {
                    crate::media_open::invalidate(app);
                    command
                } else {
                    if matches!(command, Command::AppendFiles(_))
                        && let Some(observer) = super::super::ObserverId::for_window(app)
                    {
                        observer.supersede_media();
                    }
                    command
                };
                self.send(app, command, "Plugin player command failed");
            }
            PluginHostMessage::ObserveMpvProperty(property) => {
                self.send(
                    app,
                    Command::ObserveMpvProperty(property),
                    "Could not observe mpv plugin property",
                );
            }
            PluginHostMessage::SetUiVisibility(disabled) => app
                .global::<riina_ui::PlayerControlsState>()
                .set_plugin_ui_disabled(disabled),
            PluginHostMessage::WindowAction(action) => {
                let host = self.window_host.clone();
                // Native frame and minimize requests can dispatch window events.
                // Leave the presenter timer before applying any window action.
                let _ = crate::background::to_ui(app.as_weak(), move |app| {
                    window::apply(&app, host.as_ref(), action);
                });
            }
            PluginHostMessage::ShowOsd(text) => self.show_osd(app, text),
            PluginHostMessage::ShowAlert(title, text) => {
                rfd::MessageDialog::new()
                    .set_title(title)
                    .set_description(text)
                    .set_buttons(rfd::MessageButtons::Ok)
                    .show();
            }
            PluginHostMessage::SidebarTabAdded {
                plugin_identifier,
                label,
            } => {
                plugin_sidebar_tab_added(app, &plugin_identifier, &label);
            }
            PluginHostMessage::PluginMenuUpdated {
                plugin_identifier,
                plugin_name,
                global,
                items,
            } => {
                replace_plugin_menu_rows(app, &plugin_identifier, &plugin_name, global, &items);
            }
            PluginHostMessage::RemovePluginMenu(identifier) => {
                remove_plugin_menu_rows(app, &identifier);
                crate::online_subtitles_preferences::replace_plugin_providers(
                    app,
                    &identifier,
                    Vec::new(),
                );
            }
            PluginHostMessage::SubtitleProviders(identifier, providers) => {
                crate::online_subtitles_preferences::replace_plugin_providers(
                    app,
                    &identifier,
                    providers,
                );
            }
            message @ PluginHostMessage::Overlay { .. } => {
                if let Err(error) = self.overlay.apply(message) {
                    app.set_status(error.into());
                }
            }
            PluginHostMessage::RemoveOverlay(identifier) => self.overlay.remove(&identifier),
            message @ PluginHostMessage::Sidebar { .. } => self.present_sidebar(app, message),
            PluginHostMessage::RemoveSidebar(identifier) => {
                self.sidebar.borrow_mut().remove(&identifier);
                plugin_sidebar_tab_removed(app, &identifier);
            }
        }
    }

    fn send(&self, app: &PlayerWindow, command: Command, description: &str) {
        let result = lock(&self.commands)
            .as_ref()
            .map(|sender| sender.send(command));
        if let Some(Err(error)) = result {
            crate::diagnostics::report(app, format!("{description}: {error}"));
        }
    }

    fn show_osd(&self, app: &PlayerWindow, text: String) {
        if app
            .global::<riina_ui::PlayerMusicModeState>()
            .get_music_mode()
        {
            return;
        }
        let generation = self.osd_generation.get().wrapping_add(1);
        self.osd_generation.set(generation);
        app.global::<riina_ui::PlayerOsdState>()
            .set_plugin_osd_message(text.into());
        if let Some(timer) = self.osd_timer.as_ref().and_then(Weak::upgrade) {
            let weak = app.as_weak();
            let current = self.osd_generation.clone();
            timer.start(TimerMode::SingleShot, Duration::from_secs(3), move || {
                if current.get() == generation
                    && let Some(app) = weak.upgrade()
                {
                    app.global::<riina_ui::PlayerOsdState>()
                        .set_plugin_osd_message("".into());
                }
            });
        }
    }

    fn present_sidebar(&self, app: &PlayerWindow, message: PluginHostMessage) {
        if let PluginHostMessage::Sidebar { route, effect, .. } = &message {
            match effect {
                PluginSidebarEffect::Show => select_plugin_sidebar_tab(app, &route.identifier),
                PluginSidebarEffect::Hide => {
                    if app.global::<riina_ui::PlayerSidebarState>().get_sidebar() == 7 {
                        app.global::<riina_ui::PlayerSidebarState>().set_sidebar(0);
                    }
                    if app
                        .global::<riina_ui::PlayerSidebarState>()
                        .get_leading_sidebar()
                        == 7
                    {
                        app.global::<riina_ui::PlayerSidebarState>()
                            .set_leading_sidebar(0);
                    }
                }
                _ => {}
            }
        }
        let result = app.window().with_winit_window(|parent| {
            self.sidebar.borrow_mut().apply_host_message(
                parent,
                &self.sidebar_host,
                message,
                platform::PluginSidebarPresenter::viewport(app),
            )
        });
        if let Some(Err(error)) = result {
            app.set_status(error.into());
        }
    }

    pub(super) fn update_presenters(&self, app: &PlayerWindow) {
        let selected = if app.global::<riina_ui::PlayerSidebarState>().get_sidebar() == 7
            || app
                .global::<riina_ui::PlayerSidebarState>()
                .get_leading_sidebar()
                == 7
        {
            let identifier = app
                .global::<riina_ui::PlayerSidebarState>()
                .get_plugin_sidebar_selected_identifier();
            (!identifier.is_empty()).then(|| identifier.to_string())
        } else {
            None
        };
        self.sidebar
            .borrow_mut()
            .update(platform::PluginSidebarPresenter::viewport(app), selected);
    }
}
