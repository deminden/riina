//! Own D-Bus publication on a worker independent of the player UI thread.
use super::{
    interface::{
        Endpoint, OBJECT, PLAYER, PlayerInterface, ROOT, Request, RootInterface, loop_label,
    },
    projection::{Snapshot, Status, text},
};
use riina_core::sync::lock;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tokio::sync::{mpsc, watch};
use zbus::{Connection, connection::Builder, zvariant::OwnedValue};

pub(super) fn start(
    id: u64,
    mut snapshots: watch::Receiver<Snapshot>,
    deliver: impl Fn(Request) + Send + 'static,
) -> std::io::Result<()> {
    crate::background::spawn_named("riina-mpris", move || {
        let result = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| error.to_string())
            .and_then(|runtime| {
                runtime
                    .block_on(serve(id, &mut snapshots, deliver))
                    .map_err(|error| error.to_string())
            });
        if let Err(error) = result {
            crate::diagnostics::warn!("Could not publish Linux system media controls: {error}");
        }
    })
    .map(|_| ())
}

async fn serve(
    id: u64,
    snapshots: &mut watch::Receiver<Snapshot>,
    deliver: impl Fn(Request),
) -> zbus::Result<()> {
    let (requests, mut pending) = mpsc::channel(32);
    let current = Arc::new(Mutex::new(Snapshot::default()));
    let endpoint = Endpoint::new(current.clone(), requests);
    let connection = Builder::session()?
        .serve_at(OBJECT, RootInterface(endpoint.clone()))?
        .serve_at(OBJECT, PlayerInterface(endpoint))?
        .build()
        .await?;
    let name = format!(
        "org.mpris.MediaPlayer2.riina.instance{}_{id}",
        std::process::id()
    );
    let mut published = None;
    loop {
        let snapshot = snapshots.borrow_and_update().clone();
        if published.as_ref() != Some(&snapshot) {
            *lock(&current) = snapshot.clone();
            publish(&connection, &name, &mut published, snapshot).await?;
        }
        tokio::select! {
            valid = snapshots.changed() => {
                if valid.is_err() {
                    if published.is_some() { let _ = connection.release_name(name.as_str()).await; }
                    return Ok(());
                }
            }
            request = pending.recv() => {
                if let Some(request) = request { deliver(request); }
            }
        }
    }
}

async fn publish(
    connection: &Connection,
    name: &str,
    published: &mut Option<Snapshot>,
    snapshot: Snapshot,
) -> zbus::Result<()> {
    if snapshot.media.is_none() {
        if published.take().is_some() {
            connection.release_name(name).await?;
        }
        return Ok(());
    }
    let Some(prior) = published.replace(snapshot.clone()) else {
        connection.request_name(name).await?;
        return Ok(());
    };
    let changed = properties_changed(&snapshot, &prior)?;
    emit_properties(connection, PLAYER, changed).await?;
    if prior.fullscreen != snapshot.fullscreen {
        emit_properties(
            connection,
            ROOT,
            HashMap::from([("Fullscreen", snapshot.fullscreen.into())]),
        )
        .await?;
    }
    if snapshot.seeked_from(&prior) {
        connection
            .emit_signal(
                None::<&str>,
                OBJECT,
                PLAYER,
                "Seeked",
                &(snapshot.position,),
            )
            .await?;
    }
    Ok(())
}

fn properties_changed(
    snapshot: &Snapshot,
    prior: &Snapshot,
) -> zbus::Result<HashMap<&'static str, OwnedValue>> {
    let mut changed = HashMap::new();
    if snapshot.media != prior.media {
        changed.insert(
            "Metadata",
            OwnedValue::try_from(zbus::zvariant::Value::from(snapshot.metadata()))?,
        );
    }
    if snapshot.status != prior.status {
        changed.insert("PlaybackStatus", text(snapshot.status.as_str()));
        changed.insert("CanPause", (snapshot.status != Status::Stopped).into());
    }
    if snapshot.loop_status != prior.loop_status {
        changed.insert("LoopStatus", text(loop_label(snapshot.loop_status)));
    }
    if snapshot.rate != prior.rate {
        changed.insert("Rate", snapshot.rate.into());
    }
    if snapshot.volume != prior.volume {
        changed.insert("Volume", snapshot.volume.into());
    }
    if snapshot.shuffled != prior.shuffled {
        changed.insert("Shuffle", snapshot.shuffled.into());
    }
    if snapshot.next != prior.next {
        changed.insert("CanGoNext", snapshot.next.into());
    }
    if snapshot.previous != prior.previous {
        changed.insert("CanGoPrevious", snapshot.previous.into());
    }
    if snapshot.seekable != prior.seekable {
        changed.insert("CanSeek", snapshot.seekable.into());
    }
    Ok(changed)
}

async fn emit_properties(
    connection: &Connection,
    interface: &str,
    changed: HashMap<&'static str, OwnedValue>,
) -> zbus::Result<()> {
    if changed.is_empty() {
        return Ok(());
    }
    connection
        .emit_signal(
            None::<&str>,
            OBJECT,
            "org.freedesktop.DBus.Properties",
            "PropertiesChanged",
            &(interface, changed, Vec::<&str>::new()),
        )
        .await
}
