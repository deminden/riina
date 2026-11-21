//! Settings only polls system motion and audio devices while it is visible.
use crate::{PlaybackStore, PreferencesWindow, windows::visible_refresh};
use riina_core::audio_preferences::{OutputDevice, Settings};
use slint::{ComponentHandle, Timer};
use std::{rc::Rc, sync::Arc, time::Duration};

#[derive(Default)]
pub(super) struct VisibleSettings {
    motion: Rc<Timer>,
    audio_devices: Rc<Timer>,
}

impl VisibleSettings {
    pub(super) fn install_close(&self, window: &PreferencesWindow) {
        let weak = window.as_weak();
        let motion = Rc::downgrade(&self.motion);
        let audio_devices = Rc::downgrade(&self.audio_devices);
        window.window().on_close_requested(move || {
            if let Some(window) = weak.upgrade()
                && !window.get_folder_dialog_open()
            {
                if let Some(motion) = motion.upgrade() {
                    motion.stop();
                }
                if let Some(audio_devices) = audio_devices.upgrade() {
                    audio_devices.stop();
                }
                let _ = window.hide();
            }
            slint::CloseRequestResponse::KeepWindowShown
        });
    }

    pub(super) fn start(
        &self,
        window: &PreferencesWindow,
        latest_playback: PlaybackStore,
        saved_audio: Settings,
        mut previous: Arc<Vec<OutputDevice>>,
    ) {
        visible_refresh::run(window, &self.motion, Duration::from_millis(500), |window| {
            let reduce_motion = crate::platform::system_reduce_motion_enabled();
            if window.get_system_reduce_motion() != reduce_motion {
                window.set_system_reduce_motion(reduce_motion);
            }
        });
        visible_refresh::run(
            window,
            &self.audio_devices,
            Duration::from_millis(750),
            move |window| {
                let available = latest_playback
                    .lock()
                    .map(|playback| playback.audio_devices.clone())
                    .unwrap_or_default();
                if previous == available {
                    return;
                }
                let selected = crate::audio_preferences::current_window(window)
                    .unwrap_or_else(|_| saved_audio.clone());
                let device_disconnected = selected.device != "auto"
                    && previous.iter().any(|device| device.name == selected.device)
                    && !available
                        .iter()
                        .any(|device| device.name == selected.device);
                previous = available.clone();
                let selected = if device_disconnected {
                    let mut selected = selected;
                    selected.device = "auto".into();
                    selected.device_description = "Autoselect device".into();
                    selected
                } else {
                    selected
                };
                crate::audio_preferences::install_devices(window, &selected, &available);
                if device_disconnected {
                    window.invoke_settings_changed();
                }
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::VisibleSettings;
    use crate::PreferencesWindow;
    use slint::{ComponentHandle, platform::WindowEvent};

    #[test]
    fn settings_refresh_stops_when_close_dismisses_the_window()
    -> Result<(), Box<dyn std::error::Error>> {
        i_slint_backend_testing::init_no_event_loop();
        let window = PreferencesWindow::new()?;
        let timers = VisibleSettings::default();
        timers.install_close(&window);
        assert!(!timers.motion.running());
        assert!(!timers.audio_devices.running());

        window.show()?;
        timers.start(
            &window,
            Default::default(),
            Default::default(),
            Default::default(),
        );
        assert!(timers.motion.running());
        assert!(timers.audio_devices.running());

        window.set_folder_dialog_open(true);
        window.window().dispatch_event(WindowEvent::CloseRequested);
        assert!(window.window().is_visible());
        assert!(timers.motion.running());
        assert!(timers.audio_devices.running());

        window.set_folder_dialog_open(false);
        window.window().dispatch_event(WindowEvent::CloseRequested);
        assert!(!window.window().is_visible());
        assert!(!timers.motion.running());
        assert!(!timers.audio_devices.running());
        Ok(())
    }
}
