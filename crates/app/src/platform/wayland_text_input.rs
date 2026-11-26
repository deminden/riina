//! Compositor text input for browser sidebars on the player's Wayland surface.
mod native;
mod seat;
mod surrounding;

use super::plugin_sidebar::input_method::Context;
use gtk::glib::{WeakRef, prelude::*};
use native::NativeEditors;
use seat::Seat;
use std::{
    cell::RefCell,
    collections::HashMap,
    rc::{Rc, Weak},
};
use wayland_client::{
    Connection, Dispatch, Proxy, QueueHandle, delegate_noop,
    protocol::{wl_registry::WlRegistry, wl_seat::WlSeat},
};
use wayland_protocols::wp::text_input::zv3::client::{
    zwp_text_input_manager_v3::ZwpTextInputManagerV3,
    zwp_text_input_v3::{ChangeCause, ContentHint, ContentPurpose, Event, ZwpTextInputV3},
};
use webkit2gtk::{InputHints, InputPurpose};

pub(super) use surrounding::{Deletion, Surrounding};

#[derive(Clone, PartialEq, Eq)]
pub(in crate::platform) struct Field {
    pub(super) caret: (i32, i32, i32, i32),
    pub(super) surrounding: Option<Surrounding>,
    pub(super) external_change: bool,
    pub(super) allows_surrounding: bool,
    pub(super) purpose: InputPurpose,
    pub(super) hints: InputHints,
}

impl Field {
    fn send(&self, input: &ZwpTextInputV3, previous: Option<&Self>) {
        if previous.is_none_or(|old| old.purpose != self.purpose || old.hints != self.hints) {
            let (hint, purpose) = self.content_type();
            input.set_content_type(hint, purpose);
        }
        if self.allows_surrounding
            && self.purpose != InputPurpose::Password
            && previous.is_none_or(|old| {
                old.surrounding != self.surrounding || old.external_change != self.external_change
            })
        {
            if let Some(text) = &self.surrounding {
                input.set_surrounding_text(text.text.clone(), text.cursor, text.anchor);
            } else {
                input.set_surrounding_text(String::new(), 0, 0);
            }
            input.set_text_change_cause(if self.external_change {
                ChangeCause::Other
            } else {
                ChangeCause::InputMethod
            });
        }
        if previous.is_none_or(|old| old.caret != self.caret) {
            let (x, y, width, height) = self.caret;
            input.set_cursor_rectangle(x, y, width, height);
        }
        input.commit();
    }

    fn content_type(&self) -> (ContentHint, ContentPurpose) {
        let purpose = match self.purpose {
            InputPurpose::Digits => ContentPurpose::Digits,
            InputPurpose::Number => ContentPurpose::Number,
            InputPurpose::Phone => ContentPurpose::Phone,
            InputPurpose::Url => ContentPurpose::Url,
            InputPurpose::Email => ContentPurpose::Email,
            InputPurpose::Password => {
                return (
                    ContentHint::HiddenText | ContentHint::SensitiveData,
                    ContentPurpose::Password,
                );
            }
            _ => ContentPurpose::Normal,
        };
        let mut hint = ContentHint::empty();
        for (webkit, wayland) in [
            (InputHints::SPELLCHECK, ContentHint::Spellcheck),
            (InputHints::LOWERCASE, ContentHint::Lowercase),
            (InputHints::UPPERCASE_CHARS, ContentHint::Uppercase),
            (InputHints::UPPERCASE_WORDS, ContentHint::Titlecase),
            (
                InputHints::UPPERCASE_SENTENCES,
                ContentHint::AutoCapitalization,
            ),
        ] {
            hint.set(wayland, self.hints.contains(webkit));
        }
        (hint, purpose)
    }
}

#[derive(Default)]
pub(in crate::platform) struct Edits {
    pub(super) preedit: Option<Preedit>,
    pub(super) commit: Option<String>,
    deletion: Option<(u32, u32)>,
}

pub(in crate::platform) struct Preedit {
    pub(super) text: String,
    pub(super) begin: i32,
    pub(super) end: i32,
}

struct Editor {
    context: WeakRef<Context>,
    field: Field,
}

enum Delivery {
    Browser {
        context: Context,
        edits: Edits,
        deletion: Option<Deletion>,
    },
    Native {
        surface: usize,
        edits: Edits,
    },
}

#[derive(Default)]
struct State {
    manager: Option<(u32, ZwpTextInputManagerV3)>,
    seats: HashMap<u32, Seat>,
    editors: HashMap<usize, Editor>,
    native_editors: NativeEditors,
}

pub(in crate::platform) struct InputMethods(Rc<RefCell<State>>);

