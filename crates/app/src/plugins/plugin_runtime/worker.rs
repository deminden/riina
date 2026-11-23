//! A QuickJS realm and every callback stay on one worker until cancellation.
use super::{
    AvailablePermissions, GlobalApiMode, MAX_RUNTIME_MEMORY, MAX_STACK_BYTES, PluginWorkerHost,
    RuntimeCommand, UiEffect,
    core_api::install_plugin_core_api,
    events,
    file_api::install_plugin_file_api,
    input,
    menu::PLUGIN_MENU_API,
    messaging, modules, mpv_api, mpv_hooks, network,
    network::NetworkWorkerContext,
    output::{menu_js_error, record, record_plugin_menu},
    playlist_api, preferences_api, subtitles, timers,
    timers::PluginTimers,
    utilities, web_socket, webviews,
};
use riina_core::plugin_menu::PluginMenuItem;
use riina_core::{plugin::OverlayState, plugin_sidebar::PluginSidebarState};
use rquickjs::{Context, Object, Runtime, Value, function::Func};
use std::{
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, SyncSender},
    },
    time::Instant,
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
    // The starter can time out and disconnect before receiving initialization errors.
    let execution = host.execution.clone();
    let events = host.events.clone();
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
    let services = RealmServices {
        overlay: Arc::default(),
        sidebar: Arc::default(),
        timers: Arc::default(),
        hooks: mpv_hooks::RealmHooks::default(),
        web_socket: web_socket::RealmWebSocket::default(),
        subtitles: subtitles::Realm::new(host.file_system.clone(), permissions),
        helper_workers: utilities::Workers::default(),
    };
    let _cancel_on_exit = CancelOnExit {
        execution: execution.clone(),
        workers: services.helper_workers.clone(),
    };
    execution.begin();
    let initialized =
        context.with(|ctx| initialize_realm(ctx, source, permissions, host, &network, &services));
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

    run_callbacks(&context, &runtime, commands, &execution, &events, &services);
}

struct CancelOnExit {
    execution: Arc<super::ExecutionControl>,
    workers: utilities::Workers,
}

impl Drop for CancelOnExit {
    fn drop(&mut self) {
        self.execution.cancel();
        self.workers.finish();
    }
}

struct RealmServices {
    overlay: Arc<Mutex<OverlayState>>,
    sidebar: Arc<Mutex<PluginSidebarState>>,
    timers: Arc<PluginTimers>,
    hooks: mpv_hooks::RealmHooks,
    web_socket: web_socket::RealmWebSocket,
    subtitles: subtitles::Realm,
    helper_workers: utilities::Workers,
}

