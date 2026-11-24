//! Temporarily rebind the player's existing GL context without swapping its window.

use libloading::Library;
use std::ffi::{CStr, c_int, c_uint, c_ulong, c_void};

type Pointer = *mut c_void;

#[derive(Clone, Copy, PartialEq, Eq)]
struct Current<Drawable> {
    display: Pointer,
    context: Pointer,
    draw: Drawable,
    read: Drawable,
}

struct NativeApi<Drawable, Result> {
    display: unsafe extern "C" fn() -> Pointer,
    context: unsafe extern "C" fn() -> Pointer,
    surfaces: SurfaceQueries<Drawable>,
    make_current: unsafe extern "C" fn(Pointer, Drawable, Drawable, Pointer) -> Result,
    _library: Library,
}

enum SurfaceQueries<Drawable> {
    Separate {
        draw: unsafe extern "C" fn() -> Drawable,
        read: unsafe extern "C" fn() -> Drawable,
    },
    Selector(unsafe extern "C" fn(c_int) -> Drawable),
}

enum SurfaceSymbols {
    Separate(&'static CStr, &'static CStr),
    Selector(&'static CStr),
}

impl<Drawable: Copy + Default + Eq, Status: Into<i64>> NativeApi<Drawable, Status> {
    unsafe fn load(
        library: &str,
        [display, context, make_current]: [&CStr; 3],
        surfaces: SurfaceSymbols,
    ) -> Option<Self> {
        // SAFETY: these are the platform GL client libraries with the public EGL/GLX ABI.
        let library = unsafe { Library::new(library).ok()? };
        // SAFETY: the caller provides the matching platform entry points and ABI types.
        unsafe {
            Some(Self {
                display: *library.get(display.to_bytes_with_nul()).ok()?,
                context: *library.get(context.to_bytes_with_nul()).ok()?,
                make_current: *library.get(make_current.to_bytes_with_nul()).ok()?,
                surfaces: match surfaces {
                    SurfaceSymbols::Separate(draw, read) => SurfaceQueries::Separate {
                        draw: *library.get(draw.to_bytes_with_nul()).ok()?,
                        read: *library.get(read.to_bytes_with_nul()).ok()?,
                    },
                    SurfaceSymbols::Selector(name) => {
                        SurfaceQueries::Selector(*library.get(name.to_bytes_with_nul()).ok()?)
                    }
                },
                _library: library,
            })
        }
    }

    fn current(&self) -> Current<Drawable> {
        // SAFETY: each function queries a current value on this UI thread.
        unsafe {
            let (draw, read) = match self.surfaces {
                SurfaceQueries::Separate { draw, read } => (draw(), read()),
                SurfaceQueries::Selector(query) => (query(0x3059), query(0x305a)), // EGL_DRAW, EGL_READ
            };
            Current {
                display: (self.display)(),
                context: (self.context)(),
                draw,
                read,
            }
        }
    }

    unsafe fn set_current(&self, binding: Current<Drawable>) -> bool {
        // SAFETY: the owner keeps each non-null context and drawable live through this call.
        unsafe {
            (self.make_current)(binding.display, binding.draw, binding.read, binding.context).into()
                != 0
        }
    }
}

trait Binding {
    unsafe fn with_current(
        &self,
        draw: &mut dyn FnMut() -> Result<(), String>,
    ) -> Result<(), String>;
}

struct Saved<Drawable, Result> {
    api: NativeApi<Drawable, Result>,
    player: Current<Drawable>,
}

impl<Drawable: Copy + Default + Eq, Status: Into<i64>> Binding for Saved<Drawable, Status> {
    unsafe fn with_current(
        &self,
        draw: &mut dyn FnMut() -> Result<(), String>,
    ) -> Result<(), String> {
        let previous = self.api.current();
        if previous == self.player {
            return draw();
        }
        // SAFETY: the caller retains the player's native renderer while drawing.
        if !unsafe { self.api.set_current(self.player) } {
            return Err("Could not bind the player's Picture-in-Picture graphics context".into());
        }
        let restore = Restore {
            api: &self.api,
            previous: if previous.display.is_null() {
                Current {
                    display: self.player.display,
                    context: std::ptr::null_mut(),
                    draw: Drawable::default(),
                    read: Drawable::default(),
                }
            } else {
                previous
            },
        };
        let result = draw();
        drop(restore);
        result
    }
}

struct Restore<'a, Drawable: Copy + Default + Eq, Status: Into<i64>> {
    api: &'a NativeApi<Drawable, Status>,
    previous: Current<Drawable>,
}

impl<Drawable: Copy + Default + Eq, Status: Into<i64>> Drop for Restore<'_, Drawable, Status> {
    fn drop(&mut self) {
        // SAFETY: the main-thread draw did not destroy either renderer. A null context unbinds it.
        if !unsafe { self.api.set_current(self.previous) } {
            crate::diagnostics::warn!("Could not restore the prior graphics context");
        }
    }
}

pub(super) struct PlayerBinding(Box<dyn Binding>);

impl PlayerBinding {
    pub(super) fn capture() -> Option<Self> {
        // SAFETY: EGL uses opaque pointer handles and returns EGLBoolean.
        if let Some(binding) = unsafe {
            NativeApi::<Pointer, c_uint>::load(
                "libEGL.so.1",
                [
                    c"eglGetCurrentDisplay",
                    c"eglGetCurrentContext",
                    c"eglMakeCurrent",
                ],
                SurfaceSymbols::Selector(c"eglGetCurrentSurface"),
            )
        }
        .and_then(Self::save)
        {
            return Some(binding);
        }
        // SAFETY: GLX uses XID drawables and returns the C Xlib Bool type.
        unsafe {
            NativeApi::<c_ulong, c_int>::load(
                "libGLX.so.0",
                [
                    c"glXGetCurrentDisplay",
                    c"glXGetCurrentContext",
                    c"glXMakeContextCurrent",
                ],
                SurfaceSymbols::Separate(c"glXGetCurrentDrawable", c"glXGetCurrentReadDrawable"),
            )
        }
        .and_then(Self::save)
    }

    fn save<D: Copy + Default + Eq + 'static, R: Into<i64> + 'static>(
        api: NativeApi<D, R>,
    ) -> Option<Self> {
        let player = api.current();
        (!player.context.is_null() && !player.display.is_null())
            .then(|| Self(Box::new(Saved { api, player })))
    }

    /// # Safety
    /// The original graphics context and its native drawable must still belong to the player.
    pub(super) unsafe fn draw(
        &self,
        mut draw: impl FnMut() -> Result<(), String>,
    ) -> Result<(), String> {
        // SAFETY: the caller keeps the captured renderer and any prior binding alive.
        unsafe { self.0.with_current(&mut draw) }
    }
}
