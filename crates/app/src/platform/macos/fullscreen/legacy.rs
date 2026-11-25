//! Keep the player in its current Space and restore the native window on exit.
use super::{appkit_window, native_window};
use crate::{PlayerWindow, platform::macos::display_blackout::Cover};
use objc2::{MainThreadMarker, MainThreadOnly, rc::Retained};
use objc2_app_kit::{
    NSApplication, NSApplicationPresentationOptions as Options, NSFloatingWindowLevel,
    NSMainMenuWindowLevel, NSScreen, NSWindow, NSWindowCollectionBehavior as Collection,
};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use riina_ui::{PlayerLayoutState, PlayerPreferences};
use slint::{
    ComponentHandle,
    winit_030::{
        WinitWindowAccessor,
        winit::{event::WindowEvent, platform::macos::WindowExtMacOS, window::Window},
    },
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

pub(in crate::platform) struct Legacy {
    app: slint::Weak<PlayerWindow>,
    selected: Cell<bool>,
    active: RefCell<Option<Active>>,
    original_collection: Cell<Option<Collection>>,
    deferred_refresh: slint::Timer,
    retired: Cell<bool>,
}

impl Legacy {
    pub(in crate::platform) fn install(app: &PlayerWindow) -> Rc<Self> {
        let legacy = Rc::new(Self {
            app: app.as_weak(),
            selected: Cell::new(
                app.global::<PlayerPreferences>()
                    .get_use_legacy_fullscreen(),
            ),
            active: RefCell::new(None),
            original_collection: Cell::new(None),
            deferred_refresh: slint::Timer::default(),
            retired: Cell::new(false),
        });
        let weak = Rc::downgrade(&legacy);
        app.on_legacy_fullscreen_selection_changed(move |selected| {
            if let Some(legacy) = weak.upgrade() {
                legacy.selected.set(selected);
                legacy.update_collection();
            }
        });
        legacy.update_collection();
        legacy
    }

    pub(in crate::platform) fn request(&self, app: &PlayerWindow, active: bool) -> Option<bool> {
        if active {
            if !app
                .global::<PlayerPreferences>()
                .get_use_legacy_fullscreen()
            {
                return None;
            }
            return Some(
                app.window()
                    .with_winit_window(|native| self.enter(app, native))
                    .unwrap_or(false),
            );
        }
        if self.active.borrow().is_none() {
            return None;
        }
        Some(
            app.window()
                .with_winit_window(|native| self.leave(Some(native), Some(app)))
                .unwrap_or(false),
        )
    }

    fn enter(&self, app: &PlayerWindow, native: &Window) -> bool {
        let Some(window) = appkit_window(native).filter(|window| window.screen().is_some()) else {
            return false;
        };
        let application = NSApplication::sharedApplication(window.mtm());
        let before = application.presentationOptions();
        let mut entry = Active::capture(window, app);
        if !native.set_simple_fullscreen(true) {
            return false;
        }
        acquire_presentation(&application, before);
        entry.window.setHasShadow(false);
        entry.window.setLevel(NSFloatingWindowLevel);
        entry.window.setResizeIncrements(NSSize::new(1.0, 1.0));
        entry.refresh();
        self.active.replace(Some(entry));
        true
    }

    fn leave(&self, native: Option<&Window>, app: Option<&PlayerWindow>) -> bool {
        let Some(entry) = self.active.take() else {
            return false;
        };
        let application = NSApplication::sharedApplication(entry.window.mtm());
        let before = application.presentationOptions();
        if let Some(native) = native
            && native.simple_fullscreen()
            && !native.set_simple_fullscreen(false)
            && !self.retired.get()
        {
            self.active.replace(Some(entry));
            return false;
        }
        release_presentation(&application, before);
        if !self.retired.get() {
            entry.restore(app);
        }
        true
    }

    pub(in crate::platform) fn update_collection(&self) {
        let Some(app) = self.app.upgrade().filter(|_| !self.retired.get()) else {
            return;
        };
        if app.get_fullscreen() || app.get_entering_fullscreen() {
            return;
        }
        let Some(window) = native_window(app.window()) else {
            return;
        };
        let modes = Collection::FullScreenPrimary
            | Collection::FullScreenAuxiliary
            | Collection::FullScreenNone;
        let current = window.collectionBehavior();
        let selected = app
            .global::<PlayerPreferences>()
            .get_use_legacy_fullscreen();
        let mode = if selected {
            if self.original_collection.get().is_none() {
                self.original_collection.set(Some(current & modes));
            }
            Collection::FullScreenAuxiliary
        } else if let Some(original) = self.original_collection.take() {
            original
        } else {
            return;
        };
        let policy = (current - modes) | mode;
        if policy != current {
            window.setCollectionBehavior(policy);
        }
    }

    pub(in crate::platform) fn window_event(self: &Rc<Self>, event: &WindowEvent) {
        if self.retired.get()
            || self.deferred_refresh.running()
            || (!self.selected.get()
                && self.original_collection.get().is_none()
                && self.active.borrow().is_none())
            || !matches!(
                event,
                WindowEvent::Moved(_)
                    | WindowEvent::Resized(_)
                    | WindowEvent::ScaleFactorChanged { .. }
                    | WindowEvent::Focused(_)
            )
        {
            return;
        }
        let weak = Rc::downgrade(self);
        self.deferred_refresh
            .start(slint::TimerMode::SingleShot, Duration::ZERO, move || {
                if let Some(legacy) = weak.upgrade() {
                    if let Some(entry) = legacy.active.borrow_mut().as_mut() {
                        entry.refresh();
                    } else {
                        legacy.update_collection();
                    }
                }
            });
    }

    pub(in crate::platform) fn retire(&self) {
        if self.retired.replace(true) {
            return;
        }
        self.deferred_refresh.stop();
        let handled = self.app.upgrade().and_then(|app| {
            app.window()
                .with_winit_window(|native| self.leave(Some(native), None))
        });
        if handled.is_none() {
            self.leave(None, None);
        }
    }
}

impl Drop for Legacy {
    fn drop(&mut self) {
        self.retire();
    }
}

struct Active {
    window: Retained<NSWindow>,
    restored_frame: NSRect,
    shadow: bool,
    level: isize,
    keep_above: bool,
    movable: bool,
    increments: NSSize,
    notch: Option<Cover>,
}

impl Active {
    fn capture(window: Retained<NSWindow>, app: &PlayerWindow) -> Self {
        Self {
            restored_frame: window.frame(),
            shadow: window.hasShadow(),
            level: window.level(),
            keep_above: app.global::<PlayerLayoutState>().get_always_float_on_top(),
            movable: window.isMovable(),
            increments: window.resizeIncrements(),
            window,
            notch: None,
        }
    }

    fn refresh(&mut self) {
        let Some(screen) = self.window.screen() else {
            self.notch.take();
            return;
        };
        let screen_frame = screen.frame();
        let mut frame = screen_frame;
        let top = if objc2::available!(macos = 12.0) {
            screen
                .safeAreaInsets()
                .top
                .clamp(0.0, screen_frame.size.height.max(1.0) - 1.0)
        } else {
            0.0
        };
        frame.size.height -= top;
        if self.window.frame() != frame {
            self.window.setFrame_display(frame, true);
        }
        if top == 0.0 {
            self.notch.take();
        } else {
            let safe_strip = NSRect::new(
                NSPoint::new(frame.origin.x, frame.origin.y + frame.size.height),
                NSSize::new(frame.size.width, top),
            );
            if self
                .notch
                .as_ref()
                .is_none_or(|cover| cover.frame != safe_strip)
            {
                self.notch = Some(Cover::new(
                    self.window.mtm(),
                    safe_strip,
                    NSMainMenuWindowLevel,
                    Collection::Managed,
                ));
            }
        }
    }

    fn restore(self, app: Option<&PlayerWindow>) {
        let current_above = app.map_or(self.keep_above, |app| {
            app.global::<PlayerLayoutState>().get_always_float_on_top()
        });
        let level = match (self.keep_above, current_above) {
            (before, now) if before == now => self.level,
            (_, true) => NSFloatingWindowLevel,
            (_, false) => objc2_app_kit::NSNormalWindowLevel,
        };
        self.window.setLevel(level);
        self.window.setHasShadow(self.shadow);
        self.window.setMovable(self.movable);
        self.window.setResizeIncrements(self.increments);
        self.window
            .setFrame_display(recover_frame(self.restored_frame, self.window.mtm()), true);
    }
}

fn recover_frame(frame: NSRect, main: MainThreadMarker) -> NSRect {
    if NSScreen::screens(main).iter().any(|screen| {
        let screen = screen.visibleFrame();
        frame.origin.x < screen.origin.x + screen.size.width
            && screen.origin.x < frame.origin.x + frame.size.width
            && frame.origin.y < screen.origin.y + screen.size.height
            && screen.origin.y < frame.origin.y + frame.size.height
    }) {
        return frame;
    }
    NSScreen::mainScreen(main).map_or(frame, |screen| {
        let bounds = screen.visibleFrame();
        let size = NSSize::new(
            frame.size.width.min(bounds.size.width),
            frame.size.height.min(bounds.size.height),
        );
        let origin = NSPoint::new(
            bounds.origin.x + (bounds.size.width - size.width) / 2.0,
            bounds.origin.y + (bounds.size.height - size.height) / 2.0,
        );
        NSRect::new(origin, size)
    })
}

const AUTO_HIDE: Options = Options::AutoHideDock.union(Options::AutoHideMenuBar);
const MANAGED: Options = AUTO_HIDE
    .union(Options::HideDock)
    .union(Options::HideMenuBar);

thread_local! {
    static PRESENTATION: Cell<(usize, Options)> = const { Cell::new((0, Options::empty())) };
}

fn acquire_presentation(app: &NSApplication, before: Options) {
    PRESENTATION.with(|state| {
        let (count, saved) = state.get();
        state.set((count + 1, if count == 0 { before & MANAGED } else { saved }));
    });
    app.setPresentationOptions((before - MANAGED) | AUTO_HIDE);
}

fn release_presentation(app: &NSApplication, before: Options) {
    let managed = PRESENTATION.with(|state| {
        let (count, saved) = state.get();
        let remaining = count.saturating_sub(1);
        state.set((remaining, saved));
        if remaining == 0 { saved } else { AUTO_HIDE }
    });
    app.setPresentationOptions((before - MANAGED) | managed);
}
