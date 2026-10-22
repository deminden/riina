use super::core_api::plugin_media_reference;
use super::*;
use crate::plugin_http::perform_http_request;
use riina_core::control_preferences::SeekType;
use riina_core::plugin::Author;
use std::collections::BTreeMap;
use std::{io::Read, time::Instant};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

pub(super) fn temporary_directory() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "riina-plugin-runtime-{}-{}",
        std::process::id(),
        NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&path).unwrap();
    path
}

pub(super) fn manifest(entry: PathBuf) -> Manifest {
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
        sidebar_tab_name: None,
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
fn refuses_ungranted_or_unsupported_capabilities_before_execution() {
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
    assert!(
        execute_entry(
            "iina.overlay.loadFile('overlay.html')",
            "org.example.player-tools",
            &[],
            &[]
        )
        .is_err()
    );
    assert!(
        execute_entry(
            "iina.overlay.simpleMode()",
            "org.example.player-tools",
            &[],
            &[]
        )
        .is_err()
    );
    assert!(
        execute_entry("iina.overlay.show()", "org.example.player-tools", &[], &[])
            .unwrap()
            .is_empty()
    );
}

#[test]
fn permitted_overlay_calls_become_validated_platform_effects() {
    use riina_core::plugin::OverlayEffect;
    let permissions = vec!["video-overlay".to_owned()];
    let events = execute_entry(
        "iina.overlay.show(); iina.overlay.loadFile('ui/overlay.html'); iina.overlay.setClickable(true); iina.overlay.show(); iina.overlay.setOpacity(2); iina.overlay.postMessage('ready', {version: 2});",
        "org.example.player-tools",
        &permissions,
        &permissions,
    )
    .unwrap();

    assert_eq!(
        events,
        [
            HostEvent::Overlay(OverlayEffect::LoadPluginFile("ui/overlay.html".into())),
            HostEvent::Overlay(OverlayEffect::SetClickable(true)),
            HostEvent::Overlay(OverlayEffect::Show),
            HostEvent::Overlay(OverlayEffect::SetOpacity(1.0)),
            HostEvent::Overlay(OverlayEffect::PostMessage {
                name: "ready".into(),
                data: serde_json::json!({"version": 2}),
            }),
        ]
    );
    assert!(
        execute_entry(
            "iina.overlay.loadFile('../outside.html')",
            "org.example.player-tools",
            &permissions,
            &permissions,
        )
        .is_err()
    );
}

#[test]
fn plugin_file_api_scopes_private_paths_and_checks_external_access_grants() {
    let identifier = format!("org.example.files-{}", std::process::id());
    let data_root = std::env::temp_dir()
        .join("riina-plugin-storage")
        .join(".data")
        .join(&identifier);
    let _ = std::fs::remove_dir_all(&data_root);
    let requested = vec!["file-system".to_owned()];
    let source = r#"
        iina.file.write('@data/state.txt', 'saved');
        iina.file.move('@data/state.txt', '@data/moved.txt');
        if (iina.file.exists('@data/state.txt')) throw new Error('source remains after move');
        if (!iina.file.exists('@data/moved.txt')) throw new Error('missing moved file');
        if (iina.file.read('@data/moved.txt') !== 'saved') throw new Error('read mismatch');
        if (!iina.file.list('@data/').some(entry => entry.filename === 'moved.txt')) {
            throw new Error('listing mismatch');
        }
        iina.log(iina.file.read('@data/moved.txt'));
        iina.file.delete('@data/moved.txt');
    "#;
    let events = execute_entry(source, &identifier, &requested, &requested).unwrap();
    assert_eq!(events, [HostEvent::Log("saved".into())]);
    let _ = std::fs::remove_dir_all(data_root);

    assert!(
        execute_entry(
            "iina.file.read('C:/Windows/win.ini')",
            &identifier,
            &[],
            &[],
        )
        .is_err()
    );
    assert!(
        execute_entry(
            "iina.file.showInFinder('C:/Windows/win.ini')",
            &identifier,
            &[],
            &[],
        )
        .is_err()
    );
    execute_entry(
        r#"try {
            iina.file.trash('@current/movie.mkv');
            throw new Error('ungranted trash succeeded');
        } catch (error) {
            if (!String(error).includes('file-system permission')) throw error;
        }"#,
        &identifier,
        &[],
        &[],
    )
    .unwrap();
}

