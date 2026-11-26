//! Keep the clipboard provider alive, including desktops with selection ownership.
use std::{borrow::Cow, cell::RefCell};

#[cfg(target_os = "linux")]
mod uri_list;
#[cfg(target_os = "linux")]
mod wayland;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "linux")]
mod x11;

thread_local! {
    static CLIPBOARD: RefCell<Option<arboard::Clipboard>> = const { RefCell::new(None) };
}
pub(crate) fn copy_pixels(pixels: &riina_core::CapturedFrame) -> Result<(), String> {
    CLIPBOARD.with(|provider| {
        let mut provider = provider.borrow_mut();
        if provider.is_none() {
            *provider = Some(arboard::Clipboard::new().map_err(|error| error.to_string())?);
        }
        provider
            .as_mut()
            .ok_or("Clipboard is unavailable")?
            .set_image(arboard::ImageData {
                width: pixels.width as usize,
                height: pixels.height as usize,
                bytes: Cow::Borrowed(&pixels.rgba),
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

#[cfg(target_os = "macos")]
fn read_text() -> Result<String, String> {
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
pub(crate) fn copy_playlist_references(
    _window: &slint::Window,
    paths: &[String],
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use objc2::runtime::ProtocolObject;
        use objc2_app_kit::{NSFilenamesPboardType, NSPasteboard, NSPasteboardTypeString};
        use objc2_foundation::{NSArray, NSString, NSURL};

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
        let pasteboard = NSPasteboard::generalPasteboard();
        pasteboard.clearContents();
        if !pasteboard.writeObjects(&url_objects) {
            return Err("Could not write file URLs to the pasteboard".into());
        }
        let legacy_types = NSArray::from_slice(&[filename_type, string_type]);
        // SAFETY: Both entries are exported AppKit pasteboard types. This adds
        // compatibility types to the file-URL items just written; no owner is needed.
        // The return value is the pasteboard change count, with zero on error.
        if unsafe { pasteboard.addTypes_owner(&legacy_types, None) } == 0 {
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
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    {
        let text = paths.join("\n");
        if !paths.is_empty()
            && paths
                .iter()
                .all(|path| std::path::Path::new(path).is_absolute() && !path.contains('\0'))
        {
            #[cfg(target_os = "windows")]
            return windows::copy_file_list(_window, paths, &text);
            #[cfg(target_os = "linux")]
            if let Some(uris) = uri_list::encode(paths)
                && (wayland::copy_file_list(&uris, &text).is_ok()
                    || x11::copy_file_list(uris, &text).is_ok())
            {
                return Ok(());
            }
        }
        copy_text(&text)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        copy_text(&paths.join("\n"))
    }
}

pub(crate) struct PlaylistPaste {
    paths: Vec<String>,
    #[cfg(target_os = "linux")]
    arboard_file_list: bool,
}

impl PlaylistPaste {
    fn new(paths: Vec<String>) -> Self {
        Self {
            paths,
            #[cfg(target_os = "linux")]
            arboard_file_list: false,
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    pub(crate) fn into_paths(self) -> Vec<String> {
        #[cfg(target_os = "linux")]
        if self.arboard_file_list {
            return repair_arboard_file_list(self.paths);
        }
        self.paths
    }
}

#[cfg_attr(target_os = "macos", allow(deprecated))]
pub(crate) fn read_playlist_references() -> Result<PlaylistPaste, String> {
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
                .filter_map(|path| riina_core::playlist::clipboard_reference(&path.to_string()))
                .collect::<Vec<_>>();
            if !paths.is_empty() {
                return Ok(PlaylistPaste::new(paths));
            }
        }
        if let Some(items) = pasteboard.pasteboardItems() {
            let references = (0..items.count())
                .filter_map(|index| {
                    let item = items.objectAtIndex(index);
                    item.stringForType(file_url_type)
                        .or_else(|| item.stringForType(url_type))
                })
                .filter_map(|value| riina_core::playlist::clipboard_reference(&value.to_string()))
                .collect::<Vec<_>>();
            if !references.is_empty() {
                return Ok(PlaylistPaste::new(references));
            }
        }
        read_text()
            .ok()
            .map(|text| riina_core::playlist::clipboard_references(&text))
            .filter(|paths| !paths.is_empty())
            .map(PlaylistPaste::new)
            .ok_or_else(|| "Clipboard contains no playlist references".into())
    }
    #[cfg(not(target_os = "macos"))]
    {
        #[cfg(target_os = "linux")]
        if let Some(paths) = wayland::read_references() {
            return paths.map(PlaylistPaste::new);
        }
        #[cfg(target_os = "linux")]
        if let Some(paths) = x11::read_owned_references() {
            return Ok(PlaylistPaste::new(paths));
        }

        let mut clipboard = arboard::Clipboard::new().map_err(|error| error.to_string())?;
        if let Ok(files) = clipboard.get().file_list()
            && !files.is_empty()
        {
            let paths = files
                .into_iter()
                .map(|path| {
                    path.into_os_string()
                        .into_string()
                        .map_err(|_| "A clipboard path contains invalid Unicode".to_owned())
                })
                .collect::<Result<_, _>>()?;
            return Ok(PlaylistPaste {
                paths,
                #[cfg(target_os = "linux")]
                arboard_file_list: true,
            });
        }
        clipboard
            .get_text()
            .map(|text| PlaylistPaste::new(riina_core::playlist::clipboard_references(&text)))
            .map_err(|error| error.to_string())
    }
}

#[cfg_attr(target_os = "macos", allow(deprecated))]
pub(crate) fn playlist_paste_available() -> bool {
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::{
            NSFilenamesPboardType, NSPasteboard, NSPasteboardTypeFileURL, NSPasteboardTypeString,
            NSPasteboardTypeURL,
        };
        use objc2_foundation::NSArray;

        // SAFETY: AppKit exports these immutable pasteboard type names.
        let types = unsafe {
            [
                NSFilenamesPboardType,
                NSPasteboardTypeFileURL,
                NSPasteboardTypeURL,
                NSPasteboardTypeString,
            ]
        };
        NSPasteboard::generalPasteboard()
            .availableTypeFromArray(&NSArray::from_slice(&types))
            .is_some()
    }
    #[cfg(target_os = "windows")]
    {
        use clipboard_win::{formats, is_format_avail};

        is_format_avail(formats::CF_HDROP) || is_format_avail(formats::CF_UNICODETEXT)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        #[cfg(target_os = "linux")]
        if let Some(available) = wayland::references_available() {
            return available;
        }

        read_playlist_references().is_ok_and(|paths| !paths.is_empty())
    }
}

#[cfg(target_os = "linux")]
fn repair_arboard_file_list(paths: Vec<String>) -> Vec<String> {
    use std::path::Path;
    paths
        .into_iter()
        .map(|path| {
            // arboard 3.6.1 includes the CR of a CRLF separator in file-list paths.
            let repaired = path
                .strip_suffix('\r')
                .filter(|candidate| !Path::new(&path).exists() && Path::new(candidate).exists());
            repaired.map(str::to_owned).unwrap_or(path)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "linux")]
    #[test]
    fn uri_list_repair_preserves_a_real_carriage_return_in_a_linux_filename() {
        let folder = riina_test_support::TempDir::new("clipboard-uri-list").unwrap();
        let ordinary = folder.path().join("ordinary");
        let literal = folder.path().join("literal\r");
        std::fs::write(&ordinary, []).unwrap();
        std::fs::write(&literal, []).unwrap();
        let ordinary = ordinary.to_str().unwrap();
        let literal = literal.to_str().unwrap();
        let paths = vec![
            format!("{ordinary}\r"),
            format!("{literal}\r"),
            literal.into(),
        ];
        assert_eq!(
            super::repair_arboard_file_list(paths),
            [ordinary, literal, literal]
        );
    }

    #[test]
    #[ignore = "writes the original preview fixture to the native clipboard; run explicitly"]
    fn native_clipboard_fixture_round_trip() {
        let path = riina_test_support::fixture("preview-rgb16.png").unwrap();
        let image = slint::Image::load_from_path(&path).unwrap();
        let expected = image.to_rgba8().unwrap();
        let frame = riina_core::CapturedFrame::new(
            expected.width(),
            expected.height(),
            expected.width() as usize * 4,
            expected.as_bytes(),
        )
        .unwrap();
        super::copy_pixels(&frame).unwrap();
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
