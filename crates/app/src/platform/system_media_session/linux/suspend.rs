//! Follow logind suspend notifications independently of the desktop media endpoint.
use futures_util::StreamExt;
use tokio::sync::oneshot;
use zbus::{Connection, Proxy};

pub(super) fn start(deliver: impl Fn() + Send + 'static) -> std::io::Result<oneshot::Sender<()>> {
    let (stop, stopped) = oneshot::channel();
    crate::background::spawn_named("riina-logind", move || {
        let result = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| error.to_string())
            .and_then(|runtime| {
                runtime.block_on(async {
                    tokio::select! {
                        _ = stopped => Ok(()),
                        result = observe(deliver) => result.map_err(|error| error.to_string()),
                    }
                })
            });
        if let Err(error) = result {
            crate::diagnostics::log_args(
                crate::diagnostics::Level::Debug,
                "playback-power",
                format_args!("Could not observe Linux sleep notifications: {error}"),
            );
        }
    })?;
    Ok(stop)
}

async fn observe(deliver: impl Fn()) -> zbus::Result<()> {
    let bus = Connection::system().await?;
    let manager = Proxy::new(
        &bus,
        "org.freedesktop.login1",
        "/org/freedesktop/login1",
        "org.freedesktop.login1.Manager",
    )
    .await?;
    let mut events = manager.receive_signal("PrepareForSleep").await?;
    while let Some(event) = events.next().await {
        if matches!(event.body().deserialize::<(bool,)>(), Ok((true,))) {
            deliver();
        }
    }
    Ok(())
}
