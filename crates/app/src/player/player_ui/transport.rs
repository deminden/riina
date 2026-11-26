use super::{PlayerContext, bind, playlist_subtitles::Assignments};
use crate::PlayerWindow;
use riina_core::Command;
use riina_mpv::CommandSender;
use riina_ui::{PlayerControlActions, PlayerMenuActions};
use slint::ComponentHandle;
use std::{cell::Cell, rc::Rc};

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
                crate::diagnostics::report(&app, error);
            }
        };
        if absolute {
            app.global::<PlayerMenuActions>().on_seek(seek);
        } else {
            app.global::<PlayerMenuActions>().on_seek_relative(seek);
        }
    }
}

pub(super) fn install_timeline(app: &PlayerWindow, context: &PlayerContext) {
    let actions = app.global::<PlayerControlActions>();
    let grabbed = Rc::new(Cell::new(None::<u64>));
    let press = grabbed.clone();
    bind!(context, actions, on_timeline_gesture, |ctx, active| {
        press.set(active.then(|| riina_core::sync::lock(ctx.playback()).generation));
    });

    let timeline = grabbed.clone();
    bind!(context, actions, on_seek_timeline, |ctx, fraction| {
        if fraction.is_finite() {
            send_timeline(
                &ctx,
                timeline.get(),
                Command::SeekTimeline(f64::from(fraction)),
            );
        }
    });
    bind!(
        context,
        actions,
        on_set_ab_loop_point,
        |ctx, point, value| {
            let point = match point {
                0 => riina_core::AbLoopPoint::A,
                1 => riina_core::AbLoopPoint::B,
                _ => return,
            };
            if value.is_finite() {
                send_timeline(
                    &ctx,
                    grabbed.get(),
                    Command::SetAbLoopPoint {
                        point,
                        value: Some(f64::from(value)),
                    },
                );
            }
        }
    );
}

fn send_timeline(ctx: &PlayerContext, grabbed: Option<u64>, command: Command) {
    let Some(app) = ctx.app() else { return };
    if !app.get_loaded() {
        return;
    }
    let current = riina_core::sync::lock(ctx.playback()).generation;
    let generation = grabbed.unwrap_or(current);
    if generation != current {
        return;
    }
    ctx.activity();
    if let Err(error) = ctx.commands().send_for_media(generation, command) {
        ctx.report(error);
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
            crate::diagnostics::report(&app, error);
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

pub(super) fn install_stop_and_steps(
    app: &PlayerWindow,
    context: &PlayerContext,
    subtitle_assignments: Assignments,
) {
    bind!(context, app.global::<PlayerMenuActions>(), on_stop, |ctx| {
        if let Some(app) = ctx.app() {
            app.global::<riina_ui::PlayerControlActions>()
                .invoke_activity();
            crate::media_open::invalidate(&app);
        }
        subtitle_assignments.reset();
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
                    Err(error) => ctx.report(error.to_string()),
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
                .map_err(|error| error.to_string())
                .and_then(|command| ctx.commands().send(command).map_err(String::from));
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
        app.global::<PlayerMenuActions>(),
        on_toggle_file_loop,
        Command::ToggleFileLoop
    );
    bind!(
        context,
        app.global::<PlayerMenuActions>(),
        on_toggle_file_repeat,
        Command::ToggleLoopMode(riina_core::LoopMode::File)
    );
    bind!(
        context,
        app.global::<PlayerMenuActions>(),
        on_toggle_playlist_repeat,
        Command::ToggleLoopMode(riina_core::LoopMode::Playlist)
    );
}