#[test]
fn plugin_file_handles_expose_binary_seek_read_write_and_close() {
    let identifier = format!("org.example.file-handles-{}", std::process::id());
    let data_root = std::env::temp_dir()
        .join("riina-plugin-storage")
        .join(".data")
        .join(&identifier);
    let _ = std::fs::remove_dir_all(&data_root);
    let requested = vec!["file-system".to_owned()];
    let source = r#"
        iina.file.write('@data/binary.dat', 'abcd');
        const writer = iina.file.handle('@data/binary.dat', 'write');
        writer.write(new Uint8Array([0, 255]));
        if (writer.offset() !== 2) throw new Error('write offset mismatch');
        writer.close();
        const reader = iina.file.handle('@data/binary.dat', 'read');
        const prefix = reader.read(2);
        if (prefix[0] !== 0 || prefix[1] !== 255) throw new Error('binary read mismatch');
        if (reader.offset() !== 2) throw new Error('read offset mismatch');
        reader.seekToEnd();
        if (reader.offset() !== 4) throw new Error('seek-to-end mismatch');
        reader.seekTo(0);
        const content = reader.readToEnd();
        if (content.length !== 4 || content[2] !== 99 || content[3] !== 100) {
            throw new Error('read-to-end mismatch');
        }
        reader.close();
    "#;
    execute_entry(source, &identifier, &requested, &requested).unwrap();
    assert_eq!(
        std::fs::read(data_root.join("binary.dat")).unwrap(),
        [0, 255, b'c', b'd']
    );
    let _ = std::fs::remove_dir_all(data_root);
}

#[test]
fn core_playback_methods_emit_typed_player_commands() {
    let events = execute_entry(
        "iina.core.pause(); iina.core.resume(); iina.core.stop(); iina.core.seek(-4.5, true); iina.core.seekTo(12); iina.core.setSpeed(1.25); iina.core.playChapter(2);",
        "org.example.core-api",
        &[],
        &[],
    )
    .unwrap();
    assert_eq!(
        events,
        [
            HostEvent::CoreCommand(Command::SetPaused(true)),
            HostEvent::CoreCommand(Command::SetPaused(false)),
            HostEvent::CoreCommand(Command::Stop),
            HostEvent::CoreCommand(Command::SeekWithType {
                seconds: -4.5,
                seek_type: SeekType::Exact,
            }),
            HostEvent::CoreCommand(Command::Seek {
                seconds: 12.0,
                absolute: true,
            }),
            HostEvent::CoreCommand(Command::Speed(1.25)),
            HostEvent::CoreCommand(Command::SelectChapter(2)),
        ]
    );
    assert!(
        execute_entry(
            "iina.core.seek(NaN, false)",
            "org.example.core-api",
            &[],
            &[]
        )
        .is_err()
    );
    assert!(execute_entry("iina.core.setSpeed(0)", "org.example.core-api", &[], &[]).is_err());
}

#[test]
fn core_ui_visibility_emits_the_iina_disable_ui_flag() {
    let events = execute_entry(
        "iina.core.setUIVisibility(true); iina.core.setUIVisibility(false);",
        "org.example.core-ui-visibility",
        &[],
        &[],
    )
    .unwrap();
    assert_eq!(
        events,
        [
            HostEvent::SetUiVisibility(true),
            HostEvent::SetUiVisibility(false),
        ]
    );
}

