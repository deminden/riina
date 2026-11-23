//! Exercise Settings and Welcome routes with headless window input.
use i_slint_backend_testing::{AccessibleRole, ElementHandle};
use riina_ui::{
    GeneralSettingsState, PluginPreferencesRow, PreferencesWindow, VideoSettingsState,
    WelcomeWindow,
};
use slint::{
    ComponentHandle, LogicalSize, ModelRc, VecModel,
    platform::{PointerEventButton, WindowEvent},
};
use std::{cell::Cell, error::Error, rc::Rc};

fn button(settings: &PreferencesWindow, label: &str) -> Result<ElementHandle, Box<dyn Error>> {
    let mut candidates = ElementHandle::find_by_accessible_label(settings, label)
        .filter(|element| element.accessible_role() == Some(AccessibleRole::Button));
    let button = candidates
        .next()
        .ok_or_else(|| format!("Missing Settings route {label}"))?;
    assert!(
        candidates.next().is_none(),
        "Multiple Settings routes for {label}"
    );
    Ok(button)
}

fn select(settings: &PreferencesWindow, label: &str) -> Result<(), Box<dyn Error>> {
    button(settings, label)?.invoke_accessible_default_action();
    super::flush();
    Ok(())
}

#[test]
fn settings_switch_and_disclosure_activate_on_first_click() -> Result<(), Box<dyn Error>> {
    i_slint_backend_testing::init_no_event_loop();
    let settings = PreferencesWindow::new()?;
    settings.set_disable_animations(true);
    settings.window().set_size(LogicalSize::new(750.0, 670.0));
    let general = settings.global::<GeneralSettingsState>();
    general.set_allow_multiple_windows(true);
    settings.show()?;
    super::flush();

    assert!(!general.get_group_simultaneous_opens_in_playlist());
    ElementHandle::find_by_accessible_label(
        &settings,
        "Group media in playlist when opening multiple files at once",
    )
    .find(|element| element.accessible_role() == Some(AccessibleRole::Switch))
    .ok_or("Missing media grouping switch")?
    .mock_single_click(PointerEventButton::Left);
    assert!(general.get_group_simultaneous_opens_in_playlist());

    settings
        .window()
        .dispatch_event(WindowEvent::KeyPressed { text: " ".into() });
    settings
        .window()
        .dispatch_event(WindowEvent::KeyReleased { text: " ".into() });
    assert!(!general.get_group_simultaneous_opens_in_playlist());

    assert!(!general.get_when_media_opened_expanded());
    button(&settings, "When media is opened")?.mock_single_click(PointerEventButton::Left);
    assert!(general.get_when_media_opened_expanded());
    settings.hide()?;
    Ok(())
}

#[test]
fn welcome_actions_accept_first_pointer_clicks() -> Result<(), Box<dyn Error>> {
    i_slint_backend_testing::init_no_event_loop();
    let welcome = WelcomeWindow::new()?;
    let file_requests = Rc::new(Cell::new(0));
    let recorded = file_requests.clone();
    welcome.on_open_file(move || recorded.set(recorded.get() + 1));
    welcome.show()?;
    super::flush();

    ElementHandle::find_by_accessible_label(&welcome, "Open a media file")
        .next()
        .ok_or("Missing Welcome file action")?
        .mock_single_click(PointerEventButton::Left);
    assert_eq!(file_requests.get(), 1);
    ElementHandle::find_by_accessible_label(&welcome, "Open media from a URL")
        .next()
        .ok_or("Missing Welcome URL action")?
        .mock_single_click(PointerEventButton::Left);
    assert!(welcome.get_url_entry_visible());
    welcome.hide()?;
    Ok(())
}

#[test]
fn settings_pointer_clicks_select_sections_and_open_the_requested_plugin_row()
-> Result<(), Box<dyn Error>> {
    i_slint_backend_testing::init_no_event_loop();
    let settings = PreferencesWindow::new()?;
    settings.set_disable_animations(true);
    settings.window().set_size(LogicalSize::new(750.0, 670.0));
    settings.set_plugin_rows(ModelRc::new(VecModel::from(vec![
        PluginPreferencesRow {
            name: "No form".into(),
            ..Default::default()
        },
        PluginPreferencesRow {
            name: "Form".into(),
            has_settings: true,
            ..Default::default()
        },
    ])));
    let requested = Rc::new(Cell::new(None));
    let selection = requested.clone();
    settings.on_plugin_row_settings(move |index| selection.set(Some(index)));
    settings.show()?;
    super::flush();

    button(&settings, "History")?.mock_single_click(PointerEventButton::Left);
    assert_eq!(settings.get_selected_general_section(), 1);
    button(&settings, "Plugins")?.mock_single_click(PointerEventButton::Left);
    assert_eq!(settings.get_selected_pane(), 9);
    assert!(
        ElementHandle::find_by_accessible_label(&settings, "Settings for No form")
            .next()
            .is_none()
    );
    button(&settings, "Settings for Form")?.mock_single_click(PointerEventButton::Left);
    assert_eq!(requested.get(), Some(1));
    settings.hide()?;
    Ok(())
}

#[test]
fn navigation_selects_and_reveals_settings_sections() -> Result<(), Box<dyn Error>> {
    i_slint_backend_testing::init_no_event_loop();
    let settings = PreferencesWindow::new()?;
    settings.set_disable_animations(true);
    settings.window().set_size(LogicalSize::new(750.0, 670.0));
    settings.show()?;
    super::flush();

    assert_eq!(settings.get_selected_pane(), 0);
    select(&settings, "History")?;
    assert_eq!(settings.get_selected_general_section(), 1);
    assert!(settings.get_general_scroll_position() < 0.0);

    select(&settings, "Interface")?;
    assert_eq!(settings.get_selected_pane(), 1);
    select(&settings, "Picture-in-Picture")?;
    assert_eq!(settings.get_selected_interface_section(), 5);
    assert!(settings.get_interface_scroll_position() < 0.0);

    select(&settings, "Audio")?;
    assert_eq!(settings.get_selected_pane(), 3);
    select(&settings, "Volume")?;
    assert_eq!(settings.get_selected_audio_section(), 1);
    assert!(settings.get_audio_scroll_position() < 0.0);

    select(&settings, "Subtitles")?;
    assert_eq!(settings.get_selected_pane(), 4);
    select(&settings, "Position")?;
    assert_eq!(settings.get_selected_subtitle_section(), 3);
    assert!(settings.get_subtitle_scroll_position() < 0.0);

    settings.hide()?;
    Ok(())
}

#[test]
fn video_form_changes_are_scoped_to_one_settings_window() -> Result<(), Box<dyn Error>> {
    i_slint_backend_testing::init_no_event_loop();
    let first = PreferencesWindow::new()?;
    let second = PreferencesWindow::new()?;

    let first_video = first.global::<VideoSettingsState>();
    first_video.set_tone_mapping_expanded(true);
    first_video.set_tone_mapping_enabled(false);
    first_video.set_video_scroll_position(-100.0);

    let second_video = second.global::<VideoSettingsState>();
    assert!(first_video.get_tone_mapping_expanded());
    assert!(!first_video.get_tone_mapping_enabled());
    assert!(!second_video.get_tone_mapping_expanded());
    assert!(second_video.get_tone_mapping_enabled());
    assert_eq!(second_video.get_video_scroll_position(), 0.0);
    Ok(())
}
