//! Name the playback window and update its native document identity when supported.

#[cfg(target_os = "macos")]
pub(crate) use super::macos::playback_title;

#[cfg(not(target_os = "macos"))]
pub(crate) fn playback_title(_window: &slint::Window, state: &riina_core::Playback) -> String {
    let filename = state
        .playlist
        .iter()
        .find(|entry| entry.current || entry.playing)
        .and_then(|entry| std::path::Path::new(&entry.filename).file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| state.title.clone());
    if filename.is_empty() {
        "riina".to_owned()
    } else {
        format!("{filename} — riina")
    }
}
