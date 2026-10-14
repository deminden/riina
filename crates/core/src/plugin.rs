//! Validated plugin metadata shared by installation, permissions and runtime setup.

use serde::Deserialize;
use std::{
    collections::BTreeSet,
    fs,
    path::{Component, Path, PathBuf},
};

const MAX_OVERLAY_TEXT_BYTES: usize = 1024 * 1024;
const MAX_OVERLAY_MESSAGE_NAME_BYTES: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Author {
    pub name: String,
    pub email: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permission {
    NetworkRequest,
    ShowOsd,
    ShowAlert,
    VideoOverlay,
    FileSystem,
}

/// Typed operations accepted by IINA's `core.window` plugin proxy.
#[derive(Debug, Clone, PartialEq)]
pub enum WindowAction {
    SetFrame {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    },
    SetFullscreen(bool),
    SetPictureInPicture(bool),
    SetOnTop(bool),
    SetSidebar(Option<String>),
    SetMiniaturized(bool),
}

/// Commands crossing from the isolated plugin runtime to a platform webview.
/// The core owns state transitions; a native adapter owns rendering and input.
#[derive(Debug, Clone, PartialEq)]
pub enum OverlayEffect {
    LoadPluginFile(PathBuf),
    EnterSimpleMode,
    Show,
    Hide,
    SetOpacity(f64),
    SetStyle(String),
    SetContent(String),
    SetClickable(bool),
    PostMessage {
        name: String,
        data: serde_json::Value,
    },
    DeliverMessage {
        name: String,
        data: serde_json::Value,
    },
    ListenMessage(String),
}

/// Per-plugin overlay state stays platform-neutral so Windows, macOS and Linux
/// webview adapters observe identical visibility and message semantics.
#[derive(Debug, Clone, PartialEq)]
pub struct OverlayState {
    loaded: bool,
    visible: bool,
    opacity: f64,
    clickable: bool,
    simple_mode: bool,
    message_listeners: BTreeSet<String>,
}

impl Default for OverlayState {
    fn default() -> Self {
        Self {
            loaded: false,
            visible: false,
            opacity: 1.0,
            clickable: false,
            simple_mode: false,
            message_listeners: BTreeSet::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum OverlayIntent {
    LoadFile(PathBuf),
    SimpleMode,
    Show,
    Hide,
    SetOpacity(f64),
    SetStyle(String),
    SetContent(String),
    SetClickable(bool),
    PostMessage {
        name: String,
        data: serde_json::Value,
    },
    OnMessage(String),
}

impl OverlayState {
    pub fn is_loaded(&self) -> bool {
        self.loaded
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn opacity(&self) -> f64 {
        self.opacity
    }

    pub fn is_clickable(&self) -> bool {
        self.clickable
    }

    /// Apply one permission-checked plugin request and return work for the
    /// platform webview. File containment is rechecked by that adapter.
    pub fn apply(&mut self, intent: OverlayIntent) -> Result<Vec<OverlayEffect>, String> {
        let effect = match intent {
            OverlayIntent::LoadFile(path) => {
                validate_plugin_relative_path(&path)?;
                self.loaded = true;
                self.simple_mode = false;
                self.message_listeners.clear();
                Some(OverlayEffect::LoadPluginFile(path))
            }
            OverlayIntent::SimpleMode => {
                if self.simple_mode {
                    None
                } else {
                    self.loaded = true;
                    self.simple_mode = true;
                    self.message_listeners.clear();
                    Some(OverlayEffect::EnterSimpleMode)
                }
            }
            OverlayIntent::Show if self.loaded => {
                self.visible = true;
                Some(OverlayEffect::Show)
            }
            OverlayIntent::Hide if self.loaded => {
                self.visible = false;
                Some(OverlayEffect::Hide)
            }
            OverlayIntent::SetOpacity(opacity) if self.loaded => {
                if !opacity.is_finite() {
                    return Err("Overlay opacity must be a finite number".into());
                }
                self.opacity = opacity.clamp(0.0, 1.0);
                Some(OverlayEffect::SetOpacity(self.opacity))
            }
            OverlayIntent::SetStyle(style) if self.loaded && self.simple_mode => {
                validate_overlay_text(&style)?;
                Some(OverlayEffect::SetStyle(style))
            }
            OverlayIntent::SetContent(content) if self.loaded && self.simple_mode => {
                validate_overlay_text(&content)?;
                Some(OverlayEffect::SetContent(content))
            }
            OverlayIntent::SetClickable(clickable) => {
                self.clickable = clickable;
                Some(OverlayEffect::SetClickable(clickable))
            }
            OverlayIntent::PostMessage { name, data } if self.loaded => {
                validate_message_name(&name)?;
                validate_message_data(&data)?;
                Some(OverlayEffect::PostMessage { name, data })
            }
            OverlayIntent::OnMessage(name) if self.loaded => {
                validate_message_name(&name)?;
                if self.message_listeners.insert(name.clone()) {
                    Some(OverlayEffect::ListenMessage(name))
                } else {
                    None
                }
            }
            // IINA's show/hide/opacity/message calls before a document is loaded
            // are no-ops; style and content require the simple-mode document.
            OverlayIntent::Show
            | OverlayIntent::Hide
            | OverlayIntent::SetOpacity(_)
            | OverlayIntent::PostMessage { .. }
            | OverlayIntent::OnMessage(_)
            | OverlayIntent::SetStyle(_)
            | OverlayIntent::SetContent(_) => None,
        };
        Ok(effect.into_iter().collect())
    }

    /// Deliver only messages for names this plugin explicitly subscribed to.
    pub fn receive_message(
        &self,
        name: String,
        data: serde_json::Value,
    ) -> Result<Option<OverlayEffect>, String> {
        validate_message_name(&name)?;
        validate_message_data(&data)?;
        Ok(self
            .message_listeners
            .contains(&name)
            .then_some(OverlayEffect::DeliverMessage { name, data }))
    }
}

pub(crate) fn validate_plugin_relative_path(path: &Path) -> Result<(), String> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err("Overlay files must use a safe relative plugin path".into());
    }
    Ok(())
}

fn validate_overlay_text(value: &str) -> Result<(), String> {
    if value.len() > MAX_OVERLAY_TEXT_BYTES {
        return Err("Overlay style or content exceeds the 1 MiB limit".into());
    }
    Ok(())
}

pub(crate) fn validate_message_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name.len() > MAX_OVERLAY_MESSAGE_NAME_BYTES {
        return Err("Overlay message names must contain 1–256 UTF-8 bytes".into());
    }
    Ok(())
}

pub(crate) fn validate_message_data(data: &serde_json::Value) -> Result<(), String> {
    let bytes = serde_json::to_vec(data).map_err(|error| error.to_string())?;
    if bytes.len() > MAX_OVERLAY_TEXT_BYTES {
        return Err("Overlay message data exceeds the 1 MiB limit".into());
    }
    Ok(())
}

impl Permission {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "network-request" => Some(Self::NetworkRequest),
            "show-osd" => Some(Self::ShowOsd),
            "show-alert" => Some(Self::ShowAlert),
            "video-overlay" => Some(Self::VideoOverlay),
            "file-system" => Some(Self::FileSystem),
            _ => None,
        }
    }

    pub const fn is_dangerous(self) -> bool {
        matches!(self, Self::NetworkRequest | Self::FileSystem)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub name: String,
    pub identifier: String,
    pub version: String,
    pub author: Author,
    pub description: Option<String>,
    pub entry: PathBuf,
    pub global_entry: Option<PathBuf>,
    pub permissions: Vec<String>,
    pub allowed_domains: Vec<String>,
    pub github_repo: Option<String>,
    pub github_version: Option<u64>,
    pub sidebar_tab_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawManifest {
    name: String,
    identifier: String,
    version: String,
    author: RawAuthor,
    entry: String,
    #[serde(rename = "globalEntry")]
    global_entry: Option<String>,
    description: Option<String>,
    permissions: Option<Vec<String>>,
    #[serde(rename = "allowedDomains")]
    allowed_domains: Option<Vec<String>>,
    #[serde(rename = "ghRepo")]
    github_repo: Option<String>,
    #[serde(rename = "ghVersion")]
    github_version: Option<u64>,
    #[serde(rename = "sidebarTab")]
    sidebar_tab: Option<serde_json::Value>,
    localized: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct RawAuthor {
    name: String,
    email: Option<String>,
    url: Option<String>,
}

impl Manifest {
    pub fn parse(json: &str) -> Result<Self, String> {
        Self::parse_with_locale(json, None)
    }

    pub fn parse_with_locale(json: &str, locale: Option<&str>) -> Result<Self, String> {
        let raw: RawManifest = serde_json::from_str(json).map_err(|error| error.to_string())?;
        let name = nonempty(raw.name, "name")?;
        let version = nonempty(raw.version, "version")?;
        let author = Author {
            name: nonempty(raw.author.name, "author.name")?,
            email: raw.author.email,
            url: raw.author.url,
        };
        if !valid_identifier(&raw.identifier) {
            return Err("Plugin identifier must use reverse-domain notation".into());
        }
        let entry = safe_relative_path(&raw.entry)?;
        let global_entry = raw
            .global_entry
            .as_deref()
            .map(safe_relative_path)
            .transpose()?;
        if let Some(repo) = raw.github_repo.as_deref()
            && !valid_repository(repo)
        {
            return Err("Plugin GitHub repository must use owner/name notation".into());
        }
        let permissions = raw.permissions.unwrap_or_default();
        let allowed_domains = raw.allowed_domains.unwrap_or_default();
        let sidebar_tab_name = parse_sidebar_tab_name(raw.sidebar_tab, raw.localized, locale)?;
        if permissions.iter().any(|value| value.trim().is_empty()) {
            return Err("Plugin permission names cannot be empty".into());
        }
        if allowed_domains
            .iter()
            .any(|domain| !valid_domain_pattern(domain))
        {
            return Err("Plugin allowed-domain patterns are invalid".into());
        }
        Ok(Self {
            name,
            identifier: raw.identifier,
            version,
            author,
            description: raw.description.filter(|value| !value.trim().is_empty()),
            entry,
            global_entry,
            permissions,
            allowed_domains,
            github_repo: raw.github_repo,
            github_version: raw.github_version,
            sidebar_tab_name,
        })
    }

    pub fn load_from_directory(root: &Path) -> Result<Self, String> {
        Self::load_from_directory_with_locale(root, None)
    }

    pub fn load_from_directory_with_locale(
        root: &Path,
        locale: Option<&str>,
    ) -> Result<Self, String> {
        let root = root
            .canonicalize()
            .map_err(|error| format!("Could not open plugin directory: {error}"))?;
        if !root.is_dir() {
            return Err("Plugin package is not a directory".into());
        }
        let manifest_path = root.join("Info.json");
        let bytes = fs::read(&manifest_path)
            .map_err(|error| format!("Could not read plugin manifest: {error}"))?;
        let text = std::str::from_utf8(&bytes)
            .map_err(|_| "Plugin manifest is not valid UTF-8".to_owned())?;
        let manifest = Self::parse_with_locale(text, locale)?;
        verify_package_file(&root, &manifest.entry)?;
        if let Some(path) = manifest.global_entry.as_deref() {
            verify_package_file(&root, path)?;
        }
        Ok(manifest)
    }

    pub fn known_permissions(&self) -> impl Iterator<Item = Permission> + '_ {
        self.permissions
            .iter()
            .filter_map(|value| Permission::parse(value))
    }

    pub fn unknown_permissions(&self) -> impl Iterator<Item = &str> + '_ {
        self.permissions
            .iter()
            .filter(|value| Permission::parse(value).is_none())
            .map(String::as_str)
    }
}

fn parse_sidebar_tab_name(
    value: Option<serde_json::Value>,
    localized: Option<serde_json::Value>,
    locale: Option<&str>,
) -> Result<Option<String>, String> {
    let base_name = value
        .as_ref()
        .and_then(|tab| tab.get("name"))
        .and_then(serde_json::Value::as_str);
    let localized_name = locale
        .and_then(|locale| localized_sidebar_entry(localized.as_ref(), locale))
        .and_then(|entry| entry.get("sidebarTab"))
        .and_then(|tab| tab.get("name"))
        .and_then(serde_json::Value::as_str);
    let Some(name) = localized_name.or(base_name) else {
        return Ok(None);
    };
    let name = name.trim();
    if name.is_empty() {
        return Ok(None);
    }
    if name.len() > 128 || name.chars().any(char::is_control) {
        return Err(
            "Plugin sidebar tab name must be at most 128 bytes without control characters".into(),
        );
    }
    Ok(Some(name.to_owned()))
}

fn localized_sidebar_entry<'a>(
    localized: Option<&'a serde_json::Value>,
    locale: &str,
) -> Option<&'a serde_json::Value> {
    let localized = localized?.as_object()?;
    let locale = locale.split(['.', '@']).next()?.replace('_', "-");
    let language = locale.split('-').next()?;
    for candidate in [Some(locale.as_str()), Some(language), Some("en")]
        .into_iter()
        .flatten()
    {
        if let Some(entry) = localized.get(candidate) {
            return Some(entry);
        }
        if let Some((_, entry)) = localized
            .iter()
            .find(|(key, _)| key.split(['-', '_']).next() == Some(candidate))
        {
            return Some(entry);
        }
    }
    None
}

