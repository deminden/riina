//! Plugin callbacks, presenters and timers are installed as one UI subsystem.
mod application;
mod developer;
mod effects;
mod input;
use super::playlist_menu;

use crate::{PlayerWindow, background::CoalescedUiCallback, platform};
pub(crate) use application::ApplicationPlugins;
use effects::PluginEffects;
use riina_ui::PlayerMenuActions;
use slint::{ComponentHandle, Model, winit_030::WinitWindowAccessor};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

const PLUGIN_UI_INTERVAL: Duration = Duration::from_millis(25);
const WINDOW_PROPERTIES_INTERVAL: Duration = Duration::from_millis(125);

pub(super) struct InstalledPluginUi {
    presenters: platform::PluginPresenters,
    menu_observation: platform::PluginMenuObservation,
    standalone: Option<Rc<RefCell<platform::plugin_standalone::Presenter>>>,
    developer_tools: Option<Rc<RefCell<developer::Presenter>>>,
    dialog_owner: Option<platform::plugin_dialogs::OwnerRegistration>,
    pub(super) plugin_host_timer: Option<Rc<slint::Timer>>,
    pub(super) plugin_osd_timer: Option<Rc<slint::Timer>>,
    pub(super) plugin_command_sender: Arc<std::sync::Mutex<Option<riina_mpv::CommandSender>>>,
}

pub(super) struct RuntimeSession {
    _host: Option<crate::plugin_host::PluginHost>,
    _host_timer: Option<Rc<slint::Timer>>,
    _osd_timer: Option<Rc<slint::Timer>>,
    _standalone: Option<Rc<RefCell<platform::plugin_standalone::Presenter>>>,
    _developer_tools: Option<Rc<RefCell<developer::Presenter>>>,
    _dialog_owner: Option<platform::plugin_dialogs::OwnerRegistration>,
}

impl InstalledPluginUi {
    pub(super) fn finish(
        self,
        host: Option<crate::plugin_host::PluginHost>,
        system_media: platform::SystemMediaSession,
        picture_in_picture: platform::PictureInPictureSession,
        scroll_monitor: platform::PlayerScrollMonitor,
    ) -> (platform::SessionExtras, RuntimeSession) {
        (
            platform::SessionExtras::new(
                self.presenters,
                system_media,
                picture_in_picture,
                self.menu_observation,
                scroll_monitor,
            ),
            RuntimeSession {
                _host: host,
                _host_timer: self.plugin_host_timer,
                _osd_timer: self.plugin_osd_timer,
                _standalone: self.standalone,
                _developer_tools: self.developer_tools,
                _dialog_owner: self.dialog_owner,
            },
        )
    }
}

