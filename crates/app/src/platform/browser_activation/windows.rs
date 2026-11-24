use super::{DEADLINE, Inbox, accept};
use std::{
    io,
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
};
use tokio::net::windows::named_pipe::{
    ClientOptions, NamedPipeClient, NamedPipeServer, ServerOptions,
};
use windows::{
    Win32::{
        Foundation::{ERROR_ACCESS_DENIED, ERROR_PIPE_BUSY, HANDLE, HLOCAL, LocalFree},
        Security::{
            Authorization::{
                ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
            },
            GetTokenInformation, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TOKEN_GROUPS,
            TOKEN_QUERY, TokenLogonSid,
        },
        Storage::FileSystem::SECURITY_IDENTIFICATION,
        System::{
            Pipes::GetNamedPipeServerProcessId,
            Threading::{
                GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
            },
        },
    },
    core::{BOOL, PCWSTR, PWSTR},
};

pub(super) struct Endpoint {
    name: String,
    sid: String,
}

impl Endpoint {
    pub(super) fn new(scope: &str) -> io::Result<Self> {
        // SAFETY: the pseudo-handle refers to the current process.
        let sid = process_logon_sid(unsafe { GetCurrentProcess() })?;
        let name = format!(r"\\.\pipe\org.riina.player.browser-v1.{sid}.{scope}");
        Ok(Self { name, sid })
    }

    pub(super) fn listen(&self) -> io::Result<Listener> {
        let access_control: Vec<u16> = format!("D:P(A;;GA;;;{})", self.sid)
            .encode_utf16()
            .chain([0])
            .collect();
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        // SAFETY: the string is UTF-16 and terminated, and the out pointer is valid.
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                PCWSTR(access_control.as_ptr()),
                1,
                &mut descriptor,
                None,
            )
            .map_err(io::Error::from)?;
        }
        let mut attributes = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor.0,
            bInheritHandle: BOOL(0),
        };
        // SAFETY: CreateNamedPipe copies the descriptor during this call.
        let result = unsafe {
            let server = ServerOptions::new()
                .first_pipe_instance(true)
                .reject_remote_clients(true)
                .max_instances(1)
                .create_with_security_attributes_raw(&self.name, (&raw mut attributes).cast());
            let _ = LocalFree(Some(HLOCAL(descriptor.0)));
            server
        };
        result.map(Listener)
    }

    pub(super) async fn connect(&self) -> io::Result<NamedPipeClient> {
        let stream = loop {
            match ClientOptions::new()
                .security_qos_flags(SECURITY_IDENTIFICATION.0)
                .open(&self.name)
            {
                Ok(stream) => break stream,
                Err(error) if error.raw_os_error() == Some(ERROR_PIPE_BUSY.0 as i32) => {
                    tokio::time::sleep(std::time::Duration::from_millis(25)).await;
                }
                Err(error) => return Err(error),
            }
        };
        let mut pid = 0;
        // SAFETY: the pipe handle is borrowed from the live client.
        unsafe { GetNamedPipeServerProcessId(HANDLE(stream.as_raw_handle()), &mut pid) }
            .map_err(io::Error::from)?;
        // SAFETY: OpenProcess returns an owned handle on success.
        let server = unsafe {
            OwnedHandle::from_raw_handle(
                OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid)
                    .map_err(io::Error::from)?
                    .0,
            )
        };
        if process_logon_sid(HANDLE(server.as_raw_handle()))? != self.sid {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "The browser-link server belongs to a different logon session",
            ));
        }
        Ok(stream)
    }
}

pub(super) fn occupied(error: &io::Error) -> bool {
    error.raw_os_error().is_some_and(|code| {
        code == ERROR_ACCESS_DENIED.0 as i32 || code == ERROR_PIPE_BUSY.0 as i32
    })
}

fn process_logon_sid(process: HANDLE) -> io::Result<String> {
    let mut raw_token = HANDLE::default();
    // SAFETY: the process handle and the output pointer remain valid.
    let token = unsafe {
        OpenProcessToken(process, TOKEN_QUERY, &mut raw_token).map_err(io::Error::from)?;
        OwnedHandle::from_raw_handle(raw_token.0)
    };
    let token = HANDLE(token.as_raw_handle());
    let mut size = 0;
    // SAFETY: the null buffer asks Windows for the size of the TokenLogonSid record.
    let _ = unsafe { GetTokenInformation(token, TokenLogonSid, None, 0, &mut size) };
    if (size as usize) < size_of::<TOKEN_GROUPS>() {
        return Err(io::Error::other(
            "Could not determine the Windows logon session for browser links",
        ));
    }
    let mut record = vec![0usize; (size as usize).div_ceil(size_of::<usize>())];
    let mut sid = PWSTR::null();
    // SAFETY: record is aligned and large enough for TOKEN_GROUPS and its SID.
    // The allocated string survives until it is released with LocalFree below.
    unsafe {
        GetTokenInformation(
            token,
            TokenLogonSid,
            Some(record.as_mut_ptr().cast()),
            size,
            &mut size,
        )
        .map_err(io::Error::from)?;
        let groups = &*record.as_ptr().cast::<TOKEN_GROUPS>();
        if groups.GroupCount != 1 {
            return Err(io::Error::other(
                "Could not determine the Windows logon group for browser links",
            ));
        }
        ConvertSidToStringSidW(groups.Groups[0].Sid, &mut sid).map_err(io::Error::from)?;
        let result = sid.to_string().map_err(io::Error::other);
        let _ = LocalFree(Some(HLOCAL(sid.0.cast())));
        result
    }
}

pub(super) struct Listener(NamedPipeServer);

impl Listener {
    pub(super) async fn serve(mut self, inbox: &Inbox) -> io::Result<()> {
        loop {
            self.0.connect().await?;
            let _ = tokio::time::timeout(DEADLINE, accept(&mut self.0, inbox)).await;
            self.0.disconnect()?;
        }
    }
}
