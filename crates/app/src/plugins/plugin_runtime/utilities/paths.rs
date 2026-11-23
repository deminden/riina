use crate::plugin_files::PluginFiles;
use std::path::{Component, Path, PathBuf};

const MAX_PATH_BYTES: usize = 16 * 1024;

pub(super) fn resolve(
    files: &PluginFiles,
    path: &str,
    file_system: bool,
    allow_new: bool,
) -> Option<PathBuf> {
    if !valid(path) {
        return None;
    }
    let path = match path {
        "@data" => "@data/",
        "@tmp" => "@tmp/",
        path => path,
    };
    let resolved = files.resolve(path, file_system, allow_new).ok()??;
    (file_system || files.is_private_path(&resolved).ok()?).then_some(resolved)
}

pub(super) fn find_helper(files: &PluginFiles, name: &str) -> Option<PathBuf> {
    if !valid(name) {
        return None;
    }
    let path = Path::new(name);
    let only_name = matches!(path.components().next(), Some(Component::Normal(_)))
        && path.components().count() == 1
        && !name.starts_with('@');
    if !only_name {
        return resolve(files, name, true, false).filter(|path| path.is_file());
    }

    let app = std::env::current_exe().ok();
    let app = app.as_deref().and_then(Path::parent);
    let search = std::env::var_os("PATH").unwrap_or_default();
    let directories = app
        .into_iter()
        .flat_map(|root| [root.join("bin"), root.to_path_buf()]);
    directories
        .chain(std::env::split_paths(&search))
        .filter(|directory| directory.is_absolute())
        .find_map(|directory| {
            let candidate = directory.join(name);
            let candidate = if cfg!(windows) && candidate.extension().is_none() {
                candidate.with_extension("exe")
            } else {
                candidate
            };
            let path = candidate.canonicalize().ok()?;
            path.is_file().then_some(path)
        })
}

pub(super) fn binary_found(files: &PluginFiles, name: &str) -> bool {
    find_helper(files, name).is_some() || resolve(files, name, true, false).is_some()
}

pub(super) fn valid(path: &str) -> bool {
    !path.is_empty() && path.len() <= MAX_PATH_BYTES && !path.contains('\0')
}

pub(super) fn prepare_executable(files: &PluginFiles, binary: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        let _ = files;
        if !binary
            .extension()
            .and_then(std::ffi::OsStr::to_str)
            .is_some_and(|extension| {
                extension.eq_ignore_ascii_case("exe") || extension.eq_ignore_ascii_case("com")
            })
        {
            return Err("Windows plugin helpers must be native executables".into());
        }
    }
    #[cfg(unix)]
    {
        use std::{fs, os::unix::fs::PermissionsExt};
        let mut permissions = fs::metadata(binary)
            .map_err(|error| error.to_string())?
            .permissions();
        if permissions.mode() & 0o111 == 0 && files.is_private_path(binary)? {
            permissions.set_mode(permissions.mode() | 0o100);
            fs::set_permissions(binary, permissions).map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}
