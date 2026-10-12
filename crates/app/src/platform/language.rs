//! System locale services; metadata formatting stays outside the playback core.

#[cfg(target_os = "windows")]
pub(crate) fn current_locale_identifier() -> Option<String> {
    use windows::Win32::Globalization::GetUserDefaultLocaleName;
    // LOCALE_NAME_MAX_LENGTH from the Windows SDK, including the terminator.
    let mut name = [0_u16; 85];
    // The SDK bound includes the terminator. The wrapper borrows the complete
    // writable array for the call; decode only the reported initialized prefix.
    let length = unsafe { GetUserDefaultLocaleName(&mut name) };
    let length = usize::try_from(length).ok()?.checked_sub(1)?;
    String::from_utf16(name.get(..length)?).ok()
}

#[cfg(target_os = "linux")]
pub(crate) fn current_locale_identifier() -> Option<String> {
    sys_locale::get_locale()
}

#[cfg(target_os = "macos")]
pub(crate) fn current_locale_identifier() -> Option<String> {
    sys_locale::get_locale()
}

#[cfg(target_os = "macos")]
pub(crate) fn language_name(identifier: &str) -> Option<String> {
    use objc2_foundation::{NSLocale, NSString};
    objc2::rc::autoreleasepool(|_| {
        let name = NSLocale::currentLocale()
            .localizedStringForLocaleIdentifier(&NSString::from_str(identifier))
            .to_string();
        // Foundation returns a non-null retained string; keep empty output as
        // the caller's signal to fall back to its portable locale formatter.
        (!name.is_empty()).then_some(name)
    })
}
