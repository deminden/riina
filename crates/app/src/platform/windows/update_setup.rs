//! Locate an Inno Setup installation and open its interactive upgrade wizard.
use std::{
    ffi::{OsStr, OsString},
    fs, io,
    os::windows::{
        ffi::{OsStrExt, OsStringExt},
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
    },
    path::{Path, PathBuf},
};
use windows::{
    Win32::{
        Foundation::{ERROR_SUCCESS, HANDLE, WAIT_OBJECT_0},
        System::{
            Com::{
                COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx, CoUninitialize,
            },
            Registry::{HKEY_CURRENT_USER, RRF_RT_REG_SZ, RegGetValueW},
            Threading::{GetExitCodeProcess, INFINITE, WaitForSingleObject},
        },
        UI::{
            Shell::{
                SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW,
            },
            WindowsAndMessaging::SW_SHOWNORMAL,
        },
    },
    core::{PCWSTR, w},
};

pub(crate) fn installed_destination() -> Option<PathBuf> {
    let executable = std::env::current_exe().ok()?;
    let registered = registered_directory()?;
    installation_matches(&executable, &registered).then_some(registered)
}

fn installation_matches(executable: &Path, registered: &Path) -> bool {
    if !registered.is_absolute() {
        return false;
    }
    let Ok(expected) = registered.join("riina.exe").canonicalize() else {
        return false;
    };
    executable
        .canonicalize()
        .is_ok_and(|actual| actual == expected)
}

fn registered_directory() -> Option<PathBuf> {
    let key = w!(
        "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\{575C5AEC-C763-49F6-BF2A-0D34BB908A94}_is1"
    );
    let value = w!("InstallLocation");
    let mut bytes = 0;
    // SAFETY: the first query requests the size only; both names are static UTF-16.
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key,
            value,
            RRF_RT_REG_SZ,
            None,
            None,
            Some(&mut bytes),
        )
    };
    if status != ERROR_SUCCESS || !(2..=65_536).contains(&bytes) || bytes % 2 != 0 {
        return None;
    }
    let mut buffer = vec![0_u16; bytes as usize / 2];
    // SAFETY: bytes records the capacity of the live UTF-16 buffer.
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key,
            value,
            RRF_RT_REG_SZ,
            None,
            Some(buffer.as_mut_ptr().cast()),
            Some(&mut bytes),
        )
    };
    if status != ERROR_SUCCESS {
        return None;
    }
    let end = buffer.iter().position(|&unit| unit == 0)?;
    Some(PathBuf::from(OsString::from_wide(&buffer[..end])))
}

pub(crate) fn mark_download(path: &Path, origin: &str) -> io::Result<()> {
    let mut stream = path.as_os_str().to_owned();
    stream.push(":Zone.Identifier");
    fs::write(
        stream,
        format!("[ZoneTransfer]\r\nZoneId=3\r\nHostUrl={origin}\r\n"),
    )
}

pub(crate) struct Setup(OwnedHandle);

impl Setup {
    // Let Windows handle reputation prompts and return the launched process for cleanup.
    pub(crate) fn open(path: &Path, directory: &Path) -> io::Result<Self> {
        let file = wide(path.as_os_str());
        let working_directory = wide(path.parent().unwrap_or(path).as_os_str());
        let mut argument = OsString::from("/NORESTART /DIR=\"");
        argument.push(directory);
        // Backslashes immediately before the closing quote are escaped in argv.
        if directory.as_os_str().encode_wide().last() == Some(u16::from(b'\\')) {
            argument.push("\\");
        }
        argument.push("\"");
        let parameters = wide(&argument);
        let mut info = SHELLEXECUTEINFOW {
            cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
            fMask: SEE_MASK_NOASYNC | SEE_MASK_NOCLOSEPROCESS,
            lpVerb: w!("open"),
            lpFile: PCWSTR(file.as_ptr()),
            lpParameters: PCWSTR(parameters.as_ptr()),
            lpDirectory: PCWSTR(working_directory.as_ptr()),
            nShow: SW_SHOWNORMAL.0,
            ..Default::default()
        };
        // SAFETY: this is a dedicated worker. All strings and info outlive the call.
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE)
                .ok()
                .map_err(io::Error::other)?;
            let result = ShellExecuteExW(&mut info);
            CoUninitialize();
            result.map_err(io::Error::other)?;
            if info.hProcess.is_invalid() {
                return Err(io::Error::other("Windows did not return a Setup process."));
            }
            Ok(Self(OwnedHandle::from_raw_handle(info.hProcess.0)))
        }
    }

    pub(crate) fn wait(self) -> io::Result<u32> {
        let handle = HANDLE(self.0.as_raw_handle());
        let mut exit_code = 0;
        // SAFETY: the owned process handle stays live for both calls.
        unsafe {
            if WaitForSingleObject(handle, INFINITE) != WAIT_OBJECT_0 {
                return Err(io::Error::last_os_error());
            }
            GetExitCodeProcess(handle, &mut exit_code).map_err(io::Error::other)?;
        }
        Ok(exit_code)
    }
}

fn wide(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain([0]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_portable_copy_is_not_the_registered_installation() -> io::Result<()> {
        let installed = riina_test_support::TempDir::new("installer-location")?;
        let portable = riina_test_support::TempDir::new("portable-location")?;
        let expected = installed.path().join("riina.exe");
        let other = portable.path().join("riina.exe");
        fs::write(&expected, b"fixture")?;
        fs::write(&other, b"fixture")?;
        assert!(installation_matches(&expected, installed.path()));
        assert!(!installation_matches(&other, installed.path()));
        Ok(())
    }

    #[test]
    #[ignore = "requires a benign Windows Setup stand-in on an isolated desktop"]
    fn interactive_setup_handoff_uses_the_selected_directory() -> io::Result<()> {
        let fixture = PathBuf::from(std::env::var_os("RIINA_TEST_SETUP_FIXTURE").unwrap());
        let output = fixture.with_extension("txt");
        let temporary = riina_test_support::TempDir::new("setup-handoff")?;
        let destination = temporary.path().join("Riina player é");
        fs::create_dir(&destination)?;
        assert_eq!(Setup::open(&fixture, &destination)?.wait()?, 7);
        let args = fs::read_to_string(output)?;
        let expected = format!("/NORESTART\n/DIR={}", destination.display());
        assert_eq!(args.replace("\r\n", "\n").trim(), expected);
        Ok(())
    }
}
