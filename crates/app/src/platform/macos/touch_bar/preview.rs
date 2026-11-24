//! Draw Touch Bar previews from the player's existing, generation-scoped sample cache.
use crate::{PlaybackStore, thumbnail_preview::Controller};
use objc2::{AnyThread, DefinedClass, MainThreadOnly, define_class, rc::Retained};
use objc2_app_kit::{
    NSBezierPath, NSCompositingOperation, NSGraphicsContext, NSImage, NSSlider, NSSliderCell,
    NSUserInterfaceLayoutDirection,
};
use objc2_foundation::{NSData, NSPoint, NSRect, NSSize};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    rc::Rc,
};

const STRIP_TILES: u32 = 8;
const MAX_CACHED_IMAGES: usize = 24;

pub(crate) struct Preview {
    playback: PlaybackStore,
    thumbnails: Rc<RefCell<Controller>>,
    images: RefCell<Images>,
}

#[derive(Default)]
struct Images {
    revision: u64,
    generation: u64,
    images: BTreeMap<usize, Retained<NSImage>>,
}

impl Preview {
    pub(crate) fn new(playback: PlaybackStore, thumbnails: Rc<RefCell<Controller>>) -> Self {
        Self {
            playback,
            thumbnails,
            images: RefCell::default(),
        }
    }

    fn image(&self, fraction: f64) -> Option<Retained<NSImage>> {
        let playback = riina_core::sync::lock(&self.playback).clone();
        let duration = riina_core::touch_bar::seekable_duration(&playback)?;
        let thumbnails = self.thumbnails.try_borrow().ok()?;
        let (index, bytes) = thumbnails.touch_bar_sample(&playback, duration * fraction)?;
        let mut cache = self.images.borrow_mut();
        let revision = thumbnails.preview_revision();
        if (revision, playback.generation) != (cache.revision, cache.generation) {
            cache.images.clear();
            cache.revision = revision;
            cache.generation = playback.generation;
        }
        if let Some(image) = cache.images.get(&index) {
            return Some(image.clone());
        }
        let data = NSData::with_bytes(bytes);
        let image = NSImage::initWithData(NSImage::alloc(), &data)?;
        if cache.images.len() >= MAX_CACHED_IMAGES {
            cache.images.clear();
        }
        cache.images.insert(index, image.clone());
        Some(image)
    }

    fn draw_strip(&self, bar: NSRect, flipped: bool) {
        let width = bar.size.width / f64::from(STRIP_TILES);
        if width <= 0.0 || bar.size.height <= 0.0 {
            return;
        }
        NSGraphicsContext::saveGraphicsState_class();
        NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(bar, 3.0, 3.0).addClip();
        for index in 0..STRIP_TILES {
            let fraction = (f64::from(index) + 0.5) / f64::from(STRIP_TILES);
            if let Some(image) = self.image(fraction) {
                let tile = NSRect::new(
                    NSPoint::new(bar.origin.x + f64::from(index) * width, bar.origin.y),
                    NSSize::new(width, bar.size.height),
                );
                draw_cropped(&image, tile, flipped, 0.8);
            }
        }
        NSGraphicsContext::restoreGraphicsState_class();
    }
}

struct PreviewCellState {
    preview: Rc<Preview>,
    tracking: Rc<Cell<bool>>,
}

define_class!(
    #[unsafe(super = NSSliderCell)]
    #[thread_kind = MainThreadOnly]
    #[ivars = PreviewCellState]
    struct PreviewCell;

    impl PreviewCell {
        #[unsafe(method(drawBarInside:flipped:))]
        fn draw_bar(&self, frame: NSRect, flipped: bool) {
            // SAFETY: draw the inherited slider track with the original AppKit geometry.
            unsafe { objc2::msg_send![super(self), drawBarInside: frame, flipped: flipped] }
            // SAFETY: AppKit holds the owning control during its cell draw callback.
            if let Some(view) = unsafe { self.controlView() } {
                self.ivars().preview.draw_strip(filmstrip_frame(frame, view.bounds()), flipped);
            }
        }

        #[unsafe(method(drawKnob:))]
        fn draw_knob(&self, frame: NSRect) {
            if self.ivars().tracking.get()
                // SAFETY: AppKit holds the owning control during its cell draw callback.
                && let Some(view) = unsafe { self.controlView() }
                && let Some(image) = self.ivars().preview.image(self.doubleValue())
            {
                let bounds = view.bounds();
                let width = 60.0_f64.min(bounds.size.width);
                let x = (frame.origin.x + (frame.size.width - width) / 2.0)
                    .clamp(bounds.origin.x, bounds.origin.x + bounds.size.width - width);
                let horizontal = NSRect::new(NSPoint::new(x, frame.origin.y), NSSize::new(width, frame.size.height));
                let preview = filmstrip_frame(horizontal, bounds);
                NSGraphicsContext::saveGraphicsState_class();
                NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(preview, 3.0, 3.0).addClip();
                draw_cropped(&image, preview, view.isFlipped(), 1.0);
                NSGraphicsContext::restoreGraphicsState_class();
            } else {
                // SAFETY: preserve the native knob when no sampled image is available.
                unsafe { objc2::msg_send![super(self), drawKnob: frame] }
            }
        }
    }
);

pub(super) fn install(slider: &NSSlider, preview: Rc<Preview>, tracking: Rc<Cell<bool>>) {
    let cell = PreviewCell::alloc(slider.mtm()).set_ivars(PreviewCellState { preview, tracking });
    // SAFETY: initialize the allocated NSSliderCell subclass with its state already set.
    let cell: Retained<PreviewCell> = unsafe { objc2::msg_send![super(cell), init] };
    cell.setUserInterfaceLayoutDirection(NSUserInterfaceLayoutDirection::LeftToRight);
    slider.setCell(Some(&cell));
}

fn filmstrip_frame(horizontal: NSRect, bounds: NSRect) -> NSRect {
    let inset = 2.0_f64.min(bounds.size.height / 2.0);
    NSRect::new(
        NSPoint::new(horizontal.origin.x, bounds.origin.y + inset),
        NSSize::new(horizontal.size.width, bounds.size.height - 2.0 * inset),
    )
}

fn draw_cropped(image: &NSImage, destination: NSRect, flipped: bool, opacity: f64) {
    let source = image.size();
    if source.width <= 0.0
        || source.height <= 0.0
        || destination.size.width <= 0.0
        || destination.size.height <= 0.0
    {
        return;
    }
    let ratio = destination.size.width / destination.size.height;
    let width = source.width.min(source.height * ratio);
    let height = source.height.min(source.width / ratio);
    let crop = NSRect::new(
        NSPoint::new((source.width - width) / 2.0, (source.height - height) / 2.0),
        NSSize::new(width, height),
    );
    // SAFETY: the view has an active AppKit drawing context. No optional hints
    // or context-owned data are passed to NSImage.
    unsafe {
        image.drawInRect_fromRect_operation_fraction_respectFlipped_hints(
            destination,
            crop,
            NSCompositingOperation::SourceOver,
            opacity,
            flipped,
            None,
        );
    }
}
