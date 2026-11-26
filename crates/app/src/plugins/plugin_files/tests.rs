use super::*;
use riina_test_support::TempDir;
use std::collections::BTreeMap;

fn temporary_files() -> (PluginFiles, TempDir) {
    let directory = TempDir::new("plugin-files").unwrap();
    let root = directory.path();
    (
        PluginFiles {
            data_root: root.join(".data"),
            temp_root: root.join(".tmp"),
            player_context: Arc::new(Mutex::new(PluginPlayerFileContext::default())),
            _test_directory: None,
        },
        directory,
    )
}

#[test]
fn destination_creation_is_exclusive_outside_private_storage() {
    let (files, directory) = temporary_files();
    let root = directory.path();
    let outside = root.join("outside.bin");
    let mut first = files.open_destination(&outside).unwrap();
    first.write_all(b"original").unwrap();
    assert!(files.open_destination(&outside).is_err());
    drop(first);
    assert_eq!(fs::read(&outside).unwrap(), b"original");
    files
        .write_download("@data/private.bin", b"original", false)
        .unwrap();
    files
        .write_download("@data/private.bin", b"new", false)
        .unwrap();
    assert_eq!(fs::read(root.join(".data/private.bin")).unwrap(), b"new");
    assert!(
        files
            .write_download(outside.to_str().unwrap(), b"replace", true)
            .is_err()
    );
    assert_eq!(fs::read(outside).unwrap(), b"original");
}

#[test]
fn text_reads_enforce_the_size_boundary() {
    let (files, directory) = temporary_files();
    let root = directory.path();
    files
        .write_text("@data/limit.txt", &"a".repeat(MAX_FILE_BYTES), false)
        .unwrap();
    assert_eq!(
        files
            .read_text("@data/limit.txt", "{}", false)
            .unwrap()
            .len(),
        MAX_FILE_BYTES
    );
    OpenOptions::new()
        .append(true)
        .open(root.join(".data/limit.txt"))
        .unwrap()
        .write_all(b"b")
        .unwrap();
    assert!(
        files
            .read_text("@data/limit.txt", "{}", false)
            .unwrap_err()
            .contains("4 MiB")
    );
}

#[test]
fn player_context_reports_file_started_and_loaded_with_the_incoming_url() {
    let previous = PluginPlayerFileContext::default()
        .with_media_generation(4)
        .with_media_phase(riina_core::Phase::Playing)
        .with_core_version(serde_json::json!({"mpv": null}))
        .with_core_snapshot(
            BTreeMap::from([("url".into(), serde_json::json!("file:///media/old.mkv"))]),
            Vec::new(),
        );
    let next = PluginPlayerFileContext::default()
        .with_media_generation(5)
        .with_media_phase(riina_core::Phase::Paused)
        .with_core_version(serde_json::json!({"mpv": "mpv 0.41.0-git"}))
        .with_core_snapshot(
            BTreeMap::from([("url".into(), serde_json::json!("file:///media/new.mkv"))]),
            Vec::new(),
        );

    assert_eq!(
        previous.player_events_to(&next),
        [
            ("iina.mpv-initialized".into(), None),
            ("iina.file-started".into(), None),
            (
                "iina.file-loaded".into(),
                Some(serde_json::json!("file:///media/new.mkv").into()),
            ),
        ]
    );

    let loading = PluginPlayerFileContext::default()
        .with_media_generation(5)
        .with_core_snapshot(
            BTreeMap::from([("url".into(), serde_json::json!("file:///media/new.mkv"))]),
            Vec::new(),
        );
    assert_eq!(
        previous.player_events_to(&loading),
        [("iina.file-started".into(), None)]
    );
    let loaded = loading.clone().with_media_phase(riina_core::Phase::Paused);
    assert_eq!(
        loading.player_events_to(&loaded),
        [(
            "iina.file-loaded".into(),
            Some(serde_json::json!("file:///media/new.mkv").into()),
        )]
    );
}

