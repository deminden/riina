//! Plugin callbacks, presenters and timers are installed as one UI subsystem.
use super::plugin_menus::{remove_plugin_menu_rows, replace_plugin_menu_rows};
use crate::{PlayerWindow, platform};
use riina_core::Command;
use slint::{ComponentHandle, Model, winit_030::WinitWindowAccessor};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

pub(super) struct InstalledPluginUi {
    #[cfg(target_os = "windows")]
    pub(super) plugin_overlay_presenter: Rc<RefCell<platform::PluginOverlayPresenter>>,
    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    pub(super) plugin_sidebar_presenter: Rc<RefCell<platform::PluginSidebarPresenter>>,
    #[cfg(target_os = "macos")]
    pub(super) mac_plugin_menu_observer: Option<platform::MacPluginMenuObserver>,
    pub(super) plugin_host_timer: Option<Rc<slint::Timer>>,
    pub(super) plugin_osd_timer: Option<Rc<slint::Timer>>,
    pub(super) plugin_command_sender: Arc<std::sync::Mutex<Option<riina_mpv::CommandSender>>>,
}

pub(super) fn install(
    app: &PlayerWindow,
    native: Option<&platform::NativePresentation>,
    plugin_host_handle: Option<crate::plugin_host::PluginHostHandle>,
) -> InstalledPluginUi {
    #[cfg(target_os = "macos")]
    let mac_plugin_menu_observer = plugin_host_handle.as_ref().and_then(|host| {
        match platform::MacPluginMenuObserver::new(app, host.clone()) {
            Ok(observer) => Some(observer),
            Err(error) => {
                app.set_status(format!("Could not observe the Plugin menu: {error}").into());
                None
            }
        }
    });
    let menu_action_host = plugin_host_handle.clone();
    let menu_action_app = app.as_weak();
    app.on_activate_plugin_menu_item(move |token| {
        let Some((identifier, scope_and_id)) = token.as_str().split_once('\u{1f}') else {
            return;
        };
        let Some((scope, item_id)) = scope_and_id.split_once('\u{1f}') else {
            return;
        };
        let global = match scope {
            "global" => true,
            "player" => false,
            _ => return,
        };
        let Some(host) = &menu_action_host else {
            if let Some(app) = menu_action_app.upgrade() {
                app.set_status("The plugin runtime is unavailable.".into());
            }
            return;
        };
        if let Err(error) =
            host.activate_plugin_menu_item(identifier.into(), global, item_id.into())
            && let Some(app) = menu_action_app.upgrade()
        {
            app.set_status(format!("Could not run plugin menu action: {error}").into());
        }
    });
    let menu_update_host = plugin_host_handle.clone();
    let menu_update_app = app.as_weak();
    app.on_plugin_menu_will_open(move || {
        if let Some(host) = &menu_update_host
            && let Err(error) = host.notify_menu_update()
            && let Some(app) = menu_update_app.upgrade()
        {
            app.set_status(
                format!("Could not notify plugins that their menu is opening: {error}").into(),
            );
        }
    });
    let reload_plugins_host = plugin_host_handle.clone();
    let reload_plugins_app = app.as_weak();
    app.on_reload_plugins(move || {
        let Some(host) = &reload_plugins_host else {
            if let Some(app) = reload_plugins_app.upgrade() {
                app.set_status("The plugin runtime is unavailable.".into());
            }
            return;
        };
        if let Err(error) = host.reload_all()
            && let Some(app) = reload_plugins_app.upgrade()
        {
            app.set_status(format!("Could not reload plugins: {error}").into());
        }
    });
    let close_event_sent = Rc::new(Cell::new(false));
    let close_event_sent_once = close_event_sent.clone();
    let close_event_host = plugin_host_handle.clone();
    let weak = app.as_weak();
    app.on_window_will_close(move || {
        if close_event_sent_once.replace(true) {
            return;
        }
        if let Some(host) = &close_event_host
            && let Err(error) = host.notify_window_will_close()
            && let Some(app) = weak.upgrade()
        {
            app.set_status(format!("Could not notify plugins before closing: {error}").into());
        }
    });
    let plugin_window_host = native.map(platform::NativePresentation::host);
    let plugin_command_sender = Arc::new(std::sync::Mutex::new(None::<riina_mpv::CommandSender>));
    #[cfg(target_os = "windows")]
    let plugin_overlay_presenter =
        Rc::new(RefCell::new(platform::PluginOverlayPresenter::default()));
    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    let plugin_sidebar_presenter =
        Rc::new(RefCell::new(platform::PluginSidebarPresenter::default()));
    #[cfg(target_os = "windows")]
    let plugin_overlay_parent = native.map(platform::NativePresentation::webview_parent_window);
    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    {
        let weak = app.as_weak();
        let presenter = plugin_sidebar_presenter.clone();
        app.on_select_plugin_sidebar_tab(move |identifier| {
            let Some(app) = weak.upgrade() else { return };
            let identifier = identifier.to_string();
            select_plugin_sidebar_tab(&app, &identifier);
            presenter.borrow_mut().select(Some(identifier));
        });
    }
    let plugin_osd_timer = plugin_host_handle
        .as_ref()
        .map(|_| Rc::new(slint::Timer::default()));
    let plugin_osd_timer_weak = plugin_osd_timer.as_ref().map(Rc::downgrade);
    let plugin_osd_generation = Rc::new(Cell::new(0_u64));
    let plugin_host_timer = plugin_host_handle.as_ref().map(|handle| {
        let timer = Rc::new(slint::Timer::default());
        let messages = handle.clone();
        #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
        let sidebar_host = handle.clone();
        let window_loaded_host = handle.clone();
        let window_loaded_sent = Rc::new(Cell::new(false));
        let window_messages = handle.clone();
        let window_host = plugin_window_host.clone();
        let last_window_properties =
            Rc::new(RefCell::new(None::<BTreeMap<String, serde_json::Value>>));
        let last_window_update = Rc::new(Cell::new(Instant::now()));
        let player_commands = plugin_command_sender.clone();
        let weak = app.as_weak();
        let osd_timer = plugin_osd_timer_weak.clone();
        let osd_generation = plugin_osd_generation.clone();
        #[cfg(target_os = "windows")]
        let overlay = plugin_overlay_presenter.clone();
        #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
        let sidebar_presenter = plugin_sidebar_presenter.clone();
        #[cfg(target_os = "windows")]
        let overlay_host = handle.clone();
        #[cfg(target_os = "windows")]
        let overlay_parent = plugin_overlay_parent.clone();
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(25),
            move || {
                let Some(app) = weak.upgrade() else { return };
                #[cfg(target_os = "linux")]
                platform::pump_linux_webview_events();
                if !window_loaded_sent.get() && window_loaded_host.notify_window_loaded().is_ok() {
                    window_loaded_sent.set(true);
                }
                if last_window_update.get().elapsed() >= Duration::from_millis(125) {
                    last_window_update.set(Instant::now());
                    let properties = plugin_window_properties(&app, window_host.as_ref());
                    if last_window_properties.borrow().as_ref() != Some(&properties)
                        && window_messages
                            .update_window_properties(properties.clone())
                            .is_ok()
                    {
                        *last_window_properties.borrow_mut() = Some(properties);
                    }
                }
                let Ok(messages) = messages.drain_messages() else {
                    return;
                };
                for message in messages {
                    match message {
                        crate::plugin_host::PluginHostMessage::Log(text)
                        | crate::plugin_host::PluginHostMessage::Status(text) => {
                            app.set_status(text.into());
                        }
                        crate::plugin_host::PluginHostMessage::PlayerCommand(command) => {
                            let result = player_commands.lock().ok().and_then(|sender| {
                                sender.as_ref().map(|sender| sender.send(command))
                            });
                            if let Some(Err(error)) = result {
                                app.set_status(
                                    format!("Plugin player command failed: {error}").into(),
                                );
                            }
                        }
                        crate::plugin_host::PluginHostMessage::ObserveMpvProperty(property) => {
                            let result = player_commands.lock().ok().and_then(|sender| {
                                sender.as_ref().map(|sender| {
                                    sender.send(Command::ObserveMpvProperty(property))
                                })
                            });
                            if let Some(Err(error)) = result {
                                app.set_status(
                                    format!("Could not observe mpv plugin property: {error}")
                                        .into(),
                                );
                            }
                        }
                        crate::plugin_host::PluginHostMessage::SetUiVisibility(disabled) => {
                            app.set_plugin_ui_disabled(disabled);
                        }
                        crate::plugin_host::PluginHostMessage::WindowAction(action) => {
                            apply_plugin_window_action(&app, window_host.as_ref(), action);
                        }
                        crate::plugin_host::PluginHostMessage::ShowOsd(text) => {
                            let generation = osd_generation.get().wrapping_add(1);
                            osd_generation.set(generation);
                            app.set_plugin_osd_message(text.into());
                            if let Some(timer) =
                                osd_timer.as_ref().and_then(|timer| timer.upgrade())
                            {
                                let weak = app.as_weak();
                                let current = osd_generation.clone();
                                timer.start(
                                    slint::TimerMode::SingleShot,
                                    Duration::from_secs(3),
                                    move || {
                                        if current.get() == generation
                                            && let Some(app) = weak.upgrade()
                                        {
                                            app.set_plugin_osd_message("".into());
                                        }
                                    },
                                );
                            }
                        }
                        crate::plugin_host::PluginHostMessage::ShowAlert(title, text) => {
                            rfd::MessageDialog::new()
                                .set_title(title)
                                .set_description(text)
                                .set_buttons(rfd::MessageButtons::Ok)
                                .show();
                        }
                        crate::plugin_host::PluginHostMessage::SidebarTabAdded {
                            plugin_identifier,
                            label,
                        } => {
                            plugin_sidebar_tab_added(&app, &plugin_identifier, &label);
                        }
                        crate::plugin_host::PluginHostMessage::PluginMenuUpdated {
                            plugin_identifier,
                            plugin_name,
                            global,
                            items,
                        } => replace_plugin_menu_rows(
                            &app,
                            &plugin_identifier,
                            &plugin_name,
                            global,
                            &items,
                        ),
                        crate::plugin_host::PluginHostMessage::RemovePluginMenu(identifier) => {
                            remove_plugin_menu_rows(&app, &identifier);
                        }
                        #[cfg(target_os = "windows")]
                        message @ crate::plugin_host::PluginHostMessage::Overlay { .. } => {
                            let result = if let Some(parent) = overlay_parent.as_ref() {
                                overlay.borrow_mut().apply_host_message(
                                    parent,
                                    &overlay_host,
                                    message,
                                )
                            } else {
                                Err("Plugin video overlays require native video presentation"
                                    .into())
                            };
                            if let Err(error) = result {
                                app.set_status(error.into());
                            }
                        }
                        #[cfg(target_os = "windows")]
                        crate::plugin_host::PluginHostMessage::RemoveOverlay(identifier) => {
                            overlay.borrow_mut().remove(&identifier);
                        }
                        #[cfg(any(
                            target_os = "windows",
                            target_os = "macos",
                            target_os = "linux"
                        ))]
                        message @ crate::plugin_host::PluginHostMessage::Sidebar { .. } => {
                            if let crate::plugin_host::PluginHostMessage::Sidebar {
                                plugin_identifier,
                                effect,
                                ..
                            } = &message
                            {
                                match effect {
                                    riina_core::plugin_sidebar::PluginSidebarEffect::Show => {
                                        select_plugin_sidebar_tab(&app, plugin_identifier);
                                    }
                                    riina_core::plugin_sidebar::PluginSidebarEffect::Hide => {
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
                                sidebar_presenter.borrow_mut().apply_host_message(
                                    parent,
                                    &sidebar_host,
                                    message,
                                )
                            });
                            if let Some(Err(error)) = result {
                                app.set_status(error.into());
                            }
                        }
                        #[cfg(not(target_os = "windows"))]
                        crate::plugin_host::PluginHostMessage::Overlay { .. } => {
                            app.set_status(
                                "Plugin video overlays are unavailable on this platform".into(),
                            );
                        }
                        #[cfg(not(target_os = "windows"))]
                        crate::plugin_host::PluginHostMessage::RemoveOverlay(_) => {}
                        #[cfg(any(
                            target_os = "windows",
                            target_os = "macos",
                            target_os = "linux"
                        ))]
                        crate::plugin_host::PluginHostMessage::RemoveSidebar(identifier) => {
                            sidebar_presenter.borrow_mut().remove(&identifier);
                            plugin_sidebar_tab_removed(&app, &identifier);
                        }
                    }
                }
                #[cfg(target_os = "windows")]
                overlay.borrow_mut().update(
                    app.get_window_width(),
                    app.get_window_height(),
                    f64::from(app.window().scale_factor()),
                );
                #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
                {
                    let selected = if app.get_sidebar() == 7 || app.get_leading_sidebar() == 7 {
                        let identifier = app.get_plugin_sidebar_selected_identifier();
                        (!identifier.is_empty()).then(|| identifier.to_string())
                    } else {
                        None
                    };
                    sidebar_presenter.borrow_mut().update(
                        app.get_plugin_sidebar_view_x(),
                        app.get_plugin_sidebar_view_y(),
                        app.get_plugin_sidebar_view_width(),
                        app.get_plugin_sidebar_view_height(),
                        selected,
                    );
                }
            },
        );
        timer
    });
    InstalledPluginUi {
        #[cfg(target_os = "windows")]
        plugin_overlay_presenter,
        #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
        plugin_sidebar_presenter,
        #[cfg(target_os = "macos")]
        mac_plugin_menu_observer,
        plugin_host_timer,
        plugin_osd_timer,
        plugin_command_sender,
    }
}

