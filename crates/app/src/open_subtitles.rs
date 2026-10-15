//! Bounded client for the OpenSubtitles REST endpoints used by the player.

use riina_core::online_subtitles::{Provider, ResultSelection};
use serde::{Deserialize, Serialize};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::{
    cell::{Cell, RefCell},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use ureq::Agent;
use url::Url;

const API_ROOT: &str = "https://api.opensubtitles.com/api/v1/";
const MAX_JSON_BYTES: u64 = 8 * 1024 * 1024;
const MAX_SUBTITLE_BYTES: u64 = 16 * 1024 * 1024;
const HASH_CHUNK_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ResultRow {
    pub(crate) provider: Provider,
    pub(crate) file_id: u64,
    pub(crate) file_name: String,
    pub(crate) language: String,
    pub(crate) release: String,
    pub(crate) year: Option<u16>,
    pub(crate) downloads: Option<u64>,
    pub(crate) rating: Option<f32>,
    pub(crate) format: Option<String>,
    pub(crate) uploaded_at: Option<String>,
    pub(crate) download_url: Option<String>,
}

#[derive(Serialize)]
struct DownloadRequest {
    file_id: u64,
}

#[derive(Deserialize)]
struct SearchResponse {
    #[serde(default)]
    data: Vec<SearchEntry>,
}

#[derive(Deserialize)]
struct SearchEntry {
    #[serde(default)]
    attributes: SearchAttributes,
}

#[derive(Default, Deserialize)]
struct SearchAttributes {
    #[serde(default)]
    download_count: u64,
    #[serde(default)]
    feature_details: FeatureDetails,
    #[serde(default)]
    files: Vec<SubtitleFile>,
    #[serde(default)]
    language: String,
    #[serde(default)]
    ratings: Option<f32>,
    #[serde(default)]
    release: Option<String>,
}

#[derive(Default, Deserialize)]
struct FeatureDetails {
    #[serde(default)]
    title: String,
    #[serde(default)]
    year: Option<u16>,
}

#[derive(Deserialize)]
struct SubtitleFile {
    file_id: u64,
    #[serde(default)]
    file_name: String,
}

#[derive(Deserialize)]
struct DownloadResponse {
    link: String,
    file_name: String,
}

pub(crate) fn search(
    api_key: &str,
    media_path: &Path,
    title: &str,
    languages: &str,
) -> Result<Vec<ResultRow>, String> {
    let key = checked_api_key(api_key)?;
    let title = title.trim();
    if title.is_empty() {
        return Err("Enter a title to search for subtitles.".into());
    }
    let languages = languages
        .split(',')
        .map(str::trim)
        .filter(|language| !language.is_empty())
        .collect::<Vec<_>>();
    let mut url = Url::parse(&format!("{API_ROOT}subtitles"))
        .map_err(|error| format!("Could not prepare subtitle search: {error}"))?;
    {
        let mut query = url.query_pairs_mut();
        query.append_pair("query", title);
        if !languages.is_empty() {
            query.append_pair("languages", &languages.join(","));
        }
        if let Some((hash, size)) = movie_hash(media_path)? {
            query.append_pair("moviehash", &hash);
            query.append_pair("moviebytesize", &size.to_string());
        }
    }
    let bytes = request(&agent(), &url, &key)?.body;
    let response: SearchResponse = serde_json::from_slice(&bytes)
        .map_err(|error| format!("OpenSubtitles returned invalid search data: {error}"))?;
    Ok(response
        .data
        .into_iter()
        .filter_map(|entry| {
            let file = entry.attributes.files.into_iter().next()?;
            Some(ResultRow {
                provider: Provider::OpenSubtitles,
                file_id: file.file_id,
                file_name: file.file_name,
                language: entry.attributes.language,
                release: entry
                    .attributes
                    .release
                    .filter(|release| !release.is_empty())
                    .unwrap_or(entry.attributes.feature_details.title),
                year: entry.attributes.feature_details.year,
                downloads: Some(entry.attributes.download_count),
                rating: entry.attributes.ratings,
                format: None,
                uploaded_at: None,
                download_url: None,
            })
        })
        .collect())
}

fn search_provider(
    provider: Provider,
    opensubtitles_key: &str,
    assrt_token: &str,
    media_path: &Path,
    title: &str,
    languages: &str,
) -> Result<Vec<ResultRow>, String> {
    match provider {
        Provider::OpenSubtitles => search(opensubtitles_key, media_path, title, languages),
        Provider::Assrt => crate::assrt_subtitles::search(assrt_token, title).map(|rows| {
            rows.into_iter()
                .map(|row| ResultRow {
                    provider: Provider::Assrt,
                    file_id: row.id,
                    file_name: row.file_name,
                    language: row.language,
                    release: row.release,
                    year: None,
                    downloads: None,
                    rating: None,
                    format: None,
                    uploaded_at: (!row.upload_time.is_empty()).then_some(row.upload_time),
                    download_url: None,
                })
                .collect()
        }),
        Provider::Shooter => crate::shooter_subtitles::search(media_path).map(|rows| {
            rows.into_iter()
                .map(|row| ResultRow {
                    provider: Provider::Shooter,
                    file_id: row.id,
                    file_name: row.description,
                    language: row.language,
                    release: String::new(),
                    year: None,
                    downloads: None,
                    rating: None,
                    format: Some(row.extension),
                    uploaded_at: None,
                    download_url: Some(row.download_url),
                })
                .collect()
        }),
    }
}

fn provider_name(provider: Provider) -> &'static str {
    match provider {
        Provider::OpenSubtitles => "OpenSubtitles",
        Provider::Assrt => "Assrt.net",
        Provider::Shooter => "Shooter.cn",
    }
}

