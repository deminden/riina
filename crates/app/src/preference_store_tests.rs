use super::*;
fn path(label: &str) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    std::env::temp_dir().join(format!(
        "riina-store-{}-{}-{label}/preferences.json",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ))
}
#[test]
fn independent_writers_preserve_settings_and_can_return_to_initial_values() {
    let path = path("interleave");
    let initial = Preferences::default();
    let settings = Writer::new(path.clone(), initial.clone());
    let layout = Writer::new(path.clone(), initial.clone());
    let session = Writer::new(path.clone(), initial.clone());
    let mut changed = initial.clone();
    changed.video_threads = 7;
    changed.load_icc_profile = !initial.load_icc_profile;
    settings.save(&changed).unwrap();
    let mut resized = initial.clone();
    resized.leading_sidebar_width += 30;
    layout.save(&resized).unwrap();
    session.save(&initial).unwrap();
    let saved = Preferences::parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(saved.video_threads, 7);
    assert_eq!(saved.load_icc_profile, changed.load_icc_profile);
    assert_eq!(saved.leading_sidebar_width, resized.leading_sidebar_width);
    settings.save(&initial).unwrap();
    let saved = Preferences::parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(saved.video_threads, initial.video_threads);
    assert_eq!(saved.leading_sidebar_width, resized.leading_sidebar_width);
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
}
#[test]
fn concurrent_players_do_not_lose_read_modify_write_updates() {
    let path = path("concurrent");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let gate = path.with_extension("start");
    let ready = path.with_extension("ready");
    let mut workers = Vec::new();
    for index in 0..4 {
        let child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "preference_store::tests::preference_process_worker",
                "--ignored",
                "--nocapture",
            ])
            .env("RIINA_TEST_PREFERENCES", &path)
            .env("RIINA_TEST_WRITER", index.to_string())
            .spawn()
            .unwrap();
        workers.push(ChildGuard(child));
    }
    let until = std::time::Instant::now() + std::time::Duration::from_secs(15);
    wait_until(
        until,
        || (0..4).all(|i| ready.with_extension(i.to_string()).exists()),
        "writers ready",
    );
    std::fs::write(gate, []).unwrap();
    for (index, worker) in workers.iter_mut().enumerate() {
        let mut status = None;
        wait_until(
            until,
            || {
                status = worker.0.try_wait().unwrap();
                status.is_some()
            },
            &format!("writer {index} exit"),
        );
        assert!(status.unwrap().success(), "writer {index} failed");
    }
    let saved = Preferences::parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
    // Every process performs a read/modify/write. A process-local mutex cannot
    // satisfy this total, even if each individual replacement is atomic.
    assert_eq!(saved.video_threads, 4 * 12, "lost a cross-process update");
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

struct ChildGuard(std::process::Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
        }
        let _ = self.0.wait();
    }
}

