use std::{
    cmp::Ordering,
    io::Write,
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
    let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
    let mut file = std::fs::File::create(&temporary)?;
    file.write_all(contents.as_bytes())?;
    file.sync_all()?;
    drop(file);
    // POSIX replacement stays on the same filesystem and never truncates the
    // previous preferences before the complete replacement is ready.
    std::fs::rename(&temporary, path)?;
    #[cfg(target_os = "linux")]
    if let Some(parent) = path.parent() {
        std::fs::File::open(parent)?.sync_all()?;
    }
    Ok(())
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

fn digit_run(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut run = String::new();
    while chars.peek().is_some_and(char::is_ascii_digit) {
        run.push(chars.next().expect("peeked digit"));
    }
    run
}

#[cfg(test)]
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
