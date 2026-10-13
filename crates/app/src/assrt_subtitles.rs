//! Bounded Assrt subtitle search and safe subtitle-file retrieval.

use serde::Deserialize;
use std::{
    collections::BTreeMap,
    io::{Cursor, Read},
    path::Path,
    time::Duration,
};
use ureq::Agent;
use url::Url;

const API_ROOT: &str = "https://api.assrt.net/v1/";
const MAX_RESPONSE_BYTES: u64 = 8 * 1024 * 1024;
const MAX_SUBTITLE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_RESULTS: usize = 15;
const MAX_ARCHIVE_ENTRIES: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResultRow {
    pub(crate) id: u64,
    pub(crate) file_name: String,
    pub(crate) language: String,
    pub(crate) release: String,
    pub(crate) upload_time: String,
}

#[derive(Deserialize)]
struct SearchEnvelope {
    status: i64,
    #[serde(default)]
    sub: SearchBucket,
}

#[derive(Default, Deserialize)]
struct SearchBucket {
    #[serde(default, deserialize_with = "deserialize_empty_or_vec")]
    subs: Vec<SearchEntry>,
}

#[derive(Default, Deserialize)]
struct SearchEntry {
    #[serde(default)]
    id: u64,
    #[serde(default)]
    native_name: String,
    #[serde(default)]
    videoname: String,
    #[serde(default)]
    subtype: String,
    #[serde(default)]
    upload_time: String,
    #[serde(default)]
    release_site: String,
    #[serde(default)]
    lang: Language,
}

#[derive(Default, Deserialize)]
struct Language {
    #[serde(default)]
    desc: String,
}

#[derive(Deserialize)]
struct DetailEnvelope {
    status: i64,
    #[serde(default)]
    sub: DetailBucket,
}

#[derive(Default, Deserialize)]
struct DetailBucket {
    #[serde(default, deserialize_with = "deserialize_empty_or_vec")]
    subs: Vec<DetailEntry>,
}

#[derive(Default, Deserialize)]
struct DetailEntry {
    #[serde(default)]
    filelist: Vec<DetailFile>,
}

#[derive(Default, Deserialize)]
struct DetailFile {
    #[serde(default)]
    url: String,
    #[serde(default, rename = "f")]
    file_name: String,
}

pub(crate) fn search(token: &str, query: &str) -> Result<Vec<ResultRow>, String> {
    let token = checked_token(token)?;
    let query = query.trim();
    if query.chars().count() < 3 {
        return Err("Assrt subtitle searches need at least three characters.".into());
    }
    let mut url = Url::parse(&format!("{API_ROOT}sub/search"))
        .map_err(|error| format!("Could not prepare Assrt search: {error}"))?;
    url.query_pairs_mut()
        .append_pair("q", query)
        .append_pair("cnt", &MAX_RESULTS.to_string());
    let response = request(&agent(), &url, token)?;
    let envelope: SearchEnvelope = serde_json::from_slice(&response)
        .map_err(|error| format!("Assrt returned invalid search data: {error}"))?;
    check_status(envelope.status)?;
    Ok(envelope
        .sub
        .subs
        .into_iter()
        .filter(|entry| entry.id > 0)
        .take(MAX_RESULTS)
        .map(|entry| ResultRow {
            id: entry.id,
            file_name: nonempty(entry.native_name, entry.videoname),
            language: entry.lang.desc,
            release: [entry.subtype, entry.release_site]
                .into_iter()
                .filter(|value| !value.trim().is_empty())
                .collect::<Vec<_>>()
                .join(" · "),
            upload_time: entry.upload_time,
        })
        .collect())
}

