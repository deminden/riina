use std::path::{Path, PathBuf};
use windows::core::PCWSTR;

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
