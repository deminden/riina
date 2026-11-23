//! Bounded client for the OpenSubtitles REST endpoints used by the player.

use crate::http::{self, format_api_error};
use riina_core::online_subtitles::Provider;
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};
use ureq::Agent;
use url::Url;

mod window;
pub(crate) use window::{SearchWindowGuard, connect_player};

const API_ROOT: &str = "https://api.opensubtitles.com/api/v1/";
const MAX_JSON_BYTES: u64 = 8 * 1024 * 1024;
const MAX_SUBTITLE_BYTES: u64 = 16 * 1024 * 1024;
const HASH_CHUNK_BYTES: usize = 64 * 1024;
// The provider documents the "Application vVersion" form for its API.
const API_USER_AGENT: &str = concat!("Riina v", env!("CARGO_PKG_VERSION"));
const ERROR_OVERRIDES: &[(u16, &str)] = &[
    (401, "OpenSubtitles rejected the API key or this request."),
    (403, "OpenSubtitles rejected the API key or this request."),
    (
        406,
        "OpenSubtitles download quota is unavailable for this account.",
    ),
    (429, "OpenSubtitles rate limit reached. Try again later."),
];

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
            drop(file);
            if let Err(cleanup) = std::fs::remove_file(&path) {
                crate::diagnostics::warn!(
                    "Could not remove a partial subtitle download: {cleanup}"
                );
            }
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
        .header("User-Agent", API_USER_AGENT)
        .header("Accept", "application/json")
        .call()
        .map_err(|error| format_api_error("OpenSubtitles", "search", error, ERROR_OVERRIDES))?;
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
        .header("User-Agent", API_USER_AGENT)
        .header("Accept", "application/json")
        .header("Content-Type", "application/json")
        .send(body)
        .map_err(|error| {
            format_api_error("OpenSubtitles", "download request", error, ERROR_OVERRIDES)
        })?;
    response
        .body_mut()
        .with_config()
        .limit(MAX_JSON_BYTES)
        .read_to_vec()
        .map_err(|error| format!("Could not read OpenSubtitles download response: {error}"))
}

fn agent() -> Agent {
    http::agent(Duration::from_secs(30))
}

fn checked_api_key(api_key: &str) -> Result<String, String> {
    let key = api_key.trim();
    if key.is_empty() || key.len() > 512 || key.chars().any(char::is_control) {
        return Err("Enter a valid OpenSubtitles API key in Settings > Subtitles.".into());
    }
    Ok(key.to_owned())
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
