//! AppKit owns the player window while AVKit owns the floating surface.
use super::view::content_view;
use crate::PlayerWindow;
use block2::RcBlock;
use objc2::{MainThreadMarker, rc::Retained, runtime::ProtocolObject};
use objc2_app_kit::{
    NSWindow, NSWindowDidDeminiaturizeNotification, NSWindowDidMiniaturizeNotification,
    NSWindowOcclusionState,
};
use objc2_foundation::{NSNotification, NSNotificationCenter, NSObjectProtocol, NSOperationQueue};
use riina_core::{Command, pip_lifecycle, pip_preferences::WindowBehavior};
use riina_mpv::CommandSender;
use slint::{ComponentHandle, winit_030::WinitWindowAccessor};
use std::{cell::Cell, ptr::NonNull, rc::Rc, time::Duration};

pub(super) struct WindowPolicy {
    player: slint::Weak<PlayerWindow>,
    action: Cell<WindowBehavior>,
    closing: Cell<bool>,
}

impl WindowPolicy {
    pub(super) fn new(player: slint::Weak<PlayerWindow>) -> Self {
        Self {
            player,
            action: Cell::new(WindowBehavior::DoNothing),
            closing: Cell::new(false),
        }
    }

    pub(super) fn entered(&self, commands: &CommandSender) {
        let Some(player) = self.player.upgrade().filter(|_| !self.closing.get()) else {
            return;
        };
        let Some(window) = player_window(&player) else {
            return;
        };
        let Ok(settings) = crate::pip_preferences::current_player(&player) else {
            return;
        };
        let (_, entry) = pip_lifecycle::Session::enter(
            settings,
            pip_lifecycle::MainWindowState {
                fullscreen: player.get_fullscreen() || player.get_entering_fullscreen(),
                minimized: window.isMiniaturized(),
            },
        );
        if entry.pause_playback
            && let Err(error) = commands.send(Command::SetPaused(true))
        {
            crate::diagnostics::report(
                &player,
                format!("Could not pause for Picture-in-Picture: {error}"),
            );
        }
        if window.isVisible() {
            self.action.set(entry.window_behavior);
            match entry.window_behavior {
                WindowBehavior::DoNothing => {}
                // Both operations preserve the Slint window and its render context.
                WindowBehavior::Hide => window.orderOut(None),
                WindowBehavior::Minimize => window.miniaturize(None),
            }
        }
    }

    pub(super) fn restore(&self, requested_by_system: bool) -> bool {
        let Some(player) = self.player.upgrade().filter(|_| !self.closing.get()) else {
            return false;
        };
        let Some(window) = player_window(&player) else {
            return false;
        };
        let action = self.action.replace(WindowBehavior::DoNothing);
        if requested_by_system || action == WindowBehavior::Minimize {
            window.deminiaturize(None);
        }
        if requested_by_system || action != WindowBehavior::DoNothing {
            window.makeKeyAndOrderFront(None);
            player.window().request_redraw();
        }
        true
    }

    pub(super) fn owns_minimization(&self) -> bool {
        self.action.get() == WindowBehavior::Minimize
    }

    pub(super) fn close(&self) {
        self.closing.set(true);
    }

    pub(super) fn is_closing(&self) -> bool {
        self.closing.get()
    }
}

pub(super) fn player_window(player: &PlayerWindow) -> Option<Retained<NSWindow>> {
    player
        .window()
        .with_winit_window(|window| content_view(window).and_then(|view| view.window()))
        .flatten()
}

pub(super) fn window_can_draw(player: &PlayerWindow) -> bool {
    player_window(player).is_some_and(|window| {
        window.isVisible()
            && !window.isMiniaturized()
            && window
                .occlusionState()
                .contains(NSWindowOcclusionState::Visible)
    })
}

pub(crate) struct MinimizeObservation {
    center: Retained<NSNotificationCenter>,
    tokens: [Retained<ProtocolObject<dyn NSObjectProtocol>>; 2],
}

impl MinimizeObservation {
    pub(crate) fn new(player: &PlayerWindow, event: impl Fn(bool) + 'static) -> Self {
        let center = NSNotificationCenter::defaultCenter();
        let event = Rc::new(event);
        // SAFETY: these are AppKit's static window notification names.
        let notifications = unsafe {
            [
                (NSWindowDidMiniaturizeNotification, true),
                (NSWindowDidDeminiaturizeNotification, false),
            ]
        };
        let tokens = notifications.map(|(name, minimized)| {
            let player = player.as_weak();
            let event = event.clone();
            let block = RcBlock::new(move |notification: NonNull<NSNotification>| {
                if MainThreadMarker::new().is_none() {
                    return;
                }
                // SAFETY: AppKit lends this notification for the duration of the block.
                let Some(sender) = unsafe { notification.as_ref() }.object() else {
                    return;
                };
                let event = event.clone();
                let player = player.clone();
                // AppKit can post from inside a Slint window operation.
                slint::Timer::single_shot(Duration::ZERO, move || {
                    let window = player.upgrade().and_then(|player| player_window(&player));
                    if window.is_some_and(|window| {
                        Retained::as_ptr(&window).cast() == Retained::as_ptr(&sender)
                    }) {
                        event(minimized);
                    }
                });
            });
            // SAFETY: the notification center copies the block and delivers it on the main queue.
            unsafe {
                center.addObserverForName_object_queue_usingBlock(
                    Some(name),
                    None,
                    Some(&NSOperationQueue::mainQueue()),
                    &block,
                )
            }
        });
        Self { center, tokens }
    }
}

impl Drop for MinimizeObservation {
    fn drop(&mut self) {
        for token in &self.tokens {
            // SAFETY: these tokens were registered with this retained notification center.
            unsafe { self.center.removeObserver(token.as_ref()) };
        }
    }
}
