//! GPU-only SDR feasibility surface. All methods must run with Slint's GL context current.
use crate::{
    Player,
    ffi::{Fbo, GlInit, Param},
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
    (resolver.callback)(unsafe { CStr::from_ptr(name) }).cast_mut()
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
}

impl VideoSurface {
    /// # Safety
    /// Call inside Slint's RenderingSetup with its OpenGL context current. Drop
    /// inside RenderingTeardown with that same context current.
    pub unsafe fn new(
        player: Player,
        loader: &dyn Fn(&CStr) -> *const c_void,
        repaint_callback: Repaint,
    ) -> Result<Self, String> {
        let gl = Rc::new(unsafe { glow::Context::from_loader_function_cstr(|name| loader(name)) });
        let mut resolver = Resolver { callback: loader };
        // render.h: initialization parameters are borrowed for render_context_create only.
        let mut init = GlInit {
            get_proc: resolve,
            context: (&mut resolver as *mut Resolver<'_>).cast(),
        };
        let mut params = [
            Param {
                kind: 1,
                data: c"opengl".as_ptr().cast_mut().cast(),
            },
            Param {
                kind: 2,
                data: (&mut init as *mut GlInit).cast(),
            },
            Param::end(),
        ];
        let mut context = std::ptr::null_mut();
        unsafe {
            player.api.check((player.api.render_create)(
                &mut context,
                player.handle as *mut c_void,
                params.as_mut_ptr(),
            ))?;
        }
        let texture = match unsafe { gl.create_texture() } {
            Ok(texture) => texture,
            Err(error) => {
                unsafe {
                    (player.api.render_free)(context);
                }
                return Err(error);
            }
        };
        let fbo = match unsafe { gl.create_framebuffer() } {
            Ok(fbo) => fbo,
            Err(error) => {
                unsafe {
                    gl.delete_texture(texture);
                    (player.api.render_free)(context);
                }
                return Err(error);
            }
        };
        let mut repaint = Box::new(repaint_callback);
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
        })
    }

    pub fn texture_id(&self) -> NonZeroU32 {
        self.texture.0
    }

    /// # Safety
    /// Call only from the same current GL context used at construction.
    pub unsafe fn render(&mut self, width: u32, height: u32) -> Result<(), String> {
        let width = width.clamp(1, 8192);
        let height = height.clamp(1, 8192);
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
    ) -> Result<(), String> {
        let width = width.clamp(1, 8192);
        let height = height.clamp(1, 8192);
        let framebuffer = glow::NativeFramebuffer(framebuffer);
        unsafe { self.render_into(framebuffer, width, height, format) }
    }

    /// Report that Slint submitted the composed frame for presentation.
    ///
    /// # Safety
    /// Call on the render thread for this live context after composing the frame.
    pub unsafe fn report_swap(&self) {
        unsafe { (self.player.api.render_report_swap)(self.context) };
    }

    unsafe fn render_into(
        &mut self,
        framebuffer: glow::NativeFramebuffer,
        width: u32,
        height: u32,
        format: i32,
    ) -> Result<(), String> {
        let gl = &self.gl;
        // mpv documents the state it does not restore in render_gl.h. Preserve it
        // together with bindings affected by our texture allocation.
        unsafe {
            let draw = NonZeroU32::new(gl.get_parameter_i32(glow::DRAW_FRAMEBUFFER_BINDING) as u32)
                .map(glow::NativeFramebuffer);
            let read = NonZeroU32::new(gl.get_parameter_i32(glow::READ_FRAMEBUFFER_BINDING) as u32)
                .map(glow::NativeFramebuffer);
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
                    gl.tex_image_2d(
                        glow::TEXTURE_2D,
                        0,
                        glow::RGBA8 as i32,
                        width as i32,
                        height as i32,
                        0,
                        glow::RGBA,
                        glow::UNSIGNED_BYTE,
                        glow::PixelUnpackData::Slice(None),
                    );
                    gl.tex_parameter_i32(
                        glow::TEXTURE_2D,
                        glow::TEXTURE_MIN_FILTER,
                        glow::LINEAR as i32,
                    );
                    gl.tex_parameter_i32(
                        glow::TEXTURE_2D,
                        glow::TEXTURE_MAG_FILTER,
                        glow::LINEAR as i32,
                    );
                    gl.tex_parameter_i32(
                        glow::TEXTURE_2D,
                        glow::TEXTURE_WRAP_S,
                        glow::CLAMP_TO_EDGE as i32,
                    );
                    gl.tex_parameter_i32(
                        glow::TEXTURE_2D,
                        glow::TEXTURE_WRAP_T,
                        glow::CLAMP_TO_EDGE as i32,
                    );
                    gl.framebuffer_texture_2d(
                        glow::FRAMEBUFFER,
                        glow::COLOR_ATTACHMENT0,
                        glow::TEXTURE_2D,
                        Some(self.texture),
                        0,
                    );
                    if gl.check_framebuffer_status(glow::FRAMEBUFFER) != glow::FRAMEBUFFER_COMPLETE
                    {
                        return Err("The video framebuffer is incomplete".into());
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
                        kind: 3,
                        data: (&mut fbo as *mut Fbo).cast(),
                    },
                    Param {
                        kind: 4,
                        data: (&mut flip as *mut i32).cast(),
                    },
                    Param::end(),
                ];
                self.player
                    .api
                    .check((self.player.api.render)(self.context, params.as_mut_ptr()))
            })();
            gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, draw);
            gl.bind_framebuffer(glow::READ_FRAMEBUFFER, read);
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