pub(super) fn install(
    app: &PlayerWindow,
    native: Option<&platform::NativePresentation>,
    plugin_host_handle: Option<crate::plugin_host::PluginHostHandle>,
    application_plugins: &ApplicationPlugins,
) -> InstalledPluginUi {
    let menu_observation =
        platform::PluginMenuObservation::install(app, plugin_host_handle.as_ref());
    let plugin_window_host = native.map(platform::NativePresentation::host);
    let plugin_command_sender = Arc::new(std::sync::Mutex::new(None::<riina_mpv::CommandSender>));
    let presenters = platform::PluginPresenters::new(app);
    let services = application_plugins.services.borrow();
    let developer_tools = plugin_host_handle.as_ref().map(|handle| {
        let global = services
            .as_ref()
            .map(|services| services.developer.clone())
            .unwrap_or_else(|| developer::Scope::new(Default::default()));
        developer::Presenter::install(app, handle.ui.developer.clone(), global)
    });
    let dialog_owner = services
        .as_ref()
        .map(|services| services.dialogs.register(app, plugin_window_host.clone()));
    bind_lifecycle(
        app,
        plugin_host_handle.as_ref(),
        presenters.sidebar.clone(),
        developer_tools.as_ref(),
    );
    playlist_menu::install(app, plugin_host_handle.as_ref());
    if let Some(handle) = &plugin_host_handle {
        input::install(app, &handle.ui.input);
        platform::plugin_dialogs::install(app, plugin_window_host.clone(), &handle.ui.dialogs);
    }
    {
        let weak = app.as_weak();
        let presenter = presenters.sidebar.clone();
        app.global::<riina_ui::PlayerSidebarActions>()
            .on_select_plugin_sidebar_tab(move |identifier| {
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
    let standalone = plugin_host_handle.as_ref().map(|handle| {
        let presenter = Rc::new(RefCell::new(platform::plugin_standalone::Presenter::new(
            handle.ui.windows.clone(),
        )));
        let presenter_weak = Rc::downgrade(&presenter);
        let overlay = presenters.overlay_delivery(native, handle);
        let global_weak = services
            .as_ref()
            .map(|services| Rc::downgrade(&services.standalone));
        let launches = services.as_ref().map(|services| {
            (
                Rc::downgrade(&application_plugins.managed),
                services.host.handle(),
            )
        });
        let developer_weak = developer_tools.as_ref().map(Rc::downgrade);
        let app_weak = app.as_weak();
        app.on_update_plugin_windows(move || {
            let Some(app) = app_weak.upgrade() else {
                return;
            };
            overlay.update(&app);
            if let Some(presenter) = developer_weak.as_ref().and_then(std::rc::Weak::upgrade)
                && let Ok(mut presenter) = presenter.try_borrow_mut()
            {
                presenter.update(&app);
            }
            if let Some(presenter) = presenter_weak.upgrade()
                && let Ok(mut presenter) = presenter.try_borrow_mut()
            {
                platform::pump_plugin_webviews();
                presenter.update(&app);
            }
            if let Some(presenter) = global_weak.as_ref().and_then(std::rc::Weak::upgrade)
                && let Ok(mut presenter) = presenter.try_borrow_mut()
            {
                presenter.update(&app);
            }
            if let Some((managed, global)) = &launches
                && let Some(managed) = managed.upgrade()
                && let Ok(mut managed) = managed.try_borrow_mut()
            {
                managed.update(&app, global);
            }
        });
        presenter
    });
    let plugin_host_timer = plugin_host_handle.as_ref().map(|handle| {
        let timer = Rc::new(slint::Timer::default());
        let messages = handle.clone();
        let sidebar_host = handle.clone();
        let window_host = plugin_window_host.clone();
        let mut window_events = WindowEvents::new(handle.clone(), window_host.clone());
        let window_update = CoalescedUiCallback::default();
        let weak = app.as_weak();
        let effects = PluginEffects {
            commands: plugin_command_sender.clone(),
            window_host,
            osd_timer: plugin_osd_timer_weak.clone(),
            osd_generation: plugin_osd_generation.clone(),
            overlay: presenters.overlay_delivery(native, handle),
            sidebar: presenters.sidebar.clone(),
            sidebar_host,
        };
        let running_timer = Rc::downgrade(&timer);
        timer.start(slint::TimerMode::Repeated, PLUGIN_UI_INTERVAL, move || {
            let (Some(app), Some(timer)) = (weak.upgrade(), running_timer.upgrade()) else {
                return;
            };
            // Native WebView operations can dispatch window events. Keep them
            // outside Slint's timer callback and its timer registry borrows.
            window_update.post(|| {
                let weak = weak.clone();
                move || {
                    if let Some(app) = weak.upgrade() {
                        app.invoke_update_plugin_windows();
                    }
                }
            });
            window_events.observe(&app, timer.interval() == WINDOW_PROPERTIES_INTERVAL);
            let Ok(batch) = messages.drain_messages() else {
                return;
            };
            let update_presenters = !batch.is_empty() || messages.has_active_sessions();
            for message in batch {
                effects.apply(&app, message);
            }
            if update_presenters {
                effects.update_presenters(&app);
            }
            update_timer_interval(&timer, messages.has_active_sessions());
        });
        bind_host_wake(app, handle, &timer);
        timer
    });
    InstalledPluginUi {
        presenters,
        menu_observation,
        standalone,
        developer_tools,
        dialog_owner,
        plugin_host_timer,
        plugin_osd_timer,
        plugin_command_sender,
    }
}

fn bind_host_wake(
    app: &PlayerWindow,
    handle: &crate::plugin_host::PluginHostHandle,
    timer: &Rc<slint::Timer>,
) {
    let timer = Rc::downgrade(timer);
    app.on_plugin_host_wake(move || {
        if let Some(timer) = timer.upgrade() {
            update_timer_interval(&timer, true);
        }
    });
    let weak = app.as_weak();
    let wake = CoalescedUiCallback::default();
    handle.set_ui_waker(move || {
        wake.post(|| {
            let weak = weak.clone();
            move || {
                if let Some(app) = weak.upgrade() {
                    app.invoke_plugin_host_wake();
                }
            }
        });
    });
}

fn update_timer_interval(timer: &slint::Timer, busy: bool) {
    // Keep the latest native window properties available to plugins enabled
    // later. With no sessions, the faster presenter cadence is unnecessary.
    let interval = if busy {
        PLUGIN_UI_INTERVAL
    } else {
        WINDOW_PROPERTIES_INTERVAL
    };
    if timer.interval() != interval {
        timer.set_interval(interval);
    }
}

fn bind_lifecycle(
    app: &PlayerWindow,
    plugin_host_handle: Option<&crate::plugin_host::PluginHostHandle>,
    sidebar: Rc<RefCell<platform::PluginSidebarPresenter>>,
    developer_tools: Option<&Rc<RefCell<developer::Presenter>>>,
) {
    let menu_action_host = plugin_host_handle.cloned();
    let menu_action_app = app.as_weak();
    app.global::<PlayerMenuActions>()
        .on_activate_plugin_menu_item(move |token| {
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
                crate::diagnostics::report(
                    &app,
                    format!("Could not run plugin menu action: {error}"),
                );
            }
        });
    let menu_update_host = plugin_host_handle.cloned();
    let menu_update_app = app.as_weak();
    app.on_plugin_menu_will_open(move || {
        if let Some(host) = &menu_update_host
            && let Err(error) = host.notify_menu_update()
            && let Some(app) = menu_update_app.upgrade()
        {
            crate::diagnostics::report(
                &app,
                format!("Could not notify plugins that their menu is opening: {error}"),
            );
        }
    });
    let reload_plugins_host = plugin_host_handle.cloned();
    let reload_plugins_app = app.as_weak();
    app.global::<PlayerMenuActions>()
        .on_reload_plugins(move || {
            let Some(host) = &reload_plugins_host else {
                if let Some(app) = reload_plugins_app.upgrade() {
                    app.set_status("The plugin runtime is unavailable.".into());
                }
                return;
            };
            if let Err(error) = host.reload_all()
                && let Some(app) = reload_plugins_app.upgrade()
            {
                crate::diagnostics::report(&app, format!("Could not reload plugins: {error}"));
            }
        });
    let close_event_sent = Rc::new(Cell::new(false));
    let close_event_sent_once = close_event_sent.clone();
    let close_event_host = plugin_host_handle.cloned();
    let developer_tools = developer_tools.map(Rc::downgrade);
    let weak = app.as_weak();
    app.on_window_will_close(move || {
        if close_event_sent_once.replace(true) {
            return;
        }
        if let Some(app) = weak.upgrade() {
            playlist_menu::dismiss(&app);
        }
        if let Some(presenter) = developer_tools.as_ref().and_then(std::rc::Weak::upgrade)
            && let Ok(mut presenter) = presenter.try_borrow_mut()
        {
            presenter.close_all();
        }
        if let Some(host) = &close_event_host
            && let Err(error) = host.notify_window_will_close()
            && let Some(app) = weak.upgrade()
        {
            crate::diagnostics::report(
                &app,
                format!("Could not notify plugins before closing: {error}"),
            );
        }
        sidebar.borrow_mut().close();
    });
}

struct WindowEvents {
    host: crate::plugin_host::PluginHostHandle,
    window: Option<slint::Weak<crate::NativeVideoHost>>,
    announced: bool,
    last_properties: Option<BTreeMap<String, serde_json::Value>>,
    last_update: Instant,
}

impl WindowEvents {
    fn new(
        host: crate::plugin_host::PluginHostHandle,
        window: Option<slint::Weak<crate::NativeVideoHost>>,
    ) -> Self {
        Self {
            host,
            window,
            announced: false,
            last_properties: None,
            last_update: Instant::now(),
        }
    }

    fn observe(&mut self, app: &PlayerWindow, window_properties_tick: bool) {
        if !self.announced && self.host.notify_window_loaded().is_ok() {
            self.announced = true;
        }
        if window_properties_tick || self.last_update.elapsed() >= WINDOW_PROPERTIES_INTERVAL {
            self.last_update = Instant::now();
            let properties = plugin_window_properties(app, self.window.as_ref());
            if self.last_properties.as_ref() != Some(&properties)
                && self
                    .host
                    .update_window_properties(properties.clone())
                    .is_ok()
            {
                self.last_properties = Some(properties);
            }
        }
    }
}

fn plugin_sidebar_tab_added(app: &PlayerWindow, identifier: &str, label: &str) {
    let mut identifiers = (0..app
        .global::<riina_ui::PlayerSidebarState>()
        .get_plugin_sidebar_identifiers()
        .row_count())
        .filter_map(|row| {
            app.global::<riina_ui::PlayerSidebarState>()
                .get_plugin_sidebar_identifiers()
                .row_data(row)
        })
        .map(|value| value.to_string())
        .collect::<Vec<_>>();
    let mut labels = (0..app
        .global::<riina_ui::PlayerSidebarState>()
        .get_plugin_sidebar_labels()
        .row_count())
        .filter_map(|row| {
            app.global::<riina_ui::PlayerSidebarState>()
                .get_plugin_sidebar_labels()
                .row_data(row)
        })
        .map(|value| value.to_string())
        .collect::<Vec<_>>();
    if let Some(index) = identifiers.iter().position(|current| current == identifier) {
        labels[index] = label.to_owned();
    } else {
        identifiers.push(identifier.to_owned());
        labels.push(label.to_owned());
    }
    app.global::<riina_ui::PlayerSidebarState>()
        .set_plugin_sidebar_identifiers(slint::ModelRc::new(slint::VecModel::from(
            identifiers
                .iter()
                .map(|value| slint::SharedString::from(value.as_str()))
                .collect::<Vec<_>>(),
        )));
    app.global::<riina_ui::PlayerSidebarState>()
        .set_plugin_sidebar_labels(slint::ModelRc::new(slint::VecModel::from(
            labels
                .iter()
                .map(|value| slint::SharedString::from(value.as_str()))
                .collect::<Vec<_>>(),
        )));
    let selected = app
        .global::<riina_ui::PlayerSidebarState>()
        .get_plugin_sidebar_selected_identifier()
        .to_string();
    app.global::<riina_ui::PlayerSidebarState>()
        .set_plugin_sidebar_selected_index(
            identifiers
                .iter()
                .position(|current| current == &selected)
                .map_or(-1, |index| index as i32),
        );
}

fn plugin_sidebar_tab_removed(app: &PlayerWindow, identifier: &str) {
    let mut identifiers = (0..app
        .global::<riina_ui::PlayerSidebarState>()
        .get_plugin_sidebar_identifiers()
        .row_count())
        .filter_map(|row| {
            app.global::<riina_ui::PlayerSidebarState>()
                .get_plugin_sidebar_identifiers()
                .row_data(row)
        })
        .map(|value| value.to_string())
        .collect::<Vec<_>>();
    let mut labels = (0..app
        .global::<riina_ui::PlayerSidebarState>()
        .get_plugin_sidebar_labels()
        .row_count())
        .filter_map(|row| {
            app.global::<riina_ui::PlayerSidebarState>()
                .get_plugin_sidebar_labels()
                .row_data(row)
        })
        .map(|value| value.to_string())
        .collect::<Vec<_>>();
    if let Some(index) = identifiers.iter().position(|current| current == identifier) {
        identifiers.remove(index);
        labels.remove(index);
    }
    if app
        .global::<riina_ui::PlayerSidebarState>()
        .get_plugin_sidebar_selected_identifier()
        .as_str()
        == identifier
    {
        app.global::<riina_ui::PlayerSidebarState>()
            .set_plugin_sidebar_selected_identifier("".into());
    }
    app.global::<riina_ui::PlayerSidebarState>()
        .set_plugin_sidebar_identifiers(slint::ModelRc::new(slint::VecModel::from(
            identifiers
                .iter()
                .map(|value| slint::SharedString::from(value.as_str()))
                .collect::<Vec<_>>(),
        )));
    app.global::<riina_ui::PlayerSidebarState>()
        .set_plugin_sidebar_labels(slint::ModelRc::new(slint::VecModel::from(
            labels
                .iter()
                .map(|value| slint::SharedString::from(value.as_str()))
                .collect::<Vec<_>>(),
        )));
    let selected = app
        .global::<riina_ui::PlayerSidebarState>()
        .get_plugin_sidebar_selected_identifier()
        .to_string();
    app.global::<riina_ui::PlayerSidebarState>()
        .set_plugin_sidebar_selected_index(
            identifiers
                .iter()
                .position(|current| current == &selected)
                .map_or(-1, |index| index as i32),
        );
}

fn select_plugin_sidebar_tab(app: &PlayerWindow, identifier: &str) {
    app.global::<riina_ui::PlayerSidebarState>()
        .set_plugin_sidebar_selected_identifier(identifier.into());
    if app
        .global::<riina_ui::PlayerSidebarState>()
        .get_leading_sidebar()
        != 7
        && app.global::<riina_ui::PlayerSidebarState>().get_sidebar() != 7
    {
        // Use the configured IINA placement while leaving an already-open
        // Plugins panel on its current side when a different tab is selected.
        crate::sidebar_preferences::show_plugins_panel(app);
    }
    let index = (0..app
        .global::<riina_ui::PlayerSidebarState>()
        .get_plugin_sidebar_identifiers()
        .row_count())
        .find(|row| {
            app.global::<riina_ui::PlayerSidebarState>()
                .get_plugin_sidebar_identifiers()
                .row_data(*row)
                .is_some_and(|value| value.as_str() == identifier)
        })
        .map_or(-1, |row| row as i32);
    app.global::<riina_ui::PlayerSidebarState>()
        .set_plugin_sidebar_selected_index(index);
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
        (
            "music-mode".into(),
            serde_json::json!(
                app.global::<riina_ui::PlayerMusicModeState>()
                    .get_music_mode()
            ),
        ),
        ("fullscreen".into(), serde_json::json!(app.get_fullscreen())),
        (
            "pip".into(),
            serde_json::json!(app.get_picture_in_picture_active()),
        ),
        (
            "ontop".into(),
            serde_json::json!(
                app.global::<riina_ui::PlayerLayoutState>()
                    .get_always_float_on_top()
            ),
        ),
        (
            "sidebar".into(),
            plugin_sidebar_name(app.global::<riina_ui::PlayerSidebarState>().get_sidebar())
                .or_else(|| {
                    plugin_sidebar_name(
                        app.global::<riina_ui::PlayerSidebarState>()
                            .get_leading_sidebar(),
                    )
                })
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
        WindowAction::Quit => app.global::<PlayerMenuActions>().invoke_quit_riina(),
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
            app.global::<PlayerMenuActions>()
                .invoke_request_fullscreen(fullscreen);
        }
        WindowAction::SetPictureInPicture(pip) => {
            if app.get_picture_in_picture_active() != pip {
                app.global::<PlayerMenuActions>()
                    .invoke_toggle_picture_in_picture();
            }
        }
        WindowAction::SetOnTop(ontop) => app
            .global::<riina_ui::PlayerLayoutState>()
            .set_always_float_on_top(ontop),
        WindowAction::SetSidebar(name) => {
            let index = name.as_deref().map_or(Some(0), plugin_sidebar_index);
            if let Some(index) = index {
                app.global::<riina_ui::PlayerSidebarState>()
                    .set_sidebar(index);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin_host::PluginHost;
    use riina_test_support::TempDir;

    #[test]
    fn empty_host_uses_the_window_tick_and_messages_restore_the_fast_tick() {
        i_slint_backend_testing::init_no_event_loop();
        let app = PlayerWindow::new().unwrap();
        let directory = TempDir::new("idle-plugin-presenter").unwrap();
        let host = PluginHost::start(&directory.path().join("preferences.json"), &[], &[]).unwrap();
        let handle = host.handle();
        let deadline = Instant::now() + Duration::from_secs(2);
        while handle.has_active_sessions() {
            assert!(
                Instant::now() < deadline,
                "Empty plugin host did not become idle"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        let installed = install(&app, None, Some(handle), &ApplicationPlugins::default());
        let timer = installed.plugin_host_timer.as_ref().unwrap();
        assert_eq!(timer.interval(), PLUGIN_UI_INTERVAL);
        i_slint_backend_testing::mock_elapsed_time(PLUGIN_UI_INTERVAL);
        assert_eq!(timer.interval(), WINDOW_PROPERTIES_INTERVAL);
        app.invoke_plugin_host_wake();
        assert_eq!(timer.interval(), PLUGIN_UI_INTERVAL);
        i_slint_backend_testing::mock_elapsed_time(PLUGIN_UI_INTERVAL);
        assert_eq!(timer.interval(), WINDOW_PROPERTIES_INTERVAL);
    }
}
