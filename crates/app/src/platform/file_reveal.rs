//! Reveal related items together in the desktop's file manager.
use std::{io, path::PathBuf};

#[cfg(target_os = "windows")]
mod windows;

pub(super) fn reveal_files(paths: &[PathBuf]) -> io::Result<()> {
    if paths.is_empty() {
        return Ok(());
    }
    let absolute = paths
        .iter()
        .map(std::path::absolute)
        .collect::<io::Result<Vec<_>>>()?;
    reveal_absolute_files(&absolute)
}

#[cfg(target_os = "windows")]
fn reveal_absolute_files(paths: &[PathBuf]) -> io::Result<()> {
    windows::reveal(paths)
}

#[cfg(target_os = "macos")]
fn reveal_absolute_files(paths: &[PathBuf]) -> io::Result<()> {
    use objc2_app_kit::NSWorkspace;
    use objc2_foundation::{NSArray, NSString, NSURL};

    objc2::rc::autoreleasepool(|_| {
        let urls = paths
            .iter()
            .map(|path| {
                let text = path
                    .to_str()
                    .ok_or_else(|| io::Error::other("The file path is not Unicode"))?;
                Ok(NSURL::fileURLWithPath(&NSString::from_str(text)))
            })
            .collect::<io::Result<Vec<_>>>()?;
        NSWorkspace::sharedWorkspace()
            .activateFileViewerSelectingURLs(&NSArray::from_retained_slice(&urls));
        Ok(())
    })
}

#[cfg(target_os = "linux")]
fn reveal_absolute_files(paths: &[PathBuf]) -> io::Result<()> {
    let urls = paths
        .iter()
        .map(|path| {
            url::Url::from_file_path(path)
                .map(|url| url.to_string())
                .map_err(|()| {
                    io::Error::other("The file path could not be converted to a file URL")
                })
        })
        .collect::<io::Result<Vec<_>>>()?;
    let show_items = || -> zbus::Result<()> {
        let connection = zbus::blocking::Connection::session()?;
        let proxy = zbus::blocking::Proxy::new(
            &connection,
            "org.freedesktop.FileManager1",
            "/org/freedesktop/FileManager1",
            "org.freedesktop.FileManager1",
        )?;
        proxy.call("ShowItems", &(urls, ""))
    };
    if show_items().is_err() {
        let folders = paths
            .iter()
            .filter_map(|path| path.parent())
            .collect::<std::collections::BTreeSet<_>>();
        for folder in folders {
            super::open_directory(folder)?;
        }
    }
    Ok(())
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn reveal_absolute_files(_paths: &[PathBuf]) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Showing selected files is unavailable",
    ))
}
