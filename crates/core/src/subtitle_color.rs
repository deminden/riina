//! Portable subtitle colors; mpv uses alpha-first hex rather than CSS RGBA.

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct SubtitleColor([f64; 4]);

impl<'de> serde::Deserialize<'de> for SubtitleColor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let channels = <[f64; 4]>::deserialize(deserializer)?;
        Self::from_rgba(channels).map_err(serde::de::Error::custom)
    }
}

impl SubtitleColor {
    pub const WHITE: Self = Self([1.0, 1.0, 1.0, 1.0]);
    pub const BLACK: Self = Self([0.0, 0.0, 0.0, 1.0]);
    pub const CLEAR: Self = Self([0.0, 0.0, 0.0, 0.0]);

    pub fn from_rgba(rgba: [f64; 4]) -> Result<Self, &'static str> {
        if rgba
            .iter()
            .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
        {
            Ok(Self(rgba))
        } else {
            Err("Color channels must be finite and between zero and one")
        }
    }

    pub fn rgba(self) -> [f64; 4] {
        self.0
    }

    pub fn mpv_value(self) -> String {
        let [r, g, b, a] = self.0;
        format!("{r}/{g}/{b}/{a}")
    }

    pub fn from_mpv(value: &str) -> Option<Self> {
        if let Some(hex) = value.strip_prefix('#') {
            // Only native RGB or alpha-first ARGB hex is accepted. Never apply
            // CSS's alpha-last interpretation to an observed engine value.
            if !hex.is_ascii() || !matches!(hex.len(), 6 | 8) {
                return None;
            }
            let number = u32::from_str_radix(hex, 16).ok()?;
            let alpha = if hex.len() == 8 {
                (number >> 24) & 255
            } else {
                255
            };
            return Self::from_rgba([
                ((number >> 16) & 255) as f64 / 255.0,
                ((number >> 8) & 255) as f64 / 255.0,
                (number & 255) as f64 / 255.0,
                alpha as f64 / 255.0,
            ])
            .ok();
        }
        let channels = value
            .split('/')
            .map(str::parse::<f64>)
            .collect::<Result<Vec<_>, _>>()
            .ok()?;
        let rgba = match channels.as_slice() {
            [r, g, b] => [*r, *g, *b, 1.0],
            [r, g, b, a] => [*r, *g, *b, *a],
            _ => return None,
        };
        Self::from_rgba(rgba).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::SubtitleColor;

    #[test]
    fn native_colors_keep_alpha_order_and_fractional_precision() {
        let color = SubtitleColor::from_mpv("#80123456").unwrap();
        assert_eq!(
            color.rgba(),
            [18.0 / 255.0, 52.0 / 255.0, 86.0 / 255.0, 128.0 / 255.0]
        );
        assert_eq!(SubtitleColor::from_mpv("#123456").unwrap().rgba()[3], 1.0);
        let precise = SubtitleColor::from_rgba([0.123456789012345, 0.5, 0.0, 0.25]).unwrap();
        assert_eq!(SubtitleColor::from_mpv(&precise.mpv_value()), Some(precise));
        assert_eq!(SubtitleColor::from_mpv("1/1/1"), Some(SubtitleColor::WHITE));
        assert_eq!(
            SubtitleColor::from_mpv("#00000000"),
            Some(SubtitleColor::CLEAR)
        );
    }

    #[test]
    fn invalid_colors_cannot_create_native_intent() {
        for text in [
            "",
            "#123",
            "#1234567",
            "#é12345",
            "#gg1234",
            "red",
            "1/0",
            "1/0/0/1/0",
            "NaN/0/0",
            "inf/0/0",
            "1.01/0/0",
            "0/0/0/-0.1",
        ] {
            assert_eq!(SubtitleColor::from_mpv(text), None, "{text}");
        }
        assert!(SubtitleColor::from_rgba([0.0, 0.0, f64::NAN, 1.0]).is_err());
    }
}
