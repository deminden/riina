//! CPU transform for the native Windows Acrylic frame sample.

#[cfg(target_os = "windows")]
use super::{
    WINDOWS_NATIVE_MATERIAL_BLUR_SIGMA, WINDOWS_NATIVE_MATERIAL_SAMPLE_HEIGHT,
    WINDOWS_NATIVE_MATERIAL_SAMPLE_WIDTH,
};

const WINDOWS_NATIVE_MATERIAL_GRAIN_SEED: u64 = 0x5749_4e44_4f57_5301;
const WINDOWS_MATERIAL_BLUR_AXIS_KERNEL: [u32; 5] = [1, 4, 7, 4, 1];
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

#[expect(
    clippy::expect_used,
    reason = "The output buffer is sized from the input dimensions"
)]
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
    let sample = frame
        .downsample_to_fit(
            WINDOWS_NATIVE_MATERIAL_SAMPLE_WIDTH as u32,
            WINDOWS_NATIVE_MATERIAL_SAMPLE_HEIGHT as u32,
        )
        .map_err(|error| error.to_string())?;
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

#[expect(
    clippy::expect_used,
    reason = "The output buffer is sized from the requested dimensions"
)]
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
