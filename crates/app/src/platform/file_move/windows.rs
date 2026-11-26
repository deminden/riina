use std::{
    fs::OpenOptions,
    io,
    mem::offset_of,
    os::windows::{ffi::OsStrExt, fs::OpenOptionsExt, io::AsRawHandle},
    path::Path,
};
use windows::{
    Win32::{
        Foundation::HANDLE,
        Storage::FileSystem::{
            DELETE, FILE_ATTRIBUTE_NORMAL, FILE_RENAME_INFO, FILE_RENAME_INFO_0, FileRenameInfoEx,
            SetFileAttributesW, SetFileInformationByHandle,
        },
    },
    core::PCWSTR,
};

pub(crate) fn replace(source: &Path, destination: &Path) -> io::Result<()> {
    let source_name = source
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    // SAFETY: the named temporary file's encoded path is live and NUL-terminated.
    unsafe { SetFileAttributesW(PCWSTR(source_name.as_ptr()), FILE_ATTRIBUTE_NORMAL) }
        .map_err(io::Error::other)?;
    let file = OpenOptions::new().access_mode(DELETE.0).open(source)?;
    let name = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let wire_length = size_of_val(name.as_slice());
    let offset = offset_of!(FILE_RENAME_INFO, FileName);
    let buffer_size = (offset + wire_length).max(size_of::<FILE_RENAME_INFO>());
    let mut buffer =
        vec![FILE_RENAME_INFO::default(); buffer_size.div_ceil(size_of::<FILE_RENAME_INFO>())];
    const REPLACE_IF_EXISTS: u32 = 1;
    const POSIX_SEMANTICS: u32 = 2;
    buffer[0].Anonymous = FILE_RENAME_INFO_0 {
        Flags: REPLACE_IF_EXISTS | POSIX_SEMANTICS,
    };
    buffer[0].FileNameLength = (wire_length - size_of::<u16>()) as u32;
    let raw = buffer.as_mut_ptr().cast::<u8>();
    // SAFETY: the aligned, zero-initialized buffer contains the fixed header and
    // the entire UTF-16 name. Both it and the open DELETE handle live through the call.
    let result = unsafe {
        std::ptr::copy_nonoverlapping(name.as_ptr().cast::<u8>(), raw.add(offset), wire_length);
        SetFileInformationByHandle(
            HANDLE(file.as_raw_handle()),
            FileRenameInfoEx,
            raw.cast(),
            buffer_size as u32,
        )
    };
    drop(file);
    match result {
        Ok(()) => Ok(()),
        Err(error)
            if matches!(
                error.code().0 as u32,
                0x8007_0001 | 0x8007_0032 | 0x8007_0057
            ) =>
        {
            // Filesystems without extended rename can still replace a closed file.
            std::fs::rename(source, destination)
        }
        Err(error) => Err(io::Error::other(error)),
    }
}
