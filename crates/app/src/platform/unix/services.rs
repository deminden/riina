use std::{
    cmp::Ordering,
    path::{Path, PathBuf},
};

#[cfg(target_os = "macos")]
pub fn register_default_file_types(
    video: bool,
    audio: bool,
    playlists: bool,
) -> std::io::Result<usize> {
    use objc2_core_foundation::{CFArray, CFRetained, CFString, CFURL};
    use std::{collections::BTreeSet, ptr::NonNull};

    let app_bundle = current_app_bundle()?;
    let app_url = CFURL::from_directory_path(&app_bundle).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Riina's app bundle path cannot be represented as a file URL",
        )
    })?;
    // This deprecated setter is retained to match IINA v1.5.0's bulk category
    // action; modern NSWorkspace setters require asynchronous per-type consent.
    // SAFETY: app_url is a live directory URL for the validated .app bundle.
    let registration_status = unsafe { LSRegisterURL(CFRetained::as_ptr(&app_url).as_ptr(), 1) };
    if registration_status != 0 {
        return Err(std::io::Error::other(format!(
            "Launch Services could not register Riina (status {registration_status})"
        )));
    }

    let tag_class = CFString::from_static_str("public.filename-extension");
    let file_types = crate::file_types::load()?;
    let mut identifiers = BTreeSet::new();
    for file_type in file_types
        .iter()
        .filter(|file_type| match file_type.category.as_str() {
            "video" => video,
            "audio" => audio,
            "playlist" => playlists,
            _ => false,
        })
    {
        for extension in &file_type.extensions {
            let extension = CFString::from_str(extension);
            // SAFETY: the CF strings are retained for the call and the null
            // conformance argument requests every registered identifier.
            let array = unsafe {
                UTTypeCreateAllIdentifiersForTag(
                    CFRetained::as_ptr(&tag_class).as_ptr(),
                    CFRetained::as_ptr(&extension).as_ptr(),
                    std::ptr::null(),
                )
            };
            let Some(array) = NonNull::new(array) else {
                continue;
            };
            // SAFETY: Apple's Create rule returns a retained CFArray whose
            // elements are CFString UTI identifiers.
            let array: CFRetained<CFArray<CFString>> = unsafe { CFRetained::from_raw(array) };
            identifiers.extend(array.iter().map(|identifier| identifier.to_string()));
        }
    }

    let bundle_identifier = CFString::from_static_str("org.riina.player");
    let mut registered = 0;
    let mut failed = 0;
    for identifier in identifiers {
        let identifier = CFString::from_str(&identifier);
        // SAFETY: both CF strings remain retained across the documented
        // Launch Services call; all roles matches the IINA action.
        let status = unsafe {
            LSSetDefaultRoleHandlerForContentType(
                CFRetained::as_ptr(&identifier).as_ptr(),
                u32::MAX,
                CFRetained::as_ptr(&bundle_identifier).as_ptr(),
            )
        };
        if status == 0 {
            registered += 1;
        } else {
            failed += 1;
        }
    }
    if registered == 0 || failed > 0 {
        Err(std::io::Error::other(format!(
            "Launch Services set {registered} media defaults and failed to set {failed}"
        )))
    } else {
        Ok(registered)
    }
}

#[cfg(target_os = "macos")]
fn current_app_bundle() -> std::io::Result<std::path::PathBuf> {
    let executable = std::env::current_exe()?;
    let bundle = executable
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .filter(|path| path.extension().is_some_and(|extension| extension == "app"))
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "Default-app registration requires Riina to run from its .app bundle",
            )
        })?;
    if !bundle.join("Contents/Info.plist").is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Riina's app bundle is missing Contents/Info.plist",
        ));
    }
    Ok(bundle.to_path_buf())
}

