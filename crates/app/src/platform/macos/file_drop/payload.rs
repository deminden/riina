use crate::platform::desktop_drop::{parse_reference, parse_references};
use objc2::runtime::AnyObject;
use objc2_app_kit::{
    NSPasteboard, NSPasteboardType, NSPasteboardTypeFileURL, NSPasteboardTypeString,
    NSPasteboardTypeURL, NSURLNSPasteboardSupport,
};
use objc2_foundation::{NSArray, NSString, NSURL};
use std::ffi::OsString;

#[allow(deprecated)]
pub(super) fn types() -> [&'static NSPasteboardType; 5] {
    // SAFETY: AppKit exports these immutable pasteboard type constants.
    unsafe {
        [
            NSPasteboardTypeFileURL,
            objc2_app_kit::NSFilenamesPboardType,
            NSPasteboardTypeURL,
            objc2_app_kit::NSURLPboardType,
            NSPasteboardTypeString,
        ]
    }
}

pub(super) fn read(board: &NSPasteboard) -> Vec<OsString> {
    let [file_url, filenames, url, legacy_url, text] = types();
    let items = item_references(board, file_url, url);
    // IINA uses this array for whole playlist selections, including streams.
    if let Some(names) = board
        .propertyListForType(filenames)
        .and_then(|list| list.downcast::<NSArray<AnyObject>>().ok())
    {
        let paths: Vec<_> = names
            .iter()
            .filter_map(|name| name.downcast_ref::<NSString>().map(ToString::to_string))
            .filter_map(|name| parse_reference(&name, true))
            .map(OsString::from)
            .collect();
        if paths.len() > items.len() {
            return paths;
        }
    }
    if !items.is_empty() {
        return items;
    }
    if board
        .availableTypeFromArray(&NSArray::from_slice(&[legacy_url]))
        .is_some()
        && let Some(url) = NSURL::URLFromPasteboard(board).and_then(|url| url.absoluteString())
        && let Some(reference) = parse_reference(&url.to_string(), false)
    {
        return vec![reference.into()];
    }
    board
        .stringForType(text)
        .map_or_else(Vec::new, |text| parse_references(&text.to_string(), true))
}

fn item_references(
    board: &NSPasteboard,
    file_url: &NSPasteboardType,
    url: &NSPasteboardType,
) -> Vec<OsString> {
    board.pasteboardItems().map_or_else(Vec::new, |items| {
        items
            .iter()
            .filter_map(|item| {
                [file_url, url]
                    .into_iter()
                    .filter_map(|kind| item.stringForType(kind))
                    .find_map(|value| parse_reference(&value.to_string(), false))
            })
            .map(OsString::from)
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use objc2::runtime::ProtocolObject;
    use objc2_app_kit::NSPasteboardItem;

    #[test]
    fn mixed_pasteboard_items_and_the_iina_filename_array_keep_their_order() {
        let board = NSPasteboard::pasteboardWithUniqueName();
        let [file_url, filenames, url, ..] = types();
        let link = "https://example.test/片 one.mkv?sig=A%2fb+Z";
        let encoded = "https://example.test/%E7%89%87%20one.mkv?sig=A%2fb+Z";
        let stream = NSPasteboardItem::new();
        assert!(stream.setString_forType(&NSString::from_str(link), url));
        let file = NSPasteboardItem::new();
        assert!(file.setString_forType(&NSString::from_str("file:///tmp/a%0Ab.mkv"), file_url));
        let objects = [
            ProtocolObject::from_ref(&*stream),
            ProtocolObject::from_ref(&*file),
        ];
        assert!(board.writeObjects(&NSArray::from_slice(&objects)));
        assert_eq!(read(&board), [encoded, "/tmp/a\nb.mkv"]);

        board.clearContents();
        let paths = NSArray::from_retained_slice(&[
            NSString::from_str("/tmp/a\nb.mkv"),
            NSString::from_str(link),
        ]);
        // SAFETY: the private board declares a valid format with no deferred owner.
        let declared =
            unsafe { board.declareTypes_owner(&NSArray::from_slice(&[filenames]), None) };
        assert_ne!(declared, 0);
        // SAFETY: the private board receives a retained property-list array of strings.
        assert!(unsafe {
            board.setPropertyList_forType(&paths.into_super().into_super(), filenames)
        });
        assert_eq!(read(&board), ["/tmp/a\nb.mkv", encoded]);
        board.clearContents();
    }
}
