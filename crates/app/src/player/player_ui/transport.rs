use super::{PlayerContext, bind};
use crate::PlayerWindow;
use riina_core::Command;
use riina_mpv::CommandSender;
use riina_ui::PlayerMenuActions;
use slint::ComponentHandle;

pub(super) fn install_seek(app: &PlayerWindow, command_sender: &CommandSender) {
    for absolute in [true, false] {
        let player = command_sender.clone();
        let weak = app.as_weak();
        let seek = move |seconds: f32| {
            if let Some(app) = weak.upgrade() {
                app.global::<riina_ui::PlayerControlActions>()
                    .invoke_activity();
            }
            if let Err(error) = player.send(Command::Seek {
                seconds: seconds as f64,
                absolute,
            }) && let Some(app) = weak.upgrade()
            {
                app.set_status(error.into());
            }
        };
        if absolute {
            app.global::<PlayerMenuActions>().on_seek(seek);
        } else {
            app.global::<PlayerMenuActions>().on_seek_relative(seek);
        }
    }
}

pub(super) fn install_relative_speed(app: &PlayerWindow, command_sender: &CommandSender) {
    let speed_player = command_sender.clone();
    let weak = app.as_weak();
    app.global::<PlayerMenuActions>().on_speed(move |factor| {
        if let Some(app) = weak.upgrade() {
            app.global::<riina_ui::PlayerControlActions>()
                .invoke_activity();
        }
        if let Err(error) = speed_player.send(Command::MultiplySpeed(factor as f64))
            && let Some(app) = weak.upgrade()
        {
            app.set_status(error.into());
        }
    });
}

pub(super) fn install_pause(app: &PlayerWindow, context: &PlayerContext) {
    // mpv owns the latest pause state, including transitions the UI has not observed yet.
    bind!(
        context,
        app.global::<PlayerMenuActions>(),
        on_toggle_pause,
        |ctx| {
            let Some(app) = ctx.app() else { return };
            app.global::<riina_ui::PlayerControlActions>()
                .invoke_activity();
            if let Err(error) = ctx.commands().toggle_pause() {
                ctx.report(error);
            }
        }
    );
    bind!(
        context,
        app.global::<riina_ui::PlayerControlActions>(),
        on_set_paused,
        |ctx, paused| {
            let Some(app) = ctx.app() else { return };
            app.global::<riina_ui::PlayerControlActions>()
                .invoke_activity();
            ctx.send(Command::SetPaused(paused));
        }
    );
}

pub(super) fn install_stop_and_steps(app: &PlayerWindow, context: &PlayerContext) {
    bind!(context, app.global::<PlayerMenuActions>(), on_stop, |ctx| {
        if let Some(app) = ctx.app() {
            app.global::<riina_ui::PlayerControlActions>()
                .invoke_activity();
            crate::media_open::invalidate(&app);
        }
        ctx.send_with_osd(Command::Stop, crate::video_osd::ActionFeedback::Stop, true);
    });
    bind!(
        context,
        app.global::<PlayerMenuActions>(),
        on_frame_step,
        Command::FrameStep
    );
    bind!(
        context,
        app.global::<PlayerMenuActions>(),
        on_frame_back_step,
        Command::FrameBackStep
    );
    bind!(
        context,
        app.global::<PlayerMenuActions>(),
        on_adjust_chapter,
        |ctx, direction| {
            ctx.activity();
            ctx.send(Command::AdjustChapter(direction));
        }
    );
    bind!(
        context,
        app.global::<riina_ui::PlayerControlActions>(),
        on_subtitle_seek,
        |ctx, direction| {
            ctx.activity();
            ctx.send(Command::SubtitleSeek(direction));
        }
    );
}

pub(super) fn install_speed(app: &PlayerWindow, context: &PlayerContext) {
    bind!(
        context,
        app.global::<PlayerMenuActions>(),
        on_reset_speed,
        Command::Speed(1.0)
    );
    bind!(
        context,
        app.global::<riina_ui::PlayerSidebarActions>(),
        on_set_speed_slider,
        |ctx, position| {
            if let Some(app) = ctx.app() {
                app.global::<riina_ui::PlayerControlActions>()
                    .invoke_activity();
                match riina_core::speed_controls::from_slider(position.into()) {
                    Ok(command) => ctx.send(command),
                    Err(error) => ctx.report(error.to_owned()),
                }
            }
        }
    );
    bind!(
        context,
        app.global::<riina_ui::PlayerSidebarActions>(),
        on_set_speed_text,
        |ctx, text| {
            let result = riina_core::speed_controls::from_text(text.as_str())
                .map_err(str::to_owned)
                .and_then(|command| ctx.commands().send(command));
            if let Some(app) = ctx.app() {
                app.global::<riina_ui::PlayerControlActions>()
                    .invoke_activity();
                if let Err(error) = result {
                    ctx.report(error);
                    return false;
                }
            }
            true
        }
    );
}

pub(super) fn install_loops(app: &PlayerWindow, context: &PlayerContext) {
    bind!(
        context,
        app.global::<PlayerMenuActions>(),
        on_cycle_ab_loop,
        Command::CycleAbLoop
    );
    bind!(
        context,
        app.global::<riina_ui::PlayerControlActions>(),
        on_set_ab_loop_point,
        |ctx, point, value| {
            let Some(app) = ctx.app() else { return };
            if !app.get_loaded() || !value.is_finite() {
                return;
            }
            let point = match point {
                0 => riina_core::AbLoopPoint::A,
                1 => riina_core::AbLoopPoint::B,
                _ => return,
            };
            app.global::<riina_ui::PlayerControlActions>()
                .invoke_activity();
            ctx.send(Command::SetAbLoopPoint {
                point,
                value: Some(f64::from(value)),
            });
        }
    );
    bind!(
        context,
        app.global::<PlayerMenuActions>(),
        on_toggle_file_loop,
        Command::ToggleFileLoop
    );
}
