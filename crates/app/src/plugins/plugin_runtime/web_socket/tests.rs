use super::super::{HostEvent, PluginRuntimeSession, UiEffect};
use std::{
    io::ErrorKind,
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener, TcpStream},
    time::{Duration, Instant},
};
use tokio_tungstenite::tungstenite::{Message, WebSocket, client};

const TIMEOUT: Duration = Duration::from_secs(3);

#[test]
fn a_network_request_grant_cannot_open_a_listener() {
    let requests = ["network-request".to_owned()];
    let session = PluginRuntimeSession::start_entry(
        r#"
        try { iina.ws.createServer({ port: 0 }); }
        catch (error) { if (String(error).includes('network-listener')) iina.log('denied'); }
        "#,
        "org.example.ws-ungranted",
        &requests,
        &requests,
    )
    .unwrap();
    assert_eq!(
        session.drain_events().unwrap(),
        [HostEvent::Ui(UiEffect::Log("denied".into()))]
    );
}

#[test]
fn realm_server_reports_states_and_exchanges_text_and_binary() {
    let reserved = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = reserved.local_addr().unwrap().port();
    drop(reserved);
    let session = start(&format!(
        r#"
        const ws = iina.ws;
        ws.sendText('unknown', 'no server').catch(error => {{
            if (error === 'server does not exist') iina.log('no server');
        }});
        ws.onStateUpdate(state => iina.log('server ' + state));
        ws.onNewConnection((id, peer) => {{
            if (id.length === 36 && peer.path.includes('127.0.0.1')) iina.log('connected');
        }});
        ws.onConnectionStateUpdate((id, state) => {{
            iina.log('connection ' + state);
            if (state === 'cancelled') ws.sendText(id, 'ignore').then(status => iina.log(status));
        }});
        ws.onMessage(async (id, message) => {{
            const text = message.text();
            const bytes = message.data();
            if (!(bytes instanceof Uint8Array)) throw new Error('Expected a typed array');
            const label = text == null ? bytes.join(',') : text;
            const first = bytes[0];
            bytes[0] ^= 255;
            if (message.data()[0] !== first) throw new Error('Message bytes changed');
            const result = await ws.sendText(id, 'echo ' + label);
            iina.log(result + ' ' + label);
        }});
        try {{ ws.createServer({{ port: 65536 }}); }} catch (_) {{ iina.log('invalid port'); }}
        ws.createServer({{ port: {port} }});
        ws.startServer();
        try {{ ws.startServer(); }} catch (_) {{ iina.log('duplicate start denied'); }}
        "#
    ));
    let mut logs = Vec::new();
    wait_for_log(&session, &mut logs, "server ready");
    wait_for_log(&session, &mut logs, "no server");
    assert!(logs.iter().any(|event| event == "duplicate start denied"));
    assert!(logs.iter().any(|event| event == "invalid port"));

    let mut client = connect(port);
    client.send(Message::Ping(vec![1, 2, 3].into())).unwrap();
    assert_eq!(client.read().unwrap(), Message::Pong(vec![1, 2, 3].into()));
    for (message, label) in [
        (Message::text("Média"), "Média"),
        (Message::binary(vec![0, 255, 65]), "0,255,65"),
    ] {
        client.send(message).unwrap();
        assert_eq!(
            client.read().unwrap(),
            Message::binary(format!("echo {label}").into_bytes())
        );
        wait_for_log(&session, &mut logs, &format!("success {label}"));
    }
    let received = logs
        .iter()
        .filter(|event| {
            matches!(
                event.as_str(),
                "connected" | "connection preparing" | "connection ready"
            )
        })
        .map(String::as_str)
        .collect::<Vec<_>>();
    assert_eq!(
        received,
        ["connected", "connection preparing", "connection ready"]
    );
    client.close(None).unwrap();
    assert!(matches!(client.read(), Ok(Message::Close(_))));
    wait_for_log(&session, &mut logs, "connection cancelled");
    wait_for_log(&session, &mut logs, "no_connection");
}

