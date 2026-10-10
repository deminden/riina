//! Keep the clipboard provider alive, including desktops with selection ownership.
use std::{borrow::Cow, cell::RefCell};
thread_local! {
    static CLIPBOARD: RefCell<Option<arboard::Clipboard>> = const { RefCell::new(None) };
}
pub(crate) fn copy_image(image: &slint::Image) -> Result<(), String> {
    let pixels = image
        .to_rgba8()
        .ok_or("Screenshot pixels are unavailable")?;
    CLIPBOARD.with(|provider| {
        let mut provider = provider.borrow_mut();
        if provider.is_none() {
            *provider = Some(arboard::Clipboard::new().map_err(|error| error.to_string())?);
        }
        provider
            .as_mut()
            .ok_or("Clipboard is unavailable")?
            .set_image(arboard::ImageData {
                width: pixels.width() as usize,
                height: pixels.height() as usize,
                bytes: Cow::Borrowed(pixels.as_bytes()),
            })
            .map_err(|error| error.to_string())
    })
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "writes the original preview fixture to the native clipboard; run explicitly"]
    fn native_clipboard_fixture_round_trip() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/preview-rgb16.png");
        let image = slint::Image::load_from_path(&path).unwrap();
        super::copy_image(&image).unwrap();
        let expected = image.to_rgba8().unwrap();
        let actual = arboard::Clipboard::new().unwrap().get_image().unwrap();
        assert_eq!((actual.width, actual.height), (96, 64));
        assert!(
            actual.bytes.as_ref() == expected.as_bytes(),
            "Clipboard pixels differ from fixture"
        );
    }
    #[test]
    #[ignore = "requires clipboard containing a test capture and RIINA_SCREENSHOT_REFERENCE saved frame"]
    fn native_clipboard_pixels_match_saved_reference() {
        let path =
            std::env::var_os("RIINA_SCREENSHOT_REFERENCE").expect("Saved test frame required");
        let expected = slint::Image::load_from_path(std::path::Path::new(&path))
            .unwrap()
            .to_rgba8()
            .unwrap();
        let actual = arboard::Clipboard::new().unwrap().get_image().unwrap();
        assert_eq!(
            (actual.width, actual.height),
            (expected.width() as usize, expected.height() as usize)
        );
        assert!(
            actual.bytes.as_ref() == expected.as_bytes(),
            "Clipboard pixels differ from saved reference"
        );
    }
}
