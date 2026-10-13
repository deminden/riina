//! Bounded Shooter.cn lookup for subtitles matching local video content.

use md5::{Digest, Md5};
use serde::Deserialize;
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
    time::Duration,
};
use ureq::Agent;
use url::Url;

const API_URL: &str = "https://www.shooter.cn/api/subapi.php";
const MIN_MEDIA_BYTES: u64 = 12 * 1024;
const SAMPLE_BYTES: usize = 4096;
const MAX_RESULTS: usize = 100;
const MAX_API_BYTES: u64 = 8 * 1024 * 1024;
const MAX_SUBTITLE_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResultRow {
    pub(crate) id: u64,
    pub(crate) description: String,
    pub(crate) language: String,
    pub(crate) extension: String,
    pub(crate) download_url: String,
}

#[derive(Deserialize)]
struct ApiEntry {
    #[serde(default, rename = "Desc")]
    description: String,
    #[serde(default, rename = "Files")]
    files: Vec<ApiFile>,
}

#[derive(Deserialize)]
struct ApiFile {
    #[serde(default, rename = "Ext")]
    extension: String,
    #[serde(default, rename = "Link")]
    link: String,
}

pub(crate) fn search(media_path: &Path) -> Result<Vec<ResultRow>, String> {
    let path = media_path.to_str().ok_or_else(|| {
        "Shooter.cn search requires a local path representable as UTF-8.".to_owned()
    })?;
    let (hash, _) = shooter_hash(media_path)?;
    let body = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("filehash", &hash)
        .append_pair("pathinfo", path)
        .append_pair("format", "json")
        .finish();
    let mut response = agent()
        .post(API_URL)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("Accept", "application/json")
        .header("User-Agent", "Riina")
        .send(body.as_bytes())
        .map_err(|error| format_api_error("search", error))?;
    if response.status().is_redirection() {
        return Err(
            "Shooter.cn redirected the subtitle search; the redirect was not followed.".into(),
        );
    }
    let bytes = response
        .body_mut()
        .with_config()
        .limit(MAX_API_BYTES)
        .read_to_vec()
        .map_err(|error| format!("Could not read Shooter.cn search results: {error}"))?;
    if bytes == [0xff] || bytes.is_empty() {
        return Ok(Vec::new());
    }
    let entries: Vec<ApiEntry> = serde_json::from_slice(&bytes)
        .map_err(|error| format!("Shooter.cn returned invalid search data: {error}"))?;
    let mut rows = Vec::new();
    for (index, entry) in entries.into_iter().take(MAX_RESULTS).enumerate() {
        let Some(file) = entry.files.into_iter().find(|file| {
            is_supported_subtitle(&file.extension) && checked_download_url(&file.link).is_ok()
        }) else {
            continue;
        };
        let download_url = checked_download_url(&file.link)?;
        let description = clean_text(&entry.description, 240);
        rows.push(ResultRow {
            id: index as u64 + 1,
            description: if description.is_empty() {
                format!("Shooter subtitle {}", index + 1)
            } else {
                description
            },
            language: "—".into(),
            extension: file.extension.trim_start_matches('.').to_ascii_lowercase(),
            download_url,
        });
    }
    Ok(rows)
}

pub(crate) fn download(
    id: u64,
    extension: &str,
    download_url: &str,
) -> Result<(String, Vec<u8>), String> {
    let url = checked_download_url(download_url)?;
    let mut response = agent()
        .get(url.as_str())
        .call()
        .map_err(|error| format_api_error("download", error))?;
    if response.status().is_redirection() {
        return Err(
            "Shooter.cn redirected the subtitle download; the redirect was not followed.".into(),
        );
    }
    let bytes = response
        .body_mut()
        .with_config()
        .limit(MAX_SUBTITLE_BYTES)
        .read_to_vec()
        .map_err(|error| format!("Could not read the Shooter.cn subtitle: {error}"))?;
    if bytes.is_empty() {
        return Err("Shooter.cn returned an empty subtitle file.".into());
    }
    Ok((format!("shooter-{id}.{extension}"), bytes))
}

fn shooter_hash(path: &Path) -> Result<(String, u64), String> {
    let mut file = File::open(path)
        .map_err(|error| format!("Could not open local media for Shooter.cn matching: {error}"))?;
    let size = file
        .metadata()
        .map_err(|error| format!("Could not inspect local media for Shooter.cn matching: {error}"))?
        .len();
    if size < MIN_MEDIA_BYTES {
        return Err("Shooter.cn requires a local media file of at least 12 KiB.".into());
    }
    let offsets = [4096, size / 3 * 2, size / 3, size - 8192];
    let mut hash = String::with_capacity(4 * 32 + 3);
    let mut sample = [0_u8; SAMPLE_BYTES];
    for (index, offset) in offsets.into_iter().enumerate() {
        file.seek(SeekFrom::Start(offset))
            .and_then(|_| file.read_exact(&mut sample))
            .map_err(|error| {
                format!("Could not read local media for Shooter.cn matching: {error}")
            })?;
        if index != 0 {
            hash.push(';');
        }
        for byte in Md5::digest(sample).iter() {
            use std::fmt::Write as _;
            write!(hash, "{byte:02x}").expect("writing into a String cannot fail");
        }
    }
    Ok((hash, size))
}

