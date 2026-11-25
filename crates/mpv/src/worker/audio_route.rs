//! Keep recovery on the native event queue, alongside explicit device choices.
use super::Handle;
use crate::{
    MpvPropertyFormat, client_request::Operation, ffi::event_id, properties::decode_audio_devices,
};
use riina_core::{
    Phase, Playback,
    audio_preferences::{OutputDevice, device_alias},
};
use std::{
    ffi::CString,
    sync::Arc,
    time::{Duration, Instant},
};

const DISCOVERY_LIMIT: Duration = Duration::from_millis(200);

pub(super) fn apply_preferred_device(handle: &mut Handle, device: &str) -> Result<(), String> {
    let option = CString::new(device).map_err(|error| error.to_string())?;
    let request = Operation::Read {
        name: c"audio-device-list".to_owned(),
        format: MpvPropertyFormat::Node,
    };
    handle
        .request_property(0, &request)
        .map_err(|error| error.to_string())?;
    let deadline = Instant::now() + DISCOVERY_LIMIT;
    while let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
        let Some(event) = handle.wait_timeout(remaining) else {
            break;
        };
        if event.id() != event_id::GET_PROPERTY_REPLY || event.userdata() != 0 {
            continue;
        }
        let devices =
            decode_audio_devices(event.client_reply().map_err(|error| error.to_string())?)?;
        if devices.iter().any(|available| available.name == device) {
            return handle
                .set_option(c"audio-device", &option)
                .map_err(|error| error.to_string());
        }
        return Err(
            "The preferred audio device is unavailable. Using automatic device selection.".into(),
        );
    }
    Err("Audio device discovery timed out. Using automatic device selection.".into())
}

#[derive(Default)]
pub(super) struct Watch {
    snapshot: Option<Snapshot>,
    was_listed: bool,
}

struct Snapshot {
    selected: Option<String>,
    output: Option<String>,
    devices: Arc<Vec<OutputDevice>>,
    active: bool,
}

impl Watch {
    pub(super) fn observe(
        &mut self,
        playback: &Playback,
        waiting_for_selection: bool,
    ) -> Option<String> {
        let active = matches!(
            playback.phase,
            Phase::Playing | Phase::Paused | Phase::Buffering | Phase::Seeking | Phase::Ended
        );
        let same_selection = self
            .snapshot
            .as_ref()
            .is_some_and(|snapshot| snapshot.selected == playback.audio_device);
        if self.snapshot.as_ref().is_some_and(|snapshot| {
            same_selection
                && snapshot.output == playback.audio_output
                && snapshot.active == active
                && (Arc::ptr_eq(&snapshot.devices, &playback.audio_devices)
                    || snapshot.devices == playback.audio_devices)
        }) {
            return None;
        }
        self.snapshot = Some(Snapshot {
            selected: playback.audio_device.clone(),
            output: playback.audio_output.clone(),
            devices: playback.audio_devices.clone(),
            active,
        });
        let selected = playback.audio_device.as_deref().unwrap_or("auto");
        let has = |name: &str| {
            playback
                .audio_devices
                .iter()
                .any(|device| device.name == name)
        };
        let present = has(selected);
        self.was_listed = present || (same_selection && self.was_listed);
        if !active
            || waiting_for_selection
            || selected == "auto"
            || playback.audio_devices.is_empty()
        {
            return None;
        }
        if let Some(alias) = playback
            .audio_output
            .as_deref()
            .and_then(|driver| device_alias(selected, driver))
            && has(&alias)
        {
            return Some(alias);
        }
        // An advanced mpv address is permitted even if its driver can't enumerate it.
        (self.was_listed && !present).then(|| "auto".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn devices(names: &[&str]) -> Arc<Vec<OutputDevice>> {
        Arc::new(
            names
                .iter()
                .map(|name| OutputDevice {
                    name: (*name).into(),
                    description: (*name).into(),
                })
                .collect(),
        )
    }

    #[test]
    fn device_loss_waits_for_active_playback_and_does_not_retry_on_clock_ticks() {
        let mut state = Playback {
            audio_device: Some("wasapi/headset".into()),
            audio_devices: devices(&["auto", "wasapi/headset"]),
            ..Playback::default()
        };
        let mut watch = Watch::default();
        assert_eq!(watch.observe(&state, false), None);
        state.audio_devices = devices(&["auto"]);
        assert_eq!(watch.observe(&state, false), None);
        state.phase = Phase::Paused;
        assert_eq!(watch.observe(&state, false).as_deref(), Some("auto"));
        state.position = Some(10.);
        assert_eq!(watch.observe(&state, false), None);
    }

    #[test]
    fn apple_migration_requires_the_same_endpoint_and_yields_to_explicit_selection() {
        let mut state = Playback {
            phase: Phase::Paused,
            audio_device: Some("coreaudio/usb/4".into()),
            audio_output: Some("avfoundation".into()),
            audio_devices: devices(&["auto", "coreaudio/usb/4", "avfoundation/usb/4"]),
            ..Playback::default()
        };
        assert_eq!(
            Watch::default().observe(&state, false).as_deref(),
            Some("avfoundation/usb/4")
        );
        let mut watch = Watch::default();
        assert_eq!(watch.observe(&state, true), None);
        assert_eq!(watch.observe(&state, false), None);
        state.audio_device = Some("alsa/dmix:unlisted".into());
        assert_eq!(watch.observe(&state, false), None);
    }
}
