//! Bind the source-matched Network pane to validated persisted settings.

use riina_core::network_preferences::{Connection, RtspTransport};

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
    let preferences = Connection {
        user_agent: window.get_network_user_agent().to_string(),
        http_proxy: window.get_network_http_proxy().to_string(),
        rtsp_transport: transport(window.get_network_rtsp_transport_index())
            .ok_or_else(|| "Invalid RTSP transport selection".to_owned())?,
    };
    preferences.validate().map_err(str::to_owned)?;
    Ok(preferences)
}

pub(crate) fn install_window(window: &crate::PreferencesWindow, preferences: &Connection) {
    window.set_network_user_agent(preferences.user_agent.clone().into());
    window.set_network_http_proxy(preferences.http_proxy.clone().into());
    window.set_network_rtsp_transport_index(transport_index(preferences.rtsp_transport));
    window.set_network_rtsp_transport_options(slint::ModelRc::new(slint::VecModel::from(
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
