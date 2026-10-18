//! Opt-in, bounded framebuffer diagnostics for native controls composition.
use glow::HasContext;
use std::time::{Duration, Instant};

pub(super) struct OverlayTrace {
    enabled: bool,
    gl: Option<glow::Context>,
    last_sample: Option<Instant>,
    samples: usize,
}

impl OverlayTrace {
    pub(super) fn new() -> Self {
        Self {
            enabled: std::env::var_os("RIINA_TRACE_NATIVE_ALPHA").is_some(),
            gl: None,
            last_sample: None,
            samples: 0,
        }
    }

    pub(super) fn observe(
        &mut self,
        state: &slint::RenderingState,
        api: &slint::GraphicsAPI<'_>,
        app: &crate::PlayerWindow,
    ) {
        if !self.enabled {
            return;
        }
        match state {
            slint::RenderingState::RenderingSetup => {
                if let slint::GraphicsAPI::NativeOpenGL { get_proc_address } = api {
                    // SAFETY: Slint keeps the renderer context current in this callback.
                    self.gl = Some(unsafe {
                        glow::Context::from_loader_function_cstr(|name| get_proc_address(name))
                    });
                }
            }
            slint::RenderingState::RenderingTeardown => self.gl = None,
            slint::RenderingState::AfterRendering => {
                if self.samples >= 120
                    || self
                        .last_sample
                        .is_some_and(|time| time.elapsed() < Duration::from_millis(500))
                {
                    return;
                }
                let Some(gl) = &self.gl else { return };
                self.last_sample = Some(Instant::now());
                self.samples += 1;
                let mut viewport = [0; 4];
                let mut pixels = [[0_u8; 4]; 3];
                // SAFETY: AfterRendering runs with Slint's context current, before
                // presentation. Three single-pixel reads inspect clear canvas away
                // from the controller without capturing video or retaining GPU objects.
                let (alpha_bits, framebuffer) = unsafe {
                    gl.get_parameter_i32_slice(glow::VIEWPORT, &mut viewport);
                    let alpha_bits = gl.get_parameter_i32(0x0D55); // GL_ALPHA_BITS
                    let framebuffer = gl.get_parameter_i32(glow::FRAMEBUFFER_BINDING);
                    for (pixel, (x, y)) in pixels.iter_mut().zip([(1, 1), (2, 2), (3, 3)]) {
                        gl.read_pixels(
                            viewport[0] + viewport[2] * x / 4,
                            viewport[1] + viewport[3] * y / 4,
                            1,
                            1,
                            glow::RGBA,
                            glow::UNSIGNED_BYTE,
                            glow::PixelPackData::Slice(Some(pixel)),
                        );
                    }
                    (alpha_bits, framebuffer)
                };
                eprintln!(
                    "native-alpha fullscreen={} controls={} rendered={} viewport={viewport:?} alpha_bits={alpha_bits} framebuffer={framebuffer} rgba={pixels:?}",
                    app.get_fullscreen(),
                    app.get_controls_visible(),
                    app.get_controls_rendered(),
                );
            }
            _ => {}
        }
    }
}
