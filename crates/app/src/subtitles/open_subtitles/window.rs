//! Player-owned subtitle search window and its provider callbacks.
mod builtin_download;
mod media;
mod plugin_search;
use super::{ResultRow, provider_from_index, provider_index, provider_name, search_provider};
use crate::{OnlineSubtitlesWindow, PlaybackStore, PlayerWindow, subtitle_discovery::Readiness};
use media::Media;
use riina_core::online_subtitles::{Provider, ResultSelection};
use riina_mpv::CommandSender;
use riina_ui::PlayerMenuActions;
use slint::{ComponentHandle, ModelRc, Timer, VecModel, Weak};
use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

#[derive(Clone)]
struct SearchContext {
    media: Media,
    media_path: PathBuf,
    languages: String,
    provider: Provider,
}

impl SearchContext {
    fn for_media(
        app: &PlayerWindow,
        state: &riina_core::Playback,
        preferred: Option<Provider>,
    ) -> Result<(Self, String), &'static str> {
        let no_media = "Open media before searching for subtitles";
        let media = Media::capture(app, state).ok_or(no_media)?;
        let current = state
            .playlist
            .iter()
            .find(|entry| {
                state
                    .active_playlist_entry
                    .map_or(entry.current, |id| id == entry.id)
            })
            .ok_or(no_media)?;
        let path = crate::file_actions::local_media_path(&current.filename)
            .ok_or("Online subtitles require local media files")?;
        let query = current
            .title
            .as_ref()
            .filter(|title| !title.trim().is_empty())
            .map_or_else(
                || {
                    path.file_stem()
                        .and_then(|stem| stem.to_str())
                        .unwrap_or_default()
                        .to_owned()
                },
                String::clone,
            );
        let settings = app.global::<riina_ui::PlayerPreferences>();
        let context = Self {
            media,
            media_path: path,
            languages: settings.get_subtitle_preferred_languages().to_string(),
            provider: preferred.unwrap_or_else(|| {
                provider_from_index(settings.get_open_subtitles_provider_index())
            }),
        };
        Ok((context, query))
    }
}

#[derive(Clone, Default)]
struct SearchState {
    context: Rc<RefCell<Option<SearchContext>>>,
    rows: Arc<Mutex<Vec<ResultRow>>>,
    selection: Rc<RefCell<ResultSelection>>,
    ticket: Arc<AtomicU64>,
}

impl SearchState {
    fn retire(&self, window: &OnlineSubtitlesWindow) {
        self.ticket.fetch_add(1, Ordering::AcqRel);
        self.context.borrow_mut().take();
        riina_core::sync::lock(&self.rows).clear();
        self.selection.borrow_mut().clear();
        window.set_searching(false);
        window.set_downloading(false);
        window.set_selected_count(0);
        window.set_results(ModelRc::default());
    }

    fn request(&self) -> Option<Request> {
        Some(Request {
            context: self.context.borrow().as_ref()?.clone(),
            counter: self.ticket.clone(),
            ticket: self.ticket.load(Ordering::Acquire),
        })
    }
}

struct Request {
    context: SearchContext,
    counter: Arc<AtomicU64>,
    ticket: u64,
}

impl Request {
    fn active(&self) -> bool {
        self.ticket == self.counter.load(Ordering::Acquire)
    }

    fn check(&self, owner: &Player, window: &OnlineSubtitlesWindow) -> Option<PlayerWindow> {
        let app = owner.app.upgrade()?;
        if self
            .context
            .media
            .is_current(&app, &riina_core::sync::lock(&owner.playback))
        {
            Some(app)
        } else {
            window.set_status_text(
                "Media changed. Open subtitle search for the current file.".into(),
            );
            None
        }
    }
}

#[derive(Clone)]
struct Player {
    app: Weak<PlayerWindow>,
    playback: PlaybackStore,
}

