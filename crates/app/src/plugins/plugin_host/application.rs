//! The application owns global realms and coordinates package changes across players.
use super::{
    HOST_EVENT_POLL_INTERVAL, PluginCommand, PluginHostMessage, UiActivity, enabled_packages,
    host_message, load_grants, record,
};
use crate::plugin_runtime::{HostEvent, PluginRuntimeSession, PluginUi};
use riina_core::{plugin::Manifest, plugin_menu::PluginMenuItem, sync::lock};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

const CHANGE_TIMEOUT: Duration = Duration::from_secs(30);
type Acknowledgement = mpsc::Receiver<Result<(), String>>;

pub(crate) struct GlobalHost {
    handle: GlobalHandle,
    worker: Option<JoinHandle<()>>,
}

#[derive(Clone)]
pub(crate) struct GlobalHandle {
    pub(crate) ui: PluginUi,
    commands: mpsc::SyncSender<Request>,
}

pub(super) struct PlayerPort {
    pub(super) commands: mpsc::SyncSender<PluginCommand>,
    pub(super) messages: Arc<Mutex<Vec<PluginHostMessage>>>,
    pub(super) activity: Arc<UiActivity>,
    pub(super) closed: AtomicBool,
}

#[derive(Clone)]
pub(super) enum PackageChange {
    Enable(Arc<Manifest>, PathBuf),
    Disable(String),
    Suspend(String),
    Reload(BTreeSet<String>),
}

enum Request {
    Join(Weak<PlayerPort>),
    Ensure(Arc<Manifest>, PathBuf, mpsc::SyncSender<Result<(), String>>),
    Change(PackageChange, Option<mpsc::SyncSender<Schedule>>),
    ActivateMenu(String, String),
    FromPlayer(String, String, serde_json::Value),
    Shutdown,
}

struct Schedule {
    result: Result<(), String>,
    acknowledgements: Vec<Acknowledgement>,
}

impl GlobalHost {
    pub(crate) fn start(preferences_path: &Path) -> Result<Self, String> {
        let (commands, requests) = mpsc::sync_channel(64);
        let ui = PluginUi::default();
        let worker_ui = ui.clone();
        let profile = preferences_path.to_path_buf();
        let worker = crate::background::spawn_named("riina-application-plugins", move || {
            Coordinator::new(&profile, worker_ui).run(requests);
        })
        .map_err(|error| format!("Could not start global plugin host: {error}"))?;
        Ok(Self {
            handle: GlobalHandle { ui, commands },
            worker: Some(worker),
        })
    }

    pub(crate) fn handle(&self) -> GlobalHandle {
        self.handle.clone()
    }
}

