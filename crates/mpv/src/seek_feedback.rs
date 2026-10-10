//! User navigation feedback is distinct from decoder-internal native seeks.
#[derive(Default)]
pub(crate) struct SeekFeedback {
    request: Option<u64>,
    started: bool,
}

impl SeekFeedback {
    pub fn submitted(&mut self, id: u64) {
        self.request = Some(id);
    }
    pub fn started(&mut self) {
        self.started = self.request.is_some();
    }
    pub fn replied(&mut self, id: u64, failed: bool) {
        // A successful asynchronous reply can precede SEEK. It acknowledges
        // command processing, not playback completion; retain its provenance.
        if self.request == Some(id) && failed {
            self.clear();
        }
    }
    pub fn completed(&mut self) -> bool {
        let navigation = self.started && self.request.is_some();
        self.clear();
        navigation
    }
    pub fn clear(&mut self) {
        self.request = None;
        self.started = false;
    }
}

pub(crate) fn navigation(args: &[String]) -> bool {
    matches!(
        args.first().map(String::as_str),
        Some("seek" | "sub-seek" | "frame-step" | "frame-back-step")
    ) || (matches!(args.first().map(String::as_str), Some("add" | "set"))
        && args.get(1).is_some_and(|property| property == "chapter"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decoder_refresh_is_not_navigation_and_replacement_discards_pending_feedback() {
        let mut feedback = SeekFeedback::default();
        feedback.started();
        assert!(!feedback.completed());
        feedback.submitted(1);
        feedback.started();
        assert!(feedback.completed());
        feedback.started();
        assert!(!feedback.completed());
        feedback.submitted(2);
        feedback.started();
        feedback.clear();
        assert!(!feedback.completed());
        feedback.submitted(3);
        feedback.replied(3, true);
        feedback.started();
        assert!(!feedback.completed());
        feedback.submitted(4);
        feedback.replied(4, false);
        feedback.started();
        assert!(feedback.completed());
    }
}
