//! Video geometry, decoder and equalizer actions use validated typed intents.
use crate::PlayerWindow;
use riina_core::Command;
use riina_mpv::CommandSender;
use riina_ui::PlayerMenuActions;
use slint::ComponentHandle;

pub(super) fn install(app: &PlayerWindow, player: &CommandSender) {
    install_aspect_and_crop(app, player);
    install_rotation_and_picture(app, player);
}

fn install_aspect_and_crop(app: &PlayerWindow, player: &CommandSender) {
    let aspect_player = player.clone();
    let weak = app.as_weak();
    app.global::<riina_ui::PlayerSidebarActions>()
        .on_set_video_aspect(move |text| {
            if text.is_empty() {
                return;
            }
            let aspect = riina_core::video_controls::AspectRatio::from_text(text.as_str());
            if let Some(app) = weak.upgrade() {
                app.global::<riina_ui::PlayerControlActions>()
                    .invoke_activity();
                if let Err(error) = aspect_player.send(Command::VideoAspect(aspect)) {
                    crate::diagnostics::report(&app, error);
                }
            }
        });
    let weak = app.as_weak();
    app.global::<PlayerMenuActions>()
        .on_set_menu_video_aspect(move |index| {
            let Some(app) = weak.upgrade() else { return };
            refresh_video_menu(&app);
            if !app.get_loaded()
                || !app
                    .global::<riina_ui::PlaybackVideoState>()
                    .get_video_aspect_known()
                || app.global::<riina_ui::PlayerCropState>().get_crop_editing()
                || app
                    .global::<riina_ui::PlayerDialogState>()
                    .get_dialog_open()
            {
                return;
            }
            if let Some(label) = usize::try_from(index)
                .ok()
                .and_then(|index| riina_core::video_controls::ASPECT_MENU_PRESETS.get(index))
            {
                app.global::<riina_ui::PlayerSidebarActions>()
                    .invoke_set_video_aspect((*label).into());
            }
        });
    let crop_player = player.clone();
    let crop_ui = app.as_weak();
    app.global::<riina_ui::PlayerSidebarActions>()
        .on_set_video_crop(move |index| {
            if index == 6 {
                if let Some(app) = crop_ui.upgrade() {
                    app.invoke_begin_custom_crop();
                }
                return;
            }
            let preset = if index == 0 {
                None
            } else if let Some(preset) = usize::try_from(index - 1)
                .ok()
                .and_then(|index| riina_core::crop::Preset::ALL.get(index))
                .copied()
            {
                Some(preset)
            } else {
                return;
            };
            if let Some(app) = crop_ui.upgrade() {
                app.global::<riina_ui::PlayerControlActions>()
                    .invoke_activity();
                if let Err(error) = crop_player.send(Command::VideoCrop(preset)) {
                    crate::diagnostics::report(&app, error);
                }
            }
        });
    let delogo_player = player.clone();
    let weak = app.as_weak();
    app.global::<PlayerMenuActions>().on_toggle_delogo(move || {
        let Some(app) = weak.upgrade() else { return };
        if !app.get_delogo_enabled()
            || app.global::<riina_ui::PlayerCropState>().get_crop_editing()
            || app
                .global::<riina_ui::PlayerDialogState>()
                .get_dialog_open()
        {
            return;
        }
        app.global::<riina_ui::PlayerControlActions>()
            .invoke_activity();
        if app.get_delogo_active() {
            if let Err(error) = delogo_player.send(Command::RemoveDelogo) {
                crate::diagnostics::report(&app, error);
            }
        } else {
            app.invoke_begin_delogo();
        }
    });
    let menu_crop_player = player.clone();
    let weak = app.as_weak();
    app.global::<PlayerMenuActions>()
        .on_set_menu_video_crop(move |index| {
            let Some(app) = weak.upgrade() else { return };
            refresh_video_menu(&app);
            if !app.get_loaded()
                || !app
                    .global::<riina_ui::PlaybackVideoState>()
                    .get_video_crop_known()
                || app.global::<riina_ui::PlayerCropState>().get_crop_editing()
                || app
                    .global::<riina_ui::PlayerDialogState>()
                    .get_dialog_open()
            {
                return;
            }
            if index == riina_core::crop::MENU_CUSTOM_INDEX as i32 {
                app.invoke_begin_custom_crop();
                return;
            }
            let preset = if index == 0 {
                None
            } else if let Some(preset) = usize::try_from(index - 1)
                .ok()
                .and_then(|index| riina_core::crop::Preset::MENU_ALL.get(index))
                .copied()
            {
                Some(preset)
            } else {
                return;
            };
            app.global::<riina_ui::PlayerControlActions>()
                .invoke_activity();
            if let Err(error) = menu_crop_player.send(Command::VideoCrop(preset)) {
                crate::diagnostics::report(&app, error);
            }
        });
}

