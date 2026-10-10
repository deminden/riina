use std::{
    cmp::Ordering,
    path::{Path, PathBuf},
};

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