fn plugin_sidebar_tab_added(app: &PlayerWindow, identifier: &str, label: &str) {
    let mut identifiers = (0..app.get_plugin_sidebar_identifiers().row_count())
        .filter_map(|row| app.get_plugin_sidebar_identifiers().row_data(row))
        .map(|value| value.to_string())
        .collect::<Vec<_>>();
    let mut labels = (0..app.get_plugin_sidebar_labels().row_count())
        .filter_map(|row| app.get_plugin_sidebar_labels().row_data(row))
        .map(|value| value.to_string())
        .collect::<Vec<_>>();
    if let Some(index) = identifiers.iter().position(|current| current == identifier) {
        labels[index] = label.to_owned();
    } else {
        identifiers.push(identifier.to_owned());
        labels.push(label.to_owned());
    }
    app.set_plugin_sidebar_identifiers(slint::ModelRc::new(slint::VecModel::from(
        identifiers
            .iter()
            .map(|value| slint::SharedString::from(value.as_str()))
            .collect::<Vec<_>>(),
    )));
    app.set_plugin_sidebar_labels(slint::ModelRc::new(slint::VecModel::from(
        labels
            .iter()
            .map(|value| slint::SharedString::from(value.as_str()))
            .collect::<Vec<_>>(),
    )));
    let selected = app.get_plugin_sidebar_selected_identifier().to_string();
    app.set_plugin_sidebar_selected_index(
        identifiers
            .iter()
            .position(|current| current == &selected)
            .map_or(-1, |index| index as i32),
    );
}

