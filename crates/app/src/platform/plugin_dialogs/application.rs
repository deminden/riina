//! Application dialogs can run independently of any media window.
use super::{interaction, pick_request};
use crate::{plugin_runtime::utilities::DialogRequest, windows::close_observer::CloseObserver};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_OWNER: AtomicU64 = AtomicU64::new(0);
thread_local! {
    static OWNER: RefCell<Option<(u64, Weak<Dialogs>)>> = const { RefCell::new(None) };
}

#[derive(Default)]
pub(super) struct Dialogs {
    active: Cell<bool>,
    closed: RefCell<Option<Rc<CloseObserver>>>,
}

impl Dialogs {
    pub(super) fn bind_to_thread(self: &Rc<Self>) -> u64 {
        let id = NEXT_OWNER.fetch_add(1, Ordering::Relaxed);
        OWNER.with(|slot| slot.replace(Some((id, Rc::downgrade(self)))));
        id
    }

    #[cfg(target_os = "macos")]
    pub(super) fn bind_close(&self, callback: Rc<CloseObserver>) {
        self.closed.replace(Some(callback));
    }

    #[cfg(target_os = "macos")]
    pub(super) fn is_visible(&self) -> bool {
        self.active.get()
    }
}

pub(super) fn present(id: u64, request: DialogRequest) {
    let owner = OWNER.with(|slot| {
        let owner = slot.borrow();
        owner
            .as_ref()
            .filter(|(key, _)| *key == id)
            .and_then(|(_, owner)| owner.upgrade())
    });
    let Some(owner) = owner else {
        request.fail("The application has closed".into());
        return;
    };
    if owner.active.replace(true) {
        request.fail("An application plugin dialog is already open".into());
        return;
    }
    let active = Active(owner);
    if let Err(error) = slint::spawn_local(async move {
        match request {
            DialogRequest::Interaction(dialog) => {
                let dark = crate::platform::preferences_path()
                    .ok()
                    .and_then(|path| crate::preference_load::read_valid(&path).ok().flatten())
                    .is_some_and(|prefs| prefs.theme_material.dark_for(prefs.dark));
                let response = interaction::interact(None, None, dark, &dialog).await;
                dialog.finish(response);
            }
            DialogRequest::File(dialog) => {
                let response = pick_request(None, &dialog).await;
                dialog.finish(response);
            }
            DialogRequest::Notice(notice) => {
                if !notice.is_cancelled() {
                    rfd::AsyncMessageDialog::new()
                        .set_title(&notice.title)
                        .set_description(&notice.message)
                        .set_buttons(rfd::MessageButtons::Ok)
                        .show()
                        .await;
                }
            }
        }
        drop(active);
    }) {
        crate::diagnostics::warn!("Could not show the application plugin dialog: {error}");
    }
}

struct Active(Rc<Dialogs>);

impl Drop for Active {
    fn drop(&mut self) {
        self.0.active.set(false);
        if let Some(closed) = self.0.closed.borrow().as_ref() {
            closed.notify();
        }
    }
}