#[test]
fn core_window_proxy_exposes_window_state_and_typed_mutations() {
    let events = execute_entry(
        r#"
            iina.log(String(iina.core.window.loaded));
            iina.log(String(iina.core.window.frame));
            iina.core.window.frame = {x: 12, y: 34, width: 640, height: 360};
            iina.core.window.fullscreen = true;
            iina.core.window.pip = false;
            iina.core.window.ontop = true;
            iina.core.window.sidebar = 'playlist';
            iina.core.window.sidebar = null;
            iina.core.window.miniaturized = false;
        "#,
        "org.example.core-window",
        &[],
        &[],
    )
    .unwrap();
    assert_eq!(
        events,
        [
            HostEvent::Log("false".into()),
            HostEvent::Log("null".into()),
            HostEvent::WindowAction(WindowAction::SetFrame {
                x: 12.0,
                y: 34.0,
                width: 640.0,
                height: 360.0,
            }),
            HostEvent::WindowAction(WindowAction::SetFullscreen(true)),
            HostEvent::WindowAction(WindowAction::SetPictureInPicture(false)),
            HostEvent::WindowAction(WindowAction::SetOnTop(true)),
            HostEvent::WindowAction(WindowAction::SetSidebar(Some("playlist".into()))),
            HostEvent::WindowAction(WindowAction::SetSidebar(None)),
            HostEvent::WindowAction(WindowAction::SetMiniaturized(false)),
        ]
    );
    assert!(
        execute_entry(
            "iina.core.window.frame = {x: 0, y: 0, width: Infinity, height: 10};",
            "org.example.core-window-invalid",
            &[],
            &[],
        )
        .is_err()
    );
    assert!(
        execute_entry(
            "iina.core.window.fullscreen = 'yes';",
            "org.example.core-window-invalid-type",
            &[],
            &[],
        )
        .is_err()
    );
}

#[test]
fn core_track_apis_emit_typed_commands() {
    let events = execute_entry(
        "iina.core.audio.id = 4; iina.core.audio.id = null; iina.core.audio.delay = 0.25; iina.core.audio.volume = 125; iina.core.audio.muted = true; iina.core.subtitle.id = 5; iina.core.subtitle.secondID = 6; iina.core.subtitle.secondID = null; iina.core.subtitle.delay = -0.5; iina.core.video.id = 7;",
        "org.example.core-track-api",
        &[],
        &[],
    )
    .unwrap();
    assert_eq!(
        events,
        [
            HostEvent::CoreCommand(Command::SelectTrack {
                kind: riina_core::TrackKind::Audio,
                id: Some(4),
            }),
            HostEvent::CoreCommand(Command::AudioDelay(0.25)),
            HostEvent::CoreCommand(Command::Volume(125.0)),
            HostEvent::CoreCommand(Command::SetMuted(true)),
            HostEvent::CoreCommand(Command::SelectTrack {
                kind: riina_core::TrackKind::Sub,
                id: Some(5),
            }),
            HostEvent::CoreCommand(Command::SelectSecondarySubtitle(Some(6))),
            HostEvent::CoreCommand(Command::SubtitleDelay(-0.5)),
            HostEvent::CoreCommand(Command::SelectTrack {
                kind: riina_core::TrackKind::Video,
                id: Some(7),
            }),
        ]
    );
    assert!(
        execute_entry(
            "iina.core.audio.volume = 201;",
            "org.example.core-track-api",
            &[],
            &[],
        )
        .is_err()
    );
    assert!(
        execute_entry(
            "iina.core.audio.loadTrack('https://media.example/commentary.m4a');",
            "org.example.core-track-api",
            &[],
            &[],
        )
        .is_err()
    );
}

