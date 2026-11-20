//! Save the observed playlist as an M3U8 file from the File menu.

use crate::{NativeVideoHost, PlayerWindow};
use std::sync::{Arc, Mutex};

pub(crate) fn save_current_playlist(
    app: &PlayerWindow,
    host: Option<slint::Weak<NativeVideoHost>>,
    playback: &Arc<Mutex<riina_core::Playback>>,
) {
    let files = match playback.lock() {
        Ok(playback) => playback
            .playlist
            .iter()
            .map(|entry| entry.filename.clone())
            .collect::<Vec<_>>(),
        Err(_) => {
            app.set_status("Could not read the current playlist".into());
            return;
        }
    };
    if files.is_empty() {
        app.set_status("There is no playlist to save".into());
        return;
    }
    if files.iter().any(|path| path.contains(['\r', '\n', '\0'])) {
        app.set_status("The playlist contains a path that cannot be saved as M3U8".into());
        return;
    }
    let mut content = files.join("\n");
    content.push('\n');
    crate::platform::save_playlist_file(app, host, content);
}
