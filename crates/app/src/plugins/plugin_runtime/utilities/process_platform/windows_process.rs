use super::*;
use std::{
    mem::size_of_val,
    os::windows::{
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
        process::CommandExt,
    },
};
use windows::{
    Win32::{
        Foundation::{ERROR_BROKEN_PIPE, HANDLE},
        System::{
            JobObjects::{
                AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
                JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
                SetInformationJobObject, TerminateJobObject,
            },
            Pipes::PeekNamedPipe,
            Threading::CREATE_NO_WINDOW,
        },
    },
    core::PCWSTR,
};

pub(in crate::plugins::plugin_runtime::utilities) struct ProcessGroup(OwnedHandle);

impl ProcessGroup {
    pub(in crate::plugins::plugin_runtime::utilities) fn spawn(
        command: &mut Command,
    ) -> io::Result<(Child, Self)> {
        // SAFETY: this unnamed job has default security and no inheritable handle.
        let handle = unsafe { CreateJobObjectW(None, PCWSTR::null()) }?;
        // SAFETY: CreateJobObjectW returned a unique handle owned by this caller.
        let job = Self(unsafe { OwnedHandle::from_raw_handle(handle.0) });
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let length = u32::try_from(size_of_val(&limits)).map_err(io::Error::other)?;
        // SAFETY: the handle is live and limits is initialized for this info class.
        unsafe {
            SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                std::ptr::from_ref(&limits).cast(),
                length,
            )
        }?;
        let mut child = command.creation_flags(CREATE_NO_WINDOW.0).spawn()?;
        // SAFETY: both the job and the child keep their handles alive for the call.
        let assigned = unsafe { AssignProcessToJobObject(handle, HANDLE(child.as_raw_handle())) };
        if let Err(error) = assigned
            && !matches!(child.try_wait(), Ok(Some(_)))
        {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error.into());
        }
        Ok((child, job))
    }

    pub(in crate::plugins::plugin_runtime::utilities) fn terminate(&self, _child: &mut Child) {
        // SAFETY: the job handle remains owned until after the child has been reaped.
        let _ = unsafe { TerminateJobObject(HANDLE(self.0.as_raw_handle()), 1) };
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
