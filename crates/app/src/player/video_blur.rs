//! Downsampled, video-backed blur for translucent player surfaces.
//!
//! Windows renderer-backed surfaces add low-amplitude grain over the blur to
//! approximate Acrylic's noise layer without sampling the desktop behind video.
//!
//! All GL objects belong to the player window's render context. Methods and
//! destruction must run only while Slint has that same context current.

use crate::platform::video_materials::{
    WINDOWS_NATIVE_MATERIAL_BLUR_SIGMA, WINDOWS_NATIVE_MATERIAL_SAMPLE_HEIGHT,
    WINDOWS_NATIVE_MATERIAL_SAMPLE_WIDTH,
};
use glow::HasContext;
use riina_mpv::gl_support::{FramebufferBindings, allocate_rgba8_texture, attach_texture};
use std::{ffi::CStr, num::NonZeroU32, rc::Rc};

fn windows_shared_material_sample_step(width: u32, height: u32) -> (f32, f32) {
    let aspect_ratio = width as f32 / height as f32;
    let sample_width = (WINDOWS_NATIVE_MATERIAL_SAMPLE_HEIGHT * aspect_ratio)
        .min(WINDOWS_NATIVE_MATERIAL_SAMPLE_WIDTH);
    let sample_height = (WINDOWS_NATIVE_MATERIAL_SAMPLE_WIDTH / aspect_ratio)
        .min(WINDOWS_NATIVE_MATERIAL_SAMPLE_HEIGHT);
    (
        WINDOWS_NATIVE_MATERIAL_BLUR_SIGMA / sample_width,
        WINDOWS_NATIVE_MATERIAL_BLUR_SIGMA / sample_height,
    )
}

const VERTEX_SHADER: &str = r#"in vec2 position;
in vec2 texture_coordinate;
out vec2 uv;
void main() {
    gl_Position = vec4(position, 0.0, 1.0);
    uv = texture_coordinate;
}
"#;

const FRAGMENT_SHADER: &str = r#"uniform sampler2D source_frame;
uniform vec2 sample_step;
uniform float grain_strength;
in vec2 uv;
out vec4 output_color;
void main() {
    vec2 x = vec2(sample_step.x, 0.0);
    vec2 y = vec2(0.0, sample_step.y);
    vec4 sum = vec4(0.0);
    sum += texture(source_frame, uv + -2.0*x - 2.0*y) * 1.0;
    sum += texture(source_frame, uv + -1.0*x - 2.0*y) * 4.0;
    sum += texture(source_frame, uv +  0.0*x - 2.0*y) * 7.0;
    sum += texture(source_frame, uv +  1.0*x - 2.0*y) * 4.0;
    sum += texture(source_frame, uv +  2.0*x - 2.0*y) * 1.0;
    sum += texture(source_frame, uv + -2.0*x - 1.0*y) * 4.0;
    sum += texture(source_frame, uv + -1.0*x - 1.0*y) * 16.0;
    sum += texture(source_frame, uv +  0.0*x - 1.0*y) * 26.0;
    sum += texture(source_frame, uv +  1.0*x - 1.0*y) * 16.0;
    sum += texture(source_frame, uv +  2.0*x - 1.0*y) * 4.0;
    sum += texture(source_frame, uv + -2.0*x) * 7.0;
    sum += texture(source_frame, uv + -1.0*x) * 26.0;
    sum += texture(source_frame, uv) * 41.0;
    sum += texture(source_frame, uv + 1.0*x) * 26.0;
    sum += texture(source_frame, uv + 2.0*x) * 7.0;
    sum += texture(source_frame, uv + -2.0*x + 1.0*y) * 4.0;
    sum += texture(source_frame, uv + -1.0*x + 1.0*y) * 16.0;
    sum += texture(source_frame, uv +  0.0*x + 1.0*y) * 26.0;
    sum += texture(source_frame, uv +  1.0*x + 1.0*y) * 16.0;
    sum += texture(source_frame, uv +  2.0*x + 1.0*y) * 4.0;
    sum += texture(source_frame, uv + -2.0*x + 2.0*y) * 1.0;
    sum += texture(source_frame, uv + -1.0*x + 2.0*y) * 4.0;
    sum += texture(source_frame, uv +  0.0*x + 2.0*y) * 7.0;
    sum += texture(source_frame, uv +  1.0*x + 2.0*y) * 4.0;
    sum += texture(source_frame, uv +  2.0*x + 2.0*y) * 1.0;
    output_color = sum / 273.0;
    float grain = fract(sin(dot(floor(gl_FragCoord.xy), vec2(12.9898, 78.233))) * 43758.5453);
    output_color.rgb = clamp(output_color.rgb + (grain - 0.5) * grain_strength, 0.0, 1.0);
}
"#;

