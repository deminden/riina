//! Use one AppKit picker for mixed file and folder selections.
use block2::RcBlock;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSModalResponseAbort, NSModalResponseOK, NSOpenPanel};
use objc2_foundation::{NSString, NSURL};
use slint::winit_030::winit::window::Window;
use std::{cell::RefCell, ffi::CStr};

pub(crate) async fn choose_media(
    title: &str,
    owner: Option<&Window>,
) -> Result<Option<Vec<String>>, String> {
    let main = MainThreadMarker::new().ok_or("The Open panel must run on the main thread")?;
    let panel = NSOpenPanel::openPanel(main);
    panel.setTitle(Some(&NSString::from_str(title)));
    panel.setCanChooseFiles(true);
    panel.setCanChooseDirectories(true);
    panel.setCanCreateDirectories(false);
    panel.setAllowsMultipleSelection(true);
    let (finished, result) = tokio::sync::oneshot::channel();
    let finished = RefCell::new(Some(finished));
    let completion = RcBlock::new(move |response| {
        if let Some(finished) = finished.take() {
            let _ = finished.send(response);
        }
    });
    if let Some(owner) = owner {
        let parent = super::view::content_view(owner)
            .and_then(objc2_app_kit::NSView::window)
            .ok_or("The parent window is unavailable")?;
        panel.beginSheetModalForWindow_completionHandler(&parent, &completion);
    } else {
        panel.beginWithCompletionHandler(&completion);
    }
    let response = result.await.map_err(|error| error.to_string())?;
    if response == NSModalResponseAbort {
        return Err("Could not show the Open panel".into());
    }
    if response != NSModalResponseOK {
        return Ok(None);
    }
    let paths = panel
        .URLs()
        .iter()
        .map(|url| file_path(&url))
        .collect::<Result<Vec<_>, _>>()?;
    Ok((!paths.is_empty()).then_some(paths))
}

pub(in crate::platform) fn file_path(url: &NSURL) -> Result<String, &'static str> {
    if !url.isFileURL() {
        return Err("The selection contains a non-file reference");
    }
    // SAFETY: the file URL supplies a NUL-terminated path which is copied
    // before the caller releases the URL.
    unsafe { CStr::from_ptr(url.fileSystemRepresentation().as_ptr()) }
        .to_str()
        .map(str::to_owned)
        .map_err(|_| "The selection contains a path with invalid Unicode")
}
