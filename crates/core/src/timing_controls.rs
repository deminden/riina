//! Numeric timing values shared by the audio and subtitle controls.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DelayInputError {
    InvalidNumber,
    NonFinite,
}

impl std::fmt::Display for DelayInputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidNumber => "Enter a numeric delay in seconds",
            Self::NonFinite => "Enter a finite delay in seconds",
        })
    }
}

impl std::error::Error for DelayInputError {}

/// Parse a delay in seconds. Empty input matches the reference field's reset-to-zero behavior.
pub fn parse_delay_seconds(text: &str) -> Result<f64, DelayInputError> {
    let text = text.trim();
    let delay = if text.is_empty() {
        0.0
    } else {
        text.parse::<f64>()
            .map_err(|_| DelayInputError::InvalidNumber)?
    };
    if !delay.is_finite() {
        return Err(DelayInputError::NonFinite);
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
