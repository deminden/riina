//! Subtitle appearance and timing callbacks dispatch portable player intents.
use crate::PlayerWindow;
use riina_core::Command;
use riina_mpv::CommandSender;
use riina_ui::PlayerMenuActions;
use slint::ComponentHandle;

pub(super) fn install(app: &PlayerWindow, player: &CommandSender) {
    let menu = app.global::<PlayerMenuActions>();
    let scale_player = player.clone();
    let absolute_scale_player = player.clone();
    install_appearance(app, player);
    let weak = app.as_weak();
    menu.on_set_subtitle_scale_slider(move |value| {
        if let Some(app) = weak.upgrade() {
            app.global::<riina_ui::PlayerControlActions>()
                .invoke_activity();
            // Each side advances in 0.05 scale units; the negative side is reciprocal.
            let magnitude = ((1.0 + f64::from(value).abs()) * 20.0).round() / 20.0;
            let scale = if value < 0.0 {
                magnitude.recip()
            } else {
                magnitude
            };
            if let Err(error) = absolute_scale_player.send(Command::SubtitleScale(scale)) {
                app.set_status(error.into());
            }
        }
    });
    let weak = app.as_weak();
    app.global::<riina_ui::PlayerControlActions>()
        .on_adjust_subtitle_scale(move |delta| {
            if let Some(app) = weak.upgrade() {
                app.global::<riina_ui::PlayerControlActions>()
                    .invoke_activity();
            }
            if let Err(error) = scale_player.send(Command::AdjustSubtitleScale(delta as f64))
                && let Some(app) = weak.upgrade()
            {
                app.set_status(error.into());
            }
        });
    let multiply_scale_player = player.clone();
    let weak = app.as_weak();
    menu.on_multiply_subtitle_scale(move |factor| {
        if let Some(app) = weak.upgrade() {
            app.global::<riina_ui::PlayerControlActions>()
                .invoke_activity();
        }
        if let Err(error) =
            multiply_scale_player.send(Command::MultiplySubtitleScale(factor as f64))
            && let Some(app) = weak.upgrade()
        {
            app.set_status(error.into());
        }
    });
    let position_player = player.clone();
    let weak = app.as_weak();
    app.global::<riina_ui::PlayerControlActions>()
        .on_adjust_subtitle_position(move |delta| {
            if let Some(app) = weak.upgrade() {
                app.global::<riina_ui::PlayerControlActions>()
                    .invoke_activity();
            }
            if let Err(error) = position_player.send(Command::AdjustSubtitlePosition(delta as f64))
                && let Some(app) = weak.upgrade()
            {
                app.set_status(error.into());
            }
        });
    let audio_delay_player = player.clone();
    let weak = app.as_weak();
    menu.on_adjust_audio_delay(move |delta| {
        if let Some(app) = weak.upgrade() {
            app.global::<riina_ui::PlayerControlActions>()
                .invoke_activity();
        }
        if let Err(error) = audio_delay_player.send(Command::AdjustAudioDelay(delta as f64))
            && let Some(app) = weak.upgrade()
        {
            app.set_status(error.into());
        }
    });
    let delay_player = player.clone();
    super::delays::install_text(app, player);
    let secondary_position_player = player.clone();
    let weak = app.as_weak();
    app.global::<riina_ui::PlayerSidebarActions>()
        .on_set_subtitle_position(move |secondary, value| {
            if let Some(app) = weak.upgrade() {
                app.global::<riina_ui::PlayerControlActions>()
                    .invoke_activity();
                if let Err(error) = secondary_position_player.send(if secondary {
                    Command::SecondarySubtitlePosition(value as f64)
                } else {
                    Command::SubtitlePosition(value as f64)
                }) {
                    app.set_status(error.into());
                }
            }
        });
    let weak = app.as_weak();
    menu.on_adjust_subtitle_delay(move |delta| {
        if let Some(app) = weak.upgrade() {
            app.global::<riina_ui::PlayerControlActions>()
                .invoke_activity();
        }
        if let Err(error) = delay_player.send(Command::AdjustSubtitleDelay(delta as f64))
            && let Some(app) = weak.upgrade()
        {
            app.set_status(error.into());
        }
    });
}

