//! Track the entered surface, enabled editor and in-flight edits for one seat.
use super::{Delivery, Editor, Edits, Preedit, Surrounding};
use std::collections::VecDeque;
use wayland_client::Proxy;
use wayland_protocols::wp::text_input::zv3::client::zwp_text_input_v3::{Event, ZwpTextInputV3};
use webkit2gtk::InputPurpose;

pub(super) struct Seat {
    input: ZwpTextInputV3,
    pub(super) surface: Option<usize>,
    sent: Option<Editor>,
    serial: u32,
    enabled_at: u32,
    update_allowed: bool,
    snapshots: VecDeque<(u32, Option<Surrounding>)>,
    pending: Edits,
}

impl Seat {
    pub(super) fn new(input: ZwpTextInputV3) -> Self {
        Self {
            input,
            surface: None,
            sent: None,
            serial: 0,
            enabled_at: 0,
            update_allowed: true,
            snapshots: VecDeque::new(),
            pending: Edits::default(),
        }
    }

    fn disable(&mut self) {
        if self.sent.take().is_some() {
            self.input.disable();
            self.input.commit();
            self.serial = self.serial.wrapping_add(1);
            self.reset();
        }
    }

    fn reset(&mut self) {
        self.sent = None;
        self.pending = Edits::default();
        self.snapshots.clear();
        self.update_allowed = true;
    }

    pub(super) fn synchronize(&mut self, editor: Option<&Editor>) {
        let Some(editor) = editor.filter(|editor| editor.context.upgrade().is_some()) else {
            self.disable();
            return;
        };
        if self.sent.as_ref().is_some_and(|old| {
            old.context.upgrade() != editor.context.upgrade()
                || (old.field.purpose == InputPurpose::Password)
                    != (editor.field.purpose == InputPurpose::Password)
        }) {
            self.disable();
        }
        let previous = self.sent.as_ref().map(|old| &old.field);
        if !self.update_allowed || previous == Some(&editor.field) {
            return;
        }
        if previous.is_none() {
            self.input.enable();
            self.enabled_at = self.serial.wrapping_add(1);
        }
        editor.field.send(&self.input, previous);
        self.serial = self.serial.wrapping_add(1);
        if self.snapshots.len() == 8 {
            self.snapshots.pop_front();
        }
        self.snapshots
            .push_back((self.serial, editor.field.surrounding.clone()));
        self.sent = Some(Editor {
            context: editor.context.clone(),
            field: editor.field.clone(),
        });
    }

    pub(super) fn event(&mut self, event: Event, editor: Option<&Editor>) -> Option<Delivery> {
        match event {
            Event::Enter { surface } => {
                self.reset();
                self.surface = Some(surface.id().as_ptr().addr());
            }
            Event::Leave { .. } => {
                self.reset();
                self.surface = None;
            }
            Event::PreeditString {
                text,
                cursor_begin,
                cursor_end,
            } => {
                self.pending.preedit = Some(Preedit {
                    text: text.unwrap_or_default(),
                    begin: cursor_begin,
                    end: cursor_end,
                });
            }
            Event::CommitString { text } => {
                self.pending.commit = text.filter(|text| !text.is_empty());
            }
            Event::DeleteSurroundingText {
                before_length,
                after_length,
            } => {
                self.pending.deletion = Some((before_length, after_length));
            }
            Event::Done { serial } => {
                return self.finish(serial, editor);
            }
            _ => {}
        }
        None
    }

    fn finish(&mut self, serial: u32, editor: Option<&Editor>) -> Option<Delivery> {
        let edits = std::mem::take(&mut self.pending);
        let Some(sent) = self.sent.as_ref() else {
            // Sway can deliver the toolkit's reply to a disabled object.
            // Only the current serial and an already focused native editor
            // can use it. Native editors never promise surrounding text here.
            return (serial == self.serial && matches!(edits.deletion, None | Some((0, 0))))
                .then_some(Delivery::Native {
                    surface: self.surface?,
                    edits,
                });
        };
        let editor = editor?;
        let context = sent.context.upgrade()?;
        if editor.context.upgrade().as_ref() != Some(&context)
            || serial.wrapping_sub(self.enabled_at) > i32::MAX as u32
        {
            return None;
        }
        self.update_allowed = serial == self.serial;
        let deletion = match edits.deletion {
            Some((0, 0)) | None => None,
            Some(lengths) => {
                let (_, advertised) = self.snapshots.iter().find(|(id, _)| *id == serial)?;
                if *advertised != editor.field.surrounding {
                    return None;
                }
                Some(
                    advertised
                        .as_ref()?
                        .deletion(lengths, edits.commit.is_some())?,
                )
            }
        };
        Some(Delivery::Browser {
            context,
            edits,
            deletion,
        })
    }
}

impl Drop for Seat {
    fn drop(&mut self) {
        self.input.destroy();
    }
}
