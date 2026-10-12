use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use windows::{
    Win32::{
        Storage::FileSystem::{GetDriveTypeW, GetVolumePathNameW},
        System::WindowsProgramming::DRIVE_REMOTE,
        System::{
            Com::{
                CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
                CoUninitialize, IPersistFile, STGM_READ,
            },
            Registry::{
                HKEY, HKEY_CURRENT_USER, REG_NONE, REG_SZ, RegCloseKey, RegCreateKeyW,
                RegDeleteValueW, RegSetValueExW,
            },
        },
        UI::Shell::{IShellLinkW, ShellLink},
    },
    core::{Interface, PCWSTR},
};

pub fn resolve_shortcut_path(path: &Path) -> Option<PathBuf> {
    let is_shell_link = path
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .is_some_and(|extension| extension.eq_ignore_ascii_case("lnk"));
    if !is_shell_link {
        return None;
    }

    // File selection runs on a worker thread, so this apartment is independent
    // from the UI thread and remains alive for every shell interface call below.
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }
        .ok()
        .ok()?;
    struct Apartment;
    impl Drop for Apartment {
        fn drop(&mut self) {
            unsafe { CoUninitialize() };
        }
    }
    let _apartment = Apartment;

    let link: IShellLinkW =
        unsafe { CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER) }.ok()?;
    let persisted: IPersistFile = link.cast().ok()?;
    let source = path
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    unsafe { persisted.Load(PCWSTR(source.as_ptr()), STGM_READ) }.ok()?;

    let mut target = vec![0_u16; 32_768];
    unsafe { link.GetPath(&mut target, std::ptr::null_mut(), 0) }.ok()?;
    let length = target.iter().position(|unit| *unit == 0)?;
    if length == 0 {
        return None;
    }
    Some(PathBuf::from(std::ffi::OsString::from_wide(
        &target[..length],
    )))
}

pub fn is_remote_volume(path: &Path) -> bool {
    let source = path
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let mut volume = vec![0_u16; 1024];
    if unsafe { GetVolumePathNameW(PCWSTR(source.as_ptr()), &mut volume) }.is_err() {
        return false;
    }
    unsafe { GetDriveTypeW(PCWSTR(volume.as_ptr())) == DRIVE_REMOTE }
}

pub fn register_default_file_types(
    video: bool,
    audio: bool,
    playlists: bool,
) -> std::io::Result<usize> {
    use std::os::windows::ffi::OsStrExt;

    let executable = std::env::current_exe()?;
    let executable_wide = executable.as_os_str().encode_wide().collect::<Vec<_>>();
    let executable_name = executable
        .file_name()
        .ok_or_else(|| std::io::Error::other("Riina executable name is unavailable"))?
        .to_string_lossy()
        .into_owned();
    let file_types = crate::file_types::load()?;
    let mut extensions = file_types
        .iter()
        .filter(|file_type| match file_type.category.as_str() {
            "video" => video,
            "audio" => audio,
            "playlist" => playlists,
            _ => false,
        })
        .flat_map(|file_type| file_type.extensions.iter().cloned())
        .collect::<Vec<_>>();
    extensions.sort_unstable();
    extensions.dedup();

    for extension in file_types
        .iter()
        .flat_map(|file_type| &file_type.extensions)
    {
        if !extensions.contains(extension) {
            let prog_id = format!("Riina.{extension}");
            delete_registry_value(
                "Software\\Riina\\Capabilities\\FileAssociations",
                &format!(".{extension}"),
            )?;
            delete_registry_value(
                &format!("Software\\Classes\\.{extension}\\OpenWithProgids"),
                &prog_id,
            )?;
            delete_registry_value(
                &format!("Software\\Classes\\Applications\\{executable_name}\\SupportedTypes"),
                &format!(".{extension}"),
            )?;
        }
    }

    let mut capabilities = Vec::with_capacity(extensions.len());
    for extension in &extensions {
        let prog_id = format!("Riina.{extension}");
        let description = format!("{} file", extension.to_ascii_uppercase());
        let icon = executable_icon_value(&executable_wide);
        let command = open_command_value(&executable_wide);

        set_registry_string(
            &format!("Software\\Classes\\Applications\\{executable_name}\\SupportedTypes"),
            &format!(".{extension}"),
            "",
        )?;
        set_registry_string(&format!("Software\\Classes\\{prog_id}"), "", &description)?;
        set_registry_wide(
            &format!("Software\\Classes\\{prog_id}\\DefaultIcon"),
            "",
            &icon,
        )?;
        set_registry_wide(
            &format!("Software\\Classes\\{prog_id}\\shell\\open\\command"),
            "",
            &command,
        )?;
        set_registry_none(
            &format!("Software\\Classes\\.{extension}\\OpenWithProgids"),
            &prog_id,
        )?;
        capabilities.push((format!(".{extension}"), prog_id));
    }

    set_registry_string(
        "Software\\RegisteredApplications",
        "Riina",
        "Software\\Riina\\Capabilities",
    )?;
    set_registry_string("Software\\Riina\\Capabilities", "ApplicationName", "Riina")?;
    set_registry_string(
        "Software\\Riina\\Capabilities",
        "ApplicationDescription",
        "Media player",
    )?;
    set_registry_wide(
        "Software\\Riina\\Capabilities",
        "ApplicationIcon",
        &executable_icon_value(&executable_wide),
    )?;
    for (extension, prog_id) in &capabilities {
        set_registry_string(
            "Software\\Riina\\Capabilities\\FileAssociations",
            extension,
            prog_id,
        )?;
    }
    Ok(capabilities.len())
}

fn set_registry_string(path: &str, name: &str, value: &str) -> std::io::Result<()> {
    set_registry_wide(path, name, &wide(value))
}