pub(crate) struct SearchWindowGuard {
    window: Rc<RefCell<Option<OnlineSubtitlesWindow>>>,
    builtin: SearchState,
    plugin: Option<plugin_search::Search>,
    _automatic_search: Rc<Timer>,
}

impl Drop for SearchWindowGuard {
    fn drop(&mut self) {
        if let Some(window) = self.window.borrow_mut().take() {
            self.builtin.retire(&window);
            let _ = window.hide();
        }
        if let Some(plugin) = &self.plugin {
            plugin.close();
        }
    }
}

pub(crate) fn connect_player(
    app: &PlayerWindow,
    sender: CommandSender,
    playback: PlaybackStore,
    subtitle_discovery_ready: Arc<Readiness>,
    plugins: Option<crate::plugin_host::PluginHostHandle>,
) -> SearchWindowGuard {
    let window_slot = Rc::new(RefCell::new(None::<crate::OnlineSubtitlesWindow>));
    let search = SearchState::default();
    let auto_search_request = Rc::new(Cell::new(false));
    let requested_provider = Rc::new(Cell::new(None::<Provider>));
    let plugin_search =
        plugins.map(|host| plugin_search::Search::new(app, host, sender.clone(), playback.clone()));
    if let Some(plugin) = plugin_search.clone() {
        let built_in = window_slot.clone();
        let search = search.clone();
        app.global::<PlayerMenuActions>()
            .on_find_online_subtitles_with_plugin(move |key| {
                if let Some(window) = built_in.borrow().as_ref() {
                    search.retire(window);
                    _ = window.hide();
                }
                plugin.open(key.as_str());
            });
    }
    let app_weak = app.as_weak();
    let app_weak_for_provider = app.as_weak();
    let window_slot_callback = window_slot.clone();
    let playback_callback = playback.clone();
    let auto_search_request_for_open = auto_search_request.clone();
    let requested_provider_for_open = requested_provider.clone();
    let callback_search = search.clone();
    let callback_plugin = plugin_search.clone();
    app.global::<PlayerMenuActions>()
        .on_find_online_subtitles(move || {
            let should_search_after_open = auto_search_request_for_open.replace(false);
            let requested_provider = requested_provider_for_open.replace(None);
            let Some(app) = app_weak.upgrade() else {
                return;
            };
            let search = &callback_search;
            if let Some(plugin) = &callback_plugin {
                let key = app
                    .global::<riina_ui::PlayerPreferences>()
                    .get_open_subtitles_plugin_provider();
                if requested_provider.is_none()
                    && crate::online_subtitles_preferences::plugin_label(&app, key.as_str())
                        .is_some()
                {
                    if let Some(window) = window_slot_callback.borrow().as_ref() {
                        search.retire(window);
                        _ = window.hide();
                    }
                    plugin.open(key.as_str());
                    return;
                }
                plugin.close();
            }
            let playback = riina_core::sync::lock(&playback_callback).clone();
            let (context, query) =
                match SearchContext::for_media(&app, &playback, requested_provider) {
                    Ok(context) => context,
                    Err(error) => {
                        app.set_status(error.into());
                        return;
                    }
                };
            let provider = context.provider;
            let existing = window_slot_callback
                .borrow()
                .as_ref()
                .map(ComponentHandle::clone_strong);
            let window = if let Some(window) = existing {
                window
            } else {
                let window = match OnlineSubtitlesWindow::new() {
                    Ok(window) => window,
                    Err(error) => {
                        crate::diagnostics::report(
                            &app,
                            format!("Could not open online subtitle search: {error}"),
                        );
                        return;
                    }
                };
                install_window_callbacks(&app, &window, search, &sender, &playback_callback);
                crate::platform::center_auxiliary_window(&window, &app);
                *window_slot_callback.borrow_mut() = Some(window.clone_strong());
                window
            };
            search.retire(&window);
            *search.context.borrow_mut() = Some(context);
            window.set_query(query.into());
            window.set_provider_index(provider_index(provider));
            window.set_status_text(if provider == Provider::Shooter {
                "Search Shooter.cn by the local video's sampled content hash.".into()
            } else {
                format!("Search {} by title or filename.", provider_name(provider)).into()
            });
            if let Err(error) = crate::platform::show_window(&window) {
                crate::diagnostics::report(
                    &app,
                    format!("Could not show online subtitle search: {error}"),
                );
            } else {
                crate::platform::enable_online_subtitles_acrylic(&window);
                if should_search_after_open {
                    window.invoke_search();
                }
            }
        });
    let requested_provider_for_menu = requested_provider.clone();
    app.global::<PlayerMenuActions>()
        .on_find_online_subtitles_with_provider(move |index| {
            let Some(app) = app_weak_for_provider.upgrade() else {
                return;
            };
            requested_provider_for_menu.set(Some(provider_from_index(index)));
            app.global::<PlayerMenuActions>()
                .invoke_find_online_subtitles();
        });

    SearchWindowGuard {
        window: window_slot,
        builtin: search,
        plugin: plugin_search,
        _automatic_search: install_automatic_search(
            app,
            playback,
            subtitle_discovery_ready,
            auto_search_request,
        ),
    }
}

