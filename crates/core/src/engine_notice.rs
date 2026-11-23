use std::sync::Arc;

const MAX_NOTICES: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoticeSeverity {
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineNotice {
    pub sequence: u64,
    pub severity: NoticeSeverity,
    pub message: String,
}

/// The latest notice drives the current status while prior notices remain available.
#[derive(Debug, Clone, Default)]
pub struct EngineNotices {
    entries: Arc<Vec<EngineNotice>>,
    current: bool,
}

impl EngineNotices {
    pub fn push(&mut self, severity: NoticeSeverity, message: String) {
        let entries = Arc::make_mut(&mut self.entries);
        let sequence = entries
            .last()
            .map_or(1, |notice| notice.sequence.wrapping_add(1));
        if entries.len() == MAX_NOTICES {
            entries.remove(0);
        }
        entries.push(EngineNotice {
            sequence,
            severity,
            message,
        });
        self.current = true;
    }

    pub fn all(&self) -> &[EngineNotice] {
        &self.entries
    }

    pub fn current(&self) -> Option<&EngineNotice> {
        self.entries.last().filter(|_| self.current)
    }

    pub fn clear_current(&mut self) {
        self.current = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshots_keep_recent_notices_when_playback_clears_the_status() {
        let mut notices = EngineNotices::default();
        for sequence in 1..=20 {
            notices.push(NoticeSeverity::Error, format!("native failure {sequence}"));
        }
        let snapshot = notices.clone();
        notices.clear_current();
        notices.push(NoticeSeverity::Warning, "new media warning".into());

        assert_eq!(snapshot.all().len(), MAX_NOTICES);
        assert_eq!(snapshot.all()[0].sequence, 5);
        assert_eq!(snapshot.current().unwrap().message, "native failure 20");
        assert_eq!(notices.all()[0].sequence, 6);
        assert_eq!(notices.current().unwrap().sequence, 21);
        assert_eq!(notices.current().unwrap().severity, NoticeSeverity::Warning);
        notices.clear_current();
        assert!(notices.current().is_none());
        assert_eq!(notices.all().len(), MAX_NOTICES);
    }
}