fn set_registry_wide(path: &str, name: &str, value: &[u16]) -> std::io::Result<()> {
    let key = open_registry_key(path)?;
    let name = wide(name);
    let value = registry_string_wide(value);
    // SAFETY: the key is open, strings are NUL-terminated UTF-16, and the data
    // slice contains the matching UTF-16 bytes including the terminator.
    let status = unsafe {
        RegSetValueExW(
            key,
            PCWSTR(name.as_ptr()),
            None,
            REG_SZ,
            Some(value.as_slice()),
        )
    };
    close_registry_key(key, status.0)
}

fn set_registry_none(path: &str, name: &str) -> std::io::Result<()> {
    let key = open_registry_key(path)?;
    let name = wide(name);
    // SAFETY: the key and NUL-terminated value name remain valid for the call;
    // REG_NONE is used as the OpenWithProgids marker and has no data payload.
    let status = unsafe { RegSetValueExW(key, PCWSTR(name.as_ptr()), None, REG_NONE, None) };
    close_registry_key(key, status.0)
}

fn delete_registry_value(path: &str, name: &str) -> std::io::Result<()> {
    let key = open_registry_key(path)?;
    let name = wide(name);
    // SAFETY: the open key and NUL-terminated UTF-16 value name remain valid
    // for the duration of this value-only removal.
    let status = unsafe { RegDeleteValueW(key, PCWSTR(name.as_ptr())) };
    if status.0 == 2 {
        close_registry_key(key, 0)
    } else {
        close_registry_key(key, status.0)
    }
}

fn open_registry_key(path: &str) -> std::io::Result<HKEY> {
    let path = wide(path);
    let mut key = HKEY::default();
    // SAFETY: the path is NUL-terminated UTF-16, and `key` is a valid output
    // slot for the handle returned by RegCreateKeyW.
    let status = unsafe { RegCreateKeyW(HKEY_CURRENT_USER, PCWSTR(path.as_ptr()), &mut key) };
    if status.0 == 0 {
        Ok(key)
    } else {
        Err(std::io::Error::from_raw_os_error(status.0 as i32))
    }
}

fn close_registry_key(key: HKEY, status: u32) -> std::io::Result<()> {
    // SAFETY: `key` was returned by RegCreateKeyW and is closed exactly once.
    let close_status = unsafe { RegCloseKey(key) };
    if status != 0 {
        Err(std::io::Error::from_raw_os_error(status as i32))
    } else if close_status.0 != 0 {
        Err(std::io::Error::from_raw_os_error(close_status.0 as i32))
    } else {
        Ok(())
    }
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

fn registry_string_wide(value: &[u16]) -> Vec<u8> {
    value
        .iter()
        .copied()
        .chain((value.last() != Some(&0)).then_some(0))
        .flat_map(u16::to_le_bytes)
        .collect()
}

fn quoted_executable_value(executable: &[u16], with_index: bool) -> Vec<u16> {
    let mut value = Vec::with_capacity(executable.len() + 5);
    value.push(b'"' as u16);
    value.extend_from_slice(executable);
    value.push(b'"' as u16);
    if with_index {
        value.extend([b',' as u16, b'0' as u16]);
    }
    value
}

fn executable_icon_value(executable: &[u16]) -> Vec<u16> {
    let mut value = Vec::with_capacity(executable.len() + 2);
    value.extend_from_slice(executable);
    value.extend([b',' as u16, b'0' as u16]);
    value
}

fn open_command_value(executable: &[u16]) -> Vec<u16> {
    let mut value = quoted_executable_value(executable, false);
    value.extend([
        b' ' as u16,
        b'"' as u16,
        b'%' as u16,
        b'1' as u16,
        b'"' as u16,
    ]);
    value
}

pub fn preferences_path() -> std::io::Result<PathBuf> {
    let data = std::env::var_os("LOCALAPPDATA")
        .ok_or_else(|| std::io::Error::other("LOCALAPPDATA is unavailable"))?;
    let data = PathBuf::from(data);
    let directory = data.join("riina");
    std::fs::create_dir_all(&directory)?;
    let path = directory.join("preferences.json");
    let legacy = data.join("wiina").join("preferences.json");
    // Preserve the first installation's settings without overwriting riina's own
    // preferences or moving the legacy file out from under an older executable.
    if !path.exists() && legacy.is_file() {
        save_preferences(&path, &std::fs::read_to_string(legacy)?)?;
    }
    Ok(path)
}

pub fn save_preferences(path: &Path, contents: &str) -> std::io::Result<()> {
    super::super::settings_file::save(path, contents)
}

pub fn playback_library() -> PathBuf {
    // Development uses the pinned local dependency; installed builds resolve only
    // beside their executable, never beside media or from PATH.
    let installed = std::env::current_exe()
        .unwrap_or_default()
        .with_file_name("libmpv-2.dll");
    if installed.is_file() {
        installed
    } else {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tools/mpv/libmpv-2.dll")
    }
}

use std::cmp::Ordering;
use windows::Win32::UI::Shell::StrCmpLogicalW;
pub fn natural_compare(left: &str, right: &str) -> Ordering {
    let left: Vec<u16> = left.encode_utf16().chain(Some(0)).collect();
    let right: Vec<u16> = right.encode_utf16().chain(Some(0)).collect();
    // SAFETY: both arrays are NUL-terminated and live throughout this read-only call.
    // Explorer's comparator supplies the Windows equivalent of localizedStandardCompare.
    // Locale-specific ordering is an explicit platform adaptation, not pixel/byte parity.
    unsafe { StrCmpLogicalW(PCWSTR(left.as_ptr()), PCWSTR(right.as_ptr())).cmp(&0) }
}