#[cfg(target_os = "linux")]
pub fn register_default_file_types(
    video: bool,
    audio: bool,
    playlists: bool,
) -> std::io::Result<usize> {
    use std::collections::BTreeSet;

    const DESKTOP_ID: &str = "org.riina.player.desktop";
    let file_types = crate::file_types::load()?;
    let selected_mime_types = file_types
        .iter()
        .filter(|file_type| match file_type.category.as_str() {
            "video" => video,
            "audio" => audio,
            "playlist" => playlists,
            _ => false,
        })
        .flat_map(|file_type| file_type.extensions.iter())
        .filter_map(|extension| mime_type_for_extension(extension))
        .collect::<BTreeSet<_>>();
    let supported_mime_types = file_types
        .iter()
        .flat_map(|file_type| file_type.extensions.iter())
        .filter_map(|extension| mime_type_for_extension(extension))
        .collect::<BTreeSet<_>>();
    if selected_mime_types.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "The selected media categories contain no registered MIME types",
        ));
    }

    let executable = std::env::current_exe()?;
    let data_directory = dirs::data_local_dir()
        .ok_or_else(|| std::io::Error::other("The user data directory is unavailable"))?;
    let applications = data_directory.join("applications");
    std::fs::create_dir_all(&applications)?;
    // The desktop entry declares all capabilities; only the selected categories
    // below change the user's defaults.
    let desktop_entry = format!(
        "[Desktop Entry]\nType=Application\nName=Riina\nComment=Play video and audio\nExec={} %F\nTerminal=false\nCategories=AudioVideo;Player;\nMimeType={};\n",
        desktop_exec_argument(&executable.to_string_lossy()),
        supported_mime_types
            .iter()
            .map(|mime| format!("{mime};"))
            .collect::<String>()
    );
    super::super::settings_file::save(&applications.join(DESKTOP_ID), &desktop_entry)?;

    let mut registered = 0;
    for mime_type in &selected_mime_types {
        let result = std::process::Command::new("xdg-mime")
            .args(["default", DESKTOP_ID, mime_type])
            .status()?;
        if !result.success() {
            return Err(std::io::Error::other(format!(
                "xdg-mime registered {registered} of {} media types, then failed for {mime_type} (status {result})",
                selected_mime_types.len()
            )));
        }
        registered += 1;
    }
    Ok(registered)
}

#[cfg(target_os = "linux")]
fn desktop_exec_argument(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('"');
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '`' => escaped.push_str("\\`"),
            '$' => escaped.push_str("\\$"),
            '%' => escaped.push_str("%%"),
            _ => escaped.push(character),
        }
    }
    escaped.push('"');
    escaped
}

#[cfg(target_os = "linux")]
fn mime_type_for_extension(extension: &str) -> Option<&'static str> {
    Some(match extension {
        "flv" => "video/x-flv",
        "f4v" | "f4p" => "video/mp4",
        "f4a" | "f4b" => "audio/mp4",
        "ape" => "audio/x-ape",
        "flac" => "audio/flac",
        "wv" => "audio/x-wavpack",
        "rm" | "rmvb" => "application/vnd.rn-realmedia",
        "ram" => "audio/vnd.rn-realaudio",
        "xm" => "audio/x-xm",
        "aiff" | "aif" => "audio/aiff",
        "wav" => "audio/wav",
        "ac3" | "a52" => "audio/ac3",
        "aac" => "audio/aac",
        "opus" => "audio/opus",
        "ogg" | "oga" => "audio/ogg",
        "avi" => "video/x-msvideo",
        "mov" | "moov" | "qt" => "video/quicktime",
        "divx" => "video/divx",
        "dv" => "video/dv",
        "asf" => "video/x-ms-asf",
        "wma" => "audio/x-ms-wma",
        "wmv" => "video/x-ms-wmv",
        "wtv" => "video/x-ms-wtv",
        "mpg" | "mpeg" | "mpeg1" | "mpeg2" | "m1v" | "m2v" => "video/mpeg",
        "mpv" | "m2p" | "ps" => "video/mpeg",
        "mp4" | "mpeg4" | "m4v" => "video/mp4",
        "m2a" | "mp1" | "mp2" | "mpa" | "mp3" | "mpg3" => "audio/mpeg",
        "m4a" => "audio/mp4",
        "m2ts" | "mts" | "mt2s" | "ts" => "video/mp2t",
        "3gp" | "3gpp" => "video/3gpp",
        "3g2" | "3gp2" => "video/3gpp2",
        "mka" => "audio/x-matroska",
        "mkv" => "video/x-matroska",
        "webm" => "video/webm",
        "cue" => "application/x-cue",
        "m3u8" => "application/vnd.apple.mpegurl",
        "m3u" => "audio/x-mpegurl",
        "dsf" => "audio/x-dsf",
        "dff" => "audio/x-dff",
        _ => return None,
    })
}

#[cfg(target_os = "macos")]
#[link(name = "CoreServices", kind = "framework")]
unsafe extern "C" {
    fn LSRegisterURL(url: *const objc2_core_foundation::CFURL, update: u8) -> i32;
    fn UTTypeCreateAllIdentifiersForTag(
        tag_class: *const objc2_core_foundation::CFString,
        tag: *const objc2_core_foundation::CFString,
        conforming_to: *const objc2_core_foundation::CFString,
    ) -> *mut objc2_core_foundation::CFArray<objc2_core_foundation::CFString>;
    fn LSSetDefaultRoleHandlerForContentType(
        content_type: *const objc2_core_foundation::CFString,
        role: u32,
        handler_bundle_id: *const objc2_core_foundation::CFString,
    ) -> i32;
}

