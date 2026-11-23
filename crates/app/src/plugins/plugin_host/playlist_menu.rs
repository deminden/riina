//! A popup collects player-realm contributions within a single deadline.
use super::{
    PluginCommand, PluginDirectory, PluginHostHandle, PluginInstanceKey, PluginRuntimeSession,
    command_error, read_plugin_order,
};
use riina_core::plugin_menu::PluginMenuItem;
use std::{
    collections::BTreeMap,
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};

const MENU_BUILD_TIME: Duration = Duration::from_millis(150);

pub(crate) struct PlaylistMenuContribution {
    pub(crate) identifier: String,
    pub(crate) items: Vec<PluginMenuItem>,
}

pub(super) struct BuildRequest {
    request: u64,
    indices: Arc<[usize]>,
    deadline: Instant,
    ready: Box<dyn FnOnce(Vec<PlaylistMenuContribution>) + Send>,
}

impl PluginHostHandle {
    pub(crate) fn build_playlist_menu(
        &self,
        request: u64,
        indices: Vec<usize>,
        ready: impl FnOnce(Vec<PlaylistMenuContribution>) + Send + 'static,
    ) -> Result<(), String> {
        self.commands
            .try_send(PluginCommand::BuildPlaylistMenu(BuildRequest {
                request,
                indices: indices.into(),
                deadline: Instant::now() + MENU_BUILD_TIME,
                ready: Box::new(ready),
            }))
            .map_err(command_error)
    }

    pub(crate) fn activate_playlist_menu_item(
        &self,
        request: u64,
        identifier: String,
        item_id: String,
    ) -> Result<(), String> {
        self.commands
            .try_send(PluginCommand::ActivatePlaylistMenuItem {
                request,
                identifier,
                item_id,
            })
            .map_err(command_error)
    }
}

impl BuildRequest {
    pub(super) fn run(
        self,
        sessions: &BTreeMap<PluginInstanceKey, PluginRuntimeSession>,
        directories: &BTreeMap<String, PluginDirectory>,
        enabled_path: &Path,
    ) {
        if Instant::now() >= self.deadline {
            (self.ready)(Vec::new());
            return;
        }
        // Start all builders before waiting. A slow realm shares the deadline
        // with every other realm and the caller can still use completed menus.
        let responses = sessions
            .iter()
            .filter(|(key, _)| !key.global)
            .filter_map(|(key, session)| {
                session
                    .build_playlist_menu(self.request, self.indices.clone(), self.deadline)
                    .map(|receiver| (key, receiver))
            })
            .collect::<Vec<_>>();
        let mut contributions = responses
            .into_iter()
            .filter_map(|(key, receiver)| {
                let items = receiver
                    .recv_timeout(self.deadline.saturating_duration_since(Instant::now()))
                    .ok()?;
                (!items.is_empty()).then(|| PlaylistMenuContribution {
                    identifier: key.identifier.clone(),
                    items,
                })
            })
            .collect::<Vec<_>>();
        if contributions.len() > 1 {
            let order = read_plugin_order(enabled_path);
            contributions.sort_by(|left, right| {
                let by_order = |identifier: &str| {
                    order
                        .iter()
                        .position(|item| item == identifier)
                        .unwrap_or(usize::MAX)
                };
                let name = |identifier: &str| {
                    directories
                        .get(identifier)
                        .map(|directory| directory.name.as_str())
                };
                by_order(&left.identifier)
                    .cmp(&by_order(&right.identifier))
                    .then_with(|| name(&left.identifier).cmp(&name(&right.identifier)))
            });
        }
        (self.ready)(contributions);
    }
}
