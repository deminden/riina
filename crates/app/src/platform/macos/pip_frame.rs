//! IOSurface-backed sample buffers rendered by the player's existing CGL context.
// CGL is deprecated by Apple, but it is the context Slint and libmpv currently share.
#![allow(deprecated)]

use glow::{
    CLAMP_TO_EDGE, HasContext, LINEAR, TEXTURE_MAG_FILTER, TEXTURE_MIN_FILTER, TEXTURE_RECTANGLE,
    TEXTURE_WRAP_S, TEXTURE_WRAP_T,
};
use objc2_core_foundation::{CFBoolean, CFDictionary, CFNumber, CFRetained, CFString, CFType};
use objc2_core_media::{
    CMSampleBuffer, CMSampleTimingInfo, CMTime, CMVideoFormatDescription,
    CMVideoFormatDescriptionCreateForImageBuffer, kCMTimeInvalid,
};
use objc2_core_video::{
    CVPixelBuffer, CVPixelBufferGetIOSurface, CVPixelBufferPool, kCVPixelBufferHeightKey,
    kCVPixelBufferIOSurfacePropertiesKey, kCVPixelBufferOpenGLCompatibilityKey,
    kCVPixelBufferPixelFormatTypeKey, kCVPixelBufferPoolAllocationThresholdKey,
    kCVPixelBufferWidthKey, kCVPixelFormatType_32BGRA, kCVReturnSuccess,
};
use objc2_open_gl::{
    CGLContextObj, CGLGetCurrentContext, CGLSetCurrentContext, CGLTexImageIOSurface2D,
};
use std::{
    ffi::CStr,
    num::NonZeroU32,
    ptr::NonNull,
    time::{Duration, Instant},
};

const MAX_DIMENSION: u32 = 640;
const POOL_LIMIT: i32 = 3;
const SAMPLE_INTERVAL: Duration = Duration::from_millis(33);

pub(crate) struct PipFrameSource {
    gl: glow::Context,
    context: CGLContextObj,
    framebuffer: glow::NativeFramebuffer,
    pool: Option<CFRetained<CVPixelBufferPool>>,
    size: (u32, u32),
    last_sample_at: Option<Instant>,
}

impl PipFrameSource {
    /// # Safety
    /// Must be called while Slint's macOS OpenGL context is current. The
    /// loader must resolve GL symbols for that same context.
    pub(crate) unsafe fn new(
        loader: &dyn Fn(&CStr) -> *const std::ffi::c_void,
    ) -> Result<Self, String> {
        // SAFETY: querying the calling thread's current CGL context takes no arguments.
        let context = unsafe { CGLGetCurrentContext() };
        if context.is_null() {
            return Err("The macOS OpenGL context is unavailable".into());
        }
        // SAFETY: the caller supplies a symbol loader for the checked current context.
        let gl = unsafe { glow::Context::from_loader_function_cstr(|name| loader(name)) };
        // SAFETY: that CGL context remains current on this thread during allocation.
        let framebuffer = unsafe { gl.create_framebuffer() }.map_err(|e| e.to_string())?;
        Ok(Self {
            gl,
            context,
            framebuffer,
            pool: None,
            size: (0, 0),
            last_sample_at: None,
        })
    }

    pub(crate) fn reset_after_seek(&mut self) {
        self.last_sample_at = None;
    }

    /// # Safety
    /// The caller must keep the Slint context alive until the returned guard drops.
    pub(crate) unsafe fn bind_context(&self) -> Result<CurrentContext, String> {
        // SAFETY: the caller owns the same live context from RenderingSetup.
        let previous = unsafe { CGLGetCurrentContext() };
        if previous != self.context {
            // SAFETY: the retained renderer's context is live on this UI thread.
            let status = unsafe { CGLSetCurrentContext(self.context) };
            if status.0 != 0 {
                return Err(format!(
                    "Could not bind the Picture-in-Picture context ({})",
                    status.0
                ));
            }
        }
        Ok(CurrentContext {
            previous,
            changed: previous != self.context,
        })
    }

