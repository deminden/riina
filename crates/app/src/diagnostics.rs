//! App-owned logging for startup, shutdown and background failures.
use std::{
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, LineWriter, Write},
    path::Path,
    sync::{Mutex, OnceLock},
};

const ROTATION_LIMIT: u64 = 2 * 1024 * 1024;
static LOG: OnceLock<Mutex<LineWriter<File>>> = OnceLock::new();

#[derive(Clone, Copy)]
pub(crate) enum Level {
    Info,
    Warning,
    Error,
}

impl Level {
    fn label(self) -> &'static str {
        match self {
            Self::Info => "INFO",
            Self::Warning => "WARN",
            Self::Error => "ERROR",
        }
    }
}

/// An action failure with a presentation message and log severity.
pub(crate) struct UserMessage {
    text: String,
    level: Level,
}

impl UserMessage {
    pub(crate) fn info(text: String) -> Self {
        Self {
            text,
            level: Level::Info,
        }
    }
}

impl From<String> for UserMessage {
    fn from(text: String) -> Self {
        Self {
            text,
            level: Level::Warning,
        }
    }
}

impl From<riina_mpv::EngineError> for UserMessage {
    fn from(error: riina_mpv::EngineError) -> Self {
        use riina_mpv::EngineError;
        let level = match error {
            EngineError::QueueFull => Level::Warning,
            EngineError::Stopped => Level::Error,
            _ => Level::Warning,
        };
        Self {
            text: error.to_string(),
            level,
        }
    }
}

pub(crate) fn report(app: &crate::PlayerWindow, message: impl Into<UserMessage>) {
    let message = message.into();
    log(message.level, "player", &message.text);
    app.set_status(message.text.into());
}

pub(crate) fn init(profile: &Path) -> io::Result<()> {
    let file = open_log(profile)?;
    // The first successful initialization owns logging for the process.
    let _ = LOG.set(Mutex::new(file));
    Ok(())
}

fn open_log(profile: &Path) -> io::Result<LineWriter<File>> {
    let current = profile.join("riina.log");
    if fs::metadata(&current).is_ok_and(|metadata| metadata.len() > ROTATION_LIMIT) {
        match fs::rename(&current, profile.join("riina.previous.log")) {
            Ok(()) => {}
            // Another player can rotate the same profile's log first.
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(current)
        .map(LineWriter::new)
}

pub(crate) fn log(level: Level, component: &str, message: &str) {
    let message = redact(message);
    let timestamp = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ");
    let record = format!("{timestamp} {} [{component}] {message}\n", level.label());
    // These sinks cannot report their own write errors through the logger.
    if let Some(log) = LOG.get() {
        let mut file = log
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let _ = file.write_all(record.as_bytes());
    }
    if cfg!(debug_assertions) {
        let _ = io::stderr().write_all(record.as_bytes());
    }
}

pub(crate) fn log_args(level: Level, component: &str, args: fmt::Arguments<'_>) {
    match args.as_str() {
        Some(message) => log(level, component, message),
        None => log(level, component, &args.to_string()),
    }
}

macro_rules! warning {
    ($($args:tt)*) => {
        $crate::diagnostics::log_args($crate::diagnostics::Level::Warning, module_path!(), format_args!($($args)*))
    };
}

macro_rules! error {
    ($($args:tt)*) => {
        $crate::diagnostics::log_args($crate::diagnostics::Level::Error, module_path!(), format_args!($($args)*))
    };
}

pub(crate) use {error, warning as warn};

pub(crate) fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current();
        // Formatted payloads can include arbitrary input. Keep fixed messages
        // and the location, which still identifies formatted panic sites.
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .copied()
            .unwrap_or("Panic with a runtime message");
        let location = info.location().map_or_else(
            || "unknown location".into(),
            |location| {
                format!(
                    "{}:{}:{}",
                    location
                        .file()
                        .rsplit(['/', '\\'])
                        .next()
                        .unwrap_or_default(),
                    location.line(),
                    location.column(),
                )
            },
        );
        log(
            Level::Error,
            "panic",
            &format!(
                "Thread {} at {location}: {}",
                redact(thread.name().unwrap_or("unnamed")),
                redact(message)
            ),
        );
        previous(info);
    }));
}