fn provider_from_index(index: i32) -> Provider {
    match index {
        1 => Provider::Assrt,
        2 => Provider::Shooter,
        _ => Provider::OpenSubtitles,
    }
}

fn provider_index(provider: Provider) -> i32 {
    match provider {
        Provider::OpenSubtitles => 0,
        Provider::Assrt => 1,
        Provider::Shooter => 2,
    }
}

pub(crate) fn download(api_key: &str, file_id: u64) -> Result<(String, Vec<u8>), String> {
    let key = checked_api_key(api_key)?;
    let url = Url::parse(&format!("{API_ROOT}download"))
        .map_err(|error| format!("Could not prepare subtitle download: {error}"))?;
    let body = serde_json::to_vec(&DownloadRequest { file_id })
        .map_err(|error| format!("Could not prepare subtitle download: {error}"))?;
    let response = request_json(&agent(), &url, &key, &body)?;
    let response: DownloadResponse = serde_json::from_slice(&response)
        .map_err(|error| format!("OpenSubtitles returned invalid download data: {error}"))?;
    let download_url = Url::parse(&response.link)
        .map_err(|error| format!("OpenSubtitles returned an invalid download link: {error}"))?;
    if download_url.scheme() != "https"
        || download_url.host_str().is_none_or(|host| {
            !(host == "opensubtitles.com" || host.ends_with(".opensubtitles.com"))
        })
    {
        return Err("OpenSubtitles returned a download link outside its secure service.".into());
    }
    let file_name = safe_file_name(&response.file_name);
    let mut response = agent()
        .get(download_url.as_str())
        .header("User-Agent", "Riina/0.3.0")
        .call()
        .map_err(|error| format!("Could not download subtitle file: {error}"))?;
    let bytes = response
        .body_mut()
        .with_config()
        .limit(MAX_SUBTITLE_BYTES)
        .read_to_vec()
        .map_err(|error| format!("Could not read subtitle file: {error}"))?;
    if bytes.is_empty() {
        return Err("OpenSubtitles returned an empty subtitle file.".into());
    }
    Ok((file_name, bytes))
}

