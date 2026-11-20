//! Effective profile keys are compiled once when installing a profile.
use super::keys::normalize_profile_key;
use riina_core::key_bindings::Mapping;
use std::collections::{HashMap, hash_map::Entry};

pub(super) struct Bindings {
    mappings: Vec<Mapping>,
    by_key: HashMap<String, usize>,
    order: Vec<usize>,
}

impl Bindings {
    pub(super) fn new(mappings: Vec<Mapping>) -> Self {
        let mut by_key = HashMap::new();
        let mut order = Vec::new();
        for (index, mapping) in mappings.iter().enumerate() {
            let key = normalize_profile_key(&mapping.key);
            if key.is_empty() {
                continue;
            }
            match by_key.entry(key) {
                Entry::Occupied(slot) => order[*slot.get()] = index,
                Entry::Vacant(slot) => {
                    slot.insert(order.len());
                    order.push(index);
                }
            }
        }
        // A duplicate replaces its action without moving its first position in
        // the menu preference order, matching the pinned profile contract.
        Self {
            mappings,
            by_key,
            order,
        }
    }

    pub(super) fn get(&self, key: &str) -> Option<&Mapping> {
        self.by_key
            .get(key)
            .map(|&slot| &self.mappings[self.order[slot]])
    }

    pub(super) fn effective(&self) -> impl Iterator<Item = &Mapping> {
        self.order.iter().map(|&index| &self.mappings[index])
    }
}
