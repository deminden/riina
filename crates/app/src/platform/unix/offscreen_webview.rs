//! Supply a transparent WebKitGTK surface for the shared player.
mod keyboard;
mod pointer;

use gtk::prelude::*;
use slint::{Image, Rgba8Pixel, SharedPixelBuffer};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::{Duration, Instant},
};
use webkit2gtk::WebViewExt as _;
use webkit2gtk::{HardwareAccelerationPolicy, SettingsExt, SnapshotOptions, SnapshotRegion};
use wry::{Rect, WebView, WebViewBuilder, WebViewBuilderExtUnix, WebViewExtUnix};

const FRAME_INTERVAL: Duration = Duration::from_millis(33);
const IDLE_INTERVAL: Duration = Duration::from_millis(125);
const ACTIVE_PERIOD: Duration = Duration::from_millis(500);

pub(in crate::platform) struct OffscreenWebView {
    view: WebView,
    capture: Rc<RefCell<Capture>>,
    size: Cell<(i32, i32)>,
    window: gtk::OffscreenWindow,
}

#[derive(Default)]
struct Capture {
    generation: u64,
    changed: bool,
    dirty: bool,
    last_change: Option<Instant>,
    last_request: Option<Instant>,
    pending: Option<gtk::gio::Cancellable>,
    pixels: Option<Image>,
}

impl OffscreenWebView {
    pub(in crate::platform) fn build(
        builder: WebViewBuilder,
        bounds: Rect,
        url: &str,
    ) -> Result<Self, String> {
        gtk::init().map_err(|error| error.to_string())?;
        let window = gtk::OffscreenWindow::new();
        let view = builder
            .build_gtk(&window)
            .map_err(|error| error.to_string())?;
        let widget = view.webview();
        if let Some(settings) = webkit2gtk::WebViewExt::settings(&widget) {
            // The first navigation chooses WebKit's scrolling pipeline. An
            // offscreen GTK window needs software rendering from that point.
            settings.set_hardware_acceleration_policy(HardwareAccelerationPolicy::Never);
        }
        view.load_url(url).map_err(|error| error.to_string())?;
        let capture = Rc::new(RefCell::new(Capture::default()));
        let damage = Rc::downgrade(&capture);
        window.connect_damage_event(move |_, _| {
            if let Some(capture) = damage.upgrade() {
                capture.borrow_mut().dirty = true;
            }
            false
        });
        let surface = Self {
            view,
            capture,
            size: Cell::default(),
            window,
        };
        surface.set_bounds(bounds)?;
        // Keep document loading and message callbacks alive while the layer is hidden.
        surface.window.show_all();
        Ok(surface)
    }

    pub(in crate::platform) fn browser(&self) -> &WebView {
        &self.view
    }

    pub(in crate::platform) fn set_bounds(&self, bounds: Rect) -> Result<(), String> {
        let size = bounds.size.to_logical::<i32>(1.0);
        let size = (size.width.max(1), size.height.max(1));
        if self.size.replace(size) != size {
            self.invalidate();
            self.view.webview().set_size_request(size.0, size.1);
            self.window.resize(size.0, size.1);
        }
        Ok(())
    }

    pub(in crate::platform) fn invalidate(&self) {
        let pending = {
            let mut state = self.capture.borrow_mut();
            state.generation = state.generation.wrapping_add(1);
            state.changed |= state.pixels.take().is_some();
            state.dirty = true;
            state.pending.take()
        };
        if let Some(pending) = pending {
            pending.cancel();
        }
    }

    pub(in crate::platform) fn capture(&self) {
        let widget = self.view.webview();
        let (generation, request) = {
            let mut state = self.capture.borrow_mut();
            // WebKit's offscreen page can repaint without a GTK damage event.
            // Resume more frequent captures while its pixels are changing.
            let active = state.dirty
                || state
                    .last_change
                    .is_some_and(|time| time.elapsed() < ACTIVE_PERIOD);
            let interval = if active {
                FRAME_INTERVAL
            } else {
                IDLE_INTERVAL
            };
            if state.pending.is_some()
                || state
                    .last_request
                    .is_some_and(|time| time.elapsed() < interval)
                || self.size.get() != (widget.allocated_width(), widget.allocated_height())
            {
                return;
            }
            state.dirty = false;
            state.last_request = Some(Instant::now());
            let request = gtk::gio::Cancellable::new();
            state.pending = Some(request.clone());
            (state.generation, request)
        };
        let capture = Rc::downgrade(&self.capture);
        widget.snapshot(
            SnapshotRegion::Visible,
            SnapshotOptions::TRANSPARENT_BACKGROUND,
            Some(&request),
            move |result| {
                let Some(capture) = capture.upgrade() else {
                    return;
                };
                let mut capture = capture.borrow_mut();
                if capture.generation != generation {
                    return;
                }
                capture.pending = None;
                if let Some(image) = result
                    .ok()
                    .and_then(|surface| snapshot_image(surface, capture.pixels.as_ref()))
                {
                    capture.changed = true;
                    capture.last_change = Some(Instant::now());
                    capture.pixels = Some(image);
                }
            },
        );
    }

