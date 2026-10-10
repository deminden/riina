//! Playback speed edits retain double precision across the UI boundary.
use crate::Command;

pub fn slider_position(speed: f64) -> f64 {
    (speed / 0.25).ln() / 64.0_f64.ln() * 24.0
}

pub fn from_slider(position: f64) -> Result<Command, &'static str> {
    if !position.is_finite() {
        return Err("Enter a finite speed position");
    }
    let tick = position.clamp(0.0, 24.0).round();
    let speed = 0.25 * 64.0_f64.powf(tick / 24.0);
    Ok(Command::Speed((speed * 100.0).round() / 100.0))
}

pub fn from_text(text: &str) -> Result<Command, &'static str> {
    let text = text.trim();
    let speed = if text.is_empty() {
        1.0
    } else {
        text.parse::<f64>()
            .map_err(|_| "Enter a numeric playback speed")?
    };
    if !speed.is_finite() || speed < 0.01 {
        return Err("Enter a playback speed of at least 0.01");
    }
    // Positive half-down rounding limits custom precision before dispatch;
    // libmpv can echo an unrounded request despite using six decimal places.
    let speed = (speed * 1_000_000.0 - 0.5).ceil() / 1_000_000.0;
    let command = Command::Speed(speed);
    command
        .mpv_args()
        .map_err(|_| "Playback speed is outside the supported range")?;
    Ok(command)
}

pub fn text(speed: f64) -> String {
    format!("{speed:.6}")
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speed_ticks_cover_reference_octaves_and_round_intermediate_rates() {
        for (tick, speed) in [
            (0.0, 0.25),
            (8.0, 1.0),
            (16.0, 4.0),
            (24.0, 16.0),
            (9.0, 1.19),
        ] {
            assert_eq!(from_slider(tick), Ok(Command::Speed(speed)));
        }
        assert_eq!(slider_position(1.0), 8.0);
        assert_eq!(slider_position(4.0), 16.0);
        assert_eq!(from_slider(8.6), Ok(Command::Speed(1.19)));
        assert!(from_slider(f64::NAN).is_err());
    }

    #[test]
    fn numeric_speed_keeps_six_decimals_and_does_not_snap_to_ticks() {
        for (input, speed) in [
            ("", 1.0),
            ("0.01", 0.01),
            ("1.2345674", 1.234567),
            ("1.2345675", 1.234567),
            ("1.2345676", 1.234568),
            ("32", 32.0),
        ] {
            assert_eq!(from_text(input), Ok(Command::Speed(speed)));
        }
        for invalid in ["NaN", "inf", "-1", "0", "0.009", "bad"] {
            assert!(from_text(invalid).is_err());
        }
        assert_eq!(text(1.0), "1");
        assert_eq!(text(1.234567), "1.234567");
    }
}
