//! Playlist indices are translated to observed native IDs before leaving the plugin realm.
#[cfg(test)]
mod tests;

use super::{
    AvailablePermissions, HostEvent, core_api::plugin_media_reference,
    output::record_playlist_command,
};
use crate::plugin_files::{PluginFiles, PluginPlaylist};
use riina_core::Command;
use rquickjs::{
    Array, Ctx, Object, Value,
    function::{Func, Opt},
};
use std::{
    collections::BTreeSet,
    rc::Rc,
    sync::{Arc, Mutex},
};

const MAX_ARGUMENTS: usize = 256;
const MAX_REFERENCE_BYTES: usize = 16 * 1024;
const MAX_BATCH_BYTES: usize = 1024 * 1024;

struct PlaylistApi {
    events: Arc<Mutex<Vec<HostEvent>>>,
    files: PluginFiles,
    permissions: AvailablePermissions,
    allowed_domains: Vec<String>,
}

pub(super) fn install<'js>(
    iina: &Object<'js>,
    events: &Arc<Mutex<Vec<HostEvent>>>,
    files: &PluginFiles,
    permissions: AvailablePermissions,
    allowed_domains: &[String],
) -> rquickjs::Result<()> {
    let state = Rc::new(PlaylistApi {
        events: events.clone(),
        files: files.clone(),
        permissions,
        allowed_domains: allowed_domains.to_vec(),
    });
    let api = Object::new(iina.ctx().clone())?;
    let state_for_list = state.clone();
    api.set(
        "list",
        Func::from(move |ctx: Ctx<'js>| state_for_list.list(ctx)),
    )?;
    let state_for_count = state.clone();
    api.set(
        "count",
        Func::from(move || state_for_count.snapshot().map(|rows| rows.entries.len())),
    )?;
    let state_for_add = state.clone();
    api.set(
        "add",
        Func::from(move |paths: Opt<Value<'js>>, at: Opt<Value<'js>>| {
            paths
                .0
                .map_or(Ok(false), |paths| state_for_add.add(paths, at.0))
        }),
    )?;
    let state_for_remove = state.clone();
    api.set(
        "remove",
        Func::from(move |rows: Opt<Value<'js>>| {
            rows.0
                .map_or(Ok(false), |rows| state_for_remove.remove(rows))
        }),
    )?;
    let state_for_move = state.clone();
    api.set(
        "move",
        Func::from(move |from: Opt<Value<'js>>, to: Opt<Value<'js>>| {
            from.0.zip(to.0).map_or(Ok(false), |(from, to)| {
                state_for_move.move_entry(&from, &to)
            })
        }),
    )?;
    let state_for_play = state.clone();
    api.set(
        "play",
        Func::from(move |row: Opt<Value<'js>>| {
            row.0.map_or(Ok(()), |row| state_for_play.play(&row))
        }),
    )?;
    for (name, command) in [
        ("playNext", Command::PlaylistNext),
        ("playPrevious", Command::PlaylistPrevious),
    ] {
        let state = state.clone();
        api.set(name, Func::from(move || state.navigate(&command)))?;
    }
    iina.set("playlist", api)
}

impl PlaylistApi {
    fn snapshot(&self) -> rquickjs::Result<PluginPlaylist> {
        self.files.playlist().map_err(|error| {
            rquickjs::Error::new_from_js_message(
                "iina.playlist",
                "an available player snapshot",
                error,
            )
        })
    }

