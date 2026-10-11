//! Typed network-cache settings shared by the UI, persistence, and player.

use serde::{Deserialize, Serialize};

/// Connection options in IINA's Network section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Connection {
    pub user_agent: String,
    pub http_proxy: String,
    pub rtsp_transport: RtspTransport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RtspTransport {
    Lavf,
    Tcp,
    Udp,
    Http,
}

impl RtspTransport {
    pub const ALL: [Self; 4] = [Self::Lavf, Self::Tcp, Self::Udp, Self::Http];

    pub const fn native_value(self) -> &'static str {
        match self {
            Self::Lavf => "lavf",
            Self::Tcp => "tcp",
            Self::Udp => "udp",
            Self::Http => "http",
        }
    }
}

impl Default for Connection {
    fn default() -> Self {
        // IINA v1.5.0 leaves both text overrides empty and routes RTSP over TCP.
        Self {
            user_agent: String::new(),
            http_proxy: String::new(),
            rtsp_transport: RtspTransport::Tcp,
        }
    }
}

impl Connection {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.user_agent.contains('\0') {
            return Err("User agent cannot contain a NUL character");
        }
        if self.http_proxy.contains('\0') {
            return Err("HTTP proxy cannot contain a NUL character");
        }
        Ok(())
    }

    /// Empty overrides stay unset so libmpv retains its own defaults.
    pub fn engine_options(&self) -> Result<Vec<(String, String)>, String> {
        self.validate().map_err(str::to_owned)?;
        let mut options = Vec::with_capacity(3);
        if !self.user_agent.is_empty() {
            options.push(("user-agent".into(), self.user_agent.clone()));
        }
        if !self.http_proxy.is_empty() {
            options.push(("http-proxy".into(), self.http_proxy.clone()));
        }
        options.push((
            "rtsp-transport".into(),
            self.rtsp_transport.native_value().into(),
        ));
        Ok(options)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connection_options_keep_defaults_and_apply_explicit_overrides() {
        assert_eq!(
            Connection::default().engine_options().unwrap(),
            vec![("rtsp-transport".into(), "tcp".into())]
        );
        let preferences = Connection {
            user_agent: "Riina test agent".into(),
            http_proxy: "http://proxy.example:8080".into(),
            rtsp_transport: RtspTransport::Udp,
        };
        assert_eq!(
            preferences.engine_options().unwrap(),
            vec![
                ("user-agent".into(), "Riina test agent".into()),
                ("http-proxy".into(), "http://proxy.example:8080".into()),
                ("rtsp-transport".into(), "udp".into()),
            ]
        );
        assert!(
            Connection {
                user_agent: "bad\0header".into(),
                ..Connection::default()
            }
            .validate()
            .is_err()
        );
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Cache {
    pub enabled: bool,
    pub pause_initial: bool,
    pub pause_wait_secs: f64,
    pub prefetch_secs: f64,
    pub buffer_size_kib: f64,
    pub show_buffering_throbber: bool,
    pub show_seeking_throbber: bool,
}

impl Default for Cache {
    fn default() -> Self {
        // IINA v1.5.0 defaults; preserve the same engine cache behavior on first launch.
        Self {
            enabled: true,
            pause_initial: false,
            pause_wait_secs: 1.0,
            prefetch_secs: 36_000.0,
            buffer_size_kib: 153_600.0,
            show_buffering_throbber: true,
            show_seeking_throbber: true,
        }
    }
}

impl Cache {
    pub fn validate(&self) -> Result<(), &'static str> {
        if !self.pause_wait_secs.is_finite()
            || !(0.0..=2_000_000_000.0).contains(&self.pause_wait_secs)
        {
            return Err("Buffer-before-start duration is out of range");
        }
        if !self.prefetch_secs.is_finite() || !(0.0..=2_000_000_000.0).contains(&self.prefetch_secs)
        {
            return Err("Prefetch duration is out of range");
        }
        if !self.buffer_size_kib.is_finite()
            || !(0.0..=2_000_000_000.0).contains(&self.buffer_size_kib)
        {
            return Err("Cache buffer size is out of range");
        }
        Ok(())
    }

    pub fn commands(&self) -> Result<Vec<Vec<String>>, String> {
        self.validate().map_err(str::to_owned)?;
        let enabled = if self.enabled { "yes" } else { "no" };
        let pause_initial = if self.pause_initial { "yes" } else { "no" };
        Ok(vec![
            vec!["set".into(), "cache".into(), enabled.into()],
            vec![
                "set".into(),
                "cache-pause-initial".into(),
                pause_initial.into(),
            ],
            vec![
                "set".into(),
                "cache-pause-wait".into(),
                self.pause_wait_secs.to_string(),
            ],
            vec![
                "set".into(),
                "cache-secs".into(),
                self.prefetch_secs.to_string(),
            ],
            vec![
                "set".into(),
                "demuxer-max-bytes".into(),
                format!("{}KiB", self.buffer_size_kib),
            ],
        ])
    }

    pub fn engine_options(&self) -> Result<Vec<(String, String)>, String> {
        self.validate().map_err(str::to_owned)?;
        Ok(vec![
            (
                "cache".into(),
                if self.enabled { "yes" } else { "no" }.into(),
            ),
            (
                "cache-pause-initial".into(),
                if self.pause_initial { "yes" } else { "no" }.into(),
            ),
            ("cache-pause-wait".into(), self.pause_wait_secs.to_string()),
            ("cache-secs".into(), self.prefetch_secs.to_string()),
            (
                "demuxer-max-bytes".into(),
                format!("{}KiB", self.buffer_size_kib),
            ),
        ])
    }
}