fn wait_until(until: std::time::Instant, mut done: impl FnMut() -> bool, label: &str) {
    while !done() {
        assert!(
            std::time::Instant::now() < until,
            "Timed out waiting for {label}"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

#[test]
#[ignore = "helper launched in isolated child processes by concurrent_players_do_not_lose_read_modify_write_updates"]
fn preference_process_worker() {
    let path =
        PathBuf::from(std::env::var_os("RIINA_TEST_PREFERENCES").expect("child fixture path"));
    let index = std::env::var("RIINA_TEST_WRITER").unwrap();
    std::fs::write(path.with_extension(index), []).unwrap();
    wait_until(
        std::time::Instant::now() + std::time::Duration::from_secs(10),
        || path.with_extension("start").exists(),
        "start gate",
    );
    for _ in 0..12 {
        update(&path, &Preferences::default(), |latest| {
            // Widen the contested read/commit interval deterministically; the
            // start gate ensures all four independent processes participate.
            let previous = latest.video_threads;
            std::thread::sleep(std::time::Duration::from_millis(2));
            latest.video_threads = previous + 1;
            Ok(())
        })
        .unwrap();
    }
}

#[test]
fn stale_writer_sequences_match_explicit_field_ownership_model() {
    for seed in [1_u64, 17, 0x5eed] {
        let path = path(&format!("model-{seed}"));
        let initial = Preferences::default();
        let writers: Vec<_> = (0..4)
            .map(|_| Writer::new(path.clone(), initial.clone()))
            .collect();
        let mut snapshots = vec![initial.clone(); 4];
        let mut expected = initial.clone();
        let mut random = seed;
        for step in 0..96 {
            random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
            let index = ((random >> 32) % 4) as usize;
            let previous = snapshots[index].clone();
            let next = &mut snapshots[index];
            match (random >> 40) % 8 {
                0 => next.video_threads = (step % 8) + 1,
                1 => next.video_threads = initial.video_threads,
                2 => next.audio.audio_threads = (step % 4 + 1) as u8,
                3 => next.network.user_agent = format!("writer-{index}-step-{step}"),
                4 => next.load_icc_profile = !next.load_icc_profile,
                5 => next.network.http_proxy = format!("http://127.0.0.1:{}", 8000 + step),
                6 => next
                    .network
                    .user_agent
                    .clone_from(&initial.network.user_agent),
                _ => {} // An unchanged save must not reclaim another writer's fields.
            }
            // This oracle names individual fields; it does not reuse the JSON
            // merge algorithm whose nested-object behavior it is checking.
            if previous.video_threads != next.video_threads {
                expected.video_threads = next.video_threads;
            }
            if previous.audio.audio_threads != next.audio.audio_threads {
                expected.audio.audio_threads = next.audio.audio_threads;
            }
            if previous.network.user_agent != next.network.user_agent {
                expected
                    .network
                    .user_agent
                    .clone_from(&next.network.user_agent);
            }
            if previous.network.http_proxy != next.network.http_proxy {
                expected
                    .network
                    .http_proxy
                    .clone_from(&next.network.http_proxy);
            }
            if previous.load_icc_profile != next.load_icc_profile {
                expected.load_icc_profile = next.load_icc_profile;
            }
            writers[index]
                .save(next)
                .unwrap_or_else(|e| panic!("seed {seed}, step {step}, writer {index}: {e}"));
            let saved = Preferences::parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
            assert_eq!(
                serde_json::to_value(saved).unwrap(),
                serde_json::to_value(&expected).unwrap(),
                "seed {seed}, step {step}, writer {index}"
            );
        }
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
}

#[test]
fn rejected_changes_preserve_bytes_release_lock_and_do_not_advance_writer() {
    let path = path("rollback");
    let initial = Preferences::default();
    let writer = Writer::new(path.clone(), initial.clone());
    writer.save(&initial).unwrap();
    let original = std::fs::read(&path).unwrap();
    assert!(
        update(&path, &initial, |latest| {
            latest.volume = 19.0;
            Err("cancelled edit".into())
        })
        .is_err()
    );
    assert_eq!(std::fs::read(&path).unwrap(), original);
    let mut invalid = initial.clone();
    invalid.video_threads = u32::MAX;
    invalid.volume = 19.0;
    assert!(writer.save(&invalid).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), original);
    update(&path, &initial, |latest| {
        latest.volume = 37.0;
        Ok(())
    })
    .unwrap();
    writer.save(&initial).unwrap();
    let saved = Preferences::parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(saved.video_threads, initial.video_threads);
    assert_eq!(
        saved.volume, 37.0,
        "failed save must not change the writer's observed snapshot"
    );
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

#[test]
fn optional_theme_removal_and_history_clear_preserve_newer_network_settings() {
    let path = path("optional-history");
    let initial = Preferences::default();
    let appearance = Writer::new(path.clone(), initial.clone());
    let session = Writer::new(path.clone(), initial.clone());
    let mut theme = initial.clone();
    theme.theme_material = None;
    appearance.save(&theme).unwrap();
    let mut history = initial.clone();
    history.recent_media.push(riina_core::RecentMedia {
        path: "C:/media/example.mp4".into(),
        title: "Example".into(),
        position: 12.0,
    });
    session.save(&history).unwrap();
    update(&path, &initial, |latest| {
        latest.network.user_agent = "newer setting".into();
        Ok(())
    })
    .unwrap();
    session.save(&initial).unwrap();
    let saved = Preferences::parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert!(saved.recent_media.is_empty());
    assert_eq!(
        saved.theme_material, None,
        "stale history writer must not recreate a removed optional field"
    );
    assert_eq!(saved.network.user_agent, "newer setting");
    appearance.save(&initial).unwrap();
    let saved = Preferences::parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(saved.theme_material, initial.theme_material);
    assert!(saved.recent_media.is_empty());
    assert_eq!(saved.network.user_agent, "newer setting");
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

#[cfg(target_os = "windows")]
#[test]
fn failed_atomic_commit_can_retry_the_identical_candidate() {
    let path = path("commit-retry");
    let initial = Preferences::default();
    let writer = Writer::new(path.clone(), initial.clone());
    writer.save(&initial).unwrap();
    let before = std::fs::read(&path).unwrap();
    let permissions = std::fs::metadata(&path).unwrap().permissions();
    let mut read_only = permissions.clone();
    read_only.set_readonly(true);
    std::fs::set_permissions(&path, read_only).unwrap();
    let mut next = initial;
    next.video_threads = 7;
    next.volume = 19.0;
    let result = writer.save(&next);
    std::fs::set_permissions(&path, permissions).unwrap();
    assert!(result.is_err(), "fixture must fail at atomic replacement");
    assert_eq!(std::fs::read(&path).unwrap(), before);
    writer.save(&next).unwrap();
    let saved = Preferences::parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(saved.video_threads, 7);
    assert_eq!(saved.volume, 19.0);
    assert_eq!(
        std::fs::read_dir(path.parent().unwrap()).unwrap().count(),
        2,
        "only preferences and the stable lock file should remain"
    );
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

#[test]
fn json_merge_handles_removal_arrays_and_type_changes_without_losing_siblings() {
    use serde_json::json;
    let cases = [
        (
            json!({"a":{"keep":9,"edit":1,"new":8}}),
            json!({"a":{"keep":0,"edit":1}}),
            json!({"a":{"keep":0,"edit":2}}),
            json!({"a":{"keep":9,"edit":2,"new":8}}),
        ),
        (
            json!({"optional":1,"other":2}),
            json!({"optional":1}),
            json!({}),
            json!({"other":2}),
        ),
        (
            json!({"list":[9],"other":2}),
            json!({"list":[1]}),
            json!({"list":[2,3]}),
            json!({"list":[2,3],"other":2}),
        ),
        (
            json!({"a":null,"other":2}),
            json!({"a":1}),
            json!({"a":{"b":3}}),
            json!({"a":{"b":3},"other":2}),
        ),
        (
            json!({"a":9}),
            json!({}),
            json!({"a":null}),
            json!({"a":null}),
        ),
        (
            json!({"other":2}),
            json!({"a":1,"edit":1}),
            json!({"a":1,"edit":2}),
            json!({"other":2,"edit":2}),
        ),
    ];
    for (index, (mut latest, previous, next, expected)) in cases.into_iter().enumerate() {
        merge_changes(&mut latest, &previous, &next);
        assert_eq!(latest, expected, "merge case {index}");
    }
}
#[test]
fn invalid_existing_file_is_preserved_and_failed_write_can_be_retried() {
    let path = path("failure");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "not json").unwrap();
    let initial = Preferences::default();
    let writer = Writer::new(path.clone(), initial.clone());
    let mut next = initial;
    next.video_threads = 3;
    assert!(writer.save(&next).is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "not json");
    std::fs::remove_file(&path).unwrap();
    writer.save(&next).unwrap();
    assert_eq!(
        Preferences::parse(&std::fs::read_to_string(&path).unwrap())
            .unwrap()
            .video_threads,
        3
    );
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
}
