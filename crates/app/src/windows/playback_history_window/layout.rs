//! Remember the History window's normal bounds and table widths per profile.
use crate::PlaybackHistoryWindow;
use serde::{Deserialize, Serialize};
use slint::{
    ComponentHandle, LogicalSize,
    winit_030::{WinitWindowAccessor, winit::window::Window as NativeWindow},
};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
struct Saved {
    frame: Option<Frame>,
    progress_width: Option<f32>,
    played_at_width: Option<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
struct Frame {
    logical_width: f32,
    logical_height: f32,
    outer_position: Option<[i32; 2]>,
}

pub(super) struct Layout {
    path: PathBuf,
    saved: Saved,
    last_normal_frame: Option<Frame>,
}

impl Layout {
    pub(super) fn load(preferences: &Path) -> Self {
        let path = preferences.with_file_name("playback-history-layout.json");
        let saved: Saved = std::fs::read_to_string(&path)
            .ok()
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default();
        Self {
            path,
            saved,
            last_normal_frame: saved.frame,
        }
    }

    pub(super) fn restore_columns(&self, window: &PlaybackHistoryWindow) {
        if let Some(width) = self.saved.progress_width.filter(|width| positive(*width)) {
            window.set_progress_column_width(width);
        }
        if let Some(width) = self.saved.played_at_width.filter(|width| positive(*width)) {
            window.set_played_at_column_width(width);
        }
    }

    pub(super) fn restore_frame(&self, window: &slint::Window, native: &NativeWindow) -> bool {
        let Some(frame) = self.saved.frame.filter(|frame| frame.valid_size()) else {
            return false;
        };
        // Wayland leaves placement to its compositor. Restore size there.
        let saved_position = frame
            .outer_position
            .filter(|_| native.outer_position().is_ok());
        let monitor_at_position = saved_position.and_then(|position| {
            native.available_monitors().find(|monitor| {
                let origin = monitor.position();
                let size = monitor.size();
                contains_title_bar([origin.x, origin.y], [size.width, size.height], position)
            })
        });
        #[cfg(windows)]
        if monitor_at_position.is_none()
            && let Some(area) =
                crate::platform::window_sizing::auxiliary_work_area(native, saved_position)
        {
            // A private Windows desktop can provide a work area without
            // listing any monitors through the window's enumeration API.
            let position = saved_position.filter(|saved| {
                contains_title_bar(
                    [area.x, area.y],
                    [area.size.width, area.size.height],
                    *saved,
                )
            });
            let scale = native.scale_factor() as f32;
            let size = frame.size_within(
                area.size.width as f32 / scale,
                area.size.height as f32 / scale,
            );
            return apply_frame(window, position, size);
        }
        let position = saved_position.filter(|_| monitor_at_position.is_some());
        let monitor = monitor_at_position
            .or_else(|| native.current_monitor())
            .or_else(|| native.primary_monitor())
            .or_else(|| native.available_monitors().next());
        let size = if let Some(monitor) = monitor {
            let display = monitor.size().to_logical::<f32>(monitor.scale_factor());
            frame.size_within(display.width, display.height)
        } else {
            LogicalSize::new(frame.logical_width, frame.logical_height)
        };
        apply_frame(window, position, size)
    }

    pub(super) fn sample(&mut self, window: &slint::Window) {
        let frame = window.with_winit_window(|native| {
            if native.is_maximized() || native.is_minimized() == Some(true) {
                return None;
            }
            let size = native.inner_size().to_logical(native.scale_factor());
            let frame = Frame {
                logical_width: size.width,
                logical_height: size.height,
                outer_position: native
                    .outer_position()
                    .ok()
                    .map(|position| [position.x, position.y]),
            };
            frame.valid_size().then_some(frame)
        });
        if let Some(Some(frame)) = frame {
            self.last_normal_frame = Some(frame);
        }
    }

    pub(super) fn save(&mut self, window: &PlaybackHistoryWindow) -> std::io::Result<()> {
        self.sample(window.window());
        let current = Saved {
            frame: self.last_normal_frame,
            progress_width: Some(window.get_progress_column_width())
                .filter(|width| positive(*width)),
            played_at_width: Some(window.get_played_at_column_width())
                .filter(|width| positive(*width)),
        };
        if self.saved != current {
            let json = serde_json::to_string(&current).map_err(std::io::Error::other)?;
            crate::platform::save_preferences(&self.path, &json)?;
            self.saved = current;
        }
        Ok(())
    }
}

fn apply_frame(window: &slint::Window, position: Option<[i32; 2]>, size: LogicalSize) -> bool {
    if let Some([x, y]) = position {
        window.set_position(slint::PhysicalPosition::new(x, y));
    }
    window.set_size(size);
    position.is_some()
}

impl Frame {
    fn valid_size(self) -> bool {
        positive(self.logical_width) && positive(self.logical_height)
    }

