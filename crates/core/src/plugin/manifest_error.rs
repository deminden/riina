use std::{fmt, io, path::PathBuf, str::Utf8Error};

/// Invalid declarations and package access errors failed at different boundaries.
#[derive(Debug)]
pub enum PluginManifestError {
    Json(serde_json::Error),
    EmptyField(&'static str),
    Identifier,
    Repository,
    Permission,
    AllowedDomain,
    SidebarName,
    InvalidPath(String),
    OpenDirectory(io::Error),
    NotDirectory,
    ReadManifest(io::Error),
    Encoding(Utf8Error),
    UnavailableFile { path: PathBuf, source: io::Error },
    OutsidePackage(PathBuf),
}

impl fmt::Display for PluginManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => write!(f, "{error}"),
            Self::EmptyField(field) => write!(f, "Plugin manifest field {field} cannot be empty"),
            Self::Identifier => f.write_str("Plugin identifier must use reverse-domain notation"),
            Self::Repository => {
                f.write_str("Plugin GitHub repository must use owner/name notation")
            }
            Self::Permission => f.write_str("Plugin permission names cannot be empty"),
            Self::AllowedDomain => f.write_str("Plugin allowed-domain patterns are invalid"),
            Self::SidebarName => f.write_str(
                "Plugin sidebar tab name must be at most 128 bytes without control characters",
            ),
            Self::InvalidPath(path) => write!(f, "Plugin path is not a safe relative path: {path}"),
            Self::OpenDirectory(error) => write!(f, "Could not open plugin directory: {error}"),
            Self::NotDirectory => f.write_str("Plugin package is not a directory"),
            Self::ReadManifest(error) => write!(f, "Could not read plugin manifest: {error}"),
            Self::Encoding(_) => f.write_str("Plugin manifest is not valid UTF-8"),
            Self::UnavailableFile { path, source } => {
                write!(f, "Plugin file {} is unavailable: {source}", path.display())
            }
            Self::OutsidePackage(path) => write!(
                f,
                "Plugin file {} escapes its package or is not a file",
                path.display()
            ),
        }
    }
}

impl std::error::Error for PluginManifestError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Json(error) => Some(error),
            Self::OpenDirectory(error) | Self::ReadManifest(error) => Some(error),
            Self::Encoding(error) => Some(error),
            Self::UnavailableFile { source, .. } => Some(source),
            _ => None,
        }
    }
}
