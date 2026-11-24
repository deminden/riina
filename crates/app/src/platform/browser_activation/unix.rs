use super::{DEADLINE, Inbox, accept};
use std::{
    fs::{self, File, OpenOptions},
    io,
    os::{
        fd::AsRawFd,
        unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt},
    },
    path::PathBuf,
};
use tokio::net::{UnixListener, UnixStream};

pub(super) struct Endpoint {
    socket: PathBuf,
    lock: PathBuf,
    uid: u32,
}

impl Endpoint {
    pub(super) fn new(scope: &str) -> io::Result<Self> {
        // SAFETY: geteuid takes no arguments and returns this process's identity.
        let uid = unsafe { libc::geteuid() };
        let dir = std::env::temp_dir().join(format!("riina-browser-{uid}"));
        match fs::DirBuilder::new().mode(0o700).create(&dir) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
        let meta = fs::symlink_metadata(&dir)?;
        if !meta.is_dir() || meta.uid() != uid || meta.mode() & 0o077 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "The browser-link directory is not private",
            ));
        }
        Ok(Self {
            socket: dir.join(format!("{scope}.sock")),
            lock: dir.join(format!("{scope}.lock")),
            uid,
        })
    }

    pub(super) fn listen(&self) -> io::Result<Listener> {
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&self.lock)?;
        let meta = lock.metadata()?;
        if !meta.is_file() || meta.uid() != self.uid || meta.mode() & 0o077 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "The browser-link lock is not private",
            ));
        }
        // SAFETY: the owned file descriptor is valid for the lifetime of the lock.
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err(io::Error::last_os_error());
        }
        match fs::symlink_metadata(&self.socket) {
            Ok(meta) if meta.file_type().is_socket() && meta.uid() == self.uid => {
                fs::remove_file(&self.socket)?
            }
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "The browser-link socket has an unexpected owner or type",
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        let socket = UnixListener::bind(&self.socket)?;
        fs::set_permissions(&self.socket, fs::Permissions::from_mode(0o600))?;
        Ok(Listener {
            socket,
            path: self.socket.clone(),
            uid: self.uid,
            _lock: lock,
        })
    }

    pub(super) async fn connect(&self) -> io::Result<UnixStream> {
        let stream = UnixStream::connect(&self.socket).await?;
        if stream.peer_cred()?.uid() != self.uid {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "The browser-link server belongs to a different user",
            ));
        }
        Ok(stream)
    }
}

pub(super) fn occupied(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::WouldBlock
}

pub(super) struct Listener {
    socket: UnixListener,
    path: PathBuf,
    uid: u32,
    _lock: File,
}

impl Listener {
    pub(super) async fn serve(self, inbox: &Inbox) -> io::Result<()> {
        loop {
            let (stream, _) = self.socket.accept().await?;
            if stream.peer_cred().is_ok_and(|peer| peer.uid() == self.uid) {
                let _ = tokio::time::timeout(DEADLINE, accept(stream, inbox)).await;
            }
        }
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}
