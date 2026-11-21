//! Project typed plugin messages onto the player or the native presenters.
use super::super::plugin_menus::{remove_plugin_menu_rows, replace_plugin_menu_rows};
use super::{
    apply_plugin_window_action, plugin_sidebar_tab_added, plugin_sidebar_tab_removed,
    select_plugin_sidebar_tab,
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
                self.send(app, command, "Plugin player command failed");
            }
            PluginHostMessage::ObserveMpvProperty(property) => {
                self.send(
                    app,
                    Command::ObserveMpvProperty(property),
                    "Could not observe mpv plugin property",
                );
            }
            PluginHostMessage::SetUiVisibility(disabled) => app.set_plugin_ui_disabled(disabled),
            PluginHostMessage::WindowAction(action) => {
                apply_plugin_window_action(app, self.window_host.as_ref(), action);
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
                remove_plugin_menu_rows(app, &identifier)
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
            app.set_status(format!("{description}: {error}").into());
        }
    }

    fn show_osd(&self, app: &PlayerWindow, text: String) {
        let generation = self.osd_generation.get().wrapping_add(1);
        self.osd_generation.set(generation);
        app.set_plugin_osd_message(text.into());
        if let Some(timer) = self.osd_timer.as_ref().and_then(Weak::upgrade) {
            let weak = app.as_weak();
            let current = self.osd_generation.clone();
            timer.start(TimerMode::SingleShot, Duration::from_secs(3), move || {
                if current.get() == generation
                    && let Some(app) = weak.upgrade()
                {
                    app.set_plugin_osd_message("".into());
                }
            });
        }
    }

    fn present_sidebar(&self, app: &PlayerWindow, message: PluginHostMessage) {
        if let PluginHostMessage::Sidebar {
            plugin_identifier,
            effect,
            ..
        } = &message
        {
            match effect {
                PluginSidebarEffect::Show => select_plugin_sidebar_tab(app, plugin_identifier),
                PluginSidebarEffect::Hide => {
                    if app.get_sidebar() == 7 {
                        app.set_sidebar(0);
                    }
                    if app.get_leading_sidebar() == 7 {
                        app.set_leading_sidebar(0);
                    }
                }
                _ => {}
            }
        }
        let result = app.window().with_winit_window(|parent| {
            self.sidebar
                .borrow_mut()
                .apply_host_message(parent, &self.sidebar_host, message)
        });
        if let Some(Err(error)) = result {
            app.set_status(error.into());
        }
    }

    pub(super) fn update_presenters(&self, app: &PlayerWindow) {
        self.overlay.update(app);
        let selected = if app.get_sidebar() == 7 || app.get_leading_sidebar() == 7 {
            let identifier = app.get_plugin_sidebar_selected_identifier();
            (!identifier.is_empty()).then(|| identifier.to_string())
        } else {
            None
        };
        self.sidebar.borrow_mut().update(
            app.get_plugin_sidebar_view_x(),
            app.get_plugin_sidebar_view_y(),
            app.get_plugin_sidebar_view_width(),
            app.get_plugin_sidebar_view_height(),
            selected,
        );
    }
}