pub(crate) fn connect_player(
    app: &crate::PlayerWindow,
    sender: riina_mpv::CommandSender,
    playback: Arc<Mutex<riina_core::Playback>>,
    subtitle_discovery_ready: Arc<AtomicU64>,
) -> (
    Rc<RefCell<Option<crate::OnlineSubtitlesWindow>>>,
    Rc<slint::Timer>,
) {
    #[derive(Clone, Default)]
    struct SearchContext {
        media_path: PathBuf,
        languages: String,
        provider: Provider,
    }

    let window_slot = Rc::new(RefCell::new(None::<crate::OnlineSubtitlesWindow>));
    let context = Rc::new(RefCell::new(SearchContext::default()));
    let result_rows = Arc::new(Mutex::new(Vec::<ResultRow>::new()));
    let result_selection = Rc::new(RefCell::new(ResultSelection::default()));
    let auto_search_request = Rc::new(Cell::new(false));
    let requested_provider = Rc::new(Cell::new(None::<Provider>));
    let app_weak = app.as_weak();
    let app_weak_for_provider = app.as_weak();
    let window_slot_callback = window_slot.clone();
    let context_callback = context.clone();
    let rows_callback = result_rows.clone();
    let sender_callback = sender.clone();
    let playback_callback = playback.clone();
    let auto_search_request_for_open = auto_search_request.clone();
    let requested_provider_for_open = requested_provider.clone();
    app.on_find_online_subtitles(move || {
        let should_search_after_open = auto_search_request_for_open.replace(false);
        let requested_provider = requested_provider_for_open.replace(None);
        let Some(app) = app_weak.upgrade() else {
            return;
        };
        let current = playback_callback
            .lock()
            .ok()
            .and_then(|state| state.playlist.iter().find(|entry| entry.current).cloned());
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
        *context_callback.borrow_mut() = SearchContext {
            media_path: path,
            languages: app.get_subtitle_preferred_languages().to_string(),
            provider,
        };
        let window = if let Some(window) = window_slot_callback.borrow().as_ref() {
            window.clone_strong()
        } else {
            let window = match crate::OnlineSubtitlesWindow::new() {
                Ok(window) => window,
                Err(error) => {
                    app.set_status(
                        format!("Could not open online subtitle search: {error}").into(),
                    );
                    return;
                }
            };
            crate::appearance_preferences::install_system_typography(&window);
            let weak_window = window.as_weak();
            let weak_app = app.as_weak();
            let search_context = context_callback.clone();
            let rows = rows_callback.clone();
            let selection = result_selection.clone();
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
                if let Ok(mut rows) = rows.lock() {
                    rows.clear();
                }
                window.set_results(ModelRc::new(VecModel::from(
                    Vec::<crate::OnlineSubtitleRow>::new(),
                )));
                let weak_window = window.as_weak();
                let rows = rows.clone();
                std::thread::spawn(move || {
                    let result = search_provider(
                        provider,
                        &key,
                        &assrt_token,
                        &search_context.media_path,
                        &query,
                        &search_context.languages,
                    );
                    let _ = slint::invoke_from_event_loop(move || {
                        let Some(window) = weak_window.upgrade() else {
                            return;
                        };
                        window.set_searching(false);
                        match result {
                            Ok(found) => {
                                let count = found.len();
                                let model = ui_rows(&found, &ResultSelection::default());
                                if let Ok(mut rows) = rows.lock() {
                                    *rows = found;
                                }
                                window.set_selected_count(0);
                                window.set_results(ModelRc::new(VecModel::from(model)));
                                window.set_status_text(if count == 0 {
                                    format!("No matching subtitles found on {}.", provider_name(provider)).into()
                                } else {
                                    format!("{count} subtitle matches found on {}.", provider_name(provider)).into()
                                });
                            }
                            Err(error) => window.set_status_text(error.into()),
                        }
                    });
                });
            });
            let weak_window = window.as_weak();
            let rows = rows_callback.clone();
            let selection = result_selection.clone();
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
                    format!("{selected_count} subtitles selected. Use Ctrl/Command-click to change selection.").into()
                } else if selected_count == 1 {
                    "1 subtitle selected.".into()
                } else {
                    "No subtitles selected.".into()
                });
            });

            let weak_window = window.as_weak();
            let rows = rows_callback.clone();
            let selection = result_selection.clone();
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

            let weak_window = window.as_weak();
            let weak_app = app.as_weak();
            let rows = rows_callback.clone();
            let selection = result_selection.clone();
            let sender = sender_callback.clone();
            window.on_download_selected(move || {
                let (Some(window), Some(app)) = (weak_window.upgrade(), weak_app.upgrade()) else {
                    return;
                };
                let selected = selection.borrow().indices();
                let selected_rows = rows.lock().ok().map(|rows| {
                    selected
                        .iter()
                        .filter_map(|index| rows.get(*index).cloned())
                        .collect::<Vec<_>>()
                }).unwrap_or_default();
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
                std::thread::spawn(move || {
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
                    let _ = slint::invoke_from_event_loop(move || {
                        let Some(window) = weak_window.upgrade() else {
                            return;
                        };
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
            });
            let weak_window = window.as_weak();
            window.on_cancel(move || {
                if let Some(window) = weak_window.upgrade() {
                    let _ = window.hide();
                }
            });
            let weak_app = app.as_weak();
            window.on_open_assrt_site(move || {
                if let Some(app) = weak_app.upgrade()
                    && let Err(error) =
                        crate::platform::open_http_url_in_browser("https://assrt.net")
                {
                    app.set_status(format!("Could not open Assrt.net: {error}").into());
                }
            });
            let weak_app = app.as_weak();
            window.on_open_shooter_site(move || {
                if let Some(app) = weak_app.upgrade()
                    && let Err(error) =
                        crate::platform::open_http_url_in_browser("https://www.shooter.cn")
                {
                    app.set_status(format!("Could not open Shooter.cn: {error}").into());
                }
            });
            crate::platform::center_auxiliary_window(&window, &app);
            *window_slot_callback.borrow_mut() = Some(window.clone_strong());
            window
        };
        window.set_query(title.into());
        window.set_results(ModelRc::new(VecModel::from(
            Vec::<crate::OnlineSubtitleRow>::new(),
        )));
        result_rows.lock().map(|mut rows| rows.clear()).ok();
        result_selection.borrow_mut().clear();
        window.set_selected_count(0);
        window.set_provider_index(provider_index(provider));
        window.set_status_text(
            if provider == Provider::Shooter {
                "Search Shooter.cn by the local video's sampled content hash.".into()
            } else {
                format!("Search {} by title or filename.", provider_name(provider)).into()
            },
        );
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
    app.on_find_online_subtitles_with_provider(move |index| {
        let Some(app) = app_weak_for_provider.upgrade() else {
            return;
        };
        requested_provider_for_menu.set(Some(provider_from_index(index)));
        app.invoke_find_online_subtitles();
    });

    let auto_seen = Rc::new(Cell::new(None::<u64>));
    let auto_pending = Rc::new(RefCell::new(None::<(u64, std::time::Instant)>));
    let auto_app = app.as_weak();
    let auto_playback = playback;
    let pending_search = auto_pending.clone();
    let searched_generation = auto_seen.clone();
    let trigger_search = auto_search_request.clone();
    let auto_timer = Rc::new(slint::Timer::default());
    auto_timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(500),
        move || {
            let Some(app) = auto_app.upgrade() else {
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
                *pending_search.borrow_mut() = None;
                return;
            }
            let Ok(state) = auto_playback.lock().map(|state| state.clone()) else {
                return;
            };
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
                *pending_search.borrow_mut() = None;
                return;
            }
            if state
                .tracks
                .iter()
                .any(|track| track.kind == riina_core::TrackKind::Sub)
            {
                *pending_search.borrow_mut() = None;
                searched_generation.set(Some(state.generation));
                return;
            }
            if auto_load.mode != riina_core::subtitle_controls::AutoLoadMode::Disabled
                && subtitle_discovery_ready.load(Ordering::Acquire) != state.generation
            {
                *pending_search.borrow_mut() = None;
                return;
            }
            let now = std::time::Instant::now();
            let mut pending = pending_search.borrow_mut();
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
            app.invoke_find_online_subtitles();
        },
    );
    (window_slot, auto_timer)
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

fn save_download(name: &str, bytes: &[u8]) -> Result<PathBuf, String> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let root = dirs::cache_dir()
        .ok_or("The user cache directory is unavailable")?
        .join("riina")
        .join("online-subtitles");
    std::fs::create_dir_all(&root)
        .map_err(|error| format!("Could not prepare subtitle cache: {error}"))?;
    let name = safe_file_name(name);
    for _ in 0..16 {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| format!("Could not name downloaded subtitle: {error}"))?
            .as_nanos();
        let path = root.join(format!(
            "{stamp}-{}-{name}",
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("Could not save downloaded subtitle: {error}")),
        };
        if let Err(error) = file.write_all(bytes) {
            let _ = std::fs::remove_file(&path);
            return Err(format!("Could not save downloaded subtitle: {error}"));
        }
        return Ok(path);
    }
    Err("Could not allocate a unique subtitle cache file".into())
}

