//! The Linux desktop's per-user recent files are shared with other applications.
mod bookmarks;

use bookmarks::Bookmarks;
use gtk::{
    gio,
    gio::prelude::{FileExt, SettingsExt as GioSettingsExt},
    glib,
    prelude::GtkSettingsExt,
};
use std::{
    io,
    path::{Path, PathBuf},
};

const APPLICATION: &std::ffi::CStr = c"Riina";
const BOOKMARKS_FILE: &str = "recently-used.xbel";

pub(in crate::platform) fn remember(paths: impl Iterator<Item = PathBuf>) -> io::Result<()> {
    if !recording_enabled() {
        return Ok(());
    }
    let files = paths
        .filter_map(|path| path.canonicalize().ok())
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    if files.is_empty() {
        return Ok(());
    }
    let executable = std::env::current_exe()?;
    let argument = glib::shell_quote(&executable)
        .to_string_lossy()
        .replace('%', "%%");
    let command = std::ffi::CString::new(format!("{argument} %u")).map_err(io::Error::other)?;
    let path = location()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    update(&path, |bookmarks| {
        for path in &files {
            let uri = gio::File::for_path(path).uri();
            let (content, _) = gio::content_type_guess(Some(path), &[]);
            let mime = gio::content_type_get_mime_type(&content);
            let mime = mime.as_ref().map_or(c"application/octet-stream", |mime| {
                mime.as_gstr().to_cstr_until_nul()
            });
            bookmarks.register(
                uri.as_gstr().to_cstr_until_nul(),
                mime,
                APPLICATION,
                &command,
            );
        }
        true
    })
}

fn recording_enabled() -> bool {
    if gtk::is_initialized_main_thread()
        && let Some(settings) = gtk::Settings::default()
        && !settings.is_gtk_recent_files_enabled()
    {
        return false;
    }
    // The Slint player can run without initializing a GTK widget. Read the
    // system privacy setting directly when the GNOME schema is available.
    const REMEMBER: &str = "remember-recent-files";
    gio::SettingsSchemaSource::default()
        .and_then(|source| source.lookup("org.gnome.desktop.privacy", true))
        .filter(|schema| schema.has_key(REMEMBER))
        .is_none_or(|schema| {
            gio::Settings::new_full(&schema, gio::SettingsBackend::NONE, None).boolean(REMEMBER)
        })
}

pub(in crate::platform) fn clear() -> io::Result<()> {
    update(&location()?, |bookmarks| {
        bookmarks.remove_application(APPLICATION)
    })
}

fn location() -> io::Result<PathBuf> {
    dirs::data_local_dir()
        .map(|path| path.join(BOOKMARKS_FILE))
        .ok_or_else(|| io::Error::other("The user data directory is unavailable"))
}

fn update(path: &Path, edit: impl Fn(&mut Bookmarks) -> bool) -> io::Result<()> {
    let file = gio::File::for_path(path);
    for _ in 0..4 {
        let mut bookmarks = Bookmarks::new()?;
        let etag = match file.load_contents(gio::Cancellable::NONE) {
            Ok((data, etag)) => {
                bookmarks.load(&data)?;
                Some(etag.ok_or_else(|| {
                    io::Error::other("The desktop recent-file version is unavailable")
                })?)
            }
            Err(error) if error.matches(gio::IOErrorEnum::NotFound) => None,
            Err(error) => return Err(io::Error::other(error)),
        };
        if !edit(&mut bookmarks) {
            return Ok(());
        }
        let bytes = bookmarks.to_bytes()?;
        if let Some(etag) = etag {
            match file.replace_contents(
                &bytes,
                Some(&etag),
                false,
                gio::FileCreateFlags::PRIVATE,
                gio::Cancellable::NONE,
            ) {
                Ok(_) => return Ok(()),
                Err(error) if error.matches(gio::IOErrorEnum::WrongEtag) => continue,
                Err(error) => return Err(io::Error::other(error)),
            }
        }
        match super::super::settings_file::save_new(path, &bytes) {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::other(
        "The desktop recent-file list kept changing",
    ))
}

#[cfg(test)]
mod tests;
