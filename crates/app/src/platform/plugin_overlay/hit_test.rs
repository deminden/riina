//! Cache only the last DOM hit for the current document and cursor position.
use riina_core::sync::lock;
use std::sync::{Arc, Mutex};
use wry::WebView;

pub(super) type Point = (f64, f64);

#[derive(Default)]
pub(super) struct HitTest(Mutex<State>);

#[derive(Default)]
struct State {
    generation: u64,
    pending: bool,
    point: Option<Point>,
}

impl HitTest {
    pub(super) fn clear(&self) {
        let mut state = lock(&self.0);
        state.generation = state.generation.wrapping_add(1);
        state.pending = false;
        state.point = None;
    }

    pub(super) fn contains(&self, point: Point) -> bool {
        lock(&self.0)
            .point
            .is_some_and(|(x, y)| (x - point.0).abs() < 1.0 && (y - point.1).abs() < 1.0)
    }

    pub(super) fn sample(self: &Arc<Self>, browser: &WebView, document: u64, point: Option<Point>) {
        let Some((x, y)) = point else {
            self.clear();
            return;
        };
        let generation = {
            let mut state = lock(&self.0);
            if state.pending {
                return;
            }
            state.pending = true;
            state.generation
        };
        let script = format!(
            r#"(() => {{
            if (!window.iina?._matchesDocument("{document}")) return false;
            return document.elementFromPoint({x}, {y})?.hasAttribute("data-clickable") ?? false;
        }})()"#
        );
        let hits = self.clone();
        if browser
            .evaluate_script_with_callback(&script, move |result| {
                hits.complete(generation, (result == "true").then_some((x, y)));
            })
            .is_err()
        {
            self.complete(generation, None);
        }
    }

    fn complete(&self, generation: u64, point: Option<Point>) {
        let mut state = lock(&self.0);
        if state.generation == generation {
            state.pending = false;
            state.point = point;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn overlay_hit_results_expire_on_pointer_motion_and_document_reset() {
        let hits = HitTest::default();
        hits.complete(0, Some((15.0, 36.0)));
        assert!(hits.contains((15.0, 36.0)));
        assert!(!hits.contains((19.0, 36.0)));
        hits.clear();
        hits.complete(0, Some((15.0, 36.0)));
        assert!(!hits.contains((15.0, 36.0)));
        hits.complete(1, Some((21.0, 19.0)));
        assert!(hits.contains((21.0, 19.0)));
    }
}
