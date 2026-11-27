use std::collections::BTreeMap;

#[derive(Default)]
pub(super) struct PlaylistShuffle {
    pending: BTreeMap<u64, Change>,
}

enum Change {
    Fifo(bool),
    LoadList,
}

impl PlaylistShuffle {
    pub(super) fn submitted(&mut self, id: u64, args: &[String]) {
        let change = match args.first().map(String::as_str) {
            Some("playlist-shuffle") => Change::Fifo(true),
            Some(
                "playlist-unshuffle" | "loadfile" | "playlist-clear" | "playlist-remove"
                | "playlist-move",
            ) => Change::Fifo(false),
            Some("stop") if args.get(1).is_none_or(|flag| flag != "keep-playlist") => {
                Change::Fifo(false)
            }
            Some("loadlist") => Change::LoadList,
            _ => return,
        };
        self.pending.insert(id, change);
    }

    pub(super) fn expected(&self, confirmed: bool) -> bool {
        self.pending
            .values()
            .rev()
            .find_map(|change| match change {
                Change::Fifo(enabled) => Some(*enabled),
                // libmpv reads a playlist off the command thread before applying it.
                Change::LoadList => None,
            })
            .unwrap_or(confirmed)
    }

    pub(super) fn replied(&mut self, id: u64, failed: bool) -> Option<bool> {
        let change = self.pending.remove(&id)?;
        (!failed).then_some(matches!(change, Change::Fifo(true)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejected_edits_preserve_shuffle_and_async_loads_reset_it_when_applied() {
        let mut shuffle = PlaylistShuffle::default();
        shuffle.submitted(1, &["playlist-shuffle".into()]);
        assert!(shuffle.expected(false));

        shuffle.submitted(2, &["playlist-remove".into(), "99".into()]);
        assert!(!shuffle.expected(false));
        assert_eq!(shuffle.replied(1, false), Some(true));
        assert_eq!(shuffle.replied(2, true), None);
        assert!(shuffle.expected(true));

        shuffle.submitted(3, &["loadlist".into(), "list.m3u".into()]);
        assert!(shuffle.expected(true));
        shuffle.submitted(4, &["playlist-shuffle".into()]);
        assert_eq!(shuffle.replied(4, false), Some(true));
        assert!(shuffle.expected(true));
        assert_eq!(shuffle.replied(3, false), Some(false));
    }
}
