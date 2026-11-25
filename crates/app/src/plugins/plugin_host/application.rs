//! The application owns global realms and coordinates package changes across players.
use super::{
    HOST_EVENT_POLL_INTERVAL, PluginCommand, PluginHostMessage, UiActivity, enabled_packages,
    host_message, load_grants, record,
};
use crate::plugin_runtime::{HostEvent, PluginRuntimeSession, PluginUi, standalone::Hub};
use crate::plugins::plugin_players::{Message, Options, Target, Ticket};
use riina_core::{plugin::Manifest, plugin_menu::PluginMenuItem, sync::lock};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

const CHANGE_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_PLAYERS_PER_PLUGIN: usize = 16;
const MAX_PENDING_PLAYERS: usize = 32;
type Acknowledgement = mpsc::Receiver<Result<(), String>>;

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct GlobalMenu {
    pub(crate) package: String,
    pub(crate) generation: u64,
    pub(crate) title: String,
    pub(crate) items: Vec<PluginMenuItem>,
}

struct WindowlessUi {
    activity: UiActivity,
    menus: Mutex<Vec<GlobalMenu>>,
}

impl WindowlessUi {
    fn new() -> Self {
        Self {
            activity: UiActivity::new(),
            menus: Mutex::default(),
        }
    }
}

pub(crate) struct GlobalHost {
    handle: GlobalHandle,
    worker: Option<JoinHandle<()>>,
}

#[derive(Clone)]
pub(crate) struct GlobalHandle {
    pub(crate) ui: PluginUi,
    commands: mpsc::SyncSender<Request>,
    next_player: Arc<AtomicU64>,
    launches: Arc<Mutex<Vec<Arc<Ticket>>>>,
    windowless: Option<Arc<WindowlessUi>>,
}

pub(super) struct PlayerPort {
    pub(super) commands: mpsc::SyncSender<PluginCommand>,
    pub(super) messages: Arc<Mutex<Vec<PluginHostMessage>>>,
    pub(super) activity: Arc<UiActivity>,
    pub(super) closed: AtomicBool,
    pub(super) label: String,
    pub(super) managed: Option<Arc<Ticket>>,
}

#[derive(Clone)]
pub(super) enum PackageChange {
    Enable(Arc<Manifest>, PathBuf),
    Disable(String),
    Suspend(String),
    Reload(BTreeSet<String>),
}

enum Request {
    #[cfg(any(target_os = "macos", test))]
    LoadEnabledGlobals,
    Join(Weak<PlayerPort>),
    Ensure(
        Weak<PlayerPort>,
        Arc<Manifest>,
        PathBuf,
        mpsc::SyncSender<Result<(), String>>,
    ),
    Change(PackageChange, Option<mpsc::SyncSender<Schedule>>),
    ActivateMenu {
        package: String,
        item: String,
        generation: Option<u64>,
    },
    FromPlayer(Weak<PlayerPort>, String, Message),
    Shutdown,
}

struct Schedule {
    result: Result<(), String>,
    acknowledgements: Vec<Acknowledgement>,
}

impl GlobalHost {
    pub(crate) fn start(preferences_path: &Path) -> Result<Self, String> {
        Self::start_with_windowless(
            preferences_path,
            crate::platform::plugin_application::SUPPORTS_WINDOWLESS,
        )
    }