pub(crate) fn download(token: &str, id: u64) -> Result<(String, Vec<u8>), String> {
    let token = checked_token(token)?;
    if id == 0 {
        return Err("Assrt returned an invalid subtitle ID.".into());
    }
    let mut url = Url::parse(&format!("{API_ROOT}sub/detail"))
        .map_err(|error| format!("Could not prepare Assrt download: {error}"))?;
    url.query_pairs_mut().append_pair("id", &id.to_string());
    let response = request(&agent(), &url, token)?;
    let envelope: DetailEnvelope = serde_json::from_slice(&response)
        .map_err(|error| format!("Assrt returned invalid subtitle details: {error}"))?;
    check_status(envelope.status)?;
    let file = envelope
        .sub
        .subs
        .into_iter()
        .flat_map(|entry| entry.filelist)
        .find(|file| supported_subtitle_file(&file.file_name) || archive_file(&file.file_name))
        .ok_or_else(|| {
            "Assrt did not provide a supported subtitle file or ZIP archive.".to_owned()
        })?;
    let url = checked_download_url(&file.url)?;
    let mut response = agent()
        .get(url.as_str())
        .call()
        .map_err(|error| format!("Could not download the Assrt subtitle: {error}"))?;
    if response.status().is_redirection() {
        return Err("Assrt returned a redirect for its subtitle file; it was not followed.".into());
    }
    let bytes = response
        .body_mut()
        .with_config()
        .limit(MAX_SUBTITLE_BYTES)
        .read_to_vec()
        .map_err(|error| format!("Could not read the Assrt subtitle: {error}"))?;
    unpack_subtitle(&file.file_name, bytes)
}

fn unpack_subtitle(file_name: &str, bytes: Vec<u8>) -> Result<(String, Vec<u8>), String> {
    if bytes.is_empty() {
        return Err("Assrt returned an empty subtitle file.".into());
    }
    let is_zip = archive_file(file_name) || bytes.starts_with(b"PK\x03\x04");
    if !is_zip {
        return Ok((safe_file_name(file_name), bytes));
    }
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|_| "Assrt returned an invalid subtitle ZIP archive.".to_owned())?;
    if archive.len() > MAX_ARCHIVE_ENTRIES {
        return Err("Assrt's subtitle archive contains too many files.".into());
    }
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|_| "Assrt's subtitle archive could not be read.".to_owned())?;
        if entry.is_dir()
            || entry.is_symlink()
            || entry.enclosed_name().is_none()
            || !supported_subtitle_file(entry.name())
            || entry.size() == 0
            || entry.size() > MAX_SUBTITLE_BYTES
        {
            continue;
        }
        let name = entry
            .enclosed_name()
            .and_then(|path| path.file_name().map(|name| name.to_owned()))
            .and_then(|name| name.into_string().ok())
            .map(|name| safe_file_name(&name))
            .unwrap_or_else(|| "subtitle.srt".into());
        let mut content = Vec::with_capacity(entry.size() as usize);
        entry
            .take(MAX_SUBTITLE_BYTES + 1)
            .read_to_end(&mut content)
            .map_err(|_| "Assrt's subtitle archive entry could not be read.".to_owned())?;
        if content.is_empty() || content.len() as u64 > MAX_SUBTITLE_BYTES {
            continue;
        }
        return Ok((name, content));
    }
    Err("Assrt's archive contains no supported subtitle file.".into())
}

fn archive_file(name: &str) -> bool {
    Path::new(name)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("zip"))
}

fn checked_token(token: &str) -> Result<&str, String> {
    let token = token.trim();
    if token.len() != 32 || !token.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
        return Err("Enter a valid 32-character Assrt token in Settings > Subtitles.".into());
    }
    Ok(token)
}

fn request(agent: &Agent, url: &Url, token: &str) -> Result<Vec<u8>, String> {
    let mut response = agent
        .get(url.as_str())
        .header("Authorization", &format!("Bearer {token}"))
        .call()
        .map_err(|error| format!("Could not contact Assrt: {error}"))?;
    if response.status().is_redirection() {
        return Err("Assrt redirected its API request; the redirect was not followed.".into());
    }
    response
        .body_mut()
        .with_config()
        .limit(MAX_RESPONSE_BYTES)
        .read_to_vec()
        .map_err(|error| format!("Could not read Assrt's response: {error}"))
}

fn agent() -> Agent {
    Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(15)))
        .https_only(true)
        .max_redirects(0)
        .build()
        .into()
}

