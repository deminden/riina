use riina_ui::{PlaybackMenuCatalog, PlaybackMenuRow};
use slint::{Keys, ModelRc, VecModel};

#[test]
fn idle_playback_catalog_follows_the_player_sections_and_profile() {
    i_slint_backend_testing::init_no_event_loop();
    let catalog = PlaybackMenuCatalog::new().unwrap();
    let mut prefs = riina_core::Preferences::default();
    prefs
        .video_filters
        .save("My video filter", "hflip")
        .unwrap();
    catalog.set_options(super::playback_menu_options::from_profile(&prefs));
    let profile = crate::key_binding_preferences::profile_menu_shortcuts("IINA Default").unwrap();
    let rows = catalog.snapshot(ModelRc::new(VecModel::from(profile.clone())));
    assert_eq!(
        rows.iter()
            .map(|row| row.title.as_str())
            .collect::<Vec<_>>(),
        PlaybackMenuCatalog::FAMILIES
    );
    let [playback, video, audio, subtitles] = rows.as_slice() else {
        panic!("playback families missing")
    };
    assert_eq!(row(playback, "Pause").shortcut, profile[0].shortcut);
    assert_ne!(row(playback, "Pause").shortcut, Keys::default());
    row(playback, "Go to Screenshot Folder");
    let rotation = row(video, "Rotation");
    assert!(rotation.children.iter().any(|child| child.title == "90°"));
    row(row(video, "Aspect Ratio"), "16:9");
    row(row(video, "Saved Video Filters"), "My video filter");
    row(audio, "Mute");
    assert_eq!(
        row(subtitles, "Encoding").children.len(),
        riina_core::subtitle_controls::SUBTITLE_ENCODING_CHOICES.len()
    );
    assert!(playback.children.iter().any(|child| child.separator));

    let mut profile = profile;
    let alternate = Keys::from_parts(["Meta", "p"]).unwrap();
    profile[0].shortcut = alternate.clone();
    let changed = catalog.snapshot(ModelRc::new(VecModel::from(profile)));
    assert_eq!(row(&changed[0], "Pause").shortcut, alternate);
    assert_eq!(&changed[1..], &rows[1..]);
}

fn row<'a>(section: &'a PlaybackMenuRow, title: &str) -> &'a PlaybackMenuRow {
    section
        .children
        .iter()
        .find(|row| row.title == title)
        .unwrap_or_else(|| panic!("{} has no {title:?} row", section.title))
}