#[test]
fn replacing_the_server_reports_bind_errors_and_shuts_down_live_sockets() {
    // Reserve the wildcard endpoint the listener will bind. Winsock allows a
    // specific interface and wildcard listeners to coexist in some cases.
    let occupied = TcpListener::bind((Ipv6Addr::UNSPECIFIED, 0))
        .or_else(|_| TcpListener::bind((Ipv4Addr::UNSPECIFIED, 0)))
        .unwrap();
    let occupied_port = occupied.local_addr().unwrap().port();
    let reserved = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = reserved.local_addr().unwrap().port();
    drop(reserved);
    let session = start(&format!(
        r#"
        const ws = iina.ws;
        let recovered = false;
        let started = 0;
        ws.onStateUpdate((state, error) => {{
            if (state === 'failed') {{
                if (recovered) throw new Error(error.message);
                recovered = true;
                if (typeof error.message === 'string') iina.log('bind failed');
                ws.createServer({{ port: {port} }});
                ws.startServer();
            }} else if (state === 'ready') {{
                iina.log('ready ' + ++started);
            }} else {{ iina.log(state); }}
        }});
        ws.onMessage((id, message) => {{
            if (message.text() !== 'replace') return;
            ws.createServer({{ port: {port} }});
            ws.startServer();
        }});
        ws.createServer({{ port: {occupied_port} }});
        ws.startServer();
        "#
    ));
    let mut logs = Vec::new();
    wait_for_log(&session, &mut logs, "ready 1");
    assert!(logs.iter().any(|event| event == "bind failed"));
    assert!(logs.iter().any(|event| event == "cancelled"));
    let mut client = connect(port);
    client.send(Message::text("replace")).unwrap();
    assert_disconnected(&mut client);
    drop(client);
    wait_for_log(&session, &mut logs, "ready 2");
    let mut client = connect(port);
    drop(session);
    assert_disconnected(&mut client);
    drop(client);
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let deadline = Instant::now() + TIMEOUT;
    loop {
        match TcpStream::connect_timeout(&address, Duration::from_millis(100)) {
            Err(error)
                if matches!(
                    error.kind(),
                    ErrorKind::ConnectionRefused | ErrorKind::TimedOut
                ) =>
            {
                break;
            }
            result => assert!(
                Instant::now() < deadline,
                "Still accepting on {address}: {result:?}"
            ),
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn assert_disconnected(client: &mut WebSocket<TcpStream>) {
    let error = client.read().unwrap_err();
    assert!(match error {
        tokio_tungstenite::tungstenite::Error::Io(error) => matches!(
            error.kind(),
            ErrorKind::ConnectionReset | ErrorKind::ConnectionAborted
        ),
        tokio_tungstenite::tungstenite::Error::Protocol(_)
        | tokio_tungstenite::tungstenite::Error::ConnectionClosed => true,
        _ => false,
    });
}

fn start(source: &str) -> PluginRuntimeSession {
    let listener = ["network-listener".into()];
    PluginRuntimeSession::start_entry(source, "org.example.ws-test", &listener, &listener).unwrap()
}

fn connect(port: u16) -> WebSocket<TcpStream> {
    let socket = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
    socket.set_read_timeout(Some(TIMEOUT)).unwrap();
    socket.set_write_timeout(Some(TIMEOUT)).unwrap();
    client(format!("ws://127.0.0.1:{port}/"), socket).unwrap().0
}

fn wait_for_log(session: &PluginRuntimeSession, logs: &mut Vec<String>, expected: &str) {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        logs.extend(
            session
                .drain_events()
                .unwrap()
                .into_iter()
                .filter_map(|event| match event {
                    HostEvent::Ui(UiEffect::Log(message)) => Some(message),
                    _ => None,
                }),
        );
        if logs.iter().any(|event| event == expected) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "Missing {expected:?} in {logs:?}"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}
