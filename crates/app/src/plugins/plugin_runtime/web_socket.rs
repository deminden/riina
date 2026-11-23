//! iina.ws methods and callback values stay on the plugin's QuickJS worker.
#[cfg(test)]
mod tests;
mod transport;

use super::RuntimeCommand;
use rquickjs::{ArrayBuffer, Context, Ctx, Function, Object, function::Func};
use std::{cell::RefCell, net::TcpListener, rc::Rc, sync::mpsc::SyncSender};
pub(super) use transport::Delivery;
use transport::{ConnectionState, Event, MAX_MESSAGE_BYTES, SendStatus, SendText, Server};
use uuid::Uuid;

#[derive(Default)]
struct Listener {
    generation: u32,
    port: Option<u16>,
    running: Option<Server>,
    bound: Option<TcpListener>,
}

#[derive(Default)]
pub(super) struct RealmWebSocket(Rc<RefCell<Listener>>);

impl RealmWebSocket {
    pub(super) fn install<'js>(
        &self,
        api: &Object<'js>,
        permitted: bool,
        commands: SyncSender<RuntimeCommand>,
    ) -> rquickjs::Result<()> {
        let ctx = api.ctx();
        let native = Object::new(ctx.clone())?;
        let listener = Rc::downgrade(&self.0);
        native.set(
            "create",
            Func::from(move |port: Option<u16>| {
                if !permitted {
                    return Err(js_error("The network-listener permission is required"));
                }
                let listener = listener.upgrade().ok_or_else(closed)?;
                Ok(listener.borrow_mut().configure(port))
            }),
        )?;
        let listener = Rc::downgrade(&self.0);
        native.set(
            "start",
            Func::from(move || {
                let listener = listener.upgrade().ok_or_else(closed)?;
                let mut listener = listener.borrow_mut();
                let port = listener
                    .port
                    .ok_or_else(|| js_error("WebSocket server has not been created"))?;
                if listener.running.is_some() {
                    return Err(js_error("WebSocket server has already started"));
                }
                let bound = listener.bound.take();
                let server = Server::start(port, bound, listener.generation, commands.clone())
                    .map_err(|error| js_error(error.to_string()))?;
                listener.running = Some(server);
                Ok(())
            }),
        )?;
        let listener = Rc::downgrade(&self.0);
        native.set(
            "send",
            Func::from(move |request: u32, id: String, text: String| {
                let listener = listener.upgrade().ok_or_else(closed)?;
                listener.borrow().send(request, &id, text)
            }),
        )?;
        native.set(
            "text",
            Func::from(move |buffer: ArrayBuffer<'js>| {
                // SAFETY: the callback copies the text before returning to
                // JavaScript, with no script execution while borrowing bytes.
                let bytes = unsafe { buffer.as_bytes()? };
                std::str::from_utf8(bytes).ok().map(str::to_owned)
            }),
        )?;
        let install: Function<'_> = ctx.eval(include_str!("web_socket.js"))?;
        let dispatch: Object<'_> = install.call((api.clone(), native))?;
        ctx.globals().set("__riinaWebSocket", dispatch)
    }

    pub(super) fn dispatch(&self, context: &Context, delivery: Delivery) -> Result<(), String> {
        if self.0.borrow().generation != delivery.generation {
            return Ok(());
        }
        context.with(|ctx| {
            dispatch_event(&ctx, delivery.event).map_err(|error| {
                if error.is_exception() {
                    ctx.catch();
                }
                error.to_string()
            })
        })
    }
}

impl Listener {
    fn configure(&mut self, port: Option<u16>) -> bool {
        let running = self.running.take();
        let replaced = running.is_some();
        self.bound = if port == self.port && port.is_some_and(|port| port != 0) {
            // Keep the listening endpoint while old client tasks close. The
            // socket cannot bind afresh during some TCP shutdown states.
            running
                .and_then(Server::release_listener)
                .or(self.bound.take())
        } else {
            drop(running);
            None
        };
        self.generation = self.generation.wrapping_add(1);
        self.port = port;
        replaced
    }

    fn send(&self, request: u32, id: &str, text: String) -> rquickjs::Result<Option<&'static str>> {
        if self.port.is_none() {
            return Err(js_error("server does not exist"));
        }
        let (Some(server), Ok(connection)) = (&self.running, Uuid::parse_str(id)) else {
            return Ok(Some(SendStatus::NoConnection.as_str()));
        };
        if text.len() > MAX_MESSAGE_BYTES {
            return Err(js_error("WebSocket message exceeds the 1 MiB limit"));
        }
        server
            .send(SendText {
                request,
                connection,
                text,
            })
            .map(|status| status.map(SendStatus::as_str))
            .map_err(js_error)
    }
}

fn dispatch_event<'js>(ctx: &Ctx<'js>, event: Event) -> rquickjs::Result<()> {
    let dispatch: Object<'_> = ctx.globals().get("__riinaWebSocket")?;
    match event {
        Event::ServerReady => dispatch.get::<_, Function<'_>>("server")?.call(("ready",)),
        Event::ServerFailed(error) => dispatch
            .get::<_, Function<'_>>("server")?
            .call(("failed", error_info(ctx, &error)?)),
        Event::Connected(id, remote) => {
            let peer = Object::new(ctx.clone())?;
            peer.set("path", remote.to_string())?;
            dispatch
                .get::<_, Function<'_>>("connected")?
                .call((id.to_string().to_ascii_uppercase(), peer))
        }
        Event::ConnectionState(id, state) => {
            let callback: Function<'_> = dispatch.get("connection")?;
            let id = id.to_string().to_ascii_uppercase();
            match state {
                ConnectionState::Preparing => callback.call((id, "preparing")),
                ConnectionState::Ready => callback.call((id, "ready")),
                ConnectionState::Failed(error) => {
                    callback.call((id, "failed", error_info(ctx, &error)?))
                }
                ConnectionState::Cancelled => callback.call((id, "cancelled")),
            }
        }
        Event::Message(id, bytes) => {
            // The realm's allocator accounts for messages the plugin retains.
            let bytes = ArrayBuffer::new_copy(ctx.clone(), bytes.as_ref())?;
            dispatch
                .get::<_, Function<'_>>("message")?
                .call((id.to_string().to_ascii_uppercase(), bytes))
        }
        Event::Sent(request, status) => {
            let callback: Function<'_> = dispatch.get("sent")?;
            match status {
                Ok(status) => callback.call((request, true, status.as_str())),
                Err(error) => callback.call((request, false, error_info(ctx, &error)?)),
            }
        }
    }
}

fn error_info<'js>(ctx: &Ctx<'js>, error: &str) -> rquickjs::Result<Object<'js>> {
    let result = Object::new(ctx.clone())?;
    result.set("description", error)?;
    result.set("message", error)?;
    Ok(result)
}

fn closed() -> rquickjs::Error {
    js_error("The plugin has closed")
}

fn js_error(message: impl Into<String>) -> rquickjs::Error {
    rquickjs::Error::new_from_js_message("iina.ws", "WebSocket operation", message.into())
}
