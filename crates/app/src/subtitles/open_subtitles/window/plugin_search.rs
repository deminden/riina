//! Presents plugin results and applies downloaded files to the originating media.
use crate::plugin_runtime::subtitles::{Description, Job, Operation};
use crate::{
    OnlineSubtitleRow, OnlineSubtitlesWindow, PlaybackStore, PlayerWindow, background,
    plugin_host::PluginHostHandle,
};
use riina_core::{Command, online_subtitles::ResultSelection, sync::lock};
use riina_mpv::CommandSender;
use slint::{ComponentHandle, ModelRc, VecModel, Weak};
use std::{
    cell::RefCell,
    io::Read,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

const TIMEOUT: Duration = Duration::from_secs(20);
static NEXT: AtomicU64 = AtomicU64::new(1);

#[derive(Clone)]
pub(super) struct Search {
    window: Rc<RefCell<Option<OnlineSubtitlesWindow>>>,
    owner: Owner,
}

#[derive(Clone)]
struct Owner {
    app: Weak<PlayerWindow>,
    plugin: PluginHostHandle,
    player: CommandSender,
    playback: PlaybackStore,
    active: Arc<Mutex<Option<Results>>>,
}

#[derive(Clone)]
struct Request {
    ticket: u64,
    generation: u64,
    media_request: i32,
    provider: String,
    label: String,
}

struct Results {
    request: Request,
    items: Vec<Description>,
    selection: ResultSelection,
}

impl Search {
    pub(super) fn new(
        app: &PlayerWindow,
        plugin: PluginHostHandle,
        player: CommandSender,
        playback: PlaybackStore,
    ) -> Self {
        Self {
            window: Rc::default(),
            owner: Owner {
                app: app.as_weak(),
                plugin,
                player,
                playback,
                active: Arc::default(),
            },
        }
    }

    pub(super) fn close(&self) {
        lock(&self.owner.active).take();
        if let Some(window) = self.window.borrow().as_ref() {
            window.set_searching(false);
            window.set_downloading(false);
            _ = window.hide();
        }
    }

    pub(super) fn open(&self, key: &str) {
        let Some(app) = self.owner.app.upgrade() else {
            return;
        };
        let Some(label) = crate::online_subtitles_preferences::plugin_label(&app, key) else {
            app.set_status("The selected subtitle provider is no longer available".into());
            return;
        };
        let playback = lock(&self.owner.playback).clone();
        if !matches!(
            playback.phase,
            riina_core::Phase::Playing | riina_core::Phase::Paused
        ) {
            app.set_status("Open media before searching for subtitles".into());
            return;
        }
        let existing = self
            .window
            .borrow()
            .as_ref()
            .map(ComponentHandle::clone_strong);
        let window = match existing {
            Some(window) if window.get_searching() || window.get_downloading() => return,
            Some(window) => window,
            None => match OnlineSubtitlesWindow::new() {
                Ok(window) => {
                    self.owner.bind(&app, &window);
                    *self.window.borrow_mut() = Some(window.clone_strong());
                    window
                }
                Err(error) => {
                    crate::diagnostics::report(
                        &app,
                        format!("Could not create the subtitle chooser. {error}"),
                    );
                    return;
                }
            },
        };
        _ = window.hide();
        let request = Request {
            ticket: NEXT.fetch_add(1, Ordering::Relaxed),
            generation: playback.generation,
            media_request: app.get_media_request_generation(),
            provider: key.into(),
            label: label.to_string(),
        };
        *lock(&self.owner.active) = Some(Results {
            request: request.clone(),
            items: Vec::new(),
            selection: ResultSelection::default(),
        });
        window.set_searching(true);
        window.set_query(label);
        window.set_results(ModelRc::default());
        window.set_selected_count(0);
        app.set_status(format!("Searching {}…", request.label).into());
        self.owner.search(&window, request);
    }
}

impl Owner {
    fn bind(&self, app: &PlayerWindow, window: &OnlineSubtitlesWindow) {
        window.set_provider_index(3);
        crate::appearance_preferences::install_system_typography(window);
        crate::platform::center_auxiliary_window(window, app);
        let handle = window.as_weak();
        let active = self.active.clone();
        window.on_select_result(move |index, extend, range| {
            if let (Some(window), Some(results), Ok(index)) = (
                handle.upgrade(),
                lock(&active).as_mut(),
                usize::try_from(index),
            ) {
                results
                    .selection
                    .click(index, results.items.len(), extend, range);
                project(&window, results);
            }
        });
        let handle = window.as_weak();
        let active = self.active.clone();
        window.on_selection_action(move |_| {
            if let (Some(window), Some(results)) = (handle.upgrade(), lock(&active).as_mut()) {
                results.selection.select_all(results.items.len());
                project(&window, results);
            }
        });
        let handle = window.as_weak();
        let owner = self.clone();
        window.on_download_selected(move || {
            let Some(window) = handle.upgrade() else {
                return;
            };
            if window.get_downloading() {
                return;
            }
            let selected = lock(&owner.active)
                .as_ref()
                .map(|result| (result.request.clone(), result.selection.indices()));
            if let Some((request, indices)) = selected
                && !indices.is_empty()
            {
                owner.download(&window, request, indices);
            }
        });
        let handle = window.as_weak();
        let active = self.active.clone();
        window.on_cancel(move || {
            lock(&active).take();
            if let Some(window) = handle.upgrade() {
                _ = window.hide();
            }
        });
        let active = self.active.clone();
        let handle = window.as_weak();
        window.window().on_close_requested(move || {
            lock(&active).take();
            if let Some(window) = handle.upgrade() {
                window.set_searching(false);
                window.set_downloading(false);
            }
            slint::CloseRequestResponse::HideWindow
        });
    }

    fn search(&self, window: &OnlineSubtitlesWindow, request: Request) {
        let (reply, response) = mpsc::sync_channel(1);
        let owner = self.clone();
        let weak = window.as_weak();
        let job = request.job(Operation::Search {
            provider: String::new(),
            reply,
        });
        self.plugin
            .run_subtitle_provider(request.provider.clone(), job);
        if let Err(error) = background::spawn_named("plugin-subtitle-search", move || {
            let found = response
                .recv_timeout(TIMEOUT)
                .map_err(|_| "The subtitle search timed out or the plugin closed".to_owned())
                .and_then(|found| found);
            _ = background::to_ui(weak, move |window| owner.present(&window, request, found));
        }) {
            window.set_searching(false);
            if let Some(app) = self.app.upgrade() {
                crate::diagnostics::report(
                    &app,
                    format!("Could not start the subtitle search. {error}"),
                );
            }
        }
    }

    fn present(
        &self,
        window: &OnlineSubtitlesWindow,
        request: Request,
        found: Result<Option<Vec<Description>>, String>,
    ) {
        if !self.has_ticket(request.ticket) {
            return;
        }
        window.set_searching(false);
        let Some(app) = self.current_player(&request) else {
            return;
        };
        let items = match found {
            Err(error) => {
                app.set_status(error.into());
                return;
            }
            Ok(None) => return,
            Ok(Some(items)) if items.is_empty() => {
                app.set_status("No matching subtitles found".into());
                return;
            }
            Ok(Some(items)) => items,
        };
        if items.len() == 1 {
            self.download(window, request, vec![0]);
            return;
        }
        if let Some(results) = lock(&self.active).as_mut() {
            results.items = items;
            window
                .set_status_text(format!("{} subtitle matches found", results.items.len()).into());
            project(window, results);
        }
        if let Err(error) = crate::platform::show_window(window) {
            crate::diagnostics::report(
                &app,
                format!("Could not show the subtitle chooser. {error}"),
            );
        } else {
            crate::platform::enable_online_subtitles_acrylic(window);
        }
    }

    fn download(&self, window: &OnlineSubtitlesWindow, request: Request, indices: Vec<usize>) {
        let Some(app) = self.current_player(&request) else {
            return;
        };
        let (reply, response) = mpsc::sync_channel(1);
        let job = request.job(Operation::Download { indices, reply });
        window.set_downloading(true);
        window.set_status_text("Downloading subtitles…".into());
        app.set_status("Downloading subtitles…".into());
        let weak = window.as_weak();
        let owner = self.clone();
        self.plugin
            .run_subtitle_provider(request.provider.clone(), job);
        if let Err(error) = background::spawn_named("plugin-subtitle-download", move || {
            let paths = response
                .recv_timeout(TIMEOUT)
                .map_err(|_| "The subtitle download timed out or the plugin closed".to_owned())
                .and_then(|result| result)
                .and_then(|paths| paths.iter().map(|path| cache_file(path)).collect());
            _ = background::to_ui(weak, move |window| owner.attach(&window, request, paths));
        }) {
            window.set_downloading(false);
            crate::diagnostics::report(
                &app,
                format!("Could not start the subtitle download. {error}"),
            );
        }
    }

    fn attach(
        &self,
        window: &OnlineSubtitlesWindow,
        request: Request,
        paths: Result<Vec<String>, String>,
    ) {
        if !self.has_ticket(request.ticket) {
            return;
        }
        window.set_downloading(false);
        let Some(app) = self.current_player(&request) else {
            return;
        };
        let result = paths.and_then(|paths| {
            let count = paths.len();
            if count == 0 {
                return Err("The plugin returned no subtitle files".into());
            }
            self.player
                .send(Command::AddSubtitlesForMedia {
                    generation: request.generation,
                    paths,
                })
                .map_err(|error| error.to_string())?;
            Ok(count)
        });
        match result {
            Ok(count) => {
                app.set_status(
                    format!(
                        "Queued {count} subtitle file{}",
                        if count == 1 { "" } else { "s" }
                    )
                    .into(),
                );
                _ = window.hide();
            }
            Err(error) => {
                window.set_status_text(error.clone().into());
                app.set_status(error.into());
            }
        }
    }

    fn current_player(&self, request: &Request) -> Option<PlayerWindow> {
        let app = self.app.upgrade()?;
        (app.get_media_request_generation() == request.media_request
            && lock(&self.playback).generation == request.generation
            && crate::online_subtitles_preferences::plugin_label(&app, &request.provider).is_some())
        .then_some(app)
    }

    fn has_ticket(&self, ticket: u64) -> bool {
        lock(&self.active)
            .as_ref()
            .is_some_and(|results| results.request.ticket == ticket)
    }
}

impl Request {
    fn job(&self, operation: Operation) -> Job {
        Job {
            operation,
            ticket: self.ticket,
            media_generation: self.generation,
            deadline: Instant::now() + TIMEOUT,
        }
    }
}

fn project(window: &OnlineSubtitlesWindow, results: &Results) {
    let rows = results
        .items
        .iter()
        .enumerate()
        .map(|(index, row)| OnlineSubtitleRow {
            file_name: row.name.clone().into(),
            language: row.left.clone().into(),
            release: row.right.clone().into(),
            selected: results.selection.contains(index),
            ..Default::default()
        })
        .collect::<Vec<_>>();
    window.set_results(ModelRc::new(VecModel::from(rows)));
    window.set_selected_count(i32::try_from(results.selection.len()).unwrap_or_default());
}

fn cache_file(path: &Path) -> Result<String, String> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| riina_core::file_selection::is_supported_subtitle(name))
        .ok_or("The plugin returned a file that is not a supported subtitle")?;
    if !std::fs::metadata(path)
        .map_err(|error| error.to_string())?
        .is_file()
    {
        return Err("The plugin returned a path that is not a regular file".into());
    }
    let file = std::fs::File::open(path).map_err(|error| error.to_string())?;
    let mut bytes = Vec::new();
    file.take(super::super::MAX_SUBTITLE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.is_empty() || bytes.len() as u64 > super::super::MAX_SUBTITLE_BYTES {
        return Err("The plugin subtitle is empty or exceeds 16 MiB".into());
    }
    super::super::save_download(name, &bytes).and_then(|path: PathBuf| {
        path.into_os_string()
            .into_string()
            .map_err(|_| "The subtitle cache path contains invalid Unicode".into())
    })
}
