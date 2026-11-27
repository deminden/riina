//! Classify keyboard focus before applying a player's inactive-window preference.
use crate::{
    NativeVideoHost, PlaybackStore, PlayerWindow,
    player_ui::lifecycle::{InactivePause, apply_inactive_pause},
};
use slint::{
    ComponentHandle, Timer, TimerMode, Weak,
    winit_030::{
        WinitWindowAccessor,
        winit::{
            event::WindowEvent,
            window::{Window, WindowId},
        },
    },
};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc, time::Duration};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Target {
    #[default]
    Outside,
    Auxiliary,
    Player(u64),
}

impl Target {
    fn player(self) -> Option<u64> {
        match self {
            Self::Player(id) => Some(id),
            _ => None,
        }
    }
}

#[derive(Default)]
struct Handoff(Target);

impl Handoff {
    fn update(&mut self, next: Target) -> (Option<u64>, Option<u64>) {
        let previous = std::mem::replace(&mut self.0, next);
        if previous == next {
            return (None, None);
        }
        let lose = (next != Target::Auxiliary)
            .then_some(previous.player())
            .flatten();
        (lose, next.player())
    }
}

struct Route {
    app: Weak<PlayerWindow>,
    owner: Option<Weak<NativeVideoHost>>,
    playback: PlaybackStore,
    pause: RefCell<InactivePause>,
}

impl Route {
    fn owns(&self, id: WindowId) -> bool {
        let matches = |window: &slint::Window| window.with_winit_window(Window::id) == Some(id);
        self.app.upgrade().is_some_and(|app| matches(app.window()))
            || self
                .owner
                .as_ref()
                .and_then(Weak::upgrade)
                .is_some_and(|owner| matches(owner.window()))
    }

    fn focus_changed(&self, focused: bool, outside: bool) {
        if let Some(app) = self.app.upgrade() {
            if !focused
                && outside
                && app
                    .global::<riina_ui::PlayerDialogState>()
                    .get_dialog_open()
            {
                // A host file chooser may own focus outside the Winit event loop.
                return;
            }
            apply_inactive_pause(&self.pause, &app, &self.playback, focused);
        }
    }
}

#[derive(Default)]
struct WindowFocus {
    routes: BTreeMap<u64, Rc<Route>>,
    next_id: u64,
    focused: Option<WindowId>,
    handoff: Handoff,
    settle: Timer,
}

impl WindowFocus {
    fn schedule(&self) {
        if !self.routes.is_empty() {
            // Focus-out and focus-in can arrive in separate event batches.
            // Classify the destination once the handoff has settled.
            self.settle.start(
                TimerMode::SingleShot,
                Duration::from_millis(30),
                settle_focus,
            );
        }
    }

    fn target(&self) -> Target {
        self.focused.map_or(Target::Outside, |focused| {
            self.routes
                .iter()
                .find_map(|(&id, route)| route.owns(focused).then_some(Target::Player(id)))
                .unwrap_or(Target::Auxiliary)
        })
    }
}

thread_local! {
    static FOCUS: RefCell<WindowFocus> = RefCell::default();
}

pub(super) struct Registration(u64);

impl Registration {
    pub(super) fn new(
        app: &PlayerWindow,
        owner: Option<Weak<NativeVideoHost>>,
        playback: PlaybackStore,
    ) -> Self {
        FOCUS.with_borrow_mut(|focus| {
            focus.next_id += 1;
            let id = focus.next_id;
            focus.routes.insert(
                id,
                Rc::new(Route {
                    app: app.as_weak(),
                    owner,
                    playback,
                    pause: RefCell::default(),
                }),
            );
            focus.schedule();
            Self(id)
        })
    }
}

impl Drop for Registration {
    fn drop(&mut self) {
        FOCUS.with_borrow_mut(|focus| {
            focus.routes.remove(&self.0);
            if focus.routes.is_empty() {
                focus.settle.stop();
                focus.handoff = Handoff::default();
            } else {
                focus.schedule();
            }
        });
    }
}

pub(super) fn window_event(id: WindowId, event: &WindowEvent) {
    FOCUS.with_borrow_mut(|focus| {
        let target = match event {
            WindowEvent::Focused(true) => Some(id),
            WindowEvent::Focused(false) | WindowEvent::Destroyed if focus.focused == Some(id) => {
                None
            }
            _ => return,
        };
        if std::mem::replace(&mut focus.focused, target) != target {
            focus.schedule();
        }
    });
}

fn settle_focus() {
    let (losing, gaining, outside) = FOCUS.with_borrow_mut(|focus| {
        let target = focus.target();
        let (losing, gaining) = focus.handoff.update(target);
        let get = |id: Option<u64>| id.and_then(|id| focus.routes.get(&id).cloned());
        (get(losing), get(gaining), target == Target::Outside)
    });
    if let Some(route) = losing {
        route.focus_changed(false, outside);
    }
    if let Some(route) = gaining {
        route.focus_changed(true, false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_handoffs_pause_only_the_player_that_lost_focus_to_playback_or_the_desktop() {
        let mut focus = Handoff::default();
        assert_eq!(focus.update(Target::Player(1)), (None, Some(1)));
        assert_eq!(focus.update(Target::Player(1)), (None, None));
        assert_eq!(focus.update(Target::Auxiliary), (None, None));
        assert_eq!(focus.update(Target::Outside), (None, None));
        assert_eq!(focus.update(Target::Player(1)), (None, Some(1)));
        assert_eq!(focus.update(Target::Player(2)), (Some(1), Some(2)));
        assert_eq!(focus.update(Target::Outside), (Some(2), None));
    }
}
