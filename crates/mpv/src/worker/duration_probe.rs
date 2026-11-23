//! Read duration using a separate paused client without opening a presentation surface.

use super::handle::Handle;
use crate::{
    MpvPropertyFormat,
    client_request::Operation,
    ffi::{Api, event_id},
};
use std::{
    ffi::CString,
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
    if cancelled() || !file.is_absolute() || !file.is_file() {
        return None;
    }
    #[cfg(windows)]
    let _apartment = crate::windows_apartment::Apartment::initialize().ok()?;
    let mut probe = Handle::new(Api::load(library).ok()?).ok()?;
    for (key, value) in [
        (c"config", c"no"),
        (c"load-scripts", c"no"),
        (c"autoload-files", c"no"),
        (c"access-references", c"no"),
        (c"demuxer-lavf-o", c"protocol_whitelist=file"),
        (c"ao", c"null"),
        (c"vo", c"null"),
        (c"pause", c"yes"),
    ] {
        probe.set_option(key, value).ok()?;
    }
    probe.initialize().ok()?;
    probe
        .command(1, &[c"loadfile".into(), CString::new(file.to_str()?).ok()?])
        .ok()?;

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
