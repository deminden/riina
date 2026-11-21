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
    install_overlay(
        ctx.clone(),
        &events,
        permissions.video_overlay,
        &overlay_state,
    )?;
    install_sidebar(ctx, events, sidebar_state)
}

fn install_overlay(
    ctx: Ctx<'_>,
    events: &Arc<Mutex<Vec<HostEvent>>>,
    can_use_overlay: bool,
    overlay_state: &Arc<Mutex<OverlayState>>,
) -> rquickjs::Result<()> {
    let globals = ctx.globals();
    let overlay = Object::new(ctx.clone())?;
    let overlay_events = events.clone();
    let overlay_state_for_call = overlay_state.clone();
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
    install_overlay_messages(&overlay, events, overlay_state, can_use_overlay)?;
    globals.set("__riinaOverlay", overlay)?;
    ctx.eval::<(), _>(include_str!("overlay_api.js"))?;

    Ok(())
}

fn install_overlay_messages(
    overlay: &Object<'_>,
    events: &Arc<Mutex<Vec<HostEvent>>>,
    overlay_state: &Arc<Mutex<OverlayState>>,
    can_use_overlay: bool,
) -> rquickjs::Result<()> {
    let overlay_events = events.clone();
    let overlay_state_for_call = overlay_state.clone();
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
    Ok(())
}

fn install_sidebar(
    ctx: Ctx<'_>,
    events: Arc<Mutex<Vec<HostEvent>>>,
    sidebar_state: Arc<Mutex<PluginSidebarState>>,
) -> rquickjs::Result<()> {
    let globals = ctx.globals();
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
    ctx.eval::<(), _>(include_str!("sidebar_api.js"))?;

    Ok(())
}