const QUAD: [f32; 16] = [
    -1.0, -1.0, 0.0, 0.0, 1.0, -1.0, 1.0, 0.0, -1.0, 1.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0,
];

pub(crate) struct VideoBlurSurface {
    gl: Rc<glow::Context>,
    texture: glow::NativeTexture,
    framebuffer: glow::NativeFramebuffer,
    program: glow::NativeProgram,
    vertex_array: glow::NativeVertexArray,
    vertex_buffer: glow::NativeBuffer,
    source_uniform: Option<glow::NativeUniformLocation>,
    step_uniform: Option<glow::NativeUniformLocation>,
    grain_uniform: Option<glow::NativeUniformLocation>,
    grain_strength: f32,
    size: (u32, u32),
}

impl VideoBlurSurface {
    /// # Safety
    /// Call from Slint's `RenderingSetup` with the window's GL context current.
    /// Resolve symbols from that context and drop this surface during its
    /// `RenderingTeardown`, before the renderer releases the context.
    pub(crate) unsafe fn new(
        loader: &dyn Fn(&CStr) -> *const std::ffi::c_void,
    ) -> Result<Self, String> {
        // SAFETY: the caller supplies the loader for the current Slint GL context.
        let gl = Rc::new(unsafe { glow::Context::from_loader_function_cstr(|name| loader(name)) });
        // SAFETY: the setup callback keeps the context current while the shaders link.
        let program = unsafe { create_program(&gl)? };
        // SAFETY: the host GL context is still current for texture allocation.
        let texture = match unsafe { gl.create_texture() } {
            Ok(value) => value,
            Err(error) => {
                // SAFETY: setup owns the linked program and has not exposed it.
                unsafe { gl.delete_program(program) };
                return Err(error);
            }
        };
        // SAFETY: the host GL context is still current for framebuffer allocation.
        let framebuffer = match unsafe { gl.create_framebuffer() } {
            Ok(value) => value,
            Err(error) => {
                // SAFETY: setup owns both successfully allocated objects in this context.
                unsafe {
                    gl.delete_texture(texture);
                    gl.delete_program(program);
                }
                return Err(error);
            }
        };
        // SAFETY: the current host context owns the vertex array namespace.
        let vertex_array = match unsafe { gl.create_vertex_array() } {
            Ok(value) => value,
            Err(error) => {
                // SAFETY: setup owns the framebuffer, texture and program here.
                unsafe {
                    gl.delete_framebuffer(framebuffer);
                    gl.delete_texture(texture);
                    gl.delete_program(program);
                }
                return Err(error);
            }
        };
        // SAFETY: the current host context owns the buffer namespace.
        let vertex_buffer = match unsafe { gl.create_buffer() } {
            Ok(value) => value,
            Err(error) => {
                // SAFETY: these handles are exclusively owned by the failed setup.
                unsafe {
                    gl.delete_vertex_array(vertex_array);
                    gl.delete_framebuffer(framebuffer);
                    gl.delete_texture(texture);
                    gl.delete_program(program);
                }
                return Err(error);
            }
        };
        // SAFETY: the program has linked successfully in this current GL context.
        let (source_uniform, step_uniform, grain_uniform) = unsafe {
            (
                gl.get_uniform_location(program, "source_frame"),
                gl.get_uniform_location(program, "sample_step"),
                gl.get_uniform_location(program, "grain_strength"),
            )
        };
        let grain_strength = if cfg!(target_os = "windows") {
            0.05
        } else {
            0.0
        };
        // SAFETY: the current context owns both vertex objects, and QUAD holds
        // exactly the initialized bytes supplied for the synchronous upload.
        unsafe {
            gl.bind_vertex_array(Some(vertex_array));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(vertex_buffer));
            let bytes = std::slice::from_raw_parts(
                QUAD.as_ptr().cast::<u8>(),
                std::mem::size_of_val(&QUAD),
            );
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytes, glow::STATIC_DRAW);
            for (index, offset) in [(0, 0), (1, 2)] {
                gl.enable_vertex_attrib_array(index);
                gl.vertex_attrib_pointer_f32(index, 2, glow::FLOAT, false, 16, offset * 4);
            }
            gl.bind_vertex_array(None);
            gl.bind_buffer(glow::ARRAY_BUFFER, None);
        }
        Ok(Self {
            gl,
            texture,
            framebuffer,
            program,
            vertex_array,
            vertex_buffer,
            source_uniform,
            step_uniform,
            grain_uniform,
            grain_strength,
            size: (0, 0),
        })
    }

    pub(crate) fn texture_id(&self) -> NonZeroU32 {
        self.texture.0
    }

    /// # Safety
    /// `source` must be a live 2D texture from this window's current GL context.
    pub(crate) unsafe fn render(
        &mut self,
        source: glow::NativeTexture,
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        let width = width.clamp(1, 8192).div_ceil(4).max(1);
        let height = height.clamp(1, 8192).div_ceil(4).max(1);
        let gl = &self.gl;
        // Slint and mpv share this context. Preserve the bindings and rasterization state that
        // our full-screen pass changes so the renderer can continue its frame.
        // SAFETY: the caller keeps the source texture alive in the current context
        // throughout this draw and the restoration of the caller's GL state.
        unsafe {
            let bindings = FramebufferBindings::capture(gl);
            let old_program = NonZeroU32::new(gl.get_parameter_i32(glow::CURRENT_PROGRAM) as u32)
                .map(glow::NativeProgram);
            let old_vao = NonZeroU32::new(gl.get_parameter_i32(glow::VERTEX_ARRAY_BINDING) as u32)
                .map(glow::NativeVertexArray);
            let old_buffer =
                NonZeroU32::new(gl.get_parameter_i32(glow::ARRAY_BUFFER_BINDING) as u32)
                    .map(glow::NativeBuffer);
            let old_texture_unit = gl.get_parameter_i32(glow::ACTIVE_TEXTURE) as u32;
            gl.active_texture(glow::TEXTURE0);
            let old_texture =
                NonZeroU32::new(gl.get_parameter_i32(glow::TEXTURE_BINDING_2D) as u32)
                    .map(glow::NativeTexture);
            let mut old_viewport = [0; 4];
            gl.get_parameter_i32_slice(glow::VIEWPORT, &mut old_viewport);
            let blend_enabled = gl.is_enabled(glow::BLEND);
            let scissor_enabled = gl.is_enabled(glow::SCISSOR_TEST);
            let depth_enabled = gl.is_enabled(glow::DEPTH_TEST);
            let cull_enabled = gl.is_enabled(glow::CULL_FACE);
            let stencil_enabled = gl.is_enabled(glow::STENCIL_TEST);

            let result = (|| {
                gl.bind_texture(glow::TEXTURE_2D, Some(self.texture));
                if self.size != (width, height) {
                    allocate_rgba8_texture(gl, width, height);
                    self.size = (width, height);
                }
                gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.framebuffer));
                if !attach_texture(gl, self.texture) {
                    return Err("The video-blur framebuffer is incomplete".to_owned());
                }
                gl.viewport(0, 0, width as i32, height as i32);
                gl.disable(glow::BLEND);
                gl.disable(glow::SCISSOR_TEST);
                gl.disable(glow::DEPTH_TEST);
                gl.disable(glow::CULL_FACE);
                gl.disable(glow::STENCIL_TEST);
                gl.use_program(Some(self.program));
                gl.bind_vertex_array(Some(self.vertex_array));
                gl.active_texture(glow::TEXTURE0);
                gl.bind_texture(glow::TEXTURE_2D, Some(source));
                if let Some(uniform) = &self.source_uniform {
                    gl.uniform_1_i32(Some(uniform), 0);
                }
                if let Some(uniform) = &self.step_uniform {
                    let sample_step = if cfg!(target_os = "windows") {
                        windows_shared_material_sample_step(width, height)
                    } else {
                        (3.0 / width as f32, 3.0 / height as f32)
                    };
                    gl.uniform_2_f32(Some(uniform), sample_step.0, sample_step.1);
                }
                if let Some(uniform) = &self.grain_uniform {
                    gl.uniform_1_f32(Some(uniform), self.grain_strength);
                }
                gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
                Ok(())
            })();

            gl.bind_vertex_array(old_vao);
            gl.bind_buffer(glow::ARRAY_BUFFER, old_buffer);
            gl.use_program(old_program);
            bindings.restore(gl);
            gl.bind_texture(glow::TEXTURE_2D, old_texture);
            gl.active_texture(old_texture_unit);
            gl.viewport(
                old_viewport[0],
                old_viewport[1],
                old_viewport[2],
                old_viewport[3],
            );
            if blend_enabled {
                gl.enable(glow::BLEND);
            } else {
                gl.disable(glow::BLEND);
            }
            if scissor_enabled {
                gl.enable(glow::SCISSOR_TEST);
            } else {
                gl.disable(glow::SCISSOR_TEST);
            }
            if depth_enabled {
                gl.enable(glow::DEPTH_TEST);
            } else {
                gl.disable(glow::DEPTH_TEST);
            }
            if cull_enabled {
                gl.enable(glow::CULL_FACE);
            } else {
                gl.disable(glow::CULL_FACE);
            }
            if stencil_enabled {
                gl.enable(glow::STENCIL_TEST);
            } else {
                gl.disable(glow::STENCIL_TEST);
            }
            result
        }
    }
}

