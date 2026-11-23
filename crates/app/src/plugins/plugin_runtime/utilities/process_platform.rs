//! OS ownership and nonblocking reads for helper processes.
use std::{
    io::{self, Read},
    process::{Child, Command},
};

pub(super) trait OutputPipe: Read {
    fn prepare(&self) -> io::Result<()>;
    fn read_available(&mut self, buffer: &mut [u8]) -> io::Result<usize>;
}

#[cfg(unix)]
pub(super) struct ProcessGroup;

#[cfg(unix)]
impl ProcessGroup {
    pub(super) fn spawn(command: &mut Command) -> io::Result<(Child, Self)> {
        use std::os::unix::process::CommandExt;
        Ok((command.process_group(0).spawn()?, Self))
    }

    pub(super) fn terminate(&self, child: &mut Child) {
        if let Ok(pid) = i32::try_from(child.id()) {
            // SAFETY: an unreaped child reserves this PID. The negative PID
            // selects the private group created by process_group(0).
            unsafe { libc::kill(-pid, libc::SIGKILL) };
        }
    }
}

#[cfg(unix)]
impl<T: Read + std::os::fd::AsFd> OutputPipe for T {
    fn prepare(&self) -> io::Result<()> {
        use std::os::fd::AsRawFd;
        let fd = self.as_fd().as_raw_fd();
        // SAFETY: the caller still owns the descriptor, so both commands refer
        // to the same pipe read end for the lifetime of this borrow.
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: the borrowed descriptor is still live and flags came from F_GETFL.
        if unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    fn read_available(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.read(buffer)
    }
}

#[cfg(windows)]
mod windows_process;
#[cfg(windows)]
pub(super) use windows_process::ProcessGroup;