    pub(super) fn start_with_windowless(
        preferences_path: &Path,
        windowless: bool,
    ) -> Result<Self, String> {
        let (commands, requests) = mpsc::sync_channel(64);
        let handle = GlobalHandle {
            ui: PluginUi {
                windows: Hub::new(windowless),
                ..PluginUi::default()
            },
            commands,
            next_player: Arc::default(),
            launches: Arc::default(),
            windowless: windowless.then(|| Arc::new(WindowlessUi::new())),
        };
        let worker_ui = handle.ui.clone();
        let launches = handle.launches.clone();
        let windowless = handle.windowless.clone();
        let profile = preferences_path.to_path_buf();
        let worker = crate::background::spawn_named("riina-application-plugins", move || {
            Coordinator::new(&profile, worker_ui, launches, windowless).run(requests);
        })
        .map_err(|error| format!("Could not start global plugin host: {error}"))?;
        Ok(Self {
            handle,
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

#[cfg(any(target_os = "macos", test))]
impl GlobalHandle {
    pub(crate) fn menus(&self) -> Vec<GlobalMenu> {
        self.windowless
            .as_ref()
            .map_or_else(Vec::new, |windowless| lock(&windowless.menus).clone())
    }

    pub(crate) fn activate_idle_menu(
        &self,
        package: String,
        generation: u64,
        id: String,
    ) -> Result<(), String> {
        self.send(Request::ActivateMenu {
            package,
            item: id,
            generation: Some(generation),
        })
    }

    pub(crate) fn reload_all(&self) -> Result<(), String> {
        self.change(PackageChange::Reload(BTreeSet::new()))
    }

    pub(crate) fn set_ui_waker(&self, wake: impl Fn() + Send + Sync + 'static) {
        if let Some(windowless) = &self.windowless {
            windowless.activity.set_waker(wake);
        }
    }

    pub(crate) fn load_enabled_globals(&self) -> Result<(), String> {
        self.send(Request::LoadEnabledGlobals)
    }
}

impl GlobalHandle {
    pub(crate) fn enable(&self, manifest: Manifest, directory: PathBuf) -> Result<(), String> {
        self.change(PackageChange::Enable(Arc::new(manifest), directory))
    }

    pub(crate) fn disable(&self, identifier: String) -> Result<(), String> {
        self.change(PackageChange::Disable(identifier))
    }

    pub(crate) fn suspend_for_update(&self, identifier: String) -> Result<(), String> {
        self.change_and_wait(PackageChange::Suspend(identifier))
    }

    pub(crate) fn activate_after_update(
        &self,
        manifest: Manifest,
        directory: PathBuf,
    ) -> Result<(), String> {
        self.change_and_wait(PackageChange::Enable(Arc::new(manifest), directory))
    }

    pub(crate) fn take_player_launches(&self) -> Vec<Arc<Ticket>> {
        std::mem::take(&mut *lock(&self.launches))
    }

    pub(super) fn next_player_label(&self) -> String {
        self.next_player.fetch_add(1, Ordering::Relaxed).to_string()
    }

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
        port: &Arc<PlayerPort>,
    ) -> Result<(), String> {
        let (sender, receiver) = mpsc::sync_channel(1);
        self.send(Request::Ensure(
            Arc::downgrade(port),
            manifest.clone(),
            directory.to_owned(),
            sender,
        ))?;
        // Closing one player must not wait for an application-level startup dialog.
        while port.is_open() {
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
        self.send(Request::ActivateMenu {
            package: identifier,
            item: id,
            generation: None,
        })
    }

    pub(super) fn deliver(
        &self,
        port: &Arc<PlayerPort>,
        identifier: String,
        message: Message,
    ) -> Result<(), String> {
        self.send(Request::FromPlayer(
            Arc::downgrade(port),
            identifier,
            message,
        ))
    }
}

struct Entry {
    session: PluginRuntimeSession,
    name: String,
    generation: u64,
    menu: Option<Vec<PluginMenuItem>>,
    managed: BTreeMap<u64, PendingPlayer>,
}

struct PendingPlayer {
    ticket: Arc<Ticket>,
    messages: Vec<Message>,
    pending_bytes: usize,
}

impl Drop for Entry {
    fn drop(&mut self) {
        for player in self.managed.values() {
            player.ticket.close();
        }
    }
}

struct Coordinator {
    directory: PathBuf,
    permissions: PathBuf,
    enabled: PathBuf,
    ui: PluginUi,
    entries: BTreeMap<String, Entry>,
    suspended: BTreeSet<String>,
    players: Vec<Weak<PlayerPort>>,
    launches: Arc<Mutex<Vec<Arc<Ticket>>>>,
    windowless: Option<Arc<WindowlessUi>>,
    next_generation: u64,
}

impl Coordinator {
    fn new(
        profile: &Path,
        ui: PluginUi,
        launches: Arc<Mutex<Vec<Arc<Ticket>>>>,
        windowless: Option<Arc<WindowlessUi>>,
    ) -> Self {
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
            launches,
            windowless,
            next_generation: 0,
        }
    }

    fn run(mut self, requests: mpsc::Receiver<Request>) {
        let mut shown = self.windowless.as_ref().map(|_| self.ui_revisions());
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
            if let (Some(shown), Some(windowless)) = (&mut shown, &self.windowless) {
                let current = self.ui_revisions();
                if current != *shown {
                    *shown = current;
                    windowless.activity.wake();
                }
            }
        }
    }

    fn ui_revisions(&self) -> (u64, u64) {
        (self.ui.windows.revision(), self.ui.developer.revision())
    }

    fn handle(&mut self, request: Request) {
        match request {
            #[cfg(any(target_os = "macos", test))]
            Request::LoadEnabledGlobals => self.load_enabled_globals(),
            Request::Join(player) => {
                if let Some(port) = player.upgrade() {
                    for (identifier, entry) in &mut self.entries {
                        if let Some(items) = &entry.menu
                            && port.allows(identifier)
                        {
                            port.menu(menu_update(identifier, &entry.name, items.clone()));
                        }
                        entry.attach(identifier, &port);
                    }
                    self.players.push(player);
                }
            }
            Request::Ensure(player, manifest, root, response) => {
                let result = if player.upgrade().is_some_and(|port| port.is_open()) {
                    self.ensure(&manifest, &root)
                } else {
                    Err("Plugin startup was cancelled".into())
                };
                let _ = response.try_send(result);
            }
            Request::Change(change, response) => self.change(change, response),
            Request::ActivateMenu {
                package,
                item,
                generation,
            } => {
                if let Some(entry) = self.entries.get(&package)
                    && generation.is_none_or(|generation| entry.generation == generation)
                    && let Err(error) = entry.session.deliver_plugin_menu_action(item)
                {
                    self.report(error);
                }
            }
            Request::FromPlayer(player, identifier, message) => {
                if let Some(player) = player.upgrade()
                    && player.is_open()
                    && let Some(entry) = self.entries.get(&identifier)
                    && let Err(error) = entry.session.deliver_plugin_message(
                        message.name,
                        message.data,
                        Some(player.label.clone()),
                    )
                {
                    self.report(error);
                }
            }
            Request::Shutdown => {}
        }
    }

    fn load_enabled_globals(&mut self) {
        for plugin in enabled_packages(&self.directory, &self.enabled) {
            if !self.suspended.contains(&plugin.manifest.identifier)
                && let Err(error) = self.ensure(&plugin.manifest, &plugin.directory)
            {
                self.report(error);
            }
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
        self.next_generation = self.next_generation.wrapping_add(1);
        self.entries.insert(
            manifest.identifier.clone(),
            Entry {
                session,
                name: manifest.name.clone(),
                generation: self.next_generation,
                menu: None,
                managed: BTreeMap::new(),
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
        self.refresh_menus();
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
                self.load_enabled_globals();
                Ok(())
            }
        }
    }

    fn live_players(&mut self) -> Vec<Arc<PlayerPort>> {
        let mut live = Vec::new();
        self.players.retain(|player| {
            let Some(player) = player.upgrade().filter(|port| port.is_open()) else {
                return false;
            };
            live.push(player);
            true
        });
        live
    }

    fn report(&mut self, message: String) {
        let players = self.live_players();
        if players.is_empty() {
            crate::diagnostics::warn!("{message}");
        }
        for port in players {
            port.publish(PluginHostMessage::Status(message.clone()));
        }
    }

    fn pump(&mut self, players: &[Arc<PlayerPort>]) {
        let mut menu_changed = false;
        for (identifier, entry) in &mut self.entries {
            entry.managed.retain(|_, player| player.ticket.is_open());
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
                    HostEvent::PluginMessage(message) => {
                        entry.route(identifier, message, players);
                    }
                    HostEvent::CreatePluginPlayer(id, options) => {
                        match entry.create_player(identifier, id, options, &self.launches) {
                            Ok(()) => {
                                if let Some(windowless) = &self.windowless {
                                    windowless.activity.wake();
                                }
                            }
                            Err(error) => {
                                if let Some(port) = players.last() {
                                    port.publish(PluginHostMessage::Status(error));
                                } else {
                                    crate::diagnostics::warn!("{error}");
                                }
                            }
                        }
                    }
                    HostEvent::MenuUpdate(items) => {
                        menu_changed = true;
                        for port in players.iter().filter(|port| port.allows(identifier)) {
                            port.menu(menu_update(identifier, &entry.name, items.clone()));
                        }
                        entry.menu = Some(items);
                    }
                    HostEvent::Ui(effect) => {
                        if let Some(port) = players.last() {
                            port.publish(host_message(&entry.name, effect));
                        } else {
                            crate::platform::plugin_application::present_without_player(
                                &self.ui.dialogs,
                                &entry.name,
                                effect,
                            );
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
        if menu_changed {
            self.refresh_menus();
        }
    }

    fn refresh_menus(&self) {
        let Some(windowless) = &self.windowless else {
            return;
        };
        let mut next = self
            .entries
            .iter()
            .filter_map(|(package, entry)| {
                let items = entry.menu.as_ref().filter(|items| !items.is_empty())?;
                Some(GlobalMenu {
                    package: package.clone(),
                    generation: entry.generation,
                    title: entry.name.clone(),
                    items: items.clone(),
                })
            })
            .collect::<Vec<_>>();
        let order = super::read_plugin_order(&self.enabled);
        next.sort_by(|left, right| {
            let position = |package: &str| {
                order
                    .iter()
                    .position(|id| id == package)
                    .unwrap_or(usize::MAX)
            };
            position(&left.package)
                .cmp(&position(&right.package))
                .then_with(|| left.title.cmp(&right.title))
                .then_with(|| left.package.cmp(&right.package))
        });
        *lock(&windowless.menus) = next;
        windowless.activity.wake();
    }
}

impl Entry {
    fn create_player(
        &mut self,
        package: &str,
        instance: u64,
        options: Options,
        launches: &Mutex<Vec<Arc<Ticket>>>,
    ) -> Result<(), String> {
        let mut launches = lock(launches);
        launches.retain(|ticket| ticket.is_open());
        if self.managed.len() >= MAX_PLAYERS_PER_PLUGIN || launches.len() >= MAX_PENDING_PLAYERS {
            return Err(format!("{} reached the plugin player limit", self.name));
        }
        let ticket = Ticket::new(package.into(), instance, options);
        self.managed.insert(
            instance,
            PendingPlayer {
                ticket: ticket.clone(),
                messages: Vec::new(),
                pending_bytes: 0,
            },
        );
        launches.push(ticket);
        Ok(())
    }

    fn attach(&mut self, package: &str, port: &PlayerPort) {
        let Some(ticket) = &port.managed else { return };
        if ticket.package == package
            && let Some(slot) = self.managed.get_mut(&ticket.instance)
            && Arc::ptr_eq(&slot.ticket, ticket)
        {
            for message in slot.messages.drain(..) {
                port.deliver(package, &message);
            }
            slot.pending_bytes = 0;
        }
    }

    fn route(&mut self, package: &str, mut message: Message, ports: &[Arc<PlayerPort>]) {
        match message.target.take() {
            Some(Target::PlayerLabel(label)) => {
                if let Some(port) = ports
                    .iter()
                    .find(|port| port.managed.is_none() && port.label == label)
                {
                    port.deliver(package, &message);
                }
            }
            Some(Target::Instance(instance)) => {
                if let Some(player) = self.managed.get_mut(&instance) {
                    player.deliver(package, &message, ports);
                }
            }
            Some(Target::Broadcast) => {
                for player in self.managed.values_mut() {
                    player.deliver(package, &message, ports);
                }
            }
            None => {}
        }
    }
}

impl PendingPlayer {
    fn deliver(&mut self, package: &str, message: &Message, ports: &[Arc<PlayerPort>]) {
        if !self.ticket.is_open() {
            return;
        }
        if let Some(port) = ports.iter().find(|port| {
            port.managed
                .as_ref()
                .is_some_and(|ticket| Arc::ptr_eq(ticket, &self.ticket))
        }) {
            port.deliver(package, message);
        } else {
            // Keep messages sent immediately after createPlayerInstance until the
            // player registers. Its host delivers them after running the entry.
            let bytes = message.name.len() + message.data.to_string().len();
            if self.messages.len() < 16
                && self.pending_bytes.saturating_add(bytes) <= 2 * 1024 * 1024
            {
                self.pending_bytes += bytes;
                self.messages.push(message.clone());
            } else if let Some(port) = ports.last() {
                port.publish(PluginHostMessage::Status(
                    "A starting plugin player's message queue is full".into(),
                ));
            }
        }
    }
}

impl PlayerPort {
    fn is_open(&self) -> bool {
        !self.closed.load(Ordering::Acquire)
            && self.managed.as_ref().is_none_or(|ticket| ticket.is_open())
    }

    pub(super) fn allows(&self, package: &str) -> bool {
        self.managed
            .as_ref()
            .is_none_or(|ticket| ticket.allows(package))
    }

    fn deliver(&self, package: &str, message: &Message) {
        let command =
            PluginCommand::FromGlobal(package.into(), message.name.clone(), message.data.clone());
        if matches!(
            self.commands.try_send(command),
            Err(mpsc::TrySendError::Full(_))
        ) {
            self.publish(PluginHostMessage::Status(
                "A player missed a plugin message because its queue is full".into(),
            ));
        }
    }

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