#[test]
fn core_status_chapter_recent_document_and_version_reads_follow_playback_context() {
    let identifier = "org.example.core-snapshot";
    let files = PluginFiles::temporary(identifier);
    files.set_player_context(
        PluginPlayerFileContext::new(None, BTreeMap::new())
            .with_core_snapshot(
                BTreeMap::from([
                    ("paused".into(), serde_json::json!(true)),
                    ("position".into(), serde_json::json!(17.25)),
                    ("title".into(), serde_json::json!("Sample")),
                ]),
                vec![serde_json::json!({"title":"Opening", "start":0.0})],
            )
            .with_core_version(serde_json::json!({
                "iina": "0.3.1",
                "build": "1234",
                "mpv": "mpv 0.41.0-git"
            }))
            .with_recent_documents(vec![serde_json::json!({
                "name": "sample.mp4",
                "url": "file:///Media/sample.mp4"
            })])
            .with_playback_history(vec![serde_json::json!({
                "name": "sample.mp4",
                "url": "file:///Media/sample.mp4",
                "date": 1_750_000_000_000_u64,
                "progress": 12.5,
                "duration": 80.0
            })]),
    );
    let script = r#"
        if (iina.core.status.paused !== true) throw new Error('pause status mismatch');
        if (iina.core.status.position !== 17.25) throw new Error('position mismatch');
        if (iina.core.status.title !== 'Sample') throw new Error('title mismatch');
        if (iina.core.status.unknown !== undefined) throw new Error('unknown status should be undefined');
        const chapters = iina.core.getChapters();
        if (chapters.length !== 1 || chapters[0].title !== 'Opening' || chapters[0].start !== 0) {
            throw new Error('chapter snapshot mismatch');
        }
        const recent = iina.core.getRecentDocuments();
        if (recent.length !== 1 || recent[0].name !== 'sample.mp4' || recent[0].url !== 'file:///Media/sample.mp4') {
            throw new Error('recent-document snapshot mismatch');
        }
        const history = iina.core.getHistory();
        if (history.length !== 1 || history[0].name !== 'sample.mp4' ||
            history[0].url !== 'file:///Media/sample.mp4' ||
            !(history[0].date instanceof Date) || history[0].date.getTime() !== 1750000000000 ||
            history[0].progress !== 12.5 || history[0].duration !== 80) {
            throw new Error('playback-history snapshot mismatch');
        }
        const version = iina.core.getVersion();
        if (version.iina !== '0.3.1' || version.build !== '1234' || version.mpv !== 'mpv 0.41.0-git') {
            throw new Error('version snapshot mismatch');
        }
    "#;
    let session = PluginRuntimeSession::start_entry_with_context(
        script,
        identifier,
        &[],
        &[],
        GlobalApiMode::None,
        None,
        PluginStorageContext {
            file_system: files,
            allowed_domains: Vec::new(),
        },
    )
    .unwrap();
    assert!(session.drain_events().unwrap().is_empty());
}

#[test]
fn core_open_requires_the_matching_network_or_filesystem_grant() {
    let identifier = format!("org.example.core-open-{}", std::process::id());
    let files = PluginFiles::temporary(&identifier);
    files
        .write_text("@data/clip.mkv", "fixture", false)
        .unwrap();
    let path = files
        .existing_path("@data/clip.mkv", false)
        .unwrap()
        .to_string_lossy()
        .into_owned();
    assert_eq!(
        execute_entry("iina.core.open('@data/clip.mkv')", &identifier, &[], &[],).unwrap(),
        [HostEvent::CoreCommand(Command::Load(path))]
    );
    let _ = std::fs::remove_dir_all(
        std::env::temp_dir()
            .join("riina-plugin-storage")
            .join(".data")
            .join(&identifier),
    );
    let files = PluginFiles::temporary("org.example.core-open-policy");
    assert!(
        plugin_media_reference(
            "https://media.example/video.mp4",
            &files,
            false,
            false,
            &["media.example".into()],
        )
        .is_err()
    );
    assert_eq!(
        plugin_media_reference(
            "https://media.example/video.mp4",
            &files,
            false,
            true,
            &["media.example".into()],
        )
        .unwrap(),
        "https://media.example/video.mp4"
    );
    assert_eq!(
        plugin_media_reference(
            "smb://media.example/share/video.mkv",
            &files,
            false,
            true,
            &["media.example".into()],
        )
        .unwrap(),
        "smb://media.example/share/video.mkv"
    );
    assert!(
        plugin_media_reference(
            "rtsp://media.example/live",
            &files,
            false,
            false,
            &["media.example".into()],
        )
        .is_err()
    );
    assert_eq!(
        plugin_media_reference(
            "rtsp://media.example/live",
            &files,
            false,
            true,
            &["media.example".into()],
        )
        .unwrap(),
        "rtsp://media.example/live"
    );
    assert!(
        plugin_media_reference(
            "rtmp://media.example/live/channel",
            &files,
            true,
            false,
            &["media.example".into()],
        )
        .is_err()
    );
    assert_eq!(
        plugin_media_reference(
            "rtmp://media.example/live/channel",
            &files,
            false,
            true,
            &["media.example".into()],
        )
        .unwrap(),
        "rtmp://media.example/live/channel"
    );
    assert_eq!(
        plugin_media_reference(
            "ytdl://https://media.example/watch?id=42",
            &files,
            false,
            true,
            &["media.example".into()],
        )
        .unwrap(),
        "ytdl://https://media.example/watch?id=42"
    );
    assert!(
        plugin_media_reference(
            "ytdl://https://media.example/watch?id=42",
            &files,
            false,
            true,
            &["elsewhere.example".into()],
        )
        .is_err()
    );
    assert!(
        plugin_media_reference(
            "https://media.example/video.mp4",
            &files,
            false,
            true,
            &["elsewhere.example".into()],
        )
        .is_err()
    );
    assert!(plugin_media_reference("C:/private/video.mp4", &files, false, false, &[],).is_err());
}

