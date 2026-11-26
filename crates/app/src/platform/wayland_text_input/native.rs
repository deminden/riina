//! Keep toolkit composition working when a compositor replies on the browser object.
use super::{Edits, Preedit};
use crate::platform::text_edit;
use i_slint_core::input::{InternalKeyEvent, KeyEvent, KeyEventType};
use slint::{
    platform::WindowEvent as SlintEvent,
    private_unstable_api::re_exports::{ItemRef, TextInput, WindowAdapter, WindowInner},
    winit_030::{
        WinitWindowAccessor,
        winit::{
            event::WindowEvent,
            raw_window_handle::{HasWindowHandle, RawWindowHandle},
            window::{Window, WindowId},
        },
    },
};
use std::{
    collections::HashMap,
    ops::Range,
    rc::{Rc, Weak},
};

#[derive(Default)]
pub(super) struct NativeEditors(HashMap<WindowId, (usize, Weak<dyn WindowAdapter>)>);

impl NativeEditors {
    pub(super) fn window_event(
        &mut self,
        id: WindowId,
        native: Option<&Window>,
        window: Option<&slint::Window>,
        event: &WindowEvent,
    ) {
        match event {
            WindowEvent::Focused(true) => {
                let Some((native, window)) = native.zip(window) else {
                    return;
                };
                if let Ok(handle) = native.window_handle()
                    && let RawWindowHandle::Wayland(handle) = handle.as_raw()
                {
                    let adapter = WindowInner::from_pub(window).window_adapter();
                    self.0.insert(
                        id,
                        (handle.surface.as_ptr().addr(), Rc::downgrade(&adapter)),
                    );
                }
            }
            WindowEvent::Focused(false) | WindowEvent::Destroyed => {
                self.0.remove(&id);
            }
            _ => {}
        }
    }

    pub(super) fn get(&self, surface: usize) -> Option<Rc<dyn WindowAdapter>> {
        let (_, window) = self.0.values().find(|(address, _)| *address == surface)?;
        let adapter = window.upgrade()?;
        let window = adapter.window();
        (window.is_visible() && window.with_winit_window(Window::has_focus) == Some(true))
            .then_some(adapter)
    }
}

pub(super) fn deliver(adapter: &Rc<dyn WindowAdapter>, edits: Edits) {
    let Some(item) = text_edit::focused_text(adapter) else {
        return;
    };
    let Some(input) = ItemRef::downcast_pin::<TextInput>(item.borrow()) else {
        return;
    };
    if TextInput::FIELD_OFFSETS.read_only().apply_pin(input).get() {
        return;
    }
    let window = adapter.window();
    if edits.commit.is_some() || edits.preedit.is_none() {
        update_preedit(window, None);
    }
    if let Some(text) = edits.commit {
        let mut key_event = KeyEvent::default();
        key_event.text = text.into();
        window.dispatch_event(SlintEvent::internal(InternalKeyEvent {
            event_type: KeyEventType::CommitComposition,
            key_event,
            ..Default::default()
        }));
    }
    if let Some(preedit) = edits.preedit {
        update_preedit(window, Some(preedit));
    }
}

fn update_preedit(window: &slint::Window, preedit: Option<Preedit>) {
    let (preedit_text, preedit_selection) = preedit.map_or_else(Default::default, |preedit| {
        let selection = preedit_range(&preedit);
        (preedit.text.into(), selection)
    });
    window.dispatch_event(SlintEvent::internal(InternalKeyEvent {
        event_type: KeyEventType::UpdateComposition,
        preedit_text,
        preedit_selection,
        ..Default::default()
    }));
}

fn preedit_range(preedit: &Preedit) -> Option<Range<i32>> {
    let boundary = |position| {
        usize::try_from(position)
            .ok()
            .is_some_and(|byte| preedit.text.is_char_boundary(byte))
    };
    boundary(preedit.begin).then(|| {
        let end = if boundary(preedit.end) {
            preedit.end
        } else {
            preedit.begin
        };
        preedit.begin..end
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use slint::ComponentHandle;

    slint::slint! {
        export component NativeEditor inherits Window {
            in-out property <string> value <=> input.text;
            in property <bool> read-only;
            callback focus-editor;
            focus-editor => { input.focus(); }
            input := TextInput { read-only: root.read-only; }
        }
    }

    #[test]
    fn redirected_composition_requires_a_focused_writable_editor() {
        i_slint_backend_testing::init_no_event_loop();
        let editor = NativeEditor::new().unwrap();
        editor.show().unwrap();
        let adapter = WindowInner::from_pub(editor.window()).window_adapter();
        let commit = || Edits {
            commit: Some("仮名🪷".into()),
            ..Edits::default()
        };
        deliver(&adapter, commit());
        assert!(editor.get_value().is_empty());
        editor.invoke_focus_editor();
        editor.set_read_only(true);
        deliver(&adapter, commit());
        assert!(editor.get_value().is_empty());
        editor.set_read_only(false);
        deliver(
            &adapter,
            Edits {
                preedit: Some(Preedit {
                    text: "仮".into(),
                    begin: 0,
                    end: 3,
                }),
                ..Edits::default()
            },
        );
        assert!(editor.get_value().is_empty());
        deliver(&adapter, commit());
        assert_eq!(editor.get_value(), "仮名🪷");
    }
}