pub fn preferences_path() -> std::io::Result<PathBuf> {
    let directory = dirs::config_dir()
        .ok_or_else(|| std::io::Error::other("The user configuration directory is unavailable"))?
        .join("riina");
    std::fs::create_dir_all(&directory)?;
    Ok(directory.join("preferences.json"))
}

pub fn save_preferences(path: &Path, contents: &str) -> std::io::Result<()> {
    super::super::settings_file::save(path, contents)
}

pub fn playback_library() -> PathBuf {
    #[cfg(target_os = "macos")]
    let name = "libmpv.2.dylib";
    #[cfg(target_os = "linux")]
    let name = "libmpv.so.2";
    if let Ok(executable) = std::env::current_exe() {
        let adjacent = executable.with_file_name(name);
        if adjacent.is_file() {
            return adjacent;
        }
        #[cfg(target_os = "macos")]
        if let Some(directory) = executable.parent() {
            let bundled = directory.join("../Frameworks").join(name);
            if bundled.is_file() {
                return bundled;
            }
        }
    }
    #[cfg(target_os = "macos")]
    for directory in ["/opt/homebrew/lib", "/usr/local/lib"] {
        let library = Path::new(directory).join(name);
        if library.is_file() {
            return library;
        }
    }
    // The system loader resolves a versioned SONAME. Media directories never
    // participate in discovery; user-installed codecs stay a separate dependency.
    PathBuf::from(name)
}

#[cfg(target_os = "macos")]
pub fn resolve_alias_path(path: &str) -> String {
    use objc2::rc::autoreleasepool;
    use objc2_foundation::{NSString, NSURL, NSURLBookmarkResolutionOptions};

    autoreleasepool(|_| {
        let source = NSURL::fileURLWithPath(&NSString::from_str(path));
        NSURL::URLByResolvingAliasFileAtURL_options_error(
            &source,
            NSURLBookmarkResolutionOptions::empty(),
        )
        .ok()
        .and_then(|resolved| resolved.path())
        .map(|resolved| resolved.to_string())
        .unwrap_or_else(|| path.to_owned())
    })
}

#[cfg(target_os = "macos")]
pub fn is_remote_volume(path: &Path) -> bool {
    use objc2::rc::autoreleasepool;
    use objc2_foundation::{NSNumber, NSString, NSURL, NSURLVolumeIsLocalKey};

    autoreleasepool(|_| {
        let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
        let mut value = None;
        // SAFETY: NSURL returns the documented NSNumber value for this resource key.
        let Ok(()) =
            (unsafe { url.getResourceValue_forKey_error(&mut value, NSURLVolumeIsLocalKey) })
        else {
            return false;
        };
        value
            .and_then(|value| value.downcast::<NSNumber>().ok())
            .is_some_and(|is_local| !is_local.boolValue())
    })
}

#[cfg(target_os = "linux")]
pub fn is_remote_volume(path: &Path) -> bool {
    use std::{fs, path::PathBuf};

    let Ok(mounts) = fs::read_to_string("/proc/self/mountinfo") else {
        return false;
    };
    let Some((mount, kind)) = mounts
        .lines()
        .filter_map(|line| {
            let (left, right) = line.split_once(" - ")?;
            let mount = left.split_whitespace().nth(4)?;
            let mount = PathBuf::from(unescape_mount_field(mount));
            let kind = right.split_whitespace().next()?;
            path.starts_with(&mount).then_some((mount, kind))
        })
        .max_by_key(|(mount, _)| mount.components().count())
    else {
        return false;
    };
    let _ = mount;
    matches!(
        kind,
        "nfs"
            | "nfs4"
            | "cifs"
            | "smb3"
            | "afpfs"
            | "davfs"
            | "sshfs"
            | "9p"
            | "ceph"
            | "glusterfs"
    ) || kind.starts_with("fuse.sshfs")
}

#[cfg(target_os = "linux")]
fn unescape_mount_field(value: &str) -> String {
    value
        .replace("\\040", " ")
        .replace("\\011", "\t")
        .replace("\\012", "\n")
        .replace("\\134", "\\")
}

#[cfg(target_os = "macos")]
pub fn natural_compare(left: &str, right: &str) -> Ordering {
    use objc2_foundation::NSString;
    // Discovery runs on a Rust worker, outside AppKit's event-loop pool. Drain
    // Foundation temporaries here while retaining both strings through the call.
    // Native localizedStandardCompare follows the user's Finder ordering.
    objc2::rc::autoreleasepool(|_| {
        let left = NSString::from_str(left);
        let right = NSString::from_str(right);
        left.localizedStandardCompare(&right).into()
    })
}

