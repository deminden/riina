//! Apply live decoder preferences and undo accepted changes on failure.
use crate::PlayerWindow;
use riina_core::Command;

pub(super) fn apply_text_style(
    app: &PlayerWindow,
    player: &riina_mpv::CommandSender,
    previous: riina_core::subtitle_controls::TextStylePreferences,
    selected: riina_core::subtitle_controls::TextStylePreferences,
) -> Result<(), String> {
    selected.validate().map_err(str::to_owned)?;
    let previous_commands = crate::subtitle_preferences::commands(previous);
    let selected_commands = crate::subtitle_preferences::commands(selected);
    let mut applied = Vec::<usize>::new();
    for (index, (old, new)) in previous_commands
        .into_iter()
        .zip(selected_commands)
        .enumerate()
    {
        if old == new {
            continue;
        }
        if let Err(error) = player.send(new) {
            let rollback = crate::subtitle_preferences::commands(previous);
            for applied_index in applied.into_iter().rev() {
                let _ = player.send(rollback[applied_index].clone());
            }
            return Err(error.to_string());
        }
        applied.push(index);
    }
    crate::subtitle_preferences::install_style(app, selected);
    Ok(())
}

pub(super) fn restore_text_style(
    app: &PlayerWindow,
    player: &riina_mpv::CommandSender,
    previous: riina_core::subtitle_controls::TextStylePreferences,
    selected: riina_core::subtitle_controls::TextStylePreferences,
) {
    if previous != selected {
        for command in crate::subtitle_preferences::commands(previous) {
            let _ = player.send(command);
        }
    }
    crate::subtitle_preferences::install_style(app, previous);
}

pub(super) fn apply_subtitle_text_preferences(
    app: &PlayerWindow,
    player: &riina_mpv::CommandSender,
    previous_font: &str,
    previous_size: f64,
    selected_font: &str,
    selected_size: f64,
) -> Result<(String, f64), String> {
    let font_command =
        riina_core::subtitle_controls::font_from_text(selected_font).map_err(str::to_owned)?;
    let font = match font_command {
        Command::SubtitleFont(value) => value,
        _ => return Err("Invalid subtitle font".into()),
    };
    if (selected_size - previous_size).abs() > f64::EPSILON {
        player
            .send(Command::SubtitleFontSize(selected_size))
            .map_err(|error| error.to_string())?;
    }
    if font != previous_font
        && let Err(error) = player.send(Command::SubtitleFont(font.clone()))
    {
        if (selected_size - previous_size).abs() > f64::EPSILON {
            let _ = player.send(Command::SubtitleFontSize(previous_size));
        }
        return Err(error.to_string());
    }
    app.set_subtitle_font(font.clone().into());
    app.set_subtitle_font_size_preference(selected_size as f32);
    Ok((font, selected_size))
}

pub(super) fn restore_subtitle_text_preferences(
    app: &PlayerWindow,
    player: &riina_mpv::CommandSender,
    previous_font: &str,
    previous_size: f64,
    selected_font: &str,
    selected_size: f64,
) {
    if selected_size != previous_size {
        let _ = player.send(Command::SubtitleFontSize(previous_size));
    }
    if selected_font != previous_font {
        let _ = player.send(Command::SubtitleFont(previous_font.to_owned()));
    }
    app.set_subtitle_font(previous_font.into());
    app.set_subtitle_font_size_preference(previous_size as f32);
}

pub(super) fn apply_ass_override(
    app: &PlayerWindow,
    player: &riina_mpv::CommandSender,
    previous: (i32, i32),
    selected: (i32, i32),
) -> Result<
    (
        riina_core::subtitle_controls::AssOverrideLevel,
        riina_core::subtitle_controls::AssOverrideLevel,
    ),
    String,
> {
    use riina_core::subtitle_controls::{AssOverrideLevel, SubtitleLane};

    let primary = AssOverrideLevel::from_slider_index(selected.0)
        .ok_or_else(|| "Invalid primary ASS override selection".to_owned())?;
    let secondary = AssOverrideLevel::from_slider_index(selected.1)
        .ok_or_else(|| "Invalid secondary ASS override selection".to_owned())?;
    if selected.0 != previous.0 {
        player
            .send(Command::SubtitleAssOverride {
                lane: SubtitleLane::Primary,
                level: primary,
            })
            .map_err(|error| error.to_string())?;
    }
    if selected.1 != previous.1
        && let Err(error) = player.send(Command::SubtitleAssOverride {
            lane: SubtitleLane::Secondary,
            level: secondary,
        })
    {
        if selected.0 != previous.0
            && let Some(previous_level) = AssOverrideLevel::from_slider_index(previous.0)
        {
            let _ = player.send(Command::SubtitleAssOverride {
                lane: SubtitleLane::Primary,
                level: previous_level,
            });
        }
        return Err(error.to_string());
    }
    app.set_subtitle_ass_override_index(selected.0);
    app.set_secondary_subtitle_ass_override_index(selected.1);
    Ok((primary, secondary))
}

pub(super) fn restore_ass_override(
    app: &PlayerWindow,
    player: &riina_mpv::CommandSender,
    previous: (i32, i32),
    selected: (i32, i32),
) {
    use riina_core::subtitle_controls::{AssOverrideLevel, SubtitleLane};

    if selected.0 != previous.0
        && let Some(level) = AssOverrideLevel::from_slider_index(previous.0)
    {
        let _ = player.send(Command::SubtitleAssOverride {
            lane: SubtitleLane::Primary,
            level,
        });
    }
    if selected.1 != previous.1
        && let Some(level) = AssOverrideLevel::from_slider_index(previous.1)
    {
        let _ = player.send(Command::SubtitleAssOverride {
            lane: SubtitleLane::Secondary,
            level,
        });
    }
    app.set_subtitle_ass_override_index(previous.0);
    app.set_secondary_subtitle_ass_override_index(previous.1);
}

pub(super) fn restore_live_volume_and_decoder(
    app: &PlayerWindow,
    player: &riina_mpv::CommandSender,
    volume_boost_applied: bool,
    previous_volume_boost: bool,
    previous_volume: f32,
    decoder_applied: bool,
    previous_decoder: i32,
) {
    if volume_boost_applied {
        let _ = player.send(Command::SetVolumeBoost(previous_volume_boost));
        if previous_volume != app.get_volume() {
            let _ = player.send(Command::Volume(previous_volume as f64));
        }
        app.set_volume_boost(previous_volume_boost);
        app.set_volume(previous_volume);
    }
    if decoder_applied {
        if let Some(previous_mode) = crate::codec_preferences::hardware_decoder(previous_decoder) {
            let _ = player.send(Command::HardwareDecoding(previous_mode));
        }
        app.set_hardware_decoder_index(previous_decoder);
    }
}