fn initialize_realm(
    ctx: rquickjs::Ctx<'_>,
    source: String,
    permissions: AvailablePermissions,
    host: PluginWorkerHost,
    network: &NetworkWorkerContext,
    services: &RealmServices,
) -> rquickjs::Result<()> {
    let PluginWorkerHost {
        file_dialogs,
        input,
        global_api_mode,
        module_context,
        file_system,
        player_mpv,
        execution,
        preferences,
        events,
        player_event_subscriptions,
        observed_mpv_properties,
        mpv_property_observation_requests,
    } = host;
    let globals = ctx.globals();
    let api = Object::new(ctx.clone())?;

    let log_events = events.clone();
    api.set(
        "log",
        Func::from(move |message: String| {
            record(&log_events, UiEffect::Log(message));
        }),
    )?;
    if permissions.show_osd {
        let osd_events = events.clone();
        api.set(
            "showOSD",
            Func::from(move |message: String| {
                record(&osd_events, UiEffect::ShowOsd(message));
            }),
        )?;
    }
    if permissions.show_alert {
        let alert_events = events.clone();
        api.set(
            "showAlert",
            Func::from(move |message: String| {
                record(&alert_events, UiEffect::ShowAlert(message));
            }),
        )?;
    }
    globals.set("iina", api.clone())?;
    utilities::install(
        &api,
        &file_system,
        permissions,
        execution.clone(),
        network.commands.clone(),
        file_dialogs,
        services.helper_workers.clone(),
    )?;
    preferences_api::install(&api, preferences)?;
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
            PluginMenuItem::validate_all(&items)
                .map_err(|error| menu_js_error(error.to_string()))?;
            record_plugin_menu(&menu_events, items).map_err(menu_js_error)
        }),
    )?;
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
    if global_api_mode != Some(GlobalApiMode::Controller) {
        input::install(&api, input)?;
        subtitles::Realm::install(&api)?;
        mpv_api::install(
            &api,
            player_mpv.clone(),
            execution.clone(),
            &file_system,
            permissions,
            &network.allowed_domains,
            &events,
        )?;
        services
            .hooks
            .install(&api, player_mpv, execution, network.commands.clone())?;
        playlist_api::install(
            &api,
            &events,
            &file_system,
            permissions,
            &network.allowed_domains,
        )?;
    }
    ctx.eval::<(), _>(PLUGIN_MENU_API)?;
    network::install(ctx.clone(), file_system.clone(), permissions, network)?;
    services
        .web_socket
        .install(&api, permissions.network_listener, network.commands.clone())?;
    modules::install(ctx.clone(), module_context.clone())?;
    if let Some(mode) = global_api_mode {
        messaging::install(ctx.clone(), api.clone(), mode, events.clone())?;
    }
    timers::install(ctx.clone(), services.timers.clone())?;
    webviews::install(
        ctx.clone(),
        events.clone(),
        permissions,
        services.overlay.clone(),
        services.sidebar.clone(),
    )?;
    install_console(&globals, events)?;

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
    // A poisoned ancestry stack cannot establish the base of a require.
    if let Some(module_context) = module_context.as_ref()
        && let Ok(mut current_files) = module_context.current_files.lock()
    {
        current_files.pop();
    }
    evaluated
}

fn install_console<'js>(
    globals: &Object<'js>,
    events: Arc<Mutex<Vec<super::HostEvent>>>,
) -> rquickjs::Result<()> {
    let console = Object::new(globals.ctx().clone())?;
    for method in ["log", "info", "warn", "error"] {
        let log_events = events.clone();
        console.set(
            method,
            Func::from(move |message: String| record(&log_events, UiEffect::Log(message))),
        )?;
    }
    globals.set("console", console)
}