fn install_window_callbacks(
    app: &PlayerWindow,
    window: &OnlineSubtitlesWindow,
    state: &SearchState,
    sender: &CommandSender,
    playback: &PlaybackStore,
) {
    let owner = Player {
        app: app.as_weak(),
        playback: playback.clone(),
    };
    crate::appearance_preferences::install_system_typography(window);
    install_search(window, state, owner.clone());
    install_selection(window, state);
    builtin_download::install(window, state, owner, sender);
    install_auxiliary_actions(app, window, state);
}

fn install_search(window: &OnlineSubtitlesWindow, state: &SearchState, owner: Player) {
    let weak_window = window.as_weak();
    let state = state.clone();
    window.on_search(move || {
        let (Some(window), Some(request)) = (weak_window.upgrade(), state.request()) else {
            return;
        };
        if window.get_searching() || window.get_downloading() {
            return;
        }
        let Some(app) = request.check(&owner, &window) else {
            return;
        };
        let query = window.get_query().to_string();
        let key = app
            .global::<riina_ui::PlayerPreferences>()
            .get_open_subtitles_api_key()
            .to_string();
        let assrt_token = app
            .global::<riina_ui::PlayerPreferences>()
            .get_assrt_token()
            .to_string();
        let provider = request.context.provider;
        window.set_searching(true);
        window.set_provider_index(provider_index(provider));
        window.set_status_text(format!("Searching {}…", provider_name(provider)).into());
        window.set_selected_count(0);
        state.selection.borrow_mut().clear();
        riina_core::sync::lock(&state.rows).clear();
        window.set_results(ModelRc::default());
        let weak_window = window.as_weak();
        let rows = state.rows.clone();
        let owner = owner.clone();
        let started = crate::background::spawn_named("riina-subtitle-search", move || {
            if !request.active() {
                return;
            }
            let result = search_provider(
                provider,
                &key,
                &assrt_token,
                &request.context.media_path,
                &query,
                &request.context.languages,
            );
            // The result has no recipient once its window or event loop closes.
            let _ = crate::background::to_ui(weak_window, move |window| {
                if !request.active() {
                    return;
                }
                window.set_searching(false);
                if request.check(&owner, &window).is_none() {
                    return;
                }
                match result {
                    Ok(found) => {
                        let count = found.len();
                        let model = ui_rows(&found, &ResultSelection::default());
                        *riina_core::sync::lock(&rows) = found;
                        window.set_selected_count(0);
                        window.set_results(ModelRc::new(VecModel::from(model)));
                        window.set_status_text(if count == 0 {
                            format!(
                                "No matching subtitles found on {}.",
                                provider_name(provider)
                            )
                            .into()
                        } else {
                            format!(
                                "{count} subtitle matches found on {}.",
                                provider_name(provider)
                            )
                            .into()
                        });
                    }
                    Err(error) => window.set_status_text(error.into()),
                }
            });
        });
        if let Err(error) = started {
            window.set_searching(false);
            window.set_status_text(format!("Could not start subtitle search: {error}").into());
        }
    });
}

