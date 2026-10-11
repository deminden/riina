//! Bounded JavaScript entry evaluation for permission-reviewed plugins.

use riina_core::plugin::Manifest;
use rquickjs::{Context, Object, Runtime, Value, function::Func};
use std::{
    fs,
    path::Path,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

const MAX_ENTRY_BYTES: usize = 1024 * 1024;
const MAX_RUNTIME_MEMORY: usize = 32 * 1024 * 1024;
const MAX_EXECUTION_TIME: Duration = Duration::from_millis(500);
const MAX_STACK_BYTES: usize = 256 * 1024;
const MAX_HOST_EVENTS: usize = 128;
const MAX_MESSAGE_CHARS: usize = 2048;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostEvent {
    Log(String),
    ShowOsd(String),
    ShowAlert(String),
}

/// Resolve and execute the already-validated entry again at activation time.
/// Rechecking containment prevents a replaced package file from escaping its root.
pub fn execute_package_entry(
    package_root: &Path,
    manifest: &Manifest,
    granted_permissions: &[String],
) -> Result<Vec<HostEvent>, String> {
    let root = package_root
        .canonicalize()
        .map_err(|error| format!("Could not open plugin package: {error}"))?;
    if !root.is_dir() {
        return Err("Plugin package is not a directory".into());
    }
    let entry = root
        .join(&manifest.entry)
        .canonicalize()
        .map_err(|error| format!("Could not open plugin entry: {error}"))?;
    if !entry.starts_with(&root) || !entry.is_file() {
        return Err("Plugin entry must be a file inside its package".into());
    }
    let length = fs::metadata(&entry)
        .map_err(|error| format!("Could not inspect plugin entry: {error}"))?
        .len();
    if length > MAX_ENTRY_BYTES as u64 {
        return Err("Plugin entry exceeds the 1 MiB execution limit".into());
    }
    let bytes = fs::read(entry).map_err(|error| format!("Could not read plugin entry: {error}"))?;
    if bytes.len() > MAX_ENTRY_BYTES {
        return Err("Plugin entry exceeds the 1 MiB execution limit".into());
    }
    let source =
        std::str::from_utf8(&bytes).map_err(|_| "Plugin entry is not valid UTF-8".to_owned())?;
    execute_entry(
        source,
        &manifest.identifier,
        &manifest.permissions,
        granted_permissions,
    )
}

/// Execute one plugin entry in a fresh realm with only its granted host calls.
/// The caller should run this on a worker thread: plugins are third-party scripts.
pub fn execute_entry(
    source: &str,
    plugin_identifier: &str,
    requested_permissions: &[String],
    granted_permissions: &[String],
) -> Result<Vec<HostEvent>, String> {
    if source.len() > MAX_ENTRY_BYTES {
        return Err("Plugin entry exceeds the 1 MiB execution limit".into());
    }
    let permissions = reviewed_permissions(requested_permissions, granted_permissions)?;
    let deadline = Instant::now() + MAX_EXECUTION_TIME;
    let runtime =
        Runtime::new().map_err(|error| format!("Could not create JavaScript runtime: {error}"))?;
    runtime.set_memory_limit(MAX_RUNTIME_MEMORY);
    runtime.set_max_stack_size(MAX_STACK_BYTES);
    // QuickJS calls this while evaluating bytecode, so runaway synchronous code
    // returns control without blocking the player event loop indefinitely.
    runtime.set_interrupt_handler(Some(Box::new(move || Instant::now() >= deadline)));
    runtime
        .set_info(format!("riina plugin {plugin_identifier}"))
        .map_err(|error| format!("Could not label JavaScript runtime: {error}"))?;

    let events = Arc::new(Mutex::new(Vec::new()));
    let context = Context::full(&runtime)
        .map_err(|error| format!("Could not create plugin context: {error}"))?;
    context
        .with(|ctx| {
            let globals = ctx.globals();
            let api = Object::new(ctx.clone())?;

            let log_events = events.clone();
            api.set(
                "log",
                Func::from(move |message: String| {
                    record(&log_events, HostEvent::Log(message));
                }),
            )?;
            if permissions.show_osd {
                let osd_events = events.clone();
                api.set(
                    "showOSD",
                    Func::from(move |message: String| {
                        record(&osd_events, HostEvent::ShowOsd(message));
                    }),
                )?;
            }
            if permissions.show_alert {
                let alert_events = events.clone();
                api.set(
                    "showAlert",
                    Func::from(move |message: String| {
                        record(&alert_events, HostEvent::ShowAlert(message));
                    }),
                )?;
            }
            globals.set("iina", api)?;

            let console = Object::new(ctx.clone())?;
            for method in ["log", "info", "warn", "error"] {
                let log_events = events.clone();
                console.set(
                    method,
                    Func::from(move |message: String| {
                        record(&log_events, HostEvent::Log(message));
                    }),
                )?;
            }
            globals.set("console", console)?;

            // Evaluate as a script, discarding its completion value; normal plugin
            // entries may end in expressions that are not part of their API.
            ctx.eval::<Value, _>(source).map(|_| ())
        })
        .map_err(|error| format!("Plugin {plugin_identifier} failed to initialize: {error}"))?;

    let events = events
        .lock()
        .map_err(|_| "Plugin event queue was poisoned".to_owned())?
        .clone();
    Ok(events)
}

#[derive(Default)]
struct AvailablePermissions {
    show_osd: bool,
    show_alert: bool,
}

fn reviewed_permissions(
    requested: &[String],
    granted: &[String],
) -> Result<AvailablePermissions, String> {
    let mut available = AvailablePermissions::default();
    for permission in requested {
        if !granted.iter().any(|grant| grant == permission) {
            return Err(format!(
                "Plugin permission {permission} has not been granted"
            ));
        }
        match permission.as_str() {
            "show-osd" => available.show_osd = true,
            "show-alert" => available.show_alert = true,
            "network-request" | "file-system" | "video-overlay" => {
                return Err(format!(
                    "Plugin permission {permission} is not available yet"
                ));
            }
            _ => return Err(format!("Unknown plugin permission {permission}")),
        }
    }
    Ok(available)
}

fn record(events: &Mutex<Vec<HostEvent>>, event: HostEvent) {
    let Ok(mut events) = events.lock() else {
        return;
    };
    if events.len() >= MAX_HOST_EVENTS {
        return;
    }
    let message = match &event {
        HostEvent::Log(message) | HostEvent::ShowOsd(message) | HostEvent::ShowAlert(message) => {
            message
        }
    };
    let bounded = message.chars().take(MAX_MESSAGE_CHARS).collect::<String>();
    let event = match event {
        HostEvent::Log(_) => HostEvent::Log(bounded),
        HostEvent::ShowOsd(_) => HostEvent::ShowOsd(bounded),
        HostEvent::ShowAlert(_) => HostEvent::ShowAlert(bounded),
    };
    events.push(event);
}

#[cfg(test)]
mod tests {
    use super::*;
    use riina_core::plugin::Author;
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    fn temporary_directory() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "riina-plugin-runtime-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn manifest(entry: PathBuf) -> Manifest {
        Manifest {
            name: "Example plugin".into(),
            identifier: "org.example.player-tools".into(),
            version: "1.0.0".into(),
            author: Author {
                name: "Example".into(),
                email: None,
                url: None,
            },
            description: None,
            entry,
            global_entry: None,
            permissions: Vec::new(),
            allowed_domains: Vec::new(),
            github_repo: None,
            github_version: None,
        }
    }

    #[test]
    fn exposes_only_the_host_apis_requested_and_granted_by_the_plugin() {
        let requested = vec!["show-osd".to_owned(), "show-alert".to_owned()];
        let events = execute_entry(
            r#"iina.log("ready"); iina.showOSD("Loaded"); iina.showAlert("Notice"); console.warn("done");"#,
            "org.example.player-tools",
            &requested,
            &requested,
        )
        .unwrap();

        assert_eq!(
            events,
            [
                HostEvent::Log("ready".into()),
                HostEvent::ShowOsd("Loaded".into()),
                HostEvent::ShowAlert("Notice".into()),
                HostEvent::Log("done".into()),
            ]
        );
        assert!(
            execute_entry(
                "iina.showOSD('not granted')",
                "org.example.player-tools",
                &[],
                &[],
            )
            .is_err()
        );
    }

    #[test]
    fn refuses_unimplemented_or_ungranted_capabilities_before_execution() {
        for permission in ["network-request", "file-system", "video-overlay"] {
            let requested = vec![permission.to_owned()];
            assert!(
                execute_entry(
                    "throw new Error('must not run')",
                    "org.example.player-tools",
                    &requested,
                    &requested,
                )
                .is_err()
            );
        }
        let requested = vec!["show-osd".to_owned()];
        assert!(
            execute_entry(
                "throw new Error('must not run')",
                "org.example.player-tools",
                &requested,
                &[],
            )
            .is_err()
        );
    }

    #[test]
    fn interrupts_unbounded_entry_code() {
        let started = Instant::now();
        let result = execute_entry("while (true) {}", "org.example.player-tools", &[], &[]);
        assert!(result.is_err());
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn bounds_source_and_host_output() {
        let too_large = " ".repeat(MAX_ENTRY_BYTES + 1);
        assert!(execute_entry(&too_large, "org.example.player-tools", &[], &[]).is_err());

        let many_logs = (0..MAX_HOST_EVENTS + 10)
            .map(|index| format!("iina.log('{index}')"))
            .collect::<Vec<_>>()
            .join(";");
        let events = execute_entry(&many_logs, "org.example.player-tools", &[], &[]).unwrap();
        assert_eq!(events.len(), MAX_HOST_EVENTS);
    }

    #[test]
    fn package_entry_must_resolve_inside_its_installed_root() {
        let root = temporary_directory();
        std::fs::write(root.join("entry.js"), "iina.log('inside')").unwrap();
        let events = execute_package_entry(&root, &manifest("entry.js".into()), &[]).unwrap();
        assert_eq!(events, [HostEvent::Log("inside".into())]);

        let sibling_entry = root.with_extension("js");
        std::fs::write(&sibling_entry, "iina.log('outside')").unwrap();
        let traversal_entry = PathBuf::from("..").join(
            sibling_entry
                .file_name()
                .expect("temporary file has a name"),
        );
        assert!(execute_package_entry(&root, &manifest(traversal_entry), &[]).is_err());

        let _ = std::fs::remove_file(sibling_entry);
        let _ = std::fs::remove_dir_all(root);
    }
}