fn install_rotation_and_picture(app: &PlayerWindow, player: &CommandSender) {
    let rotation_player = player.clone();
    let weak = app.as_weak();
    app.global::<riina_ui::PlayerSidebarActions>()
        .on_set_video_rotation(move |degrees| {
            if let Some(app) = weak.upgrade()
                && let Some(rotation) =
                    riina_core::video_controls::Rotation::from_degrees(degrees.into())
            {
                app.global::<riina_ui::PlayerControlActions>()
                    .invoke_activity();
                if let Err(error) = rotation_player.send(Command::VideoRotation(rotation)) {
                    crate::diagnostics::report(&app, error);
                }
            }
        });
    let menu_rotation_player = player.clone();
    let weak = app.as_weak();
    app.global::<PlayerMenuActions>()
        .on_set_menu_video_rotation(move |degrees| {
            let Some(app) = weak.upgrade() else { return };
            refresh_video_menu(&app);
            if !app.get_loaded()
                || !app
                    .global::<riina_ui::PlaybackVideoState>()
                    .get_video_rotation_known()
                || app.global::<riina_ui::PlayerCropState>().get_crop_editing()
                || app
                    .global::<riina_ui::PlayerDialogState>()
                    .get_dialog_open()
            {
                return;
            }
            let Some(rotation) = riina_core::video_controls::Rotation::from_degrees(degrees.into())
            else {
                return;
            };
            app.global::<riina_ui::PlayerControlActions>()
                .invoke_activity();
            if let Err(error) = menu_rotation_player.send(Command::VideoRotation(rotation)) {
                crate::diagnostics::report(&app, error);
            }
        });
    let hardware_player = player.clone();
    let weak = app.as_weak();
    app.global::<riina_ui::PlayerSidebarActions>()
        .on_set_hardware_decoding(move |enabled| {
            if let Some(app) = weak.upgrade() {
                app.global::<riina_ui::PlayerControlActions>()
                    .invoke_activity();
                let mode = if enabled {
                    crate::codec_preferences::hardware_decoder(
                        app.global::<riina_ui::PlayerPreferences>()
                            .get_hardware_decoder_index(),
                    )
                    .unwrap_or(riina_core::video_controls::HardwareDecoder::Auto)
                } else {
                    riina_core::video_controls::HardwareDecoder::Disabled
                };
                if let Err(error) = hardware_player.send(Command::HardwareDecoding(mode)) {
                    crate::diagnostics::report(&app, error);
                }
            }
        });
    let equalizer_player = player.clone();
    let weak = app.as_weak();
    app.global::<riina_ui::PlayerSidebarActions>()
        .on_set_video_equalizer(move |index, value| {
            let option = usize::try_from(index).ok().and_then(|index| {
                riina_core::video_controls::Equalizer::ALL
                    .get(index)
                    .copied()
            });
            if let Some(option) = option
                && value.is_finite()
                && (-100.0..=100.0).contains(&value)
                && let Some(app) = weak.upgrade()
            {
                app.global::<riina_ui::PlayerControlActions>()
                    .invoke_activity();
                // NSSlider's integer action uses whole units. The backend validates
                // the range again so every caller obeys the same portable contract.
                if let Err(error) = equalizer_player.send(Command::VideoEqualizer {
                    option,
                    value: value as i64,
                }) {
                    crate::diagnostics::report(&app, error);
                }
            }
        });
}

pub(super) fn refresh_video_menu(app: &PlayerWindow) {
    // Native check items toggle before dispatch. A no-op or rejected command
    // cannot dirty the observed selection, so renew the model identity to make
    // Slint rebuild the menu from confirmed state rather than that local toggle.
    let video = app.global::<riina_ui::PlaybackVideoState>();
    video.set_video_crop_menu_labels(slint::ModelRc::new(slint::VecModel::from(
        riina_core::crop::MENU_LABELS
            .map(slint::SharedString::from)
            .to_vec(),
    )));
    video.set_video_aspect_menu_labels(slint::ModelRc::new(slint::VecModel::from(
        riina_core::video_controls::ASPECT_MENU_PRESETS
            .map(slint::SharedString::from)
            .to_vec(),
    )));
}

pub(super) fn install_reflection(app: &PlayerWindow, player: &CommandSender) {
    let reflection_player = player.clone();
    let weak = app.as_weak();
    app.global::<PlayerMenuActions>()
        .on_toggle_video_reflection(move |horizontal| {
            if let Some(app) = weak.upgrade() {
                refresh_video_menu(&app);
                if !app.get_loaded()
                    || !app.get_reflection_known()
                    || app.global::<riina_ui::PlayerCropState>().get_crop_editing()
                    || app
                        .global::<riina_ui::PlayerDialogState>()
                        .get_dialog_open()
                {
                    return;
                }
                let axis = if horizontal {
                    riina_core::video_reflection::Axis::Horizontal
                } else {
                    riina_core::video_reflection::Axis::Vertical
                };
                if let Err(error) = reflection_player.send(Command::ToggleVideoReflection(axis)) {
                    crate::diagnostics::report(&app, error);
                }
            }
        });
}

pub(super) fn install_deinterlace(app: &PlayerWindow, player: &CommandSender) {
    let deinterlace_player = player.clone();
    let weak = app.as_weak();
    app.global::<PlayerMenuActions>()
        .on_toggle_deinterlace(move || {
            let Some(app) = weak.upgrade() else { return };
            refresh_video_menu(&app);
            if !app.get_loaded()
                || !app.get_deinterlace_known()
                || app.global::<riina_ui::PlayerCropState>().get_crop_editing()
                || app
                    .global::<riina_ui::PlayerDialogState>()
                    .get_dialog_open()
            {
                return;
            }
            app.global::<riina_ui::PlayerControlActions>()
                .invoke_activity();
            if let Err(error) = deinterlace_player.send(Command::ToggleDeinterlace) {
                crate::diagnostics::report(&app, error);
            }
        });
}
