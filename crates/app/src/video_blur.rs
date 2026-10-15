//! Downsampled, video-backed blur for translucent player surfaces.
//!
//! Windows renderer-backed surfaces add low-amplitude grain over the blur to
//! approximate Acrylic's noise layer without sampling the desktop behind video.
//!
//! All GL objects belong to the player window's render context. Methods and
//! destruction must run only while Slint has that same context current.

use glow::HasContext;
use std::{ffi::CStr, num::NonZeroU32, rc::Rc};

// At the measured 915-point player width, sigma 10 on this 320px sample scales
// to about 29 points on the displayed material.
const WINDOWS_NATIVE_MATERIAL_BLUR_SIGMA: f32 = 10.0;
const WINDOWS_NATIVE_MATERIAL_GRAIN_SEED: u64 = 0x5749_4e44_4f57_5301;
const WINDOWS_NATIVE_MATERIAL_SAMPLE_WIDTH: f32 = 320.0;
const WINDOWS_NATIVE_MATERIAL_SAMPLE_HEIGHT: f32 = 180.0;
#[cfg(any(target_os = "windows", test))]
const WINDOWS_MATERIAL_BLUR_AXIS_KERNEL: [u32; 5] = [1, 4, 7, 4, 1];
#[cfg(any(target_os = "windows", test))]
const WINDOWS_MATERIAL_BLUR_WEIGHT: u32 = 273;
#[cfg(test)]
const WINDOWS_MATERIAL_BLUR_SAMPLE_STEP: i32 = 8;

#[cfg(test)]
fn blur_native_windows_sample(sample: &image::RgbaImage) -> image::RgbaImage {
    blur_native_windows_sample_with_steps(
        sample,
        WINDOWS_MATERIAL_BLUR_SAMPLE_STEP,
        WINDOWS_MATERIAL_BLUR_SAMPLE_STEP,
    )
}

#[cfg(any(target_os = "windows", test))]
fn blur_native_windows_sample_with_steps(
    sample: &image::RgbaImage,
    sample_step_x: i32,
    sample_step_y: i32,
) -> image::RgbaImage {
    let (width, height) = sample.dimensions();
    let source = sample.as_raw();
    // The 5x5 material kernel is the outer product of this axis kernel with
    // five small center-line corrections. Preserve its exact weights while
    // reducing each pixel from 25 source taps to 10 pass taps plus corrections.
    let horizontal_samples = (0..width)
        .map(|x| {
            std::array::from_fn(|kernel_x| {
                (x as i32 + (kernel_x as i32 - 2) * sample_step_x).clamp(0, width as i32 - 1)
                    as usize
            })
        })
        .collect::<Vec<[usize; 5]>>();
    let vertical_samples = (0..height)
        .map(|y| {
            std::array::from_fn(|kernel_y| {
                (y as i32 + (kernel_y as i32 - 2) * sample_step_y).clamp(0, height as i32 - 1)
                    as usize
            })
        })
        .collect::<Vec<[usize; 5]>>();
    let horizontal_neighbors = (0..width)
        .map(|x| {
            [
                (x as i32 - sample_step_x).clamp(0, width as i32 - 1) as usize,
                (x as i32 + sample_step_x).clamp(0, width as i32 - 1) as usize,
            ]
        })
        .collect::<Vec<[usize; 2]>>();
    let mut horizontal = vec![0_u16; source.len()];
    for y in 0..height {
        for x in 0..width {
            let output_offset = (y as usize * width as usize + x as usize) * 4;
            for (kernel_x, weight) in WINDOWS_MATERIAL_BLUR_AXIS_KERNEL
                .iter()
                .copied()
                .enumerate()
            {
                let sample_x = horizontal_samples[x as usize][kernel_x];
                let source_offset = (y as usize * width as usize + sample_x) * 4;
                for channel in 0..4 {
                    horizontal[output_offset + channel] +=
                        u16::from(source[source_offset + channel]) * weight as u16;
                }
            }
        }
    }

    let mut output = vec![0; source.len()];
    for y in 0..height {
        for x in 0..width {
            let output_offset = (y as usize * width as usize + x as usize) * 4;
            let mut vertical_sum = [0_u32; 4];
            for (kernel_y, weight) in WINDOWS_MATERIAL_BLUR_AXIS_KERNEL
                .iter()
                .copied()
                .enumerate()
            {
                let sample_y = vertical_samples[y as usize][kernel_y];
                let sample_offset = (sample_y * width as usize + x as usize) * 4;
                for channel in 0..4 {
                    vertical_sum[channel] +=
                        u32::from(horizontal[sample_offset + channel]) * weight;
                }
            }

            let above_y = vertical_samples[y as usize][1];
            let below_y = vertical_samples[y as usize][3];
            let [left_x, right_x] = horizontal_neighbors[x as usize];
            let center_offset = (y as usize * width as usize + x as usize) * 4;
            let adjacent_offsets = [
                (above_y * width as usize + x as usize) * 4,
                (y as usize * width as usize + left_x) * 4,
                (y as usize * width as usize + right_x) * 4,
                (below_y * width as usize + x as usize) * 4,
            ];
            for channel in 0..4 {
                let cross_sum = u32::from(source[adjacent_offsets[0] + channel])
                    + u32::from(source[adjacent_offsets[1] + channel])
                    + u32::from(source[adjacent_offsets[2] + channel])
                    + u32::from(source[adjacent_offsets[3] + channel]);
                let correction = 2 * cross_sum + 8 * u32::from(source[center_offset + channel]);
                let corrected_sum = vertical_sum[channel] - correction;
                output[output_offset + channel] =
                    ((corrected_sum + WINDOWS_MATERIAL_BLUR_WEIGHT / 2)
                        / WINDOWS_MATERIAL_BLUR_WEIGHT) as u8;
            }
        }
    }
    image::RgbaImage::from_raw(width, height, output)
        .expect("blur output keeps the source sample dimensions")
}

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

