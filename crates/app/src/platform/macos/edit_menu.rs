//! AppKit keeps native editing on the first responder. Slint text inputs use
//! their own clipboard and undo stack through the same menu.
use super::menu_labels::MenuLabels;
use super::view::content_view;
use crate::PlayerWindow;
use crate::platform::text_edit::{Edit, Editor};
use objc2::{
    MainThreadMarker, MainThreadOnly, define_class, msg_send,
    rc::Retained,
    runtime::{AnyObject, Bool, NSObject, Sel},
    sel,
};
use objc2_app_kit::{
    NSApplication, NSEventModifierFlags as Modifiers, NSMenu, NSMenuItem, NSMenuItemValidation,
};
use objc2_foundation::{NSObjectProtocol, NSString};
use riina_ui::{PlaylistEdit, PlaylistInteraction};
use slint::{ComponentHandle, Weak, winit_030::WinitWindowAccessor};
use std::{
    any::Any,
    cell::{OnceCell, RefCell},
    collections::HashMap,
};

const EDIT_MENU_TAG: isize = 114082;

thread_local! {
    static TARGET: OnceCell<Retained<EditTarget>> = const { OnceCell::new() };
    static EDITORS: RefCell<HashMap<usize, TrackedEditor>> = RefCell::default();
}

#[derive(Clone)]
struct TrackedEditor {
    text: Editor,
    player: Option<Weak<PlayerWindow>>,
}

define_class!(
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    struct EditTarget;

    // SAFETY: NSObject supplies the protocol implementation.
    unsafe impl NSObjectProtocol for EditTarget {}

    // SAFETY: AppKit invokes this validation on the main thread for our edit items.
    unsafe impl NSMenuItemValidation for EditTarget {
        #[unsafe(method(validateMenuItem:))]
        fn validate_menu_item(&self, item: &NSMenuItem) -> bool {
            with_edit(self.mtm(), item, false)
        }
    }

    impl EditTarget {
        #[unsafe(method(riinaEdit:))]
        fn perform_edit(&self, sender: Option<&NSMenuItem>) {
            if let Some(sender) = sender {
                with_edit(self.mtm(), sender, true);
            }
        }
    }
);

pub(in crate::platform) fn track<T: ComponentHandle + 'static>(component: &T) {
    let window = component.window();
    let player = (component as &dyn Any)
        .downcast_ref::<PlayerWindow>()
        .map(ComponentHandle::as_weak);
    if window
        .with_winit_window(|native| register(native, window, player.clone()))
        .is_some()
    {
        return;
    }
    let weak = component.as_weak();
    let _ = slint::spawn_local(async move {
        if let Some(component) = weak.upgrade()
            && let Ok(native) = component.window().winit_window().await
        {
            register(&native, component.window(), player);
        }
    });
}

fn register(
    native: &slint::winit_030::winit::window::Window,
    window: &slint::Window,
    player: Option<Weak<PlayerWindow>>,
) {
    if let Some(view) = content_view(native) {
        EDITORS.with_borrow_mut(|editors| {
            editors.retain(|_, editor| editor.text.is_live());
            editors.insert(
                std::ptr::from_ref(view) as usize,
                TrackedEditor {
                    text: Editor::new(window),
                    player,
                },
            );
        });
    }
}

pub(in crate::platform) fn install(
    main: MainThreadMarker,
    parent: &NSMenuItem,
    labels: &MenuLabels,
) {
    if parent.tag() != EDIT_MENU_TAG {
        parent.setSubmenu(Some(&new(main, labels)));
        parent.setTag(EDIT_MENU_TAG);
    }
}

pub(in crate::platform) fn parent(
    main: MainThreadMarker,
    labels: &MenuLabels,
) -> Retained<NSMenuItem> {
    let parent = row(main, &labels.edit_title, None, "", None);
    install(main, &parent, labels);
    parent
}

fn new(main: MainThreadMarker, labels: &MenuLabels) -> Retained<NSMenu> {
    let menu = submenu(main, &labels.edit_title);
    let target = TARGET.with(|target| {
        target
            .get_or_init(|| {
                let allocated = EditTarget::alloc(main);
                // SAFETY: the allocated main-thread object has no uninitialized ivars.
                unsafe { msg_send![allocated, init] }
            })
            .clone()
    });
    for (index, command) in COMMANDS.iter().enumerate() {
        if command.edit == Edit::Cut {
            menu.addItem(&NSMenuItem::separatorItem(main));
        }
        let item = row(
            main,
            (command.title)(labels),
            Some(sel!(riinaEdit:)),
            command.shortcut,
            Some(&target),
        );
        item.setKeyEquivalentModifierMask(command.modifiers);
        item.setTag(index as isize);
        menu.addItem(&item);
    }
    menu.addItem(&NSMenuItem::separatorItem(main));
    for (title, items) in [
        (
            &labels.transformations,
            &[
                (&labels.uppercase, sel!(uppercaseWord:)),
                (&labels.lowercase, sel!(lowercaseWord:)),
                (&labels.capitalize, sel!(capitalizeWord:)),
            ][..],
        ),
        (
            &labels.speech,
            &[
                (&labels.start_speaking, sel!(startSpeaking:)),
                (&labels.stop_speaking, sel!(stopSpeaking:)),
            ][..],
        ),
    ] {
        let parent = row(main, title, None, "", None);
        let children = submenu(main, title);
        for &(title, selector) in items {
            children.addItem(&row(main, title, Some(selector), "", None));
        }
        parent.setSubmenu(Some(&children));
        menu.addItem(&parent);
    }
    menu
}

