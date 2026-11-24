//! Route audio and subtitle delay editors through typed, lane-specific commands.
use crate::{DelayLane, PlayerWindow};
use riina_core::Command;
use riina_mpv::CommandSender;
use slint::ComponentHandle;

pub(super) fn install_sliders(app: &PlayerWindow, player: &CommandSender) {
    let delay_player = player.clone();
    app.global::<riina_ui::PlayerSidebarActions>()
        .on_format_delay_preview(|value| {
            crate::localized_number::format_delay(value.into()).into()
        });
    let weak = app.as_weak();
    app.global::<riina_ui::PlayerSidebarActions>()
        .on_set_delay(move |lane, value| {
            let value = f64::from(value);
            let command = match lane {
                DelayLane::Audio => Command::AudioDelay(value),
                DelayLane::PrimarySubtitle => Command::SubtitleDelay(value),
                DelayLane::SecondarySubtitle => Command::SecondarySubtitleDelay(value),
            };
            if let Some(app) = weak.upgrade() {
                app.global::<riina_ui::PlayerControlActions>()
                    .invoke_activity();
                if let Err(error) = delay_player.send(command) {
                    crate::diagnostics::report(&app, error);
                }
            }
        });
}

pub(super) fn install_text(app: &PlayerWindow, player: &CommandSender) {
    let text_delay_player = player.clone();
    let weak = app.as_weak();
    app.global::<riina_ui::PlayerSidebarActions>()
        .on_set_delay_text(move |lane, text| {
            if let Some(app) = weak.upgrade() {
                app.global::<riina_ui::PlayerControlActions>()
                    .invoke_activity();
                use riina_core::subtitle_controls::SubtitleLane;
                let normalized = match crate::localized_number::normalize_delay_entry(&text) {
                    Ok(normalized) => normalized,
                    Err(error) => {
                        app.set_status(error.into());
                        return false;
                    }
                };
                let command = match lane {
                    DelayLane::Audio => {
                        riina_core::timing_controls::parse_delay_seconds(&normalized)
                            .map(Command::AudioDelay)
                            .map_err(|error| error.to_string())
                    }
                    DelayLane::PrimarySubtitle => SubtitleLane::Primary
                        .delay_from_text(&normalized)
                        .map_err(|error| error.to_string()),
                    DelayLane::SecondarySubtitle => SubtitleLane::Secondary
                        .delay_from_text(&normalized)
                        .map_err(|error| error.to_string()),
                };
                if let Err(error) = command
                    .and_then(|command| text_delay_player.send(command).map_err(String::from))
                {
                    app.set_status(error.into());
                    return false;
                }
                return true;
            }
            false
        });
}
