use super::*;

#[test]
fn color_popup_focuses_the_first_channel_and_retains_live_edits_on_escape() {
    let app = player(2);
    let subtitles = app.global::<PlaybackSubtitleState>();
    subtitles.set_subtitle_text_color_known(true);
    subtitles.set_subtitle_text_color(slint::Color::from_argb_u8(120, 18, 52, 86));
    let colors = Rc::new(RefCell::new(Vec::new()));
    let observed = colors.clone();
    app.global::<PlayerSidebarActions>()
        .on_set_subtitle_color(move |target, color| observed.borrow_mut().push((target, color)));
    flush();

    let well = field(&app, "Subtitle text color");
    well.invoke_accessible_default_action();
    flush();
    field(&app, "Subtitle text color red");
    key(&app, Key::RightArrow);
    key(&app, Key::Tab);
    key(&app, Key::RightArrow);
    key(&app, Key::Backtab);
    key(&app, Key::LeftArrow);
    key(&app, Key::Escape);
    let expected = [(19, 52), (19, 53), (18, 53)].map(|(red, green)| {
        (
            SubtitleColorTarget::Text,
            slint::Color::from_argb_u8(120, red, green, 86),
        )
    });
    assert_eq!(colors.borrow().as_slice(), &expected);
    assert!(
        ElementHandle::find_by_accessible_label(&app, "Subtitle text color red")
            .next()
            .is_none()
    );
    key(&app, " ");
    field(&app, "Subtitle text color red");
    subtitles.set_subtitle_text_color_known(false);
    flush();
    assert!(
        ElementHandle::find_by_accessible_label(&app, "Subtitle text color red")
            .next()
            .is_none()
    );
    assert_eq!(colors.borrow().as_slice(), &expected);
}

#[test]
fn macos_subtitle_wells_request_the_shared_panel_with_the_observed_target_and_color() {
    let app = player(2);
    app.global::<PlayerAppearance>().set_platform_macos(true);
    let subtitles = app.global::<PlaybackSubtitleState>();
    let text = slint::Color::from_argb_u8(255, 20, 40, 60);
    let outline = slint::Color::from_argb_u8(90, 21, 41, 61);
    let background = slint::Color::from_argb_u8(40, 22, 42, 62);
    subtitles.set_subtitle_text_color(text);
    subtitles.set_subtitle_border_color(outline);
    subtitles.set_subtitle_shadow_color(background);
    subtitles.set_subtitle_text_color_known(true);
    subtitles.set_subtitle_border_color_known(true);
    subtitles.set_subtitle_shadow_color_known(true);
    let requests = Rc::new(RefCell::new(Vec::new()));
    let observed = requests.clone();
    app.global::<PlayerSidebarActions>()
        .on_request_native_subtitle_color(move |target, color| {
            observed.borrow_mut().push((target, color))
        });
    flush();

    for label in [
        "Subtitle text color",
        "Subtitle border color",
        "Subtitle shadow color",
    ] {
        field(&app, label).invoke_accessible_default_action();
        flush();
    }
    assert_eq!(
        requests.borrow().as_slice(),
        &[
            (SubtitleColorTarget::Text, text),
            (SubtitleColorTarget::Outline, outline),
            (SubtitleColorTarget::Background, background),
        ]
    );
    assert!(
        ElementHandle::find_by_accessible_label(&app, "Subtitle text color red")
            .next()
            .is_none()
    );
}
