//! Keep AppKit's recent-document list across changes to the bundle signature.
use objc2::{MainThreadMarker, rc::Retained, runtime::AnyObject};
use objc2_app_kit::NSDocumentController;
use objc2_foundation::{
    NSArray, NSData, NSObject, NSString, NSURL, NSURLBookmarkCreationOptions,
    NSURLBookmarkResolutionOptions, NSURLPathKey, NSUserDefaults,
};
use std::path::PathBuf;

fn workaround_enabled(defaults: &NSUserDefaults) -> bool {
    defaults.boolForKey(objc2_foundation::ns_string!(
        "enableRecentDocumentsWorkaround"
    ))
}

fn backup_key() -> &'static NSString {
    objc2_foundation::ns_string!("recentDocumentBookmarks")
}

pub(in crate::platform) fn remember(
    paths: impl IntoIterator<Item = PathBuf>,
) -> std::io::Result<()> {
    let main = MainThreadMarker::new().ok_or_else(|| {
        std::io::Error::other("Recent documents must be updated on the macOS main thread")
    })?;
    let controller = NSDocumentController::sharedDocumentController(main);
    let mut changed = false;
    for path in paths {
        let Ok(path) = path.canonicalize() else {
            continue;
        };
        let directory = path.is_dir();
        if directory || path.is_file() {
            let path = NSString::from_str(&path.to_string_lossy());
            let url = NSURL::fileURLWithPath_isDirectory(&path, directory);
            controller.noteNewRecentDocumentURL(&url);
            changed = true;
        }
    }
    let defaults = NSUserDefaults::standardUserDefaults();
    if changed && workaround_enabled(&defaults) {
        save(&controller.recentDocumentURLs(), &defaults);
    }
    Ok(())
}

pub(in crate::platform) fn clear() -> std::io::Result<()> {
    let main = MainThreadMarker::new().ok_or_else(|| {
        std::io::Error::other("Recent documents must be cleared on the macOS main thread")
    })?;
    NSUserDefaults::standardUserDefaults().removeObjectForKey(backup_key());
    // SAFETY: AppKit accepts a nil sender for a programmatic clear on the main thread.
    unsafe { NSDocumentController::sharedDocumentController(main).clearRecentDocuments(None) };
    Ok(())
}

pub(in crate::platform) fn restore(main: MainThreadMarker) {
    let defaults = NSUserDefaults::standardUserDefaults();
    if !workaround_enabled(&defaults) {
        return;
    }
    let controller = NSDocumentController::sharedDocumentController(main);
    let limit = controller.maximumRecentDocumentCount();
    if limit == 0 {
        return;
    }
    let backup = defaults.arrayForKey(backup_key());
    let current = controller.recentDocumentURLs();
    if !current.is_empty() {
        if backup.is_none() {
            save(&current, &defaults);
        }
        return;
    }
    let Some(backup) = backup else {
        return;
    };
    let urls = backup
        .iter()
        .filter_map(restore_url)
        .take(limit)
        .collect::<Vec<_>>();
    // AppKit gives each added document first place in the recent list.
    for url in urls.iter().rev() {
        controller.noteNewRecentDocumentURL(url);
    }
    // Keep the saved bookmarks until a real document open. Recapturing a
    // recovered path here would request metadata from an unavailable volume.
}

fn save(urls: &NSArray<NSURL>, defaults: &NSUserDefaults) {
    if urls.is_empty() {
        return;
    }
    let keys = path_keys();
    let bookmarks: Vec<Retained<NSObject>> = urls
        .iter()
        .filter(|url| url.isFileURL())
        .filter_map(|url| {
            url.bookmarkDataWithOptions_includingResourceValuesForKeys_relativeToURL_error(
                NSURLBookmarkCreationOptions::WithoutImplicitSecurityScope,
                Some(&keys),
                None,
            )
            .ok()
            .map(Retained::into_super)
            .or_else(|| url.absoluteString().map(Retained::into_super))
        })
        .collect();
    let bookmarks = NSArray::from_retained_slice(&bookmarks);
    // SAFETY: the array contains immutable property-list data and strings.
    unsafe { defaults.setObject_forKey(Some(&bookmarks), backup_key()) };
}

fn restore_url(value: Retained<AnyObject>) -> Option<Retained<NSURL>> {
    match value.downcast::<NSData>() {
        Ok(bookmark) => resolve_bookmark(&bookmark),
        Err(value) => {
            let url = value.downcast::<NSString>().ok()?;
            NSURL::URLWithString(&url)
        }
    }
    .filter(|url| url.isFileURL())
}

fn resolve_bookmark(data: &NSData) -> Option<Retained<NSURL>> {
    // SAFETY: the bookmark is retained for the call. A null stale pointer is supported.
    let result = unsafe {
        NSURL::URLByResolvingBookmarkData_options_relativeToURL_bookmarkDataIsStale_error(
            data,
            NSURLBookmarkResolutionOptions::WithoutUI
                | NSURLBookmarkResolutionOptions::WithoutMounting
                | NSURLBookmarkResolutionOptions::WithoutImplicitStartAccessing,
            None,
            std::ptr::null_mut(),
        )
    };
    result.ok().or_else(|| {
        let keys = path_keys();
        let values = NSURL::resourceValuesForKeys_fromBookmarkData(&keys, data)?;
        // SAFETY: NSURLPathKey is an immutable system constant.
        let path = values
            .objectForKey(unsafe { NSURLPathKey })?
            .downcast::<NSString>()
            .ok()?;
        Some(NSURL::fileURLWithPath_isDirectory(&path, false))
    })
}

fn path_keys() -> Retained<NSArray<NSString>> {
    // SAFETY: NSURLPathKey is an immutable system constant.
    NSArray::from_slice(&[unsafe { NSURLPathKey }])
}