#[cfg(target_os = "windows")]
pub(crate) struct SampledFramePixels {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) rgba: Vec<u8>,
}

/// Turn a throttled native D3D11 frame sample into the low-resolution material
/// texture used by the shared OpenGL path. Raw mpv samples may be 4K or larger;
/// only the small, blurred image crosses into Slint's renderer. The native
/// Windows surface scales this sample back to video bounds, so use a wider
/// low-resolution blur to preserve Acrylic's broad diffusion at display size.
#[cfg(target_os = "windows")]
pub(crate) fn sampled_frame_pixels(
    frame: &riina_core::CapturedFrame,
) -> Result<SampledFramePixels, String> {
    let sample = frame.downsample_to_fit(
        WINDOWS_NATIVE_MATERIAL_SAMPLE_WIDTH as u32,
        WINDOWS_NATIVE_MATERIAL_SAMPLE_HEIGHT as u32,
    )?;
    let width = sample.width;
    let height = sample.height;
    let sample = image::RgbaImage::from_raw(width, height, sample.rgba.to_vec())
        .ok_or_else(|| "The captured material frame has invalid RGBA dimensions".to_owned())?;
    let reduced_width = width.div_ceil(4);
    let reduced_height = height.div_ceil(4);
    let reduced = image::imageops::resize(
        &sample,
        reduced_width,
        reduced_height,
        image::imageops::FilterType::Triangle,
    );
    // Blur quarter-size pixels with quarter-size spacing, then restore the
    // original resolution so the material keeps its fine, stable grain pattern.
    let step_x = (WINDOWS_NATIVE_MATERIAL_BLUR_SIGMA * reduced_width as f32 / width as f32)
        .round()
        .max(1.0) as i32;
    let step_y = (WINDOWS_NATIVE_MATERIAL_BLUR_SIGMA * reduced_height as f32 / height as f32)
        .round()
        .max(1.0) as i32;
    let reduced_blur = blur_native_windows_sample_with_steps(&reduced, step_x, step_y);
    let mut blurred = resize_native_windows_material_sample(&reduced_blur, width, height);
    add_native_windows_material_grain(&mut blurred);
    Ok(SampledFramePixels {
        width,
        height,
        rgba: blurred.into_raw(),
    })
}

