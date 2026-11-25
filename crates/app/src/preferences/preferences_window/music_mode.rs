//! Explicit Music Mode choices belong to the profile. Resize gestures are local.
use super::PreferenceModel;
use crate::{PlayerWindow, PreferencesWindow, music_mode_layout};
use riina_ui::{GeneralSettingsState, PlayerMusicModeState};
use slint::ComponentHandle;
use std::{cell::RefCell, path::PathBuf, rc::Rc};

#[derive(Clone, Copy)]
enum Choice {
    Catalog,
    Artwork,
}

pub(super) fn connect(
    app: &PlayerWindow,
    window: Rc<RefCell<Option<PreferencesWindow>>>,
    path: PathBuf,
    model: PreferenceModel,
) {
    let weak = app.as_weak();
    let save = Rc::new(move |choice, visible| {
        let Some(app) = weak.upgrade() else { return };
        let mode = app.global::<PlayerMusicModeState>();
        let edit = |prefs: &mut riina_core::Preferences| match choice {
            Choice::Catalog => prefs.music_mode_show_playlist = visible,
            Choice::Artwork => prefs.music_mode_show_album_art = visible,
        };
        if let Err(error) = crate::preference_store::update(&path, &model.snapshot(), |prefs| {
            edit(prefs);
            Ok(())
        }) {
            crate::diagnostics::report(&app, error);
            return;
        }
        model.adopt_committed(edit);
        let window = window.borrow();
        let general = window
            .as_ref()
            .map(|window| window.global::<GeneralSettingsState>());
        mode.set_music_mode_live_catalog_height(-1.0);
        match choice {
            Choice::Catalog => {
                set_catalog_preference(&app, visible);
                if let Some(general) = general {
                    general.set_music_mode_show_playlist(visible);
                }
            }
            Choice::Artwork => {
                mode.set_music_mode_show_album_art(visible);
                if let Some(general) = general {
                    general.set_music_mode_show_album_art(visible);
                }
            }
        }
    });
    let mode = app.global::<PlayerMusicModeState>();
    let catalog = save.clone();
    mode.on_set_catalog_visible(move |visible| catalog(Choice::Catalog, visible));
    mode.on_set_artwork_visible(move |visible| save(Choice::Artwork, visible));
}

pub(super) fn set_catalog_preference(app: &PlayerWindow, visible: bool) {
    let mode = app.global::<PlayerMusicModeState>();
    mode.set_music_mode_playlist_preference(visible);
    if visible != mode.get_music_mode_show_playlist() {
        mode.set_music_mode_catalog_height(music_mode_layout::DEFAULT_CATALOG_HEIGHT);
        mode.set_music_mode_show_playlist(visible);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_choices_save_immediately_and_automatic_resize_visibility_stays_local() {
        i_slint_backend_testing::init_no_event_loop();
        let app = PlayerWindow::new().unwrap();
        let settings = PreferencesWindow::new().unwrap();
        let general = settings.global::<GeneralSettingsState>();
        let profile = riina_test_support::TempDir::new("music-mode-choices").unwrap();
        let path = profile.path().join("preferences.json");
        let model = PreferenceModel::new(path.clone(), riina_core::Preferences::default());
        connect(
            &app,
            Rc::new(RefCell::new(Some(settings.clone_strong()))),
            path.clone(),
            model.clone(),
        );
        let mode = app.global::<PlayerMusicModeState>();
        mode.invoke_set_catalog_visible(true);
        mode.invoke_set_artwork_visible(false);
        assert!(mode.get_music_mode_show_playlist());
        assert!(general.get_music_mode_show_playlist());
        assert!(!general.get_music_mode_show_album_art());
        // The resize changes the current presentation and leaves the next
        // player's preference alone, including through session save.
        mode.set_music_mode_show_playlist(false);
        model.save_session(model.snapshot()).unwrap();
        let saved = crate::preference_load::read_valid(&path).unwrap().unwrap();
        assert!(saved.music_mode_show_playlist);
        assert!(!saved.music_mode_show_album_art);
        assert!(mode.get_music_mode_playlist_preference());
    }
}
