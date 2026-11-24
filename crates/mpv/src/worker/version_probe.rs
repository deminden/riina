//! Query an installed engine's versions without opening a media or video surface.

use super::handle::Handle;
use crate::{
    EngineError, MpvPropertyFormat,
    client_request::Operation,
    ffi::{Api, event_id},
};
use std::{
    path::Path,
    time::{Duration, Instant},
};

pub struct EngineVersions {
    pub mpv: String,
    pub ffmpeg: String,
}

/// Read version properties from a short-lived libmpv client on a background thread.
pub fn read_engine_versions(library: &Path) -> Result<EngineVersions, EngineError> {
    #[cfg(windows)]
    let _apartment = crate::windows_apartment::Apartment::initialize()?;
    let mut probe = Handle::new(Api::load(library)?)?;
    for (name, value) in [
        (c"config", c"no"),
        (c"load-scripts", c"no"),
        (c"ao", c"null"),
        (c"vo", c"null"),
    ] {
        probe.set_option(name, value)?;
    }
    probe.initialize()?;

    for (id, name) in [(1, c"mpv-version"), (2, c"ffmpeg-version")] {
        probe.request_property(
            id,
            &Operation::Read {
                name: name.into(),
                format: MpvPropertyFormat::Text,
            },
        )?;
    }

    let deadline = Instant::now() + Duration::from_secs(2);
    let mut responses = [None, None];
    while let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
        let Some(event) = probe.wait_timeout(remaining) else {
            break;
        };
        if event.id() != event_id::GET_PROPERTY_REPLY {
            continue;
        }
        let slot = match event.userdata() {
            1 => &mut responses[0],
            2 => &mut responses[1],
            _ => continue,
        };
        let value = event.client_reply()?;
        let text = value
            .as_str()
            .filter(|text| !text.is_empty())
            .ok_or_else(|| EngineError::Native {
                call: "mpv_get_property_async",
                message: "The engine returned no version text".into(),
            })?;
        *slot = Some(text.to_owned());
        if responses.iter().all(Option::is_some)
            && let [Some(mpv), Some(ffmpeg)] = responses
        {
            return Ok(EngineVersions { mpv, ffmpeg });
        }
    }
    Err(EngineError::ClientTimeout)
}
