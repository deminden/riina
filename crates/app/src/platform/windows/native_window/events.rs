//! Event routing and geometry updates for the framed Win32 owner.
use super::{
    placement::StartupCenter,
    surfaces::{align, dismiss_unaligned_transition_cover, set_overlay_visibility},
    visibility::window_is_drawable,
};
use crate::{NativeVideoHost, PlayerWindow};
use slint::{
    ComponentHandle,
    winit_030::{EventResult, WinitWindowAccessor, winit},
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

pub(super) struct FilterInputs {
    pub(super) event_chrome: Rc<RefCell<Option<PlayerWindow>>>,
    pub(super) event_overlay_hidden: Rc<Cell<bool>>,
    pub(super) event_placement: StartupCenter,
    pub(super) event_center_host: slint::Weak<NativeVideoHost>,
    pub(super) owner_center: Option<(i32, i32)>,
    pub(super) resize_filter: Rc<RefCell<Option<crate::window_aspect::WindowEventFilter>>>,
}

pub(super) fn install_filter(host: &NativeVideoHost, inputs: FilterInputs) {
    let FilterInputs {
        event_chrome,
        event_overlay_hidden,
        event_placement,
        event_center_host,
        owner_center,
        resize_filter,
    } = inputs;
    let event_scroll_transition = Rc::new(Cell::new(0_u64));
    let host_input = super::super::host_input::HostInput::default();
    host.window().on_winit_window_event(move |host, event| {
        if let Some(app) = event_chrome.borrow().as_ref() {
            if matches!(event, winit::event::WindowEvent::Focused(true)) {
                app.invoke_history_player_activated();
            }
            crate::platform::track_plugin_pointer(app, host, event);
            if host_input.handle(app, event) == EventResult::PreventDefault {
                return EventResult::PreventDefault;
            }
        }
        if event_placement.media_loaded()
            && matches!(
                event,
                winit::event::WindowEvent::Resized(_)
                    | winit::event::WindowEvent::ScaleFactorChanged { .. }
            )
            && let Some(app) = event_chrome.borrow().as_ref()
        {
            event_placement.schedule(event_center_host.clone(), app.as_weak(), owner_center);
        }
        let result = resize_filter
            .borrow()
            .as_ref()
            .map_or(EventResult::Propagate, |filter| filter(host, event));
        if matches!(
            event,
            winit::event::WindowEvent::Focused(_)
                | winit::event::WindowEvent::CursorEntered { .. }
                | winit::event::WindowEvent::CursorLeft { .. }
        ) && let Some(app) = event_chrome.borrow().as_ref()
        {
            // Switching between the aligned video and chrome HWNDs can produce
            // a stale wheel delta. Keep that transition from becoming a seek.
            app.set_scroll_input_suppressed(true);
            let generation = event_scroll_transition.get().wrapping_add(1);
            event_scroll_transition.set(generation);
            let scroll_transition = event_scroll_transition.clone();
            let weak_app = app.as_weak();
            slint::Timer::single_shot(std::time::Duration::from_millis(220), move || {
                if scroll_transition.get() == generation
                    && let Some(app) = weak_app.upgrade()
                {
                    app.set_scroll_input_suppressed(false);
                }
            });
        }
        if matches!(
            event,
            winit::event::WindowEvent::Moved(_)
                | winit::event::WindowEvent::Resized(_)
                | winit::event::WindowEvent::ScaleFactorChanged { .. }
        ) && let Some(app) = event_chrome.borrow().as_ref()
        {
            let host_moved = matches!(event, winit::event::WindowEvent::Moved(_));
            if host_moved {
                // Retained frames belong to the old client geometry. Keep the
                // live popup visible while aligning it: fullscreen also emits
                // Moved, and hiding/showing a surface during that transition can
                // strand its compositor image even while UI state advances.
                dismiss_unaligned_transition_cover(app);
            }
            crate::platform::update_hdr_display(
                app,
                super::super::presentation::hdr_display_active_for_slint(host),
            );
            if !host.with_winit_window(window_is_drawable).unwrap_or(false) {
                return result;
            }
            let aligned = align(host, app);
            if !aligned {
                // Hide only a proven mismatch; the retry loop restores it once
                // Win32 confirms matching host, child and popup client bounds.
                if !event_overlay_hidden.replace(true) {
                    set_overlay_visibility(app, false);
                }
            } else if aligned && event_overlay_hidden.get() {
                set_overlay_visibility(app, true);
                event_overlay_hidden.set(false);
            }
        }
        result
    });
}
