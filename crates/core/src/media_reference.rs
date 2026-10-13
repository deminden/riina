//! Validation shared by media dialogs, startup arguments, and playlist loading.

use url::Url;

const STREAM_SCHEMES: &[&str] = &[
    "ftp", "ftps", "http", "https", "mms", "mmsh", "mmst", "rtmp", "rtmpe", "rtmps", "rtmpt",
    "rtmpte", "rtmpts", "rtp", "rtsp", "rtsps", "smb", "srt", "srtp", "tcp", "udp", "ytdl",
];

/// Validate a network media URL while preserving its original spelling and
/// escaping for the native playback engine.
pub fn validate_network_url(value: &str) -> Result<&str, &'static str> {
    let value = value.trim();
    let parsed = Url::parse(value).map_err(|_| "Enter a valid supported stream URL")?;
    if value.contains('\0')
        || value.chars().any(char::is_whitespace)
        || !STREAM_SCHEMES.contains(&parsed.scheme())
        || parsed.host_str().is_none_or(str::is_empty)
    {
        return Err("Enter a valid supported stream URL");
    }
    Ok(value)
}

/// Return whether an argument uses a supported network scheme even when its
/// authority or the rest of the URL is malformed.
pub fn is_network_url_candidate(value: &str) -> bool {
    let Some((scheme, _)) = value.trim().split_once("://") else {
        return false;
    };
    let scheme = scheme.to_ascii_lowercase();
    STREAM_SCHEMES.contains(&scheme.as_str())
}

#[cfg(test)]
mod tests {
    use super::{is_network_url_candidate, validate_network_url};

    #[test]
    fn accepts_common_engine_stream_protocols_without_rewriting_the_url() {
        for value in [
            "https://media.example/video.m3u8?token=a%2Fb",
            "ftp://media.example/archive.mkv",
            "rtmp://media.example/live/channel",
            "srt://media.example:9000?mode=caller",
            "udp://239.1.2.3:1234",
            "smb://nas.example/video/movie.mkv",
            "ytdl://https://video.example/watch?id=42",
        ] {
            assert!(is_network_url_candidate(value), "{value}");
            assert_eq!(validate_network_url(value), Ok(value));
        }
    }

    #[test]
    fn rejects_malformed_stream_urls_and_nonstream_schemes() {
        for (value, candidate) in [
            ("https://", true),
            ("rtsp:// media.example/live", true),
            ("javascript://example.invalid/run", false),
            ("file:///C:/Videos/movie.mkv", false),
        ] {
            assert_eq!(is_network_url_candidate(value), candidate, "{value}");
            assert!(validate_network_url(value).is_err(), "{value}");
        }
    }

    #[test]
    fn a_windows_drive_path_is_not_mistaken_for_a_stream_url() {
        let path = "C:/Videos/movie.mkv";
        assert!(!is_network_url_candidate(path));
        assert!(validate_network_url(path).is_err());
    }
}