    pub(in crate::platform) fn take_changed(&self) -> bool {
        std::mem::take(&mut self.capture.borrow_mut().changed)
    }

    pub(in crate::platform) fn pixels(&self) -> Option<Image> {
        self.capture.borrow().pixels.clone()
    }

    pub(in crate::platform) fn pointer(
        &self,
        kind: gtk::gdk::EventType,
        point: (f64, f64),
        button: u32,
        held: u32,
        modifiers: u32,
    ) {
        pointer::dispatch(&self.view.webview(), kind, point, button, held, modifiers);
    }

    pub(in crate::platform) fn scroll(
        &self,
        point: (f64, f64),
        delta: (f64, f64),
        held: u32,
        modifiers: u32,
    ) {
        pointer::scroll(&self.view.webview(), point, delta, held, modifiers);
    }

    pub(in crate::platform) fn focus(&self, focused: bool) {
        keyboard::focus(&self.view.webview(), focused);
    }

    pub(in crate::platform) fn key(&self, text: &str, modifiers: u32, released: bool) {
        keyboard::dispatch(&self.view.webview(), text, modifiers, released);
    }
}

impl Drop for OffscreenWebView {
    fn drop(&mut self) {
        self.invalidate();
    }
}

fn snapshot_image(surface: gtk::cairo::Surface, previous: Option<&Image>) -> Option<Image> {
    let image = gtk::cairo::ImageSurface::try_from(surface).ok()?;
    if image.format() != gtk::cairo::Format::ARgb32 {
        return None;
    }
    let width = u32::try_from(image.width()).ok()?;
    let height = u32::try_from(image.height()).ok()?;
    let stride = usize::try_from(image.stride()).ok()?;
    if width == 0 || height == 0 {
        return None;
    }
    let previous = previous
        .and_then(Image::to_rgba8_premultiplied)
        .filter(|pixels| pixels.width() == width && pixels.height() == height);
    let row_bytes = width as usize * 4;
    let mut updated = None;
    image
        .with_data(|bytes| {
            let rows = || {
                bytes
                    .chunks_exact(stride)
                    .map(|row| row[..row_bytes].as_chunks::<4>().0)
            };
            if previous.is_some_and(|pixels| {
                pixels
                    .as_slice()
                    .iter()
                    .zip(rows().flatten())
                    .all(|(pixel, &argb)| *pixel == rgba_pixel(argb))
            }) {
                return;
            }
            let mut pixels = SharedPixelBuffer::<Rgba8Pixel>::new(width, height);
            for (target, source) in pixels
                .make_mut_slice()
                .chunks_exact_mut(width as usize)
                .zip(rows())
            {
                for (pixel, &argb) in target.iter_mut().zip(source) {
                    *pixel = rgba_pixel(argb);
                }
            }
            updated = Some(Image::from_rgba8_premultiplied(pixels));
        })
        .ok()?;
    updated
}

fn rgba_pixel(native_argb: [u8; 4]) -> Rgba8Pixel {
    // Cairo stores native-endian premultiplied ARGB words. Slint uses RGBA bytes.
    let [a, r, g, b] = u32::from_ne_bytes(native_argb).to_be_bytes();
    Rgba8Pixel { r, g, b, a }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transparent_cairo_rows_keep_premultiplied_channels_and_skip_padding() {
        let mut data = Vec::new();
        for value in [0x8040_2010_u32, 0xffff_ffff, 0x0000_0000, 0xffff_ffff] {
            data.extend(value.to_ne_bytes());
        }
        let surface =
            gtk::cairo::ImageSurface::create_for_data(data, gtk::cairo::Format::ARgb32, 1, 2, 8)
                .unwrap();
        let image = snapshot_image(surface.as_ref().clone(), None).unwrap();
        let pixels = image.to_rgba8_premultiplied().unwrap();
        assert_eq!(pixels.as_bytes(), &[64, 32, 16, 128, 0, 0, 0, 0]);
        assert!(snapshot_image(surface.as_ref().clone(), Some(&image)).is_none());
    }
}
