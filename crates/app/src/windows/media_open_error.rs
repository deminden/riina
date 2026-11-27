//! Open failures keep the application alive until the user dismisses the alert.
use super::close_observer::CloseObserver;
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    rc::Rc,
};

#[derive(Default)]
pub(crate) struct OpenErrors {
    active: Cell<bool>,
    pending: RefCell<VecDeque<(String, bool)>>,
    pub(crate) closed: CloseObserver,
}

impl OpenErrors {
    pub(crate) fn is_active(&self) -> bool {
        self.active.get()
    }

    pub(crate) fn report(self: &Rc<Self>, detail: String, dark: bool) {
        self.pending.borrow_mut().push_back((detail, dark));
        if self.active.replace(true) {
            return;
        }
        let owner = self.clone();
        if let Err(error) = slint::spawn_local(async move { owner.show_pending().await }) {
            self.pending.borrow_mut().clear();
            self.active.set(false);
            crate::diagnostics::warn!("Could not present the media opening error: {error}");
        }
    }

    async fn show_pending(self: Rc<Self>) {
        // A set of separate opens can fail together. Show one alert at a time.
        while let Some((detail, dark)) = self.next() {
            show(&detail, dark).await;
        }
        self.active.set(false);
        self.closed.notify();
    }

    fn next(&self) -> Option<(String, bool)> {
        self.pending.borrow_mut().pop_front()
    }
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
async fn show(detail: &str, _dark: bool) {
    rfd::AsyncMessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_title("Cannot open this file or stream.")
        .set_description(detail)
        .set_buttons(rfd::MessageButtons::Ok)
        .show()
        .await;
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
async fn show(detail: &str, dark: bool) {
    if let Err(error) = show_desktop_alert(detail, dark).await {
        crate::diagnostics::warn!("Could not show the media opening error: {error}");
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
async fn show_desktop_alert(detail: &str, dark: bool) -> Result<(), slint::PlatformError> {
    use riina_ui::MediaOpenErrorWindow;
    use slint::{CloseRequestResponse, ComponentHandle};

    let window = MediaOpenErrorWindow::new()?;
    crate::appearance_preferences::install_system_typography(&window);
    window.set_dark_theme(dark);
    window.set_detail(detail.into());
    let (dismiss, response) = tokio::sync::oneshot::channel();
    let dismiss = Rc::new(Cell::new(Some(dismiss)));
    let close = move || {
        if let Some(dismiss) = dismiss.take() {
            let _ = dismiss.send(());
        }
    };
    window.on_dismiss(close.clone());
    window.window().on_close_requested(move || {
        close();
        CloseRequestResponse::HideWindow
    });
    crate::platform::show_window(&window)?;
    window.invoke_focus_default();
    let _ = response.await;
    window.hide()
}
