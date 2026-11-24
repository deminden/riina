use super::*;
use riina_core::Preferences;
use std::path::PathBuf;

mod transactions;

struct TestPath {
    file: PathBuf,
    _directory: riina_test_support::TempDir,
}

impl TestPath {
    fn new() -> Self {
        let directory = riina_test_support::TempDir::new("preferences-window").unwrap();
        Self {
            file: directory.path().join("preferences.json"),
            _directory: directory,
        }
    }

    fn read(&self) -> Preferences {
        Preferences::parse(&std::fs::read_to_string(&self.file).unwrap()).unwrap()
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
    let model = PreferenceModel::new(path.file.clone(), initial.clone());
    window
        .global::<riina_ui::GeneralSettingsState>()
        .set_auto_switch_to_music_mode(!initial.auto_switch_to_music_mode);
    window
        .global::<riina_ui::GeneralSettingsState>()
        .set_music_mode_show_playlist(!initial.music_mode_show_playlist);
    window
        .global::<riina_ui::GeneralSettingsState>()
        .set_music_mode_show_album_art(!initial.music_mode_show_album_art);
    save_preferences(&window, &app, &model, initial.clone()).unwrap();
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
        app.global::<riina_ui::PlayerMusicModeState>()
            .get_auto_switch_to_music_mode(),
        saved.auto_switch_to_music_mode
    );
    assert_eq!(
        app.global::<riina_ui::PlayerMusicModeState>()
            .get_music_mode_show_playlist(),
        saved.music_mode_show_playlist
    );
    assert_eq!(
        app.global::<riina_ui::PlayerMusicModeState>()
            .get_music_mode_show_album_art(),
        saved.music_mode_show_album_art
    );

    window
        .global::<riina_ui::GeneralSettingsState>()
        .set_auto_switch_to_music_mode(initial.auto_switch_to_music_mode);
    window
        .global::<riina_ui::GeneralSettingsState>()
        .set_music_mode_show_playlist(initial.music_mode_show_playlist);
    window
        .global::<riina_ui::GeneralSettingsState>()
        .set_music_mode_show_album_art(initial.music_mode_show_album_art);
    save_preferences(&window, &app, &model, initial.clone()).unwrap();
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
    let model = PreferenceModel::new(path.file.clone(), initial.clone());
    // A directory in place of the preferences file reliably rejects the write.
    std::fs::create_dir_all(&path.file).unwrap();
    window
        .global::<riina_ui::GeneralSettingsState>()
        .set_auto_switch_to_music_mode(!initial.auto_switch_to_music_mode);
    window
        .global::<riina_ui::GeneralSettingsState>()
        .set_music_mode_show_playlist(!initial.music_mode_show_playlist);
    window
        .global::<riina_ui::GeneralSettingsState>()
        .set_music_mode_show_album_art(!initial.music_mode_show_album_art);
    assert!(save_preferences(&window, &app, &model, initial.clone()).is_err());
    assert_eq!(
        app.global::<riina_ui::PlayerMusicModeState>()
            .get_auto_switch_to_music_mode(),
        initial.auto_switch_to_music_mode
    );
    assert_eq!(
        app.global::<riina_ui::PlayerMusicModeState>()
            .get_music_mode_show_playlist(),
        initial.music_mode_show_playlist
    );
    assert_eq!(
        app.global::<riina_ui::PlayerMusicModeState>()
            .get_music_mode_show_album_art(),
        initial.music_mode_show_album_art
    );
    std::fs::remove_dir(&path.file).unwrap();
    save_preferences(&window, &app, &model, initial).unwrap();
    assert_eq!(
        path.read().music_mode_show_playlist,
        window
            .global::<riina_ui::GeneralSettingsState>()
            .get_music_mode_show_playlist()
    );
    assert_eq!(
        app.global::<riina_ui::PlayerMusicModeState>()
            .get_music_mode_show_playlist(),
        window
            .global::<riina_ui::GeneralSettingsState>()
            .get_music_mode_show_playlist()
    );
}