impl Drop for VideoBlurSurface {
    fn drop(&mut self) {
        // SAFETY: the constructor requires drop with this GL context current.
        // The exclusively owned program and GL objects are released here once.
        unsafe {
            self.gl.delete_buffer(self.vertex_buffer);
            self.gl.delete_vertex_array(self.vertex_array);
            self.gl.delete_framebuffer(self.framebuffer);
            self.gl.delete_texture(self.texture);
            self.gl.delete_program(self.program);
        }
    }
}

unsafe fn create_program(gl: &glow::Context) -> Result<glow::NativeProgram, String> {
    // SAFETY: the caller supplies the current context for this vertex shader.
    let vertex = unsafe { compile_shader(gl, glow::VERTEX_SHADER, VERTEX_SHADER)? };
    // SAFETY: the same context is current for fragment shader compilation.
    let fragment = match unsafe { compile_shader(gl, glow::FRAGMENT_SHADER, FRAGMENT_SHADER) } {
        Ok(shader) => shader,
        Err(error) => {
            // SAFETY: this call owns the unattached vertex shader.
            unsafe { gl.delete_shader(vertex) };
            return Err(error);
        }
    };
    // SAFETY: this call is still on the thread with the context current.
    let program = unsafe { gl.create_program() }.map_err(|error| error.to_string())?;
    // SAFETY: all three handles were created here and remain exclusively owned
    // while linking in the current context.
    unsafe {
        gl.attach_shader(program, vertex);
        gl.attach_shader(program, fragment);
        // GLSL 1.50 does not guarantee linker-assigned locations for these
        // inputs; the VAO binds position at 0 and UV at 1.
        gl.bind_attrib_location(program, 0, "position");
        gl.bind_attrib_location(program, 1, "texture_coordinate");
        gl.link_program(program);
        gl.detach_shader(program, vertex);
        gl.detach_shader(program, fragment);
        gl.delete_shader(vertex);
        gl.delete_shader(fragment);
    }
    // SAFETY: the newly linked program is still live in the current context.
    if !unsafe { gl.get_program_link_status(program) } {
        // SAFETY: the failed link leaves the program valid for its diagnostic query.
        let log = unsafe { gl.get_program_info_log(program) };
        // SAFETY: the failed program is owned here and has not been returned.
        unsafe { gl.delete_program(program) };
        return Err(format!("Could not link video-blur shader: {log}"));
    }
    Ok(program)
}