fn submenu(main: MainThreadMarker, title: &str) -> Retained<NSMenu> {
    NSMenu::initWithTitle(NSMenu::alloc(main), &NSString::from_str(title))
}

fn row(
    main: MainThreadMarker,
    title: &str,
    action: Option<Sel>,
    key: &str,
    target: Option<&AnyObject>,
) -> Retained<NSMenuItem> {
    // SAFETY: these selectors have the AppKit sender signature. The custom
    // target is retained on the main thread for the life of the menu bar.
    unsafe {
        let row = NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(main),
            &NSString::from_str(title),
            action,
            &NSString::from_str(key),
        );
        row.setTarget(target);
        row
    }
}

struct Command {
    edit: Edit,
    title: fn(&MenuLabels) -> &str,
    selector: fn() -> Sel,
    shortcut: &'static str,
    modifiers: Modifiers,
}

const COMMANDS: &[Command] = &[
    Command {
        edit: Edit::Undo,
        title: |labels| &labels.undo,
        selector: || sel!(undo:),
        shortcut: "z",
        modifiers: Modifiers::Command,
    },
    Command {
        edit: Edit::Redo,
        title: |labels| &labels.redo,
        selector: || sel!(redo:),
        shortcut: "z",
        modifiers: Modifiers::Command.union(Modifiers::Shift),
    },
    Command {
        edit: Edit::Cut,
        title: |labels| &labels.cut,
        selector: || sel!(cut:),
        shortcut: "x",
        modifiers: Modifiers::Command,
    },
    Command {
        edit: Edit::Copy,
        title: |labels| &labels.copy,
        selector: || sel!(copy:),
        shortcut: "c",
        modifiers: Modifiers::Command,
    },
    Command {
        edit: Edit::Paste,
        title: |labels| &labels.paste,
        selector: || sel!(paste:),
        shortcut: "v",
        modifiers: Modifiers::Command,
    },
    Command {
        edit: Edit::Delete,
        title: |labels| &labels.delete,
        selector: || sel!(delete:),
        shortcut: "\u{8}",
        modifiers: Modifiers::empty(),
    },
    Command {
        edit: Edit::SelectAll,
        title: |labels| &labels.select_all,
        selector: || sel!(selectAll:),
        shortcut: "a",
        modifiers: Modifiers::Command,
    },
];

fn with_edit(main: MainThreadMarker, row: &NSMenuItem, apply: bool) -> bool {
    let Some(command) = usize::try_from(row.tag())
        .ok()
        .and_then(|tag| COMMANDS.get(tag))
    else {
        return false;
    };
    let app = NSApplication::sharedApplication(main);
    let Some(responder) = app.keyWindow().and_then(|window| window.firstResponder()) else {
        return false;
    };
    let editor = EDITORS.with_borrow(|editors| {
        editors
            .get(&(Retained::as_ptr(&responder) as usize))
            .filter(|editor| editor.text.is_live())
            .cloned()
    });
    if let Some(editor) = editor {
        if editor.text.has_text_focus() {
            return editor.text.perform(command.edit, apply);
        }
        let edit = match command.edit {
            Edit::Cut => PlaylistEdit::Cut,
            Edit::Copy => PlaylistEdit::Copy,
            Edit::Paste => PlaylistEdit::Paste,
            Edit::Delete => PlaylistEdit::Remove,
            Edit::SelectAll => PlaylistEdit::SelectAll,
            Edit::Undo | Edit::Redo => return false,
        };
        return editor
            .player
            .and_then(|player| player.upgrade())
            .is_some_and(|player| {
                player
                    .global::<PlaylistInteraction>()
                    .invoke_edit(edit, apply)
            });
    }
    forward(main, &app, command, row, apply)
}

fn forward(
    main: MainThreadMarker,
    app: &NSApplication,
    command: &Command,
    sender: &NSMenuItem,
    apply: bool,
) -> bool {
    let selector = (command.selector)();
    let proxy = row(
        main,
        &sender.title().to_string(),
        Some(selector),
        command.shortcut,
        None,
    );
    proxy.setTag(sender.tag());
    proxy.setKeyEquivalentModifierMask(command.modifiers);
    // SAFETY: use the standard one-argument editing selectors. The proxy
    // exposes the original action to validation without mutating the menu.
    unsafe {
        let Some(target) = app.targetForAction_to_from(selector, None, Some(&proxy)) else {
            return false;
        };
        if apply {
            return app.sendAction_to_from(selector, Some(&target), Some(&proxy));
        }
        let validates_menu: Bool = msg_send![&target, respondsToSelector: sel!(validateMenuItem:)];
        if validates_menu.as_bool() {
            let valid: Bool = msg_send![&target, validateMenuItem: &*proxy];
            return valid.as_bool();
        }
        let validates_ui: Bool =
            msg_send![&target, respondsToSelector: sel!(validateUserInterfaceItem:)];
        if validates_ui.as_bool() {
            let valid: Bool = msg_send![&target, validateUserInterfaceItem: &*proxy];
            return valid.as_bool();
        }
        true
    }
}
