//! Apply one playback snapshot to the window and its transient controls.
use super::{
    catalog::{current_chapter, should_fallback_removed_audio_device, update_catalog},
    clear_screenshot_preview, media_geometry_requires_resize, window_video_geometry,
};
use crate::{
    PlayerWindow,
    seek_osd::{self, Change, SeekOsd},
};
use riina_core::{Command, LoopMode, Phase, Playback, TrackKind, time_label};
#[cfg(not(target_os = "macos"))]
use std::path::Path;
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

pub(super) struct UiUpdateResources<'a> {
    pub(super) audio_device_player: Option<&'a riina_mpv::CommandSender>,
    pub(super) hdr_player: Option<&'a riina_mpv::CommandSender>,
    pub(super) saved_video_filters: &'a Arc<std::sync::Mutex<riina_core::video_filters::Settings>>,
    pub(super) saved_audio_filters: &'a Arc<std::sync::Mutex<riina_core::video_filters::Settings>>,
    pub(super) transition_overlay: &'a AtomicU64,
    #[cfg(target_os = "windows")]
    pub(super) transition_release_generation: &'a AtomicU64,
}

pub(super) fn update_ui(
    app: &PlayerWindow,
    state: Playback,
    resources: UiUpdateResources<'_>,
    displayed: &mut Option<Playback>,
    last_seek: &mut SeekOsd,
    last_video: &mut crate::video_osd::VideoOsd,
    last_screenshot: &mut crate::screenshot_osd::ScreenshotOsd,
) -> bool {
    use riina_core::video_reflection::Axis;
    if app.get_native_video() {
        let show_transition = {
            #[cfg(target_os = "windows")]
            {
                crate::transition_frame::should_display_until_incoming_stable(
                    &state,
                    app.get_playlist_auto_play_next(),
                    match resources
                        .transition_release_generation
                        .load(Ordering::Acquire)
                    {
                        u64::MAX => None,
                        generation => Some(generation),
                    },
                )
            }
            #[cfg(not(target_os = "windows"))]
            {
                crate::transition_frame::should_display(&state, app.get_playlist_auto_play_next())
            }
        };
        // The transition surface has an opaque black matte beneath its image.
        // Keep the surface disabled unless the retained frame is present, so
        // a missing capture can never become a black rectangle over playback.
        let show_transition =
            show_transition && crate::transition_frame::cover_frame(&state).is_some();
        let source_generation = state.transition_frame_generation.unwrap_or(u64::MAX);
        let incoming_generation = state
            .incoming_transition_frame_generation
            .unwrap_or(u64::MAX);
        let key = source_generation.rotate_left(13)
            ^ state.transition_frame_revision.rotate_left(29)
            ^ incoming_generation.rotate_left(37)
            ^ state.incoming_transition_frame_revision.rotate_left(7)
            ^ u64::from(show_transition);
        if resources.transition_overlay.swap(key, Ordering::AcqRel) != key {
            let image = if show_transition {
                crate::transition_frame::cover_frame(&state).map(|frame| {
                    let mut rgba = frame.rgba.as_ref().to_vec();
                    for pixel in rgba.as_chunks_mut::<4>().0 {
                        // Video pixels are an opaque surface. mpv's raw RGBA
                        // capture may retain a partial alpha channel, which
                        // lets the reconfiguring native child bleed through
                        // the transition cover as a brief double image.
                        pixel[3] = u8::MAX;
                    }
                    let pixels = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
                        &rgba,
                        frame.width,
                        frame.height,
                    );
                    slint::Image::from_rgba8(pixels)
                })
            } else {
                None
            };
            if let Some(image) = image {
                // Install the opaque image before exposing its cover rectangle.
                // Slint can schedule a render between property updates; enabling
                // the cover first would briefly show its black backing instead of
                // the retained frame during a native-output handoff.
                app.set_video_frame(image);
                app.set_native_transition_frame(true);
            } else {
                // Keep the last opaque frame until the cover is disabled. Clearing
                // it first can expose the cover's black backing for one render.
                app.set_native_transition_frame(false);
                app.set_video_frame(slint::Image::default());
            }
        }
    }
    let previous_generation = app.get_window_geometry_source().parse::<u64>().ok();
    app.set_window_geometry_source(state.generation.to_string().into());
    if let Ok(settings) = resources.saved_video_filters.lock() {
        let active = state.video_filters.as_deref().unwrap_or_default();
        crate::ui_models::set_rows_if_changed(
            &app.get_saved_video_filter_rows(),
            crate::video_filter_window::saved_filter_rows(&settings.saved, active),
            |rows| app.set_saved_video_filter_rows(rows),
        );
    }
    if let Ok(settings) = resources.saved_audio_filters.lock() {
        let active = state.audio_filters.as_deref().unwrap_or_default();
        crate::ui_models::set_rows_if_changed(
            &app.get_saved_audio_filter_rows(),
            crate::video_filter_window::saved_filter_rows(&settings.saved, active),
            |rows| app.set_saved_audio_filter_rows(rows),
        );
    }
    #[cfg(target_os = "macos")]
    let current_media_path = match crate::key_binding_preferences::current_media_location(&state) {
        crate::key_binding_preferences::CurrentMediaLocation::Local(path) => {
            std::path::absolute(path).ok()
        }
        crate::key_binding_preferences::CurrentMediaLocation::NoMedia
        | crate::key_binding_preferences::CurrentMediaLocation::NonLocal => None,
    };
    #[cfg(not(target_os = "macos"))]
    let current_media_path = state
        .playlist
        .iter()
        .find(|entry| entry.current || entry.playing)
        .map(|entry| PathBuf::from(&entry.filename));
    #[cfg(target_os = "macos")]
    platform::set_represented_file(app.window(), current_media_path.as_deref());
    #[cfg(target_os = "macos")]
    let title_path = current_media_path.as_deref();
    #[cfg(target_os = "macos")]
    let filename = title_path
        .and_then(platform::display_name_for_path)
        .unwrap_or_else(|| state.title.clone());
    #[cfg(not(target_os = "macos"))]
    let title_path = current_media_path.as_deref();
    #[cfg(not(target_os = "macos"))]
    let filename = title_path
        .and_then(Path::file_name)
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| state.title.clone());
    let window_title = if filename.is_empty() {
        if cfg!(target_os = "macos") {
            "Riina".to_owned()
        } else {
            "riina".to_owned()
        }
    } else if cfg!(target_os = "macos") {
        filename
    } else {
        format!("{filename} — riina")
    };
    app.set_window_title(window_title.into());
    app.set_screenshot_available(
        state
            .tracks
            .iter()
            .any(|track| track.kind == TrackKind::Video && track.is_primary_selected())
            && !matches!(state.phase, Phase::Loading | Phase::Stopped | Phase::Failed),
    );
    app.set_reveal_current_media_available(
        match crate::key_binding_preferences::current_media_location(&state) {
            crate::key_binding_preferences::CurrentMediaLocation::Local(_) => true,
            crate::key_binding_preferences::CurrentMediaLocation::NoMedia
            | crate::key_binding_preferences::CurrentMediaLocation::NonLocal => false,
        },
    );
    app.set_delete_current_media_available(matches!(
        crate::key_binding_preferences::current_media_location(&state),
        crate::key_binding_preferences::CurrentMediaLocation::Local(_)
    ));
    app.set_save_current_playlist_available(!state.playlist.is_empty());
    app.set_reflection_known(
        state.video_filters.is_some()
            && state.video.is_some()
            && !matches!(state.phase, Phase::Loading | Phase::Stopped | Phase::Failed),
    );
    app.set_video_mirrored(
        Axis::Horizontal
            .observed(state.video_filters.as_deref())
            .unwrap_or(false),
    );
    app.set_video_flipped(
        Axis::Vertical
            .observed(state.video_filters.as_deref())
            .unwrap_or(false),
    );
    let filters = state.video_filters.as_deref().unwrap_or_default();
    app.set_delogo_enabled(
        state.video.is_some()
            && state.video_filters.is_some()
            && !matches!(state.phase, Phase::Loading | Phase::Stopped | Phase::Failed),
    );
    app.set_delogo_active(
        filters
            .iter()
            .any(|filter| filter.label.as_deref() == Some(riina_core::delogo::FILTER_LABEL)),
    );
    let keep_stopped_feedback = app.get_video_osd_request_feedback();
    if displayed
        .as_ref()
        .is_none_or(|old| old.generation != state.generation)
        || matches!(state.phase, Phase::Loading | Phase::Failed)
        || (state.phase == Phase::Stopped && !keep_stopped_feedback)
        || (state.error.is_some() && !keep_stopped_feedback)
    {
        app.set_video_osd_visible(false);
        app.set_video_osd_request_feedback(false);
    }
    match last_seek.observe_with_suppression(&state, app.get_disable_osd_seek()) {
        Change::Hide => app.set_seek_osd_visible(false),
        Change::Show => app.invoke_show_seek_osd(),
        Change::Keep => {}
    }
    last_video.set_volume_limit(if app.get_volume_boost() { 200.0 } else { 100.0 });
    last_video.set_disable_speed(app.get_disable_osd_speed());
    last_video.set_disable_pause_resume(app.get_disable_osd_pause_resume());
    let video_message = last_video.observe(&state);
    if !app.get_osd_enabled() {
        last_video.clear_pending();
    } else if let Some(message) = video_message {
        app.set_video_osd_request_feedback(message.kind == crate::video_osd::Kind::Aspect);
        app.set_video_osd_text(message.text.into());
        app.set_video_osd_progress(message.progress.unwrap_or(-1.0));
        app.invoke_show_video_osd();
    }
    match last_screenshot.observe(&state) {
        crate::screenshot_osd::Change::Hide => {
            clear_screenshot_preview(app);
        }
        crate::screenshot_osd::Change::Show(path)
            if app.get_osd_enabled() && !app.get_crop_editing() =>
        {
            if !app.get_screenshot_show_preview() {
                app.set_video_osd_request_feedback(false);
                app.set_video_osd_text("Screenshot Captured".into());
                app.set_video_osd_progress(-1.0);
                app.invoke_show_video_osd();
            } else {
                match slint::Image::load_from_path(std::path::Path::new(&path)) {
                    Ok(image) => {
                        let size = image.size();
                        let (width, height) =
                            crate::screenshot_osd::preview_size(size.width, size.height);
                        app.set_screenshot_preview_image(image);
                        app.set_screenshot_preview_width(width);
                        app.set_screenshot_preview_height(height);
                        app.set_screenshot_preview_path(if app.get_screenshot_save_to_file() {
                            path.into()
                        } else {
                            "".into()
                        });
                        app.invoke_show_screenshot_preview();
                    }
                    Err(_) => {
                        // Saving succeeded even if the preview decoder cannot open
                        // this format; preserve reference normal-message fallback.
                        app.set_video_osd_request_feedback(false);
                        app.set_video_osd_text("Screenshot Captured".into());
                        app.set_video_osd_progress(-1.0);
                        app.invoke_show_video_osd();
                    }
                }
            }
        }
        _ => {}
    }
    if app.get_crop_editing() {
        clear_screenshot_preview(app);
    }
    app.set_seek_position_label(seek_osd::clock(state.position).into());
    app.set_seek_duration_label(seek_osd::clock(state.duration).into());
    app.set_playlist_shuffled(state.playlist_shuffled);
    if should_fallback_removed_audio_device(displayed.as_ref(), &state)
        && let Some(audio_device_player) = resources.audio_device_player
        && let Err(error) = audio_device_player.send(Command::AudioDevice("auto".into()))
    {
        app.set_status(format!("Could not restore automatic audio output: {error}").into());
    }
    // Preserve row identity across clock ticks so focus and accessibility indexes
    // are not destroyed by unrelated time-pos notifications.
    if displayed.as_ref().is_none_or(|old| {
        old.tracks != state.tracks
            || old.chapters != state.chapters
            || old.playlist != state.playlist
            || old.audio_devices != state.audio_devices
            || old.audio_device != state.audio_device
            || old.audio_output != state.audio_output
            || current_chapter(old) != current_chapter(&state)
    }) {
        update_catalog(app, &state);
        *displayed = Some(state.clone());
    }
    let first_loaded = (!app.get_loaded() || previous_generation != Some(state.generation))
        && matches!(
            state.phase,
            Phase::Playing | Phase::Paused | Phase::Buffering | Phase::Ended
        );
    if previous_generation != Some(state.generation)
        || matches!(state.phase, Phase::Loading | Phase::Stopped | Phase::Failed)
    {
        // Do not leave the prior media's resize commands enabled while a
        // replacement is loading or after the current item has stopped.
        app.set_video_window_width(0.0);
        app.set_video_window_height(0.0);
        app.set_video_window_rotation(0);
    }
    app.set_loaded(!matches!(
        state.phase,
        Phase::Stopped | Phase::Failed | Phase::Loading
    ));
    app.set_paused(state.paused);
    app.set_playing(state.phase == Phase::Playing);
    if matches!(state.phase, Phase::Paused | Phase::Ended | Phase::Failed) {
        app.set_controls_visible(true);
    }
    let mut resize_for_media = false;
    if let Some((width, height, rotation)) = window_video_geometry(&state) {
        let geometry_was_missing =
            app.get_video_window_width() <= 0.0 || app.get_video_window_height() <= 0.0;
        app.set_video_window_width(width);
        app.set_video_window_height(height);
        app.set_video_window_rotation(rotation);
        if media_geometry_requires_resize(previous_generation, geometry_was_missing, &state) {
            // mpv can publish the loaded phase before its first usable video
            // dimensions. Retry initial sizing when those dimensions arrive.
            resize_for_media = true;
        }
    }
    if first_loaded {
        app.invoke_activity();
        if let Some(entry) = state.playlist.iter().find(|entry| entry.current) {
            let title = entry
                .title
                .as_deref()
                .filter(|title| !title.trim().is_empty())
                .map(str::to_owned)
                .unwrap_or_else(|| {
                    PathBuf::from(&entry.filename).file_name().map_or_else(
                        || entry.filename.clone(),
                        |name| name.to_string_lossy().into_owned(),
                    )
                });
            app.invoke_media_opened(entry.filename.clone().into(), title.into());
        }
        if let Some(message) = crate::video_osd::file_start_message(
            &state.title,
            app.get_osd_enabled(),
            app.get_disable_osd_file_start(),
        ) {
            app.set_video_osd_request_feedback(false);
            app.set_video_osd_text(message.text.into());
            app.set_video_osd_progress(message.progress.unwrap_or(-1.0));
            app.invoke_show_video_osd();
        }
    }
    app.set_seekable(state.seekable && state.duration.is_some());
    app.set_position(state.position.unwrap_or(0.0) as f32);
    app.set_duration(state.duration.unwrap_or(0.0) as f32);
    app.set_ab_loop_a(state.ab_loop_a.unwrap_or_default() as f32);
    app.set_ab_loop_a_known(state.ab_loop_a.is_some());
    app.set_ab_loop_b(state.ab_loop_b.unwrap_or_default() as f32);
    app.set_ab_loop_b_known(state.ab_loop_b.is_some());
    app.set_volume(state.volume as f32);
    app.set_volume_menu_title(
        format!(
            "Volume: {}{}",
            crate::localized_number::format_delay(state.volume),
            if state.muted { " (Muted)" } else { "" }
        )
        .into(),
    );
    app.set_muted(state.muted);
    app.set_speed_value(state.speed as f32);
    app.set_speed_slider_position(riina_core::speed_controls::slider_position(state.speed) as f32);
    app.set_speed_edit_value(riina_core::speed_controls::text(state.speed).into());
    app.set_video_aspect_known(state.video_aspect_override.is_some());
    let display_aspect = state.display_video_parameters().and_then(|video| {
        riina_core::window_aspect::display_aspect_ratio(
            video.w,
            video.h,
            video.dw,
            video.dh,
            state.video_rotation.or(video.rotate),
            state.video_aspect_override,
        )
    });
    app.set_video_display_aspect_ratio(display_aspect.unwrap_or(0.0) as f32);
    let is_audio = state.audio_status() == riina_core::music_mode::AudioStatus::Audio;
    let album_art = state
        .tracks
        .iter()
        .find(|track| track.kind == riina_core::TrackKind::Video && track.albumart);
    let has_visible_subtitles = state.tracks.iter().any(|track| {
        track.kind == riina_core::TrackKind::Sub
            && ((state.subtitle_visible && track.is_primary_selected())
                || (state.secondary_subtitle_visible.unwrap_or(false)
                    && track.selected
                    && track.main_selection == Some(1)))
    });
    app.set_music_mode_is_audio(is_audio);
    app.set_music_mode_has_album_art(album_art.is_some());
    app.set_music_mode_has_visible_subtitles(has_visible_subtitles);
    let artwork_aspect = if is_audio {
        album_art
            .and_then(|track| track.width.zip(track.height))
            .filter(|(width, height)| *width > 0 && *height > 0)
            .map(|(width, height)| width as f64 / height as f64)
    } else {
        display_aspect
    };
    app.set_music_mode_artwork_aspect_ratio(artwork_aspect.unwrap_or(0.0) as f32);
    app.set_video_aspect_index(
        state
            .video_aspect
            .preset_index()
            .map_or(-1, |index| index as i32),
    );
    app.set_video_aspect_menu_index(
        state
            .video_aspect
            .menu_index()
            .map_or(-1, |index| index as i32),
    );
    app.set_video_rotation_known(state.video_rotation.is_some());
    app.set_deinterlace_known(state.deinterlace.is_some() && state.video.is_some());
    app.set_deinterlace_enabled(state.deinterlace.unwrap_or(false));
    app.set_video_crop_known(
        state.video_filters.is_some()
            && state
                .video
                .as_ref()
                .is_some_and(|video| video.w > 0 && video.h > 0),
    );
    app.set_video_crop_index(
        riina_core::crop::observed_index(&state).map_or(-1, |index| index as i32),
    );
    app.set_video_crop_menu_index(
        riina_core::crop::observed_menu_index(&state).map_or(-1, |index| index as i32),
    );
    app.set_video_equalizer_values(slint::ModelRc::new(slint::VecModel::from(
        state
            .video_equalizer
            .map(|value| value.unwrap_or(0) as i32)
            .to_vec(),
    )));
    app.set_video_equalizer_known(slint::ModelRc::new(slint::VecModel::from(
        state.video_equalizer.map(|value| value.is_some()).to_vec(),
    )));
    if let Some(gains) = state.audio_equalizer {
        app.set_audio_equalizer_values(slint::ModelRc::new(slint::VecModel::from(
            gains.map(|gain| gain as f32).to_vec(),
        )));
    }
    app.set_hardware_decoding_known(state.hardware_decoding.is_some());
    app.set_hardware_decoding_enabled(
        state
            .hardware_decoding
            .as_deref()
            .is_some_and(|mode| mode != "no"),
    );
    app.set_video_rotation_index(
        state
            .video_rotation
            .filter(|value| matches!(value, 0 | 90 | 180 | 270))
            .map_or(-1, |value| (value / 90) as i32),
    );
    app.set_audio_delay(state.audio_delay as f32);
    app.set_audio_delay_text(crate::localized_number::format_delay(state.audio_delay).into());
    app.set_subtitle_delay(state.subtitle_delay as f32);
    app.set_subtitle_delay_text(crate::localized_number::format_delay(state.subtitle_delay).into());
    app.set_subtitle_scale_known(state.subtitle_scale.is_some());
    app.set_subtitle_font_size_known(state.subtitle_font_size.is_some());
    app.set_subtitle_font_known(state.subtitle_font.is_some());
    if let Some(font) = &state.subtitle_font {
        app.set_subtitle_font(font.clone().into());
    }
    app.set_subtitle_font_size(state.subtitle_font_size.unwrap_or(0.0) as f32);
    if let Some(size) = state.subtitle_font_size {
        app.set_subtitle_font_size_preference(size as f32);
    }
    app.set_subtitle_outline_size_known(state.subtitle_outline_size.is_some());
    app.set_subtitle_outline_size(state.subtitle_outline_size.unwrap_or(0.0) as f32);
    let ui_color = |color: Option<riina_core::subtitle_color::SubtitleColor>| {
        let [r, g, b, a] = color
            .unwrap_or(riina_core::subtitle_color::SubtitleColor::CLEAR)
            .rgba();
        slint::Color::from_argb_u8(
            (a * 255.0).round() as u8,
            (r * 255.0).round() as u8,
            (g * 255.0).round() as u8,
            (b * 255.0).round() as u8,
        )
    };
    app.set_subtitle_text_color_known(state.subtitle_text_color.is_some());
    app.set_subtitle_border_color_known(state.subtitle_border_color.is_some());
    app.set_subtitle_shadow_color_known(state.subtitle_shadow_color.is_some());
    app.set_subtitle_text_color(ui_color(state.subtitle_text_color));
    app.set_subtitle_border_color(ui_color(state.subtitle_border_color));
    app.set_subtitle_shadow_color(ui_color(state.subtitle_shadow_color));
    let subtitle_style = state.subtitle_style_availability();
    app.set_subtitle_text_style_available(subtitle_style.text_controls);
    app.set_subtitle_ass_style_warning(subtitle_style.ass_warning);
    let scale = state.subtitle_scale.unwrap_or(1.0).clamp(0.1, 10.0);
    app.set_subtitle_scale_slider(
        (if scale >= 1.0 {
            scale - 1.0
        } else {
            1.0 - scale.recip()
        }) as f32,
    );
    app.set_subtitle_position_known(state.subtitle_position.is_some());
    app.set_subtitle_position(state.subtitle_position.unwrap_or(0.0) as f32);
    app.set_secondary_subtitle_delay_known(state.secondary_subtitle_delay.is_some());
    app.set_secondary_subtitle_delay(state.secondary_subtitle_delay.unwrap_or(0.0) as f32);
    app.set_secondary_subtitle_delay_text(
        crate::localized_number::format_delay(state.secondary_subtitle_delay.unwrap_or(0.0)).into(),
    );
    app.set_secondary_subtitle_position_known(state.secondary_subtitle_position.is_some());
    app.set_secondary_subtitle_position(state.secondary_subtitle_position.unwrap_or(0.0) as f32);
    app.set_subtitle_visible(state.subtitle_visible);
    app.set_secondary_subtitle_visibility_known(state.secondary_subtitle_visible.is_some());
    app.set_secondary_subtitle_visible(state.secondary_subtitle_visible.unwrap_or(false));
    app.set_loop_label(
        match state.loop_mode() {
            LoopMode::Off => "Off",
            LoopMode::File => "File",
            LoopMode::Playlist => "Playlist",
        }
        .into(),
    );
    let hdr_source_detected = state
        .video
        .as_ref()
        .or(state.video_target.as_ref())
        .or(state.video_output.as_ref())
        .is_some_and(riina_core::VideoParameters::is_hdr);
    let hdr_source_changed = app.get_hdr_source_detected() != hdr_source_detected;
    app.set_hdr_source_detected(hdr_source_detected);
    if first_loaded || hdr_source_changed {
        let enable_hdr = app.get_default_hdr_mode_enabled()
            && app.get_hdr_source_detected()
            && app.get_hdr_display_active();
        app.set_hdr_enabled(enable_hdr);
        if let Some(player) = resources.hdr_player
            && let Err(error) = player.send(Command::SetHdrEnabledWithIcc {
                enabled: enable_hdr,
                load_icc_profile: app.get_load_icc_profile(),
            })
        {
            app.set_status(format!("Could not apply the HDR output preference: {error}").into());
        }
    }
    app.set_media_title(state.title.into());
    app.set_music_artist(state.artist.unwrap_or_default().into());
    app.set_music_album(state.album.unwrap_or_default().into());
    app.set_position_label(time_label(state.position).into());
    app.set_duration_label(time_label(state.duration).into());
    // Playback state belongs to controls and transient OSD, not permanent text
    // over the video. Keep failures visible while ordinary clock ticks stay quiet.
    app.set_status(state.error.unwrap_or_default().into());
    resize_for_media
}
