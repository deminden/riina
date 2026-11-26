use super::*;
use gtk::cairo::{Context, FontSlant, FontWeight, Format, ImageSurface};

#[test]
fn a_cancelled_request_does_not_open_the_engine() {
    let frame = CapturedFrame {
        width: 0,
        height: 0,
        rgba: Default::default(),
    };
    let cancellation = Cancellation::default();
    cancellation.cancel();
    assert!(recognize_frame(&frame, &cancellation).unwrap().is_empty());
    assert!(recognize_frame(&frame, &Cancellation::default()).is_err());
}

#[test]
#[ignore = "requires the Linux Tesseract 5 runtime and English recognition data"]
fn native_tesseract_detects_selectable_lines_and_original_pixel_bounds() {
    use std::{sync::mpsc, time::Duration};

    let (available, probe) = mpsc::channel();
    let worker = check_availability(move |value| available.send(value).unwrap()).unwrap();
    assert!(probe.recv_timeout(Duration::from_secs(5)).unwrap());
    worker.join().unwrap();

    let (width, height) = (3200, 640);
    let mut surface = ImageSurface::create(Format::ARgb32, width, height).unwrap();
    {
        let context = Context::new(&surface).unwrap();
        context.set_source_rgb(1.0, 1.0, 1.0);
        context.paint().unwrap();
        context.set_source_rgb(0.0, 0.0, 0.0);
        context.select_font_face("DejaVu Sans", FontSlant::Normal, FontWeight::Normal);
        context.set_font_size(104.0);
        context.move_to(180.0, 205.0);
        context.show_text("Riina media playback").unwrap();
        context.move_to(1660.0, 490.0);
        context.show_text("Second text region").unwrap();
    }
    let stride = surface.stride() as usize;
    let frame = CapturedFrame::new(
        width as u32,
        height as u32,
        stride,
        &surface.data().unwrap(),
    )
    .unwrap();
    let (result, completion) = mpsc::channel();
    let worker = recognize(frame, Cancellation::default(), move |regions| {
        result.send(regions).unwrap()
    })
    .unwrap();
    let regions = completion
        .recv_timeout(Duration::from_secs(7))
        .unwrap()
        .unwrap();
    worker.join().unwrap();
    for (expected, bounds) in [
        ("Riina media playback", [180.0, 120.0, 1300.0, 235.0]),
        ("Second text region", [1660.0, 400.0, 2710.0, 520.0]),
    ] {
        let region = regions
            .iter()
            .find(|region| region.text == expected)
            .unwrap_or_else(|| panic!("Could not find {expected}: {regions:?}"));
        assert!(region.x >= bounds[0] && region.y >= bounds[1]);
        assert!(region.x + region.width <= bounds[2] && region.y + region.height <= bounds[3]);
    }
}
