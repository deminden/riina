use super::*;
use riina_test_support::TempDir;
use std::sync::{Arc, Barrier};

#[test]
fn concurrent_plugin_permission_updates_preserve_other_plugins() {
    let fixture = TempDir::new("plugin-grants").unwrap();
    let barrier = Arc::new(Barrier::new(6));
    std::thread::scope(|scope| {
        for writer in 0..6 {
            let barrier = barrier.clone();
            let preferences = fixture.path().join("preferences.json");
            scope.spawn(move || {
                for round in 0..12 {
                    barrier.wait();
                    write_plugin_grants(
                        &preferences,
                        &format!("org.example.writer{writer}.round{round}"),
                        Some(vec!["show-osd".into()]),
                    )
                    .unwrap();
                }
            });
        }
    });
    let path = fixture.path().join("plugin-permissions.json");
    let grants: BTreeMap<String, Vec<String>> =
        serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(grants.len(), 72, "lost a concurrently saved plugin grant");
}

#[test]
fn permission_updates_preserve_denials_and_rollback_only_the_target_plugin() {
    let fixture = TempDir::new("plugin-grants").unwrap();
    let preferences = fixture.path().join("preferences.json");
    let manifest = |permissions| {
        Manifest::parse(&serde_json::json!({"name":"Test","identifier":"org.example.target","version":"1.0.0","author":{"name":"Test"},"entry":"main.js","permissions":permissions}).to_string()).unwrap()
    };
    write_plugin_grants(
        &preferences,
        "org.example.other",
        Some(vec!["show-alert".into()]),
    )
    .unwrap();
    let old = manifest(vec!["show-osd", "file-system"]);
    let updated = manifest(vec!["show-osd", "file-system", "show-alert"]);
    save_updated_permissions(&preferences, &updated, &old, &["show-osd".into()]).unwrap();
    assert_eq!(
        read_plugin_grants(&preferences, "org.example.target")
            .unwrap()
            .unwrap(),
        ["show-alert", "show-osd"]
    );
    restore_plugin_grants(&preferences, "org.example.target", None).unwrap();
    assert_eq!(
        read_plugin_grants(&preferences, "org.example.target").unwrap(),
        None
    );
    assert_eq!(
        read_plugin_grants(&preferences, "org.example.other")
            .unwrap()
            .unwrap(),
        ["show-alert"]
    );
}

#[test]
fn malformed_permission_state_is_not_overwritten() {
    let fixture = TempDir::new("plugin-grants").unwrap();
    let path = fixture.path().join("plugin-permissions.json");
    let original = b"{ interrupted grant state";
    fs::write(&path, original).unwrap();
    assert!(
        write_plugin_grants(
            &fixture.path().join("preferences.json"),
            "org.example.target",
            Some(vec!["show-osd".into()])
        )
        .is_err()
    );
    assert_eq!(fs::read(path).unwrap(), original);
}
