//! Own D-Bus publication on a worker independent of the player UI thread.
use super::{
    interface::{
        Endpoint, OBJECT, PLAYER, PlayerInterface, ROOT, Request, RootInterface, loop_label,
    },
    projection::{Snapshot, Status, text},
    track_list::{self, NO_TRACK, TRACK_LIST, TrackListInterface},
};
use riina_core::sync::lock;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tokio::sync::{mpsc, watch};
use zbus::{
    Connection, ObjectPath, OwnedVariant, connection::Builder, object_server::SignalEmitter,
};

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
    let connection = Builder::session()
        .serve_at(OBJECT, RootInterface(endpoint.clone()))
        .serve_at(OBJECT, PlayerInterface(endpoint.clone()))
        .serve_at(OBJECT, TrackListInterface(endpoint))
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
    if snapshot.media.is_none() && snapshot.loading.is_none() {
        if published.take().is_some() {
            connection.release_name(name).await?;
        }
        return Ok(());
    }
    if published.is_none() && snapshot.media.is_none() {
        return Ok(());
    }
    let Some(prior) = published.replace(snapshot.clone()) else {
        connection.request_name(name).await?;
        return Ok(());
    };
    let changed = properties_changed(&snapshot, &prior)?;
    emit_properties(connection, PLAYER, changed, &[]).await?;
    if prior.fullscreen != snapshot.fullscreen {
        emit_properties(
            connection,
            ROOT,
            HashMap::from([("Fullscreen", snapshot.fullscreen.into())]),
            &[],
        )
        .await?;
    }
    publish_queue(connection, &snapshot, &prior).await?;
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

async fn publish_queue(
    connection: &Connection,
    snapshot: &Snapshot,
    prior: &Snapshot,
) -> zbus::Result<()> {
    if snapshot.queue == prior.queue && snapshot.media == prior.media {
        return Ok(());
    }
    let current = snapshot.media.as_ref().and_then(|media| media.track_id());
    let previous = prior.media.as_ref().and_then(|media| media.track_id());
    let tracks_changed = !snapshot.queue.same_tracks(&prior.queue);
    let emitter = SignalEmitter::new(connection, OBJECT)?;
    if tracks_changed {
        emit_properties(connection, TRACK_LIST, HashMap::new(), &["Tracks"]).await?;
    }
    if tracks_changed || current != previous {
        let current = current.unwrap_or(ObjectPath::try_from(NO_TRACK)?);
        return TrackListInterface::track_list_replaced(&emitter, snapshot.queue.tracks(), current)
            .await;
    }
    for (entry, old) in snapshot.queue.entries().iter().zip(prior.queue.entries()) {
        let path = snapshot.queue.path(entry.id)?;
        let changed = if current.as_ref() == Some(&path) {
            (snapshot.media != prior.media).then(|| snapshot.metadata())
        } else {
            (track_list::title(entry) != track_list::title(old))
                .then(|| track_list::metadata(path.clone(), entry))
        };
        if let Some(changed) = changed {
            TrackListInterface::track_metadata_changed(&emitter, path, changed).await?;
        }
    }
    Ok(())
}

fn properties_changed(
    snapshot: &Snapshot,
    prior: &Snapshot,
) -> zbus::Result<HashMap<&'static str, OwnedVariant>> {
    let mut changed = HashMap::new();
    if snapshot.media != prior.media {
        changed.insert(
            "Metadata",
            OwnedVariant::try_from(zbus::Variant::from(snapshot.metadata()))?,
        );
    }
    if snapshot.status != prior.status {
        changed.insert("PlaybackStatus", text(snapshot.status.as_str()));
        changed.insert("CanPause", (snapshot.status != Status::Stopped).into());
    }
    if snapshot.media.is_some() != prior.media.is_some() {
        changed.insert("CanPlay", snapshot.media.is_some().into());
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
    changed: HashMap<&'static str, OwnedVariant>,
    invalidated: &[&str],
) -> zbus::Result<()> {
    if changed.is_empty() && invalidated.is_empty() {
        return Ok(());
    }
    connection
        .emit_signal(
            None::<&str>,
            OBJECT,
            "org.freedesktop.DBus.Properties",
            "PropertiesChanged",
            &(interface, changed, invalidated),
        )
        .await
}
