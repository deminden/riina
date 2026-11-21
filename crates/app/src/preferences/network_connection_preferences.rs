//! Bind the source-matched Network pane to validated persisted settings.
use slint::ComponentHandle;

use super::model::{PreferenceSection, SectionError};
use crate::PreferencesWindow;
use riina_core::Preferences;
use riina_core::network_preferences::{Connection, RtspTransport};

impl PreferenceSection for Connection {
    fn get(all: &Preferences) -> Self {
        all.network.clone()
    }

    fn put(self, all: &mut Preferences) {
        all.network = self;
    }

    fn read_window(window: &PreferencesWindow) -> Result<Self, SectionError> {
        current_window(window).map_err(SectionError::from)
    }

    fn write_window(&self, window: &PreferencesWindow) {
        install_window(window, self);
    }
}

pub(crate) fn transport_index(transport: RtspTransport) -> i32 {
    match transport {
        RtspTransport::Lavf => 0,
        RtspTransport::Tcp => 1,
        RtspTransport::Udp => 2,
        RtspTransport::Http => 3,
    }
}

pub(crate) fn transport(index: i32) -> Option<RtspTransport> {
    match index {
        0 => Some(RtspTransport::Lavf),
        1 => Some(RtspTransport::Tcp),
        2 => Some(RtspTransport::Udp),
        3 => Some(RtspTransport::Http),
        _ => None,
    }
}

pub(crate) fn current_window(window: &crate::PreferencesWindow) -> Result<Connection, String> {
    let form = window.global::<crate::NetworkSettingsState>();
    let preferences = Connection {
        user_agent: form.get_network_user_agent().to_string(),
        http_proxy: form.get_network_http_proxy().to_string(),
        rtsp_transport: transport(form.get_network_rtsp_transport_index())
            .ok_or_else(|| "Invalid RTSP transport selection".to_owned())?,
    };
    preferences.validate().map_err(str::to_owned)?;
    Ok(preferences)
}

pub(crate) fn install_window(window: &crate::PreferencesWindow, preferences: &Connection) {
    let form = window.global::<crate::NetworkSettingsState>();
    form.set_network_user_agent(preferences.user_agent.clone().into());
    form.set_network_http_proxy(preferences.http_proxy.clone().into());
    form.set_network_rtsp_transport_index(transport_index(preferences.rtsp_transport));
    form.set_network_rtsp_transport_options(slint::ModelRc::new(slint::VecModel::from(
        vec!["lavf", "tcp", "udp", "http"]
            .into_iter()
            .map(Into::into)
            .collect::<Vec<slint::SharedString>>(),
    )));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rtsp_transport_selection_preserves_source_order_and_tcp_default() {
        assert_eq!(transport_index(RtspTransport::Tcp), 1);
        assert_eq!(transport(0), Some(RtspTransport::Lavf));
        assert_eq!(transport(1), Some(RtspTransport::Tcp));
        assert_eq!(transport(2), Some(RtspTransport::Udp));
        assert_eq!(transport(3), Some(RtspTransport::Http));
        assert_eq!(transport(4), None);
    }
}
