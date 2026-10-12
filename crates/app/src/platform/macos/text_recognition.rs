use objc2::{AnyThread, rc::Retained, rc::autoreleasepool, runtime::AnyObject};
use objc2_core_graphics::{
    CGBitmapInfo, CGColorRenderingIntent, CGColorSpace, CGDataProvider, CGImage, CGImageAlphaInfo,
    CGImageByteOrderInfo,
};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Region {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}
use objc2_foundation::{NSDictionary, NSOperatingSystemVersion, NSProcessInfo};
use objc2_vision::{
    VNImageOption, VNImageRequestHandler, VNRecognizeTextRequest, VNRequest,
    VNRequestTextRecognitionLevel,
};
use riina_core::CapturedFrame;
use std::{
    ffi::c_void,
    io, ptr,
    thread::{self, JoinHandle},
};

/// IINA's Live Text interaction is available starting with macOS 13.
pub(crate) fn check_availability(
    complete: impl FnOnce(bool) + Send + 'static,
) -> io::Result<JoinHandle<()>> {
    thread::Builder::new()
        .name("riina-text-availability".into())
        .spawn(move || {
            let version = NSOperatingSystemVersion {
                majorVersion: 13,
                minorVersion: 0,
                patchVersion: 0,
            };
            complete(NSProcessInfo::processInfo().isOperatingSystemAtLeastVersion(version));
        })
}

/// Keep Vision's synchronous analysis away from playback and the window event loop.
pub(crate) fn recognize(
    frame: CapturedFrame,
    complete: impl FnOnce(Result<Vec<Region>, String>) + Send + 'static,
) -> io::Result<JoinHandle<()>> {
    thread::Builder::new()
        .name("riina-live-text".into())
        .spawn(move || {
            let result = autoreleasepool(|_| recognize_frame(&frame));
            complete(result);
        })
}

fn recognize_frame(frame: &CapturedFrame) -> Result<Vec<Region>, String> {
    let expected_bytes = (frame.width as usize)
        .checked_mul(frame.height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| "Captured frame dimensions are too large".to_owned())?;
    if frame.width == 0 || frame.height == 0 || frame.rgba.len() != expected_bytes {
        return Err("Captured frame data does not match its dimensions".into());
    }

    let color_space = CGColorSpace::new_device_rgb()
        .ok_or_else(|| "Could not create an RGB color space for text recognition".to_owned())?;
    // The provider borrows the owned captured frame and the synchronous Vision
    // request finishes before either the image or provider leaves this function.
    let provider = unsafe {
        CGDataProvider::with_data(
            ptr::null_mut(),
            frame.rgba.as_ptr().cast::<c_void>(),
            frame.rgba.len(),
            None,
        )
    }
    .ok_or_else(|| "Could not expose the captured frame to Vision".to_owned())?;
    let bitmap_info = CGBitmapInfo(CGImageAlphaInfo::Last.0 | CGImageByteOrderInfo::Order32Big.0);
    let image = unsafe {
        CGImage::new(
            frame.width as usize,
            frame.height as usize,
            8,
            32,
            frame.width as usize * 4,
            Some(&color_space),
            bitmap_info,
            Some(&provider),
            ptr::null(),
            false,
            CGColorRenderingIntent::RenderingIntentDefault,
        )
    }
    .ok_or_else(|| "Could not create an image for text recognition".to_owned())?;

    let options = NSDictionary::<VNImageOption, AnyObject>::new();
    let handler = unsafe {
        VNImageRequestHandler::initWithCGImage_options(
            VNImageRequestHandler::alloc(),
            &image,
            &options,
        )
    };
    let request = VNRecognizeTextRequest::new();
    request.setRecognitionLevel(VNRequestTextRecognitionLevel::Accurate);
    request.setUsesLanguageCorrection(true);
    let requests: [Retained<VNRequest>; 1] = [request.clone().into_super().into_super()];
    let requests = objc2_foundation::NSArray::from_retained_slice(&requests);
    handler
        .performRequests_error(&requests)
        .map_err(|error| error.to_string())?;

    let Some(observations) = request.results() else {
        return Ok(Vec::new());
    };
    let mut regions = Vec::with_capacity(observations.count());
    for index in 0..observations.count() {
        let observation = observations.objectAtIndex(index);
        if let Some(candidate) = observation.topCandidates(1).firstObject() {
            let text = candidate.string().to_string();
            // Vision uses a normalized bottom-left origin; the player overlay
            // uses pixel coordinates measured down from the frame's top edge.
            let bounds = unsafe { observation.boundingBox() };
            if let Some(region) = from_vision_box(
                text,
                bounds.origin.x,
                bounds.origin.y,
                bounds.size.width,
                bounds.size.height,
                frame.width,
                frame.height,
            ) {
                regions.push(region);
            }
        }
    }
    Ok(regions)
}

/// Vision reports normalized rectangles from the lower-left; the overlay uses top-left pixels.
fn from_vision_box(
    text: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    frame_width: u32,
    frame_height: u32,
) -> Option<Region> {
    if text.trim().is_empty()
        || frame_width == 0
        || frame_height == 0
        || [x, y, width, height].iter().any(|value| !value.is_finite())
        || width <= 0.0
        || height <= 0.0
    {
        return None;
    }
    let frame_width = f64::from(frame_width);
    let frame_height = f64::from(frame_height);
    let left = (x * frame_width).clamp(0.0, frame_width);
    let right = ((x + width) * frame_width).clamp(left, frame_width);
    let top = ((1.0 - y - height) * frame_height).clamp(0.0, frame_height);
    let bottom = ((1.0 - y) * frame_height).clamp(top, frame_height);
    (right > left && bottom > top).then_some(Region {
        text,
        x: left as f32,
        y: top as f32,
        width: (right - left) as f32,
        height: (bottom - top) as f32,
    })
}

#[cfg(test)]
mod tests {
    use super::from_vision_box;

    #[test]
    fn normalized_vision_box_maps_to_video_top_left_pixels() {
        assert_eq!(
            from_vision_box("text".into(), 0.1, 0.2, 0.3, 0.4, 1000, 500),
            Some(super::Region {
                text: "text".into(),
                x: 100.0,
                y: 200.0,
                width: 300.0,
                height: 200.0,
            })
        );
    }

    #[test]
    fn normalized_vision_box_clips_and_rejects_empty_content() {
        assert_eq!(
            from_vision_box("edge".into(), 0.9, 0.0, 0.2, 0.2, 100, 50),
            Some(super::Region {
                text: "edge".into(),
                x: 90.0,
                y: 40.0,
                width: 10.0,
                height: 10.0,
            })
        );
        assert!(from_vision_box("".into(), 0.0, 0.0, 0.2, 0.2, 100, 50).is_none());
        assert!(from_vision_box("off-frame".into(), 1.0, 0.0, 0.2, 0.2, 100, 50).is_none());
    }
}
