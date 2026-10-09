//! One owned picker per player; native HWNDs outlive its dedicated COM apartment.
use crate::{NativeVideoHost, PlayerWindow, file_dialog, media_open};
use riina_core::Command;
use riina_mpv::CommandSender;
use slint::{
    ComponentHandle,
    winit_030::{WinitWindowAccessor, winit},
};
use std::sync::Arc;
use winit::{platform::windows::WindowExtWindows, window::Window};

#[derive(Clone, Copy)]
pub enum Purpose {
    Media,
    Audio,
    Subtitle,
    Playlist,
}

impl Purpose {
    fn title(self) -> &'static str {
        match self {
            Self::Media => "Open File",
            Self::Audio => "Load External Audio",
            Self::Subtitle => "Load Subtitle",
            Self::Playlist => "Add to Playlist",
        }
    }

    fn command(self, paths: Vec<String>) -> Option<Command> {
        if paths.is_empty() {
            return None;
        }
        Some(match self {
            Self::Media => Command::OpenFiles(paths),
            Self::Audio => Command::AddAudio(paths[0].clone()),
            Self::Subtitle => Command::AddSubtitle(paths[0].clone()),
            Self::Playlist => Command::AppendFiles(paths),
        })
    }
}

struct ModalWindows {
    app: slint::Weak<PlayerWindow>,
    // The first HWND is the dialog owner (chrome); the optional second is its VO host.
    windows: Vec<Arc<Window>>,
}

impl ModalWindows {
    fn restore(&mut self) {
        if let Some(app) = self.app.upgrade() {
            app.set_dialog_open(false);
            app.invoke_activity();
        }
        for window in &self.windows {
            window.set_enable(true);
        }
        if let Some(owner) = self.windows.first() {
            owner.focus_window();
        }
        self.windows.clear();
    }
}

impl Drop for ModalWindows {
    fn drop(&mut self) {
        if self.windows.is_empty() {
            return;
        }
        // Startup/thread failures still restore modality on the GUI thread. No raw
        // HWND is captured without the Arc that owns it, and no fake Send is needed.
        let app = self.app.clone();
        let windows = std::mem::take(&mut self.windows);
        // Capture plain fields: a rejected dispatch must not recursively drop
        // another guard and try to enqueue itself on a stopped event loop.
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(app) = app.upgrade() {
                app.set_dialog_open(false);
                app.invoke_activity();
            }
            for window in &windows {
                window.set_enable(true);
            }
            if let Some(owner) = windows.first() {
                owner.focus_window();
            }
        });
    }
}

pub fn open(
    app: &PlayerWindow,
    host: Option<slint::Weak<NativeVideoHost>>,
    purpose: Purpose,
    sender: CommandSender,
) {
    // The gate closes before the async HWND lookup: queued shortcuts cannot open
    // a second picker while the first is still being constructed.
    if app.get_dialog_open() {
        return;
    }
    app.set_dialog_open(true);
    app.invoke_activity();
    let weak = app.as_weak();
    let failure = weak.clone();
    let started = slint::spawn_local(async move {
        let Some(app) = weak.upgrade() else { return };
        let result = async {
            let owner = app
                .window()
                .winit_window()
                .await
                .map_err(|e| e.to_string())?;
            let mut windows = vec![owner.clone()];
            if let Some(host) = host {
                let host = host.upgrade().ok_or("Native video host closed")?;
                windows.push(
                    host.window()
                        .winit_window()
                        .await
                        .map_err(|e| e.to_string())?,
                );
            }
            let mut modality = ModalWindows {
                app: weak.clone(),
                windows,
            };
            // The Shell dialog runs on a separate STA. Explicitly disable every
            // player HWND; cross-thread Show alone did not reliably block its owner.
            // Restore only after Show returns, then focus the live chrome again.
            for window in &modality.windows {
                window.set_enable(false);
            }
            let owner = file_dialog::DialogOwner::new(owner)?;
            std::thread::Builder::new()
                .name("file-picker".into())
                .spawn(move || {
                    let result = file_dialog::open_files(
                        &owner,
                        matches!(purpose, Purpose::Media | Purpose::Playlist),
                        purpose.title(),
                    );
                    let _ = slint::invoke_from_event_loop(move || {
                        modality.restore();
                        if let Some(app) = weak.upgrade() {
                            let result = result.and_then(|paths| {
                                if matches!(purpose, Purpose::Media | Purpose::Playlist) {
                                    media_open::selection(
                                        &app,
                                        paths,
                                        matches!(purpose, Purpose::Playlist),
                                        sender,
                                    );
                                    return Ok(());
                                }
                                purpose
                                    .command(paths)
                                    .map_or(Ok(()), |command| sender.send(command))
                            });
                            if let Err(error) = result {
                                app.set_status(error.into());
                            }
                        }
                    });
                })
                .map_err(|e| e.to_string())?;
            Ok::<(), String>(())
        }
        .await;
        if let Err(error) = result {
            app.set_dialog_open(false);
            app.set_status(error.into());
        }
    });
    if let Err(error) = started
        && let Some(app) = failure.upgrade()
    {
        app.set_dialog_open(false);
        app.set_status(error.to_string().into());
    }
}