fn install_selection(window: &OnlineSubtitlesWindow, state: &SearchState) {
    let weak_window = window.as_weak();
    let rows = state.rows.clone();
    let selection = state.selection.clone();
    window.on_select_result(move |index, extend, range| {
        let (Some(window), Ok(rows)) = (weak_window.upgrade(), rows.lock()) else {
            return;
        };
        let Ok(index) = usize::try_from(index) else {
            return;
        };
        let mut selection = selection.borrow_mut();
        selection.click(index, rows.len(), extend, range);
        let selected_count = selection.len();
        let model = ui_rows(&rows, &selection);
        window.set_results(ModelRc::new(VecModel::from(model)));
        window.set_selected_count(selected_count as i32);
        window.set_status_text(if selected_count > 1 {
            format!(
                "{selected_count} subtitles selected. Use Ctrl/Command-click to change selection."
            )
            .into()
        } else if selected_count == 1 {
            "1 subtitle selected.".into()
        } else {
            "No subtitles selected.".into()
        });
    });

    let weak_window = window.as_weak();
    let rows = state.rows.clone();
    let selection = state.selection.clone();
    window.on_selection_action(move |action| {
        if action != 0 {
            return;
        }
        let (Some(window), Ok(rows)) = (weak_window.upgrade(), rows.lock()) else {
            return;
        };
        let mut selection = selection.borrow_mut();
        selection.select_all(rows.len());
        window.set_results(ModelRc::new(VecModel::from(ui_rows(&rows, &selection))));
        window.set_selected_count(selection.len() as i32);
        window.set_status_text(format!("{} subtitles selected.", selection.len()).into());
    });
}

fn install_auxiliary_actions(
    app: &PlayerWindow,
    window: &OnlineSubtitlesWindow,
    state: &SearchState,
) {
    let weak_window = window.as_weak();
    let cancel = state.clone();
    window.on_cancel(move || {
        if let Some(window) = weak_window.upgrade() {
            cancel.retire(&window);
            let _ = window.hide();
        }
    });
    let weak_window = window.as_weak();
    let close = state.clone();
    window.window().on_close_requested(move || {
        if let Some(window) = weak_window.upgrade() {
            close.retire(&window);
        }
        slint::CloseRequestResponse::HideWindow
    });
    let weak_app = app.as_weak();
    window.on_open_assrt_site(move || {
        if let Some(app) = weak_app.upgrade()
            && let Err(error) = crate::platform::open_http_url_in_browser("https://assrt.net")
        {
            crate::diagnostics::report(&app, format!("Could not open Assrt.net: {error}"));
        }
    });
    let weak_app = app.as_weak();
    window.on_open_shooter_site(move || {
        if let Some(app) = weak_app.upgrade()
            && let Err(error) = crate::platform::open_http_url_in_browser("https://www.shooter.cn")
        {
            crate::diagnostics::report(&app, format!("Could not open Shooter.cn: {error}"));
        }
    });
}

