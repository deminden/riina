//! Decode the playback settings form in the order its errors are shown.

use super::read_or_refresh;
use crate::{
    PlayerWindow, PreferencesWindow,
    preferences::model::{PreferenceModel, PreferenceSection, SectionError},
};
use riina_core::{
    Preferences, advanced_preferences, audio_preferences, control_preferences, hdr_preferences,
    key_bindings,
    network_preferences::{Cache, Connection, OnlineVideos},
};

pub(super) struct Draft {
    pub(super) previous_cache: Cache,
    pub(super) cache: Cache,
    pub(super) network: Connection,
    pub(super) audio: audio_preferences::Settings,
    pub(super) tone_mapping: hdr_preferences::Settings,
    pub(super) previous_tone_mapping: hdr_preferences::Settings,
    pub(super) tone_mapping_changed: bool,
    pub(super) video_threads: u32,
    pub(super) previous_video_threads: u32,
    pub(super) video_threads_changed: bool,
    pub(super) controls: control_preferences::Settings,
    pub(super) key_bindings: key_bindings::Settings,
    pub(super) advanced: advanced_preferences::Settings,
    advanced_changed: bool,
    audio_changed: bool,
    pub(super) previous_online_videos: OnlineVideos,
    pub(super) online_videos: OnlineVideos,
}

impl Draft {
    pub(super) fn read(
        app: &PlayerWindow,
        window: &PreferencesWindow,
        model: &PreferenceModel,
        saved: &Preferences,
        baseline: &Preferences,
    ) -> Option<Self> {
        let previous_cache = read_or_refresh(
            crate::network_cache_preferences::current_player(app),
            app,
            window,
        )?;
        let cache = read_or_refresh(model.read_section::<Cache>(window), app, window)?;
        let network = read_or_refresh(
            Connection::read_window(window).map_err(SectionError::into_message),
            app,
            window,
        )?;
        let audio = read_or_refresh(
            crate::audio_preferences::current_window(window),
            app,
            window,
        )?;
        let tone_mapping = match model.read_section::<hdr_preferences::Settings>(window) {
            Ok(settings) => settings,
            Err(error) => {
                app.set_status(error.into());
                return None;
            }
        };
        let previous_tone_mapping = saved.hdr_tone_mapping.clone();
        let tone_mapping_changed = tone_mapping != previous_tone_mapping;
        let video_threads = match window.get_video_threads_text().trim().parse::<u32>() {
            Ok(threads) if threads <= i32::MAX as u32 => threads,
            _ => {
                window.set_video_threads_text(saved.video_threads.to_string().into());
                app.set_status("Video decoder threads must be between 0 and 2,147,483,647".into());
                return None;
            }
        };
        let previous_video_threads = saved.video_threads;
        let video_threads_changed = video_threads != previous_video_threads;
        let controls = read_or_refresh(
            model.read_section::<control_preferences::Settings>(window),
            app,
            window,
        )?;
        let key_bindings = read_or_refresh(
            crate::key_binding_preferences::current_window(window),
            app,
            window,
        )?;
        let advanced = read_or_refresh(
            advanced_preferences::Settings::read_window(window).map_err(SectionError::into_message),
            app,
            window,
        )?;
        let advanced_changed = advanced != baseline.advanced;
        let audio_changed = audio != baseline.audio;
        let previous_online_videos = read_or_refresh(
            crate::network_ytdl_preferences::current_player(app),
            app,
            window,
        )?;
        let online_videos =
            read_or_refresh(model.read_section::<OnlineVideos>(window), app, window)?;
        Some(Self {
            previous_cache,
            cache,
            network,
            audio,
            tone_mapping,
            previous_tone_mapping,
            tone_mapping_changed,
            video_threads,
            previous_video_threads,
            video_threads_changed,
            controls,
            key_bindings,
            advanced,
            advanced_changed,
            audio_changed,
            previous_online_videos,
            online_videos,
        })
    }

    pub(super) fn restart_notice(&self) -> Option<&'static str> {
        if self.advanced_changed {
            Some("Advanced settings apply after restarting Riina")
        } else if self.audio_changed {
            Some("Audio engine preferences apply after restarting Riina")
        } else {
            None
        }
    }
}