fn agent() -> Agent {
    Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(20)))
        .https_only(true)
        .max_redirects(0)
        .build()
        .into()
}

fn checked_download_url(raw: &str) -> Result<String, String> {
    let url = Url::parse(raw)
        .map_err(|error| format!("Shooter.cn returned an invalid subtitle URL: {error}"))?;
    let host = url.host_str().unwrap_or_default();
    if url.scheme() != "https"
        || !(host == "shooter.cn" || host.ends_with(".shooter.cn"))
        || url.username() != ""
        || url.password().is_some()
    {
        return Err("Shooter.cn returned a download URL outside its secure service.".into());
    }
    Ok(url.to_string())
}

fn is_supported_subtitle(extension: &str) -> bool {
    matches!(
        extension
            .trim_start_matches('.')
            .to_ascii_lowercase()
            .as_str(),
        "srt" | "ass" | "ssa" | "vtt" | "sup" | "sub" | "idx"
    )
}

fn clean_text(value: &str, max_chars: usize) -> String {
    value
        .chars()
        .filter(|character| !character.is_control())
        .take(max_chars)
        .collect()
}

fn format_api_error(action: &str, error: ureq::Error) -> String {
    match error {
        ureq::Error::StatusCode(429) => {
            "Shooter.cn rate limited this request. Try again later.".into()
        }
        ureq::Error::StatusCode(status) => {
            format!("Shooter.cn {action} failed with HTTP {status}.")
        }
        error => format!("Shooter.cn {action} failed: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::{ApiEntry, checked_download_url, clean_text, is_supported_subtitle, shooter_hash};
    use std::io::Write;

    #[test]
    fn parses_shooter_response_and_filters_unsupported_or_external_files() {
        let json = br#"[
            {"Desc":"Release A","Files":[
                {"Ext":"zip","Link":"https://www.shooter.cn/a.zip"},
                {"Ext":"srt","Link":"https://dl.shooter.cn/a.srt"},
                {"Ext":"ass","Link":"https://elsewhere.example/a.ass"}
            ]},
            {"Desc":"unsafe","Files":[{"Ext":"srt","Link":"http://shooter.cn/a.srt"}]}
        ]"#;
        let entries: Vec<ApiEntry> = serde_json::from_slice(json).unwrap();
        let rows = entries
            .into_iter()
            .enumerate()
            .filter_map(|(index, entry)| {
                let file = entry.files.into_iter().find(|file| {
                    is_supported_subtitle(&file.extension)
                        && checked_download_url(&file.link).is_ok()
                })?;
                Some((
                    entry.description,
                    file.extension,
                    checked_download_url(&file.link).unwrap(),
                    index,
                ))
            })
            .collect::<Vec<_>>();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, "Release A");
        assert_eq!(rows[0].1, "srt");
        assert_eq!(rows[0].2, "https://dl.shooter.cn/a.srt");
    }

    #[test]
    fn hashes_only_four_bounded_samples_from_local_media() {
        let mut path = std::env::temp_dir();
        path.push(format!("riina-shooter-hash-{}.bin", std::process::id()));
        let mut file = std::fs::File::create(&path).unwrap();
        for value in 0..(16 * 1024) {
            file.write_all(&[(value % 251) as u8]).unwrap();
        }
        drop(file);
        let (hash, size) = shooter_hash(&path).unwrap();
        let _ = std::fs::remove_file(path);
        assert_eq!(size, 16 * 1024);
        assert_eq!(hash.split(';').count(), 4);
        assert!(hash.split(';').all(|part| part.len() == 32));
    }

    #[test]
    fn rejects_too_small_media_and_unsafe_download_urls() {
        let mut path = std::env::temp_dir();
        path.push(format!("riina-shooter-small-{}.bin", std::process::id()));
        std::fs::write(&path, [0_u8; 4096]).unwrap();
        assert!(shooter_hash(&path).unwrap_err().contains("12 KiB"));
        let _ = std::fs::remove_file(path);
        assert!(checked_download_url("http://www.shooter.cn/a.srt").is_err());
        assert!(checked_download_url("https://shooter.cn@attacker.example/a.srt").is_err());
        assert!(checked_download_url("https://attacker.example/a.srt").is_err());
    }

    #[test]
    fn metadata_text_is_bounded_and_has_no_controls() {
        assert_eq!(clean_text("A\nB\u{7}", 10), "AB");
        assert_eq!(clean_text("abcdef", 3), "abc");
    }
}