#[test]
fn permission_gated_http_transport_returns_json_from_an_allowed_host() {
    use std::{io::Write, net::TcpListener};

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut request = [0; 4096];
        let _ = stream.read(&mut request).unwrap();
        let body = r#"{"ready":true}"#;
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
        .unwrap();
    });
    let result = perform_http_request(
        "GET",
        &format!("http://{address}/health"),
        &serde_json::json!({}),
        &["127.0.0.1".into()],
    )
    .unwrap();
    server.join().unwrap();
    assert_eq!(result["statusCode"], 200);
    assert_eq!(result["data"]["ready"], true);
    assert_eq!(result["text"], r#"{"ready":true}"#);
}

#[test]
fn simple_mode_style_and_content_are_forwarded_after_activation() {
    use riina_core::plugin::OverlayEffect;
    let permissions = vec!["video-overlay".to_owned()];
    let events = execute_entry(
        "iina.overlay.setStyle('ignored before mode'); iina.overlay.simpleMode(); iina.overlay.setStyle('body { color: white; }'); iina.overlay.setContent('<b>Ready</b>');",
        "org.example.player-tools",
        &permissions,
        &permissions,
    )
    .unwrap();

    assert_eq!(
        events,
        [
            HostEvent::Overlay(OverlayEffect::EnterSimpleMode),
            HostEvent::Overlay(OverlayEffect::SetStyle("body { color: white; }".into())),
            HostEvent::Overlay(OverlayEffect::SetContent("<b>Ready</b>".into())),
        ]
    );
}

#[test]
fn overlay_listener_registration_keeps_the_source_api_shape() {
    use riina_core::plugin::OverlayEffect;
    let permissions = vec!["video-overlay".to_owned()];
    let events = execute_entry(
        "iina.overlay.simpleMode(); iina.overlay.onMessage('reply', () => {});",
        "org.example.player-tools",
        &permissions,
        &permissions,
    )
    .unwrap();
    assert_eq!(
        events,
        [
            HostEvent::Overlay(OverlayEffect::EnterSimpleMode),
            HostEvent::Overlay(OverlayEffect::ListenMessage("reply".into())),
        ]
    );
}

#[test]
fn persistent_session_delivers_overlay_messages_to_registered_callbacks() {
    let permissions = vec!["video-overlay".to_owned()];
    let session = PluginRuntimeSession::start_entry(
        "iina.overlay.simpleMode(); iina.overlay.onMessage('reply', value => iina.log(value.answer));",
        "org.example.player-tools",
        &permissions,
        &permissions,
    )
    .unwrap();
    assert!(session.drain_events().unwrap().iter().any(|event| {
        matches!(event, HostEvent::Overlay(OverlayEffect::ListenMessage(name)) if name == "reply")
    }));
    session
        .deliver_overlay_message("unregistered".into(), serde_json::json!(null))
        .unwrap();
    session
        .deliver_overlay_message("reply".into(), serde_json::json!({"answer": "received"}))
        .unwrap();

    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        let events = session.drain_events().unwrap();
        if events
            .iter()
            .any(|event| matches!(event, HostEvent::Log(message) if message == "received"))
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "plugin callback was not delivered"
        );
        std::thread::yield_now();
    }
}

#[test]
fn sidebar_api_emits_confined_document_and_visibility_effects() {
    let events = execute_entry(
        "iina.sidebar.show(); iina.sidebar.loadFile('sidebar/index.html'); iina.sidebar.show(); iina.sidebar.hide();",
        "org.example.sidebar",
        &[],
        &[],
    )
    .unwrap();
    assert_eq!(
        events,
        [
            HostEvent::Sidebar(PluginSidebarEffect::LoadFile("sidebar/index.html".into())),
            HostEvent::Sidebar(PluginSidebarEffect::Show),
            HostEvent::Sidebar(PluginSidebarEffect::Hide),
        ]
    );
    assert!(
        execute_entry(
            "iina.sidebar.loadFile('../outside.html')",
            "org.example.sidebar",
            &[],
            &[],
        )
        .is_err()
    );
}

