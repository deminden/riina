use super::*;
use crate::platform::windows_job::KillOnCloseJob;
use std::os::windows::{io::AsRawHandle, process::CommandExt};
use windows::Win32::{
    Foundation::{ERROR_BROKEN_PIPE, HANDLE},
    System::{Pipes::PeekNamedPipe, Threading::CREATE_NO_WINDOW},
};

pub(in crate::plugins::plugin_runtime::utilities) struct ProcessGroup(KillOnCloseJob);

impl ProcessGroup {
    pub(in crate::plugins::plugin_runtime::utilities) fn spawn(
        command: &mut Command,
    ) -> io::Result<(Child, Self)> {
        let job = KillOnCloseJob::new()?;
        let child = job.spawn(command.creation_flags(CREATE_NO_WINDOW.0))?;
        Ok((child, Self(job)))
    }

    pub(in crate::plugins::plugin_runtime::utilities) fn terminate(&self, _child: &mut Child) {
        self.0.terminate();
    }
}

impl<T: Read + AsRawHandle> OutputPipe for T {
    fn prepare(&self) -> io::Result<()> {
        Ok(())
    }

    fn read_available(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let mut available = 0;
        // SAFETY: self owns the read handle, only this worker consumes the pipe,
        // and PeekNamedPipe writes one DWORD to the live output variable.
        let status = unsafe {
            PeekNamedPipe(
                HANDLE(self.as_raw_handle()),
                None,
                0,
                None,
                Some(&mut available),
                None,
            )
        };
        if let Err(error) = status {
            if error.code() == ERROR_BROKEN_PIPE.to_hresult() {
                return Ok(0);
            }
            return Err(error.into());
        }
        if available == 0 {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        let length = buffer.len().min(available as usize);
        self.read(&mut buffer[..length])
    }
}