    /// # Safety
    /// Must run on the same current CGL context used to create this source.
    #[expect(
        clippy::expect_used,
        reason = "A pool is initialized for every nonzero bounded size and Core Video outputs are checked for null"
    )]
    pub(crate) unsafe fn sample(
        &mut self,
        video: &mut riina_mpv::video::VideoSurface,
        dimensions: (u32, u32),
        presentation_time: CMTime,
    ) -> Result<Option<CFRetained<CMSampleBuffer>>, String> {
        let now = Instant::now();
        if self
            .last_sample_at
            .is_some_and(|last| now.saturating_duration_since(last) < SAMPLE_INTERVAL)
        {
            return Ok(None);
        }
        let size = bounded_dimensions(dimensions);
        if size != self.size {
            self.pool = Some(create_pool(size)?);
            self.size = size;
        }
        let pool = self.pool.as_ref().expect("pool initialized for frame size");
        let mut pixel_buffer = std::ptr::null_mut();
        let aux = allocation_limit_attributes()?;
        // SAFETY: the pool and attribute dictionary are live and the out pointer is writable.
        let status = unsafe {
            CVPixelBufferPool::create_pixel_buffer_with_aux_attributes(
                None,
                pool,
                Some(&aux),
                NonNull::from(&mut pixel_buffer),
            )
        };
        if status != kCVReturnSuccess || pixel_buffer.is_null() {
            // The pool's hard allocation threshold deliberately drops frames
            // while AVKit retains all three previously enqueued surfaces.
            return Ok(None);
        }
        // SAFETY: the successful Create call returns a retained, non-null pixel buffer.
        let pixel_buffer =
            unsafe { CFRetained::from_raw(NonNull::new(pixel_buffer).expect("checked non-null")) };
        let surface = CVPixelBufferGetIOSurface(Some(&pixel_buffer)).ok_or_else(|| {
            "Core Video did not allocate an IOSurface-backed pixel buffer".to_owned()
        })?;
        // SAFETY: the caller keeps the creating context current. These binding
        // queries return values from that context and do not take host pointers.
        let (texture, previous_draw, previous_read, previous_texture) = unsafe {
            (
                self.gl.create_texture().map_err(|e| e.to_string())?,
                NonZeroU32::new(self.gl.get_parameter_i32(glow::DRAW_FRAMEBUFFER_BINDING) as u32)
                    .map(glow::NativeFramebuffer),
                NonZeroU32::new(self.gl.get_parameter_i32(glow::READ_FRAMEBUFFER_BINDING) as u32)
                    .map(glow::NativeFramebuffer),
                NonZeroU32::new(self.gl.get_parameter_i32(glow::TEXTURE_BINDING_RECTANGLE) as u32)
                    .map(glow::NativeTexture),
            )
        };
        let result = (|| {
            // SAFETY: this context owns the texture and framebuffer, and the
            // retained pixel buffer supplies the IOSurface for the attachment.
            unsafe {
                self.gl.bind_texture(TEXTURE_RECTANGLE, Some(texture));
                self.gl
                    .tex_parameter_i32(TEXTURE_RECTANGLE, TEXTURE_MIN_FILTER, LINEAR as i32);
                self.gl
                    .tex_parameter_i32(TEXTURE_RECTANGLE, TEXTURE_MAG_FILTER, LINEAR as i32);
                self.gl
                    .tex_parameter_i32(TEXTURE_RECTANGLE, TEXTURE_WRAP_S, CLAMP_TO_EDGE as i32);
                self.gl
                    .tex_parameter_i32(TEXTURE_RECTANGLE, TEXTURE_WRAP_T, CLAMP_TO_EDGE as i32);
                // SAFETY: the saved context is current, and the pixel buffer
                // dimensions and BGRA storage match the bound rectangle texture.
                let error = CGLTexImageIOSurface2D(
                    self.context,
                    TEXTURE_RECTANGLE,
                    glow::RGBA8,
                    size.0 as i32,
                    size.1 as i32,
                    glow::BGRA,
                    glow::UNSIGNED_INT_8_8_8_8_REV,
                    &surface,
                    0,
                );
                if error.0 != 0 {
                    return Err(format!(
                        "CGL could not bind the PiP IOSurface ({})",
                        error.0
                    ));
                }
                self.gl
                    .bind_framebuffer(glow::FRAMEBUFFER, Some(self.framebuffer));
                self.gl.framebuffer_texture_2d(
                    glow::FRAMEBUFFER,
                    glow::COLOR_ATTACHMENT0,
                    TEXTURE_RECTANGLE,
                    Some(texture),
                    0,
                );
                if self.gl.check_framebuffer_status(glow::FRAMEBUFFER) != glow::FRAMEBUFFER_COMPLETE
                {
                    return Err("The PiP IOSurface framebuffer is incomplete".into());
                }
            }
            let framebuffer = NonZeroU32::new(self.framebuffer.0.get())
                .ok_or_else(|| "The PiP framebuffer id is invalid".to_owned())?;
            // SAFETY: this notifier keeps the creating context current; the
            // attached IOSurface framebuffer is complete and matches `size`.
            unsafe {
                video.render_to_framebuffer(framebuffer, size.0, size.1, glow::RGBA8 as i32)?;
                self.gl.flush();
            }
            create_sample(&pixel_buffer, presentation_time)
        })();
        // SAFETY: the original CGL context is still current. The saved objects
        // belong to it and this call owns the temporary texture.
        unsafe {
            self.gl
                .bind_framebuffer(glow::DRAW_FRAMEBUFFER, previous_draw);
            self.gl
                .bind_framebuffer(glow::READ_FRAMEBUFFER, previous_read);
            self.gl.bind_texture(TEXTURE_RECTANGLE, previous_texture);
            self.gl.delete_texture(texture);
        }
        self.last_sample_at = Some(now);
        result.map(Some)
    }
}

