//! A bounded, asynchronous transfer between independent Slint GL contexts.

use glow::{Context, HasContext, NativeBuffer, NativeFence, NativeFramebuffer, NativeTexture};
use riina_mpv::video::VideoSurface;
use slint::{Image, Rgba8Pixel, SharedPixelBuffer};
use std::{
    ffi::{CStr, c_void},
    num::NonZeroU32,
    time::{Duration, Instant},
};

const INTERVAL: Duration = Duration::from_millis(34);
const MAX_EDGE: u32 = 640;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct FrameKey {
    pub(super) preview: u64,
    pub(super) media: u64,
    pub(super) seek: u64,
}

struct Readback {
    key: FrameKey,
    fence: NativeFence,
}

pub(super) struct FrameTransfer {
    gl: Context,
    fbo: NativeFramebuffer,
    texture: NativeTexture,
    buffer: NativeBuffer,
    dimensions: (u32, u32),
    pending: Option<Readback>,
    last_submission: Option<Instant>,
}

impl FrameTransfer {
    /// # Safety
    /// The loader and current context must belong to the player's renderer.
    pub(super) unsafe fn new(get_proc: &dyn Fn(&CStr) -> *const c_void) -> Result<Self, String> {
        // SAFETY: the caller keeps the renderer current during allocation and teardown.
        let gl = unsafe { Context::from_loader_function_cstr(get_proc) };
        let version = gl.version();
        if version.major < 3 || (!version.is_embedded && version.major == 3 && version.minor < 2) {
            return Err("Picture-in-Picture requires OpenGL sync and pixel buffers".into());
        }
        // SAFETY: all resources are allocated in the checked current context.
        let fbo = unsafe { gl.create_framebuffer()? };
        // SAFETY: the creating context is still current.
        let texture = match unsafe { gl.create_texture() } {
            Ok(texture) => texture,
            Err(error) => {
                // SAFETY: the framebuffer belongs to this context and was just allocated.
                unsafe { gl.delete_framebuffer(fbo) };
                return Err(error.to_string());
            }
        };
        // SAFETY: the creating context is still current.
        let buffer = match unsafe { gl.create_buffer() } {
            Ok(buffer) => buffer,
            Err(error) => {
                // SAFETY: both newly allocated names belong to this context.
                unsafe {
                    gl.delete_texture(texture);
                    gl.delete_framebuffer(fbo);
                }
                return Err(error.to_string());
            }
        };
        Ok(Self {
            gl,
            fbo,
            texture,
            buffer,
            dimensions: (0, 0),
            pending: None,
            last_submission: None,
        })
    }

    /// # Safety
    /// The creating renderer must be current. This method never waits for a frame.
    pub(super) unsafe fn collect(&mut self) -> Result<Option<(FrameKey, Image)>, String> {
        let Some(pending) = self.pending.as_ref() else {
            return Ok(None);
        };
        // SAFETY: this fence belongs to the retained current context. A zero timeout polls only.
        match unsafe { self.gl.client_wait_sync(pending.fence, 0, 0) } {
            glow::TIMEOUT_EXPIRED => return Ok(None),
            glow::ALREADY_SIGNALED | glow::CONDITION_SATISFIED => {}
            _ => return Err("Could not read the Picture-in-Picture frame fence".into()),
        }
        let Some(pending) = self.pending.take() else {
            return Ok(None);
        };
        let (width, height) = self.dimensions;
        let mut pixels = SharedPixelBuffer::<Rgba8Pixel>::new(width, height);
        let dest = pixels.make_mut_bytes();
        // SAFETY: the signaled fence covers exactly this one pixel buffer. Copy
        // only the original RGBA8 payload and restore the caller's pack binding.
        unsafe {
            self.gl.delete_sync(pending.fence);
            let previous = self.buffer_binding(glow::PIXEL_PACK_BUFFER_BINDING);
            self.gl
                .bind_buffer(glow::PIXEL_PACK_BUFFER, Some(self.buffer));
            let mapped = self.gl.map_buffer_range(
                glow::PIXEL_PACK_BUFFER,
                0,
                dest.len() as i32,
                glow::MAP_READ_BIT,
            );
            if !mapped.is_null() {
                dest.copy_from_slice(std::slice::from_raw_parts(mapped, dest.len()));
                self.gl.unmap_buffer(glow::PIXEL_PACK_BUFFER);
            }
            self.gl.bind_buffer(glow::PIXEL_PACK_BUFFER, previous);
            if mapped.is_null() {
                return Err("Could not map the Picture-in-Picture frame".into());
            }
        }
        Ok(Some((pending.key, Image::from_rgba8(pixels))))
    }

    /// # Safety
    /// Both video and the transfer framebuffer belong to the current context.
    pub(super) unsafe fn submit(
        &mut self,
        video: &mut VideoSurface,
        key: FrameKey,
        dimensions: (u32, u32),
    ) -> Result<bool, String> {
        let now = Instant::now();
        if self.pending.is_some()
            || self
                .last_submission
                .is_some_and(|last| now.duration_since(last) < INTERVAL)
        {
            return Ok(false);
        }
        let (width, height) = bounded_dimensions(dimensions);
        // SAFETY: these are framebuffer operations on the creating context.
        unsafe { self.configure((width, height))? };
        // SAFETY: configure has checked the color attachment and its size.
        unsafe { video.render_to_framebuffer(self.fbo.0, width, height, glow::RGBA8 as i32) }
            .map_err(|error| error.to_string())?;
        // SAFETY: the transfer's checked framebuffer and buffer are currently live.
        let fence = unsafe { self.queue_pixels(width as i32, height as i32)? };
        self.pending = Some(Readback { key, fence });
        self.last_submission = Some(now);
        Ok(true)
    }

