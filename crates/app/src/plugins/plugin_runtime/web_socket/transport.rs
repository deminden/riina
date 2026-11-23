//! The listener owns its sockets on one I/O thread. The realm owns the listener.
mod connection;
mod listener;

use super::super::RuntimeCommand;
use std::{
    collections::HashMap,
    io,
    net::{SocketAddr, TcpListener as BoundListener},
    sync::mpsc::{SyncSender, TrySendError},
    thread::JoinHandle,
    time::Duration,
};
use tokio::{
    net::TcpListener,
    runtime::Builder,
    sync::{mpsc, oneshot},
    task::JoinSet,
};
use tokio_tungstenite::tungstenite::Bytes;
use uuid::Uuid;

pub(super) const MAX_MESSAGE_BYTES: usize = 1024 * 1024;
const MAX_CONNECTIONS: usize = 8;

pub(in crate::plugins::plugin_runtime) struct Delivery {
    pub(super) generation: u32,
    pub(super) event: Event,
}

pub(super) enum Event {
    ServerReady,
    ServerFailed(String),
    Connected(Uuid, SocketAddr),
    ConnectionState(Uuid, ConnectionState),
    Message(Uuid, Bytes),
    Sent(u32, Result<SendStatus, String>),
}

pub(super) enum ConnectionState {
    Preparing,
    Ready,
    Failed(String),
    Cancelled,
}

#[derive(Clone, Copy)]
pub(super) enum SendStatus {
    Success,
    NoConnection,
}

impl SendStatus {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::NoConnection => "no_connection",
        }
    }
}

#[derive(Clone)]
struct Events {
    generation: u32,
    commands: SyncSender<RuntimeCommand>,
}

impl Events {
    async fn send(&self, event: Event) {
        let mut command = RuntimeCommand::DeliverWebSocket(Delivery {
            generation: self.generation,
            event,
        });
        loop {
            match self.commands.try_send(command) {
                Ok(()) | Err(TrySendError::Disconnected(_)) => return,
                Err(TrySendError::Full(pending)) => command = pending,
            }
            // Pause reads for this socket while the bounded realm queue catches up.
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }
}

pub(super) struct SendText {
    pub(super) request: u32,
    pub(super) connection: Uuid,
    pub(super) text: String,
}

pub(super) struct Server {
    requests: mpsc::Sender<SendText>,
    stop: Option<oneshot::Sender<()>>,
    worker: Option<JoinHandle<Option<BoundListener>>>,
}

impl Server {
    pub(super) fn start(
        port: u16,
        bound: Option<BoundListener>,
        generation: u32,
        commands: SyncSender<RuntimeCommand>,
    ) -> io::Result<Self> {
        let (requests, inbox) = mpsc::channel(16);
        let (stop, stopped) = oneshot::channel();
        let events = Events {
            generation,
            commands,
        };
        let runtime = Builder::new_current_thread().enable_all().build()?;
        let worker = crate::background::spawn_named("plugin-websocket", move || {
            runtime.block_on(listen(port, bound, stopped, inbox, events))
        })?;
        Ok(Self {
            requests,
            stop: Some(stop),
            worker: Some(worker),
        })
    }

    pub(super) fn send(&self, request: SendText) -> Result<Option<SendStatus>, &'static str> {
        match self.requests.try_send(request) {
            Ok(()) => Ok(None),
            Err(mpsc::error::TrySendError::Closed(_)) => Ok(Some(SendStatus::NoConnection)),
            Err(mpsc::error::TrySendError::Full(_)) => Err("WebSocket send queue is full"),
        }
    }

    pub(super) fn release_listener(mut self) -> Option<BoundListener> {
        self.stop()
    }

    fn stop(&mut self) -> Option<BoundListener> {
        drop(self.stop.take());
        self.worker.take()?.join().ok().flatten()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        // Stop takes precedence over pending socket I/O and delivery backpressure.
        drop(self.stop());
    }
}

async fn listen(
    port: u16,
    bound: Option<BoundListener>,
    stopped: oneshot::Receiver<()>,
    inbox: mpsc::Receiver<SendText>,
    events: Events,
) -> Option<BoundListener> {
    let bound = bound.map_or_else(|| listener::bind(port), Ok);
    let listener = match bound.and_then(TcpListener::from_std) {
        Ok(listener) => listener,
        Err(error) => {
            tokio::select! {
                _ = stopped => (),
                () = events.send(Event::ServerFailed(error.to_string())) => (),
            }
            return None;
        }
    };
    tokio::select! {
        biased;
        _ = stopped => (),
        () = serve(&listener, inbox, events) => return None,
    }
    listener.into_std().ok()
}

async fn serve(listener: &TcpListener, mut inbox: mpsc::Receiver<SendText>, events: Events) {
    events.send(Event::ServerReady).await;
    let mut clients = HashMap::new();
    let mut connections = JoinSet::new();
    loop {
        tokio::select! {
            accepted = listener.accept(), if clients.len() < MAX_CONNECTIONS => {
                let (stream, address) = match accepted {
                    Ok(accepted) => accepted,
                    Err(error) if error.kind() == io::ErrorKind::ConnectionAborted => continue,
                    Err(error) => {
                        events.send(Event::ServerFailed(error.to_string())).await;
                        return;
                    }
                };
                let id = Uuid::new_v4();
                let (send, requests) = mpsc::channel(4);
                clients.insert(id, send);
                connections.spawn(connection::serve(stream, address, id, events.clone(), requests));
            }
            finished = connections.join_next(), if !connections.is_empty() => {
                match finished {
                    Some(Ok(id)) => { clients.remove(&id); }
                    Some(Err(error)) => {
                        events.send(Event::ServerFailed(error.to_string())).await;
                        return;
                    }
                    None => {}
                }
            }
            request = inbox.recv() => {
                let Some(request) = request else { return; };
                send_to_client(&clients, request, &events).await;
            }
        }
    }
}

async fn send_to_client(
    clients: &HashMap<Uuid, mpsc::Sender<SendText>>,
    request: SendText,
    events: &Events,
) {
    let id = request.request;
    let result = match clients
        .get(&request.connection)
        .map(|client| client.try_send(request))
    {
        Some(Ok(())) => return,
        Some(Err(mpsc::error::TrySendError::Full(_))) => {
            Err("WebSocket connection send queue is full".to_owned())
        }
        Some(Err(mpsc::error::TrySendError::Closed(_))) | None => Ok(SendStatus::NoConnection),
    };
    events.send(Event::Sent(id, result)).await;
}
