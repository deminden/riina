//! Track selected records by identity through filtered and rolling log views.
use std::collections::BTreeSet;

#[derive(Default)]
pub(super) struct Selection {
    selected: BTreeSet<u64>,
    anchor: Option<u64>,
    active: Option<u64>,
}

impl Selection {
    pub(super) fn contains(&self, id: u64) -> bool {
        self.selected.contains(&id)
    }

    pub(super) fn active_index(&self, entries: &[u64]) -> Option<usize> {
        entries.iter().position(|id| Some(*id) == self.active)
    }

    pub(super) fn retain_visible(&mut self, entries: &[u64]) {
        self.selected.retain(|id| entries.binary_search(id).is_ok());
        self.anchor = self.anchor.filter(|id| entries.binary_search(id).is_ok());
        self.active = self.active.filter(|id| entries.binary_search(id).is_ok());
    }

    pub(super) fn select(&mut self, entries: &[u64], index: usize, toggle: bool, range: bool) {
        let Some(&id) = entries.get(index) else {
            return;
        };
        if !toggle {
            self.selected.clear();
        }
        if let Some(anchor) = self.anchor
            && range
            && let Ok(start) = entries.binary_search(&anchor)
        {
            self.selected
                .extend(&entries[start.min(index)..=start.max(index)]);
        } else {
            self.anchor = Some(id);
            if !toggle || !self.selected.remove(&id) {
                self.selected.insert(id);
            }
        }
        self.active = Some(id);
    }

    pub(super) fn select_all(&mut self, entries: &[u64]) {
        self.selected = entries.iter().copied().collect();
        self.anchor = entries.first().copied();
        self.active = self.anchor;
    }
}
