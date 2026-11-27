//! Preserve the selected subtitle lanes across asynchronous native reloads.
use super::{Handle, WorkerState};
use crate::{MpvPropertyFormat, client_request::Operation, pending::ReplyKind};
use riina_core::{
    Command, NoticeSeverity, Track, TrackKind, key_bindings::mpv_command_prefix_len,
    media_reference::same_native_path,
};
use std::{
    collections::HashSet,
    sync::Arc,
    time::{Duration, Instant},
};

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

fn selected_lanes(catalog: &[Track]) -> [Option<&Track>; 2] {
    [0, 1].map(|lane| {
        catalog.iter().find(|track| {
            track.kind == TrackKind::Sub && track.selected && track.main_selection == Some(lane)
        })
    })
}

fn cycle_lane(args: &[String]) -> Option<usize> {
    let args = &args[mpv_command_prefix_len(args)..];
    match args {
        [verb, property, ..] if verb == "cycle" => match property.as_str() {
            "sub" | "sid" => Some(0),
            "secondary-sid" => Some(1),
            _ => None,
        },
        _ => None,
    }
}

pub(super) struct Reload {
    generation: u64,
    original: Arc<Vec<Track>>,
    selection: [Option<TrackIdentity>; 2],
    cycles: Vec<Vec<String>>,
    pending: HashSet<u64>,
    catalog_request: Option<u64>,
    reload_again: bool,
    restoring: Option<Restoration>,
}

struct Restoration {
    expected: [Option<i64>; 2],
    previous: Arc<Vec<Track>>,
    deadline: Instant,
    failed: bool,
}

impl Reload {
    fn new(generation: u64, original: Arc<Vec<Track>>) -> Self {
        let selection = selected_lanes(&original)
            .map(|track| track.map(|item| TrackIdentity::of(item, &original)));
        Self {
            generation,
            original,
            selection,
            cycles: Vec::new(),
            pending: HashSet::new(),
            catalog_request: None,
            reload_again: false,
            restoring: None,
        }
    }

    pub(super) fn replied(&mut self, id: u64, failed: bool) {
        if self.pending.remove(&id)
            && let Some(restoring) = &mut self.restoring
        {
            restoring.failed |= failed;
        }
    }

    pub(super) fn reads_catalog(&self, id: u64) -> bool {
        self.catalog_request == Some(id)
    }

    pub(super) fn deadline(&self) -> Option<Instant> {
        self.restoring.as_ref().map(|restoring| restoring.deadline)
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

    fn awaits_subtitle_selection(&self) -> bool {
        self.subtitle_reload
            .as_ref()
            .is_some_and(|reload| self.accepts_media_intent(reload.generation))
    }

    pub(super) fn retain_subtitle_cycles(&mut self, commands: &mut Vec<Vec<String>>) -> bool {
        // A mixed input binding can contain playback or media replacement.
        // Keep that plan's normal order and timing.
        let defer = self.awaits_subtitle_selection()
            && !commands.is_empty()
            && commands.iter().all(|args| cycle_lane(args).is_some());
        let Some(reload) = self.subtitle_reload.as_mut().filter(|_| defer) else {
            return false;
        };
        reload.cycles.append(commands);
        true
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
        let defer = self.awaits_subtitle_selection();
        let Some(reload) = self.subtitle_reload.as_mut().filter(|_| defer) else {
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
        reload.cycles.retain(|args| cycle_lane(args) != Some(lane));
        true
    }

    pub(super) fn advance_subtitle_reload(&mut self, handle: &Handle) {
        let Some(reload) = &mut self.subtitle_reload else {
            return;
        };
        if let Some(restoring) = reload.restoring.as_ref() {
            if restoring.failed {
                self.subtitle_reload = None;
            } else if !Arc::ptr_eq(&restoring.previous, &self.playback.tracks)
                && restoring.expected
                    == selected_lanes(&self.playback.tracks).map(|item| item.map(|track| track.id))
                && reload.pending.is_empty()
            {
                let tracks = self.playback.tracks.clone();
                if let Some(mut reload) = self.subtitle_reload.take() {
                    reload.restoring = None;
                    if self.accepts_media_intent(reload.generation)
                        && let Err(error) = self.settle_subtitle_selection(handle, reload, &tracks)
                    {
                        self.report_subtitle_reload_error(error);
                    }
                }
            } else if restoring.deadline <= Instant::now() {
                self.subtitle_reload = None;
                self.report_subtitle_reload_error(
                    "The engine did not confirm the subtitle tracks".into(),
                );
            }
            return;
        }
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
        reload.catalog_request = None;
        let result = result.map_err(|error| error.to_string()).and_then(|value| {
            let tracks =
                serde_json::from_value::<Vec<Track>>(value).map_err(|error| error.to_string())?;
            self.settle_subtitle_selection(handle, reload, &tracks)
        });
        if let Err(error) = result {
            self.report_subtitle_reload_error(error);
        }
    }

    fn settle_subtitle_selection(
        &mut self,
        handle: &Handle,
        mut reload: Reload,
        catalog: &[Track],
    ) -> Result<(), String> {
        if std::mem::take(&mut reload.reload_again)
            && catalog
                .iter()
                .any(|track| track.kind == TrackKind::Sub && track.external)
        {
            return self.submit_subtitle_reloads(handle, reload, catalog);
        }
        let desired = reload
            .selection
            .each_ref()
            .map(|choice| choice.as_ref().and_then(|track| track.resolve(catalog)));
        let observed = selected_lanes(catalog).map(|item| item.map(|track| track.id));
        if desired != observed {
            self.restore_subtitle_lanes(handle, &mut reload, observed, desired)?;
            // A set command can reply before mpv switches the underlying track.
            reload.restoring = Some(Restoration {
                expected: desired,
                previous: self.playback.tracks.clone(),
                deadline: Instant::now() + Duration::from_secs(10),
                failed: false,
            });
            self.subtitle_reload = Some(reload);
        } else {
            for args in reload.cycles {
                self.issue(handle, &args, ReplyKind::Plain)?;
            }
        }
        Ok(())
    }

    fn restore_subtitle_lanes(
        &mut self,
        handle: &Handle,
        reload: &mut Reload,
        mut observed: [Option<i64>; 2],
        desired: [Option<i64>; 2],
    ) -> Result<(), String> {
        // mpv refuses to put one track in both lanes during a swap.
        if desired[0].is_some() && desired[0] == observed[1] && observed[1] != desired[1] {
            self.issue(
                handle,
                &["set".into(), "secondary-sid".into(), "no".into()],
                ReplyKind::Plain,
            )?;
            reload.pending.insert(self.next_id);
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
                reload.pending.insert(self.next_id);
            }
        }
        Ok(())
    }

    pub(super) fn report_subtitle_reload_error(&mut self, error: String) {
        crate::record_notice(&mut self.playback, NoticeSeverity::Error, error);
        self.mark_changed();
    }
}