thread_local! {
    static INPUT_METHODS: RefCell<Weak<RefCell<State>>> = const { RefCell::new(Weak::new()) };
}

impl InputMethods {
    pub(in crate::platform) fn new() -> Self {
        let methods = Rc::new(RefCell::new(State::default()));
        INPUT_METHODS.with(|current| *current.borrow_mut() = Rc::downgrade(&methods));
        Self(methods)
    }

    pub(in crate::platform) fn bind(
        &self,
        name: u32,
        interface: &str,
        registry: &WlRegistry,
        seats: impl Iterator<Item = (u32, WlSeat)>,
        queue: &QueueHandle<super::wayland_drop::protocol::State>,
    ) {
        let mut state = self.0.borrow_mut();
        if interface == ZwpTextInputManagerV3::interface().name && state.manager.is_none() {
            let manager = registry.bind(name, 1, queue, ());
            state.manager = Some((name, manager));
            for (name, seat) in seats {
                state.add_seat(name, &seat, queue);
            }
        }
    }

    pub(in crate::platform) fn add_seat(
        &self,
        name: u32,
        seat: &WlSeat,
        queue: &QueueHandle<super::wayland_drop::protocol::State>,
    ) {
        self.0.borrow_mut().add_seat(name, seat, queue);
    }

    pub(in crate::platform) fn remove_global(&self, name: u32) {
        let mut state = self.0.borrow_mut();
        state.seats.remove(&name);
        if state
            .manager
            .as_ref()
            .is_some_and(|(global, _)| *global == name)
        {
            state.clear_manager();
        }
    }

    pub(in crate::platform) fn window_event(
        &self,
        id: slint::winit_030::winit::window::WindowId,
        native: Option<&slint::winit_030::winit::window::Window>,
        window: Option<&slint::Window>,
        event: &slint::winit_030::winit::event::WindowEvent,
    ) {
        self.0
            .borrow_mut()
            .native_editors
            .window_event(id, native, window, event);
    }

    fn dispatch(&self, seat_name: u32, event: Event) {
        let delivery = self.0.borrow_mut().event(seat_name, event);
        match delivery {
            Some(Delivery::Browser {
                context,
                edits,
                deletion,
            }) => context.apply(edits, deletion),
            Some(Delivery::Native { surface, edits }) => {
                let editor = self.0.borrow().native_editors.get(surface);
                if let Some(editor) = editor {
                    native::deliver(&editor, edits);
                }
            }
            None => {}
        }
        self.0.borrow_mut().synchronize();
    }
}

impl State {
    fn clear_manager(&mut self) {
        self.seats.clear();
        if let Some((_, manager)) = self.manager.take() {
            manager.destroy();
        }
    }

    fn add_seat(
        &mut self,
        name: u32,
        seat: &WlSeat,
        queue: &QueueHandle<super::wayland_drop::protocol::State>,
    ) {
        if let Some((_, manager)) = &self.manager {
            self.seats
                .insert(name, Seat::new(manager.get_text_input(seat, queue, name)));
        }
    }

    fn event(&mut self, name: u32, event: Event) -> Option<Delivery> {
        let seat = self.seats.get_mut(&name)?;
        let editor = seat.surface.and_then(|surface| self.editors.get(&surface));
        seat.event(event, editor)
    }

    fn synchronize(&mut self) {
        for seat in self.seats.values_mut() {
            seat.synchronize(seat.surface.and_then(|surface| self.editors.get(&surface)));
        }
    }
}

impl Drop for State {
    fn drop(&mut self) {
        self.clear_manager();
    }
}

pub(super) fn set_editor(surface: usize, context: &Context, field: Option<Field>) {
    let Some(methods) = INPUT_METHODS.with(|current| current.borrow().upgrade()) else {
        return;
    };
    let mut methods = methods.borrow_mut();
    if let Some(field) = field {
        methods.editors.insert(
            surface,
            Editor {
                context: context.downgrade(),
                field,
            },
        );
    } else if methods.editors.get(&surface).is_some_and(|editor| {
        editor
            .context
            .upgrade()
            .is_none_or(|active| active == *context)
    }) {
        methods.editors.remove(&surface);
    }
    methods.synchronize();
}

type Desktop = super::wayland_drop::protocol::State;

delegate_noop!(Desktop: ignore ZwpTextInputManagerV3);

impl Dispatch<ZwpTextInputV3, u32> for Desktop {
    fn event(
        state: &mut Self,
        _: &ZwpTextInputV3,
        event: Event,
        seat: &u32,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        state.input_methods.dispatch(*seat, event);
    }
}