fn checked_download_url(raw: &str) -> Result<Url, String> {
    let mut url = Url::parse(raw)
        .map_err(|error| format!("Assrt returned an invalid download URL: {error}"))?;
    let host = url
        .host_str()
        .ok_or_else(|| "Assrt returned a download URL without a host.".to_owned())?;
    if url.scheme() == "http" && (host == "assrt.net" || host.ends_with(".assrt.net")) {
        url.set_scheme("https")
            .map_err(|()| "Assrt's download URL could not be secured.".to_owned())?;
    }
    let host = url.host_str().unwrap_or_default();
    if url.scheme() != "https"
        || !(host == "assrt.net"
            || host.ends_with(".assrt.net")
            || host == "makedie.me"
            || host.ends_with(".makedie.me"))
    {
        return Err("Assrt returned a download URL outside its secure service.".into());
    }
    Ok(url)
}

fn check_status(status: i64) -> Result<(), String> {
    match status {
        0 => Ok(()),
        101 => Err("Assrt needs a longer search phrase.".into()),
        20001 => Err("Assrt rejected the API token. Check it in Subtitles settings.".into()),
        20900 => Err("Assrt no longer has that subtitle.".into()),
        30900 => Err("Assrt's request quota has been reached. Try again later.".into()),
        code => Err(format!("Assrt returned error {code}.")),
    }
}

fn supported_subtitle_file(name: &str) -> bool {
    std::path::Path::new(name)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "srt" | "ass" | "ssa" | "vtt" | "sup"
            )
        })
}

fn safe_file_name(name: &str) -> String {
    let name = std::path::Path::new(name)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("subtitle.srt");
    if name.is_empty() || name == "." || name == ".." {
        "subtitle.srt".into()
    } else {
        name.chars()
            .filter(|character| !character.is_control() && !matches!(character, '/' | '\\'))
            .collect()
    }
}

fn nonempty(value: String, fallback: String) -> String {
    if value.trim().is_empty() {
        fallback
    } else {
        value
    }
}

