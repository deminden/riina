//! A QuickJS realm and every callback stay on one worker until cancellation.
use super::{
    AvailablePermissions, HostEvent, MAX_RUNTIME_MEMORY, MAX_STACK_BYTES, PluginWorkerHost,
    RuntimeCommand,
    core_api::install_plugin_core_api,
    events,
    file_api::install_plugin_file_api,
    menu::PLUGIN_MENU_API,
    messaging, modules, network,
    network::NetworkWorkerContext,
    output::{menu_js_error, record, record_plugin_menu},
    timers,
    timers::PluginTimers,
    webviews,
};
use riina_core::plugin_menu::PluginMenuItem;
use riina_core::{plugin::OverlayState, plugin_sidebar::PluginSidebarState};
use rquickjs::{Context, Object, Runtime, Value, function::Func};
use std::sync::{
    Arc, Mutex,
    mpsc::{self, Receiver, SyncSender},
};

pub(super) fn run_entry_worker(
    source: String,
    plugin_identifier: String,
    permissions: AvailablePermissions,
    host: PluginWorkerHost,
    commands: Receiver<RuntimeCommand>,
    ready: SyncSender<Result<(), String>>,
    network: NetworkWorkerContext,
) {
    let PluginWorkerHost {
        execution,
        global_api_mode,
        module_context,
        file_system,
        events,
        player_event_subscriptions,
        observed_mpv_properties,
        mpv_property_observation_requests,
    } = host;
    let interrupt_control = execution.clone();
    let runtime = match Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => {
            let _ = ready.send(Err(format!("Could not create JavaScript runtime: {error}")));
            return;
        }
    };
    runtime.set_memory_limit(MAX_RUNTIME_MEMORY);
    runtime.set_max_stack_size(MAX_STACK_BYTES);
    // Each evaluation gets its own bounded deadline; plugin sessions may remain
    // idle indefinitely without their interrupt handler expiring in the future.
    runtime.set_interrupt_handler(Some(Box::new(move || interrupt_control.interrupted())));
    if let Err(error) = runtime.set_info(format!("riina plugin {plugin_identifier}")) {
        let _ = ready.send(Err(format!("Could not label JavaScript runtime: {error}")));
        return;
    }

    let context = match Context::full(&runtime) {
        Ok(context) => context,
        Err(error) => {
            let _ = ready.send(Err(format!("Could not create plugin context: {error}")));
            return;
        }
    };
    let overlay_state = Arc::new(Mutex::new(OverlayState::default()));
    let worker_overlay_state = overlay_state.clone();
    let sidebar_state = Arc::new(Mutex::new(PluginSidebarState::default()));
    let worker_sidebar_state = sidebar_state.clone();
    let timers = Arc::new(PluginTimers::default());
    let worker_timers = timers.clone();
    execution.begin();
    let initialized = context.with(|ctx| {
        let globals = ctx.globals();
        let api = Object::new(ctx.clone())?;

        let log_events = events.clone();
        api.set(
            "log",
            Func::from(move |message: String| {
                record(&log_events, HostEvent::Log(message));
            }),
        )?;
        if permissions.show_osd {
            let osd_events = events.clone();
            api.set(
                "showOSD",
                Func::from(move |message: String| {
                    record(&osd_events, HostEvent::ShowOsd(message));
                }),
            )?;
        }
        if permissions.show_alert {
            let alert_events = events.clone();
            api.set(
                "showAlert",
                Func::from(move |message: String| {
                    record(&alert_events, HostEvent::ShowAlert(message));
                }),
            )?;
        }
        globals.set("iina", api.clone())?;
        events::install(
            ctx.clone(),
            api.clone(),
            global_api_mode,
            player_event_subscriptions,
            observed_mpv_properties,
            mpv_property_observation_requests,
        )?;
        let menu_events = events.clone();
        api.set(
            "__menuUpdate",
            Func::from(move |json: String| -> rquickjs::Result<()> {
                let items = serde_json::from_str::<Vec<PluginMenuItem>>(&json)
                    .map_err(|error| menu_js_error(error.to_string()))?;
                PluginMenuItem::validate_all(&items).map_err(menu_js_error)?;
                record_plugin_menu(&menu_events, items).map_err(menu_js_error)
            }),
        )?;
        ctx.eval::<(), _>(PLUGIN_MENU_API)?;
        install_plugin_file_api(
            ctx.clone(),
            globals.clone(),
            file_system.clone(),
            permissions.file_system,
        )?;
        install_plugin_core_api(
            ctx.clone(),
            api.clone(),
            events.clone(),
            file_system.clone(),
            permissions,
            network.allowed_domains.clone(),
        )?;
        network::install(ctx.clone(), file_system.clone(), permissions, &network)?;
        modules::install(ctx.clone(), module_context.clone())?;
        messaging::install(ctx.clone(), api.clone(), global_api_mode, events.clone())?;
        timers::install(ctx.clone(), timers.clone())?;
        webviews::install(
            ctx.clone(),
            events.clone(),
            permissions,
            overlay_state.clone(),
            sidebar_state.clone(),
        )?;
        let console = Object::new(ctx.clone())?;
        for method in ["log", "info", "warn", "error"] {
            let log_events = events.clone();
            console.set(
                method,
                Func::from(move |message: String| {
                    record(&log_events, HostEvent::Log(message));
                }),
            )?;
        }
        globals.set("console", console)?;

        // Keep the entry's path on the stack only while it is being evaluated:
        // nested CommonJS loads resolve relative to that file.
        if let Some(module_context) = module_context.as_ref() {
            module_context
                .current_files
                .lock()
                .map_err(|_| {
                    rquickjs::Error::new_from_js_message(
                        "require",
                        "entry path",
                        "module path stack was poisoned",
                    )
                })?
                .push(module_context.entry.clone());
        }
        let evaluated = ctx.eval::<Value, _>(source.as_str()).map(|_| ());
        if let Some(module_context) = module_context.as_ref()
            && let Ok(mut current_files) = module_context.current_files.lock()
        {
            current_files.pop();
        }
        evaluated
    });
    if let Err(error) = initialized {
        let _ = ready.send(Err(format!(
            "Plugin {plugin_identifier} failed to initialize: {error}"
        )));
        return;
    }
    if let Err(error) = execution.drain_jobs(&runtime) {
        let _ = ready.send(Err(format!(
            "Plugin {plugin_identifier} failed to initialize: {error}"
        )));
        return;
    }
    if ready.send(Ok(())).is_err() {
        return;
    }

    while !execution.is_cancelled() {
        let command = match worker_timers.next_wait() {
            Some(timeout) => match commands.recv_timeout(timeout) {
                Ok(command) => Some(command),
                Err(mpsc::RecvTimeoutError::Timeout) => None,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            },
            None => match commands.recv() {
                Ok(command) => Some(command),
                Err(_) => break,
            },
        };
        if execution.is_cancelled() {
            break;
        }
        let (script, label) = match command {
            None => {
                let Some(id) = worker_timers.take_due() else {
                    continue;
                };
                (format!("__riinaTimers.fire({id})"), "timer callback")
            }
            Some(RuntimeCommand::Shutdown) => break,
            Some(RuntimeCommand::Barrier(response)) => {
                let _ = response.send(());
                continue;
            }
            Some(RuntimeCommand::DeliverPluginMenuAction(id)) => (
                format!("iina.menu.__activate({})", serde_json::json!(id)),
                "menu action",
            ),
            Some(RuntimeCommand::DeliverOverlayMessage(name, data)) => {
                if !worker_overlay_state.lock().is_ok_and(|state| {
                    matches!(
                        state.receive_message(name.clone(), data.clone()),
                        Ok(Some(_))
                    )
                }) {
                    continue;
                }
                (
                    message_script("iina.overlay.__deliverMessage", &name, &data),
                    "overlay callback",
                )
            }
            Some(RuntimeCommand::DeliverSidebarMessage(name, data)) => {
                if !worker_sidebar_state.lock().is_ok_and(|state| {
                    matches!(
                        state.receive_message(name.clone(), data.clone()),
                        Ok(Some(_))
                    )
                }) {
                    continue;
                }
                (
                    message_script("iina.sidebar.__deliverMessage", &name, &data),
                    "sidebar callback",
                )
            }
            Some(RuntimeCommand::DeliverPluginMessage(name, data)) => (
                message_script("iina.global.__deliverMessage", &name, &data),
                "message callback",
            ),
            Some(RuntimeCommand::DeliverPlayerEvent(name, data)) => (
                message_script("iina.event.__dispatch", &name, &data),
                "player event callback",
            ),
            Some(RuntimeCommand::DeliverNetworkResult(id, result)) => (
                network_result_script("__riinaDeliverHttp", id, result),
                "HTTP response callback",
            ),
            Some(RuntimeCommand::DeliverNetworkDownload(id, result)) => {
                let error = result
                    .err()
                    .map_or(serde_json::Value::Null, serde_json::Value::String);
                (
                    format!("__riinaDeliverDownload({id}, {error})"),
                    "HTTP download callback",
                )
            }
            Some(RuntimeCommand::DeliverXmlRpcResult(id, result)) => (
                network_result_script("__riinaDeliverXmlrpc", id, result),
                "XML-RPC response callback",
            ),
        };
        execution.begin();
        let evaluated = context.with(|ctx| {
            ctx.eval::<Value, _>(script).map(|_| ()).map_err(|error| {
                if matches!(error, rquickjs::Error::Exception) {
                    ctx.catch();
                }
                error.to_string()
            })
        });
        // Jobs queued before a synchronous exception still belong to this turn.
        // Every route drains them under the same cancellation/deadline budget.
        let pending = execution.drain_jobs(&runtime);
        for error in evaluated.err().into_iter().chain(pending.err()) {
            record(
                &events,
                HostEvent::Log(format!("Plugin {label} failed: {error}")),
            );
        }
        if execution.interrupted() {
            break;
        }
    }
}

fn message_script(function: &str, name: &str, data: &serde_json::Value) -> String {
    format!("{function}({}, {data})", serde_json::json!(name))
}

fn network_result_script(
    function: &str,
    id: u32,
    result: Result<serde_json::Value, String>,
) -> String {
    let (response, error) = match result {
        Ok(response) => (response, serde_json::Value::Null),
        Err(error) => (serde_json::Value::Null, serde_json::Value::String(error)),
    };
    format!("{function}({id}, {response}, {error})")
}
