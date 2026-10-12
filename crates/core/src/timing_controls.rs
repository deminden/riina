//! Numeric timing values shared by the audio and subtitle controls.

/// Parse a delay in seconds. Empty input matches the reference field's reset-to-zero behavior.
pub fn parse_delay_seconds(text: &str) -> Result<f64, &'static str> {
    let text = text.trim();
    let delay = if text.is_empty() {
        0.0
    } else {
        text.parse::<f64>()
            .map_err(|_| "Enter a numeric delay in seconds")?
    };
    if !delay.is_finite() {
        return Err("Enter a finite delay in seconds");
    }
    Ok(delay)
}

#[cfg(test)]
mod tests {
    use super::parse_delay_seconds;

    #[test]
    fn delay_entry_accepts_precise_seconds_and_empty_reset() {
        assert_eq!(parse_delay_seconds(" 0.05 "), Ok(0.05));
        assert_eq!(parse_delay_seconds("-12.345678901"), Ok(-12.345678901));
        assert_eq!(parse_delay_seconds(""), Ok(0.0));
        assert_eq!(parse_delay_seconds("   "), Ok(0.0));
    }

    #[test]
    fn delay_entry_rejects_invalid_and_non_finite_values() {
        for text in ["abc", "NaN", "inf", "-inf"] {
            assert!(parse_delay_seconds(text).is_err(), "{text:?}");
        }
    }
}
