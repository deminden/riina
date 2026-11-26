use std::{
    fs, io,
    path::{Path, PathBuf},
};

use super::rename_new;

pub(super) fn move_new(source: &Path, destination: &Path) -> io::Result<()> {
    if destination.starts_with(source) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "cannot move a directory into itself",
        ));
    }
    let source_stage = stage_next_to(source)?;
    let destination_stage = stage_next_to(destination)?;
    let staged_source = source_stage.path().join("item");
    let staged_destination = destination_stage.path().join("item");

    rename_new(source, &staged_source)?;
    // The original may contain a mounted filesystem. Only clean it after the
    // copy has validated the whole tree. In particular, don't let Drop erase it.
    let source_stage = source_stage.keep();
    if let Err(error) = copy_tree(&staged_source, &staged_destination)
        .and_then(|()| rename_new(&staged_destination, destination))
    {
        if let Err(restore_error) = rename_new(&staged_source, source) {
            return Err(retained_source_error(&staged_source, error, restore_error));
        }
        let _ = fs::remove_dir(source_stage);
        return Err(error);
    }

    let result = fs::symlink_metadata(&staged_source)
        .and_then(|metadata| remove_source(&staged_source, &metadata));
    if let Err(error) = result {
        return Err(io::Error::other(format!(
            "destination published, but original cleanup failed at {}: {error}",
            staged_source.display()
        )));
    }
    let _ = fs::remove_dir(source_stage);
    Ok(())
}

fn stage_next_to(path: &Path) -> io::Result<tempfile::TempDir> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "move requires a parent"))?;
    let mut builder = tempfile::Builder::new();
    builder.prefix(".riina-move-");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        builder.permissions(fs::Permissions::from_mode(0o700));
    }
    builder.tempdir_in(parent)
}

fn retained_source_error(
    path: &Path,
    move_error: io::Error,
    restore_error: io::Error,
) -> io::Error {
    io::Error::other(format!(
        "{move_error}; could not restore the original ({restore_error}). It remains at {}",
        path.display()
    ))
}

enum Pending {
    Copy(PathBuf, PathBuf),
    FinishDirectory(PathBuf, fs::Metadata),
}

fn copy_tree(source: &Path, destination: &Path) -> io::Result<()> {
    #[cfg(unix)]
    let source_volume = volume(source)?;
    let mut pending = vec![Pending::Copy(source.to_owned(), destination.to_owned())];
    while let Some(step) = pending.pop() {
        let (source, destination) = match step {
            Pending::Copy(source, destination) => (source, destination),
            Pending::FinishDirectory(destination, metadata) => {
                copy_directory_metadata(&destination, &metadata)?;
                continue;
            }
        };
        let metadata = fs::symlink_metadata(&source)?;
        if metadata.is_symlink() {
            copy_link(&source, &destination, &metadata)?;
            continue;
        }
        #[cfg(unix)]
        if volume(&source)? != source_volume {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "cannot move a directory containing a mount",
            ));
        }
        #[cfg(windows)]
        reject_reparse_point(&metadata)?;
        if metadata.is_file() {
            fs::copy(&source, &destination)?;
            #[cfg(unix)]
            copy_times(&destination, &metadata)?;
        } else if metadata.is_dir() {
            fs::create_dir(&destination)?;
            pending.push(Pending::FinishDirectory(destination.clone(), metadata));
            for entry in fs::read_dir(source)? {
                let entry = entry?;
                pending.push(Pending::Copy(
                    entry.path(),
                    destination.join(entry.file_name()),
                ));
            }
        } else {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "cannot move a special filesystem entry across volumes",
            ));
        }
    }
    Ok(())
}

fn copy_directory_metadata(destination: &Path, metadata: &fs::Metadata) -> io::Result<()> {
    copy_times(destination, metadata)?;
    fs::set_permissions(destination, metadata.permissions())
}

#[cfg(unix)]
fn copy_times(path: &Path, metadata: &fs::Metadata) -> io::Result<()> {
    fs::File::open(path)?.set_times(file_times(metadata)?)
}

#[cfg(windows)]
fn copy_times(path: &Path, metadata: &fs::Metadata) -> io::Result<()> {
    use std::os::windows::fs::{FileTimesExt, OpenOptionsExt};
    use windows::Win32::Storage::FileSystem::{FILE_FLAG_BACKUP_SEMANTICS, FILE_WRITE_ATTRIBUTES};

    fs::OpenOptions::new()
        .access_mode(FILE_WRITE_ATTRIBUTES.0)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS.0)
        .open(path)?
        .set_times(file_times(metadata)?.set_created(metadata.created()?))
}

fn file_times(metadata: &fs::Metadata) -> io::Result<fs::FileTimes> {
    Ok(fs::FileTimes::new()
        .set_accessed(metadata.accessed()?)
        .set_modified(metadata.modified()?))
}

#[cfg(target_os = "linux")]
fn volume(path: &Path) -> io::Result<u64> {
    use std::{ffi::CString, mem::MaybeUninit, os::unix::ffi::OsStrExt};

    let path = CString::new(path.as_os_str().as_bytes())?;
    let mut result = MaybeUninit::<libc::statx>::uninit();
    // SAFETY: path is terminated and the kernel fills statx on success.
    let status = unsafe {
        libc::statx(
            libc::AT_FDCWD,
            path.as_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
            libc::STATX_MNT_ID,
            result.as_mut_ptr(),
        )
    };
    if status != 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: statx succeeded and the mask identifies the valid fields.
    let result = unsafe { result.assume_init() };
    if result.stx_mask & libc::STATX_MNT_ID == 0 {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "filesystem mount identity is unavailable",
        ));
    }
    // Linux bind mounts can share the same device number as their parent.
    Ok(result.stx_mnt_id)
}

#[cfg(target_os = "macos")]
fn volume(path: &Path) -> io::Result<u64> {
    use std::os::unix::fs::MetadataExt;

    fs::symlink_metadata(path).map(|metadata| metadata.dev())
}

#[cfg(unix)]
fn copy_link(source: &Path, destination: &Path, _metadata: &fs::Metadata) -> io::Result<()> {
    std::os::unix::fs::symlink(fs::read_link(source)?, destination)
}

#[cfg(windows)]
fn copy_link(source: &Path, destination: &Path, metadata: &fs::Metadata) -> io::Result<()> {
    use std::os::windows::fs::{FileTypeExt, symlink_dir, symlink_file};

    let target = fs::read_link(source)?;
    if metadata.file_type().is_symlink_dir() {
        symlink_dir(target, destination)
    } else {
        symlink_file(target, destination)
    }
}

#[cfg(windows)]
fn reject_reparse_point(metadata: &fs::Metadata) -> io::Result<()> {
    use std::os::windows::fs::MetadataExt;
    use windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;

    if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "cannot move this reparse point across volumes",
        ));
    }
    Ok(())
}

fn remove_source(path: &Path, metadata: &fs::Metadata) -> io::Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::FileTypeExt;
        if metadata.file_type().is_symlink_dir() {
            return fs::remove_dir(path);
        }
    }
    if metadata.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

#[cfg(test)]
mod tests;
