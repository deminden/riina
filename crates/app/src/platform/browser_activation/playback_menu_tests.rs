use riina_ui::{
    IdlePlaybackAction, PlaybackAudioState, PlaybackMenuCatalog, PlaybackMenuRow, PlaybackMenuText,
};
use slint::{ComponentHandle, Keys, ModelRc, VecModel};

#[test]
fn idle_playback_catalog_follows_the_player_sections_and_profile() {
    i_slint_backend_testing::init_no_event_loop();
    let catalog = PlaybackMenuCatalog::new().unwrap();
    slint::select_bundled_translation("en").unwrap();
    let mut prefs = riina_core::Preferences::default();
    for title in ["Video Filters…", "Видеофильтры…", "مرشّحات الفيديو…"]
    {
        prefs.video_filters.save(title, "hflip").unwrap();
    }
    catalog.set_options(super::playback_menu_options::from_profile(&prefs));
    let profile = crate::key_binding_preferences::profile_menu_shortcuts("IINA Default").unwrap();
    let rows = catalog.snapshot(ModelRc::new(VecModel::from(profile.clone())));
    assert_eq!(
        rows.iter()
            .map(|row| row.title.as_str())
            .collect::<Vec<_>>(),
        ["Playback", "Video", "Audio", "Subtitles"]
    );
    let [playback, video, audio, subtitles] = rows.as_slice() else {
        panic!("playback families missing")
    };
    assert_eq!(row(playback, "Pause").shortcut, profile[0].shortcut);
    assert_ne!(row(playback, "Pause").shortcut, Keys::default());
    assert_eq!(
        row(playback, "Go to Screenshot Folder").idle_action,
        Some(IdlePlaybackAction::ScreenshotFolder)
    );
    let rotation = row(video, "Rotation");
    assert!(rotation.children.iter().any(|child| child.title == "90°"));
    row(row(video, "Aspect Ratio"), "16:9");
    assert_eq!(
        row(video, "Video Filters…").idle_action,
        Some(IdlePlaybackAction::VideoFilters)
    );
    assert_eq!(
        row(row(video, "Saved Video Filters"), "Video Filters…").idle_action,
        None
    );
    assert_eq!(
        row(audio, "Audio Filters…").idle_action,
        Some(IdlePlaybackAction::AudioFilters)
    );
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

#[test]
fn localized_playback_menus_keep_intents_shortcuts_and_user_names() {
    i_slint_backend_testing::init_no_event_loop();
    let catalog = PlaybackMenuCatalog::new().unwrap();
    let mut prefs = riina_core::Preferences::default();
    prefs.video_filters.save("Видеофильтры…", "hflip").unwrap();
    catalog.set_options(super::playback_menu_options::from_profile(&prefs));
    let profile = crate::key_binding_preferences::profile_menu_shortcuts("IINA Default").unwrap();
    let shortcuts = ModelRc::new(VecModel::from(profile));
    let player = crate::PlayerWindow::new().unwrap();
    let audio = player.global::<PlaybackAudioState>();
    audio.set_volume_menu_value("42".into());
    audio.set_muted(true);
    slint::select_bundled_translation("en").unwrap();
    assert_eq!(audio.get_volume_menu_title(), "Volume: 42 (Muted)");
    let english = catalog.snapshot(shortcuts.clone());
    for (language, families, filters, saved) in [
        (
            "ru",
            ["Воспроизведение", "Видео", "Аудио", "Субтитры"],
            "Видеофильтры…",
            "Сохранённые видеофильтры",
        ),
        (
            "ar",
            ["التشغيل", "الفيديو", "الصوت", "الترجمة"],
            "مرشّحات الفيديو…",
            "مرشّحات الفيديو المحفوظة",
        ),
    ] {
        slint::select_bundled_translation(language).unwrap();
        let localized = catalog.snapshot(shortcuts.clone());
        assert_eq!(
            localized
                .iter()
                .map(|row| row.title.as_str())
                .collect::<Vec<_>>(),
            families
        );
        assert_structure(&english, &localized);
        assert_eq!(
            row(&localized[1], filters).idle_action,
            Some(IdlePlaybackAction::VideoFilters)
        );
        assert_eq!(
            row(row(&localized[1], saved), "Видеофильтры…").idle_action,
            None
        );
    }
    assert_eq!(audio.get_volume_menu_title(), "مستوى الصوت: 42 (مكتوم)");
    let provider = player
        .global::<PlaybackMenuText>()
        .invoke_online_subtitles("External Service".into(), 1);
    assert_eq!(provider, "البحث عن ترجمة على External Service");
    slint::select_bundled_translation("en").unwrap();
}

fn assert_structure(expected: &[PlaybackMenuRow], actual: &[PlaybackMenuRow]) {
    assert_eq!(expected.len(), actual.len());
    for (expected, actual) in expected.iter().zip(actual) {
        assert_eq!(actual.separator, expected.separator);
        assert_eq!(actual.shortcut, expected.shortcut);
        assert_eq!(actual.idle_action, expected.idle_action);
        assert_structure(&expected.children, &actual.children);
    }
}

fn row<'a>(section: &'a PlaybackMenuRow, title: &str) -> &'a PlaybackMenuRow {
    section
        .children
        .iter()
        .find(|row| row.title == title)
        .unwrap_or_else(|| panic!("{} has no {title:?} row", section.title))
}
