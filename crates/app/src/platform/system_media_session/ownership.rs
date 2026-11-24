//! Select the last focused playback window and identify its current media.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::platform) struct PlaybackTarget {
    pub(in crate::platform) window: u64,
    pub(in crate::platform) generation: u64,
}

#[derive(Default)]
pub(super) struct FocusOrder {
    recent: Vec<u64>,
}

impl FocusOrder {
    pub(super) fn select(&mut self, windows: impl IntoIterator<Item = (u64, bool)>) -> Option<u64> {
        let mut focused = None;
        let mut fallback = None;
        let mut remembered = None;
        for (id, has_focus) in windows {
            fallback = Some(id);
            if has_focus {
                focused = Some(id);
            }
            if let Some(rank) = self.recent.iter().position(|candidate| *candidate == id)
                && remembered.is_none_or(|(previous, _)| rank > previous)
            {
                remembered = Some((rank, id));
            }
        }
        if let Some(id) = focused
            && self.recent.last() != Some(&id)
        {
            self.remove(id);
            self.recent.push(id);
        }
        focused.or(remembered.map(|(_, id)| id)).or(fallback)
    }

    pub(super) fn remove(&mut self, id: u64) {
        self.recent.retain(|candidate| *candidate != id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_owner_follows_focus_and_keeps_it_when_settings_or_other_apps_take_focus() {
        let mut order = FocusOrder::default();
        assert_eq!(order.select([(1, false), (2, false)]), Some(2));
        assert_eq!(order.select([(1, true), (2, false)]), Some(1));
        assert_eq!(order.select([(1, false), (2, false)]), Some(1));
        assert_eq!(order.select([(1, false), (2, true), (3, false)]), Some(2));
        assert_eq!(order.select([(1, false), (2, false), (3, false)]), Some(2));
        order.remove(2);
        assert_eq!(order.select([(1, false), (3, false)]), Some(1));
        assert_eq!(order.select([]), None);
    }

    #[test]
    fn remote_command_identity_includes_window_and_media_generation() {
        let target = PlaybackTarget {
            window: 1,
            generation: 2,
        };
        assert_eq!(
            target,
            PlaybackTarget {
                window: 1,
                generation: 2
            }
        );
        assert_ne!(
            target,
            PlaybackTarget {
                window: 1,
                generation: 3
            }
        );
        assert_ne!(
            target,
            PlaybackTarget {
                window: 2,
                generation: 2
            }
        );
    }
}
