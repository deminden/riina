use objc2::rc::Retained;
use objc2_app_kit::{NSView, NSWindow};
use slint::winit_030::winit::window::Window;

pub(crate) struct Modal {
    owner: Retained<NSWindow>,
    sheet: Retained<NSWindow>,
}

impl Drop for Modal {
    fn drop(&mut self) {
        if self
            .sheet
            .sheetParent()
            .is_some_and(|parent| std::ptr::eq(&*parent, &*self.owner))
        {
            self.owner.endSheet(&self.sheet);
        }
    }
}

pub(crate) fn attach(owner: &Window, picker: &Window) -> Result<Modal, String> {
    let (Some(owner), Some(sheet)) = (native_window(owner), native_window(picker)) else {
        return Err("The Settings font chooser window is unavailable".into());
    };
    if owner.attachedSheet().is_some() {
        return Err("Close the current Settings dialog before choosing a font".into());
    }
    // Showing through Slint realizes the native window. AppKit then displays it as a sheet.
    sheet.orderOut(None);
    owner.beginSheet_completionHandler(&sheet, None);
    Ok(Modal { owner, sheet })
}

fn native_window(window: &Window) -> Option<Retained<NSWindow>> {
    super::super::macos::view::content_view(window).and_then(NSView::window)
}
