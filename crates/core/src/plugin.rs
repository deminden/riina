//! Validated plugin metadata shared by installation, permissions and runtime setup.

use serde::Deserialize;
use std::{
    fs,
    path::{Component, Path, PathBuf},
};

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
}

#[derive(Debug, Deserialize)]
struct RawAuthor {
    name: String,
    email: Option<String>,
    url: Option<String>,
}

impl Manifest {
    pub fn parse(json: &str) -> Result<Self, String> {
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
        })
    }

    pub fn load_from_directory(root: &Path) -> Result<Self, String> {
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
        let manifest = Self::parse(text)?;
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
}