fn redact(message: &str) -> String {
    let message = message.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut retained = 0;
    for token in message.split(' ') {
        let lower = token.to_ascii_lowercase();
        let named_secret = ["authorization", "password", "api-key", "api_key", "token"]
            .iter()
            .any(|name| lower.starts_with(name) && lower[name.len()..].starts_with([':', '=']));
        // Paths or URLs can contain spaces. Drop the remainder of the record
        // once an opaque value starts so its subsequent pieces cannot leak.
        if named_secret || token.contains(['/', '\\']) {
            return format!("{}[redacted]", &message[..retained]);
        }
        retained += token.len() + 1;
    }
    message
}

#[cfg(test)]
mod tests {
    use super::*;

    use riina_test_support::TempDir;

    #[test]
    fn playback_send_errors_have_distinct_severities_and_keep_their_text() {
        let full = UserMessage::from(riina_mpv::EngineError::QueueFull);
        let stopped = UserMessage::from(riina_mpv::EngineError::Stopped);
        assert!(matches!(full.level, Level::Warning));
        assert!(matches!(stopped.level, Level::Error));
        assert_eq!(
            full.text,
            "Player command was not accepted: sending on a full channel"
        );
        assert_eq!(stopped.text, "The playback engine has stopped");
    }

    #[test]
    fn rotates_only_above_the_threshold_and_replaces_the_previous_log() {
        let profile = TempDir::new("diagnostics").unwrap();
        let current = profile.path().join("riina.log");
        let previous = profile.path().join("riina.previous.log");
        File::create(&current)
            .unwrap()
            .set_len(ROTATION_LIMIT)
            .unwrap();
        drop(open_log(profile.path()).unwrap());
        assert!(!previous.exists());
        assert_eq!(fs::metadata(&current).unwrap().len(), ROTATION_LIMIT);

        fs::write(&previous, "old log").unwrap();
        OpenOptions::new()
            .write(true)
            .open(&current)
            .unwrap()
            .set_len(ROTATION_LIMIT + 1)
            .unwrap();
        writeln!(open_log(profile.path()).unwrap(), "new log").unwrap();
        assert_eq!(fs::metadata(&previous).unwrap().len(), ROTATION_LIMIT + 1);
        assert_eq!(fs::read_to_string(&current).unwrap(), "new log\n");
    }

    #[test]
    fn panic_hook_records_a_named_thread_and_location_in_an_isolated_process() {
        let profile = TempDir::new("diagnostics").unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "diagnostics::tests::panic_hook_child",
                "--ignored",
            ])
            .env("RIINA_TEST_DIAGNOSTICS_PROFILE", profile.path())
            .status()
            .unwrap();
        assert!(status.success());
        let log = fs::read_to_string(profile.path().join("riina.log")).unwrap();
        assert!(log.contains("ERROR [panic] Thread diagnostic-test at diagnostics.rs:"));
        assert!(log.contains("controlled panic"));
    }

    #[test]
    #[ignore = "helper run by panic_hook_records_a_named_thread_and_location_in_an_isolated_process"]
    fn panic_hook_child() {
        let profile = std::env::var_os("RIINA_TEST_DIAGNOSTICS_PROFILE").unwrap();
        init(Path::new(&profile)).unwrap();
        install_panic_hook();
        std::thread::Builder::new()
            .name("diagnostic-test".into())
            .spawn(|| assert!(std::panic::catch_unwind(|| panic!("controlled panic")).is_err()))
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn log_redaction_removes_private_suffixes_and_line_breaks() {
        for message in [
            r"Open failed C:\Private Files\movie.mkv",
            "Open failed https://example.test/secret",
            "Open failed Authorization: Bearer example",
            "Open failed api_key=example",
            "Open failed /home/user/movie.mkv",
        ] {
            assert_eq!(redact(message), "Open failed [redacted]");
        }
        assert_eq!(redact("Engine not ready\nRetry"), "Engine not ready Retry");
    }
}