impl Drop for GlobalHost {
    fn drop(&mut self) {
        self.handle.ui.dialogs.stop();
        let _ = self.handle.commands.send(Request::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl GlobalHandle {
    fn send(&self, request: Request) -> Result<(), String> {
        self.commands
            .try_send(request)
            .map_err(|error| match error {
                mpsc::TrySendError::Full(_) => "Global plugin host is busy".into(),
                mpsc::TrySendError::Disconnected(_) => "Global plugin host has stopped".into(),
            })
    }

    pub(super) fn join(&self, port: &Arc<PlayerPort>) -> Result<(), String> {
        self.send(Request::Join(Arc::downgrade(port)))
    }

    pub(super) fn ensure(
        &self,
        manifest: &Arc<Manifest>,
        directory: &Path,
        port: &PlayerPort,
    ) -> Result<(), String> {
        let (sender, receiver) = mpsc::sync_channel(1);
        self.send(Request::Ensure(
            manifest.clone(),
            directory.to_owned(),
            sender,
        ))?;
        // Closing one player must not wait for an application-level startup dialog.
        while !port.closed.load(Ordering::Acquire) {
            match receiver.recv_timeout(HOST_EVENT_POLL_INTERVAL) {
                Ok(result) => return result,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        Err("Plugin startup was cancelled".into())
    }

    pub(super) fn change(&self, change: PackageChange) -> Result<(), String> {
        self.send(Request::Change(change, None))
    }

    pub(super) fn change_and_wait(&self, change: PackageChange) -> Result<(), String> {
        let (sender, receiver) = mpsc::sync_channel(1);
        let deadline = Instant::now() + CHANGE_TIMEOUT;
        self.send(Request::Change(change, Some(sender)))?;
        let scheduled = receiver
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .map_err(|error| format!("Global plugin change was not scheduled: {error}"))?;
        let mut result = scheduled.result;
        // The caller waits for each player independently of the coordinator, so
        // a starting player can still receive its global entry acknowledgement.
        for receiver in scheduled.acknowledgements {
            let ack =
                match receiver.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                    Ok(ack) => ack,
                    // A closing player has already released the package.
                    Err(mpsc::RecvTimeoutError::Disconnected) => Ok(()),
                    Err(error) => Err(format!("Player did not finish the plugin change: {error}")),
                };
            result = result.and(ack);
        }
        result
    }

    pub(super) fn activate_menu(&self, identifier: String, id: String) -> Result<(), String> {
        self.send(Request::ActivateMenu(identifier, id))
    }

    pub(super) fn deliver(
        &self,
        identifier: String,
        name: String,
        data: serde_json::Value,
    ) -> Result<(), String> {
        self.send(Request::FromPlayer(identifier, name, data))
    }
}

struct Entry {
    session: PluginRuntimeSession,
    name: String,
    menu: Option<Vec<PluginMenuItem>>,
}

struct Coordinator {
    directory: PathBuf,
    permissions: PathBuf,
    enabled: PathBuf,
    ui: PluginUi,
    entries: BTreeMap<String, Entry>,
    suspended: BTreeSet<String>,
    players: Vec<Weak<PlayerPort>>,
}

impl Coordinator {
    fn new(profile: &Path, ui: PluginUi) -> Self {
        let directory = profile.with_file_name("Plugins");
        crate::plugin_packages::sweep_staging(&directory);
        Self {
            directory,
            permissions: profile.with_file_name("plugin-permissions.json"),
            enabled: profile.with_file_name("plugin-enabled.json"),
            ui,
            entries: BTreeMap::new(),
            suspended: BTreeSet::new(),
            players: Vec::new(),
        }
    }

    fn run(mut self, requests: mpsc::Receiver<Request>) {
        loop {
            let request = if self.entries.is_empty() {
                requests
                    .recv()
                    .map_err(|_| mpsc::RecvTimeoutError::Disconnected)
            } else {
                requests.recv_timeout(HOST_EVENT_POLL_INTERVAL)
            };
            match request {
                Ok(Request::Shutdown) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Ok(request) => self.handle(request),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
            let players = self.live_players();
            self.pump(&players);
        }
    }

    fn handle(&mut self, request: Request) {
        match request {
            Request::Join(player) => {
                if let Some(port) = player.upgrade() {
                    for (identifier, entry) in &self.entries {
                        if let Some(items) = &entry.menu {
                            port.menu(menu_update(identifier, &entry.name, items.clone()));
                        }
                    }
                    self.players.push(player);
                }
            }
            Request::Ensure(manifest, root, response) => {
                let _ = response.try_send(self.ensure(&manifest, &root));
            }
            Request::Change(change, response) => self.change(change, response),
            Request::ActivateMenu(identifier, id) => {
                if let Some(entry) = self.entries.get(&identifier)
                    && let Err(error) = entry.session.deliver_plugin_menu_action(id)
                {
                    self.report(error);
                }
            }
            Request::FromPlayer(identifier, name, data) => {
                if let Some(entry) = self.entries.get(&identifier)
                    && let Err(error) = entry.session.deliver_plugin_message(name, data)
                {
                    self.report(error);
                }
            }
            Request::Shutdown => {}
        }
    }

    fn ensure(&mut self, manifest: &Manifest, root: &Path) -> Result<(), String> {
        if self.suspended.contains(&manifest.identifier) {
            return Err(format!("{} is being updated", manifest.name));
        }
        if manifest.global_entry.is_none() || self.entries.contains_key(&manifest.identifier) {
            return Ok(());
        }
        if !crate::plugin_preferences::current_runtime_supports(manifest) {
            return Err(format!(
                "{} requests unavailable plugin capabilities",
                manifest.name
            ));
        }
        let grants = load_grants(&self.permissions, &manifest.identifier)?;
        let session = PluginRuntimeSession::start_package_global_entry(
            root,
            manifest,
            &grants,
            self.ui.clone(),
        )
        .map_err(|error| format!("Could not start global {}: {error}", manifest.name))?;
        self.entries.insert(
            manifest.identifier.clone(),
            Entry {
                session,
                name: manifest.name.clone(),
                menu: None,
            },
        );
        Ok(())
    }

    fn change(&mut self, mut change: PackageChange, response: Option<mpsc::SyncSender<Schedule>>) {
        let players = self.live_players();
        if let PackageChange::Reload(exclusions) = &mut change {
            exclusions.clone_from(&self.suspended);
        }
        let mut schedule = Schedule {
            result: self.apply_change(&change),
            acknowledgements: Vec::new(),
        };
        if schedule.result.is_ok() {
            let change = Arc::new(change);
            for port in players {
                let (reply, ack) = mpsc::sync_channel(1);
                let reply = response.as_ref().map(|_| reply);
                if let Err(error) = port
                    .commands
                    .try_send(PluginCommand::Change(change.clone(), reply))
                {
                    if matches!(error, mpsc::TrySendError::Full(_)) {
                        let message = "A player plugin host is busy".to_owned();
                        port.publish(PluginHostMessage::Status(message.clone()));
                        schedule.result = Err(message);
                    }
                } else if response.is_some() {
                    schedule.acknowledgements.push(ack);
                }
            }
        } else if let Err(error) = &schedule.result {
            self.report(error.clone());
        }
        if let Some(response) = response {
            let _ = response.try_send(schedule);
        }
    }

    fn apply_change(&mut self, change: &PackageChange) -> Result<(), String> {
        match change {
            PackageChange::Enable(manifest, root) => {
                self.suspended.remove(&manifest.identifier);
                self.ensure(manifest, root)
            }
            PackageChange::Disable(identifier) => {
                self.suspended.remove(identifier);
                self.entries.remove(identifier);
                Ok(())
            }
            PackageChange::Suspend(identifier) => {
                self.suspended.insert(identifier.clone());
                self.entries.remove(identifier);
                Ok(())
            }
            PackageChange::Reload(_) => {
                self.entries.clear();
                for plugin in enabled_packages(&self.directory, &self.enabled) {
                    if !self.suspended.contains(&plugin.manifest.identifier)
                        && let Err(error) = self.ensure(&plugin.manifest, &plugin.directory)
                    {
                        self.report(error);
                    }
                }
                Ok(())
            }
        }
    }

    fn live_players(&mut self) -> Vec<Arc<PlayerPort>> {
        let mut live = Vec::new();
        self.players.retain(|player| {
            let Some(player) = player
                .upgrade()
                .filter(|port| !port.closed.load(Ordering::Acquire))
            else {
                return false;
            };
            live.push(player);
            true
        });
        live
    }

    fn report(&mut self, message: String) {
        for port in self.live_players() {
            port.publish(PluginHostMessage::Status(message.clone()));
        }
    }

    fn pump(&mut self, players: &[Arc<PlayerPort>]) {
        for (identifier, entry) in &mut self.entries {
            let events = match entry.session.drain_events() {
                Ok(events) => events,
                Err(error) => {
                    for port in players {
                        port.publish(PluginHostMessage::Status(error.clone()));
                    }
                    continue;
                }
            };
            for event in events {
                match event {
                    HostEvent::PluginMessage(name, data) => {
                        for port in players {
                            let result = port.commands.try_send(PluginCommand::FromGlobal(
                                identifier.clone(),
                                name.clone(),
                                data.clone(),
                            ));
                            if matches!(result, Err(mpsc::TrySendError::Full(_))) {
                                port.publish(PluginHostMessage::Status(
                                    "A player missed a plugin message because its queue is full"
                                        .into(),
                                ));
                            }
                        }
                    }
                    HostEvent::MenuUpdate(items) => {
                        for port in players {
                            port.menu(menu_update(identifier, &entry.name, items.clone()));
                        }
                        entry.menu = Some(items);
                    }
                    HostEvent::Ui(effect) => {
                        if let Some(port) = players.last() {
                            port.publish(host_message(&entry.name, effect));
                        }
                    }
                    HostEvent::Console(message) => {
                        crate::diagnostics::log(
                            message.level,
                            &format!("plugin/{identifier}"),
                            &message.text,
                        );
                    }
                    HostEvent::Overlay(_) | HostEvent::Sidebar(_) => {}
                }
            }
        }
    }
}

impl PlayerPort {
    fn menu(&self, message: PluginHostMessage) {
        if matches!(
            self.commands.try_send(PluginCommand::GlobalMenu(message)),
            Err(mpsc::TrySendError::Full(_))
        ) {
            self.publish(PluginHostMessage::Status(
                "A player plugin host could not receive a menu update".into(),
            ));
        }
    }

    fn publish(&self, message: PluginHostMessage) {
        record(&self.messages, message);
        if !self.activity.has_active_sessions() && !lock(&self.messages).is_empty() {
            self.activity.wake();
        }
    }
}

fn menu_update(identifier: &str, name: &str, items: Vec<PluginMenuItem>) -> PluginHostMessage {
    PluginHostMessage::PluginMenuUpdated {
        plugin_identifier: identifier.into(),
        plugin_name: name.into(),
        global: true,
        items,
    }
}
