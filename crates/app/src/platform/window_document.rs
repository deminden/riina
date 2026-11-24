//! Name the playback window and update its native document identity when supported.

#[cfg(target_os = "macos")]
pub(crate) use super::macos::playback_title;

pub(super) fn network_display_name(reference: &str) -> Option<String> {
    let reference = riina_core::media_reference::validate_network_url(reference).ok()?;
    let url = url::Url::parse(reference).ok()?;
    let filename = url
        .path_segments()
        .and_then(|segments| segments.rev().find(|segment| !segment.is_empty()))
        .map(percent_encoding::percent_decode_str)
        .map(|name| name.decode_utf8_lossy().into_owned())
        .filter(|name| !name.chars().any(char::is_control));
    filename.or_else(|| url.host_str().map(str::to_owned))
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn playback_title(_window: &slint::Window, state: &riina_core::Playback) -> String {
    let filename = state
        .playlist
        .iter()
        .find(|entry| entry.current || entry.playing)
        .and_then(|entry| {
            network_display_name(&entry.filename).or_else(|| {
                std::path::Path::new(&entry.filename)
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
            })
        })
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| state.title.clone());
    if filename.is_empty() {
        "riina".to_owned()
    } else {
        format!("{filename} — riina")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn network_titles_decode_filenames_and_omit_signed_parameters() {
        assert_eq!(
            network_display_name("https://media.example/second%20clip.avi?token=x&signature=x")
                .as_deref(),
            Some("second clip.avi")
        );
        assert_eq!(
            network_display_name("https://media.example/?token=x").as_deref(),
            Some("media.example")
        );
        assert_eq!(network_display_name("C:\\Videos\\example.mkv"), None);
    }
}