unsafe fn compile_shader(
    gl: &glow::Context,
    kind: u32,
    source: &str,
) -> Result<glow::NativeShader, String> {
    // SAFETY: the caller supplies the current context and an OpenGL shader stage.
    let shader = unsafe { gl.create_shader(kind) }.map_err(|error| error.to_string())?;
    // FemtoVG can create an OpenGL ES context on Linux. Use the same
    // shader body with the version and precision declarations it requires.
    let preamble = if gl.version().is_embedded {
        "#version 300 es\nprecision highp float;\nprecision highp int;\n"
    } else {
        "#version 150\n"
    };
    let source = format!("{preamble}{source}");
    // SAFETY: the shader is still exclusively owned and GL copies the source during the call.
    unsafe {
        gl.shader_source(shader, &source);
        gl.compile_shader(shader);
    }
    // SAFETY: the shader remains live in its current context after compilation.
    if !unsafe { gl.get_shader_compile_status(shader) } {
        // SAFETY: the failed compilation leaves the shader valid for its diagnostic query.
        let log = unsafe { gl.get_shader_info_log(shader) };
        // SAFETY: this call owns the failed shader and has not attached it to a program.
        unsafe { gl.delete_shader(shader) };
        return Err(format!("Could not compile video-blur shader: {log}"));
    }
    Ok(shader)
}