#[cfg(any(target_os = "windows", test))]
fn resize_native_windows_material_sample(
    sample: &image::RgbaImage,
    width: u32,
    height: u32,
) -> image::RgbaImage {
    // The native overlay refreshes this material on every captured video sample;
    // precomputing each axis avoids rebuilding generic filter taps per frame.
    let (source_width, source_height) = sample.dimensions();
    let x_coordinates = (0..width)
        .map(|x| {
            let source_x = ((x as f64 + 0.5) * f64::from(source_width) / f64::from(width) - 0.5)
                .clamp(0.0, f64::from(source_width - 1));
            let left = source_x.floor() as u32;
            (
                left,
                (left + 1).min(source_width - 1),
                (source_x - f64::from(left)) as f32,
            )
        })
        .collect::<Vec<_>>();
    let y_coordinates = (0..height)
        .map(|y| {
            let source_y = ((y as f64 + 0.5) * f64::from(source_height) / f64::from(height) - 0.5)
                .clamp(0.0, f64::from(source_height - 1));
            let top = source_y.floor() as u32;
            (
                top,
                (top + 1).min(source_height - 1),
                (source_y - f64::from(top)) as f32,
            )
        })
        .collect::<Vec<_>>();
    let source = sample.as_raw();
    let mut output = vec![0_u8; width as usize * height as usize * 4];
    for (y, (top, bottom, fy)) in y_coordinates.into_iter().enumerate() {
        let top_row = top as usize * source_width as usize * 4;
        let bottom_row = bottom as usize * source_width as usize * 4;
        for (x, (left, right, fx)) in x_coordinates.iter().copied().enumerate() {
            let top_left = top_row + left as usize * 4;
            let top_right = top_row + right as usize * 4;
            let bottom_left = bottom_row + left as usize * 4;
            let bottom_right = bottom_row + right as usize * 4;
            let destination = (y * width as usize + x) * 4;
            for channel in 0..4 {
                let upper = f32::from(source[top_left + channel]) * (1.0 - fx)
                    + f32::from(source[top_right + channel]) * fx;
                let lower = f32::from(source[bottom_left + channel]) * (1.0 - fx)
                    + f32::from(source[bottom_right + channel]) * fx;
                output[destination + channel] = (upper * (1.0 - fy) + lower * fy).round() as u8;
            }
        }
    }
    image::RgbaImage::from_raw(width, height, output)
        .expect("bilinear output keeps the requested material sample dimensions")
}

#[cfg(target_os = "windows")]
pub(crate) fn slint_image_from_sampled_pixels(pixels: SampledFramePixels) -> slint::Image {
    let buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
        &pixels.rgba,
        pixels.width,
        pixels.height,
    );
    slint::Image::from_rgba8(buffer)
}

#[cfg(any(target_os = "windows", test))]
fn add_native_windows_material_grain(sample: &mut image::RgbaImage) {
    let width = u64::from(sample.width());
    for (index, pixel) in sample.pixels_mut().enumerate() {
        let x = (index as u64) % width;
        let y = (index as u64) / width;
        // Keep the noise anchored to the surface; reseeding on every video
        // sample makes the Acrylic approximation visibly shimmer during motion.
        let noise = ((x.wrapping_mul(73_856_093)
            ^ y.wrapping_mul(19_349_663)
            ^ WINDOWS_NATIVE_MATERIAL_GRAIN_SEED)
            % 13) as i16
            - 6;
        for channel in &mut pixel.0[..3] {
            *channel = (i16::from(*channel) + noise).clamp(0, 255) as u8;
        }
        pixel.0[3] = u8::MAX;
    }
}

