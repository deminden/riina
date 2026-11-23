use super::*;
use crate::plugin_runtime::{HostEvent, PluginRuntimeSession, UiEffect};
use std::sync::atomic::AtomicBool;

#[test]
fn utility_capabilities_require_their_own_review_and_keep_private_paths_confined() {
    let perms = ["file-system".to_owned()];
    let source = r#"
        const u = iina.utils;
        const result = [
            u.exec("unused", []) === null,
            u.open("https://example.invalid") === false,
            u.resolvePath("@data/probe.txt").endsWith("probe.txt"),
            u.resolvePath("@data/../escape") === null,
            u.keychainRead("", "not-a-real-credential") === false,
            Array.isArray(u.preferredLocalizations())
        ];
        iina.log(JSON.stringify(result));
    "#;
    let session =
        PluginRuntimeSession::start_entry(source, "org.test.utils.denied", &perms, &perms).unwrap();
    assert_eq!(
        session.drain_events().unwrap(),
        [HostEvent::Ui(UiEffect::Log(
            "[true,true,true,true,true,true]".into()
        ))]
    );
    let mut requested = perms.to_vec();
    requested.push("process-execution".into());
    assert!(
        PluginRuntimeSession::start_entry("", "org.test.utils.denied", &requested, &perms).is_err()
    );
    let open_only = ["open-external".to_owned()];
    let session = PluginRuntimeSession::start_entry(
        r#"
        iina.file.write("@data/private.txt", "fixture");
        iina.log(String(iina.utils.open("@data/private.txt")));
        "#,
        "org.test.utils.open-only",
        &open_only,
        &open_only,
    )
    .unwrap();
    assert_eq!(
        session.drain_events().unwrap(),
        [HostEvent::Ui(UiEffect::Log("false".into()))]
    );
}

#[test]
fn credential_targets_separate_plugins_services_and_accounts() {
    let first = credential_target("org.test.one", "service", "account").unwrap();
    for (identity, service, account) in [
        ("org.test.two", "service", "account"),
        ("org.test.one", "other", "account"),
        ("org.test.one", "service", "other"),
    ] {
        assert_ne!(
            first,
            credential_target(identity, service, account).unwrap()
        );
    }
    assert!(credential_target("org.test.one", "", "account").is_none());
    assert!(credential_target("org.test.one", "unsafe\0service", "account").is_none());
}

#[test]
fn chooser_delivers_selection_and_cancellation_to_the_requesting_entry() {
    let root = riina_test_support::TempDir::new("utility-file-picker").unwrap();
    let manifest = super::super::tests::manifest("main.js".into());
    std::fs::write(
        root.path().join(&manifest.entry),
        r#"
        (async () => {
            const first = iina.utils.chooseFile("Choose a subtitle", { allowedFileTypes: ["srt"] });
            try {
                await iina.utils.chooseFile("Busy");
            } catch(error) {
                iina.log(String(error.code));
            }
            iina.log(await first);
            const cancelled = await iina.utils.chooseFile("Choose a folder", { chooseDir: true });
            iina.log(String(cancelled == null));
        })().catch(error => iina.log(String(error)));
        "#,
    )
    .unwrap();
    let dialogs = FileDialogs::default();
    let (requests, pending) = std::sync::mpsc::sync_channel::<FileRequest>(1);
    dialogs.install(move |request| requests.try_send(request).map_err(|_| "Picker busy".into()));
    let session = PluginRuntimeSession::start_package_entry_with_player_context(
        root.path(),
        &manifest,
        &[],
        Default::default(),
        None,
        Default::default(),
        dialogs,
    )
    .unwrap();
    let selected = pending.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(selected.title, "Choose a subtitle");
    assert_eq!(selected.extensions, ["srt"]);
    assert!(!selected.folder);
    selected.finish(Ok(Some("selected.srt".into())));
    let cancelled = pending.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(cancelled.folder);
    cancelled.finish(Ok(None));
    session.flush_events(Duration::from_secs(2)).unwrap();
    assert_eq!(
        session.drain_events().unwrap(),
        ["-2", "selected.srt", "true"].map(|line| HostEvent::Ui(UiEffect::Log(line.into())))
    );
}