fn install_automatic_search(
    app: &PlayerWindow,
    playback: PlaybackStore,
    subtitle_discovery_ready: Arc<Readiness>,
    trigger_search: Rc<Cell<bool>>,
) -> Rc<Timer> {
    let pending_search = Rc::new(RefCell::new(None::<(u64, std::time::Instant)>));
    let weak_app = app.as_weak();
    let pending_for_tick = pending_search.clone();
    let searched_generation = Cell::new(None::<u64>);
    let auto_timer = Rc::new(Timer::default());
    auto_timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(500),
        move || {
            let Some(app) = weak_app.upgrade() else {
                return;
            };
            let Ok(settings) = crate::online_subtitles_preferences::current_player(&app) else {
                return;
            };
            let Ok(auto_load) = crate::subtitle_auto_load_preferences::current_player(&app) else {
                return;
            };
            let plugin_available = settings.plugin_provider.as_deref().is_some_and(|key| {
                crate::online_subtitles_preferences::plugin_label(&app, key).is_some()
            });
            let credentials_available = plugin_available
                || match settings.provider {
                    Provider::OpenSubtitles => !settings.api_key.trim().is_empty(),
                    Provider::Assrt => !settings.assrt_token.trim().is_empty(),
                    Provider::Shooter => true,
                };
            if !settings.auto_search || !credentials_available {
                *pending_for_tick.borrow_mut() = None;
                return;
            }
            let state = riina_core::sync::lock(&playback).clone();
            let duration = state.duration.unwrap_or_default();
            if !matches!(
                state.phase,
                riina_core::Phase::Playing | riina_core::Phase::Paused
            ) || duration < f64::from(settings.auto_search_threshold_minutes) * 60.0
                || state
                    .playlist
                    .iter()
                    .find(|entry| entry.current)
                    .is_none_or(|entry| !plugin_available && entry.filename.contains("://"))
                || searched_generation.get() == Some(state.generation)
            {
                *pending_for_tick.borrow_mut() = None;
                return;
            }
            if state
                .tracks
                .iter()
                .any(|track| track.kind == riina_core::TrackKind::Sub)
            {
                *pending_for_tick.borrow_mut() = None;
                searched_generation.set(Some(state.generation));
                return;
            }
            if auto_load.mode != riina_core::subtitle_controls::AutoLoadMode::Disabled
                && !subtitle_discovery_ready.is_ready(state.generation)
            {
                *pending_for_tick.borrow_mut() = None;
                return;
            }
            let now = std::time::Instant::now();
            let mut pending = pending_for_tick.borrow_mut();
            match *pending {
                Some((generation, since))
                    if generation == state.generation
                        && now.duration_since(since) >= Duration::from_millis(500) =>
                {
                    *pending = None;
                    searched_generation.set(Some(state.generation));
                }
                Some((generation, _)) if generation == state.generation => return,
                _ => {
                    *pending = Some((state.generation, now));
                    return;
                }
            }
            drop(pending);
            trigger_search.set(true);
            app.global::<PlayerMenuActions>()
                .invoke_find_online_subtitles();
        },
    );
    if !app.get_loaded()
        || !app
            .global::<riina_ui::PlayerPreferences>()
            .get_open_subtitles_auto_search()
    {
        auto_timer.stop();
    }
    let weak_timer = Rc::downgrade(&auto_timer);
    app.on_open_subtitles_auto_search_changed(move |enabled| {
        let Some(timer) = weak_timer.upgrade() else {
            return;
        };
        if enabled {
            if !timer.running() {
                timer.restart();
            }
        } else {
            timer.stop();
            pending_search.borrow_mut().take();
        }
    });
    auto_timer
}

fn ui_rows(rows: &[ResultRow], selection: &ResultSelection) -> Vec<crate::OnlineSubtitleRow> {
    rows.iter()
        .enumerate()
        .map(|(index, row)| ui_row(row, selection.contains(index)))
        .collect()
}