pub(crate) struct CurrentContext {
    previous: CGLContextObj,
    changed: bool,
}

impl Drop for CurrentContext {
    fn drop(&mut self) {
        if self.changed {
            // SAFETY: the guard stays on the UI thread. A null prior context clears the binding.
            let status = unsafe { CGLSetCurrentContext(self.previous) };
            if status.0 != 0 {
                crate::diagnostics::warn!(
                    "Could not restore the current CGL context: {}",
                    status.0
                );
            }
        }
    }
}

impl Drop for PipFrameSource {
    fn drop(&mut self) {
        // A dead context releases its own objects. Never issue deletion through
        // an unrelated context if the window teardown did not run first.
        // SAFETY: querying the calling thread's current CGL context takes no arguments.
        if unsafe { CGLGetCurrentContext() } == self.context {
            // SAFETY: the creating context is current and this source owns the framebuffer.
            unsafe { self.gl.delete_framebuffer(self.framebuffer) };
        }
    }
}

fn bounded_dimensions((width, height): (u32, u32)) -> (u32, u32) {
    let scale = (MAX_DIMENSION as f64 / width.max(height).max(1) as f64).min(1.0);
    let even = |value: u32| (value.max(2) / 2) * 2;
    (
        even((f64::from(width.max(1)) * scale).round() as u32),
        even((f64::from(height.max(1)) * scale).round() as u32),
    )
}