#[test]
fn persistent_sidebar_session_delivers_only_registered_messages() {
    let session = PluginRuntimeSession::start_entry(
        "iina.sidebar.loadFile('sidebar/index.html'); iina.sidebar.onMessage('ready', value => iina.log(value.answer));",
        "org.example.sidebar",
        &[],
        &[],
    )
    .unwrap();
    assert!(matches!(
        session.drain_events().unwrap().as_slice(),
        [
            HostEvent::Sidebar(PluginSidebarEffect::LoadFile(_)),
            HostEvent::Sidebar(PluginSidebarEffect::ListenMessage(name))
        ] if name == "ready"
    ));
    session
        .deliver_sidebar_message("ignored".into(), serde_json::Value::Null)
        .unwrap();
    session
        .deliver_sidebar_message("ready".into(), serde_json::json!({"answer": "received"}))
        .unwrap();

    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        let events = session.drain_events().unwrap();
        if events
            .iter()
            .any(|event| matches!(event, HostEvent::Log(message) if message == "received"))
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "sidebar callback was not delivered"
        );
        std::thread::yield_now();
    }
}

#[test]
fn plugin_menu_snapshot_and_action_callback_reach_the_host() {
    let session = PluginRuntimeSession::start_entry(
        "const entry = iina.menu.item('Toggle', item => iina.log(item.selected ? 'checked' : 'clear'), { selected: true, keyBinding: 'Cmd+T' }); iina.menu.addItem(entry); iina.menu.removeAt(0); iina.menu.addItem(entry);",
        "org.example.player-tools",
        &[],
        &[],
    )
    .unwrap();
    let events = session.drain_events().unwrap();
    assert!(events.iter().any(|event| matches!(event,
        HostEvent::MenuUpdate(items)
            if items.len() == 1
                && items[0].title == "Toggle"
                && items[0].selected
                && items[0].key_binding.as_deref() == Some("Cmd+T")
    )));

    session.deliver_plugin_menu_action("menu-1".into()).unwrap();
    session.flush_events(Duration::from_secs(1)).unwrap();
    assert!(
        session
            .drain_events()
            .unwrap()
            .iter()
            .any(|event| matches!(event, HostEvent::Log(message) if message == "checked"))
    );
}

