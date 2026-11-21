use crate::PlayerWindow;
use riina_core::Command;
use riina_mpv::CommandSender;
use slint::ComponentHandle;

pub(super) fn install_seek(app: &PlayerWindow, command_sender: &CommandSender) {
    for absolute in [true, false] {
        let player = command_sender.clone();
        let weak = app.as_weak();
        let seek = move |seconds: f32| {
            if let Some(app) = weak.upgrade() {
                app.invoke_activity();
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
            app.on_seek(seek);
        } else {
            app.on_seek_relative(seek);
        }
    }
}

pub(super) fn install_relative_speed(app: &PlayerWindow, command_sender: &CommandSender) {
    let speed_player = command_sender.clone();
    let weak = app.as_weak();
    app.on_speed(move |factor| {
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
        }
        if let Err(error) = speed_player.send(Command::MultiplySpeed(factor as f64))
            && let Some(app) = weak.upgrade()
        {
            app.set_status(error.into());
        }
    });
}