#[expect(
    clippy::expect_used,
    reason = "The Core Video pool output is checked for null after creation"
)]
fn create_pool(size: (u32, u32)) -> Result<CFRetained<CVPixelBufferPool>, String> {
    let format = cf_number(kCVPixelFormatType_32BGRA as i32);
    let width = cf_number(size.0 as i32);
    let height = cf_number(size.1 as i32);
    // SAFETY: a retained Core Foundation dictionary can be used as its base CF type.
    let iosurface =
        unsafe { CFRetained::cast_unchecked::<CFType>(CFDictionary::<CFString, CFType>::empty()) };
    // SAFETY: the immutable CFString keys are provided by the linked Core Video framework.
    let keys = unsafe {
        [
            kCVPixelBufferPixelFormatTypeKey,
            kCVPixelBufferWidthKey,
            kCVPixelBufferHeightKey,
            kCVPixelBufferIOSurfacePropertiesKey,
            kCVPixelBufferOpenGLCompatibilityKey,
        ]
    };
    let values: [&CFType; 5] = [
        format.as_ref(),
        width.as_ref(),
        height.as_ref(),
        iosurface.as_ref(),
        CFBoolean::new(true),
    ];
    let attributes = CFDictionary::<CFString, CFType>::from_slices(&keys, &values);
    // SAFETY: erasing the generic key/value types keeps the same CFDictionary object.
    let attributes = unsafe { CFRetained::cast_unchecked::<CFDictionary>(attributes) };
    let mut pool = std::ptr::null_mut();
    // SAFETY: the retained dictionary describes the buffers and the out pointer is writable.
    let status = unsafe {
        CVPixelBufferPool::create(None, None, Some(&attributes), NonNull::from(&mut pool))
    };
    if status != kCVReturnSuccess || pool.is_null() {
        return Err(format!(
            "Core Video could not create the PiP buffer pool ({status})"
        ));
    }
    // SAFETY: Core Video transferred ownership of the checked non-null pool.
    Ok(unsafe { CFRetained::from_raw(NonNull::new(pool).expect("checked non-null")) })
}

fn allocation_limit_attributes() -> Result<CFRetained<CFDictionary>, String> {
    let limit = cf_number(POOL_LIMIT);
    // SAFETY: this immutable CFString key is provided by Core Video.
    let keys = unsafe { [kCVPixelBufferPoolAllocationThresholdKey] };
    let values: [&CFType; 1] = [limit.as_ref()];
    let attributes = CFDictionary::<CFString, CFType>::from_slices(&keys, &values);
    // SAFETY: the cast erases generic parameters of the same live dictionary.
    Ok(unsafe { CFRetained::cast_unchecked::<CFDictionary>(attributes) })
}

fn cf_number(value: i32) -> CFRetained<CFType> {
    // SAFETY: the retained CFNumber is a Core Foundation object.
    unsafe { CFRetained::cast_unchecked::<CFType>(CFNumber::new_i32(value)) }
}

#[expect(
    clippy::expect_used,
    reason = "The Core Media creation outputs are checked for null"
)]
fn create_sample(
    pixel_buffer: &CVPixelBuffer,
    presentation_time: CMTime,
) -> Result<CFRetained<CMSampleBuffer>, String> {
    let mut format_description: *const CMVideoFormatDescription = std::ptr::null();
    // SAFETY: the pixel buffer is borrowed for the call and the out pointer is writable.
    let status = unsafe {
        CMVideoFormatDescriptionCreateForImageBuffer(
            None,
            pixel_buffer,
            NonNull::from(&mut format_description),
        )
    };
    if status != 0 || format_description.is_null() {
        return Err(format!(
            "Core Media could not describe the PiP frame ({status})"
        ));
    }
    // SAFETY: the successful Create call returned one retained format description.
    let format_description = unsafe {
        CFRetained::from_raw(NonNull::new(format_description.cast_mut()).expect("checked non-null"))
    };
    // SAFETY: the timescale is positive and Core Media exports the immutable invalid time.
    let mut timing = unsafe {
        CMSampleTimingInfo {
            duration: CMTime::new(1, 30),
            presentationTimeStamp: presentation_time,
            decodeTimeStamp: kCMTimeInvalid,
        }
    };
    let mut sample = std::ptr::null_mut();
    // SAFETY: the format was created from this pixel buffer, and the timing
    // descriptor and sample out pointer remain live for the call.
    let status = unsafe {
        CMSampleBuffer::create_ready_with_image_buffer(
            None,
            pixel_buffer,
            &format_description,
            NonNull::from(&mut timing),
            NonNull::from(&mut sample),
        )
    };
    if status != 0 || sample.is_null() {
        return Err(format!(
            "Core Media could not create the PiP sample ({status})"
        ));
    }
    // SAFETY: the successful Create call returned one retained, non-null sample.
    Ok(unsafe { CFRetained::from_raw(NonNull::new(sample).expect("checked non-null")) })
}
