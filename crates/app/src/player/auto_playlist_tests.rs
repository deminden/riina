use super::*;
use riina_core::PlaylistEntry;
use riina_test_support::TempDir;

struct Folder(TempDir);

impl Folder {
    fn new() -> Self {
        Self(TempDir::new("folder-日本語").unwrap())
    }

    fn file(&self, name: &str) -> String {
        let path = self.0.path().join(name);
        std::fs::write(&path, []).unwrap();
        path.to_str().unwrap().to_owned()
    }
}

fn state(path: &str) -> Playback {
    Playback {
        generation: 7,
        phase: Phase::Paused,
        playlist: vec![PlaylistEntry {
            id: 31,
            filename: path.into(),
            title: None,
            current: true,
            playing: true,
        }]
        .into(),
        ..Playback::default()
    }
}

#[test]
fn discovery_orders_each_media_group_and_excludes_hidden_nested_and_existing_files() {
    let folder = Folder::new();
    let first = folder.file("clip 2.mp4");
    let current = folder.file("clip 10.mp4");
    let last = folder.file("clip 20.mkv");
    let audio = folder.file("audio 2.wav");
    folder.file("audio 10.wav");
    folder.file(".hidden.mp4");
    folder.file("notes.txt");
    std::fs::create_dir(folder.0.path().join("nested.mp4")).unwrap();
    let mut state = state(&current);
    std::sync::Arc::make_mut(&mut state.playlist).push(PlaylistEntry {
        id: 32,
        filename: folder
            .0
            .path()
            .join("audio 10.wav")
            .to_str()
            .unwrap()
            .into(),
        title: None,
        current: false,
        playing: false,
    });
    let request = AutoFolderAdder::default().observe(&state, true).unwrap();
    let Some(Command::InsertPlaylistFilesAroundCurrent { before, after }) =
        prepare(&request, &state)
    else {
        panic!("Expected files around the active item")
    };
    assert_eq!(before, [first]);
    assert_eq!(after, [last, audio]);
    // Discovery must not change the playlist, current item or engine identity.
    assert_eq!(state.playlist.len(), 2);
    assert_eq!(state.playlist[0].id, 31);
    assert!(state.playlist[0].current);
}

#[test]
fn discovery_is_once_per_media_and_generation_and_can_be_reenabled() {
    let folder = Folder::new();
    let mut state = state(&folder.file("clip.mp4"));
    let mut adder = AutoFolderAdder::default();
    assert!(adder.observe(&state, true).is_some());
    assert!(adder.observe(&state, true).is_none());
    state.generation += 1;
    assert!(adder.observe(&state, true).is_some());
    assert!(adder.observe(&state, false).is_none());
    assert!(adder.observe(&state, true).is_some());
    std::sync::Arc::make_mut(&mut state.playlist)[0].filename = folder.file("next.mp4");
    assert!(adder.observe(&state, true).is_some());
}

#[test]
fn worker_results_cannot_attach_to_a_new_media_or_generation() {
    let folder = Folder::new();
    let mut state = state(&folder.file("clip.mp4"));
    folder.file("next.mp4");
    let request = AutoFolderAdder::default().observe(&state, true).unwrap();
    state.generation += 1;
    assert!(!request_is_current(&request, &state));
    assert!(prepare(&request, &state).is_none());
    state.generation -= 1;
    std::sync::Arc::make_mut(&mut state.playlist)[0].filename =
        folder.0.path().join("next.mp4").to_str().unwrap().into();
    assert!(prepare(&request, &state).is_none());
}

#[test]
fn remote_loading_and_already_complete_playlists_do_not_add_files() {
    let mut adder = AutoFolderAdder::default();
    assert!(
        adder
            .observe(&state("https://example.invalid/clip.mp4"), true)
            .is_none()
    );
    let folder = Folder::new();
    let mut state = state(&folder.file("clip.mp4"));
    state.phase = Phase::Loading;
    assert!(adder.observe(&state, true).is_none());
    state.phase = Phase::Playing;
    let request = adder.observe(&state, true).unwrap();
    assert!(prepare(&request, &state).is_none());
}
