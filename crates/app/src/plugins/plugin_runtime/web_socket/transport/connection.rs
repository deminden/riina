//! A socket task owns its handshake, message stream and queued sends.
use super::{ConnectionState, Event, Events, MAX_MESSAGE_BYTES, SendStatus, SendText};
use futures_util::{SinkExt, StreamExt};
use std::{net::SocketAddr, time::Duration};
use tokio::{net::TcpStream, sync::mpsc, time::timeout};
use tokio_tungstenite::{
    WebSocketStream, accept_async_with_config,
    tungstenite::{Error, Message, protocol::WebSocketConfig},
};
use uuid::Uuid;

const IO_TIMEOUT: Duration = Duration::from_secs(5);

pub(super) async fn serve(
    stream: TcpStream,
    peer: SocketAddr,
    id: Uuid,
    events: Events,
    mut requests: mpsc::Receiver<SendText>,
) -> Uuid {
    events.send(Event::Connected(id, peer)).await;
    events
        .send(Event::ConnectionState(id, ConnectionState::Preparing))
        .await;
    let result = match upgrade(stream).await {
        Ok(socket) => {
            events
                .send(Event::ConnectionState(id, ConnectionState::Ready))
                .await;
            exchange(socket, id, &events, &mut requests).await
        }
        Err(error) => Err(error),
    };
    // Close the mailbox before settling prior sends so new ones see no_connection.
    requests.close();
    while let Some(request) = requests.recv().await {
        events
            .send(Event::Sent(request.request, Ok(SendStatus::NoConnection)))
            .await;
    }
    if let Err(error) = result {
        events
            .send(Event::ConnectionState(id, ConnectionState::Failed(error)))
            .await;
    }
    events
        .send(Event::ConnectionState(id, ConnectionState::Cancelled))
        .await;
    id
}

async fn upgrade(stream: TcpStream) -> Result<WebSocketStream<TcpStream>, String> {
    let config = WebSocketConfig::default()
        .read_buffer_size(4096)
        .write_buffer_size(0)
        .max_write_buffer_size(MAX_MESSAGE_BYTES + 4096)
        .max_message_size(Some(MAX_MESSAGE_BYTES))
        .max_frame_size(Some(MAX_MESSAGE_BYTES));
    timeout(IO_TIMEOUT, accept_async_with_config(stream, Some(config)))
        .await
        .map_err(|_| "WebSocket handshake timed out".to_owned())?
        .map_err(|error| error.to_string())
}

async fn exchange(
    mut socket: WebSocketStream<TcpStream>,
    id: Uuid,
    events: &Events,
    requests: &mut mpsc::Receiver<SendText>,
) -> Result<(), String> {
    loop {
        tokio::select! {
            message = socket.next() => match message {
                Some(Ok(Message::Text(text))) => events.send(Event::Message(id, text.into())).await,
                Some(Ok(Message::Binary(bytes))) => events.send(Event::Message(id, bytes)).await,
                Some(Ok(Message::Ping(_))) => flush_control(&mut socket).await?,
                Some(Ok(Message::Close(_))) => return flush_control(&mut socket).await,
                Some(Ok(_)) => (),
                None | Some(Err(Error::ConnectionClosed)) => return Ok(()),
                Some(Err(error)) => return Err(error.to_string()),
            },
            outgoing = requests.recv() => {
                let Some(outgoing) = outgoing else { return Ok(()); };
                // Apple's server sends the UTF-8 bytes in a binary frame.
                let result = timeout(IO_TIMEOUT, socket.send(Message::Binary(outgoing.text.into())))
                    .await
                    .map_err(|_| "WebSocket send timed out".to_owned())
                    .and_then(|result| result.map_err(|error| error.to_string()));
                let sent = result.as_ref().map(|()| SendStatus::Success).map_err(Clone::clone);
                events.send(Event::Sent(outgoing.request, sent)).await;
                result?;
            }
        }
    }
}

async fn flush_control(socket: &mut WebSocketStream<TcpStream>) -> Result<(), String> {
    match timeout(IO_TIMEOUT, socket.flush()).await {
        Ok(Ok(()) | Err(Error::ConnectionClosed)) => Ok(()),
        Ok(Err(error)) => Err(error.to_string()),
        Err(_) => Err("WebSocket control reply timed out".into()),
    }
}
