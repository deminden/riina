//! Track a native file drag only for the lifetime of its owning player window.
use crate::{PlayerWindow, background, platform};
use riina_ui::{PlayerCropState, PlayerMenuActions, PlayerMusicModeState, PlayerSidebarState};
use slint::{
    ComponentHandle, LogicalPosition, Timer, TimerMode, Weak, winit_030::winit::event::WindowEvent,
};
use std::{
    cell::RefCell,
    path::PathBuf,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

const SAMPLE_INTERVAL: Duration = Duration::from_millis(25);
const REVEAL_DELAY: Duration = Duration::from_millis(300);

pub(super) struct PlaylistHover {
    timer: Rc<Timer>,
    drag: Rc<RefCell<Option<Drag>>>,
    player: Weak<PlayerWindow>,
}

impl PlaylistHover {
    pub(super) fn new(app: &PlayerWindow) -> Self {
        let timer = Rc::new(Timer::default());
        let drag: Rc<RefCell<Option<Drag>>> = Rc::default();
        let tick_timer = Rc::downgrade(&timer);
        let tick_drag = drag.clone();
        let player = app.as_weak();
        let tick_player = player.clone();
        timer.start(TimerMode::Repeated, SAMPLE_INTERVAL, move || {
            let keep_sampling = tick_player
                .upgrade()
                .is_some_and(|app| sample(&app, &mut tick_drag.borrow_mut()));
            if !keep_sampling && let Some(timer) = tick_timer.upgrade() {
                timer.stop();
            }
        });
        timer.stop();
        Self {
            timer,
            drag,
            player,
        }
    }

    pub(super) fn on_event(&self, event: &WindowEvent) {
        match event {
            WindowEvent::HoveredFile(path) => {
                let mut drag = self.drag.borrow_mut();
                let drag = drag.get_or_insert_with(Drag::default);
                if !drag.revealed {
                    drag.paths.push(path.clone());
                    if !self.timer.running() {
                        self.timer.restart();
                    }
                }
            }
            WindowEvent::HoveredFileCancelled
            | WindowEvent::DroppedFile(_)
            | WindowEvent::CloseRequested
            | WindowEvent::Destroyed => {
                self.timer.stop();
                let drag = self.drag.borrow_mut().take();
                if let (Some(drag), Some(app)) = (drag, self.player.upgrade()) {
                    drag.finish(&app);
                }
            }
            _ => {}
        }
    }
}

fn sample(app: &PlayerWindow, active: &mut Option<Drag>) -> bool {
    if !app.window().is_visible()
        || app.global::<PlayerMusicModeState>().get_music_mode()
        || app.global::<PlayerCropState>().get_crop_editing()
    {
        if let Some(drag) = active.take() {
            drag.finish(app);
        }
        return false;
    }
    let Some(drag) = active.as_mut() else {
        return false;
    };
    if drag.revealed {
        return true;
    }
    let window = app.window();
    let size = window.size().to_logical(window.scale_factor());
    // Winit reports the file list on entry, without the later drag coordinates.
    let position = platform::file_drop_position(window).filter(|point| {
        point.x > size.width * 0.8
            && point.x < size.width
            && point.y >= 0.0
            && point.y < size.height
    });
    let settled = drag.dwell.settled(position, Instant::now());
    if position.is_none() || drag.eligibility.scanning.load(Ordering::Acquire) {
        return true;
    }
    if drag.eligibility.playable.load(Ordering::Acquire) {
        if settled {
            drag.revealed = true;
            app.global::<PlayerMenuActions>()
                .invoke_toggle_playlist_pane(4);
        }
    } else if drag.paths.is_empty() {
        return false;
    } else {
        drag.check_files();
    }
    true
}

#[derive(Default)]
struct Dwell(Option<(LogicalPosition, Instant)>);

impl Dwell {
    fn settled(&mut self, position: Option<LogicalPosition>, now: Instant) -> bool {
        if let (Some((previous, since)), Some(point)) = (self.0, position)
            && point == previous
        {
            return now.duration_since(since) >= REVEAL_DELAY;
        }
        self.0 = position.map(|point| (point, now));
        false
    }
}

#[derive(Default)]
struct Drag {
    paths: Vec<PathBuf>,
    eligibility: Arc<Eligibility>,
    dwell: Dwell,
    revealed: bool,
}

#[derive(Default)]
struct Eligibility {
    scanning: AtomicBool,
    playable: AtomicBool,
    cancelled: AtomicBool,
}

impl Drag {
    fn check_files(&mut self) {
        let paths = std::mem::take(&mut self.paths);
        let result = self.eligibility.clone();
        result.scanning.store(true, Ordering::Release);
        let started = background::spawn_named("playlist-drag-hover", move || {
            let cancelled = || result.cancelled.load(Ordering::Relaxed);
            let playable = paths.iter().filter_map(|path| path.to_str()).any(|path| {
                !cancelled()
                    && riina_core::file_selection::can_add_to_playlist(
                        &platform::resolve_alias_path(path),
                        cancelled,
                    )
            });
            result.playable.store(playable, Ordering::Release);
            result.scanning.store(false, Ordering::Release);
        });
        if started.is_err() {
            self.eligibility.scanning.store(false, Ordering::Release);
        }
    }

    fn finish(self, app: &PlayerWindow) {
        if self.revealed {
            let sidebar = app.global::<PlayerSidebarState>();
            sidebar.set_leading_sidebar(0);
            sidebar.set_sidebar(0);
        }
    }
}

impl Drop for Drag {
    fn drop(&mut self) {
        self.eligibility.cancelled.store(true, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_move_or_exit_restarts_the_reveal_delay() {
        let start = Instant::now();
        let at = |millis| start + Duration::from_millis(millis);
        let first = Some(LogicalPosition::new(850.0, 100.0));
        let next = Some(LogicalPosition::new(851.0, 100.0));
        let mut dwell = Dwell::default();

        assert!(!dwell.settled(first, at(0)));
        assert!(!dwell.settled(first, at(299)));
        assert!(!dwell.settled(next, at(300)));
        assert!(dwell.settled(next, at(600)));
        assert!(!dwell.settled(None, at(601)));
        assert!(!dwell.settled(next, at(900)));
        assert!(dwell.settled(next, at(1200)));
    }
}
