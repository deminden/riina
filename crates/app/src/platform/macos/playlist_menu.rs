//! Use AppKit's tracking loop for the current player's playlist menu.
use super::view::content_view;
use crate::platform::playlist_menu::{Entry, Target};
use objc2::{
    ClassType, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, rc::Retained,
    runtime::NSObject, sel,
};
use objc2_app_kit::{NSControlStateValueOn, NSMenu, NSMenuItem};
use objc2_foundation::{
    NSObjectProtocol, NSOperatingSystemVersion, NSPoint, NSProcessInfo, NSString,
};
use slint::{LogicalPosition, Window, winit_030::WinitWindowAccessor};
use std::cell::Cell;

define_class!(
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = Cell<Option<usize>>]
    struct PlaylistMenuSelection;

    // SAFETY: the superclass supplies the NSObject protocol implementation.
    unsafe impl NSObjectProtocol for PlaylistMenuSelection {}

    impl PlaylistMenuSelection {
        #[unsafe(method(riinaSelectPlaylistItem:))]
        fn select(&self, item: &NSMenuItem) {
            self.ivars().set(usize::try_from(item.tag()).ok());
        }
    }
);

pub(in crate::platform) fn popup<A>(
    window: &Window,
    position: LogicalPosition,
    entries: Vec<Entry<A>>,
) -> Result<Option<A>, String> {
    let main_thread = MainThreadMarker::new()
        .ok_or_else(|| "The playlist menu must open on the main thread".to_owned())?;
    let scale = f64::from(window.scale_factor());
    Ok(window
        .with_winit_window(|window| {
            let view = content_view(window)?;
            let bounds = view.bounds();
            let scale_to_points = scale / window.scale_factor();
            let x = f64::from(position.x) * scale_to_points;
            let y = f64::from(position.y) * scale_to_points;
            let local = NSPoint::new(
                bounds.origin.x + x,
                bounds.origin.y
                    + if view.isFlipped() {
                        y
                    } else {
                        bounds.size.height - y
                    },
            );
            let target = PlaylistMenuSelection::alloc(main_thread).set_ivars(Cell::new(None));
            // SAFETY: the main thread owns the allocation and its initialized ivars.
            let target: Retained<PlaylistMenuSelection> =
                unsafe { objc2::msg_send![super(target), init] };
            let mut actions = Vec::new();
            let menu = build(main_thread, &target, entries, &mut actions);
            menu.popUpMenuPositioningItem_atLocation_inView(None, local, Some(view));
            // AppKit's targets are weak. Keep this target and its action table
            // alive until AppKit returns control after selection or dismissal.
            target
                .ivars()
                .get()
                .and_then(|index| actions.into_iter().nth(index))
        })
        .flatten())
}

fn build<A>(
    main_thread: MainThreadMarker,
    selection: &PlaylistMenuSelection,
    entries: Vec<Entry<A>>,
    actions: &mut Vec<A>,
) -> Retained<NSMenu> {
    let empty = NSString::from_str("");
    let menu = NSMenu::initWithTitle(NSMenu::alloc(main_thread), &empty);
    menu.setAutoenablesItems(false);
    for entry in entries {
        let (title, enabled, checked, target) = match entry {
            Entry::Separator => {
                menu.addItem(&NSMenuItem::separatorItem(main_thread));
                continue;
            }
            Entry::Heading(title) => {
                menu.addItem(&heading(main_thread, &title));
                continue;
            }
            Entry::Item {
                title,
                enabled,
                checked,
                target,
                ..
            } => (title, enabled, checked, target),
        };
        let title = NSString::from_str(&title);
        // SAFETY: this item starts without an action and owns its copied strings.
        let item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(main_thread),
                &title,
                None,
                &empty,
            )
        };
        match target {
            Target::Action(action) => {
                item.setTag(actions.len() as isize);
                actions.push(action);
                // SAFETY: selection implements the selector and outlives menu tracking.
                unsafe {
                    item.setTarget(Some(selection.as_super()));
                    item.setAction(Some(sel!(riinaSelectPlaylistItem:)));
                }
            }
            Target::Submenu(children) => {
                item.setSubmenu(Some(&build(main_thread, selection, children, actions)));
            }
        }
        item.setEnabled(enabled);
        if checked {
            item.setState(NSControlStateValueOn);
        }
        menu.addItem(&item);
    }
    menu
}

fn heading(main_thread: MainThreadMarker, title: &str) -> Retained<NSMenuItem> {
    let title = NSString::from_str(title);
    let version = NSOperatingSystemVersion {
        majorVersion: 14,
        minorVersion: 0,
        patchVersion: 0,
    };
    if NSProcessInfo::processInfo().isOperatingSystemAtLeastVersion(version) {
        return NSMenuItem::sectionHeaderWithTitle(&title, main_thread);
    }
    // SAFETY: the disabled item only presents the title and has no action.
    let item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(main_thread),
            &title,
            None,
            &NSString::from_str(""),
        )
    };
    item.setEnabled(false);
    item
}
