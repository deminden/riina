//! Framebuffer operations shared by the video renderer and the material pass.
use glow::{Context, HasContext, NativeFramebuffer, NativeTexture};
use std::num::NonZeroU32;

pub struct FramebufferBindings {
    draw: Option<NativeFramebuffer>,
    read: Option<NativeFramebuffer>,
}

impl FramebufferBindings {
    /// # Safety
    /// Call with the window's GL context current and restore before releasing it.
    pub unsafe fn capture(gl: &Context) -> Self {
        // SAFETY: the context is current and these queries read its bindings.
        unsafe {
            Self {
                draw: NonZeroU32::new(gl.get_parameter_i32(glow::DRAW_FRAMEBUFFER_BINDING) as u32)
                    .map(NativeFramebuffer),
                read: NonZeroU32::new(gl.get_parameter_i32(glow::READ_FRAMEBUFFER_BINDING) as u32)
                    .map(NativeFramebuffer),
            }
        }
    }

    /// # Safety
    /// The captured context must be current and the framebuffer names still live.
    pub unsafe fn restore(self, gl: &Context) {
        // SAFETY: the caller retains the queried framebuffers in the same context.
        unsafe {
            gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, self.draw);
            gl.bind_framebuffer(glow::READ_FRAMEBUFFER, self.read);
        }
    }
}

/// # Safety
/// A live 2D texture must already be bound in the current context. Width and
/// height must be within the context's supported texture dimensions.
pub unsafe fn allocate_rgba8_texture(gl: &Context, width: u32, height: u32) {
    // SAFETY: the caller owns the bound texture and supplies validated dimensions.
    unsafe {
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
    }
}

/// Attach the color texture and report whether the framebuffer is complete.
///
/// # Safety
/// A live framebuffer must already be bound for draw and read. The texture
/// must belong to that context and remain live for all renders using it.
pub unsafe fn attach_texture(gl: &Context, texture: NativeTexture) -> bool {
    // SAFETY: the current context owns both the attachment and bound framebuffer.
    unsafe {
        gl.framebuffer_texture_2d(
            glow::FRAMEBUFFER,
            glow::COLOR_ATTACHMENT0,
            glow::TEXTURE_2D,
            Some(texture),
            0,
        );
        gl.check_framebuffer_status(glow::FRAMEBUFFER) == glow::FRAMEBUFFER_COMPLETE
    }
}