struct ResponseBody {
    body: Vec<u8>,
}

fn request(agent: &Agent, url: &Url, api_key: &str) -> Result<ResponseBody, String> {
    let mut response = agent
        .get(url.as_str())
        .header("Api-Key", api_key)
        .header("User-Agent", "Riina/0.3.0")
        .header("Accept", "application/json")
        .call()
        .map_err(|error| format_api_error("search", error))?;
    let body = response
        .body_mut()
        .with_config()
        .limit(MAX_JSON_BYTES)
        .read_to_vec()
        .map_err(|error| format!("Could not read OpenSubtitles search results: {error}"))?;
    Ok(ResponseBody { body })
}

fn request_json(agent: &Agent, url: &Url, api_key: &str, body: &[u8]) -> Result<Vec<u8>, String> {
    let mut response = agent
        .post(url.as_str())
        .header("Api-Key", api_key)
        .header("User-Agent", "Riina/0.3.0")
        .header("Accept", "application/json")
        .header("Content-Type", "application/json")
        .send(body)
        .map_err(|error| format_api_error("download request", error))?;
    response
        .body_mut()
        .with_config()
        .limit(MAX_JSON_BYTES)
        .read_to_vec()
        .map_err(|error| format!("Could not read OpenSubtitles download response: {error}"))
}