fn plugin_sidebar_tab_removed(app: &PlayerWindow, identifier: &str) {
    let mut identifiers = (0..app.get_plugin_sidebar_identifiers().row_count())
        .filter_map(|row| app.get_plugin_sidebar_identifiers().row_data(row))
        .map(|value| value.to_string())
        .collect::<Vec<_>>();
    let mut labels = (0..app.get_plugin_sidebar_labels().row_count())
        .filter_map(|row| app.get_plugin_sidebar_labels().row_data(row))
        .map(|value| value.to_string())
        .collect::<Vec<_>>();
    if let Some(index) = identifiers.iter().position(|current| current == identifier) {
        identifiers.remove(index);
        labels.remove(index);
    }
    if app.get_plugin_sidebar_selected_identifier().as_str() == identifier {
        app.set_plugin_sidebar_selected_identifier("".into());
    }
    app.set_plugin_sidebar_identifiers(slint::ModelRc::new(slint::VecModel::from(
        identifiers
            .iter()
            .map(|value| slint::SharedString::from(value.as_str()))
            .collect::<Vec<_>>(),
    )));
    app.set_plugin_sidebar_labels(slint::ModelRc::new(slint::VecModel::from(
        labels
            .iter()
            .map(|value| slint::SharedString::from(value.as_str()))
            .collect::<Vec<_>>(),
    )));
    let selected = app.get_plugin_sidebar_selected_identifier().to_string();
    app.set_plugin_sidebar_selected_index(
        identifiers
            .iter()
            .position(|current| current == &selected)
            .map_or(-1, |index| index as i32),
    );
}

