//! Exercise Settings routes through the generated accessibility actions.
use i_slint_backend_testing::{AccessibleRole, ElementHandle};
use riina_ui::{PreferencesWindow, VideoSettingsState};
use slint::{ComponentHandle, LogicalSize};
use std::error::Error;

fn select(settings: &PreferencesWindow, label: &str) -> Result<(), Box<dyn Error>> {
    let mut candidates = ElementHandle::find_by_accessible_label(settings, label)
        .filter(|element| element.accessible_role() == Some(AccessibleRole::Button));
    let button = candidates
        .next()
        .ok_or_else(|| format!("Missing Settings route {label}"))?;
    assert!(
        candidates.next().is_none(),
        "Multiple Settings routes for {label}"
    );
    button.invoke_accessible_default_action();
    super::flush();
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