#[test]
fn player_context_reports_observed_window_geometry_and_focus_changes() {
    let previous = PluginPlayerFileContext::default();
    let initial = BTreeMap::from([
        (
            "frame".into(),
            serde_json::json!({"x": 10, "y": 20, "width": 640, "height": 360}),
        ),
        ("miniaturized".into(), serde_json::Value::Null),
        ("main".into(), serde_json::json!(false)),
        ("window-size-adjusted-event".into(), serde_json::json!(0)),
        ("screens".into(), serde_json::json!([])),
    ]);
    assert!(previous.window_events_to(&initial).is_empty());

    let mut settled = PluginPlayerFileContext::default();
    settled.set_core_window_properties(initial);
    let changed = BTreeMap::from([
        (
            "frame".into(),
            serde_json::json!({"x": 35, "y": 42, "width": 800, "height": 450}),
        ),
        ("miniaturized".into(), serde_json::json!(false)),
        ("main".into(), serde_json::json!(true)),
        ("window-size-adjusted-event".into(), serde_json::json!(1)),
        (
            "screens".into(),
            serde_json::json!([{"frame":{"x":0,"y":0,"width":1920,"height":1080}}]),
        ),
    ]);

    assert_eq!(
        settled.window_events_to(&changed),
        [
            (
                "iina.window-moved".into(),
                Some(serde_json::json!({"x":35,"y":42,"width":800,"height":450}).into()),
            ),
            (
                "iina.window-resized".into(),
                Some(serde_json::json!({"x":35,"y":42,"width":800,"height":450}).into()),
            ),
            (
                "iina.window-size-adjusted".into(),
                Some(serde_json::json!({"x":35,"y":42,"width":800,"height":450}).into()),
            ),
            (
                "iina.window-main.changed".into(),
                Some(serde_json::json!(true).into())
            ),
        ]
    );

    let adjusted_without_frame_change = BTreeMap::from([
        (
            "frame".into(),
            serde_json::json!({"x":10,"y":20,"width":640,"height":360}),
        ),
        ("window-size-adjusted-event".into(), serde_json::json!(1)),
    ]);
    assert_eq!(
        settled.window_events_to(&adjusted_without_frame_change),
        [(
            "iina.window-size-adjusted".into(),
            Some(serde_json::json!({"x":10,"y":20,"width":640,"height":360}).into()),
        )]
    );

    let mut with_screen = PluginPlayerFileContext::default();
    with_screen.set_core_window_properties(changed);
    let next_screen = BTreeMap::from([
        (
            "frame".into(),
            serde_json::json!({"x": 35, "y": 42, "width": 800, "height": 450}),
        ),
        ("miniaturized".into(), serde_json::json!(true)),
        ("main".into(), serde_json::json!(true)),
        (
            "screens".into(),
            serde_json::json!([{"frame":{"x":0,"y":0,"width":2560,"height":1440}}]),
        ),
    ]);
    assert_eq!(
        with_screen.window_events_to(&next_screen),
        [
            ("iina.window-miniaturized".into(), None),
            ("iina.window-screen.changed".into(), None),
        ]
    );
}

#[test]
fn plugin_data_paths_support_bounded_text_and_listing() {
    let (files, _directory) = temporary_files();
    files
        .write_text("@data/state.json", "{\"ready\":true}", false)
        .unwrap();

    assert!(files.exists("@data/state.json", false).unwrap());
    assert_eq!(
        files.read_text("@data/state.json", "{}", false).unwrap(),
        "{\"ready\":true}"
    );
    let entries: serde_json::Value =
        serde_json::from_str(&files.list("@data/", "{}", false).unwrap()).unwrap();
    assert_eq!(entries[0]["filename"], "state.json");
    assert_eq!(entries[0]["path"], "state.json");

    files.delete("@data/state.json").unwrap();
    assert!(!files.exists("@data/state.json", false).unwrap());
}

#[test]
fn private_plugin_paths_reject_traversal_and_require_grant_for_absolute_paths() {
    let (files, directory) = temporary_files();
    let root = directory.path();
    assert!(
        files
            .write_text("@data/../../outside.txt", "denied", false)
            .is_err()
    );
    let external = root.join("external.txt");
    assert!(
        files
            .write_text(external.to_str().unwrap(), "denied", false)
            .is_err()
    );

    files
        .write_text(external.to_str().unwrap(), "granted", true)
        .unwrap();
    assert_eq!(fs::read_to_string(&external).unwrap(), "granted");
}

#[test]
fn private_paths_normalize_dot_segments_without_escaping_the_plugin_root() {
    let (files, directory) = temporary_files();
    let root = directory.path();
    files
        .write_text("@data/nested/../normalized.txt", "inside", false)
        .unwrap();
    assert_eq!(
        files
            .read_text("@data/./normalized.txt", "{}", false)
            .unwrap(),
        "inside"
    );
    assert!(
        files
            .write_text("@data/../outside.txt", "outside", false)
            .is_err()
    );
    assert!(
        files
            .write_text("@data/nested/../../outside.txt", "outside", false)
            .is_err()
    );
    assert!(!root.join("outside.txt").exists());
}

