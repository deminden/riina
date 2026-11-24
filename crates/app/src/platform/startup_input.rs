//! Inspect inherited stdin without reading or waiting for the producer.

#[cfg(unix)]
pub(crate) fn buffered() -> bool {
    available(libc::STDIN_FILENO)
}

#[cfg(unix)]
fn available(fd: std::os::fd::RawFd) -> bool {
    let mut attributes = std::mem::MaybeUninit::<libc::stat>::uninit();
    // SAFETY: fstat only writes to the live stat allocation on success.
    if unsafe { libc::fstat(fd, attributes.as_mut_ptr()) } != 0 {
        return false;
    }
    // SAFETY: the successful fstat initialized the entire structure.
    let attributes = unsafe { attributes.assume_init() };
    // SAFETY: F_GETFL takes no trailing argument and does not change the descriptor.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || flags & libc::O_ACCMODE == libc::O_WRONLY {
        return false;
    }
    match attributes.st_mode & libc::S_IFMT {
        libc::S_IFREG => {
            // SAFETY: querying the offset with SEEK_CUR and a zero distance preserves it.
            let offset = unsafe { libc::lseek(fd, 0, libc::SEEK_CUR) };
            offset >= 0 && offset < attributes.st_size
        }
        libc::S_IFIFO | libc::S_IFSOCK => {
            let mut bytes: libc::c_int = 0;
            // SAFETY: FIONREAD writes an int through the live output pointer without reading stdin.
            (unsafe { libc::ioctl(fd, libc::FIONREAD, &mut bytes) }) == 0 && bytes > 0
        }
        _ => false,
    }
}

#[cfg(windows)]
pub(crate) fn buffered() -> bool {
    use std::os::windows::io::AsRawHandle;
    available(std::io::stdin().as_raw_handle())
}

#[cfg(windows)]
fn available(handle: std::os::windows::io::RawHandle) -> bool {
    use windows::Win32::{
        Foundation::HANDLE,
        Storage::FileSystem::{
            FILE_CURRENT, FILE_TYPE_DISK, FILE_TYPE_PIPE, GetFileSizeEx, GetFileType,
            SetFilePointerEx,
        },
        System::Pipes::PeekNamedPipe,
    };

    let handle = HANDLE(handle);
    // SAFETY: GetFileType checks the borrowed handle, including absent standard handles.
    match unsafe { GetFileType(handle) } {
        FILE_TYPE_DISK => {
            let mut length = 0;
            let mut offset = 0;
            // SAFETY: the output variables are live. A zero-relative seek preserves the offset.
            unsafe {
                GetFileSizeEx(handle, &mut length)
                    .and_then(|()| SetFilePointerEx(handle, 0, Some(&mut offset), FILE_CURRENT))
            }
            .is_ok()
                && offset >= 0
                && offset < length
        }
        FILE_TYPE_PIPE => {
            let mut bytes = 0;
            // SAFETY: startup has no other reader and PeekNamedPipe only writes the byte count.
            unsafe { PeekNamedPipe(handle, None, 0, None, Some(&mut bytes), None) }.is_ok()
                && bytes > 0
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Seek, SeekFrom, Write};

    #[cfg(unix)]
    use std::os::fd::AsRawFd;
    #[cfg(windows)]
    use std::os::windows::io::AsRawHandle;

    #[test]
    fn pipe_probe_neither_waits_nor_consumes_available_bytes() {
        let (mut reader, mut writer) = std::io::pipe().unwrap();
        #[cfg(unix)]
        let handle = reader.as_raw_fd();
        #[cfg(windows)]
        let handle = reader.as_raw_handle();
        assert!(!available(handle));
        writer.write_all(b"media").unwrap();
        drop(writer);
        assert!(available(handle));
        let mut payload = Vec::new();
        reader.read_to_end(&mut payload).unwrap();
        assert_eq!(payload, b"media");
        assert!(!available(handle));
    }

    #[test]
    fn file_probe_respects_the_inherited_offset() {
        let folder = riina_test_support::TempDir::new("stdin-probe").unwrap();
        let path = folder.path().join("clip");
        std::fs::write(&path, b"test").unwrap();
        let mut file = std::fs::File::open(path).unwrap();
        #[cfg(unix)]
        let handle = file.as_raw_fd();
        #[cfg(windows)]
        let handle = file.as_raw_handle();
        file.seek(SeekFrom::Start(2)).unwrap();
        assert!(available(handle));
        assert_eq!(file.stream_position().unwrap(), 2);
        file.seek(SeekFrom::End(0)).unwrap();
        assert!(!available(handle));
    }
}