fn agent() -> Agent {
    Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(30)))
        .build()
        .new_agent()
}

fn checked_api_key(api_key: &str) -> Result<String, String> {
    let key = api_key.trim();
    if key.is_empty() || key.len() > 512 || key.chars().any(char::is_control) {
        return Err("Enter a valid OpenSubtitles API key in Settings > Subtitles.".into());
    }
    Ok(key.to_owned())
}

fn format_api_error(action: &str, error: ureq::Error) -> String {
    match error {
        ureq::Error::StatusCode(401 | 403) => {
            "OpenSubtitles rejected the API key or this request.".into()
        }
        ureq::Error::StatusCode(406) => {
            "OpenSubtitles download quota is unavailable for this account.".into()
        }
        ureq::Error::StatusCode(429) => "OpenSubtitles rate limit reached. Try again later.".into(),
        ureq::Error::StatusCode(status) => {
            format!("OpenSubtitles {action} failed with HTTP {status}.")
        }
        error => format!("OpenSubtitles {action} failed: {error}"),
    }
}

fn movie_hash(path: &Path) -> Result<Option<(String, u64)>, String> {
    if path.to_string_lossy().contains("://") {
        return Ok(None);
    }
    let mut file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "Could not inspect media for subtitle matching: {error}"
            ));
        }
    };
    let size = file
        .metadata()
        .map_err(|error| format!("Could not inspect media for subtitle matching: {error}"))?
        .len();
    if size < (HASH_CHUNK_BYTES * 2) as u64 {
        return Ok(None);
    }
    let mut sum = size;
    let mut chunk = [0_u8; HASH_CHUNK_BYTES];
    file.read_exact(&mut chunk)
        .map_err(|error| format!("Could not read media hash data: {error}"))?;
    for word in chunk.as_chunks::<8>().0 {
        sum = sum.wrapping_add(u64::from_le_bytes(*word));
    }
    file.seek(SeekFrom::End(-(HASH_CHUNK_BYTES as i64)))
        .map_err(|error| format!("Could not seek media hash data: {error}"))?;
    file.read_exact(&mut chunk)
        .map_err(|error| format!("Could not read media hash data: {error}"))?;
    for word in chunk.as_chunks::<8>().0 {
        sum = sum.wrapping_add(u64::from_le_bytes(*word));
    }
    Ok(Some((format!("{sum:016x}"), size)))
}

fn safe_file_name(name: &str) -> String {
    let leaf = Path::new(name)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("subtitle.srt");
    let clean = leaf
        .chars()
        .map(|character| {
            if character.is_control()
                || matches!(
                    character,
                    '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
                )
            {
                '_'
            } else {
                character
            }
        })
        .collect::<String>();
    if Path::new(&clean).extension().is_some_and(|extension| {
        matches!(
            extension.to_string_lossy().to_ascii_lowercase().as_str(),
            "srt" | "ass" | "ssa" | "vtt" | "sub" | "sup" | "idx"
        )
    }) {
        clean
    } else {
        "subtitle.srt".into()
    }
}

#[cfg(test)]
mod provider_tests {
    use super::{Provider, provider_from_index, provider_index, provider_name, safe_file_name};

    #[test]
    fn all_online_subtitle_providers_have_stable_selection_indices() {
        for (index, provider) in [Provider::OpenSubtitles, Provider::Assrt, Provider::Shooter]
            .into_iter()
            .enumerate()
        {
            assert_eq!(provider_index(provider), index as i32);
            assert_eq!(provider_from_index(index as i32), provider);
            assert!(!provider_name(provider).is_empty());
        }
        assert_eq!(provider_from_index(-1), Provider::OpenSubtitles);
    }

    #[test]
    fn subtitle_cache_keeps_supported_binary_and_indexed_formats() {
        assert_eq!(safe_file_name("subtitles.sup"), "subtitles.sup");
        assert_eq!(safe_file_name("subtitles.idx"), "subtitles.idx");
        assert_eq!(safe_file_name("subtitles.zip"), "subtitle.srt");
    }
}
