//! Video geometry, decoder and equalizer actions use validated typed intents.
use super::refresh_video_menu;
use crate::PlayerWindow;
use riina_core::Command;
use riina_mpv::CommandSender;
use slint::ComponentHandle;

pub(super) fn install(app: &PlayerWindow, player: &CommandSender) {
    let aspect_player = player.clone();
    let weak = app.as_weak();
    app.on_set_video_aspect(move |text| {
        if text.is_empty() {
            return;
        }
        let aspect = riina_core::video_controls::AspectRatio::from_text(text.as_str());
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
            if let Err(error) = aspect_player.send(Command::VideoAspect(aspect)) {
                app.set_status(error.into());
            }
        }
    });
    let rotation_player = player.clone();
    let weak = app.as_weak();
    app.on_set_menu_video_aspect(move |index| {
        let Some(app) = weak.upgrade() else { return };
        refresh_video_menu(&app);
        if !app.get_loaded()
            || !app.get_video_aspect_known()
            || app.get_crop_editing()
            || app.get_dialog_open()
        {
            return;
        }
        if let Some(label) = usize::try_from(index)
            .ok()
            .and_then(|index| riina_core::video_controls::ASPECT_MENU_PRESETS.get(index))
        {
            app.invoke_set_video_aspect((*label).into());
        }
    });
    let crop_player = player.clone();
    let crop_ui = app.as_weak();
    app.on_set_video_crop(move |index| {
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
            app.invoke_activity();
            if let Err(error) = crop_player.send(Command::VideoCrop(preset)) {
                app.set_status(error.into());
            }
        }
    });
    let delogo_player = player.clone();
    let weak = app.as_weak();
    app.on_toggle_delogo(move || {
        let Some(app) = weak.upgrade() else { return };
        if !app.get_delogo_enabled() || app.get_crop_editing() || app.get_dialog_open() {
            return;
        }
        app.invoke_activity();
        if app.get_delogo_active() {
            if let Err(error) = delogo_player.send(Command::RemoveDelogo) {
                app.set_status(error.into());
            }
        } else {
            app.invoke_begin_delogo();
        }
    });
    let menu_crop_player = player.clone();
    let weak = app.as_weak();
    app.on_set_menu_video_crop(move |index| {
        let Some(app) = weak.upgrade() else { return };
        refresh_video_menu(&app);
        if !app.get_loaded()
            || !app.get_video_crop_known()
            || app.get_crop_editing()
            || app.get_dialog_open()
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
        app.invoke_activity();
        if let Err(error) = menu_crop_player.send(Command::VideoCrop(preset)) {
            app.set_status(error.into());
        }
    });
    let weak = app.as_weak();
    app.on_set_video_rotation(move |degrees| {
        if let Some(app) = weak.upgrade()
            && let Some(rotation) =
                riina_core::video_controls::Rotation::from_degrees(degrees.into())
        {
            app.invoke_activity();
            if let Err(error) = rotation_player.send(Command::VideoRotation(rotation)) {
                app.set_status(error.into());
            }
        }
    });
    let menu_rotation_player = player.clone();
    let weak = app.as_weak();
    app.on_set_menu_video_rotation(move |degrees| {
        let Some(app) = weak.upgrade() else { return };
        refresh_video_menu(&app);
        if !app.get_loaded()
            || !app.get_video_rotation_known()
            || app.get_crop_editing()
            || app.get_dialog_open()
        {
            return;
        }
        let Some(rotation) = riina_core::video_controls::Rotation::from_degrees(degrees.into())
        else {
            return;
        };
        app.invoke_activity();
        if let Err(error) = menu_rotation_player.send(Command::VideoRotation(rotation)) {
            app.set_status(error.into());
        }
    });
    let hardware_player = player.clone();
    let weak = app.as_weak();
    app.on_set_hardware_decoding(move |enabled| {
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
            let mode = if enabled {
                crate::codec_preferences::hardware_decoder(app.get_hardware_decoder_index())
                    .unwrap_or(riina_core::video_controls::HardwareDecoder::Auto)
            } else {
                riina_core::video_controls::HardwareDecoder::Disabled
            };
            if let Err(error) = hardware_player.send(Command::HardwareDecoding(mode)) {
                app.set_status(error.into());
            }
        }
    });
    let equalizer_player = player.clone();
    let weak = app.as_weak();
    app.on_set_video_equalizer(move |index, value| {
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
            app.invoke_activity();
            // NSSlider's integer action uses whole units. The backend validates
            // the range again so every caller obeys the same portable contract.
            if let Err(error) = equalizer_player.send(Command::VideoEqualizer {
                option,
                value: value as i64,
            }) {
                app.set_status(error.into());
            }
        }
    });
}