#[test]
fn reviewed_helper_exec_streams_both_pipes_reports_exit_and_rejects_missing_binary() {
    let (executable, arguments) = Fixture::Output.command();
    let name = serde_json::to_string(executable.to_str().unwrap()).unwrap();
    let arguments = serde_json::to_string(&arguments).unwrap();
    let source = format!(
        r#"
        (async () => {{
            const api = iina.utils;
            let out = "", err = "";
            if (!api.fileInPath({name})) throw new Error("current binary missing");
            const result = await api.exec({name}, {arguments},
                "@data/", chunk => out += chunk, chunk => err += chunk);
            iina.log(JSON.stringify({{
                status: result.status,
                both: out.includes("helper café") && err.includes("helper getrennt"),
                match: out === result.stdout && err === result.stderr
            }}));
            try {{ await api.exec("@tmp/riina-no-such-helper", []); }} catch(error) {{
                iina.log(String(error));
            }}
        }})().catch(error => iina.log(String(error)));
    "#
    );
    let permissions = ["file-system".into(), "process-execution".into()];
    let session = PluginRuntimeSession::start_entry(
        &source,
        "org.test.utils.exec",
        &permissions,
        &permissions,
    )
    .unwrap();
    let mut observed = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(8);
    while observed.len() < 2 && Instant::now() < deadline {
        session.flush_events(Duration::from_millis(400)).unwrap();
        observed.extend(session.drain_events().unwrap());
        if observed.len() < 2 {
            std::thread::sleep(Duration::from_millis(12));
        }
    }
    assert_eq!(
        observed,
        [
            HostEvent::Ui(UiEffect::Log(
                r#"{"status":7,"both":true,"match":true}"#.into()
            )),
            HostEvent::Ui(UiEffect::Log("-1".into()))
        ]
    );
}

#[test]
fn helper_supervisor_reaps_cancelled_and_oversized_children() {
    let directory = riina_test_support::TempDir::new("helper-supervisor").unwrap();
    let invocation = |fixture: Fixture| {
        let (executable, args) = fixture.command();
        Invocation {
            executable,
            args,
            directory: directory.path().to_path_buf(),
        }
    };
    let started = AtomicBool::new(false);
    let result = invocation(Fixture::Wait).run(
        Instant::now() + Duration::from_secs(8),
        || started.load(Ordering::Acquire),
        |_, text| {
            if text.contains("helper running") {
                started.store(true, Ordering::Release);
            }
            true
        },
    );
    assert!(result.is_err_and(|error| error.kind() == std::io::ErrorKind::Interrupted));
    let result = invocation(Fixture::Oversized).run(
        Instant::now() + Duration::from_secs(8),
        || false,
        |_, _| true,
    );
    assert!(result.is_err_and(|error| error.to_string().contains("exceeded 1 MiB")));
}

enum Fixture {
    Output,
    Wait,
    Oversized,
}

impl Fixture {
    #[cfg(unix)]
    fn command(self) -> (std::path::PathBuf, Vec<String>) {
        // The application test binary may depend on a local SDK search path.
        // These host fixtures run with the same clean environment as plugins.
        let script = match self {
            Self::Output => "printf 'helper café\n'; printf 'helper getrennt\n' >&2; exit 7",
            Self::Wait => "printf 'helper running\n'; sleep 30; printf 'done\n'",
            Self::Oversized => "head -c 1310720 /dev/zero",
        };
        ("/bin/sh".into(), vec!["-c".into(), script.into()])
    }

    #[cfg(windows)]
    fn command(self) -> (std::path::PathBuf, Vec<String>) {
        let test = match self {
            Self::Output => "helper_output_fixture",
            Self::Wait => "helper_wait_fixture",
            Self::Oversized => "helper_oversized_fixture",
        };
        let test = format!("plugins::plugin_runtime::utilities::tests::{test}");
        (
            std::env::current_exe().unwrap(),
            vec![
                "--ignored".into(),
                "--exact".into(),
                test,
                "--nocapture".into(),
                "--skip".into(),
                FIXTURE_MARKER.into(),
            ],
        )
    }
}

#[cfg(windows)]
const FIXTURE_MARKER: &str = "riina-utility-subprocess";

#[cfg(windows)]
#[test]
#[ignore = "launched as a child by the utility integration test"]
fn helper_output_fixture() {
    use std::io::Write;
    if !in_subprocess() {
        return;
    }
    println!("helper café");
    eprintln!("helper getrennt");
    std::io::stdout().flush().unwrap();
    std::io::stderr().flush().unwrap();
    std::process::exit(7);
}

#[cfg(windows)]
#[test]
#[ignore = "launched as a child by the utility cancellation test"]
fn helper_wait_fixture() {
    use std::io::Write;
    if !in_subprocess() {
        return;
    }
    println!("helper running");
    std::io::stdout().flush().unwrap();
    std::thread::sleep(Duration::from_secs(30));
}

#[cfg(windows)]
#[test]
#[ignore = "launched as a child by the utility output limit test"]
fn helper_oversized_fixture() {
    use std::io::Write;
    if !in_subprocess() {
        return;
    }
    let mut output = std::io::stdout().lock();
    for _ in 0..160 {
        output.write_all(&[b'x'; 8192]).unwrap();
    }
}

#[cfg(windows)]
fn in_subprocess() -> bool {
    std::env::args().any(|argument| argument == FIXTURE_MARKER)
}