impl Manifest {
    /// Network access is restricted to HTTP(S) hosts listed by the plugin.
    /// Wildcard entries match subdomains only, matching IINA's manifest rules.
    pub fn allows_network_url(&self, value: &str) -> bool {
        network_url_allowed(value, &self.allowed_domains)
    }
}

/// Validate an HTTP(S) URL against manifest exact-host and subdomain patterns.
pub fn network_url_allowed(value: &str, domains: &[String]) -> bool {
    let Ok(url) = url::Url::parse(value) else {
        return false;
    };
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return false;
    }
    let Some(host) = url.host_str() else {
        return false;
    };
    let host = host.to_ascii_lowercase();
    domains.iter().any(|pattern| {
        if pattern == "*" {
            return true;
        }
        if let Some(domain) = pattern.strip_prefix("*.") {
            let domain = domain.to_ascii_lowercase();
            host.len() > domain.len()
                && host
                    .strip_suffix(&domain)
                    .is_some_and(|prefix| prefix.ends_with('.'))
        } else {
            host == pattern.to_ascii_lowercase()
        }
    })
}

fn nonempty(value: String, field: &str) -> Result<String, String> {
    if value.trim().is_empty() {
        Err(format!("Plugin manifest field {field} cannot be empty"))
    } else {
        Ok(value.trim().to_owned())
    }
}

