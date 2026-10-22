use super::*;
use riina_core::Preferences;

struct TestPath(PathBuf);
impl TestPath {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        Self(std::env::temp_dir().join(format!(
            "riina-preferences-window-{}-{}/preferences.json",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        )))
    }
    fn read(&self) -> Preferences {
        Preferences::parse(&std::fs::read_to_string(&self.0).unwrap()).unwrap()
    }
}
impl Drop for TestPath {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(self.0.parent().unwrap());
    }
}

fn windows() -> (PlayerWindow, PreferencesWindow) {
    i_slint_backend_testing::init_no_event_loop();
    (
        PlayerWindow::new().unwrap(),
        PreferencesWindow::new().unwrap(),
    )
}

#[test]
fn music_mode_choices_are_saved_immediately_and_can_return_to_defaults() {
    let (app, window) = windows();
    let path = TestPath::new();
    let initial = Preferences::default();
    let writer = crate::preference_store::Writer::new(path.0.clone(), initial.clone());
    window.set_auto_switch_to_music_mode(!initial.auto_switch_to_music_mode);
    window.set_music_mode_show_playlist(!initial.music_mode_show_playlist);
    window.set_music_mode_show_album_art(!initial.music_mode_show_album_art);
    save_preferences(&window, &app, &writer, initial.clone()).unwrap();
    let saved = path.read();
    assert_eq!(
        saved.auto_switch_to_music_mode,
        !initial.auto_switch_to_music_mode
    );
    assert_eq!(
        saved.music_mode_show_playlist,
        !initial.music_mode_show_playlist
    );
    assert_eq!(
        saved.music_mode_show_album_art,
        !initial.music_mode_show_album_art
    );
    assert_eq!(
        app.get_auto_switch_to_music_mode(),
        saved.auto_switch_to_music_mode
    );
    assert_eq!(
        app.get_music_mode_show_playlist(),
        saved.music_mode_show_playlist
    );
    assert_eq!(
        app.get_music_mode_show_album_art(),
        saved.music_mode_show_album_art
    );

    window.set_auto_switch_to_music_mode(initial.auto_switch_to_music_mode);
    window.set_music_mode_show_playlist(initial.music_mode_show_playlist);
    window.set_music_mode_show_album_art(initial.music_mode_show_album_art);
    save_preferences(&window, &app, &writer, initial.clone()).unwrap();
    let saved = path.read();
    assert_eq!(
        saved.auto_switch_to_music_mode,
        initial.auto_switch_to_music_mode
    );
    assert_eq!(
        saved.music_mode_show_playlist,
        initial.music_mode_show_playlist
    );
    assert_eq!(
        saved.music_mode_show_album_art,
        initial.music_mode_show_album_art
    );
}

#[test]
fn failed_preferences_save_keeps_live_music_mode_choices_and_is_retryable() {
    let (app, window) = windows();
    let path = TestPath::new();
    let initial = Preferences::default();
    let writer = crate::preference_store::Writer::new(path.0.clone(), initial.clone());
    // A directory in place of the preferences file reliably rejects the write.
    std::fs::create_dir_all(&path.0).unwrap();
    window.set_auto_switch_to_music_mode(!initial.auto_switch_to_music_mode);
    window.set_music_mode_show_playlist(!initial.music_mode_show_playlist);
    window.set_music_mode_show_album_art(!initial.music_mode_show_album_art);
    assert!(save_preferences(&window, &app, &writer, initial.clone()).is_err());
    assert_eq!(
        app.get_auto_switch_to_music_mode(),
        initial.auto_switch_to_music_mode
    );
    assert_eq!(
        app.get_music_mode_show_playlist(),
        initial.music_mode_show_playlist
    );
    assert_eq!(
        app.get_music_mode_show_album_art(),
        initial.music_mode_show_album_art
    );
    std::fs::remove_dir(&path.0).unwrap();
    save_preferences(&window, &app, &writer, initial).unwrap();
    assert_eq!(
        path.read().music_mode_show_playlist,
        window.get_music_mode_show_playlist()
    );
    assert_eq!(
        app.get_music_mode_show_playlist(),
        window.get_music_mode_show_playlist()
    );
}