    fn list<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<Array<'js>> {
        let snapshot = self.snapshot()?;
        let result = Array::new(ctx.clone())?;
        if snapshot.active {
            for (index, row) in snapshot.entries.iter().enumerate() {
                let item = Object::new(ctx.clone())?;
                item.set("filename", row.filename.as_str())?;
                if let Some(title) = &row.title {
                    item.set("title", title.as_str())?;
                } else {
                    item.set("title", Value::new_null(ctx.clone()))?;
                }
                item.set("isPlaying", row.playing)?;
                item.set("isCurrent", row.current)?;
                result.set(index, item)?;
            }
        }
        Ok(result)
    }

    fn add(&self, paths: Value<'_>, at: Option<Value<'_>>) -> rquickjs::Result<bool> {
        let snapshot = self.snapshot()?;
        if !snapshot.active {
            return Ok(false);
        }
        let before_id = match at.filter(|value| !value.is_undefined()) {
            None => None,
            Some(value) => match value.as_number() {
                Some(at) if at.is_finite() && at.fract() == 0.0 && at < 0.0 => None,
                _ => {
                    let Some(index) = row_index(&value, snapshot.entries.len()) else {
                        return Ok(false);
                    };
                    Some(snapshot.entries[index].id)
                }
            },
        };
        let Some(paths) = arguments(paths, |value| Ok(value.into_string()))? else {
            return Ok(false);
        };
        let requested = paths.len();
        let mut accepted = Vec::new();
        let mut bytes = 0usize;
        for path in paths {
            let path = path.to_string()?;
            bytes = bytes.saturating_add(path.len());
            if bytes > MAX_BATCH_BYTES {
                return Ok(false);
            }
            if let Some(path) = self.media_reference(&path) {
                accepted.push(path);
            }
        }
        let accepted_all = accepted.len() == requested;
        if accepted.is_empty() {
            return Ok(accepted_all);
        }
        // IINA appends allowed entries from a mixed request and returns false for the rejected part.
        let queued = record_playlist_command(
            &self.events,
            Command::InsertPlaylistFilesBefore {
                paths: accepted,
                before_id,
            },
        );
        Ok(queued && accepted_all)
    }

    fn media_reference(&self, path: &str) -> Option<String> {
        if path.is_empty() || path.len() > MAX_REFERENCE_BYTES || path.contains('\0') {
            return None;
        }
        plugin_media_reference(
            path,
            &self.files,
            self.permissions.file_system,
            self.permissions.network_request,
            &self.allowed_domains,
        )
        .ok()
    }

    fn remove(&self, rows: Value<'_>) -> rquickjs::Result<bool> {
        let snapshot = self.snapshot()?;
        if !snapshot.active {
            return Ok(false);
        }
        let Some(ids) = arguments(rows, |value| {
            Ok(row_index(&value, snapshot.entries.len()).map(|index| snapshot.entries[index].id))
        })?
        else {
            return Ok(false);
        };
        let ids: BTreeSet<_> = ids.into_iter().collect();
        Ok(ids.is_empty()
            || record_playlist_command(
                &self.events,
                Command::RemovePlaylistIds(ids.into_iter().collect()),
            ))
    }

    fn move_entry(&self, from: &Value<'_>, to: &Value<'_>) -> rquickjs::Result<bool> {
        let snapshot = self.snapshot()?;
        let len = snapshot.entries.len();
        if snapshot.active
            && let (Some(from), Some(to)) = (row_index(from, len), row_index(to, len))
            && from != to
        {
            return Ok(record_playlist_command(
                &self.events,
                Command::MovePlaylistIdBefore {
                    id: snapshot.entries[from].id,
                    before_id: snapshot.entries[to].id,
                },
            ));
        }
        Ok(false)
    }

    fn play(&self, row: &Value<'_>) -> rquickjs::Result<()> {
        let snapshot = self.snapshot()?;
        if snapshot.active
            && let Some(index) = row_index(row, snapshot.entries.len())
        {
            record_playlist_command(
                &self.events,
                Command::SelectPlaylistId(snapshot.entries[index].id),
            );
        }
        Ok(())
    }

    fn navigate(&self, command: &Command) -> rquickjs::Result<()> {
        if self.snapshot()?.active {
            record_playlist_command(&self.events, command.clone());
        }
        Ok(())
    }
}

fn row_index(value: &Value<'_>, len: usize) -> Option<usize> {
    let index = value.as_number()?;
    (index >= 0.0 && index < len as f64 && index.fract() == 0.0).then_some(index as usize)
}

fn arguments<'js, T>(
    value: Value<'js>,
    decode: impl Fn(Value<'js>) -> rquickjs::Result<Option<T>>,
) -> rquickjs::Result<Option<Vec<T>>> {
    let Some(array) = value.as_array() else {
        return Ok(decode(value)?.map(|item| vec![item]));
    };
    // Arrays can have a sparse, unsigned 32-bit length. Read it fallibly and
    // bound it before retrieving elements or allocating a Rust collection.
    let length: f64 = array.clone().into_object().get("length")?;
    if !(0.0..=MAX_ARGUMENTS as f64).contains(&length) || length.fract() != 0.0 {
        return Ok(None);
    }
    let mut items = Vec::with_capacity(length as usize);
    for index in 0..length as usize {
        let Some(item) = decode(array.get(index)?)? else {
            return Ok(None);
        };
        items.push(item);
    }
    Ok(Some(items))
}