fn install_appearance(app: &PlayerWindow, player: &CommandSender) {
    let font_size_player = player.clone();
    crate::font_picker::connect(app);
    let outline_size_player = player.clone();
    let color_player = player.clone();
    let weak = app.as_weak();
    app.global::<riina_ui::PlayerSidebarActions>()
        .on_set_subtitle_color(move |target, value| {
            if let Some(app) = weak.upgrade() {
                let rgba = [value.red(), value.green(), value.blue(), value.alpha()]
                    .map(|channel| f64::from(channel) / 255.0);
                let Ok(color) = riina_core::subtitle_color::SubtitleColor::from_rgba(rgba) else {
                    return;
                };
                let command = match target {
                    1 => Command::SubtitleTextColor(color),
                    2 => Command::SubtitleShadowColor(color),
                    3 => Command::SubtitleBorderColor(color),
                    _ => return,
                };
                app.global::<riina_ui::PlayerControlActions>()
                    .invoke_activity();
                if let Err(error) = color_player.send(command) {
                    app.set_status(error.into());
                } else {
                    match target {
                        1 => app
                            .global::<riina_ui::PlayerPreferences>()
                            .set_subtitle_text_preference_color(value),
                        2 => {
                            app.global::<riina_ui::PlayerPreferences>()
                                .set_subtitle_shadow_preference_color(value);
                            app.global::<riina_ui::PlayerPreferences>()
                                .set_subtitle_background_preference_color(value);
                        }
                        3 => app
                            .global::<riina_ui::PlayerPreferences>()
                            .set_subtitle_outline_preference_color(value),
                        _ => unreachable!("target was validated above"),
                    }
                }
            }
        });
    let weak = app.as_weak();
    app.global::<riina_ui::PlayerSidebarActions>()
        .on_set_subtitle_outline_size(move |value| {
            if let Some(app) = weak.upgrade() {
                app.global::<riina_ui::PlayerControlActions>()
                    .invoke_activity();
                if let Err(error) =
                    outline_size_player.send(Command::SubtitleOutlineSize(value as f64))
                {
                    app.set_status(error.into());
                } else {
                    app.global::<riina_ui::PlayerPreferences>()
                        .set_subtitle_outline_size_preference(value);
                }
            }
        });
    let weak = app.as_weak();
    app.global::<riina_ui::PlayerSidebarActions>()
        .on_set_subtitle_font_size(move |value| {
            if let Some(app) = weak.upgrade() {
                app.global::<riina_ui::PlayerControlActions>()
                    .invoke_activity();
                if let Err(error) = font_size_player.send(Command::SubtitleFontSize(value as f64)) {
                    app.set_status(error.into());
                } else {
                    app.global::<riina_ui::PlayerPreferences>()
                        .set_subtitle_font_size_preference(value);
                }
            }
        });
}

pub(super) fn install_encoding(app: &PlayerWindow, player: &CommandSender) {
    let subtitle_encoding_player = player.clone();
    let weak = app.as_weak();
    app.global::<PlayerMenuActions>()
        .on_set_subtitle_encoding(move |encoding| {
            let Some(app) = weak.upgrade() else { return };
            let encoding = encoding.to_string();
            if !riina_core::subtitle_controls::SUBTITLE_ENCODING_CHOICES
                .iter()
                .any(|(_, code)| *code == encoding.as_str())
            {
                app.set_status("Unsupported subtitle encoding".into());
                return;
            }
            app.global::<riina_ui::PlayerControlActions>()
                .invoke_activity();
            match subtitle_encoding_player.send(Command::SubtitleEncoding(encoding.clone())) {
                Ok(()) => app
                    .global::<riina_ui::PlayerPreferences>()
                    .set_subtitle_default_encoding(encoding.into()),
                Err(error) => app.set_status(error.into()),
            }
        });
}
