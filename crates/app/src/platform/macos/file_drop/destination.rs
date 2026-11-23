use super::{Event, Key, Position, ROUTES, Route, lookup, payload, queue};
use objc2::{
    ffi::object_setClass,
    rc::Weak,
    runtime::{AnyClass, AnyObject, Bool, ClassBuilder, ProtocolObject, Sel},
    sel,
};
use objc2_app_kit::{NSDragOperation, NSDraggingInfo, NSPasteboardType, NSView};
use objc2_foundation::NSArray;
use std::rc::Rc;

type DragInfo = ProtocolObject<dyn NSDraggingInfo>;

pub(super) struct Attachment {
    key: Key,
    view: Weak<NSView>,
    class: &'static AnyClass,
    original: &'static AnyClass,
    original_types: objc2::rc::Retained<NSArray<NSPasteboardType>>,
}

impl Attachment {
    pub(super) fn new(view: &NSView, route: &Rc<Route>) -> Option<Self> {
        let original = view.class();
        let class = destination_class(original)?;
        let original_types = view.registeredDraggedTypes();
        // SAFETY: only this main-thread view changes class. The subclass adds
        // no ivars and implements only AppKit's typed drag destination methods.
        unsafe { AnyObject::set_class(view, class) };
        let key = Key::new(view);
        ROUTES.with_borrow_mut(|routes| routes.insert(key.view, (key, Rc::downgrade(route))));
        view.registerForDraggedTypes(&NSArray::from_slice(&payload::types()));
        Some(Self {
            key,
            view: Weak::new(view),
            class,
            original,
            original_types,
        })
    }

    pub(super) fn owns(&self, view: &NSView) -> bool {
        self.view
            .load()
            .is_some_and(|attached| std::ptr::eq(&*attached, view))
    }
}

impl Drop for Attachment {
    fn drop(&mut self) {
        let _ = ROUTES.try_with(|routes| {
            let mut routes = routes.borrow_mut();
            if routes
                .get(&self.key.view)
                .is_some_and(|(key, _)| *key == self.key)
            {
                routes.remove(&self.key.view);
            }
        });
        if let Some(view) = self.view.load()
            && view.class() == self.class
        {
            view.unregisterDraggedTypes();
            // SAFETY: this restores the main-thread view's exact original class.
            // The subclass adds no storage and changes no lifetime methods.
            unsafe {
                object_setClass(
                    std::ptr::from_ref::<AnyObject>(&view).cast_mut(),
                    self.original,
                )
            };
            if !self.original_types.is_empty() {
                view.registerForDraggedTypes(&self.original_types);
            }
        }
    }
}

fn destination_class(original: &'static AnyClass) -> Option<&'static AnyClass> {
    let name = c"RiinaAppKitDropContentView";
    if let Some(class) = AnyClass::get(name) {
        return (class.superclass() == Some(original)).then_some(class);
    }
    let mut class = ClassBuilder::new(name, original)?;
    // SAFETY: these selectors and signatures match AppKit's NSDraggingDestination
    // protocol. The resulting class is used only for main-thread NSView instances.
    unsafe {
        class.add_method(sel!(draggingEntered:), entered as DragOperation<'_, '_>);
        class.add_method(sel!(draggingUpdated:), updated as DragOperation<'_, '_>);
        class.add_method(sel!(draggingExited:), exited as DragExit<'_, '_>);
        class.add_method(
            sel!(wantsPeriodicDraggingUpdates),
            wants_periodic_updates as DragUpdates<'_>,
        );
        class.add_method(
            sel!(prepareForDragOperation:),
            prepare as DropOperation<'_, '_>,
        );
        class.add_method(
            sel!(performDragOperation:),
            perform as DropOperation<'_, '_>,
        );
    }
    Some(class.register())
}

type DragOperation<'view, 'drag> =
    extern "C-unwind" fn(&'view NSView, Sel, &'drag DragInfo) -> NSDragOperation;
type DragExit<'view, 'drag> = extern "C-unwind" fn(&'view NSView, Sel, Option<&'drag DragInfo>);
type DragUpdates<'view> = extern "C-unwind" fn(&'view NSView, Sel) -> Bool;
type DropOperation<'view, 'drag> =
    extern "C-unwind" fn(&'view NSView, Sel, &'drag DragInfo) -> Bool;

fn operation(info: &DragInfo) -> NSDragOperation {
    let allowed = info.draggingSourceOperationMask();
    if allowed.contains(NSDragOperation::Copy) {
        NSDragOperation::Copy
    } else {
        allowed & NSDragOperation::Link
    }
}

extern "C-unwind" fn entered(view: &NSView, _: Sel, info: &DragInfo) -> NSDragOperation {
    let Some((key, route)) = lookup(view) else {
        return NSDragOperation::None;
    };
    let paths = payload::read(&info.draggingPasteboard());
    route.accepts_drag.set(!paths.is_empty());
    if !paths.is_empty()
        && let Some(position) = Position::new(view, info.draggingLocation())
    {
        queue(key, Event::Entered(paths, position));
        operation(info)
    } else {
        queue(key, Event::Left);
        NSDragOperation::None
    }
}

extern "C-unwind" fn updated(view: &NSView, _: Sel, info: &DragInfo) -> NSDragOperation {
    if let Some((key, route)) = lookup(view)
        && route.accepts_drag.get()
        && let Some(position) = Position::new(view, info.draggingLocation())
    {
        queue(key, Event::Moved(position));
        return operation(info);
    }
    NSDragOperation::None
}

extern "C-unwind" fn exited(view: &NSView, _: Sel, _: Option<&DragInfo>) {
    if let Some((key, route)) = lookup(view) {
        route.accepts_drag.set(false);
        queue(key, Event::Left);
    }
}

extern "C-unwind" fn wants_periodic_updates(_: &NSView, _: Sel) -> Bool {
    // Playlist hover already measures dwell time using the last reported position.
    Bool::NO
}

extern "C-unwind" fn prepare(view: &NSView, _: Sel, info: &DragInfo) -> Bool {
    Bool::new(
        !operation(info).is_empty()
            && lookup(view).is_some_and(|(_, route)| route.accepts_drag.get()),
    )
}

extern "C-unwind" fn perform(view: &NSView, _: Sel, info: &DragInfo) -> Bool {
    let Some((key, route)) = lookup(view) else {
        return Bool::NO;
    };
    route.accepts_drag.set(false);
    let paths = payload::read(&info.draggingPasteboard());
    if !paths.is_empty()
        && let Some(position) = Position::new(view, info.draggingLocation())
    {
        queue(key, Event::Dropped(paths, position));
        Bool::YES
    } else {
        queue(key, Event::Left);
        Bool::NO
    }
}
