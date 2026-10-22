//! Overlay and sidebar API registration; effects cross to the UI as typed data.
use super::{
    AvailablePermissions, HostEvent,
    output::{overlay_js_error, record_overlay, record_plugin_sidebar, sidebar_js_error},
};
use riina_core::{
    plugin::{OverlayIntent, OverlayState},
    plugin_sidebar::{PluginSidebarIntent, PluginSidebarState},
};
use rquickjs::{Ctx, Object, Value, function::Func};
use std::sync::{Arc, Mutex};

pub(super) fn install(
    ctx: Ctx<'_>,
    events: Arc<Mutex<Vec<HostEvent>>>,
    permissions: AvailablePermissions,
    overlay_state: Arc<Mutex<OverlayState>>,
    sidebar_state: Arc<Mutex<PluginSidebarState>>,
) -> rquickjs::Result<()> {
    let globals = ctx.globals();
    let overlay = Object::new(ctx.clone())?;
    let overlay_state = overlay_state.clone();
    let overlay_events = events.clone();
    let overlay_state_for_call = overlay_state.clone();
    let can_use_overlay = permissions.video_overlay;
    overlay.set(
        "loadFile",
        Func::from(move |path: String| -> rquickjs::Result<()> {
            if !can_use_overlay {
                return Err(rquickjs::Error::new_from_js_message(
                    "plugin permission",
                    "video overlay",
                    "overlay API called without permission",
                ));
            }
            record_overlay(
                &overlay_events,
                &overlay_state_for_call,
                OverlayIntent::LoadFile(path.into()),
            )
            .map_err(overlay_js_error)?;
            Ok(())
        }),
    )?;
    let overlay_events = events.clone();
    let overlay_state_for_call = overlay_state.clone();
    let can_use_overlay = permissions.video_overlay;
    overlay.set(
        "simpleMode",
        Func::from(move || -> rquickjs::Result<()> {
            if !can_use_overlay {
                return Err(rquickjs::Error::new_from_js_message(
                    "plugin permission",
                    "video overlay",
                    "overlay API called without permission",
                ));
            }
            record_overlay(
                &overlay_events,
                &overlay_state_for_call,
                OverlayIntent::SimpleMode,
            )
            .map_err(overlay_js_error)?;
            Ok(())
        }),
    )?;
    for (method, intent) in [("show", 0), ("hide", 1)] {
        let overlay_events = events.clone();
        let overlay_state_for_call = overlay_state.clone();
        overlay.set(
            method,
            Func::from(move || -> rquickjs::Result<()> {
                if can_use_overlay {
                    let intent = if intent == 0 {
                        OverlayIntent::Show
                    } else {
                        OverlayIntent::Hide
                    };
                    record_overlay(&overlay_events, &overlay_state_for_call, intent)
                        .map_err(overlay_js_error)?;
                }
                Ok(())
            }),
        )?;
    }
    let overlay_events = events.clone();
    let overlay_state_for_call = overlay_state.clone();
    overlay.set(
        "setOpacity",
        Func::from(move |opacity: f64| -> rquickjs::Result<()> {
            if can_use_overlay {
                record_overlay(
                    &overlay_events,
                    &overlay_state_for_call,
                    OverlayIntent::SetOpacity(opacity),
                )
                .map_err(overlay_js_error)?;
            }
            Ok(())
        }),
    )?;
    for (method, style) in [("setStyle", true), ("setContent", false)] {
        let overlay_events = events.clone();
        let overlay_state_for_call = overlay_state.clone();
        overlay.set(
            method,
            Func::from(move |value: String| -> rquickjs::Result<()> {
                if can_use_overlay {
                    let intent = if style {
                        OverlayIntent::SetStyle(value)
                    } else {
                        OverlayIntent::SetContent(value)
                    };
                    record_overlay(&overlay_events, &overlay_state_for_call, intent)
                        .map_err(overlay_js_error)?;
                }
                Ok(())
            }),
        )?;
    }
    let overlay_events = events.clone();
    let overlay_state_for_call = overlay_state.clone();
    overlay.set(
        "setClickable",
        Func::from(move |clickable: bool| -> rquickjs::Result<()> {
            if can_use_overlay {
                record_overlay(
                    &overlay_events,
                    &overlay_state_for_call,
                    OverlayIntent::SetClickable(clickable),
                )
                .map_err(overlay_js_error)?;
            }
            Ok(())
        }),
    )?;
    let overlay_events = events.clone();
    let overlay_state_for_call = overlay_state.clone();
    let can_use_overlay = permissions.video_overlay;
    overlay.set(
        "postMessage",
        Func::from(move |name: String, data: String| -> rquickjs::Result<()> {
            if !can_use_overlay {
                return Ok(());
            }
            let data = serde_json::from_str(&data).map_err(|error| {
                rquickjs::Error::new_from_js_message("string", "JSON", error.to_string())
            })?;
            record_overlay(
                &overlay_events,
                &overlay_state_for_call,
                OverlayIntent::PostMessage { name, data },
            )
            .map_err(overlay_js_error)?;
            Ok(())
        }),
    )?;
    let overlay_events = events.clone();
    let overlay_state_for_call = overlay_state.clone();
    let can_use_overlay = permissions.video_overlay;
    overlay.set(
        "onMessage",
        Func::from(
            move |name: String, _callback: Value| -> rquickjs::Result<()> {
                if can_use_overlay {
                    record_overlay(
                        &overlay_events,
                        &overlay_state_for_call,
                        OverlayIntent::OnMessage(name),
                    )
                    .map_err(overlay_js_error)?;
                }
                Ok(())
            },
        ),
    )?;
    globals.set("__riinaOverlay", overlay)?;
    ctx.eval::<(), _>(
    r#"iina.overlay = __riinaOverlay;
    (() => {
        const overlay = iina.overlay;
        const postMessage = overlay.postMessage;
        const loadFile = overlay.loadFile;
        const simpleMode = overlay.simpleMode;
        const onMessage = overlay.onMessage;
        const listeners = new Map();
        overlay.postMessage = (name, data) => postMessage(name, JSON.stringify(data));
        overlay.loadFile = path => { const result = loadFile(path); listeners.clear(); return result; };
        overlay.simpleMode = () => { const result = simpleMode(); listeners.clear(); return result; };
        overlay.onMessage = (name, callback) => {
            if (typeof callback !== 'function') throw new TypeError('callback must be a function');
            onMessage(name, callback);
            const callbacks = listeners.get(name) || [];
            callbacks.push(callback);
            listeners.set(name, callbacks);
        };
        overlay.__deliverMessage = (name, data) => {
            for (const callback of listeners.get(name) || []) callback(data);
        };
    })();"#,
)?;

    let sidebar = Object::new(ctx.clone())?;
    for (method, intent) in [("show", 0), ("hide", 1)] {
        let sidebar_events = events.clone();
        let sidebar_state = sidebar_state.clone();
        sidebar.set(
            method,
            Func::from(move || -> rquickjs::Result<()> {
                let intent = if intent == 0 {
                    PluginSidebarIntent::Show
                } else {
                    PluginSidebarIntent::Hide
                };
                record_plugin_sidebar(&sidebar_events, &sidebar_state, intent)
                    .map_err(sidebar_js_error)?;
                Ok(())
            }),
        )?;
    }
    let sidebar_events = events.clone();
    let sidebar_state_for_call = sidebar_state.clone();
    sidebar.set(
        "loadFile",
        Func::from(move |path: String| -> rquickjs::Result<()> {
            record_plugin_sidebar(
                &sidebar_events,
                &sidebar_state_for_call,
                PluginSidebarIntent::LoadFile(path.into()),
            )
            .map_err(sidebar_js_error)?;
            Ok(())
        }),
    )?;
    let sidebar_events = events.clone();
    let sidebar_state_for_call = sidebar_state.clone();
    sidebar.set(
        "postMessage",
        Func::from(move |name: String, data: String| -> rquickjs::Result<()> {
            let data = serde_json::from_str(&data).map_err(|error| {
                rquickjs::Error::new_from_js_message("string", "JSON", error.to_string())
            })?;
            record_plugin_sidebar(
                &sidebar_events,
                &sidebar_state_for_call,
                PluginSidebarIntent::PostMessage { name, data },
            )
            .map_err(sidebar_js_error)?;
            Ok(())
        }),
    )?;
    let sidebar_events = events.clone();
    let sidebar_state_for_call = sidebar_state.clone();
    sidebar.set(
        "onMessage",
        Func::from(
            move |name: String, _callback: Value| -> rquickjs::Result<()> {
                record_plugin_sidebar(
                    &sidebar_events,
                    &sidebar_state_for_call,
                    PluginSidebarIntent::OnMessage(name),
                )
                .map_err(sidebar_js_error)?;
                Ok(())
            },
        ),
    )?;
    globals.set("__riinaSidebar", sidebar)?;
    ctx.eval::<(), _>(
    r#"iina.sidebar = __riinaSidebar;
    (() => {
        const sidebar = iina.sidebar;
        const postMessage = sidebar.postMessage;
        const loadFile = sidebar.loadFile;
        const onMessage = sidebar.onMessage;
        const listeners = new Map();
        sidebar.postMessage = (name, data) => postMessage(name, JSON.stringify(data));
        sidebar.loadFile = path => { const result = loadFile(path); listeners.clear(); return result; };
        sidebar.onMessage = (name, callback) => {
            if (typeof callback !== 'function') throw new TypeError('callback must be a function');
            onMessage(name, callback);
            const callbacks = listeners.get(name) || [];
            callbacks.push(callback);
            listeners.set(name, callbacks);
        };
        sidebar.__deliverMessage = (name, data) => {
            for (const callback of listeners.get(name) || []) callback(data);
        };
    })();"#,
)?;

    Ok(())
}
