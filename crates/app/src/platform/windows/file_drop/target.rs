//! Translate OLE drag callbacks into ordered player events.
use super::{
    Event, Key, Preview,
    payload::{self, Kind},
    queue,
};
use std::{cell::RefCell, path::PathBuf, ptr::NonNull};
use windows::{
    Win32::{
        Foundation::{E_POINTER, POINT, POINTL},
        Graphics::Gdi::ScreenToClient,
        System::{
            Com::IDataObject,
            Ole::{
                DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_LINK, DROPEFFECT_NONE, IDropTarget,
                IDropTarget_Impl,
            },
            SystemServices::MODIFIERKEYS_FLAGS,
        },
    },
    core::{Ref, Result, implement},
};

#[implement(IDropTarget)]
pub(super) struct DropTarget {
    key: Key,
    drag: RefCell<Option<Drag>>,
}

struct Drag {
    formats: Vec<payload::Format>,
    last: POINT,
    link_only: bool,
}

impl DropTarget {
    pub(super) fn interface(key: Key) -> IDropTarget {
        Self {
            key,
            drag: RefCell::new(None),
        }
        .into()
    }

    fn client(&self, point: &POINTL) -> Option<POINT> {
        let mut point = POINT {
            x: point.x,
            y: point.y,
        };
        // SAFETY: OLE is invoking this target for its attached HWND.
        unsafe { ScreenToClient(self.key.window(), &mut point) }
            .as_bool()
            .then_some(point)
    }
}

impl IDropTarget_Impl for DropTarget_Impl {
    fn DragEnter(
        &self,
        data: Ref<IDataObject>,
        _: MODIFIERKEYS_FLAGS,
        point: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> Result<()> {
        let effect = Effect::new(effect)?;
        self.drag.borrow_mut().take();
        let (Some(data), Some(point)) = (data.as_ref(), self.client(point)) else {
            return Ok(());
        };
        let formats = payload::available(data);
        // QueryGetData is advisory. Some Shell objects advertise a file format
        // they cannot render, including drag sources that supply only a URL.
        let files = formats
            .iter()
            .find(|format| format.kind == Kind::Files)
            .map_or_else(Vec::new, |format| format.read(data));
        let link_only = files.is_empty();
        let accepted = effect.preferred(link_only);
        if formats.is_empty() || accepted == DROPEFFECT_NONE {
            return Ok(());
        }
        let preview = if !link_only {
            Some(Preview::Files(
                files.into_iter().map(PathBuf::from).collect(),
            ))
        } else if formats
            .iter()
            .any(|format| matches!(format.kind, Kind::WideLink | Kind::AnsiLink))
        {
            Some(Preview::Link)
        } else {
            None
        };
        queue(self.key, Event::Entered(preview, point));
        *self.drag.borrow_mut() = Some(Drag {
            formats,
            last: point,
            link_only,
        });
        effect.set(accepted);
        Ok(())
    }

    fn DragOver(
        &self,
        _: MODIFIERKEYS_FLAGS,
        point: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> Result<()> {
        let effect = Effect::new(effect)?;
        if let (Some(drag), Some(point)) = (self.drag.borrow_mut().as_mut(), self.client(point)) {
            effect.set(effect.preferred(drag.link_only));
            if point != drag.last {
                drag.last = point;
                queue(self.key, Event::Motion(point));
            }
        }
        Ok(())
    }

    fn DragLeave(&self) -> Result<()> {
        self.drag.borrow_mut().take();
        queue(self.key, Event::Left);
        Ok(())
    }

    fn Drop(
        &self,
        data: Ref<IDataObject>,
        _: MODIFIERKEYS_FLAGS,
        point: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> Result<()> {
        let effect = Effect::new(effect)?;
        let drag = self.drag.borrow_mut().take();
        if let (Some(drag), Some(data), Some(point)) = (drag, data.as_ref(), self.client(point)) {
            let accepted = effect.preferred(drag.link_only);
            if accepted != DROPEFFECT_NONE
                && let Some(paths) = drag
                    .formats
                    .into_iter()
                    .map(|format| format.read(data))
                    .find(|paths| !paths.is_empty())
            {
                queue(self.key, Event::Drop(paths, point));
                effect.set(accepted);
                return Ok(());
            }
        }
        queue(self.key, Event::Left);
        Ok(())
    }
}

struct Effect {
    output: NonNull<DROPEFFECT>,
    allowed: DROPEFFECT,
}

impl Effect {
    fn new(output: *mut DROPEFFECT) -> Result<Self> {
        let output = NonNull::new(output).ok_or(E_POINTER)?;
        // SAFETY: OLE lends a writable effect value for the callback.
        let allowed = unsafe { output.as_ptr().replace(DROPEFFECT_NONE) };
        Ok(Self { output, allowed })
    }

    fn preferred(&self, link_only: bool) -> DROPEFFECT {
        if self.allowed.0 & DROPEFFECT_COPY.0 != 0 {
            DROPEFFECT_COPY
        } else if link_only && self.allowed.0 & DROPEFFECT_LINK.0 != 0 {
            DROPEFFECT_LINK
        } else {
            DROPEFFECT_NONE
        }
    }

    fn set(&self, effect: DROPEFFECT) {
        // SAFETY: this guard is used only while the callback's output is live.
        unsafe { self.output.as_ptr().write(effect) };
    }
}