#[cfg(target_os = "linux")]
pub fn natural_compare(left: &str, right: &str) -> Ordering {
    let mut left = left.chars().peekable();
    let mut right = right.chars().peekable();
    loop {
        match (left.peek(), right.peek()) {
            (Some(a), Some(b)) if a.is_ascii_digit() && b.is_ascii_digit() => {
                let a = digit_run(&mut left);
                let b = digit_run(&mut right);
                let a = a.trim_start_matches('0');
                let b = b.trim_start_matches('0');
                let order = a.len().cmp(&b.len()).then_with(|| a.cmp(b));
                if order != Ordering::Equal {
                    return order;
                }
            }
            _ => match (left.next(), right.next()) {
                (Some(a), Some(b)) if a == b => {}
                (Some(a), Some(b)) => return a.cmp(&b),
                (None, None) => return Ordering::Equal,
                (None, _) => return Ordering::Less,
                (_, None) => return Ordering::Greater,
            },
        }
    }
}

#[cfg(target_os = "linux")]
fn digit_run(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut run = String::new();
    while chars.peek().is_some_and(char::is_ascii_digit) {
        run.push(chars.next().expect("peeked digit"));
    }
    run
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::natural_compare;
    use std::cmp::Ordering;

    #[test]
    fn filename_numbers_do_not_overflow_or_consume_the_next_character() {
        for (left, right) in [
            ("clip2.mkv", "clip10.mkv"),
            ("clip0002a.mkv", "clip2b.mkv"),
            (
                "clip999999999999999999999999999999",
                "clip1000000000000000000000000000000",
            ),
            ("音楽2.wav", "音楽10.wav"),
        ] {
            assert_eq!(natural_compare(left, right), Ordering::Less);
            assert_eq!(natural_compare(right, left), Ordering::Greater);
        }
        assert_eq!(natural_compare("clip000", "clip0"), Ordering::Equal);
        assert_eq!(natural_compare("", ""), Ordering::Equal);
    }
}

#[cfg(all(test, target_os = "macos"))]
mod macos_tests {
    use super::natural_compare;
    use std::cmp::Ordering;

    #[test]
    fn native_filename_order_accepts_unicode_on_discovery_workers() {
        std::thread::spawn(|| {
            for prefix in ["clip", "音楽", "épisode"] {
                let left = format!("{prefix}2.mkv");
                let right = format!("{prefix}10.mkv");
                assert_eq!(natural_compare(&left, &right), Ordering::Less);
                assert_eq!(natural_compare(&right, &left), Ordering::Greater);
                assert_eq!(natural_compare(&left, &left), Ordering::Equal);
            }
        })
        .join()
        .unwrap();
    }
}

#[cfg(test)]
mod storage_tests {
    use super::save_preferences;
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let base = std::env::temp_dir().canonicalize().unwrap();
            loop {
                let path = base.join(format!(
                    "riina-storage-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
                match std::fs::create_dir(&path) {
                    Ok(()) => return Self(path),
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(error) => panic!("Cannot create storage fixture: {error}"),
                }
            }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            assert_eq!(
                self.0.parent(),
                Some(std::env::temp_dir().canonicalize().unwrap().as_path())
            );
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn concurrent_replacements_preserve_complete_unicode_preferences() {
        let fixture = Fixture::new();
        let path = fixture.0.join("preferences.json");
        save_preferences(&path, "initial").unwrap();
        let values: Vec<_> = (0..8)
            .map(|index| format!("{{\"title\":\"音楽 {index}\",\"volume_boost\":false}}"))
            .collect();
        std::thread::scope(|scope| {
            for value in &values {
                let path = &path;
                scope.spawn(move || save_preferences(path, value).unwrap());
            }
        });
        assert!(values.contains(&std::fs::read_to_string(&path).unwrap()));
        assert_eq!(std::fs::read_dir(&fixture.0).unwrap().count(), 1);
    }

    #[test]
    fn failed_replacement_keeps_destination_and_cleans_owned_temporary() {
        let fixture = Fixture::new();
        let path = fixture.0.join("preferences.json");
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("retained"), "keep").unwrap();
        assert!(save_preferences(&path, "replacement").is_err());
        assert_eq!(
            std::fs::read_to_string(path.join("retained")).unwrap(),
            "keep"
        );
        assert_eq!(std::fs::read_dir(&fixture.0).unwrap().count(), 1);
    }
}