fn select_plugin_sidebar_tab(app: &PlayerWindow, identifier: &str) {
    app.set_plugin_sidebar_selected_identifier(identifier.into());
    if app.get_leading_sidebar() != 7 && app.get_sidebar() != 7 {
        // Use the configured IINA placement while leaving an already-open
        // Plugins panel on its current side when a different tab is selected.
        crate::sidebar_preferences::show_plugins_panel(app);
    }
    let index = (0..app.get_plugin_sidebar_identifiers().row_count())
        .find(|row| {
            app.get_plugin_sidebar_identifiers()
                .row_data(*row)
                .is_some_and(|value| value.as_str() == identifier)
        })
        .map_or(-1, |row| row as i32);
    app.set_plugin_sidebar_selected_index(index);
}

fn plugin_sidebar_name(index: i32) -> Option<&'static str> {
    match index {
        1 => Some("audio"),
        2 => Some("subtitles"),
        3 => Some("chapter"),
        4 => Some("playlist"),
        5 => Some("layout"),
        6 => Some("video"),
        7 => Some("plugins"),
        _ => None,
    }
}

fn plugin_sidebar_index(name: &str) -> Option<i32> {
    match name {
        "audio" => Some(1),
        "subtitles" => Some(2),
        "chapter" => Some(3),
        "playlist" => Some(4),
        "layout" => Some(5),
        "video" => Some(6),
        "plugins" => Some(7),
        _ => None,
    }
}

