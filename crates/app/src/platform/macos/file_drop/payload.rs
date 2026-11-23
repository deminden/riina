use crate::platform::desktop_drop::parse_references;
use objc2::runtime::AnyObject;
use objc2_app_kit::{
    NSPasteboard, NSPasteboardType, NSPasteboardTypeFileURL, NSPasteboardTypeString,
    NSPasteboardTypeURL, NSURLNSPasteboardSupport,
};
use objc2_foundation::{NSArray, NSString, NSURL};
use std::{ffi::OsString, path::Path};

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
    let files = item_references(board, file_url);
    if !files.is_empty() {
        return files;
    }
    if let Some(names) = board
        .propertyListForType(filenames)
        .and_then(|list| list.downcast::<NSArray<AnyObject>>().ok())
    {
        let paths: Vec<_> = names
            .iter()
            .filter_map(|name| name.downcast_ref::<NSString>().map(ToString::to_string))
            .filter(|name| !name.contains('\0') && Path::new(name).is_absolute())
            .map(OsString::from)
            .collect();
        if !paths.is_empty() {
            return paths;
        }
    }
    let links = item_references(board, url);
    if !links.is_empty() {
        return links;
    }
    if board
        .availableTypeFromArray(&NSArray::from_slice(&[legacy_url]))
        .is_some()
        && let Some(url) = NSURL::URLFromPasteboard(board).and_then(|url| url.absoluteString())
    {
        let links = parse_references(&url.to_string(), false);
        if !links.is_empty() {
            return links;
        }
    }
    board
        .stringForType(text)
        .map_or_else(Vec::new, |text| parse_references(&text.to_string(), true))
}

fn item_references(board: &NSPasteboard, kind: &NSPasteboardType) -> Vec<OsString> {
    board.pasteboardItems().map_or_else(Vec::new, |items| {
        items
            .iter()
            .filter_map(|item| item.stringForType(kind))
            .flat_map(|value| parse_references(&value.to_string(), false))
            .collect()
    })
}
