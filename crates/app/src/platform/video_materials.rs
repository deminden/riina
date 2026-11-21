//! Own native video-material refresh for the player's presentation lifetime.
use crate::{PlaybackStore, PlayerWindow, platform::NativePresentation};
use riina_mpv::CommandSender;
use slint::ComponentHandle;

#[cfg(any(target_os = "windows", test))]
pub(super) mod native_blur;

// At the measured 915-point player width, sigma 10 on this 320px sample scales
// to about 29 points on the displayed material.
pub(crate) const WINDOWS_NATIVE_MATERIAL_BLUR_SIGMA: f32 = 10.0;
pub(crate) const WINDOWS_NATIVE_MATERIAL_SAMPLE_WIDTH: f32 = 320.0;
pub(crate) const WINDOWS_NATIVE_MATERIAL_SAMPLE_HEIGHT: f32 = 180.0;

pub(crate) struct Monitor {
    #[cfg(target_os = "windows")]
    _sample: Option<std::rc::Rc<slint::Timer>>,
    #[cfg(target_os = "windows")]
    _policy: std::rc::Rc<slint::Timer>,
}

impl Monitor {
    #[cfg(target_os = "windows")]
    pub(crate) fn install(
        app: &PlayerWindow,
        native: Option<&NativePresentation>,
        commands: &CommandSender,
        playback: &PlaybackStore,
    ) -> Self {
        use super::windows::video_material_sampler::{
            install_native_video_material_sampler, install_windows_material_policy_monitor,
        };
        let sample = native.map(|native| {
            install_native_video_material_sampler(
                app,
                commands.clone(),
                playback.clone(),
                native.material_capture(),
            )
        });
        let policy = install_windows_material_policy_monitor(app);
        Self {
            _sample: sample,
            _policy: policy,
        }
    }

    #[cfg(not(target_os = "windows"))]
    pub(crate) fn install(
        _: &PlayerWindow,
        _: Option<&NativePresentation>,
        _: &CommandSender,
        _: &PlaybackStore,
    ) -> Self {
        Self {}
    }
}

pub(crate) fn set_video_material_sample(app: &PlayerWindow, image: slint::Image, ready: bool) {
    app.global::<riina_ui::PlayerRenderedVideo>()
        .set_video_blurred_frame(image);
    app.global::<riina_ui::PlayerRenderedVideo>()
        .set_video_material_sample_ready(ready);
}
