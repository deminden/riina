//! Parsing of absolute playback positions entered by a user.

/// Parse seconds, `minutes:seconds`, or `hours:minutes:seconds` into seconds.
///
/// The last field may contain fractional seconds. Intermediate fields follow
/// IINA's time-entry convention and are arithmetic components rather than
/// clock fields constrained to 0..60.
pub fn parse_video_time(input: &str) -> Option<f64> {
    let fields = input.trim().split(':').collect::<Vec<_>>();
    if fields.is_empty() || fields.len() > 3 || fields.iter().any(|field| field.is_empty()) {
        return None;
    }

    let last = fields.len() - 1;
    let seconds = fields[last].parse::<f64>().ok()?;
    if !seconds.is_finite() || seconds < 0.0 {
        return None;
    }

    let mut total = seconds;
    if last >= 1 {
        let minutes = fields[last - 1].parse::<u64>().ok()?;
        total += minutes as f64 * 60.0;
    }
    if last >= 2 {
        let hours = fields[last - 2].parse::<u64>().ok()?;
        total += hours as f64 * 3600.0;
    }
    total.is_finite().then_some(total)
}

#[cfg(test)]
mod tests {
    use super::parse_video_time;

    #[test]
    fn parses_seconds_minutes_and_hours_with_fractional_seconds() {
        assert_eq!(parse_video_time("35.25"), Some(35.25));
        assert_eq!(parse_video_time("20:35"), Some(1235.0));
        assert_eq!(parse_video_time("1:02:03.5"), Some(3723.5));
    }

    #[test]
    fn rejects_malformed_negative_and_non_finite_times() {
        for input in ["", ":20", "20:", "1:2:3:4", "-1", "NaN", "inf", "x:20"] {
            assert_eq!(parse_video_time(input), None, "{input:?}");
        }
    }

    #[test]
    fn intermediate_components_are_duration_components_not_clock_ranges() {
        assert_eq!(parse_video_time("70:80"), Some(4280.0));
    }
}