#[test]
fn current_media_and_external_track_aliases_use_player_context() {
    let (files, directory) = temporary_files();
    let root = directory.path();
    let media_dir = root.join("media");
    fs::create_dir_all(&media_dir).unwrap();
    fs::write(media_dir.join("captions.srt"), "subtitle").unwrap();
    let audio = root.join("external-audio.mka");
    fs::write(&audio, "audio").unwrap();
    files.set_player_context(PluginPlayerFileContext::new(
        Some(media_dir.clone()),
        BTreeMap::from([("@audio/7".into(), audio.clone())]),
    ));

    assert!(
        files
            .read_text("@current/captions.srt", "{}", false)
            .is_err()
    );
    assert_eq!(
        files
            .read_text("@current/captions.srt", "{}", true)
            .unwrap(),
        "subtitle"
    );
    assert_eq!(files.read_text("@audio/7", "{}", false).unwrap(), "audio");
    assert!(files.read_text("@sub/88", "{}", false).is_err());
}

#[test]
fn move_scopes_private_paths_and_requires_grant_for_external_paths() {
    let (files, directory) = temporary_files();
    let root = directory.path();
    assert!(files.trash("@data/", false).is_err());
    files
        .write_text("@data/source.txt", "saved", false)
        .unwrap();
    files
        .move_path("@data/source.txt", "@data/destination.txt", false)
        .unwrap();
    assert!(!files.exists("@data/source.txt", false).unwrap());
    assert_eq!(
        files
            .read_text("@data/destination.txt", "{}", false)
            .unwrap(),
        "saved"
    );

    let external_source = root.join("outside.txt");
    let external_destination = root.join("moved.txt");
    fs::write(&external_source, "external").unwrap();
    assert!(
        files
            .trash(external_source.to_str().unwrap(), false)
            .is_err()
    );
    assert!(external_source.exists());
    assert!(
        files
            .move_path(
                external_source.to_str().unwrap(),
                external_destination.to_str().unwrap(),
                false,
            )
            .is_err()
    );
    files
        .move_path(
            external_source.to_str().unwrap(),
            external_destination.to_str().unwrap(),
            true,
        )
        .unwrap();
    assert_eq!(
        fs::read_to_string(&external_destination).unwrap(),
        "external"
    );
}

#[test]
fn writes_outside_plugin_storage_do_not_overwrite_existing_files() {
    let (files, directory) = temporary_files();
    let root = directory.path();
    let external = root.join("external.txt");
    fs::write(&external, "keep").unwrap();
    assert!(
        files
            .write_text(external.to_str().unwrap(), "replace", true)
            .is_err()
    );
    assert_eq!(fs::read_to_string(&external).unwrap(), "keep");
}

#[test]
fn binary_handles_seek_read_and_write_with_bounded_operations() {
    let (files, directory) = temporary_files();
    let root = directory.path();
    files.write_text("@data/binary.bin", "abcd", false).unwrap();

    let mut reader = files
        .open_handle("@data/binary.bin", "read", false)
        .unwrap();
    assert_eq!(reader.read(2).unwrap(), b"ab");
    assert_eq!(reader.offset().unwrap(), 2);
    reader.seek_to_end().unwrap();
    assert_eq!(reader.read_to_end().unwrap(), b"");
    assert!(reader.write(b"x").is_err());

    let mut writer = files
        .open_handle("@data/binary.bin", "write", false)
        .unwrap();
    writer.write(&[0, 255]).unwrap();
    writer.seek_to(2).unwrap();
    assert_eq!(writer.offset().unwrap(), 2);
    assert_eq!(
        fs::read(root.join(".data/binary.bin")).unwrap(),
        [0, 255, b'c', b'd']
    );
    assert!(
        files
            .open_handle("@data/binary.bin", "invalid", false)
            .is_err()
    );
    assert!(writer.read(1).is_err());

    let mut oversized_read = files
        .open_handle("@data/binary.bin", "read", false)
        .unwrap();
    assert!(oversized_read.read(MAX_FILE_BYTES + 1).is_err());
}

#[test]
fn reveal_path_requires_an_existing_file_and_filesystem_grant_for_external_paths() {
    let (files, directory) = temporary_files();
    let root = directory.path();
    files
        .write_text("@data/reveal.txt", "visible", false)
        .unwrap();
    assert!(files.existing_path("@data/reveal.txt", false).is_ok());

    let external = root.join("external.txt");
    fs::write(&external, "visible").unwrap();
    assert!(
        files
            .existing_path(external.to_str().unwrap(), false)
            .is_err()
    );
    assert_eq!(
        files
            .existing_path(external.to_str().unwrap(), true)
            .unwrap(),
        external.canonicalize().unwrap()
    );
    assert!(files.existing_path("@data/missing.txt", false).is_err());
}

