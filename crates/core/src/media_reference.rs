//! Validation shared by media dialogs, startup arguments, and playlist loading.

use url::Url;

/// Validate a network media URL while preserving its original spelling and
/// escaping for the native playback engine.
pub fn validate_network_url(value: &str) -> Result<&str, &'static str> {
    let value = value.trim();
    let parsed = Url::parse(value).map_err(|_| "Enter a valid http, https, rtsp or rtsps URL")?;
    if value.contains('\0')
        || value.chars().any(char::is_whitespace)
        || !matches!(parsed.scheme(), "http" | "https" | "rtsp" | "rtsps")
        || parsed.host_str().is_none_or(str::is_empty)
    {
        return Err("Enter a valid http, https, rtsp or rtsps URL");
    }
    Ok(value)
}

/// Return whether an argument uses a supported network scheme even when its
/// authority or the rest of the URL is malformed.
pub fn is_network_url_candidate(value: &str) -> bool {
    let Some((scheme, _)) = value.trim().split_once("://") else {
        return false;
    };
    matches!(
        scheme.to_ascii_lowercase().as_str(),
        "http" | "https" | "rtsp" | "rtsps"
    )
}
