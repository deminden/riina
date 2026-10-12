//! Bounded client for the OpenSubtitles REST endpoints used by the player.

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
    pub(crate) file_id: u64,
    pub(crate) file_name: String,
    pub(crate) language: String,
    pub(crate) release: String,
    pub(crate) year: Option<u16>,
    pub(crate) downloads: u64,
    pub(crate) rating: Option<f32>,
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
                file_id: file.file_id,
                file_name: file.file_name,
                language: entry.attributes.language,
                release: entry
                    .attributes
                    .release
                    .filter(|release| !release.is_empty())
                    .unwrap_or(entry.attributes.feature_details.title),
                year: entry.attributes.feature_details.year,
                downloads: entry.attributes.download_count,
                rating: entry.attributes.ratings,
            })
        })
        .collect())
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
    }

    let window_slot = Rc::new(RefCell::new(None::<crate::OnlineSubtitlesWindow>));
    let context = Rc::new(RefCell::new(SearchContext::default()));
    let result_rows = Arc::new(Mutex::new(Vec::<ResultRow>::new()));
    let app_weak = app.as_weak();
    let window_slot_callback = window_slot.clone();
    let context_callback = context.clone();
    let rows_callback = result_rows.clone();
    let sender_callback = sender.clone();
    let playback_callback = playback.clone();
    app.on_find_online_subtitles(move || {
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
        *context_callback.borrow_mut() = SearchContext {
            media_path: path,
            languages: app.get_subtitle_preferred_languages().to_string(),
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
            let weak_window = window.as_weak();
            let weak_app = app.as_weak();
            let search_context = context_callback.clone();
            let rows = rows_callback.clone();
            window.on_search(move || {
                let (Some(window), Some(app)) = (weak_window.upgrade(), weak_app.upgrade()) else {
                    return;
                };
                let query = window.get_query().to_string();
                let key = app.get_open_subtitles_api_key().to_string();
                let search_context = search_context.borrow().clone();
                window.set_searching(true);
                window.set_status_text("Searching OpenSubtitles…".into());
                window.set_selected_index(-1);
                let weak_window = window.as_weak();
                let rows = rows.clone();
                std::thread::spawn(move || {
                    let result = search(
                        &key,
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
                                let model = found.iter().map(ui_row).collect::<Vec<_>>();
                                if let Ok(mut rows) = rows.lock() {
                                    *rows = found;
                                }
                                window.set_results(ModelRc::new(VecModel::from(model)));
                                window.set_status_text(if count == 0 {
                                    "No matching subtitles found.".into()
                                } else {
                                    format!("{count} subtitle matches found.").into()
                                });
                            }
                            Err(error) => window.set_status_text(error.into()),
                        }
                    });
                });
            });
            let weak_window = window.as_weak();
            let weak_app = app.as_weak();
            let rows = rows_callback.clone();
            let sender = sender_callback.clone();
            window.on_download(move |index| {
                let (Some(window), Some(app)) = (weak_window.upgrade(), weak_app.upgrade()) else {
                    return;
                };
                let Ok(index) = usize::try_from(index) else {
                    return;
                };
                let Some(row) = rows.lock().ok().and_then(|rows| rows.get(index).cloned()) else {
                    return;
                };
                let key = app.get_open_subtitles_api_key().to_string();
                window.set_downloading(true);
                window.set_status_text("Downloading subtitle…".into());
                let weak_window = window.as_weak();
                let weak_app = app.as_weak();
                let sender = sender.clone();
                std::thread::spawn(move || {
                    let result = download(&key, row.file_id)
                        .and_then(|(name, bytes)| save_download(&name, &bytes));
                    let _ = slint::invoke_from_event_loop(move || {
                        let Some(window) = weak_window.upgrade() else {
                            return;
                        };
                        window.set_downloading(false);
                        match result {
                            Ok(path) => match sender.send(riina_core::Command::AddSubtitle(
                                path.to_string_lossy().into_owned(),
                            )) {
                                Ok(()) => {
                                    window.set_status_text(
                                        "Subtitle added to the active media.".into(),
                                    );
                                    let _ = window.hide();
                                }
                                Err(error) => window.set_status_text(
                                    format!("Subtitle downloaded, but could not load it: {error}")
                                        .into(),
                                ),
                            },
                            Err(error) => window.set_status_text(error.into()),
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
            *window_slot_callback.borrow_mut() = Some(window.clone_strong());
            window
        };
        window.set_query(title.into());
        window.set_results(ModelRc::new(VecModel::from(
            Vec::<crate::OnlineSubtitleRow>::new(),
        )));
        window.set_selected_index(-1);
        window.set_status_text("Search OpenSubtitles by title or filename.".into());
        if let Err(error) = window.show() {
            app.set_status(format!("Could not show online subtitle search: {error}").into());
        }
    });

    let auto_seen = Rc::new(Cell::new(None::<u64>));
    let auto_pending = Rc::new(RefCell::new(None::<(u64, std::time::Instant)>));
    let auto_app = app.as_weak();
    let auto_playback = playback;
    let pending_search = auto_pending.clone();
    let searched_generation = auto_seen.clone();
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
            if !settings.auto_search || settings.api_key.trim().is_empty() {
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
            app.invoke_find_online_subtitles();
        },
    );
    (window_slot, auto_timer)
}

fn ui_row(row: &ResultRow) -> crate::OnlineSubtitleRow {
    crate::OnlineSubtitleRow {
        file_name: row.file_name.clone().into(),
        language: row.language.clone().into(),
        release: row.release.clone().into(),
        year: row
            .year
            .map(|year| year.to_string())
            .unwrap_or_default()
            .into(),
        downloads: row.downloads.to_string().into(),
        rating: row
            .rating
            .map(|rating| format!("{rating:.1}"))
            .unwrap_or_default()
            .into(),
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
            "srt" | "ass" | "ssa" | "vtt" | "sub"
        )
    }) {
        clean
    } else {
        "subtitle.srt".into()
    }
}
