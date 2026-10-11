//! Typed network-cache settings shared by the UI, persistence, and player.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Optional mpv URL extraction preferences, matching IINA's ytdl settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct OnlineVideos {
    pub enabled: bool,
    pub search_path: String,
    pub raw_options: String,
}

impl Default for OnlineVideos {
    fn default() -> Self {
        Self {
            enabled: true,
            search_path: String::new(),
            raw_options: String::new(),
        }
    }
}

impl OnlineVideos {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.search_path.contains('\0') || self.raw_options.contains('\0') {
            return Err("Online video options cannot contain a NUL character");
        }
        Ok(())
    }

    pub fn engine_options(&self) -> Result<Vec<(String, String)>, String> {
        self.validate().map_err(str::to_owned)?;
        Ok(vec![
            (
                "ytdl".into(),
                if self.enabled { "yes" } else { "no" }.into(),
            ),
            ("ytdl-raw-options".into(), self.raw_options.clone()),
        ])
    }

    /// Put the configured extractor folder and app folder before inherited PATH.
    pub fn process_search_path(
        &self,
        executable: &Path,
        inherited: Option<&std::ffi::OsStr>,
    ) -> Result<std::ffi::OsString, String> {
        self.validate().map_err(str::to_owned)?;
        let mut paths: Vec<PathBuf> = Vec::new();
        if !self.search_path.is_empty() {
            paths.push(PathBuf::from(&self.search_path));
        }
        if let Some(folder) = executable.parent() {
            paths.push(folder.to_owned());
        }
        if let Some(inherited) = inherited {
            paths.extend(std::env::split_paths(inherited));
        }
        std::env::join_paths(paths).map_err(|_| "Invalid online video search path".into())
    }
}

/// Connection options in IINA's Network section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Connection {
    pub user_agent: String,
    pub http_proxy: String,
    pub rtsp_transport: RtspTransport,
}

#[cfg(test)]
mod online_video_tests {
    use super::OnlineVideos;
    use std::path::PathBuf;

    #[test]
    fn online_video_options_follow_reference_defaults_and_custom_values() {
        let defaults = OnlineVideos::default();
        assert_eq!(
            defaults.engine_options().unwrap(),
            [
                ("ytdl".into(), "yes".into()),
                ("ytdl-raw-options".into(), "".into())
            ]
        );
        let configured = OnlineVideos {
            enabled: false,
            search_path: "C:\\tools".into(),
            raw_options: "proxy=http://proxy.example:8080".into(),
        };
        assert_eq!(
            configured.engine_options().unwrap(),
            [
                ("ytdl".into(), "no".into()),
                (
                    "ytdl-raw-options".into(),
                    "proxy=http://proxy.example:8080".into()
                ),
            ]
        );
        assert!(
            OnlineVideos {
                raw_options: "bad\0option".into(),
                ..defaults
            }
            .validate()
            .is_err()
        );
    }

    #[test]
    fn custom_extractor_and_application_folders_precede_inherited_path() {
        let custom = std::env::temp_dir().join("riina-ytdl");
        let app = std::env::temp_dir().join("riina-app").join("riina.exe");
        let inherited = std::env::join_paths(["system-one", "system-two"]).unwrap();
        let preferences = OnlineVideos {
            search_path: custom.to_string_lossy().into_owned(),
            ..OnlineVideos::default()
        };
        let actual = preferences
            .process_search_path(&app, Some(&inherited))
            .unwrap();
        let paths: Vec<_> = std::env::split_paths(&actual).collect();
        assert_eq!(
            paths,
            [
                custom,
                app.parent().unwrap().to_owned(),
                PathBuf::from("system-one"),
                PathBuf::from("system-two")
            ]
        );
        assert!(
            OnlineVideos {
                search_path: "bad\0path".into(),
                ..OnlineVideos::default()
            }
            .process_search_path(&app, None)
            .is_err()
        );
    }
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
