//! Per-file Auto seek policy measured from native SEEK to PLAYBACK_RESTART.
use std::time::{Duration, Instant};

#[derive(Default)]
pub(crate) struct AutoSeek {
    measurement: Measurement,
}

#[derive(Default)]
enum Measurement {
    #[default]
    Unmeasured,
    AwaitingSeek {
        request: u64,
    },
    Seeking {
        request: u64,
        started: Instant,
    },
    Decided {
        exact: bool,
    },
}

impl AutoSeek {
    pub fn flags(&self) -> &'static str {
        match self.measurement {
            Measurement::Decided { exact: false } => "relative",
            _ => "relative+exact",
        }
    }

    pub fn submitted(&mut self, request: u64) {
        if matches!(self.measurement, Measurement::Unmeasured) {
            self.measurement = Measurement::AwaitingSeek { request };
        }
    }

    pub fn failed(&mut self, request: u64) {
        if matches!(self.measurement,
            Measurement::AwaitingSeek { request: pending }
                | Measurement::Seeking { request: pending, .. } if pending == request)
        {
            self.interrupt_measurement();
        }
    }

    pub fn seek_started(&mut self, now: Instant) {
        if let Measurement::AwaitingSeek { request } = self.measurement {
            self.measurement = Measurement::Seeking {
                request,
                started: now,
            };
        }
    }

    pub fn playback_restarted(&mut self, now: Instant) {
        if let Measurement::Seeking { started, .. } = self.measurement {
            // IINA v1.5.0 keeps this choice for the remainder of the file.
            self.measurement = Measurement::Decided {
                exact: now.saturating_duration_since(started) < Duration::from_millis(50),
            };
        }
    }

    pub fn interrupt_measurement(&mut self) {
        // Decoder refreshes and explicit navigation must not supply timing for an
        // unfinished Auto seek. A completed decision still belongs to this file.
        if !matches!(self.measurement, Measurement::Decided { .. }) {
            self.measurement = Measurement::Unmeasured;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fast_seek_keeps_exact_mode_for_all_later_auto_seeks() {
        let mut policy = AutoSeek::default();
        let start = Instant::now();
        assert_eq!(policy.flags(), "relative+exact");
        policy.submitted(1);
        policy.seek_started(start);
        policy.playback_restarted(start + Duration::from_millis(49));
        for request in 2..5 {
            assert_eq!(policy.flags(), "relative+exact");
            policy.submitted(request);
            policy.seek_started(start);
            policy.playback_restarted(start + Duration::from_secs(1));
        }
    }

    #[test]
    fn threshold_and_slower_seeks_keep_keyframes_until_the_next_file() {
        for duration in [Duration::from_millis(50), Duration::from_secs(1)] {
            let mut policy = AutoSeek::default();
            let start = Instant::now();
            policy.submitted(1);
            policy.seek_started(start);
            policy.playback_restarted(start + duration);
            policy.interrupt_measurement();
            policy.submitted(2);
            policy.seek_started(start);
            policy.playback_restarted(start + Duration::from_millis(1));
            assert_eq!(policy.flags(), "relative");
            policy = AutoSeek::default();
            assert_eq!(policy.flags(), "relative+exact");
        }
    }

    #[test]
    fn failed_and_superseded_requests_do_not_measure_unrelated_native_seeks() {
        for started in [false, true] {
            let mut policy = AutoSeek::default();
            let start = Instant::now();
            policy.submitted(7);
            if started {
                policy.seek_started(start);
            }
            policy.failed(7);
            policy.seek_started(start);
            policy.playback_restarted(start + Duration::from_secs(1));
            assert_eq!(policy.flags(), "relative+exact");

            policy.submitted(8);
            policy.seek_started(start);
            policy.interrupt_measurement();
            policy.playback_restarted(start + Duration::from_secs(1));
            assert_eq!(policy.flags(), "relative+exact");

            policy.submitted(9);
            policy.failed(8);
            policy.seek_started(start);
            policy.playback_restarted(start + Duration::from_secs(1));
            assert_eq!(policy.flags(), "relative");
        }
    }

    #[test]
    fn initial_load_restart_does_not_decide_the_seek_mode() {
        let mut policy = AutoSeek::default();
        let start = Instant::now();
        policy.submitted(1);
        policy.playback_restarted(start);
        policy.seek_started(start);
        policy.playback_restarted(start + Duration::from_secs(1));
        assert_eq!(policy.flags(), "relative");
    }
}
