//! Bridge the selected sidebar surface to Slint's player-local input.
use super::{PluginSidebarPresenter, view::CompositedView};
use crate::PlayerWindow;
use gtk::gdk::ModifierType;
use riina_ui::PlayerPluginSidebarState;
use slint::{ComponentHandle, winit_030::winit::event::WindowEvent};
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

impl PluginSidebarPresenter {
    pub(in crate::platform) fn bind_composited(app: &PlayerWindow, presenter: &Rc<RefCell<Self>>) {
        let actions = app.global::<PlayerPluginSidebarState>();
        let owner = Rc::downgrade(presenter);
        let pointer = owner.clone();
        actions.on_pointer_input(move |phase, button, x, y, control, alt, shift, meta| {
            with_view(&pointer, |view| {
                view.pointer(
                    phase,
                    button,
                    (f64::from(x), f64::from(y)),
                    modifiers(control, alt, shift, meta),
                );
            });
        });
        let pointer = owner.clone();
        actions.on_pointer_left(move || with_view(&pointer, CompositedView::pointer_left));
        let wheel = owner.clone();
        actions.on_scroll_input(move |x, y, dx, dy, control, alt, shift, meta| {
            with_view(&wheel, |view| {
                view.scroll(
                    (f64::from(x), f64::from(y)),
                    (f64::from(dx), f64::from(dy)),
                    modifiers(control, alt, shift, meta),
                );
            });
        });
        let focus = owner.clone();
        actions.on_focus_input(move |focused| {
            if focused {
                with_view(&focus, |view| view.focus(true));
            } else {
                release_presented_focus(&focus);
            }
        });
        actions.on_key_input(move |text, control, alt, shift, meta, released| {
            with_view(&owner, |view| {
                view.focus(true);
                if !released {
                    view.input_method.external_edit();
                }
                view.surface.key(
                    text.as_str(),
                    modifiers(control, alt, shift, meta),
                    released,
                );
            });
        });
    }

    pub(super) fn update_composited(&mut self) {
        let Some(app) = self.owner.upgrade() else {
            return;
        };
        let state = app.global::<PlayerPluginSidebarState>();
        let selected = self.selected.as_ref().and_then(|identifier| {
            let entry = self.entries.get(identifier)?;
            let view = entry.view.composited()?;
            entry
                .visible
                .then_some((identifier, entry.page.ready_page().is_some(), view))
        });
        if let Some((identifier, ready, view)) = selected {
            if ready {
                view.surface.capture();
            }
            let pixels_changed = view.surface.take_changed();
            if self.composited.as_ref() != Some(identifier) || pixels_changed {
                state.set_frame(view.surface.pixels().unwrap_or_default());
            }
            if self.composited.as_ref() != Some(identifier) {
                self.composited = Some(identifier.clone());
                state.set_composited(true);
            }
        } else if self.composited.take().is_some() {
            state.set_composited(false);
            state.set_keyboard_focused(false);
            state.set_frame(slint::Image::default());
        }
    }
}

pub(crate) fn update_sidebar_window_focus(app: &slint::Weak<PlayerWindow>, event: &WindowEvent) {
    let WindowEvent::Focused(focused) = event else {
        return;
    };
    if let Some(app) = app.upgrade() {
        let state = app.global::<PlayerPluginSidebarState>();
        if state.get_composited()
            && state.get_keyboard_focused()
            && (!focused || super::sidebar_can_show(&app))
        {
            state.invoke_focus_input(*focused);
        }
    }
}

fn release_presented_focus(presenter: &Weak<RefCell<PluginSidebarPresenter>>) {
    // The visible page must release input before a player editor takes over.
    // A dialog or sidebar selection can already have changed the next page.
    if let Some(presenter) = presenter.upgrade()
        && let Ok(presenter) = presenter.try_borrow()
        && let Some(identifier) = &presenter.composited
        && let Some(view) = presenter
            .entries
            .get(identifier)
            .and_then(|entry| entry.view.composited())
    {
        view.focus(false);
    }
}

fn modifiers(control: bool, alt: bool, shift: bool, meta: bool) -> u32 {
    let mut modifiers = ModifierType::empty();
    modifiers.set(ModifierType::CONTROL_MASK, control);
    modifiers.set(ModifierType::MOD1_MASK, alt);
    modifiers.set(ModifierType::SHIFT_MASK, shift);
    modifiers.set(ModifierType::SUPER_MASK, meta);
    modifiers.bits()
}

fn with_view(
    presenter: &Weak<RefCell<PluginSidebarPresenter>>,
    action: impl FnOnce(&CompositedView),
) {
    if let Some(presenter) = presenter.upgrade()
        && let Ok(presenter) = presenter.try_borrow()
        && presenter.sidebar_can_show()
        && let Some(entry) = presenter
            .selected
            .as_ref()
            .and_then(|id| presenter.entries.get(id))
        && entry.visible
        && let Some(view) = entry.view.composited()
    {
        action(view);
    }
}
