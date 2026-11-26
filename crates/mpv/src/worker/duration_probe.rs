//! Read duration using a separate paused client without opening a presentation surface.

use super::local_probe::LocalProbe;
use crate::{MpvPropertyFormat, client_request::Operation, ffi::event_id};
use std::{
    path::Path,
    time::{Duration, Instant},
};

/// Probe a local media file on a background thread. Expired or unsupported media
/// leave duration unknown. The live player's engine is independent of this client.
pub fn read_local_duration(
    library: &Path,
    file: &Path,
    cancelled: impl Fn() -> bool,
) -> Option<f64> {
    if cancelled() {
        return None;
    }
    let mut decoder = LocalProbe::open(library, file, &[])?;
    let probe = &mut decoder.handle;

    let deadline = Instant::now() + Duration::from_secs(3);
    while !cancelled() && Instant::now() < deadline {
        let Some(event) = probe.wait_timeout(Duration::from_millis(40)) else {
            continue;
        };
        match event.id() {
            event_id::FILE_LOADED => {
                probe
                    .request_property(
                        2,
                        &Operation::Read {
                            name: c"duration".into(),
                            format: MpvPropertyFormat::Number,
                        },
                    )
                    .ok()?;
            }
            event_id::GET_PROPERTY_REPLY if event.userdata() == 2 => {
                return event
                    .client_reply()
                    .ok()?
                    .as_f64()
                    .filter(|seconds| seconds.is_finite() && *seconds > 0.0);
            }
            event_id::END_FILE | event_id::QUEUE_OVERFLOW => return None,
            event_id::COMMAND_REPLY if event.failed() => return None,
            _ => {}
        }
    }
    None
}