#[test]
fn persistent_session_delivers_player_events_and_unregisters_listeners() {
    let session = PluginRuntimeSession::start_entry(
        "iina.event.on('iina.file-started', () => iina.log('started')); iina.event.on('iina.file-loaded', url => iina.log(url)); iina.event.on('iina.window-loaded', () => iina.log('window-loaded')); iina.event.on('iina.window-resized', frame => iina.log(String(frame.width))); iina.event.on('iina.window-size-adjusted', frame => iina.log(`adjusted:${frame.width}`)); iina.event.on('iina.pip.changed', value => iina.log(`pip:${value}`)); iina.event.on('iina.music-mode.changed', value => iina.log(`music:${value}`)); iina.event.on('iina.window-main.changed', value => iina.log(`main:${value}`)); iina.event.on('mpv.pause.changed', value => iina.log(String(value))); iina.event.on('mpv.cache-buffering-state.changed', value => iina.log(String(value)));",
        "org.example.player-events",
        &[],
        &[],
    )
    .unwrap();
    assert!(session.is_subscribed_to_player_event("iina.file-started"));
    assert!(session.is_subscribed_to_player_event("iina.file-loaded"));
    assert!(session.is_subscribed_to_player_event("iina.window-loaded"));
    assert!(session.is_subscribed_to_player_event("iina.window-resized"));
    assert!(session.is_subscribed_to_player_event("iina.window-size-adjusted"));
    assert!(session.is_subscribed_to_player_event("iina.pip.changed"));
    assert!(session.is_subscribed_to_player_event("iina.music-mode.changed"));
    assert!(session.is_subscribed_to_player_event("iina.window-main.changed"));
    assert!(session.is_subscribed_to_player_event("mpv.pause.changed"));
    assert!(session.is_subscribed_to_player_event("mpv.cache-buffering-state.changed"));
    assert_eq!(
        session.take_mpv_property_observation_requests(),
        ["cache-buffering-state"]
    );
    assert!(session.take_mpv_property_observation_requests().is_empty());
    session
        .deliver_player_event("iina.file-started".into(), serde_json::Value::Null)
        .unwrap();
    session
        .deliver_player_event(
            "iina.file-loaded".into(),
            serde_json::json!("file:///media/example.mkv"),
        )
        .unwrap();
    session
        .deliver_player_event("iina.window-loaded".into(), serde_json::Value::Null)
        .unwrap();
    session
        .deliver_player_event(
            "iina.window-resized".into(),
            serde_json::json!({"x": 10, "y": 15, "width": 800, "height": 450}),
        )
        .unwrap();
    session
        .deliver_player_event("iina.pip.changed".into(), serde_json::json!(true))
        .unwrap();
    session
        .deliver_player_event("iina.music-mode.changed".into(), serde_json::json!(true))
        .unwrap();
    session
        .deliver_player_event("iina.window-main.changed".into(), serde_json::json!(true))
        .unwrap();
    session
        .deliver_player_event("mpv.pause.changed".into(), serde_json::json!(true))
        .unwrap();

    let deadline = Instant::now() + Duration::from_secs(1);
    let mut seen = Vec::new();
    loop {
        seen.extend(session.drain_events().unwrap());
        let file_loaded = seen.iter().any(|event| {
            matches!(event, HostEvent::Log(message) if message == "file:///media/example.mkv")
        });
        let file_started = seen
            .iter()
            .any(|event| matches!(event, HostEvent::Log(message) if message == "started"));
        let paused = seen
            .iter()
            .any(|event| matches!(event, HostEvent::Log(message) if message == "true"));
        let window_loaded = seen
            .iter()
            .any(|event| matches!(event, HostEvent::Log(message) if message == "window-loaded"));
        let window_resized = seen
            .iter()
            .any(|event| matches!(event, HostEvent::Log(message) if message == "800"));
        let pip_changed = seen
            .iter()
            .any(|event| matches!(event, HostEvent::Log(message) if message == "pip:true"));
        let music_mode_changed = seen
            .iter()
            .any(|event| matches!(event, HostEvent::Log(message) if message == "music:true"));
        let main_window_changed = seen
            .iter()
            .any(|event| matches!(event, HostEvent::Log(message) if message == "main:true"));
        if file_started
            && file_loaded
            && paused
            && window_loaded
            && window_resized
            && pip_changed
            && music_mode_changed
            && main_window_changed
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "player event callback was not delivered: {seen:?}"
        );
        std::thread::yield_now();
    }

    let removed = PluginRuntimeSession::start_entry(
        "const listener = iina.event.on('iina.file-loaded', url => iina.log(url)); iina.event.off('iina.file-loaded', listener);",
        "org.example.player-events-off",
        &[],
        &[],
    )
    .unwrap();
    assert!(!removed.is_subscribed_to_player_event("iina.file-loaded"));
    removed
        .deliver_player_event(
            "iina.file-loaded".into(),
            serde_json::json!("file:///media/ignored.mkv"),
        )
        .unwrap();
    assert!(removed.drain_events().unwrap().is_empty());
}

#[test]
fn player_event_listener_rejects_invalid_names() {
    assert!(
        execute_entry(
            "iina.event.on('mpv..changed', () => {});",
            "org.example.invalid-player-event",
            &[],
            &[],
        )
        .is_err()
    );
}

#[test]
fn overlay_host_output_is_bounded_across_repeated_large_calls() {
    let permissions = vec!["video-overlay".to_owned()];
    let events = execute_entry(
        "iina.overlay.simpleMode(); const payload = 'x'.repeat(1048576); for (let i = 0; i < 8; i++) iina.overlay.setContent(payload);",
        "org.example.player-tools",
        &permissions,
        &permissions,
    )
    .unwrap();
    let output_bytes = events
        .iter()
        .filter_map(|event| match event {
            HostEvent::Overlay(OverlayEffect::SetContent(content)) => Some(content.len()),
            _ => None,
        })
        .sum::<usize>();
    assert!(output_bytes <= MAX_HOST_OUTPUT_BYTES);
    assert!(output_bytes < 8 * 1024 * 1024);
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
