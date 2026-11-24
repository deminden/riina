use super::ArrowAction;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
#[repr(i32)]
pub enum TimePrecision {
    #[default]
    Seconds,
    Tenths,
    Hundredths,
    Milliseconds,
}

impl TimePrecision {
    #[must_use]
    pub fn from_index(index: i32) -> Option<Self> {
        let index = usize::try_from(index).ok()?;
        [
            Self::Seconds,
            Self::Tenths,
            Self::Hundredths,
            Self::Milliseconds,
        ]
        .get(index)
        .copied()
    }

    #[must_use]
    pub const fn index(self) -> i32 {
        self as i32
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct ControllerOptions {
    #[serde(skip_serializing_if = "ArrowAction::is_default")]
    pub arrow_action: ArrowAction,
    pub stick_to_center: bool,
    pub show_remaining_time: bool,
    pub scale_remaining_time: bool,
    pub time_precision: TimePrecision,
}

impl Default for ControllerOptions {
    fn default() -> Self {
        Self {
            arrow_action: ArrowAction::default(),
            stick_to_center: true,
            show_remaining_time: false,
            scale_remaining_time: false,
            time_precision: TimePrecision::Seconds,
        }
    }
}

impl ControllerOptions {
    #[must_use]
    pub fn elapsed_label(self, elapsed: Option<f64>) -> String {
        clock_label(elapsed, self.time_precision, false)
    }

    #[must_use]
    pub fn end_label(self, playback: &crate::Playback) -> String {
        let seconds = match (self.show_remaining_time, self.scale_remaining_time) {
            (false, _) => playback.duration,
            (true, false) => playback.remaining_time,
            (true, true) => playback.scaled_remaining_time,
        };
        clock_label(seconds, self.time_precision, self.show_remaining_time)
    }
}

fn clock_label(seconds: Option<f64>, precision: TimePrecision, remaining: bool) -> String {
    let Some(seconds) = seconds.filter(|value| value.is_finite() && *value >= 0.0) else {
        return "--:--".into();
    };
    let digits = precision as u32;
    let scale = 10_u64.pow(digits);
    let seconds = seconds.min(359_999_999.0);
    // At whole seconds IINA matches Now Playing's half-down rounding.
    let ticks = if precision == TimePrecision::Seconds {
        let floor = seconds.floor();
        floor as u64 + u64::from(seconds - floor > 0.5)
    } else {
        (seconds * scale as f64).round_ties_even() as u64
    };
    let whole = ticks / scale;
    let (hours, minutes, seconds) = (whole / 3600, whole / 60 % 60, whole % 60);
    let sign = if remaining { "-" } else { "" };
    let mut label = if hours > 0 {
        format!("{sign}{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{sign}{minutes:02}:{seconds:02}")
    };
    if digits > 0 {
        use std::fmt::Write;
        let fraction = ticks % scale;
        let width = digits as usize;
        let _ = write!(label, ".{fraction:0width$}");
    }
    label
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controller_clock_rounds_and_uses_the_selected_native_remaining_value() {
        let mut options = ControllerOptions::default();
        let mut playback = crate::Playback {
            duration: Some(123.5),
            remaining_time: Some(51.5),
            scaled_remaining_time: Some(25.75),
            ..Default::default()
        };
        assert_eq!(options.end_label(&playback), "02:03");
        assert_eq!(options.elapsed_label(Some(59.51)), "01:00");
        options.show_remaining_time = true;
        options.time_precision = TimePrecision::Hundredths;
        assert_eq!(options.end_label(&playback), "-00:51.50");
        options.scale_remaining_time = true;
        assert_eq!(options.end_label(&playback), "-00:25.75");
        assert_eq!(options.elapsed_label(Some(3599.999)), "1:00:00.00");
        assert_eq!(options.elapsed_label(Some(11.1)), "00:11.10");
        playback.begin_load();
        assert_eq!(options.end_label(&playback), "--:--");
        assert_eq!(options.elapsed_label(Some(f64::NAN)), "--:--");
    }
}