    fn size_within(self, display_width: f32, display_height: f32) -> LogicalSize {
        // Leave room for the window frame and desktop chrome after a display change.
        LogicalSize::new(
            self.logical_width.min((display_width - 24.0).max(1.0)),
            self.logical_height.min((display_height - 48.0).max(1.0)),
        )
    }
}

fn positive(value: f32) -> bool {
    value.is_finite() && value > 0.0
}

fn contains_title_bar(origin: [i32; 2], size: [u32; 2], window: [i32; 2]) -> bool {
    // Native borders can sit just outside a display. Check a reachable point
    // inside the title bar and use wide arithmetic for negative coordinates.
    let [x, y] = window.map(i64::from);
    let [left, top] = origin.map(i64::from);
    (left..left + i64::from(size[0])).contains(&(x + 64))
        && (top..top + i64::from(size[1])).contains(&(y + 20))
}

#[cfg(test)]
mod tests {
    use super::*;
    use i_slint_backend_testing::ElementHandle;
    use slint::{
        LogicalPosition,
        platform::{Key, PointerEventButton, WindowEvent},
    };

    #[test]
    fn history_geometry_survives_negative_displays_and_rejects_removed_ones() {
        let frame = Frame {
            logical_width: 1300.0,
            logical_height: 900.0,
            outer_position: Some([-1605, 0]),
        };
        let saved = frame.outer_position.unwrap();
        assert!(contains_title_bar([-1600, 0], [1600, 900], saved));
        assert!(!contains_title_bar([0, 0], [1200, 750], saved));
        let bounded = frame.size_within(1200.0, 750.0);
        assert_eq!([bounded.width, bounded.height], [1176.0, 702.0]);
    }

    #[test]
    fn history_columns_resize_by_pointer_keyboard_and_accessibility_and_reopen_with_saved_widths()
    -> Result<(), Box<dyn std::error::Error>> {
        i_slint_backend_testing::init_no_event_loop();
        let profile = riina_test_support::TempDir::new("history-layout")?;
        let preferences = profile.path().join("preferences.json");
        let window = PlaybackHistoryWindow::new()?;
        window.window().set_size(LogicalSize::new(800.0, 600.0));
        window.show()?;
        let progress = ElementHandle::find_by_accessible_label(&window, "Progress column width")
            .next()
            .unwrap();
        let position = progress.absolute_position();
        let start = LogicalPosition::new(position.x + 5.0, position.y + 12.0);
        let left = LogicalPosition::new(start.x - 30.0, start.y);
        let view = window.window();
        view.dispatch_event(WindowEvent::PointerPressed {
            position: start,
            button: PointerEventButton::Left,
        });
        view.dispatch_event(WindowEvent::PointerMoved { position: left });
        view.dispatch_event(WindowEvent::PointerReleased {
            position: left,
            button: PointerEventButton::Left,
        });
        assert_eq!(window.get_progress_column_width(), 175.0);
        view.dispatch_event(WindowEvent::KeyPressed {
            text: Key::LeftArrow.into(),
        });
        assert_eq!(window.get_progress_column_width(), 183.0);

        let time = ElementHandle::find_by_accessible_label(&window, "Played At column width")
            .next()
            .unwrap();
        time.set_accessible_value("240");
        assert_eq!(window.get_played_at_column_width(), 240.0);
        view.set_size(LogicalSize::new(520.0, 340.0));
        assert!(progress.accessible_value_maximum().unwrap() < 183.0);
        assert_eq!(window.get_progress_column_width(), 183.0);

        Layout::load(&preferences).save(&window)?;
        window.hide()?;
        let reopened = PlaybackHistoryWindow::new()?;
        Layout::load(&preferences).restore_columns(&reopened);
        assert_eq!(reopened.get_progress_column_width(), 183.0);
        assert_eq!(reopened.get_played_at_column_width(), 240.0);
        Ok(())
    }
}
