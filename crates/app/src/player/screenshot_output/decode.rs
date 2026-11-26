//! Decode owned screenshot files without involving Slint or the active player.
use riina_core::CapturedFrame;
use std::{path::Path, sync::Arc};

pub(super) fn load(path: &Path) -> Option<CapturedFrame> {
    let extension = path.extension()?;
    if extension.eq_ignore_ascii_case("jxl") || extension.eq_ignore_ascii_case("avif") {
        return riina_mpv::decode_screenshot(&crate::platform::playback_library(), path);
    }
    let mut reader = image::ImageReader::open(path).ok()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(16_384);
    limits.max_image_height = Some(16_384);
    reader.limits(limits);
    let pixels = reader.decode().ok()?.into_rgba8();
    Some(CapturedFrame {
        width: pixels.width(),
        height: pixels.height(),
        rgba: Arc::from(pixels.into_raw()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deep_rgb_png_loads_through_the_actual_preview_decoder() {
        let frame = load(&riina_test_support::fixture("preview-rgb16.png").unwrap()).unwrap();
        assert_eq!((frame.width, frame.height), (96, 64));
        let bars = [
            [0, 0, 0],
            [255, 255, 255],
            [255, 0, 0],
            [0, 255, 0],
            [0, 0, 255],
            [128, 128, 128],
            [0, 255, 255],
            [255, 0, 255],
        ];
        for (index, expected) in bars.into_iter().enumerate() {
            let offset = 4 * (32 * 96 + index * 12 + 6);
            assert_eq!(
                frame.rgba[offset..offset + 4],
                [expected[0], expected[1], expected[2], 255]
            );
        }
    }
}