    unsafe fn configure(&mut self, size: (u32, u32)) -> Result<(), String> {
        if size == self.dimensions {
            return Ok(());
        }
        let gl = &self.gl;
        // SAFETY: all queried bindings belong to this context. Allocating storage
        // uses an unbound unpack buffer so a null data pointer denotes no upload.
        let complete = unsafe {
            let texture = NonZeroU32::new(gl.get_parameter_i32(glow::TEXTURE_BINDING_2D) as u32)
                .map(NativeTexture);
            let unpack = self.buffer_binding(glow::PIXEL_UNPACK_BUFFER_BINDING);
            let pack = self.buffer_binding(glow::PIXEL_PACK_BUFFER_BINDING);
            let draw = self.framebuffer_binding(glow::DRAW_FRAMEBUFFER_BINDING);
            gl.bind_texture(glow::TEXTURE_2D, Some(self.texture));
            gl.bind_buffer(glow::PIXEL_UNPACK_BUFFER, None);
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA8 as i32,
                size.0 as i32,
                size.1 as i32,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(None),
            );
            gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, Some(self.fbo));
            gl.framebuffer_texture_2d(
                glow::DRAW_FRAMEBUFFER,
                glow::COLOR_ATTACHMENT0,
                glow::TEXTURE_2D,
                Some(self.texture),
                0,
            );
            let complete =
                gl.check_framebuffer_status(glow::DRAW_FRAMEBUFFER) == glow::FRAMEBUFFER_COMPLETE;
            gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, draw);
            gl.bind_buffer(glow::PIXEL_PACK_BUFFER, Some(self.buffer));
            gl.buffer_data_size(
                glow::PIXEL_PACK_BUFFER,
                (size.0 * size.1 * 4) as i32,
                glow::STREAM_READ,
            );
            gl.bind_buffer(glow::PIXEL_PACK_BUFFER, pack);
            gl.bind_buffer(glow::PIXEL_UNPACK_BUFFER, unpack);
            gl.bind_texture(glow::TEXTURE_2D, texture);
            complete
        };
        if !complete {
            return Err("The Linux Picture-in-Picture framebuffer is incomplete".into());
        }
        self.dimensions = size;
        Ok(())
    }

    unsafe fn queue_pixels(&self, width: i32, height: i32) -> Result<NativeFence, String> {
        let gl = &self.gl;
        // SAFETY: the bounded framebuffer is complete and a tightly packed
        // RGBA8 result has exactly width * height * 4 bytes.
        unsafe {
            let read = self.framebuffer_binding(glow::READ_FRAMEBUFFER_BINDING);
            let pack = self.buffer_binding(glow::PIXEL_PACK_BUFFER_BINDING);
            let pack_state = [
                (glow::PACK_ALIGNMENT, 4),
                (glow::PACK_ROW_LENGTH, 0),
                (glow::PACK_SKIP_PIXELS, 0),
                (glow::PACK_SKIP_ROWS, 0),
            ]
            .map(|(key, value)| {
                let prior = gl.get_parameter_i32(key);
                gl.pixel_store_i32(key, value);
                (key, prior)
            });
            gl.bind_framebuffer(glow::READ_FRAMEBUFFER, Some(self.fbo));
            gl.bind_buffer(glow::PIXEL_PACK_BUFFER, Some(self.buffer));
            gl.read_pixels(
                0,
                0,
                width,
                height,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelPackData::BufferOffset(0),
            );
            let fence = gl.fence_sync(glow::SYNC_GPU_COMMANDS_COMPLETE, 0);
            gl.flush();
            gl.bind_buffer(glow::PIXEL_PACK_BUFFER, pack);
            gl.bind_framebuffer(glow::READ_FRAMEBUFFER, read);
            for (key, prior) in pack_state {
                gl.pixel_store_i32(key, prior);
            }
            fence.map_err(|error| error.to_string())
        }
    }

    unsafe fn buffer_binding(&self, key: u32) -> Option<NativeBuffer> {
        // SAFETY: callers supply a buffer binding enum from this current context.
        NonZeroU32::new(unsafe { self.gl.get_parameter_i32(key) } as u32).map(NativeBuffer)
    }

    unsafe fn framebuffer_binding(&self, key: u32) -> Option<NativeFramebuffer> {
        // SAFETY: callers supply a framebuffer binding enum from this current context.
        NonZeroU32::new(unsafe { self.gl.get_parameter_i32(key) } as u32).map(NativeFramebuffer)
    }
}

impl Drop for FrameTransfer {
    fn drop(&mut self) {
        // SAFETY: the rendering notifier retains the creating context through teardown.
        unsafe {
            if let Some(pending) = self.pending.take() {
                self.gl.delete_sync(pending.fence);
            }
            self.gl.delete_buffer(self.buffer);
            self.gl.delete_texture(self.texture);
            self.gl.delete_framebuffer(self.fbo);
        }
    }
}

fn bounded_dimensions((width, height): (u32, u32)) -> (u32, u32) {
    let edge = width.max(height);
    if edge <= MAX_EDGE {
        return (width, height);
    }
    let scale = |axis| ((u64::from(axis) * u64::from(MAX_EDGE)) / u64::from(edge)).max(1) as u32;
    (scale(width), scale(height))
}

#[cfg(test)]
mod tests {
    use super::bounded_dimensions;

    #[test]
    fn preview_frames_preserve_orientation_and_bound_both_axes() {
        assert_eq!(bounded_dimensions((1920, 1080)), (640, 360));
        assert_eq!(bounded_dimensions((1080, 1920)), (360, 640));
        assert_eq!(bounded_dimensions((1, 4000)), (1, 640));
        assert_eq!(bounded_dimensions((320, 240)), (320, 240));
    }
}