fn run_callbacks(
    context: &Context,
    runtime: &Runtime,
    commands: Receiver<RuntimeCommand>,
    execution: &super::execution::ExecutionControl,
    events: &Mutex<Vec<super::HostEvent>>,
    services: &RealmServices,
) {
    let mut prefer_timer = true;
    while !execution.is_cancelled() {
        let Ok(command) = receive_turn(&commands, &services.timers, prefer_timer) else {
            break;
        };
        if execution.is_cancelled() {
            break;
        }
        prefer_timer = command.is_some();
        let (script, label) = match command {
            None => {
                let Some(id) = services.timers.take_due() else {
                    continue;
                };
                (format!("__riinaTimers.fire({id})"), "timer callback")
            }
            Some(RuntimeCommand::Shutdown) => break,
            Some(RuntimeCommand::Barrier(response)) => {
                // A caller can time out before the worker reaches its barrier.
                let _ = response.send(());
                continue;
            }
            Some(RuntimeCommand::DeliverPluginMenuAction(id)) => (
                format!("iina.menu.__activate({})", serde_json::json!(id)),
                "menu action",
            ),
            Some(RuntimeCommand::BuildPlaylistMenu {
                request,
                indices,
                deadline,
                response,
            }) => {
                if Instant::now() < deadline {
                    execution.begin();
                    let result =
                        build_playlist_turn(context, runtime, execution, request, &indices);
                    match result {
                        Ok(items) => _ = response.send(items),
                        Err(error) => record(
                            events,
                            UiEffect::Log(format!("Plugin playlist menu builder failed: {error}")),
                        ),
                    }
                    if execution.interrupted() {
                        break;
                    }
                }
                continue;
            }
            Some(RuntimeCommand::ActivatePlaylistMenuItem(request, id)) => (
                format!(
                    "iina.menu.__activatePlaylistItem(\"{request}\", {})",
                    serde_json::json!(id)
                ),
                "playlist menu action",
            ),
            #[cfg(any(target_os = "windows", test))]
            Some(RuntimeCommand::DeliverOverlayMessage(name, data)) => {
                // An interrupted subscription update must not admit a message.
                if !services.overlay.lock().is_ok_and(|state| {
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
                // An interrupted subscription update must not admit a message.
                if !services.sidebar.lock().is_ok_and(|state| {
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
            Some(
                command @ (RuntimeCommand::DeliverUtility(_)
                | RuntimeCommand::DeliverInput(_)
                | RuntimeCommand::DeliverMpvHook(..)
                | RuntimeCommand::DeliverWebSocket(_)
                | RuntimeCommand::RunSubtitleProvider(_)),
            ) => {
                execution.begin();
                let (label, evaluated) = dispatch_typed_callback(context, services, command);
                if finish_turn(runtime, execution, events, label, evaluated) {
                    break;
                }
                continue;
            }
        };
        execution.begin();
        let evaluated = evaluate_callback(context, script);
        if finish_turn(runtime, execution, events, label, evaluated) {
            break;
        }
    }
}

fn dispatch_typed_callback(
    context: &Context,
    services: &RealmServices,
    command: RuntimeCommand,
) -> (&'static str, Result<(), String>) {
    match command {
        RuntimeCommand::DeliverUtility(delivery) => {
            ("utility callback", delivery.dispatch(context))
        }
        RuntimeCommand::DeliverInput(request) => ("input listener", request.dispatch(context)),
        RuntimeCommand::DeliverMpvHook(index, next) => {
            ("mpv hook", mpv_hooks::dispatch(context, index, next))
        }
        RuntimeCommand::DeliverWebSocket(delivery) => (
            "WebSocket callback",
            services.web_socket.dispatch(context, delivery),
        ),
        RuntimeCommand::RunSubtitleProvider(job) => (
            "subtitle provider",
            services.subtitles.dispatch(context, job),
        ),
        _ => (
            "typed callback",
            Err("Unexpected runtime command for typed callback".into()),
        ),
    }
}

fn receive_turn(
    commands: &Receiver<RuntimeCommand>,
    timers: &PluginTimers,
    prefer_timer: bool,
) -> Result<Option<RuntimeCommand>, mpsc::RecvError> {
    match timers.next_wait() {
        // Alternate ready timers and commands so one source cannot starve the other.
        Some(timeout) if prefer_timer && timeout.is_zero() => Ok(None),
        Some(timeout) => match commands.recv_timeout(timeout) {
            Ok(command) => Ok(Some(command)),
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(mpsc::RecvError),
        },
        None => commands.recv().map(Some),
    }
}

fn build_playlist_turn(
    context: &Context,
    runtime: &Runtime,
    execution: &super::execution::ExecutionControl,
    request: u64,
    indices: &[usize],
) -> Result<Vec<PluginMenuItem>, String> {
    let items = super::menu::build_playlist_menu(context, request, indices);
    let pending = execution.drain_jobs(runtime);
    items.and_then(|items| pending.map(|()| items))
}

fn evaluate_callback(context: &Context, script: String) -> Result<(), String> {
    context.with(|ctx| {
        ctx.eval::<Value, _>(script).map(|_| ()).map_err(|error| {
            if error.is_exception() {
                ctx.catch();
            }
            error.to_string()
        })
    })
}

fn finish_turn(
    runtime: &Runtime,
    execution: &super::execution::ExecutionControl,
    events: &Mutex<Vec<super::HostEvent>>,
    label: &str,
    evaluated: Result<(), String>,
) -> bool {
    // Jobs queued before a synchronous exception still use this turn's deadline.
    let pending = execution.drain_jobs(runtime);
    for error in evaluated.err().into_iter().chain(pending.err()) {
        record(
            events,
            UiEffect::Log(format!("Plugin {label} failed: {error}")),
        );
    }
    execution.interrupted()
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
