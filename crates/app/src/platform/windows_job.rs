//! Own a process tree for the lifetime of its initiating workflow.
use std::{
    io,
    mem::size_of_val,
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    process::{Child, Command},
};
use windows::{
    Win32::{
        Foundation::HANDLE,
        System::JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
            SetInformationJobObject, TerminateJobObject,
        },
    },
    core::PCWSTR,
};

pub(crate) struct KillOnCloseJob(OwnedHandle);

impl KillOnCloseJob {
    pub(crate) fn new() -> io::Result<Self> {
        // SAFETY: this unnamed job has default security and a non-inheritable handle.
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
        Ok(job)
    }

    pub(crate) fn spawn(&self, command: &mut Command) -> io::Result<Child> {
        let mut child = command.spawn()?;
        // SAFETY: the job and child own their respective handles for the call.
        let assigned = unsafe {
            AssignProcessToJobObject(
                HANDLE(self.0.as_raw_handle()),
                HANDLE(child.as_raw_handle()),
            )
        };
        if let Err(error) = assigned
            && !matches!(child.try_wait(), Ok(Some(_)))
        {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error.into());
        }
        Ok(child)
    }

    pub(crate) fn terminate(&self) {
        // SAFETY: the job handle remains owned until after the child has been reaped.
        let _ = unsafe { TerminateJobObject(HANDLE(self.0.as_raw_handle()), 1) };
    }
}
