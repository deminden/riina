//! Keep the Video & Audio preference selection in sync with the native player.

use riina_core::video_controls::HardwareDecoder;

pub(crate) fn hardware_decoder(index: i32) -> Option<HardwareDecoder> {
    match index {
        0 => Some(HardwareDecoder::Disabled),
        1 => Some(HardwareDecoder::Auto),
        2 => Some(HardwareDecoder::AutoCopy),
        _ => None,
    }
}

pub(crate) const fn index(mode: HardwareDecoder) -> i32 {
    match mode {
        HardwareDecoder::Disabled => 0,
        HardwareDecoder::Auto => 1,
        HardwareDecoder::AutoCopy => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::{hardware_decoder, index};
    use riina_core::video_controls::HardwareDecoder;

    #[test]
    fn source_menu_tags_round_trip_without_changing_the_default() {
        for (index, mode) in [
            HardwareDecoder::Disabled,
            HardwareDecoder::Auto,
            HardwareDecoder::AutoCopy,
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(hardware_decoder(index as i32), Some(mode));
            assert_eq!(self::index(mode), index as i32);
        }
        assert_eq!(HardwareDecoder::default(), HardwareDecoder::Auto);
        assert_eq!(hardware_decoder(-1), None);
        assert_eq!(hardware_decoder(3), None);
    }
}
