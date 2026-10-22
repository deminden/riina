//! Owned, bounded RGBA snapshots independent of native event storage.
use std::sync::Arc;

/// A tightly packed RGBA snapshot copied from the native engine's event-owned buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedFrame {
    pub width: u32,
    pub height: u32,
    pub rgba: Arc<[u8]>,
}

impl CapturedFrame {
    pub fn new(width: u32, height: u32, stride: usize, bytes: &[u8]) -> Result<Self, String> {
        const MAX_DIMENSION: u32 = 16_384;
        const MAX_BYTES: usize = 512 * 1024 * 1024;
        if width == 0 || height == 0 || width > MAX_DIMENSION || height > MAX_DIMENSION {
            return Err("The captured frame dimensions are invalid".into());
        }
        let row_bytes = (width as usize)
            .checked_mul(4)
            .ok_or("The captured frame is too large")?;
        if stride < row_bytes {
            return Err("The captured frame row is shorter than its RGBA pixels".into());
        }
        let source_len = stride
            .checked_mul(height as usize)
            .filter(|length| *length <= MAX_BYTES)
            .ok_or("The captured frame is too large")?;
        if bytes.len() < source_len {
            return Err("The captured frame buffer is incomplete".into());
        }
        let packed_len = row_bytes
            .checked_mul(height as usize)
            .ok_or("The captured frame is too large")?;
        let mut rgba = Vec::with_capacity(packed_len);
        for row in bytes[..source_len]
            .chunks_exact(stride)
            .take(height as usize)
        {
            rgba.extend_from_slice(&row[..row_bytes]);
        }
        Ok(Self {
            width,
            height,
            rgba: rgba.into(),
        })
    }

    /// Bilinearly reduce a packed frame to fit within a bounded pixel size.
    ///
    /// Material snapshots can be captured from 4K/8K sources, while the UI only
    /// needs a small blurred backdrop. This keeps large decoded frames out of
    /// long-lived playback state without adding an image-processing dependency.
    pub fn downsample_to_fit(&self, max_width: u32, max_height: u32) -> Result<Self, String> {
        const MAX_DIMENSION: u32 = 16_384;
        const MAX_BYTES: usize = 512 * 1024 * 1024;
        if max_width == 0
            || max_height == 0
            || max_width > MAX_DIMENSION
            || max_height > MAX_DIMENSION
            || self.width == 0
            || self.height == 0
            || self.width > MAX_DIMENSION
            || self.height > MAX_DIMENSION
        {
            return Err("The captured frame dimensions are invalid".into());
        }
        let source_width = self.width as usize;
        let source_height = self.height as usize;
        let source_len = source_width
            .checked_mul(source_height)
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or("The captured frame is too large")?;
        if source_len > MAX_BYTES || self.rgba.len() != source_len {
            return Err("The captured frame buffer is incomplete".into());
        }
        let scale = (max_width as f64 / f64::from(self.width))
            .min(max_height as f64 / f64::from(self.height))
            .min(1.0);
        let width = (f64::from(self.width) * scale).round().max(1.0) as u32;
        let height = (f64::from(self.height) * scale).round().max(1.0) as u32;
        if width == self.width && height == self.height {
            return Ok(self.clone());
        }

        let packed_len = (width as usize)
            .checked_mul(height as usize)
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or("The captured frame is too large")?;
        if packed_len > MAX_BYTES {
            return Err("The captured frame is too large".into());
        }
        let mut rgba = vec![0; packed_len];
        for y in 0..height {
            let source_y = ((f64::from(y) + 0.5) * f64::from(self.height) / f64::from(height)
                - 0.5)
                .clamp(0.0, f64::from(self.height - 1));
            let y0 = source_y.floor() as u32;
            let y1 = (y0 + 1).min(self.height - 1);
            let fy = source_y - f64::from(y0);
            for x in 0..width {
                let source_x = ((f64::from(x) + 0.5) * f64::from(self.width) / f64::from(width)
                    - 0.5)
                    .clamp(0.0, f64::from(self.width - 1));
                let x0 = source_x.floor() as u32;
                let x1 = (x0 + 1).min(self.width - 1);
                let fx = source_x - f64::from(x0);
                let indices = [
                    (y0 as usize * source_width + x0 as usize) * 4,
                    (y0 as usize * source_width + x1 as usize) * 4,
                    (y1 as usize * source_width + x0 as usize) * 4,
                    (y1 as usize * source_width + x1 as usize) * 4,
                ];
                let destination = (y as usize * width as usize + x as usize) * 4;
                for channel in 0..4 {
                    let top = f64::from(self.rgba[indices[0] + channel]) * (1.0 - fx)
                        + f64::from(self.rgba[indices[1] + channel]) * fx;
                    let bottom = f64::from(self.rgba[indices[2] + channel]) * (1.0 - fx)
                        + f64::from(self.rgba[indices[3] + channel]) * fx;
                    rgba[destination + channel] = (top * (1.0 - fy) + bottom * fy).round() as u8;
                }
            }
        }
        Ok(Self {
            width,
            height,
            rgba: rgba.into(),
        })
    }
}
