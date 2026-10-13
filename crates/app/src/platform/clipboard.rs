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

pub(crate) fn copy_text(text: &str) -> Result<(), String> {
    CLIPBOARD.with(|provider| {
        let mut provider = provider.borrow_mut();
        if provider.is_none() {
            *provider = Some(arboard::Clipboard::new().map_err(|error| error.to_string())?);
        }
        provider
            .as_mut()
            .ok_or("Clipboard is unavailable")?
            .set_text(text.to_owned())
            .map_err(|error| error.to_string())
    })
}

pub(crate) fn read_text() -> Result<String, String> {
    CLIPBOARD.with(|provider| {
        let mut provider = provider.borrow_mut();
        if provider.is_none() {
            *provider = Some(arboard::Clipboard::new().map_err(|error| error.to_string())?);
        }
        provider
            .as_mut()
            .ok_or("Clipboard is unavailable")?
            .get_text()
            .map_err(|error| error.to_string())
    })
}

#[cfg_attr(target_os = "macos", allow(deprecated))]
pub(crate) fn copy_playlist_references(paths: &[String]) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use objc2::runtime::ProtocolObject;
        use objc2_app_kit::{NSFilenamesPboardType, NSPasteboard, NSPasteboardTypeString};
        use objc2_foundation::{NSArray, NSString, NSURL};

        let pasteboard = NSPasteboard::generalPasteboard();
        pasteboard.clearContents();
        // SAFETY: AppKit exports these immutable pasteboard type constants for
        // the lifetime of the process.
        let (filename_type, string_type) =
            unsafe { (NSFilenamesPboardType, NSPasteboardTypeString) };
        let urls = paths
            .iter()
            .map(|path| {
                let has_scheme = path.split_once(':').is_some_and(|(scheme, _)| {
                    !scheme.is_empty()
                        && scheme.chars().enumerate().all(|(index, character)| {
                            character.is_ascii_alphabetic()
                                || (index > 0
                                    && (character.is_ascii_digit()
                                        || matches!(character, '+' | '.' | '-')))
                        })
                });
                if has_scheme {
                    NSURL::URLWithString(&NSString::from_str(path))
                } else {
                    Some(NSURL::fileURLWithPath(&NSString::from_str(path)))
                }
            })
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| "A playlist reference could not be represented as a URL".to_owned())?;
        let url_objects = urls
            .iter()
            .map(|url| ProtocolObject::from_ref(&**url))
            .collect::<Vec<_>>();
        let url_objects = NSArray::from_slice(&url_objects);
        if !pasteboard.writeObjects(&url_objects) {
            return Err("Could not write file URLs to the pasteboard".into());
        }
        let legacy_types = NSArray::from_slice(&[filename_type, string_type]);
        // SAFETY: Both entries are exported AppKit pasteboard types. This adds
        // compatibility types to the file-URL items just written; no owner is needed.
        if unsafe { pasteboard.addTypes_owner(&legacy_types, None) } != 2 {
            return Err("Could not add compatible playlist formats to the pasteboard".into());
        }
        let strings = paths
            .iter()
            .map(|path| NSString::from_str(path))
            .collect::<Vec<_>>();
        let string_refs = strings.iter().map(|path| &**path).collect::<Vec<_>>();
        let filenames = NSArray::from_slice(&string_refs);
        let filenames = filenames.into_super().into_super();
        // SAFETY: NSFilenamesPboardType requires a property-list array of path
        // strings, which `filenames` contains and retains for this call.
        let filenames_written =
            unsafe { pasteboard.setPropertyList_forType(&filenames, filename_type) };
        let text = NSString::from_str(&paths.join("\n"));
        if !filenames_written || !pasteboard.setString_forType(&text, string_type) {
            return Err("Could not write playlist references to the pasteboard".into());
        }
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        copy_text(&paths.join("\n"))
    }
}

#[cfg_attr(target_os = "macos", allow(deprecated))]
pub(crate) fn read_playlist_references() -> Result<Vec<String>, String> {
    #[cfg(target_os = "macos")]
    {
        use objc2::runtime::AnyObject;
        use objc2_app_kit::{
            NSFilenamesPboardType, NSPasteboard, NSPasteboardTypeFileURL, NSPasteboardTypeURL,
        };
        use objc2_foundation::{NSArray, NSString};

        let pasteboard = NSPasteboard::generalPasteboard();
        // SAFETY: AppKit exports these immutable pasteboard type constants for
        // the lifetime of the process.
        let (filename_type, file_url_type, url_type) = unsafe {
            (
                NSFilenamesPboardType,
                NSPasteboardTypeFileURL,
                NSPasteboardTypeURL,
            )
        };
        if let Some(property_list) = pasteboard.propertyListForType(filename_type)
            && let Ok(filenames) = property_list.downcast::<NSArray<AnyObject>>()
        {
            let paths = (0..filenames.count())
                .filter_map(|index| filenames.objectAtIndex(index).downcast::<NSString>().ok())
                .map(|path| path.to_string())
                .collect::<Vec<_>>();
            if !paths.is_empty() {
                return Ok(paths);
            }
        }
        let file_urls = pasteboard
            .pasteboardItems()
            .map(|items| {
                (0..items.count())
                    .filter_map(|index| {
                        let item = items.objectAtIndex(index);
                        item.stringForType(file_url_type)
                            .or_else(|| item.stringForType(url_type))
                    })
                    .map(|value| value.to_string())
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .filter(|references| !references.is_empty());
        let text = file_urls.or_else(|| read_text().ok());
        text.map(|text| riina_core::playlist::clipboard_references(&text))
            .filter(|paths| !paths.is_empty())
            .ok_or_else(|| "Clipboard contains no playlist references".into())
    }
    #[cfg(not(target_os = "macos"))]
    {
        read_text().map(|text| riina_core::playlist::clipboard_references(&text))
    }
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