fn plugin_window_properties(
    app: &PlayerWindow,
    native_host: Option<&slint::Weak<crate::NativeVideoHost>>,
) -> BTreeMap<String, serde_json::Value> {
    let mut properties = BTreeMap::from([
        ("loaded".into(), serde_json::Value::Bool(false)),
        ("music-mode".into(), serde_json::json!(app.get_music_mode())),
        ("fullscreen".into(), serde_json::json!(app.get_fullscreen())),
        (
            "pip".into(),
            serde_json::json!(app.get_picture_in_picture_active()),
        ),
        (
            "ontop".into(),
            serde_json::json!(app.get_always_float_on_top()),
        ),
        (
            "sidebar".into(),
            plugin_sidebar_name(app.get_sidebar())
                .or_else(|| plugin_sidebar_name(app.get_leading_sidebar()))
                .map_or(serde_json::Value::Null, |name| serde_json::json!(name)),
        ),
        ("visible".into(), serde_json::Value::Null),
        ("main".into(), serde_json::Value::Null),
        ("miniaturized".into(), serde_json::Value::Null),
        ("frame".into(), serde_json::Value::Null),
        (
            "window-size-adjusted-event".into(),
            serde_json::json!(app.get_window_size_adjusted_event()),
        ),
        ("screens".into(), serde_json::Value::Array(Vec::new())),
    ]);
    let read = |window: &slint::Window| {
        window.with_winit_window(|native| {
            let scale = native.scale_factor().max(0.1);
            let position = native.outer_position().ok().unwrap_or_default();
            let size = native.outer_size();
            let current = native.current_monitor();
            let primary = native.primary_monitor();
            let screens = native
                .available_monitors()
                .map(|monitor| {
                    let origin = monitor.position();
                    let size = monitor.size();
                    let monitor_scale = monitor.scale_factor().max(0.1);
                    serde_json::json!({
                        "frame": {
                            "x": origin.x as f64 / monitor_scale,
                            "y": origin.y as f64 / monitor_scale,
                            "width": size.width as f64 / monitor_scale,
                            "height": size.height as f64 / monitor_scale,
                        },
                        "main": primary.as_ref().is_some_and(|primary| primary == &monitor),
                        "current": current.as_ref().is_some_and(|current| current == &monitor),
                    })
                })
                .collect::<Vec<_>>();
            (
                serde_json::json!({
                    "x": position.x as f64 / scale,
                    "y": position.y as f64 / scale,
                    "width": size.width as f64 / scale,
                    "height": size.height as f64 / scale,
                }),
                native
                    .is_visible()
                    .map(serde_json::Value::Bool)
                    .unwrap_or(serde_json::Value::Null),
                native
                    .is_minimized()
                    .map(serde_json::Value::Bool)
                    .unwrap_or(serde_json::Value::Null),
                screens,
                native.has_focus(),
            )
        })
    };
    let state = native_host
        .and_then(slint::Weak::upgrade)
        .and_then(|host| read(host.window()))
        .or_else(|| read(app.window()))
        .unwrap_or_else(|| {
            (
                serde_json::Value::Null,
                serde_json::Value::Null,
                serde_json::Value::Null,
                Vec::new(),
                false,
            )
        });
    properties.insert("loaded".into(), serde_json::Value::Bool(true));
    properties.insert("frame".into(), state.0);
    properties.insert("visible".into(), state.1);
    properties.insert("miniaturized".into(), state.2);
    properties.insert("screens".into(), serde_json::Value::Array(state.3));
    properties.insert("main".into(), serde_json::json!(state.4));
    properties
}

