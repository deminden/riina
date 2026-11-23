//! Route and stack the offscreen layers using the shared view's logical coordinates.
use super::{Entry, Presenter, hit_test::Point};
use gtk::gdk::EventType;
use riina_ui::{PlayerPluginPointerPhase as Phase, PluginOverlayFrame};

#[derive(Default)]
pub(super) struct Composition {
    next_order: u64,
    changed: bool,
    cursor: Option<Point>,
    hovered: Option<u64>,
    pressed: [Option<Press>; 3],
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Press {
    order: u64,
    document: u64,
}

impl Composition {
    pub(super) fn mark_changed(&mut self) {
        self.changed = true;
    }

    pub(super) fn stack(&mut self, mut entry: Entry) -> Entry {
        self.next_order = self.next_order.wrapping_add(1);
        entry.stacking_order = self.next_order;
        self.mark_changed();
        entry
    }

    fn held_buttons(&self, entry: &Entry) -> u32 {
        let press = Some(entry.press());
        u32::from(self.pressed[0] == press)
            | (u32::from(self.pressed[1] == press) << 1)
            | (u32::from(self.pressed[2] == press) << 2)
    }
}

impl Presenter {
    pub(in crate::platform) fn frames(&mut self) -> Option<Vec<PluginOverlayFrame>> {
        let mut changed = std::mem::take(&mut self.composition.changed);
        for entry in self.entries.values() {
            changed |= entry.native.take_changed();
            entry.sample(self.composition.cursor);
        }
        if !changed {
            return None;
        }
        let mut frames: Vec<_> = self
            .entries
            .values()
            .filter_map(|entry| Some((entry.stacking_order, entry.frame()?)))
            .collect();
        frames.sort_unstable_by_key(|(order, _)| *order);
        Some(frames.into_iter().map(|(_, frame)| frame).collect())
    }

    pub(in crate::platform) fn pointer(&mut self, phase: Phase, button: i32, point: Point) -> bool {
        self.composition.cursor = (phase != Phase::Cancel).then_some(point);
        let entries = &self.entries;
        let input = &mut self.composition;
        if phase == Phase::Cancel {
            for (index, press) in input.pressed.into_iter().enumerate() {
                if let Some(entry) = entries.values().find(|entry| Some(entry.press()) == press) {
                    entry.native.pointer(
                        EventType::ButtonRelease,
                        (-1.0, -1.0),
                        (index + 1) as u32,
                        input.held_buttons(entry),
                    );
                }
            }
            input.pressed = [None; 3];
        }
        let target = entries
            .values()
            .filter_map(|entry| Some((entry, entry.hit(input.cursor)?)))
            .max_by_key(|(entry, _)| entry.stacking_order);
        if phase == Phase::Move || phase == Phase::Cancel {
            let hovered = target.map(|(entry, _)| entry.stacking_order);
            if hovered != input.hovered
                && let Some(old) = entries
                    .values()
                    .find(|entry| Some(entry.stacking_order) == input.hovered)
                && input.held_buttons(old) == 0
            {
                old.native
                    .pointer(EventType::MotionNotify, (-1.0, -1.0), 0, 0);
            }
            input.hovered = hovered;
            for entry in entries.values() {
                entry.sample(input.cursor);
                let held = input.held_buttons(entry);
                if held != 0 || Some(entry.stacking_order) == hovered {
                    entry
                        .native
                        .pointer(EventType::MotionNotify, entry.local(point), 0, held);
                }
            }
            return hovered.is_some() || input.pressed.iter().any(Option::is_some);
        }
        let index = match button {
            1..=3 => (button - 1) as usize,
            _ => return false,
        };
        if phase == Phase::Down {
            let Some((entry, local)) = target else {
                return false;
            };
            entry.native.pointer(
                EventType::ButtonPress,
                local,
                button as u32,
                input.held_buttons(entry),
            );
            input.pressed[index] = Some(entry.press());
            return true;
        }
        let Some(press) = input.pressed[index] else {
            return false;
        };
        if let Some(entry) = entries.values().find(|entry| entry.press() == press) {
            let local = if entry.shown {
                entry.local(point)
            } else {
                (-1.0, -1.0)
            };
            entry.native.pointer(
                EventType::ButtonRelease,
                local,
                button as u32,
                input.held_buttons(entry),
            );
        }
        input.pressed[index] = None;
        true
    }
}

impl Entry {
    fn press(&self) -> Press {
        Press {
            order: self.stacking_order,
            document: self.revision,
        }
    }

    fn frame(&self) -> Option<PluginOverlayFrame> {
        let rect = self.bounds?;
        let pos = rect.position.to_logical::<f32>(1.0);
        let size = rect.size.to_logical::<f32>(1.0);
        Some(PluginOverlayFrame {
            pixels: self.native.pixels()?,
            x: pos.x,
            y: pos.y,
            width: size.width,
            height: size.height,
        })
    }

    fn local(&self, point: Point) -> Point {
        let position = self
            .bounds
            .unwrap_or_default()
            .position
            .to_logical::<f64>(1.0);
        (point.0 - position.x, point.1 - position.y)
    }

    fn eligible_position(&self, position: Option<Point>) -> Option<Point> {
        if !self.shown || !self.clickable || self.page.ready_page().is_none() {
            return None;
        }
        let local = self.local(position?);
        let size = self.bounds?.size.to_logical::<f64>(1.0);
        (local.0 >= 0.0 && local.0 < size.width && local.1 >= 0.0 && local.1 < size.height)
            .then_some(local)
    }

    fn hit(&self, position: Option<Point>) -> Option<Point> {
        self.eligible_position(position)
            .filter(|point| self.hit_test.contains(*point))
    }

    fn sample(&self, cursor: Option<Point>) {
        self.hit_test.sample(
            self.browser(),
            self.revision,
            self.eligible_position(cursor),
        );
    }
}
