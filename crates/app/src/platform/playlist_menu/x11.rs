//! Present dynamic playlist menus on the same X11 display as the Winit player.
use super::{Entry, Target};
use gtk::{
    gdk, glib,
    glib::translate::{ToGlibPtr, from_glib_full},
    prelude::*,
};
use slint::{
    LogicalPosition, Window,
    winit_030::{
        WinitWindowAccessor,
        winit::raw_window_handle::{HasWindowHandle, RawWindowHandle},
    },
};
use std::{cell::RefCell, ffi::c_ulong, rc::Rc, time::Duration};

#[link(name = "gdk-3")]
unsafe extern "C" {
    fn gdk_x11_window_foreign_new_for_display(
        display: *mut gdk::ffi::GdkDisplay,
        window: c_ulong,
    ) -> *mut gdk::ffi::GdkWindow;
}

struct ActiveMenu {
    owner: c_ulong,
    menu: gtk::Menu,
    _events: slint::Timer,
}

thread_local! {
    // GTK has one grabbed menu for the seat. Ownership still follows the
    // individual player that supplied the request.
    static ACTIVE: RefCell<Option<ActiveMenu>> = const { RefCell::new(None) };
}

pub(super) fn owner(window: &Window) -> Option<c_ulong> {
    window
        .with_winit_window(|window| match window.window_handle().ok()?.as_raw() {
            RawWindowHandle::Xlib(handle) => Some(handle.window),
            _ => None,
        })
        .flatten()
}

pub(super) fn dismiss(window: &Window) {
    if let Some(owner) = owner(window) {
        close_matching(|active| active.owner == owner);
    }
}

fn close_matching(matches: impl FnOnce(&ActiveMenu) -> bool) {
    let active = ACTIVE.with_borrow_mut(|active| {
        if active.as_ref().is_some_and(matches) {
            active.take()
        } else {
            None
        }
    });
    if let Some(active) = active {
        // popdown can emit deactivate. Release the slot before entering GTK.
        active.menu.popdown();
    }
}

pub(super) fn popup<A: 'static>(
    window: &Window,
    position: LogicalPosition,
    entries: Vec<Entry<A>>,
    selected: impl FnOnce(A) + 'static,
) -> Result<(), String> {
    let owner = owner(window).ok_or("The playlist menu requires an X11 player window")?;
    if !gtk::is_initialized_main_thread() {
        gtk::init().map_err(|error| format!("Could not initialize playlist menus: {error}"))?;
    }
    let display = gdk::Display::default()
        .filter(|display| display.type_().name() == "GdkX11Display")
        .ok_or("GTK is not using the player's X11 display")?;
    // SAFETY: GTK supplies a live X11 display and Winit owns the live XID.
    // GDK returns a full reference to a wrapper, or null if the XID is gone.
    let parent: Option<gdk::Window> = unsafe {
        from_glib_full(gdk_x11_window_foreign_new_for_display(
            display.to_glib_none().0,
            owner,
        ))
    };
    let parent = parent.ok_or("The X11 playlist menu owner is unavailable")?;
    let selected = Rc::new(RefCell::new(Some(selected)));
    let menu = build(entries, &selected);
    menu.connect_deactivate(|menu| {
        ACTIVE.with_borrow_mut(|active| {
            if active.as_ref().is_some_and(|active| &active.menu == menu) {
                active.take();
            }
        });
    });
    let physical = position.to_physical(window.scale_factor());
    let scale = parent.scale_factor().max(1);
    let rect = gdk::Rectangle::new(physical.x / scale, physical.y / scale, 1, 1);
    close_matching(|_| true);
    // The menu also opens when no plugin view is driving GTK's event loop.
    let events = slint::Timer::default();
    events.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(16),
        super::super::pump_plugin_webviews,
    );
    ACTIVE.set(Some(ActiveMenu {
        owner,
        menu: menu.clone(),
        _events: events,
    }));
    menu.show_all();
    menu.popup_at_rect(
        &parent,
        &rect,
        gdk::Gravity::NorthWest,
        gdk::Gravity::NorthWest,
        None,
    );
    Ok(())
}

fn build<A: 'static, F: FnOnce(A) + 'static>(
    entries: Vec<Entry<A>>,
    selected: &Rc<RefCell<Option<F>>>,
) -> gtk::Menu {
    let menu = gtk::Menu::new();
    for entry in entries {
        let Entry::Item {
            title,
            enabled,
            checked,
            target,
        } = entry
        else {
            menu.append(&gtk::SeparatorMenuItem::new());
            continue;
        };
        let item = if checked {
            let item = gtk::CheckMenuItem::with_label(&title);
            item.set_active(true);
            item.upcast()
        } else {
            gtk::MenuItem::with_label(&title)
        };
        item.set_sensitive(enabled);
        match target {
            Target::Submenu(children) => item.set_submenu(Some(&build(children, selected))),
            Target::Action(action) => {
                let action = RefCell::new(Some(action));
                let selected = selected.clone();
                item.connect_activate(move |_| {
                    if let Some(action) = action.take()
                        && let Some(selected) = selected.take()
                    {
                        // GTK closes the menu before the player handles the choice.
                        glib::idle_add_local_once(move || selected(action));
                    }
                });
            }
        }
        menu.append(&item);
    }
    menu
}
