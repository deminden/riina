//! Player-owned subtitle search window and its provider callbacks.
use super::{
    ResultRow, download, provider_from_index, provider_index, provider_name, save_download,
    search_provider,
};
use crate::{OnlineSubtitlesWindow, PlaybackStore, PlayerWindow, subtitle_discovery::Readiness};
use riina_core::online_subtitles::{Provider, ResultSelection};
use riina_mpv::CommandSender;
use riina_ui::PlayerMenuActions;
use slint::{ComponentHandle, ModelRc, Timer, VecModel};
use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    rc::Rc,
    sync::{Arc, Mutex},
    time::Duration,
};

#[derive(Clone, Default)]
struct SearchContext {
    media_path: PathBuf,
    languages: String,
    provider: Provider,
}

#[derive(Default)]
struct SearchState {
    context: Rc<RefCell<SearchContext>>,
    rows: Arc<Mutex<Vec<ResultRow>>>,
    selection: Rc<RefCell<ResultSelection>>,
}

pub(crate) struct SearchWindowGuard {
    _window: Rc<RefCell<Option<OnlineSubtitlesWindow>>>,
    _automatic_search: Rc<Timer>,
}

pub(crate) fn connect_player(
    app: &PlayerWindow,
    sender: CommandSender,
    playback: PlaybackStore,
    subtitle_discovery_ready: Arc<Readiness>,
) -> SearchWindowGuard {
    let window_slot = Rc::new(RefCell::new(None::<crate::OnlineSubtitlesWindow>));
    let search = SearchState::default();
    let auto_search_request = Rc::new(Cell::new(false));
    let requested_provider = Rc::new(Cell::new(None::<Provider>));
    let app_weak = app.as_weak();
    let app_weak_for_provider = app.as_weak();
    let window_slot_callback = window_slot.clone();
    let playback_callback = playback.clone();
    let auto_search_request_for_open = auto_search_request.clone();
    let requested_provider_for_open = requested_provider.clone();
    app.global::<PlayerMenuActions>()
        .on_find_online_subtitles(move || {
            let should_search_after_open = auto_search_request_for_open.replace(false);
            let requested_provider = requested_provider_for_open.replace(None);
            let Some(app) = app_weak.upgrade() else {
                return;
            };
            let current = riina_core::sync::lock(&playback_callback)
                .playlist
                .iter()
                .find(|entry| entry.current)
                .cloned();
            let Some(current) = current else {
                app.set_status("Open media before searching for subtitles".into());
                return;
            };
            if current.filename.contains("://") {
                app.set_status("Online subtitles require local media files".into());
                return;
            }
            let path = PathBuf::from(&current.filename);
            let fallback = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or_default();
            let title = current
                .title
                .filter(|title| !title.trim().is_empty())
                .unwrap_or_else(|| fallback.to_owned());
            let provider = requested_provider
                .unwrap_or_else(|| provider_from_index(app.get_open_subtitles_provider_index()));
            *search.context.borrow_mut() = SearchContext {
                media_path: path,
                languages: app.get_subtitle_preferred_languages().to_string(),
                provider,
            };
            let window = if let Some(window) = window_slot_callback.borrow().as_ref() {
                window.clone_strong()
            } else {
                let window = match OnlineSubtitlesWindow::new() {
                    Ok(window) => window,
                    Err(error) => {
                        app.set_status(
                            format!("Could not open online subtitle search: {error}").into(),
                        );
                        return;
                    }
                };
                install_window_callbacks(&app, &window, &search, &sender);
                crate::platform::center_auxiliary_window(&window, &app);
                *window_slot_callback.borrow_mut() = Some(window.clone_strong());
                window
            };
            window.set_query(title.into());
            window.set_results(ModelRc::new(VecModel::from(
                Vec::<crate::OnlineSubtitleRow>::new(),
            )));
            search.rows.lock().map(|mut rows| rows.clear()).ok();
            search.selection.borrow_mut().clear();
            window.set_selected_count(0);
            window.set_provider_index(provider_index(provider));
            window.set_status_text(if provider == Provider::Shooter {
                "Search Shooter.cn by the local video's sampled content hash.".into()
            } else {
                format!("Search {} by title or filename.", provider_name(provider)).into()
            });
            if let Err(error) = window.show() {
                app.set_status(format!("Could not show online subtitle search: {error}").into());
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
        _window: window_slot,
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
) {
    crate::appearance_preferences::install_system_typography(window);
    install_search(app, window, state);
    install_selection(window, state);
    install_download(app, window, state, sender);
    install_auxiliary_actions(app, window);
}

fn install_search(app: &PlayerWindow, window: &OnlineSubtitlesWindow, state: &SearchState) {
    let weak_window = window.as_weak();
    let weak_app = app.as_weak();
    let search_context = state.context.clone();
    let rows = state.rows.clone();
    let selection = state.selection.clone();
    window.on_search(move || {
        let (Some(window), Some(app)) = (weak_window.upgrade(), weak_app.upgrade()) else {
            return;
        };
        let query = window.get_query().to_string();
        let key = app.get_open_subtitles_api_key().to_string();
        let assrt_token = app.get_assrt_token().to_string();
        let search_context = search_context.borrow().clone();
        let provider = search_context.provider;
        window.set_searching(true);
        window.set_provider_index(match provider {
            Provider::OpenSubtitles => 0,
            Provider::Assrt => 1,
            Provider::Shooter => 2,
        });
        window.set_status_text(format!("Searching {}…", provider_name(provider)).into());
        window.set_selected_count(0);
        selection.borrow_mut().clear();
        riina_core::sync::lock(&rows).clear();
        window.set_results(ModelRc::new(VecModel::from(
            Vec::<crate::OnlineSubtitleRow>::new(),
        )));
        let weak_window = window.as_weak();
        let rows = rows.clone();
        let started = crate::background::spawn_named("riina-subtitle-search", move || {
            let result = search_provider(
                provider,
                &key,
                &assrt_token,
                &search_context.media_path,
                &query,
                &search_context.languages,
            );
            let _ = crate::background::to_ui(weak_window, move |window| {
                window.set_searching(false);
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

fn install_download(
    app: &PlayerWindow,
    window: &OnlineSubtitlesWindow,
    state: &SearchState,
    player: &CommandSender,
) {
    let weak_window = window.as_weak();
    let weak_app = app.as_weak();
    let rows = state.rows.clone();
    let selection = state.selection.clone();
    let sender = player.clone();
    window.on_download_selected(move || {
        let (Some(window), Some(app)) = (weak_window.upgrade(), weak_app.upgrade()) else {
            return;
        };
        let selected = selection.borrow().indices();
        let selected_rows = {
            let rows = riina_core::sync::lock(&rows);
            selected
                .iter()
                .filter_map(|index| rows.get(*index).cloned())
                .collect::<Vec<_>>()
        };
        if selected_rows.is_empty() {
            return;
        }
        let key = app.get_open_subtitles_api_key().to_string();
        let assrt_token = app.get_assrt_token().to_string();
        let total = selected_rows.len();
        window.set_downloading(true);
        window.set_status_text(format!("Downloading {total} selected subtitle{}…", if total == 1 { "" } else { "s" }).into());
        let weak_window = window.as_weak();
        let weak_app = app.as_weak();
        let sender = sender.clone();
        let started = crate::background::spawn_named("riina-subtitle-download", move || {
            let mut paths = Vec::with_capacity(total);
            let mut failure = None;
            for (index, row) in selected_rows.iter().enumerate() {
                let downloaded = match row.provider {
                    Provider::OpenSubtitles => download(&key, row.file_id),
                    Provider::Assrt => {
                        crate::assrt_subtitles::download(&assrt_token, row.file_id)
                    }
                    Provider::Shooter => row
                        .download_url
                        .as_deref()
                        .ok_or_else(|| "Shooter.cn result has no download link.".to_owned())
                        .and_then(|url| {
                            crate::shooter_subtitles::download(
                                row.file_id,
                                row.release.trim_start_matches('.'),
                                url,
                            )
                        }),
                };
                match downloaded
                    .and_then(|(name, bytes)| save_download(&name, &bytes))
                {
                    Ok(path) => paths.push(path),
                    Err(error) => {
                        failure = Some(format!("Subtitle {} of {total}: {error}", index + 1));
                        break;
                    }
                }
            }
            let _ = crate::background::to_ui(weak_window, move |window| {
                let mut queued = 0_usize;
                let mut load_error = None;
                for path in paths {
                    match sender.send(riina_core::Command::AddSubtitle(
                        path.to_string_lossy().into_owned(),
                    )) {
                        Ok(()) => queued += 1,
                        Err(error) => {
                            load_error = Some(error.to_string());
                            break;
                        }
                    }
                }
                window.set_downloading(false);
                if let Some(error) = load_error {
                    window.set_status_text(format!("Downloaded {queued} of {total}; could not load the remaining subtitles: {error}").into());
                } else if let Some(error) = failure {
                    window.set_status_text(format!("Downloaded and queued {queued} of {total}. {error}").into());
                } else {
                    window.set_status_text(format!("Downloaded and queued {queued} subtitle{}.", if queued == 1 { "" } else { "s" }).into());
                    let _ = window.hide();
                }
                if let Some(app) = weak_app.upgrade() {
                    app.window().request_redraw();
                }
            });
        });
        if let Err(error) = started {
            window.set_downloading(false);
            window.set_status_text(format!("Could not start subtitle download: {error}").into());
        }
    });
}

fn install_auxiliary_actions(app: &PlayerWindow, window: &OnlineSubtitlesWindow) {
    let weak_window = window.as_weak();
    window.on_cancel(move || {
        if let Some(window) = weak_window.upgrade() {
            let _ = window.hide();
        }
    });
    let weak_app = app.as_weak();
    window.on_open_assrt_site(move || {
        if let Some(app) = weak_app.upgrade()
            && let Err(error) = crate::platform::open_http_url_in_browser("https://assrt.net")
        {
            app.set_status(format!("Could not open Assrt.net: {error}").into());
        }
    });
    let weak_app = app.as_weak();
    window.on_open_shooter_site(move || {
        if let Some(app) = weak_app.upgrade()
            && let Err(error) = crate::platform::open_http_url_in_browser("https://www.shooter.cn")
        {
            app.set_status(format!("Could not open Shooter.cn: {error}").into());
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
            let credentials_available = match settings.provider {
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
                    .is_none_or(|entry| entry.filename.contains("://"))
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
    if !app.get_open_subtitles_auto_search() {
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
    fn automatic_search_timer_tracks_the_live_setting() -> Result<(), slint::PlatformError> {
        i_slint_backend_testing::init_no_event_loop();
        let app = PlayerWindow::new()?;
        let playback = Arc::new(Mutex::new(Arc::new(riina_core::Playback::default())));
        let readiness = Arc::new(Readiness::default());
        let requested = Rc::new(Cell::new(false));
        let timer = install_automatic_search(&app, playback, readiness, requested.clone());
        assert!(!timer.running());

        app.set_open_subtitles_auto_search(true);
        i_slint_backend_testing::mock_elapsed_time(Duration::ZERO);
        assert!(timer.running());
        i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(500));
        assert!(!requested.get());

        app.set_open_subtitles_auto_search(false);
        i_slint_backend_testing::mock_elapsed_time(Duration::ZERO);
        assert!(!timer.running());

        app.set_open_subtitles_auto_search(true);
        i_slint_backend_testing::mock_elapsed_time(Duration::ZERO);
        assert!(timer.running());
        drop(timer);
        app.set_open_subtitles_auto_search(false);
        i_slint_backend_testing::mock_elapsed_time(Duration::ZERO);
        Ok(())
    }
}
