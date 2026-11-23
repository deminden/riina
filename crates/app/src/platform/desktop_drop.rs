//! Desktop drop data shared by the native Windows and Wayland targets.
use slint::LogicalPosition;
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

#[cfg(target_os = "linux")]
pub(crate) use super::wayland_drop::Registration;
#[cfg(target_os = "linux")]
pub(crate) use super::wayland_drop::register;
#[cfg(target_os = "windows")]
pub(crate) use super::windows::file_drop::{Registration, register};

pub(crate) enum Hover {
    Entered(Vec<PathBuf>, LogicalPosition),
    #[cfg(target_os = "windows")]
    Link(LogicalPosition),
    Moved(LogicalPosition),
    Left,
}

pub(crate) type OpenDrop = Box<dyn FnOnce(Vec<OsString>)>;

pub(super) fn parse_references(text: &str, allow_plain_paths: bool) -> Vec<OsString> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#') && !line.contains('\0'))
        .filter_map(|line| parse_reference(line, allow_plain_paths))
        .collect()
}

fn parse_reference(line: &str, allow_plain_paths: bool) -> Option<OsString> {
    if allow_plain_paths && Path::new(line).is_absolute() {
        return Some(line.into());
    }
    let uri = url::Url::parse(line).ok()?;
    if uri.scheme() == "file" {
        return uri.to_file_path().ok().map(|path| path.into_os_string());
    }
    riina_core::media_reference::validate_network_url(line)
        .ok()
        .map(OsString::from)
}