fn ui_row(row: &ResultRow, selected: bool) -> crate::OnlineSubtitleRow {
    crate::OnlineSubtitleRow {
        file_name: row.file_name.clone().into(),
        language: row.language.clone().into(),
        release: row.release.clone().into(),
        year: row
            .year
            .map(|year| year.to_string())
            .unwrap_or_default()
            .into(),
        downloads: row
            .downloads
            .map(|downloads| downloads.to_string())
            .unwrap_or_default()
            .into(),
        format: row.format.clone().unwrap_or_default().into(),
        rating: row
            .uploaded_at
            .clone()
            .or_else(|| row.rating.map(|rating| format!("{rating:.1}")))
            .unwrap_or_default()
            .into(),
        uploaded_at: row.uploaded_at.clone().unwrap_or_default().into(),
        selected,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_result_chooser_passes_row_selection_to_download() -> Result<(), slint::PlatformError>
    {
        use slint::platform::{PointerEventButton, WindowEvent};
        i_slint_backend_testing::init_no_event_loop();
        let window = OnlineSubtitlesWindow::new()?;
        window
            .window()
            .set_size(slint::LogicalSize::new(620.0, 430.0));
        window.set_provider_index(3);
        window.set_results(ModelRc::new(VecModel::from(vec![
            crate::OnlineSubtitleRow {
                file_name: "First subtitle".into(),
                ..Default::default()
            },
            crate::OnlineSubtitleRow {
                file_name: "Second subtitle".into(),
                ..Default::default()
            },
        ])));
        window.show()?;
        let selected = Rc::new(Cell::new(None));
        let downloads = Rc::new(Cell::new(None));
        let chosen = selected.clone();
        let owner = window.as_weak();
        window.on_select_result(move |index, _, _| {
            chosen.set(Some(index));
            if let Some(window) = owner.upgrade() {
                window.set_selected_count(1);
            }
        });
        let finished = downloads.clone();
        window.on_download_selected(move || finished.set(selected.get()));
        let click = |x, y| {
            let position = slint::LogicalPosition::new(x, y);
            let button = PointerEventButton::Left;
            window
                .window()
                .dispatch_event(WindowEvent::PointerMoved { position });
            window
                .window()
                .dispatch_event(WindowEvent::PointerPressed { position, button });
            window
                .window()
                .dispatch_event(WindowEvent::PointerReleased { position, button });
            i_slint_backend_testing::mock_elapsed_time(Duration::ZERO);
        };
        click(68.0, 137.0);
        assert_eq!(window.get_selected_count(), 1);
        click(555.0, 400.0);
        assert_eq!(downloads.get(), Some(0));
        let cancelled = Rc::new(Cell::new(false));
        let cancel = cancelled.clone();
        window.on_cancel(move || cancel.set(true));
        window.set_downloading(true);
        click(452.0, 400.0);
        assert!(cancelled.get());
        window.hide()?;
        Ok(())
    }

    #[test]
    fn automatic_search_timer_tracks_the_live_setting() -> Result<(), slint::PlatformError> {
        i_slint_backend_testing::init_no_event_loop();
        let app = PlayerWindow::new()?;
        let playback = Arc::new(Mutex::new(Arc::new(riina_core::Playback::default())));
        let readiness = Arc::new(Readiness::default());
        let requested = Rc::new(Cell::new(false));
        let timer = install_automatic_search(&app, playback, readiness, requested.clone());
        assert!(!timer.running());

        app.global::<riina_ui::PlayerPreferences>()
            .set_open_subtitles_auto_search(true);
        i_slint_backend_testing::mock_elapsed_time(Duration::ZERO);
        assert!(!timer.running());
        app.set_loaded(true);
        i_slint_backend_testing::mock_elapsed_time(Duration::ZERO);
        assert!(timer.running());
        i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(500));
        assert!(!requested.get());

        app.global::<riina_ui::PlayerPreferences>()
            .set_open_subtitles_auto_search(false);
        i_slint_backend_testing::mock_elapsed_time(Duration::ZERO);
        assert!(!timer.running());

        app.global::<riina_ui::PlayerPreferences>()
            .set_open_subtitles_auto_search(true);
        i_slint_backend_testing::mock_elapsed_time(Duration::ZERO);
        assert!(timer.running());
        app.set_loaded(false);
        i_slint_backend_testing::mock_elapsed_time(Duration::ZERO);
        assert!(!timer.running());
        drop(timer);
        app.global::<riina_ui::PlayerPreferences>()
            .set_open_subtitles_auto_search(false);
        i_slint_backend_testing::mock_elapsed_time(Duration::ZERO);
        Ok(())
    }
}
