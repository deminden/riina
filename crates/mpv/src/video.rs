//! GPU-only SDR feasibility surface. All methods must run with Slint's GL context current.
use crate::{
    EngineError, Player,
    ffi::{Fbo, GlInit, Param, render_param},
    gl_support::{FramebufferBindings, allocate_rgba8_texture, attach_texture},
};
use glow::HasContext;
use std::{
    ffi::{CStr, c_char, c_void},
    num::NonZeroU32,
    rc::Rc,
};

struct Resolver<'a> {
    callback: &'a dyn Fn(&CStr) -> *const c_void,
}
type Repaint = Box<dyn Fn() + Send + Sync>;

unsafe extern "C" fn resolve(context: *mut c_void, name: *const c_char) -> *mut c_void {
    // SAFETY: the resolver lives throughout render_context_create. render.h
    // borrows init parameters for that call; libmpv_gl.c resolves all functions in init.
    let resolver = unsafe { &*context.cast::<Resolver<'_>>() };
    // SAFETY: libmpv supplies a NUL-terminated GL symbol for this synchronous callback.
    let name = unsafe { CStr::from_ptr(name) };
    (resolver.callback)(name).cast_mut()
}

unsafe extern "C" fn repaint_requested(context: *mut c_void) {
    // SAFETY: unregistered before the boxed callback is dropped. No mpv calls,
    // GL calls, waits, or locks are permitted in this foreign callback.
    let callback = unsafe { &*context.cast::<Repaint>() };
    callback();
}

pub struct VideoSurface {
    player: Player,
    gl: Rc<glow::Context>,
    context: *mut c_void,
    repaint: Box<Repaint>,
    texture: glow::NativeTexture,
    fbo: glow::NativeFramebuffer,
    size: (u32, u32),
    icc_profile: Option<Vec<u8>>,
}

impl VideoSurface {
    /// # Safety
    /// Call inside Slint's RenderingSetup with its OpenGL context current and
    /// resolve symbols from that context. Drop inside RenderingTeardown with
    /// that same context current.
    pub unsafe fn new(
        player: Player,
        loader: &dyn Fn(&CStr) -> *const c_void,
        repaint_callback: Repaint,
    ) -> Result<Self, EngineError> {
        // SAFETY: the caller supplies the current Slint GL context and its symbol loader.
        let gl = Rc::new(unsafe { glow::Context::from_loader_function_cstr(|name| loader(name)) });
        let mut resolver = Resolver { callback: loader };
        // render.h: initialization parameters are borrowed for render_context_create only.
        let mut init = GlInit {
            get_proc: resolve,
            context: (&mut resolver as *mut Resolver<'_>).cast(),
        };
        let mut params = [
            Param {
                kind: render_param::API_TYPE,
                data: c"opengl".as_ptr().cast_mut().cast(),
            },
            Param {
                kind: render_param::OPENGL_INIT_PARAMS,
                data: (&mut init as *mut GlInit).cast(),
            },
            Param::end(),
        ];
        let mut context = std::ptr::null_mut();
        // SAFETY: the player retains the live handle. The init parameters and
        // resolver stay alive for this call on the current GL thread.
        unsafe {
            player.api.check(
                "mpv_render_context_create",
                (player.api.render_create)(
                    &mut context,
                    player.handle as *mut c_void,
                    params.as_mut_ptr(),
                ),
            )?;
        }
        // SAFETY: the host GL context remains current on this thread.
        let texture = match unsafe { gl.create_texture() } {
            Ok(texture) => texture,
            Err(error) => {
                // SAFETY: creation succeeded and this thread alone owns the render context.
                unsafe {
                    (player.api.render_free)(context);
                }
                return Err(EngineError::Render {
                    call: "glCreateTexture",
                    message: error,
                });
            }
        };
        // SAFETY: the host GL context remains current on this thread.
        let fbo = match unsafe { gl.create_framebuffer() } {
            Ok(fbo) => fbo,
            Err(error) => {
                // SAFETY: both objects were created here and are owned by this setup call.
                unsafe {
                    gl.delete_texture(texture);
                    (player.api.render_free)(context);
                }
                return Err(EngineError::Render {
                    call: "glCreateFramebuffer",
                    message: error,
                });
            }
        };
        let mut repaint = Box::new(repaint_callback);
        // SAFETY: the boxed callback has a stable address and is cleared before either
        // its allocation or the live context is released.
        unsafe {
            (player.api.render_callback)(
                context,
                Some(repaint_requested),
                (&mut *repaint as *mut Repaint).cast(),
            );
        }
        Ok(Self {
            player,
            gl,
            context,
            repaint,
            texture,
            fbo,
            size: (0, 0),
            icc_profile: None,
        })
    }

    /// Install the active display's ICC profile for libmpv automatic color management.
    ///
    /// # Safety
    /// Call only from the same current GL context used to create this surface.
    pub unsafe fn set_icc_profile(&mut self, profile: &[u8]) -> Result<(), EngineError> {
        if self.icc_profile.as_deref() == Some(profile) {
            return Ok(());
        }
        let set_parameter =
            self.player
                .api
                .render_set_parameter
                .ok_or(EngineError::InvalidArgument {
                    reason: "The native engine does not support render ICC profiles",
                })?;
        if profile.is_empty() {
            return Err(EngineError::InvalidArgument {
                reason: "The active display returned an empty ICC profile",
            });
        }
        let mut bytes = crate::ffi::ByteArray {
            data: profile.as_ptr().cast_mut().cast(),
            size: profile.len(),
        };
        let parameter = Param {
            kind: render_param::ICC_PROFILE,
            data: (&mut bytes as *mut crate::ffi::ByteArray).cast(),
        };
        // SAFETY: libmpv copies the profile during this call on the current render
        // thread. The descriptor and the borrowed profile remain live until it returns.
        let result = unsafe { set_parameter(self.context, parameter) };
        self.player
            .api
            .check("mpv_render_context_set_parameter", result)?;
        self.icc_profile = Some(profile.to_vec());
        Ok(())
    }

    pub fn texture_id(&self) -> NonZeroU32 {
        self.texture.0
    }

    /// # Safety
    /// Call only from the same current GL context used at construction.
    pub unsafe fn render(&mut self, width: u32, height: u32) -> Result<(), EngineError> {
        let width = width.clamp(1, 8192);
        let height = height.clamp(1, 8192);
        // SAFETY: this surface owns the framebuffer. Its attachment is prepared by
        // render_into while the caller's current GL context is active.
        unsafe { self.render_into(self.fbo, width, height, glow::RGBA8 as i32) }
    }

    /// Render the current decoded frame into a caller-owned framebuffer.
    ///
    /// # Safety
    /// The framebuffer must belong to the current context used to create this
    /// surface, remain alive for the call, and have a complete color attachment
    /// sized to `width` x `height` with a format matching `format`. Call only
    /// from that same current GL context.
    pub unsafe fn render_to_framebuffer(
        &mut self,
        framebuffer: NonZeroU32,
        width: u32,
        height: u32,
        format: i32,
    ) -> Result<(), EngineError> {
        let width = width.clamp(1, 8192);
        let height = height.clamp(1, 8192);
        let framebuffer = glow::NativeFramebuffer(framebuffer);
        // SAFETY: the caller supplies a complete framebuffer in the current context
        // and retains its attachments for the synchronous render call.
        unsafe { self.render_into(framebuffer, width, height, format) }
    }

    /// Report that Slint submitted the composed frame for presentation.
    ///
    /// # Safety
    /// Call on the render thread for this live context after composing the frame.
    pub unsafe fn report_swap(&self) {
        // SAFETY: the surface owns the live context and the caller is on its render thread.
        unsafe { (self.player.api.render_report_swap)(self.context) };
    }

    unsafe fn render_into(
        &mut self,
        framebuffer: glow::NativeFramebuffer,
        width: u32,
        height: u32,
        format: i32,
    ) -> Result<(), EngineError> {
        let gl = &self.gl;
        // mpv documents the state it does not restore in render_gl.h. Preserve it
        // together with bindings affected by our texture allocation.
        // SAFETY: the caller keeps the GL context current and the framebuffer live
        // throughout the render, including restoration of the caller's GL state.
        unsafe {
            let bindings = FramebufferBindings::capture(gl);
            let texture = NonZeroU32::new(gl.get_parameter_i32(glow::TEXTURE_BINDING_2D) as u32)
                .map(glow::NativeTexture);
            let mut viewport = [0; 4];
            gl.get_parameter_i32_slice(glow::VIEWPORT, &mut viewport);
            let mut scissor = [0; 4];
            gl.get_parameter_i32_slice(glow::SCISSOR_BOX, &mut scissor);
            let mut clear = [0.0; 4];
            gl.get_parameter_f32_slice(glow::COLOR_CLEAR_VALUE, &mut clear);
            let blend = [
                gl.get_parameter_i32(glow::BLEND_SRC_RGB),
                gl.get_parameter_i32(glow::BLEND_DST_RGB),
                gl.get_parameter_i32(glow::BLEND_SRC_ALPHA),
                gl.get_parameter_i32(glow::BLEND_DST_ALPHA),
            ];
            let result = (|| {
                gl.bind_framebuffer(glow::FRAMEBUFFER, Some(framebuffer));
                if framebuffer == self.fbo && self.size != (width, height) {
                    gl.bind_texture(glow::TEXTURE_2D, Some(self.texture));
                    allocate_rgba8_texture(gl, width, height);
                    if !attach_texture(gl, self.texture) {
                        return Err(EngineError::Render {
                            call: "glCheckFramebufferStatus",
                            message: "The video framebuffer is incomplete".into(),
                        });
                    }
                    self.size = (width, height);
                }
                let mut fbo = Fbo {
                    fbo: framebuffer.0.get() as i32,
                    width: width as i32,
                    height: height as i32,
                    format,
                };
                // Slint samples borrowed textures with a top-left origin. mpv's
                // default FBO orientation already matches it; flipping twice inverts video.
                let mut flip: i32 = 0;
                let mut params = [
                    Param {
                        kind: render_param::OPENGL_FBO,
                        data: (&mut fbo as *mut Fbo).cast(),
                    },
                    Param {
                        kind: render_param::FLIP_Y,
                        data: (&mut flip as *mut i32).cast(),
                    },
                    Param::end(),
                ];
                self.player.api.check(
                    "mpv_render_context_render",
                    (self.player.api.render)(self.context, params.as_mut_ptr()),
                )
            })();
            bindings.restore(gl);
            gl.bind_texture(glow::TEXTURE_2D, texture);
            gl.viewport(viewport[0], viewport[1], viewport[2], viewport[3]);
            gl.scissor(scissor[0], scissor[1], scissor[2], scissor[3]);
            gl.clear_color(clear[0], clear[1], clear[2], clear[3]);
            gl.blend_func_separate(
                blend[0] as u32,
                blend[1] as u32,
                blend[2] as u32,
                blend[3] as u32,
            );
            result
        }
    }
}

impl Drop for VideoSurface {
    fn drop(&mut self) {
        // SAFETY: integration drops this in RenderingTeardown, before Slint releases
        // the context. Player ownership prevents the event worker from destroying mpv early.
        unsafe {
            (self.player.api.render_callback)(self.context, None, std::ptr::null_mut());
            (self.player.api.render_free)(self.context);
            self.gl.delete_framebuffer(self.fbo);
            self.gl.delete_texture(self.texture);
        }
        // Keep callback storage alive through render_free, including any in-flight call.
        let _ = &self.repaint;
    }
}