fn set_plugin_window_frame(window: &slint::Window, x: f64, y: f64, width: f64, height: f64) {
    let scale = f64::from(window.scale_factor()).max(0.1);
    window.set_position(slint::PhysicalPosition::new(
        (x * scale).round() as i32,
        (y * scale).round() as i32,
    ));
    window.set_size(slint::PhysicalSize::new(
        (width * scale).round() as u32,
        (height * scale).round() as u32,
    ));
}

fn apply_plugin_window_action(
    app: &PlayerWindow,
    native_host: Option<&slint::Weak<crate::NativeVideoHost>>,
    action: riina_core::plugin::WindowAction,
) {
    use riina_core::plugin::WindowAction;

    match action {
        WindowAction::SetFrame {
            x,
            y,
            width,
            height,
        } => {
            set_plugin_window_frame(app.window(), x, y, width, height);
            if let Some(host) = native_host.and_then(slint::Weak::upgrade) {
                set_plugin_window_frame(host.window(), x, y, width, height);
            }
        }
        WindowAction::SetFullscreen(fullscreen) => {
            if app.get_fullscreen() != fullscreen {
                app.invoke_toggle_fullscreen();
            }
        }
        WindowAction::SetPictureInPicture(pip) => {
            if app.get_picture_in_picture_active() != pip {
                app.invoke_toggle_picture_in_picture();
            }
        }
        WindowAction::SetOnTop(ontop) => app.set_always_float_on_top(ontop),
        WindowAction::SetSidebar(name) => {
            let index = name.as_deref().map_or(Some(0), plugin_sidebar_index);
            if let Some(index) = index {
                app.set_sidebar(index);
            } else {
                app.set_status("Plugin requested an unsupported sidebar name".into());
            }
        }
        WindowAction::SetMiniaturized(miniaturized) => {
            app.window()
                .with_winit_window(|window| window.set_minimized(miniaturized));
            if let Some(host) = native_host.and_then(slint::Weak::upgrade) {
                host.window()
                    .with_winit_window(|window| window.set_minimized(miniaturized));
            }
        }
    }
}