fn deserialize_empty_or_vec<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Entries<T> {
        List(Vec<T>),
        EmptyObject(BTreeMap<String, serde::de::IgnoredAny>),
    }

    match Entries::deserialize(deserializer)? {
        Entries::List(entries) => Ok(entries),
        Entries::EmptyObject(object) if object.is_empty() => Ok(Vec::new()),
        Entries::EmptyObject(_) => Err(serde::de::Error::custom("expected an empty object")),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DetailEnvelope, SearchEnvelope, check_status, checked_download_url, safe_file_name,
        supported_subtitle_file, unpack_subtitle,
    };
    use std::io::Write;

    #[test]
    fn parses_assrt_search_fields_and_partial_language_data() {
        let parsed: SearchEnvelope = serde_json::from_str(
            r#"{"status":0,"sub":{"subs":[{"id":123456,"native_name":"Movie","videoname":"Movie.1080p","subtype":"Subrip(srt)","upload_time":"2025-01-02","release_site":"Fansub","vote_score":8,"lang":{"desc":"English"}},{"id":123457,"native_name":"No language"}]}}"#,
        )
        .unwrap();
        assert_eq!(parsed.status, 0);
        assert_eq!(parsed.sub.subs[0].videoname, "Movie.1080p");
        assert_eq!(parsed.sub.subs[0].lang.desc, "English");
        assert_eq!(parsed.sub.subs[1].lang.desc, "");
    }

    #[test]
    fn accepts_assrt_empty_object_for_no_results() {
        let parsed: SearchEnvelope =
            serde_json::from_str(r#"{"status":0,"sub":{"subs":{}}}"#).unwrap();
        assert!(parsed.sub.subs.is_empty());
    }

    #[test]
    fn parses_assrt_detail_file_lists_and_requires_a_service_status() {
        let parsed: DetailEnvelope = serde_json::from_str(
            r#"{"status":0,"sub":{"subs":[{"filelist":[{"url":"https://file0.assrt.net/onthefly/movie.srt","f":"movie.srt","s":"52KB"}]}]}}"#,
        )
        .unwrap();
        assert_eq!(parsed.status, 0);
        assert_eq!(parsed.sub.subs[0].filelist[0].file_name, "movie.srt");
        assert!(parsed.sub.subs[0].filelist[0].url.ends_with("movie.srt"));
        assert!(serde_json::from_str::<DetailEnvelope>(r#"{"sub":{"subs":{}}}"#).is_err());
    }

    #[test]
    fn maps_assrt_service_errors_to_recoverable_messages() {
        assert!(check_status(0).is_ok());
        assert!(check_status(20001).unwrap_err().contains("token"));
        assert!(check_status(30900).unwrap_err().contains("quota"));
    }

    #[test]
    fn accepts_only_a_32_character_ascii_alphanumeric_token() {
        assert!(super::checked_token("5IzWrb2J099vmA96ECQXwdRSe9xdoBUv").is_ok());
        assert!(super::checked_token("too-short").is_err());
        assert!(super::checked_token("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa!").is_err());
    }

    #[test]
    fn download_urls_are_https_and_confined_to_assrt_domains() {
        let url = checked_download_url("http://file0.assrt.net/download/1/movie.srt").unwrap();
        assert_eq!(url.scheme(), "https");
        assert!(checked_download_url("https://example.com/movie.srt").is_err());
        assert!(checked_download_url("http://assrt.net.evil.example/movie.srt").is_err());
    }

    #[test]
    fn only_directly_loadable_subtitle_files_are_selected() {
        assert!(supported_subtitle_file("Movie.SRT"));
        assert!(supported_subtitle_file("Movie.ass"));
        assert!(!supported_subtitle_file("Movie.rar"));
        assert_eq!(safe_file_name("../../movie.srt"), "movie.srt");
    }

    #[test]
    fn extracts_the_first_supported_subtitle_from_a_zip_in_memory() {
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        writer
            .start_file("../escape.srt", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"must be rejected").unwrap();
        writer
            .start_file(
                "link.srt",
                zip::write::SimpleFileOptions::default().unix_permissions(0o120777),
            )
            .unwrap();
        writer.write_all(b"must also be rejected").unwrap();
        writer
            .start_file(
                "extras/readme.txt",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        writer.write_all(b"not a subtitle").unwrap();
        writer
            .start_file(
                "folder/Movie.en.srt",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        writer
            .write_all(b"1\n00:00:01,000 --> 00:00:02,000\nHello\n")
            .unwrap();
        let mut archive = writer.finish().unwrap().into_inner();
        mark_zip_entry_as_symlink(&mut archive, b"link.srt");

        let (name, subtitle) = unpack_subtitle("download.zip", archive).unwrap();
        assert_eq!(name, "Movie.en.srt");
        assert!(String::from_utf8(subtitle).unwrap().contains("Hello"));
    }

    #[test]
    fn rejects_invalid_or_subtitle_free_archives() {
        assert!(unpack_subtitle("download.zip", b"not a zip".to_vec()).is_err());
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        writer
            .start_file("readme.txt", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"no subtitle here").unwrap();
        let archive = writer.finish().unwrap().into_inner();
        assert!(
            unpack_subtitle("download.zip", archive)
                .unwrap_err()
                .contains("no supported")
        );
    }

    fn mark_zip_entry_as_symlink(archive: &mut [u8], name: &[u8]) {
        let mut offset = 0;
        while let Some(relative) = archive[offset..]
            .windows(4)
            .position(|signature| signature == b"PK\x01\x02")
        {
            let header = offset + relative;
            let name_length =
                u16::from_le_bytes([archive[header + 28], archive[header + 29]]) as usize;
            let extra_length =
                u16::from_le_bytes([archive[header + 30], archive[header + 31]]) as usize;
            let comment_length =
                u16::from_le_bytes([archive[header + 32], archive[header + 33]]) as usize;
            let name_start = header + 46;
            if &archive[name_start..name_start + name_length] == name {
                archive[header + 38..header + 42]
                    .copy_from_slice(&((0o120777_u32) << 16).to_le_bytes());
                return;
            }
            offset = name_start + name_length + extra_length + comment_length;
        }
        panic!("ZIP central directory did not contain the test entry");
    }
}
