//! Preserve the selected subtitle lanes across asynchronous native reloads.
use super::{Handle, WorkerState};
use crate::{MpvPropertyFormat, client_request::Operation, pending::ReplyKind};
use riina_core::{Command, NoticeSeverity, Track, TrackKind, media_reference::same_native_path};
use std::{collections::HashSet, sync::Arc};

enum TrackIdentity {
    Embedded(i64),
    External { filename: String, ordinal: usize },
}

impl TrackIdentity {
    fn of(track: &Track, catalog: &[Track]) -> Self {
        if let Some(filename) = &track.external_filename {
            let ordinal = catalog
                .iter()
                .filter(|item| same_file(item, filename))
                .take_while(|item| item.id != track.id)
                .count();
            Self::External {
                filename: filename.clone(),
                ordinal,
            }
        } else {
            Self::Embedded(track.id)
        }
    }

    fn resolve(&self, catalog: &[Track]) -> Option<i64> {
        match self {
            Self::Embedded(id) => catalog
                .iter()
                .find(|track| track.kind == TrackKind::Sub && !track.external && track.id == *id),
            Self::External { filename, ordinal } => catalog
                .iter()
                .filter(|track| same_file(track, filename))
                .nth(*ordinal),
        }
        .map(|track| track.id)
    }
}

fn same_file(track: &Track, filename: &str) -> bool {
    track.kind == TrackKind::Sub
        && track
            .external_filename
            .as_ref()
            .is_some_and(|path| same_native_path(path, filename))
}

pub(super) struct Reload {
    generation: u64,
    original: Arc<Vec<Track>>,
    selection: [Option<TrackIdentity>; 2],
    pending: HashSet<u64>,
    catalog_request: Option<u64>,
    reload_again: bool,
}

impl Reload {
    fn new(generation: u64, original: Arc<Vec<Track>>) -> Self {
        let selection = [0, 1].map(|lane| {
            original
                .iter()
                .find(|track| {
                    track.kind == TrackKind::Sub
                        && track.selected
                        && track.main_selection == Some(lane)
                })
                .map(|track| TrackIdentity::of(track, &original))
        });
        Self {
            generation,
            original,
            selection,
            pending: HashSet::new(),
            catalog_request: None,
            reload_again: false,
        }
    }

    pub(super) fn replied(&mut self, id: u64) {
        self.pending.remove(&id);
    }

    pub(super) fn reads_catalog(&self, id: u64) -> bool {
        self.catalog_request == Some(id)
    }
}

impl WorkerState {
    pub(super) fn change_subtitle_encoding(
        &mut self,
        handle: &Handle,
        command: &Command,
    ) -> Result<(), String> {
        let plan = command
            .plan(&self.playback)
            .map_err(|error| error.to_string())?;
        for args in plan.commands {
            self.issue(handle, &args, ReplyKind::Plain)?;
        }
        if !self.accepts_media_intent(self.playback.generation) {
            return Ok(());
        }
        if let Some(reload) = &mut self.subtitle_reload {
            reload.reload_again = true;
            return Ok(());
        }
        let tracks = self.playback.tracks.clone();
        let reload = Reload::new(self.playback.generation, tracks.clone());
        self.submit_subtitle_reloads(handle, reload, &tracks)?;
        Ok(())
    }

    fn submit_subtitle_reloads(
        &mut self,
        handle: &Handle,
        mut reload: Reload,
        catalog: &[Track],
    ) -> Result<(), String> {
        for track in catalog
            .iter()
            .filter(|track| track.kind == TrackKind::Sub && track.external)
        {
            let result = self.issue(
                handle,
                &["sub-reload".into(), track.id.to_string()],
                ReplyKind::Plain,
            );
            if let Err(error) = result {
                // Submissions already accepted still need to settle.
                if !reload.pending.is_empty() {
                    self.subtitle_reload = Some(reload);
                }
                return Err(error);
            }
            reload.pending.insert(self.next_id);
        }
        if !reload.pending.is_empty() {
            self.subtitle_reload = Some(reload);
        }
        Ok(())
    }