#[test]
fn text_read_supports_iina_encoding_names_and_rejects_malformed_input() {
    let (files, directory) = temporary_files();
    let root = directory.path();
    let path = root.join(".data/encoded.txt");
    fs::create_dir_all(path.parent().unwrap()).unwrap();

    fs::write(&path, [b'c', b'a', b'f', 0xe9, 0x80]).unwrap();
    assert_eq!(
        files
            .read_text(
                "@data/encoded.txt",
                r#"{"encoding":"windowsCP1252"}"#,
                false
            )
            .unwrap(),
        "café€"
    );
    assert_eq!(
        files
            .read_text("@data/encoded.txt", r#"{"encoding":"isoLatin1"}"#, false)
            .unwrap(),
        "café\u{0080}"
    );

    for (name, bytes, expected) in [
        ("isoLatin5", &[0xd0][..], "Ğ"),
        ("isoLatinThai", &[0xa1][..], "ก"),
        ("KOI8_R", &[0xc1][..], "а"),
        ("KOI8_U", &[0xc1][..], "а"),
        ("big5_E", b"A", "A"),
        ("big5_HKSCS_1999", b"A", "A"),
        ("ISO_2022_JP_1", b"A", "A"),
        ("ISO_2022_JP_2", b"A", "A"),
        ("ISO_2022_JP_3", b"A", "A"),
    ] {
        fs::write(&path, bytes).unwrap();
        let options = serde_json::json!({ "encoding": name }).to_string();
        assert_eq!(
            files
                .read_text("@data/encoded.txt", &options, false)
                .unwrap(),
            expected,
            "Foundation encoding name {name}"
        );
    }

    fs::write(&path, [0xff, 0xfe, b'h', 0, b'i', 0]).unwrap();
    assert_eq!(
        files
            .read_text("@data/encoded.txt", r#"{"encoding":"unicode"}"#, false)
            .unwrap(),
        "hi"
    );

    fs::write(&path, [0xff, 0xfe, 0, 0, b'h', 0, 0, 0]).unwrap();
    assert_eq!(
        files
            .read_text("@data/encoded.txt", r#"{"encoding":"utf32"}"#, false)
            .unwrap(),
        "h"
    );

    fs::write(&path, [0x82, 0xa0]).unwrap();
    assert_eq!(
        files
            .read_text("@data/encoded.txt", r#"{"encoding":"shiftJIS"}"#, false)
            .unwrap(),
        "あ"
    );

    fs::write(&path, [0xc3, 0x28]).unwrap();
    assert!(
        files
            .read_text("@data/encoded.txt", "{}", false)
            .unwrap_err()
            .contains("invalid utf8")
    );
    assert!(
        files
            .read_text(
                "@data/encoded.txt",
                r#"{"encoding":"not-an-encoding"}"#,
                false
            )
            .unwrap_err()
            .contains("Unknown or unsupported")
    );
}

#[test]
fn accepted_long_plugin_identifiers_have_isolated_private_storage() {
    let (_, directory) = temporary_files();
    let root = directory.path();
    let prefix = format!("org.example.{}", "a".repeat(128));
    let identifiers = [format!("{prefix}.alpha"), format!("{prefix}.beta")];
    for identifier in &identifiers {
        riina_core::plugin::Manifest::parse(
            &serde_json::json!({
                "name": "Storage test", "version": "1.0.0",
                "author": {"name": "Test"}, "entry": "main.js", "identifier": identifier
            })
            .to_string(),
        )
        .unwrap();
    }
    let first = PluginFiles::for_package(&root.join("first.iinaplugin"), &identifiers[0]);
    let second = PluginFiles::for_package(&root.join("second.iinaplugin"), &identifiers[1]);
    first
        .write_text("@data/private.txt", "first plugin", false)
        .unwrap();
    assert!(!second.exists("@data/private.txt", false).unwrap());
    second
        .write_text("@data/private.txt", "second plugin", false)
        .unwrap();
    assert_eq!(
        first.read_text("@data/private.txt", "{}", false).unwrap(),
        "first plugin"
    );
    assert_eq!(
        second.read_text("@data/private.txt", "{}", false).unwrap(),
        "second plugin"
    );
    assert_ne!(first.temp_root, second.temp_root);
}

#[test]
fn storage_names_do_not_alias_case_replacements_or_special_path_components() {
    let identifiers = [
        "",
        ".",
        "..",
        "org.example.plugin",
        "ORG.example.plugin",
        "org.example.plugin.",
        "org/example/plugin",
        "org?example?plugin",
        "con.example",
        "com1.example",
    ];
    let mut names = std::collections::BTreeSet::new();
    for identifier in identifiers {
        let name = safe_identifier(identifier);
        assert!(
            names.insert(name.to_ascii_lowercase()),
            "aliased {identifier}"
        );
        let mut components = Path::new(&name).components();
        assert!(matches!(components.next(), Some(Component::Normal(_))));
        assert_eq!(components.next(), None);
        assert!(!name.ends_with('.'));
        assert_eq!(name, safe_identifier(identifier));
    }
    assert_eq!(safe_identifier("org.example.plugin"), "org.example.plugin");
}