const VERTEX_SHADER: &str = r#"#version 150
in vec2 position;
in vec2 texture_coordinate;
out vec2 uv;
void main() {
    gl_Position = vec4(position, 0.0, 1.0);
    uv = texture_coordinate;
}
"#;

const FRAGMENT_SHADER: &str = r#"#version 150
uniform sampler2D source_frame;
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
    pub(crate) unsafe fn new(
        loader: &dyn Fn(&CStr) -> *const std::ffi::c_void,
    ) -> Result<Self, String> {
        let gl = Rc::new(unsafe { glow::Context::from_loader_function_cstr(|name| loader(name)) });
        let program = unsafe { create_program(&gl)? };
        let texture = match unsafe { gl.create_texture() } {
            Ok(value) => value,
            Err(error) => {
                unsafe { gl.delete_program(program) };
                return Err(error);
            }
        };
        let framebuffer = match unsafe { gl.create_framebuffer() } {
            Ok(value) => value,
            Err(error) => {
                unsafe {
                    gl.delete_texture(texture);
                    gl.delete_program(program);
                }
                return Err(error);
            }
        };
        let vertex_array = match unsafe { gl.create_vertex_array() } {
            Ok(value) => value,
            Err(error) => {
                unsafe {
                    gl.delete_framebuffer(framebuffer);
                    gl.delete_texture(texture);
                    gl.delete_program(program);
                }
                return Err(error);
            }
        };
        let vertex_buffer = match unsafe { gl.create_buffer() } {
            Ok(value) => value,
            Err(error) => {
                unsafe {
                    gl.delete_vertex_array(vertex_array);
                    gl.delete_framebuffer(framebuffer);
                    gl.delete_texture(texture);
                    gl.delete_program(program);
                }
                return Err(error);
            }
        };
        let source_uniform = unsafe { gl.get_uniform_location(program, "source_frame") };
        let step_uniform = unsafe { gl.get_uniform_location(program, "sample_step") };
        let grain_uniform = unsafe { gl.get_uniform_location(program, "grain_strength") };
        let grain_strength = if cfg!(target_os = "windows") {
            0.05
        } else {
            0.0
        };
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
        unsafe {
            let old_draw_framebuffer =
                NonZeroU32::new(gl.get_parameter_i32(glow::DRAW_FRAMEBUFFER_BINDING) as u32)
                    .map(glow::NativeFramebuffer);
            let old_read_framebuffer =
                NonZeroU32::new(gl.get_parameter_i32(glow::READ_FRAMEBUFFER_BINDING) as u32)
                    .map(glow::NativeFramebuffer);
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
                    self.size = (width, height);
                }
                gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.framebuffer));
                gl.framebuffer_texture_2d(
                    glow::FRAMEBUFFER,
                    glow::COLOR_ATTACHMENT0,
                    glow::TEXTURE_2D,
                    Some(self.texture),
                    0,
                );
                if gl.check_framebuffer_status(glow::FRAMEBUFFER) != glow::FRAMEBUFFER_COMPLETE {
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
            gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, old_draw_framebuffer);
            gl.bind_framebuffer(glow::READ_FRAMEBUFFER, old_read_framebuffer);
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
    let vertex = unsafe { compile_shader(gl, glow::VERTEX_SHADER, VERTEX_SHADER)? };
    let fragment = match unsafe { compile_shader(gl, glow::FRAGMENT_SHADER, FRAGMENT_SHADER) } {
        Ok(shader) => shader,
        Err(error) => {
            unsafe { gl.delete_shader(vertex) };
            return Err(error);
        }
    };
    let program = unsafe { gl.create_program() }.map_err(|error| error.to_string())?;
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
    if !unsafe { gl.get_program_link_status(program) } {
        let log = unsafe { gl.get_program_info_log(program) };
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
    let shader = unsafe { gl.create_shader(kind) }.map_err(|error| error.to_string())?;
    unsafe {
        gl.shader_source(shader, source);
        gl.compile_shader(shader);
    }
    if !unsafe { gl.get_shader_compile_status(shader) } {
        let log = unsafe { gl.get_shader_info_log(shader) };
        unsafe { gl.delete_shader(shader) };
        return Err(format!("Could not compile video-blur shader: {log}"));
    }
    Ok(shader)
}

