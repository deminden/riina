//! Decode still images with the same native codecs that encode screenshots.
use super::local_probe::LocalProbe;
use crate::ffi::event_id;
use riina_core::CapturedFrame;
use std::{
    path::Path,
    time::{Duration, Instant},
};

/// Obtain the first frame of a local screenshot on a background thread.
pub fn decode_screenshot(library: &Path, path: &Path) -> Option<CapturedFrame> {
    let mut decoder = LocalProbe::open(
        library,
        path,
        &[(c"audio", c"no"), (c"hwdec", c"no"), (c"keep-open", c"yes")],
    )?;
    let decoder = &mut decoder.handle;
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut requested = false;
    while Instant::now() < deadline {
        let Some(event) = decoder.wait_timeout(Duration::from_millis(40)) else {
            continue;
        };
        match event.id() {
            event_id::PLAYBACK_RESTART if !requested => {
                decoder
                    .command(
                        2,
                        &[c"screenshot-raw".into(), c"video".into(), c"rgba".into()],
                    )
                    .ok()?;
                requested = true;
            }
            event_id::COMMAND_REPLY if event.failed() => return None,
            event_id::COMMAND_REPLY if event.userdata() == 2 => return event.frame()?.ok(),
            event_id::SHUTDOWN | event_id::END_FILE | event_id::QUEUE_OVERFLOW => return None,
            _ => {}
        }
    }
    None
}
