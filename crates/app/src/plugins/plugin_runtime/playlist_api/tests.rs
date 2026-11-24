use crate::{plugin_runtime::*, plugin_snapshot::player_file_context};
use riina_core::{Phase, Playback, PlaylistEntry};
use riina_test_support::TempDir;

fn playback(phase: Phase) -> Playback {
    Playback {
        phase,
        playlist: [10, 30, 20]
            .into_iter()
            .map(|id| PlaylistEntry {
                id,
                filename: format!("{id}.mkv"),
                title: (id == 10).then(|| "Main feature".into()),
                current: id == 10,
                playing: id == 30,
            })
            .collect::<Vec<_>>()
            .into(),
        ..Default::default()
    }
}

#[test]
fn playlist_api_reads_the_observed_rows_and_queues_typed_commands() {
    let root = TempDir::new("plugin-playlist").unwrap();
    let mut manifest = super::super::tests::manifest("main.js".into());
    manifest.permissions.push("network-request".into());
    manifest.allowed_domains.push("media.example".into());
    std::fs::write(
        root.path().join(&manifest.entry),
        r#"
        const list = iina.playlist;
        const rows = list.list();
        if (list.count() !== 3 || rows.length !== 3) throw new Error('playlist length');
        if (rows[0].filename !== '10.mkv' || rows[0].title !== 'Main feature' ||
            !rows[0].isCurrent || rows[0].isPlaying || !rows[1].isPlaying ||
            rows[1].isCurrent || rows[1].title !== null) throw new Error('playlist fields');
        if (list.add(['https://media.example/one.mkv', 'https://blocked.example/x.mkv',
            'https://media.example/two.mkv'], 1) !== false) throw new Error('partial add');
        if (!list.add('https://media.example/last.mkv')) throw new Error('append');
        if (!list.remove([2, 1, 1])) throw new Error('remove');
        if (!list.move(0, 2)) throw new Error('move');
        if (list.play(1) !== undefined) throw new Error('play result');
        list.playNext();
        list.playPrevious();
        "#,
    )
    .unwrap();
    let context = player_file_context(&playback(Phase::Paused), &[], &[]);
    let session = PluginRuntimeSession::start_package_entry_with_player_context(
        root.path(),
        &manifest,
        &manifest.permissions,
        context,
        None,
        Default::default(),
    )
    .unwrap();
    let events = session.drain_events().unwrap();
    let commands = events
        .into_iter()
        .map(|event| match event {
            HostEvent::Ui(UiEffect::CoreCommand(command)) => command,
            other => panic!("Unexpected plugin event {other:?}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        commands,
        [
            Command::InsertPlaylistFilesBefore {
                paths: vec![
                    "https://media.example/one.mkv".into(),
                    "https://media.example/two.mkv".into()
                ],
                before_id: Some(30),
            },
            Command::InsertPlaylistFilesBefore {
                paths: vec!["https://media.example/last.mkv".into()],
                before_id: None,
            },
            Command::RemovePlaylistIds(vec![20, 30]),
            Command::MovePlaylistIdBefore {
                id: 10,
                before_id: 20
            },
            Command::SelectPlaylistId(30),
            Command::PlaylistNext,
            Command::PlaylistPrevious,
        ]
    );
}

#[test]
fn playlist_api_rejects_stale_idle_state_and_malformed_arguments() {
    let session = PluginRuntimeSession::start_entry(
        r#"
        const list = iina.playlist;
        iina.event.on('iina.menu-update', () => {
            if (list.list().length === 0) {
                if (list.count() !== 3 || list.add('memory://idle') || list.remove(0) || list.move(0, 2))
                    throw new Error('idle playlist');
                list.play(0);
                list.playNext();
                list.playPrevious();
                iina.log('idle');
                return;
            }
            if (list.add() || list.add([1, 'memory://x']) ||
                list.add(new Array(0xffffffff)) || list.add('memory://x', list.count()) ||
                list.add('https://media.example/no-grant.mkv') ||
                list.remove([0, list.count()]) || list.remove(1.5) || list.move(1, 1) ||
                list.move(0, Infinity)) throw new Error('invalid playlist argument');
            list.play(-1);
            iina.log('active');
        });
        "#,
        "org.example.playlist-validation",
        &[],
        &[],
    )
    .unwrap();
    for (phase, log) in [(Phase::Stopped, "idle"), (Phase::Playing, "active")] {
        session.update_player_file_context(player_file_context(&playback(phase), &[], &[]));
        session
            .deliver_player_event("iina.menu-update".into(), serde_json::Value::Null)
            .unwrap();
        session.flush_events(Duration::from_secs(1)).unwrap();
        assert_eq!(
            session.drain_events().unwrap(),
            [HostEvent::Ui(UiEffect::Log(log.into()))]
        );
    }
}