fn valid_identifier(value: &str) -> bool {
    let mut segments = value.split('.');
    let Some(first) = segments.next() else {
        return false;
    };
    let valid_segment = |part: &str| {
        !part.is_empty()
            && part
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    };
    valid_segment(first) && segments.count() > 0 && value.split('.').all(valid_segment)
}

fn valid_repository(value: &str) -> bool {
    let mut parts = value.split('/');
    let valid_part = |part: &str| {
        !part.is_empty()
            && part
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
    };
    parts.next().is_some_and(valid_part)
        && parts.next().is_some_and(valid_part)
        && parts.next().is_none()
}

fn valid_domain_pattern(value: &str) -> bool {
    if value == "*" {
        return true;
    }
    let domain = value.strip_prefix("*.").unwrap_or(value);
    !domain.is_empty()
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && domain.split('.').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
}

fn safe_relative_path(value: &str) -> Result<PathBuf, String> {
    let path = Path::new(value);
    if value.trim().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(format!("Plugin path is not a safe relative path: {value}"));
    }
    Ok(path.to_owned())
}

fn verify_package_file(root: &Path, relative: &Path) -> Result<(), String> {
    let path = root
        .join(relative)
        .canonicalize()
        .map_err(|error| format!("Plugin file {} is unavailable: {error}", relative.display()))?;
    if !path.starts_with(root) || !path.is_file() {
        return Err(format!(
            "Plugin file {} escapes its package or is not a file",
            relative.display()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = r#"{
        "name":"Sample plugin",
        "identifier":"org.example.sample",
        "version":"1.2.3",
        "author":{"name":"Example author","email":"a@example.org"},
        "entry":"main.js",
        "globalEntry":"global.js",
        "permissions":["show-osd","network-request","future-permission"],
        "allowedDomains":["example.org","*.media.example"],
        "ghRepo":"example/sample",
        "ghVersion":1
    }"#;

    #[test]
    fn manifest_keeps_declared_intent_and_separates_known_permissions() {
        let manifest = Manifest::parse(MANIFEST).unwrap();
        assert_eq!(manifest.identifier, "org.example.sample");
        assert_eq!(manifest.entry, PathBuf::from("main.js"));
        assert_eq!(manifest.permissions.len(), 3);
        assert_eq!(
            manifest.known_permissions().collect::<Vec<_>>(),
            [Permission::ShowOsd, Permission::NetworkRequest]
        );
        assert_eq!(
            manifest.unknown_permissions().collect::<Vec<_>>(),
            ["future-permission"]
        );
        assert!(Permission::NetworkRequest.is_dangerous());
        assert!(!Permission::ShowOsd.is_dangerous());
    }

    #[test]
    fn manifest_reads_a_bounded_sidebar_tab_label() {
        let json = MANIFEST.replace(
            "\"ghVersion\":1",
            "\"ghVersion\":1,\"sidebarTab\":{\"name\":\" Online Media \"}",
        );
        assert_eq!(
            Manifest::parse(&json).unwrap().sidebar_tab_name.as_deref(),
            Some("Online Media")
        );

        for name in ["x".repeat(129), "invalid\nlabel".to_owned()] {
            let json = MANIFEST.replace(
                "\"ghVersion\":1",
                &format!(
                    "\"ghVersion\":1,\"sidebarTab\":{{\"name\":{}}}",
                    serde_json::json!(name)
                ),
            );
            assert!(Manifest::parse(&json).is_err(), "accepted label {name:?}");
        }

        let json = MANIFEST.replace(
            "\"ghVersion\":1",
            "\"ghVersion\":1,\"sidebarTab\":{\"name\":23}",
        );
        assert_eq!(Manifest::parse(&json).unwrap().sidebar_tab_name, None);

        let json = MANIFEST.replace(
            "\"ghVersion\":1",
            "\"ghVersion\":1,\"sidebarTab\":{\"name\":\" \"}",
        );
        assert_eq!(Manifest::parse(&json).unwrap().sidebar_tab_name, None);
    }

    #[test]
    fn manifest_sidebar_labels_follow_the_selected_locale_and_fallbacks() {
        let json = MANIFEST.replace(
            "\"ghVersion\":1",
            "\"ghVersion\":1,\"sidebarTab\":{\"name\":\"Sidebar\"},\
                \"localized\":{\
                    \"fr-CA\":{\"sidebarTab\":{\"name\":\"Médias\"}},\
                    \"fr\":{\"sidebarTab\":{\"name\":\"French\"}},\
                    \"en\":{\"sidebarTab\":{\"name\":\"Online\"}}\
                }",
        );
        for (locale, expected) in [
            (Some("fr-CA"), "Médias"),
            (Some("fr-BE"), "French"),
            (Some("de-DE"), "Online"),
            (None, "Sidebar"),
        ] {
            assert_eq!(
                Manifest::parse_with_locale(&json, locale)
                    .unwrap()
                    .sidebar_tab_name
                    .as_deref(),
                Some(expected),
                "locale {locale:?}"
            );
        }
    }

    #[test]
    fn manifest_rejects_invalid_identity_and_escaping_entry_paths() {
        for (from, to) in [
            ("org.example.sample", "not-a-domain"),
            ("main.js", "../main.js"),
            ("main.js", "sub/../../main.js"),
        ] {
            let json = MANIFEST
                .replace(from, to)
                .replace("future-permission", "show-alert");
            assert!(
                Manifest::parse(&json).is_err(),
                "unexpectedly accepted {to}"
            );
        }
    }

    #[test]
    fn domain_patterns_are_host_only_and_allow_subdomain_wildcards() {
        assert!(valid_domain_pattern("*"));
        assert!(valid_domain_pattern("*.example.org"));
        assert!(valid_domain_pattern("video.example.org"));
        assert!(!valid_domain_pattern("https://example.org"));
        assert!(!valid_domain_pattern("*."));
    }

    #[test]
    fn network_urls_match_declared_http_hosts_only() {
        let domains = vec!["example.org".to_owned(), "*.media.example".to_owned()];
        assert!(network_url_allowed("https://example.org/watch", &domains));
        assert!(network_url_allowed(
            "http://cdn.media.example:8080/watch",
            &domains
        ));
        assert!(!network_url_allowed("https://sub.example.org/", &domains));
        assert!(!network_url_allowed("https://notexample.org/", &domains));
        assert!(!network_url_allowed("file:///etc/passwd", &domains));
        assert!(!network_url_allowed(
            "https://user@example.org/watch",
            &domains
        ));
    }

    #[test]
    fn overlay_starts_hidden_and_ignores_visibility_requests_until_loaded() {
        let mut overlay = OverlayState::default();
        assert!(!overlay.is_loaded());
        assert!(!overlay.is_visible());
        assert_eq!(overlay.opacity(), 1.0);
        assert!(!overlay.is_clickable());
        assert!(overlay.apply(OverlayIntent::Show).unwrap().is_empty());
        assert!(!overlay.is_visible());
    }

    #[test]
    fn overlay_file_load_is_relative_and_simple_mode_accepts_bounded_content() {
        let mut overlay = OverlayState::default();
        assert!(
            overlay
                .apply(OverlayIntent::LoadFile(PathBuf::from("../outside.html")))
                .is_err()
        );
        assert_eq!(
            overlay
                .apply(OverlayIntent::LoadFile(PathBuf::from("overlay/index.html")))
                .unwrap(),
            [OverlayEffect::LoadPluginFile(PathBuf::from(
                "overlay/index.html"
            ))]
        );
        assert!(!overlay.is_visible());
        assert!(
            overlay
                .apply(OverlayIntent::SetContent(
                    "<p>before simple mode</p>".into()
                ))
                .unwrap()
                .is_empty()
        );

        assert_eq!(
            overlay.apply(OverlayIntent::SimpleMode).unwrap(),
            [OverlayEffect::EnterSimpleMode]
        );
        assert_eq!(
            overlay
                .apply(OverlayIntent::SetContent("<p>Ready</p>".into()))
                .unwrap(),
            [OverlayEffect::SetContent("<p>Ready</p>".into())]
        );
        assert_eq!(
            overlay
                .apply(OverlayIntent::SetStyle("p { color: white; }".into()))
                .unwrap(),
            [OverlayEffect::SetStyle("p { color: white; }".into())]
        );
        assert!(
            overlay
                .apply(OverlayIntent::SetContent(
                    "x".repeat(MAX_OVERLAY_TEXT_BYTES + 1)
                ))
                .is_err()
        );
    }

    #[test]
    fn overlay_visibility_opacity_clickability_and_messages_are_typed() {
        let mut overlay = OverlayState::default();
        overlay.apply(OverlayIntent::SimpleMode).unwrap();
        assert_eq!(
            overlay.apply(OverlayIntent::SetOpacity(2.0)).unwrap(),
            [OverlayEffect::SetOpacity(1.0)]
        );
        assert!(overlay.apply(OverlayIntent::SetOpacity(f64::NAN)).is_err());
        assert_eq!(
            overlay.apply(OverlayIntent::SetClickable(true)).unwrap(),
            [OverlayEffect::SetClickable(true)]
        );
        assert!(overlay.is_clickable());
        assert_eq!(
            overlay.apply(OverlayIntent::Show).unwrap(),
            [OverlayEffect::Show]
        );
        assert!(overlay.is_visible());
        assert!(
            overlay
                .receive_message("ready".into(), serde_json::json!({"ok": true}))
                .unwrap()
                .is_none()
        );
        assert_eq!(
            overlay
                .apply(OverlayIntent::OnMessage("ready".into()))
                .unwrap(),
            [OverlayEffect::ListenMessage("ready".into())]
        );
        assert!(
            overlay
                .apply(OverlayIntent::OnMessage("ready".into()))
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            overlay
                .receive_message("ready".into(), serde_json::json!({"ok": true}))
                .unwrap(),
            Some(OverlayEffect::DeliverMessage {
                name: "ready".into(),
                data: serde_json::json!({"ok": true}),
            })
        );
        assert_eq!(
            overlay
                .apply(OverlayIntent::PostMessage {
                    name: "seek-to".into(),
                    data: serde_json::json!({"seconds": 12.5}),
                })
                .unwrap(),
            [OverlayEffect::PostMessage {
                name: "seek-to".into(),
                data: serde_json::json!({"seconds": 12.5}),
            }]
        );
        assert!(
            overlay
                .apply(OverlayIntent::OnMessage(String::new()))
                .is_err()
        );
    }
}