    pub(super) fn retain_subtitle_selection(&mut self, command: &Command) -> bool {
        let (lane, id) = match command {
            Command::SelectTrack {
                kind: TrackKind::Sub,
                id,
            } => (0, id),
            Command::SelectSecondarySubtitle(id) => (1, id),
            _ => return false,
        };
        if !self.accepts_media_intent(self.playback.generation) {
            return false;
        }
        let Some(reload) = &mut self.subtitle_reload else {
            return false;
        };
        let identity = if let Some(id) = id {
            let find = |catalog: &[Track]| {
                catalog
                    .iter()
                    .find(|track| track.kind == TrackKind::Sub && track.id == *id)
                    .map(|track| TrackIdentity::of(track, catalog))
            };
            let Some(identity) = find(&self.playback.tracks).or_else(|| find(&reload.original))
            else {
                return false;
            };
            Some(identity)
        } else {
            None
        };
        reload.selection[lane] = identity;
        true
    }

    pub(super) fn advance_subtitle_reload(&mut self, handle: &Handle) {
        let Some(reload) = &mut self.subtitle_reload else {
            return;
        };
        if !reload.pending.is_empty() || reload.catalog_request.is_some() {
            return;
        }
        let read = Operation::Read {
            name: c"track-list".into(),
            format: MpvPropertyFormat::Node,
        };
        self.next_id += 1;
        match handle.request_property(self.next_id, &read) {
            Ok(()) => reload.catalog_request = Some(self.next_id),
            Err(error) => {
                self.subtitle_reload = None;
                self.report_subtitle_reload_error(error.to_string());
            }
        }
    }

    pub(super) fn finish_subtitle_reload(
        &mut self,
        handle: &Handle,
        result: Result<serde_json::Value, crate::EngineError>,
    ) {
        let Some(mut reload) = self.subtitle_reload.take() else {
            return;
        };
        if !self.accepts_media_intent(reload.generation) {
            return;
        }
        let result = result.map_err(|error| error.to_string()).and_then(|value| {
            let tracks =
                serde_json::from_value::<Vec<Track>>(value).map_err(|error| error.to_string())?;
            if std::mem::take(&mut reload.reload_again)
                && tracks
                    .iter()
                    .any(|track| track.kind == TrackKind::Sub && track.external)
            {
                reload.catalog_request = None;
                return self.submit_subtitle_reloads(handle, reload, &tracks);
            }
            self.restore_subtitle_lanes(handle, &tracks, &reload.selection)
        });
        if let Err(error) = result {
            self.report_subtitle_reload_error(error);
        }
    }

    fn restore_subtitle_lanes(
        &mut self,
        handle: &Handle,
        catalog: &[Track],
        selection: &[Option<TrackIdentity>; 2],
    ) -> Result<(), String> {
        let desired = selection
            .each_ref()
            .map(|choice| choice.as_ref().and_then(|track| track.resolve(catalog)));
        let mut observed = [0, 1].map(|lane| {
            catalog
                .iter()
                .find(|track| {
                    track.kind == TrackKind::Sub
                        && track.selected
                        && track.main_selection == Some(lane)
                })
                .map(|track| track.id)
        });
        // mpv refuses to put one track in both lanes during a swap.
        if desired[0].is_some() && desired[0] == observed[1] && observed[1] != desired[1] {
            self.issue(
                handle,
                &["set".into(), "secondary-sid".into(), "no".into()],
                ReplyKind::Plain,
            )?;
            observed[1] = None;
        }
        for (lane, property) in ["sid", "secondary-sid"].into_iter().enumerate() {
            if desired[lane] != observed[lane] {
                let id = desired[lane].map_or_else(|| "no".into(), |id| id.to_string());
                self.issue(
                    handle,
                    &["set".into(), property.into(), id],
                    ReplyKind::Plain,
                )?;
            }
        }
        Ok(())
    }

    pub(super) fn report_subtitle_reload_error(&mut self, error: String) {
        crate::record_notice(&mut self.playback, NoticeSeverity::Error, error);
        self.mark_changed();
    }
}