#[cfg(test)]
mod sampled_frame_tests {
    use super::{
        add_native_windows_material_grain, blur_native_windows_sample,
        resize_native_windows_material_sample,
    };
    use image::{Rgba, RgbaImage};
    use riina_core::CapturedFrame;
    use std::sync::Arc;

    #[test]
    fn native_material_enlargement_matches_triangle_resampling() {
        let sample = RgbaImage::from_fn(7, 5, |x, y| {
            Rgba([
                (x * 31 + y * 7) as u8,
                (y * 43 + x * 3) as u8,
                (x * 11 + y * 29) as u8,
                (x * 17 + y * 13) as u8,
            ])
        });
        let expected =
            image::imageops::resize(&sample, 32, 18, image::imageops::FilterType::Triangle);
        let actual = resize_native_windows_material_sample(&sample, 32, 18);

        for (expected, actual) in expected.pixels().zip(actual.pixels()) {
            for channel in 0..4 {
                assert!(
                    expected.0[channel].abs_diff(actual.0[channel]) <= 1,
                    "channel {channel} differs: expected {}, got {}",
                    expected.0[channel],
                    actual.0[channel]
                );
            }
        }
    }

    #[test]
    fn windows_acrylic_blur_diffuses_edges_across_the_upscaled_sample() {
        let mut pattern = RgbaImage::from_pixel(320, 180, Rgba([0, 0, 0, 255]));
        for y in 0..180 {
            for x in 158..=162 {
                pattern.put_pixel(x, y, Rgba([255, 255, 255, 255]));
            }
        }

        let blurred = blur_native_windows_sample(&pattern);
        let center = blurred.get_pixel(160, 90)[0];
        let shoulder = blurred.get_pixel(176, 90)[0];

        assert!(center > shoulder);
        assert!(
            shoulder > 4,
            "expected a broad Acrylic-like blur shoulder, got {shoulder}"
        );
    }

    #[test]
    fn windows_material_grain_is_stable_across_video_samples() {
        let mut first = RgbaImage::from_pixel(32, 18, Rgba([100, 100, 100, 255]));
        let mut next = RgbaImage::from_pixel(32, 18, Rgba([120, 120, 120, 255]));

        add_native_windows_material_grain(&mut first);
        add_native_windows_material_grain(&mut next);

        for (first, next) in first.pixels().zip(next.pixels()) {
            assert_eq!(next.0[0] - first.0[0], 20);
            assert_eq!(next.0[1] - first.0[1], 20);
            assert_eq!(next.0[2] - first.0[2], 20);
            assert_eq!(next.0[3], first.0[3]);
        }
    }

    #[test]
    fn material_sample_is_bounded_and_preserves_uniform_color() {
        let mut rgba = Vec::with_capacity(640 * 360 * 4);
        for _ in 0..640 * 360 {
            rgba.extend_from_slice(&[96, 144, 192, 255]);
        }
        let frame = CapturedFrame::new(640, 360, 640 * 4, &rgba).unwrap();

        let sample = frame.downsample_to_fit(320, 180).unwrap();

        assert_eq!((sample.width, sample.height), (320, 180));
        let (pixels, remainder) = sample.rgba.as_chunks::<4>();
        assert!(remainder.is_empty());
        assert!(pixels.iter().all(|pixel| *pixel == [96, 144, 192, 255]));
    }

    #[test]
    fn material_sample_rejects_inconsistent_frame_storage() {
        let frame = CapturedFrame {
            width: 2,
            height: 2,
            rgba: Arc::from([0u8; 3]),
        };

        assert!(frame.downsample_to_fit(320, 180).is_err());
    }
}
