#[cfg(target_os = "windows")]
use crate::LiveTextRegion;
use crate::seek_osd::{self, Change, SeekOsd};
use crate::{
    CatalogRow, PlayerWindow, RecentMediaRow, WelcomeWindow, media_open, platform,
    ui_dialogs::{self, Purpose},
};
use riina_core::{Command, LoopMode, Phase, Playback, Preferences, TrackKind, time_label};
use riina_mpv::{Player, video::VideoSurface};
use slint::{ComponentHandle, Model, winit_030::WinitWindowAccessor};
use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

fn clear_screenshot_preview(app: &PlayerWindow) {
    app.set_screenshot_preview_visible(false);
    app.set_screenshot_preview_path("".into());
    app.set_screenshot_preview_image(slint::Image::default());
}

fn dispatch_control_mouse_action(
    app: &PlayerWindow,
    player: &riina_mpv::CommandSender,
    index: i32,
) {
    use riina_core::control_preferences::MouseEffect;

    let Some(action) = riina_core::control_preferences::MouseAction::from_index(index) else {
        app.set_status("Invalid mouse action preference".into());
        return;
    };
    if app.get_crop_editing() || app.get_dialog_open() {
        return;
    }
    let effect = action.effect(app.get_paused());
    let result = match effect {
        MouseEffect::None => return,
        MouseEffect::Fullscreen => {
            app.invoke_toggle_fullscreen();
            return;
        }
        MouseEffect::SetPaused(paused) => {
            if !app.get_loaded() {
                return;
            }
            player.set_paused(paused)
        }
        MouseEffect::HideControls => {
            app.set_controls_visible(false);
            return;
        }
        MouseEffect::PictureInPicture => {
            app.set_status("Picture-in-Picture is not available yet".into());
            return;
        }
        MouseEffect::CycleAbLoop => {
            if !app.get_loaded() {
                return;
            }
            player.send(Command::CycleAbLoop)
        }
        MouseEffect::ResetSpeed => {
            if !app.get_loaded() {
                return;
            }
            player.send(Command::Speed(1.0))
        }
    };
    if let Err(error) = result {
        app.set_status(error.into());
    }
}

fn set_native_video_host_visible(
    host: Option<&slint::Weak<crate::NativeVideoHost>>,
    visible: bool,
) {
    let Some(host) = host.and_then(slint::Weak::upgrade) else {
        return;
    };
    // The native presentation retains this Winit handle for mpv's HWND
    // lifetime. Toggle the OS surface directly so Slint does not keep it shown.
    host.window()
        .with_winit_window(|window| window.set_visible(visible));
}

fn configured_osd_timeout(app: &PlayerWindow) -> Duration {
    Duration::try_from_secs_f64(f64::from(app.get_osd_timeout_secs())).unwrap_or_default()
}

fn refresh_video_menu(app: &PlayerWindow) {
    // Native check items toggle before dispatch. A no-op or rejected command
    // cannot dirty the observed selection, so renew the model identity to make
    // Slint rebuild the menu from confirmed state rather than that local toggle.
    app.set_video_crop_menu_labels(slint::ModelRc::new(slint::VecModel::from(
        riina_core::crop::MENU_LABELS
            .map(slint::SharedString::from)
            .to_vec(),
    )));
    app.set_video_aspect_menu_labels(slint::ModelRc::new(slint::VecModel::from(
        riina_core::video_controls::ASPECT_MENU_PRESETS
            .map(slint::SharedString::from)
            .to_vec(),
    )));
}

fn recent_media_rows(media: &[riina_core::RecentMedia]) -> Vec<RecentMediaRow> {
    media
        .iter()
        .map(|item| {
            let title = if item.title.trim().is_empty() {
                PathBuf::from(&item.path).file_name().map_or_else(
                    || item.path.clone(),
                    |name| name.to_string_lossy().into_owned(),
                )
            } else {
                item.title.clone()
            };
            RecentMediaRow {
                title: title.into(),
                position_label: time_label(Some(item.position)).into(),
            }
        })
        .collect()
}

fn resume_media_rows(
    media: &[riina_core::RecentMedia],
    resume_last_position: bool,
) -> Vec<RecentMediaRow> {
    if resume_last_position {
        recent_media_rows(media.get(..1).unwrap_or_default())
    } else {
        Vec::new()
    }
}

fn recent_document_rows(
    media: &[riina_core::RecentMedia],
    resume_last_position: bool,
) -> Vec<RecentMediaRow> {
    // IINA removes the last-played item from the document table only while its
    // separate Resume action is enabled.
    let offset = if resume_last_position { 1 } else { 0 };
    recent_media_rows(media.get(offset..).unwrap_or_default())
}

#[cfg(target_os = "windows")]
fn install_live_text(
    app: &PlayerWindow,
    sender: riina_mpv::CommandSender,
    latest: Arc<std::sync::Mutex<Playback>>,
) -> Rc<slint::Timer> {
    let requested_frame = Rc::new(Cell::new(None::<(u64, u64)>));
    let recognized_capture = Rc::new(Cell::new(None::<(u64, u64)>));
    let recognition_running = Arc::new(AtomicBool::new(false));
    let timer = Rc::new(slint::Timer::default());
    let weak = app.as_weak();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(300),
        move || {
            let Some(app) = weak.upgrade() else { return };
            let state = latest.lock().map(|state| state.clone()).unwrap_or_default();
            if !app.get_live_text_enabled()
                || !app.get_live_text_available()
                || !app.get_loaded()
                || state.phase != Phase::Paused
            {
                requested_frame.set(None);
                recognized_capture.set(None);
                app.set_live_text_regions(slint::ModelRc::new(slint::VecModel::from(Vec::new())));
                return;
            }

            // One screenshot request per paused source generation prevents polling
            // from queueing duplicate frames while mpv completes the command.
            let frame_key = (
                state.generation,
                state.position.unwrap_or_default().to_bits(),
            );
            if requested_frame.get() != Some(frame_key) {
                match sender.send(Command::CaptureFrame) {
                    Ok(()) => requested_frame.set(Some(frame_key)),
                    Err(error) => app.set_status(
                        format!("Could not capture frame for text recognition: {error}").into(),
                    ),
                }
            }
            let Some(frame) = state.captured_frame.as_deref().cloned() else {
                return;
            };
            let key = (state.generation, state.completed_frame_captures);
            if recognized_capture.get() == Some(key) || recognition_running.load(Ordering::Acquire)
            {
                return;
            }
            recognized_capture.set(Some(key));
            recognition_running.store(true, Ordering::Release);
            let latest_result = latest.clone();
            let weak_result = app.as_weak();
            let running = recognition_running.clone();
            let width = frame.width as f32;
            let height = frame.height as f32;
            if let Err(error) = platform::text_recognition::recognize(frame, move |result| {
                let _ = slint::invoke_from_event_loop(move || {
                    running.store(false, Ordering::Release);
                    let Some(app) = weak_result.upgrade() else {
                        return;
                    };
                    let current = latest_result
                        .lock()
                        .map(|state| state.clone())
                        .unwrap_or_default();
                    if current.generation != key.0
                        || current.phase != Phase::Paused
                        || current.completed_frame_captures != key.1
                        || !app.get_live_text_enabled()
                    {
                        return;
                    }
                    match result {
                        Ok(regions) => {
                            app.set_live_text_regions(slint::ModelRc::new(slint::VecModel::from(
                                regions
                                    .into_iter()
                                    .map(|region| LiveTextRegion {
                                        text: region.text.into(),
                                        x: region.x / width,
                                        y: region.y / height,
                                        width: region.width / width,
                                        height: region.height / height,
                                    })
                                    .collect::<Vec<_>>(),
                            )))
                        }
                        Err(error) => {
                            app.set_status(format!("Text recognition failed: {error}").into())
                        }
                    }
                });
            }) {
                recognition_running.store(false, Ordering::Release);
                app.set_status(format!("Could not start text recognition: {error}").into());
            }
        },
    );
    timer
}

fn refresh_welcome_history(
    welcome: &WelcomeWindow,
    media: &[riina_core::RecentMedia],
    resume_last_position: bool,
) {
    welcome.set_recent_media(slint::ModelRc::new(slint::VecModel::from(
        resume_media_rows(media, resume_last_position),
    )));
    welcome.set_recent_documents(slint::ModelRc::new(slint::VecModel::from(
        recent_document_rows(media, resume_last_position),
    )));
    welcome.set_recent_document_offset(if resume_last_position { 1 } else { 0 });
    welcome.set_selection_index(if resume_last_position || media.is_empty() {
        -1
    } else {
        0
    });
}

fn resume_engine_options(
    watch_later_directory: &std::path::Path,
    resume_last_position: bool,
    subtitle_auto_load: &riina_core::subtitle_controls::AutoLoadPreferences,
    network: &riina_core::network_preferences::Connection,
    network_cache: &riina_core::network_preferences::Cache,
    network_online_videos: &riina_core::network_preferences::OnlineVideos,
) -> Result<Vec<(String, String)>, String> {
    let enabled = if resume_last_position { "yes" } else { "no" }.to_owned();
    let mut options = vec![
        (
            "watch-later-directory".into(),
            watch_later_directory.to_string_lossy().into_owned(),
        ),
        ("save-position-on-quit".into(), enabled.clone()),
        ("resume-playback".into(), enabled),
    ];
    options.push((
        "sub-auto".into(),
        subtitle_auto_load.mpv_matching_mode().into(),
    ));
    options.push((
        "sub-file-paths".into(),
        subtitle_auto_load.mpv_search_paths().unwrap_or_default(),
    ));
    options.extend(network.engine_options()?);
    options.extend(network_cache.engine_options()?);
    options.extend(network_online_videos.engine_options()?);
    Ok(options)
}

fn update_catalog(app: &PlayerWindow, state: &Playback) {
    let languages = crate::language_names::LanguageNames::current();
    for kind in [TrackKind::Video, TrackKind::Audio, TrackKind::Sub] {
        let tracks: Vec<_> = state
            .tracks
            .iter()
            .filter(|track| track.kind == kind)
            .collect();
        let mut rows = vec![CatalogRow {
            token: "no".into(),
            label: "<None>".into(),
            path: "".into(),
            language: "".into(),
            selected: !tracks.iter().any(|track| track.is_primary_selected()),
        }];
        rows.extend(tracks.into_iter().map(|track| CatalogRow {
            token: track.id.to_string().into(),
            label: track.display_name().into(),
            path: "".into(),
            language: languages.display_name(track.lang.as_deref()).into(),
            selected: track.is_primary_selected(),
        }));
        let model = slint::ModelRc::new(slint::VecModel::from(rows));
        match kind {
            TrackKind::Video => app.set_video_tracks(model),
            TrackKind::Audio => app.set_audio_tracks(model),
            TrackKind::Sub => app.set_subtitle_tracks(model),
        }
    }
    let subtitle_tracks: Vec<_> = state
        .tracks
        .iter()
        .filter(|t| t.kind == TrackKind::Sub)
        .collect();
    let secondary_selected =
        |track: &&riina_core::Track| track.selected && track.main_selection == Some(1);
    let mut secondary_rows = vec![CatalogRow {
        token: "no".into(),
        label: "<None>".into(),
        path: "".into(),
        language: "".into(),
        selected: !subtitle_tracks.iter().any(secondary_selected),
    }];
    secondary_rows.extend(subtitle_tracks.into_iter().map(|track| CatalogRow {
        token: track.id.to_string().into(),
        label: track.display_name().into(),
        path: "".into(),
        language: languages.display_name(track.lang.as_deref()).into(),
        selected: track.selected && track.main_selection == Some(1),
    }));
    app.set_secondary_subtitle_tracks(slint::ModelRc::new(slint::VecModel::from(secondary_rows)));
    app.set_chapters(slint::ModelRc::new(slint::VecModel::from(
        state
            .chapters
            .iter()
            .enumerate()
            .map(|(index, chapter)| CatalogRow {
                token: index.to_string().into(),
                path: "".into(),
                language: "".into(),
                label: format!(
                    "{}  {}",
                    time_label(Some(chapter.time)),
                    chapter
                        .title
                        .clone()
                        .unwrap_or_else(|| format!("Chapter {}", index + 1))
                )
                .into(),
                selected: state.position.is_some_and(|position| {
                    position >= chapter.time
                        && state
                            .chapters
                            .get(index + 1)
                            .is_none_or(|next| position < next.time)
                }),
            })
            .collect::<Vec<_>>(),
    )));
    // IINA omits the first chapter boundary because it coincides with the bar origin.
    app.set_chapter_times(slint::ModelRc::new(slint::VecModel::from(
        state
            .chapters
            .iter()
            .skip(1)
            .map(|chapter| chapter.time as f32)
            .filter(|time| time.is_finite() && *time >= 0.0)
            .collect::<Vec<_>>(),
    )));
    app.set_playlist(slint::ModelRc::new(slint::VecModel::from(
        state
            .playlist
            .iter()
            .map(|entry| CatalogRow {
                token: entry.id.to_string().into(),
                path: entry.filename.clone().into(),
                language: "".into(),
                label: entry
                    .title
                    .clone()
                    .unwrap_or_else(|| {
                        if entry.filename.contains("://") {
                            entry.filename.clone()
                        } else {
                            PathBuf::from(&entry.filename).file_name().map_or_else(
                                || entry.filename.clone(),
                                |name| name.to_string_lossy().into_owned(),
                            )
                        }
                    })
                    .into(),
                selected: entry.current,
            })
            .collect::<Vec<_>>(),
    )));
    if !state
        .playlist
        .iter()
        .any(|entry| entry.id.to_string() == app.get_selected_playlist_token().as_str())
    {
        app.set_selected_playlist_token("".into());
    }
}

fn current_chapter(state: &Playback) -> Option<usize> {
    state.position.and_then(|position| {
        state
            .chapters
            .iter()
            .rposition(|chapter| chapter.time <= position)
    })
}

fn update_ui(
    app: &PlayerWindow,
    state: Playback,
    displayed: &mut Option<Playback>,
    last_seek: &mut SeekOsd,
    last_video: &mut crate::video_osd::VideoOsd,
    last_screenshot: &mut crate::screenshot_osd::ScreenshotOsd,
) {
    use riina_core::video_reflection::Axis;
    app.set_screenshot_available(
        state
            .tracks
            .iter()
            .any(|track| track.kind == TrackKind::Video && track.is_primary_selected())
            && !matches!(state.phase, Phase::Loading | Phase::Stopped | Phase::Failed),
    );
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
    if displayed
        .as_ref()
        .is_none_or(|old| old.generation != state.generation)
        || matches!(state.phase, Phase::Loading | Phase::Stopped | Phase::Failed)
        || (state.error.is_some() && !app.get_video_osd_request_feedback())
    {
        app.set_video_osd_visible(false);
    }
    match last_seek.observe_with_suppression(&state, app.get_disable_osd_seek()) {
        Change::Hide => app.set_seek_osd_visible(false),
        Change::Show => app.invoke_show_seek_osd(),
        Change::Keep => {}
    }
    last_video.set_disable_speed(app.get_disable_osd_speed());
    last_video.set_disable_pause_resume(app.get_disable_osd_pause_resume());
    if let Some(message) = last_video.observe(&state) {
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
    // Preserve row identity across clock ticks so focus and accessibility indexes
    // are not destroyed by unrelated time-pos notifications.
    if displayed.as_ref().is_none_or(|old| {
        old.tracks != state.tracks
            || old.chapters != state.chapters
            || old.playlist != state.playlist
            || current_chapter(old) != current_chapter(&state)
    }) {
        update_catalog(app, &state);
        *displayed = Some(state.clone());
    }
    let first_loaded = !app.get_loaded()
        && matches!(
            state.phase,
            Phase::Playing | Phase::Paused | Phase::Buffering | Phase::Ended
        );
    app.set_loaded(!matches!(
        state.phase,
        Phase::Stopped | Phase::Failed | Phase::Loading
    ));
    app.set_paused(state.paused);
    app.set_playing(state.phase == Phase::Playing);
    if matches!(state.phase, Phase::Paused | Phase::Ended | Phase::Failed) {
        app.set_controls_visible(true);
    }
    if first_loaded {
        if let Some(video) = state
            .video_target
            .as_ref()
            .or(state.video.as_ref())
            .or(state.video_output.as_ref())
        {
            app.set_video_window_width(video.dw.filter(|size| *size > 0).unwrap_or(video.w) as f32);
            app.set_video_window_height(video.dh.filter(|size| *size > 0).unwrap_or(video.h) as f32);
            app.set_video_window_rotation(state.video_rotation.or(video.rotate).unwrap_or(0) as i32);
            app.invoke_resize_window_for_media();
        }
        app.invoke_activity();
        app.invoke_media_opened();
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
    app.set_muted(state.muted);
    app.set_speed_value(state.speed as f32);
    app.set_speed_slider_position(riina_core::speed_controls::slider_position(state.speed) as f32);
    app.set_speed_edit_value(riina_core::speed_controls::text(state.speed).into());
    app.set_video_aspect_known(state.video_aspect_override.is_some());
    let display_aspect = state
        .video_target
        .as_ref()
        .or(state.video.as_ref())
        .or(state.video_output.as_ref())
        .and_then(|video| {
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
    app.set_subtitle_delay(state.subtitle_delay as f32);
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
    app.set_hdr_source_detected(
        state
            .video
            .as_ref()
            .or(state.video_target.as_ref())
            .or(state.video_output.as_ref())
            .is_some_and(riina_core::VideoParameters::is_hdr),
    );
    app.set_media_title(state.title.into());
    app.set_music_artist(state.artist.unwrap_or_default().into());
    app.set_music_album(state.album.unwrap_or_default().into());
    app.set_position_label(time_label(state.position).into());
    app.set_duration_label(time_label(state.duration).into());
    // Playback state belongs to controls and transient OSD, not permanent text
    // over the video. Keep failures visible while ordinary clock ticks stay quiet.
    app.set_status(state.error.unwrap_or_default().into());
}

pub(crate) struct UiSession {
    // Native callbacks retain senders only. Field order keeps the target alive
    // until this sole native player owner has joined its decoder worker.
    pub(crate) player: Option<Player>,
    _presentation: Option<platform::NativePresentation>,
    _hdr_display_timer: Option<Rc<slint::Timer>>,
    _live_text_timer: Option<Rc<slint::Timer>>,
    // The repeated sampler is held for the same lifetime as the player window.
    _appearance_timer: Rc<slint::Timer>,
    prefs_path: PathBuf,
    prefs: Preferences,
    recent_media: Rc<RefCell<Vec<riina_core::RecentMedia>>>,
    history_cleared: Rc<Cell<bool>>,
    latest_playback: Arc<std::sync::Mutex<Playback>>,
}

enum PendingWelcomeOpen {
    Url(String),
    Recent(riina_core::RecentMedia, bool),
}

fn dispatch_welcome_open(
    app: &PlayerWindow,
    request: PendingWelcomeOpen,
    sender: &riina_mpv::CommandSender,
) {
    match request {
        PendingWelcomeOpen::Url(value) => media_open::url(app, &value, sender.clone()),
        PendingWelcomeOpen::Recent(media, resume) => {
            media_open::open_recent(app, media, resume, sender.clone());
        }
    }
}

impl UiSession {
    pub(crate) fn save(&self, app: &PlayerWindow) -> Result<(), Box<dyn std::error::Error>> {
        let mut prefs = Preferences {
            volume: (app.get_volume() as f64).min(if app.get_volume_boost() {
                200.0
            } else {
                100.0
            }),
            volume_boost: app.get_volume_boost(),
            show_chapter_pos: app.get_show_chapter_pos(),
            picture_in_picture: crate::pip_preferences::current_player(app)
                .map_err(std::io::Error::other)?,
            action_after_launch: riina_core::ActionAfterLaunch::from_index(
                app.get_launch_action_index(),
            )
            .ok_or_else(|| std::io::Error::other("Invalid launch action selection"))?,
            hdr_enabled: app.get_hdr_enabled(),
            live_text_enabled: app.get_live_text_enabled(),
            always_float_on_top: app.get_always_float_on_top(),
            always_show_on_top_icon: app.get_always_show_on_top_icon(),
            theme_material: Some(
                riina_core::ThemeMaterial::from_index(app.get_appearance_mode())
                    .ok_or_else(|| std::io::Error::other("Invalid appearance selection"))?,
            ),
            pause_when_open: app.get_pause_when_open(),
            fullscreen_when_open: app.get_fullscreen_when_open(),
            resume_last_position: app.get_resume_last_position(),
            enable_osd: app.get_osd_enabled(),
            disable_osd_file_start: app.get_disable_osd_file_start(),
            disable_osd_pause_resume: app.get_disable_osd_pause_resume(),
            disable_osd_speed: app.get_disable_osd_speed(),
            disable_osd_seek: app.get_disable_osd_seek(),
            osd_timeout_secs: f64::from(app.get_osd_timeout_secs()),
            osd_text_size: f64::from(app.get_osd_text_size()),
            display_time_and_battery_in_fullscreen: app
                .get_display_time_and_battery_in_fullscreen(),
            liquid_glass: crate::liquid_glass_preferences::current_player(app),
            leading_sidebar_width: app.get_leading_sidebar_width() as u32,
            trailing_sidebar_width: app.get_trailing_sidebar_width() as u32,
            settings_at_leading: app.get_settings_at_leading(),
            playlist_at_leading: app.get_playlist_at_leading(),
            edge_to_edge_video: app.get_edge_to_edge_video(),
            docked_controls: app.get_docked_controls(),
            unlock_window_aspect_ratio: app.get_unlock_window_aspect_ratio(),
            window_resize: crate::window_resize_preferences::from_player(app)
                .map_err(std::io::Error::other)?,
            compact_ui: app.get_compact_ui(),
            auto_switch_to_music_mode: app.get_auto_switch_to_music_mode(),
            music_mode_show_playlist: app.get_music_mode_show_playlist(),
            music_mode_show_album_art: app.get_music_mode_show_album_art(),
            osc_position: app.get_osc_position() as u8,
            hardware_decoder: crate::codec_preferences::hardware_decoder(
                app.get_hardware_decoder_index(),
            )
            .ok_or_else(|| std::io::Error::other("Invalid hardware decoder selection"))?,
            subtitle_ass_override: crate::subtitle_preferences::level(
                app.get_subtitle_ass_override_index(),
            )
            .ok_or_else(|| std::io::Error::other("Invalid ASS override selection"))?,
            secondary_subtitle_ass_override: crate::subtitle_preferences::level(
                app.get_secondary_subtitle_ass_override_index(),
            )
            .ok_or_else(|| std::io::Error::other("Invalid secondary ASS override selection"))?,
            subtitle_position: crate::subtitle_position_preferences::current_player(app)
                .map_err(std::io::Error::other)?,
            subtitle_other: crate::subtitle_other_preferences::current_player(app)
                .map_err(std::io::Error::other)?,
            subtitle_auto_load: crate::subtitle_auto_load_preferences::current_player(app)
                .map_err(std::io::Error::other)?,
            subtitle_font: app.get_subtitle_font().to_string(),
            subtitle_font_size: crate::subtitle_preferences::current_font_size(app),
            subtitle_text_style: crate::subtitle_preferences::current_style(app)
                .map_err(std::io::Error::other)?,
            screenshot: crate::screenshot_preferences::current(app)
                .map_err(std::io::Error::other)?,
            dark: app.get_dark_theme(),
            network_cache: crate::network_cache_preferences::current_player(app)
                .map_err(std::io::Error::other)?,
            network_online_videos: crate::network_ytdl_preferences::current_player(app)
                .map_err(std::io::Error::other)?,
            ..self.prefs.clone()
        };
        prefs.recent_media = self.recent_media.borrow().clone();
        // The Settings window persists audio engine preferences immediately;
        // preserve those newer values when the player writes its final state.
        if let Ok(text) = std::fs::read_to_string(&self.prefs_path)
            && let Ok(saved) = Preferences::parse(&text)
        {
            prefs.audio = saved.audio;
            prefs.advanced = saved.advanced;
        }
        if !self.history_cleared.get()
            && let Ok(state) = self.latest_playback.lock()
            && let Some(position) = state
                .position
                .filter(|value| value.is_finite() && *value >= 0.0)
            && !matches!(state.phase, Phase::Loading | Phase::Failed)
            && let Some(entry) = state.playlist.iter().find(|entry| entry.current)
        {
            let title = entry
                .title
                .as_ref()
                .filter(|title| !title.trim().is_empty())
                .cloned()
                .unwrap_or_else(|| {
                    if state.title.trim().is_empty() {
                        PathBuf::from(&entry.filename).file_name().map_or_else(
                            || entry.filename.clone(),
                            |name| name.to_string_lossy().into_owned(),
                        )
                    } else {
                        state.title.clone()
                    }
                });
            let position = if app.get_resume_last_position() {
                position
            } else {
                prefs
                    .recent_media
                    .iter()
                    .find(|item| item.path == entry.filename)
                    .map_or(0.0, |item| item.position)
            };
            prefs.remember_media(riina_core::RecentMedia {
                path: entry.filename.clone(),
                title,
                position,
            });
        }
        platform::save_preferences(&self.prefs_path, &serde_json::to_string_pretty(&prefs)?)?;
        Ok(())
    }
}

impl Drop for UiSession {
    fn drop(&mut self) {
        // Release session ownership before its presentation on every platform.
        // Native playback joins here; GL callbacks retain their player until the
        // renderer has released its surface during RenderingTeardown.
        self.player.take();
    }
}

pub(crate) fn action_after_launch()
-> Result<riina_core::ActionAfterLaunch, Box<dyn std::error::Error>> {
    let path = platform::preferences_path()?;
    let preferences = match std::fs::read_to_string(path) {
        Ok(text) => Preferences::parse(&text).map_err(std::io::Error::other)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Preferences::default(),
        Err(error) => return Err(error.into()),
    };
    Ok(preferences.action_after_launch)
}

pub(crate) fn configure_player(
    app: &PlayerWindow,
    native: Option<platform::NativePresentation>,
    media: Option<std::ffi::OsString>,
    welcome: Option<&WelcomeWindow>,
) -> Result<UiSession, Box<dyn std::error::Error>> {
    let prefs_path = platform::preferences_path()?;
    let prefs = match std::fs::read_to_string(&prefs_path) {
        Ok(text) => Preferences::parse(&text).map_err(std::io::Error::other)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Preferences::default(),
        Err(error) => return Err(error.into()),
    };
    app.set_live_text_enabled(prefs.live_text_enabled);
    app.set_live_text_regions(slint::ModelRc::new(slint::VecModel::from(Vec::new())));
    app.set_live_text_available(false);
    #[cfg(target_os = "windows")]
    {
        let weak = app.as_weak();
        let _ = platform::text_recognition::check_availability(move |available| {
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(app) = weak.upgrade() {
                    app.set_live_text_available(available);
                    if !available {
                        app.set_live_text_enabled(false);
                    }
                }
            });
        });
    }
    app.set_hdr_enabled(prefs.hdr_enabled);
    app.set_hdr_display_active(
        native
            .as_ref()
            .is_some_and(|presentation| presentation.hdr_display_active()),
    );
    let hdr_display_timer = native.as_ref().map(|presentation| {
        let timer = Rc::new(slint::Timer::default());
        let display = presentation.clone();
        let weak = app.as_weak();
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_secs(2),
            move || {
                if let Some(app) = weak.upgrade() {
                    app.set_hdr_display_active(display.hdr_display_active());
                }
            },
        );
        timer
    });
    let recent_media = Rc::new(RefCell::new(prefs.recent_media.clone()));
    let history_cleared = Rc::new(Cell::new(false));
    let watch_later_directory = prefs_path.with_file_name("watch-later");
    std::fs::create_dir_all(&watch_later_directory)?;
    let mut engine_options = resume_engine_options(
        &watch_later_directory,
        prefs.resume_last_position,
        &prefs.subtitle_auto_load,
        &prefs.network,
        &prefs.network_cache,
        &prefs.network_online_videos,
    )
    .map_err(std::io::Error::other)?;
    engine_options.extend(
        prefs
            .audio
            .engine_options()
            .map_err(std::io::Error::other)?,
    );
    engine_options.push(("vd-lavc-threads".into(), prefs.video_threads.to_string()));
    engine_options.extend(
        prefs
            .hdr_tone_mapping
            .engine_options()
            .map_err(std::io::Error::other)?,
    );
    if !prefs.hdr_enabled {
        engine_options.extend(riina_core::hdr_preferences::disabled_output_options());
    }
    engine_options.extend(
        crate::advanced_preferences::engine_options(&prefs.advanced, &prefs_path)
            .map_err(std::io::Error::other)?,
    );
    app.set_volume_boost(prefs.volume_boost);
    app.set_auto_switch_to_music_mode(prefs.auto_switch_to_music_mode);
    app.set_music_mode_show_playlist(prefs.music_mode_show_playlist);
    app.set_music_mode_show_album_art(prefs.music_mode_show_album_art);
    crate::control_preferences::install_player(app, &prefs.controls);
    crate::key_binding_preferences::install_player(app, &prefs.key_bindings);
    app.set_hardware_decoder_index(crate::codec_preferences::index(prefs.hardware_decoder));
    app.set_subtitle_ass_override_index(crate::subtitle_preferences::index(
        prefs.subtitle_ass_override,
    ));
    app.set_secondary_subtitle_ass_override_index(crate::subtitle_preferences::index(
        prefs.secondary_subtitle_ass_override,
    ));
    crate::subtitle_position_preferences::install_player(app, prefs.subtitle_position);
    crate::subtitle_other_preferences::install_player(app, &prefs.subtitle_other);
    crate::subtitle_auto_load_preferences::install_player(app, &prefs.subtitle_auto_load);
    crate::network_cache_preferences::install_player(app, &prefs.network_cache);
    crate::network_ytdl_preferences::install_player(app, &prefs.network_online_videos);
    app.set_subtitle_font(prefs.subtitle_font.clone().into());
    app.set_subtitle_font_size_preference(prefs.subtitle_font_size as f32);
    crate::subtitle_preferences::install_style(app, prefs.subtitle_text_style);
    if let Some(welcome) = welcome {
        welcome.set_app_version(env!("CARGO_PKG_VERSION").into());
        refresh_welcome_history(welcome, &prefs.recent_media, prefs.resume_last_position);
    }
    app.set_video_aspect_presets(slint::ModelRc::new(slint::VecModel::from(
        riina_core::video_controls::ASPECT_PRESETS
            .map(slint::SharedString::from)
            .to_vec(),
    )));
    app.set_osd_enabled(prefs.enable_osd);
    app.set_disable_osd_file_start(prefs.disable_osd_file_start);
    app.set_disable_osd_pause_resume(prefs.disable_osd_pause_resume);
    app.set_disable_osd_speed(prefs.disable_osd_speed);
    app.set_disable_osd_seek(prefs.disable_osd_seek);
    app.set_osd_timeout_secs(prefs.osd_timeout_secs as f32);
    app.set_osd_text_size(prefs.osd_text_size as f32);
    app.set_display_time_and_battery_in_fullscreen(prefs.display_time_and_battery_in_fullscreen);
    crate::screenshot_preferences::install(app, &prefs.screenshot);
    app.set_pause_when_open(prefs.pause_when_open);
    app.set_fullscreen_when_open(prefs.fullscreen_when_open);
    app.set_resume_last_position(prefs.resume_last_position);
    app.set_launch_action_index(prefs.action_after_launch.index());
    app.set_leading_sidebar_width(prefs.leading_sidebar_width as f32);
    app.set_trailing_sidebar_width(prefs.trailing_sidebar_width as f32);
    app.set_settings_at_leading(prefs.settings_at_leading);
    app.set_playlist_at_leading(prefs.playlist_at_leading);
    app.set_edge_to_edge_video(prefs.edge_to_edge_video);
    app.set_docked_controls(prefs.docked_controls);
    app.set_unlock_window_aspect_ratio(prefs.unlock_window_aspect_ratio);
    crate::window_resize_preferences::install_player(app, prefs.window_resize);
    app.set_compact_ui(prefs.compact_ui);
    app.set_always_float_on_top(prefs.always_float_on_top);
    app.set_always_show_on_top_icon(prefs.always_show_on_top_icon);
    crate::liquid_glass_preferences::install_player(app, prefs.liquid_glass);
    let appearance = prefs.effective_theme_material();
    app.set_appearance_mode(appearance.index());
    app.set_dark_theme(appearance.dark_for(prefs.dark));
    let appearance_timer = crate::appearance_preferences::connect(app);
    app.set_osc_position(i32::from(prefs.osc_position));
    app.set_show_chapter_pos(prefs.show_chapter_pos);
    crate::pip_preferences::install_player(app, prefs.picture_in_picture);
    crate::sidebar_preferences::connect(app, prefs_path.clone(), prefs.clone());
    crate::layout_preferences::connect(app, prefs_path.clone(), prefs.clone());
    refresh_video_menu(app);
    app.set_video_crop_presets(slint::ModelRc::new(slint::VecModel::from(
        riina_core::crop::LABELS
            .map(slint::SharedString::from)
            .to_vec(),
    )));
    app.set_primary_shortcut_is_meta(cfg!(target_os = "macos"));
    let app_weak = app.as_weak();
    // Only one pending state snapshot is retained; a decoder can produce updates
    // faster than the UI consumes them without growing the application event queue.
    // State and capture completions share one handoff. A newer completion must
    // never be cleaned up while an older snapshot is still being rendered.
    let latest = Arc::new(std::sync::Mutex::new((
        None,
        crate::screenshot_output::Completions::default(),
    )));
    let latest_playback = Arc::new(std::sync::Mutex::new(Playback::default()));
    let observed_playback = latest_playback.clone();
    let music_mode_state = Arc::new(std::sync::Mutex::new(riina_core::music_mode::State::new(
        prefs.auto_switch_to_music_mode,
    )));
    if native.is_none() {
        let original_size = Rc::new(Cell::new(None::<slint::PhysicalSize>));
        let saved_size = original_size.clone();
        let weak = app.as_weak();
        app.on_music_mode_changed(move || {
            if let Some(app) = weak.upgrade() {
                let window = app.window();
                if app.get_music_mode() {
                    if saved_size.get().is_none() {
                        saved_size.set(Some(window.size()));
                    }
                    let scale = window.scale_factor();
                    let height =
                        72.0 + if app.get_music_mode_show_album_art() {
                            300.0
                        } else {
                            0.0
                        } + if app.get_music_mode_show_playlist() {
                            300.0
                        } else {
                            0.0
                        };
                    window.set_size(slint::PhysicalSize::new(
                        (300.0 * scale).round() as u32,
                        (height * scale).round() as u32,
                    ));
                } else if let Some(size) = saved_size.take() {
                    window.set_size(size);
                }
            }
        });
    }
    let queued = Arc::new(AtomicBool::new(false));
    let displayed = Arc::new(std::sync::Mutex::new(None));
    let crop_editor = crate::crop_editor::Editor::default();
    let observed_crop_editor = crop_editor.clone();
    let last_seek = Arc::new(std::sync::Mutex::new(SeekOsd::default()));
    let last_video = Arc::new(std::sync::Mutex::new(crate::video_osd::VideoOsd::new(
        prefs.disable_osd_speed,
        prefs.disable_osd_pause_resume,
    )));
    let last_screenshot = Arc::new(std::sync::Mutex::new(
        crate::screenshot_osd::ScreenshotOsd::default(),
    ));
    let screenshot_scratch = Arc::new(crate::screenshot_output::Scratch::default());
    let opening_fullscreen = Arc::new(std::sync::Mutex::new(
        crate::opening_fullscreen::OpeningFullscreen::default(),
    ));
    let notify_music_mode_state = music_mode_state.clone();
    let worker_scratch = screenshot_scratch.clone();
    let notify = move |state: Playback| {
        // This closure remains owned by the engine until native destruction.
        let _retain_until_native_teardown = &worker_scratch;
        if let Ok(mut latest_playback) = observed_playback.lock() {
            *latest_playback = state.clone();
        }
        if let Ok(mut latest) = latest.lock() {
            latest.1.observe(&state);
            latest.0 = Some(state);
        }
        if queued.swap(true, Ordering::AcqRel) {
            return;
        }
        let latest = latest.clone();
        let queued = queued.clone();
        let weak = app_weak.clone();
        let displayed = displayed.clone();
        let last_seek = last_seek.clone();
        let last_video = last_video.clone();
        let last_screenshot = last_screenshot.clone();
        let crop_editor = observed_crop_editor.clone();
        let opening_fullscreen = opening_fullscreen.clone();
        let music_mode_state = notify_music_mode_state.clone();
        let _ = slint::invoke_from_event_loop(move || {
            queued.store(false, Ordering::Release);
            if let (Some(app), Ok(mut latest)) = (weak.upgrade(), latest.lock())
                && let Some(state) = latest.0.take()
                && let Ok(mut displayed) = displayed.lock()
                && let Ok(mut last_seek) = last_seek.lock()
                && let Ok(mut last_video) = last_video.lock()
                && let Ok(mut last_screenshot) = last_screenshot.lock()
            {
                let audio_status = state.audio_status();
                let captures = latest.1.take();
                let enter_fullscreen = opening_fullscreen.lock().is_ok_and(|mut opening| {
                    opening.observe(
                        &state,
                        app.get_fullscreen_when_open(),
                        app.get_fullscreen(),
                        false,
                    )
                });
                let clipboard_error = crate::screenshot_output::copy(&app, &captures);
                crop_editor.observe(&app, &state);
                update_ui(
                    &app,
                    state,
                    &mut displayed,
                    &mut last_seek,
                    &mut last_video,
                    &mut last_screenshot,
                );
                if let Ok(mut mode) = music_mode_state.lock()
                    && let Some(active) = mode.observe(audio_status, app.get_fullscreen())
                {
                    app.set_music_mode(active);
                }
                crate::screenshot_output::cleanup(&app, &captures);
                if enter_fullscreen {
                    app.invoke_toggle_fullscreen();
                }
                if let Some(error) = clipboard_error {
                    app.set_status(format!("Could not copy screenshot: {error}").into());
                }
            }
        });
    };
    let player = if let Some(presentation) = &native {
        presentation.start_engine(engine_options, notify)
    } else {
        Player::start_with_options(&platform::playback_library(), false, engine_options, notify)
    };
    let player = match player {
        Ok(player) => player,
        Err(error) => {
            app.set_status(error.clone().into());
            return Err(std::io::Error::other(error).into());
        }
    };
    #[cfg(target_os = "windows")]
    let live_text_timer = Some(install_live_text(
        app,
        player.command_sender(),
        latest_playback.clone(),
    ));
    #[cfg(not(target_os = "windows"))]
    let live_text_timer = None;
    let manual_music_mode = music_mode_state.clone();
    let weak = app.as_weak();
    let clipboard_parent = weak.clone();
    app.on_copy_live_text(move |text| {
        if let Err(error) = crate::platform::clipboard::copy_text(text.as_str())
            && let Some(app) = clipboard_parent.upgrade()
        {
            app.set_status(format!("Could not copy recognized text: {error}").into());
        }
    });
    app.on_toggle_music_mode(move || {
        if let (Some(app), Ok(mut mode)) = (weak.upgrade(), manual_music_mode.lock()) {
            let active = mode.toggle_manually();
            if active && app.get_fullscreen() {
                app.invoke_toggle_fullscreen();
            }
            app.set_music_mode(active);
        }
    });
    let automatic_music_mode = music_mode_state.clone();
    app.on_auto_switch_music_mode_changed(move |enabled| {
        if let Ok(mut mode) = automatic_music_mode.lock() {
            mode.set_auto_switch(enabled);
        }
    });
    crate::screenshot_preferences::connect(
        app,
        player.command_sender(),
        prefs_path.clone(),
        prefs.clone(),
        latest_playback.clone(),
        {
            let path = prefs_path.clone();
            let recent_media = recent_media.clone();
            let history_cleared = history_cleared.clone();
            let welcome = welcome.map(WelcomeWindow::as_weak);
            let resume_last_position = prefs.resume_last_position;
            move || {
                let removed = crate::utilities_preferences::clear_recent_history(&path)?;
                recent_media.borrow_mut().clear();
                history_cleared.set(true);
                if let Some(welcome) = welcome.as_ref().and_then(slint::Weak::upgrade) {
                    refresh_welcome_history(&welcome, &[], resume_last_position);
                }
                Ok(removed)
            }
        },
    );
    let idle_timer = Rc::new(slint::Timer::default());
    let opening_player = player.command_sender();
    let opening_app = app.as_weak();
    let resume_player = player.command_sender();
    let resume_app = app.as_weak();
    let resume_welcome = welcome.map(WelcomeWindow::as_weak);
    let recent_media_for_resume = recent_media.clone();
    app.on_resume_last_position_changed(move |enabled| {
        if let Err(error) = resume_player.send(Command::SetResumeLastPosition(enabled))
            && let Some(app) = resume_app.upgrade()
        {
            app.set_status(error.into());
        }
        if let Some(welcome) = resume_welcome.as_ref().and_then(slint::Weak::upgrade) {
            refresh_welcome_history(&welcome, &recent_media_for_resume.borrow(), enabled);
        }
    });
    app.on_pause_when_open_changed(move |pause| {
        if let Err(error) = opening_player.send(Command::SetPauseWhenOpen(pause))
            && let Some(app) = opening_app.upgrade()
        {
            app.set_status(error.into());
        }
    });
    crate::window_controls::install(app, native.as_ref().map(platform::NativePresentation::host));
    crop_editor.install(app, player.command_sender());
    let reflection_player = player.command_sender();
    let weak = app.as_weak();
    app.on_toggle_video_reflection(move |horizontal| {
        if let Some(app) = weak.upgrade() {
            refresh_video_menu(&app);
            if !app.get_loaded()
                || !app.get_reflection_known()
                || app.get_crop_editing()
                || app.get_dialog_open()
            {
                return;
            }
            let axis = if horizontal {
                riina_core::video_reflection::Axis::Horizontal
            } else {
                riina_core::video_reflection::Axis::Vertical
            };
            if let Err(error) = reflection_player.send(Command::ToggleVideoReflection(axis)) {
                app.set_status(error.into());
            }
        }
    });
    let seek_timer = slint::Timer::default();
    let screenshot_player = player.command_sender();
    let weak = app.as_weak();
    app.on_take_screenshot(move || {
        let Some(app) = weak.upgrade() else { return };
        if !app.get_screenshot_available() || app.get_crop_editing() || app.get_dialog_open() {
            return;
        }
        app.invoke_activity();
        let result = crate::screenshot_preferences::current(&app).and_then(|screenshot_settings| {
            if !screenshot_settings.destinations.enabled() {
                return Ok(());
            }
            (if screenshot_settings.destinations.save_to_file {
                platform::screenshot_directory(screenshot_settings.directory.as_deref())
            } else {
                screenshot_scratch.directory()
            })
            .map_err(|error| error.to_string())
            .and_then(|directory| {
                screenshot_player.send(Command::Screenshot {
                    directory: directory.to_string_lossy().into_owned(),
                    content: screenshot_settings.content(),
                    encoding: screenshot_settings.encoding.clone(),
                })
            })
        });
        if let Err(error) = result {
            app.set_status(error.into());
        }
    });
    let weak = app.as_weak();
    app.on_open_screenshot_folder(move || {
        let Some(app) = weak.upgrade() else { return };
        if app.get_crop_editing() || app.get_dialog_open() {
            return;
        }
        if let Err(error) = crate::screenshot_preferences::current(&app).and_then(|settings| {
            platform::screenshot_directory(settings.directory.as_deref())
                .and_then(|directory| platform::open_directory(&directory))
                .map_err(|error| error.to_string())
        }) {
            app.set_status(error.to_string().into());
        }
    });
    let deinterlace_player = player.command_sender();
    let weak = app.as_weak();
    app.on_toggle_deinterlace(move || {
        let Some(app) = weak.upgrade() else { return };
        refresh_video_menu(&app);
        if !app.get_loaded()
            || !app.get_deinterlace_known()
            || app.get_crop_editing()
            || app.get_dialog_open()
        {
            return;
        }
        app.invoke_activity();
        if let Err(error) = deinterlace_player.send(Command::ToggleDeinterlace) {
            app.set_status(error.into());
        }
    });
    let weak = app.as_weak();
    app.on_show_seek_osd(move || {
        let Some(app) = weak.upgrade() else { return };
        if !app.get_osd_enabled() || app.get_disable_osd_seek() {
            return;
        }
        app.set_seek_osd_visible(true);
        clear_screenshot_preview(&app);
        app.set_video_osd_visible(false);
        let weak = weak.clone();
        seek_timer.start(
            slint::TimerMode::SingleShot,
            configured_osd_timeout(&app),
            move || {
                if let Some(app) = weak.upgrade() {
                    app.set_seek_osd_visible(false);
                }
            },
        );
    });
    let video_timer = slint::Timer::default();
    let screenshot_timer = slint::Timer::default();
    let weak = app.as_weak();
    app.on_show_screenshot_preview(move || {
        let Some(app) = weak.upgrade() else { return };
        if !app.get_osd_enabled() || app.get_crop_editing() {
            return;
        }
        app.set_seek_osd_visible(false);
        app.set_video_osd_visible(false);
        app.set_screenshot_preview_visible(true);
        let weak = weak.clone();
        screenshot_timer.start(
            slint::TimerMode::SingleShot,
            Duration::from_secs(5),
            move || {
                if let Some(app) = weak.upgrade() {
                    clear_screenshot_preview(&app);
                }
            },
        );
    });
    let weak = app.as_weak();
    app.on_screenshot_preview_action(move |action| {
        let Some(app) = weak.upgrade() else { return };
        if !app.get_screenshot_preview_visible() || app.get_crop_editing() || app.get_dialog_open()
        {
            return;
        }
        let path = app.get_screenshot_preview_path();
        if path.is_empty() {
            return;
        }
        let path = std::path::Path::new(path.as_str());
        let result = match action {
            0 => {
                let _ = std::fs::remove_file(path);
                Ok(())
            }
            1 => platform::open_directory(path),
            2 => platform::reveal_file(path),
            _ => return,
        };
        clear_screenshot_preview(&app);
        if let Err(error) = result {
            app.set_status(error.to_string().into());
        }
    });
    let weak = app.as_weak();
    app.on_show_video_osd(move || {
        let Some(app) = weak.upgrade() else { return };
        if !app.get_osd_enabled() {
            return;
        }
        app.set_seek_osd_visible(false);
        clear_screenshot_preview(&app);
        app.set_video_osd_visible(true);
        let weak = weak.clone();
        video_timer.start(
            slint::TimerMode::SingleShot,
            configured_osd_timeout(&app),
            move || {
                if let Some(app) = weak.upgrade() {
                    app.set_video_osd_visible(false);
                }
            },
        );
    });
    let weak = app.as_weak();
    app.on_activity(move || {
        if let Some(app) = weak.upgrade() {
            app.set_controls_visible(true);
        }
        let weak = weak.clone();
        // One deadline per interaction: paused/hidden playback has no UI polling timer.
        idle_timer.start(
            slint::TimerMode::SingleShot,
            Duration::from_millis(2500),
            move || {
                if let Some(app) = weak.upgrade()
                    && app.get_playing()
                    && !app.get_dialog_open()
                {
                    app.set_controls_visible(false);
                }
            },
        );
    });
    let pending_single_click = Rc::new(RefCell::new(None::<i32>));
    let single_click_timer = Rc::new(slint::Timer::default());
    let pending = pending_single_click.clone();
    let timer = single_click_timer.clone();
    let single_player = player.command_sender();
    let single_app = app.as_weak();
    app.on_single_click_action(move |action| {
        *pending.borrow_mut() = Some(action);
        let pending = pending.clone();
        let player = single_player.clone();
        let weak = single_app.clone();
        // Slint reports clicked before double-clicked. Delay the single action
        // so a double click invokes only its configured action.
        timer.start(
            slint::TimerMode::SingleShot,
            Duration::from_millis(500),
            move || {
                let action = pending.borrow_mut().take();
                if let (Some(action), Some(app)) = (action, weak.upgrade()) {
                    dispatch_control_mouse_action(&app, &player, action);
                }
            },
        );
    });
    let pending = pending_single_click;
    let timer = single_click_timer;
    let double_player = player.command_sender();
    let double_app = app.as_weak();
    app.on_double_click_action(move |action| {
        timer.stop();
        pending.borrow_mut().take();
        if let Some(app) = double_app.upgrade() {
            dispatch_control_mouse_action(&app, &double_player, action);
        }
    });
    let button_player = player.command_sender();
    let button_app = app.as_weak();
    app.on_mouse_button_action(move |action| {
        if let Some(app) = button_app.upgrade() {
            dispatch_control_mouse_action(&app, &button_player, action);
        }
    });
    macro_rules! connect {
        ($method:ident, $command:expr) => {{
            let player = player.command_sender();
            let weak = app.as_weak();
            app.$method(move || {
                let command = $command;
                if let Some(app) = weak.upgrade() {
                    app.invoke_activity();
                    if matches!(command, Command::Stop) {
                        media_open::invalidate(&app);
                    }
                }
                if let Err(error) = player.send(command) {
                    if let Some(app) = weak.upgrade() {
                        app.set_status(error.into());
                    }
                }
            });
        }};
    }
    // Let mpv toggle its current state: UI pause snapshots can lag an in-flight
    // transition, while the playback worker serializes this action with engine events.
    let pause_player = player.command_sender();
    let pause_app = app.as_weak();
    app.on_toggle_pause(move || {
        let Some(app) = pause_app.upgrade() else {
            return;
        };
        app.invoke_activity();
        if let Err(error) = pause_player.toggle_pause() {
            app.set_status(error.into());
        }
    });
    connect!(on_toggle_mute, Command::ToggleMute);
    let hdr_player = player.command_sender();
    let weak = app.as_weak();
    app.on_set_hdr_enabled(move |enabled| {
        let Some(app) = weak.upgrade() else { return };
        if !app.get_hdr_source_detected() || (enabled && !app.get_hdr_display_active()) {
            app.set_status("HDR needs an HDR source and an HDR-enabled display".into());
            return;
        }
        match hdr_player.send(Command::SetHdrEnabled(enabled)) {
            Ok(()) => {
                app.set_hdr_enabled(enabled);
                app.invoke_activity();
            }
            Err(error) => app.set_status(error.into()),
        }
    });
    connect!(on_stop, Command::Stop);
    connect!(on_frame_step, Command::FrameStep);
    connect!(on_frame_back_step, Command::FrameBackStep);
    let chapter_player = player.command_sender();
    let weak = app.as_weak();
    app.on_adjust_chapter(move |direction| {
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
        }
        if let Err(error) = chapter_player.send(Command::AdjustChapter(direction))
            && let Some(app) = weak.upgrade()
        {
            app.set_status(error.into());
        }
    });
    let subtitle_player = player.command_sender();
    let weak = app.as_weak();
    app.on_subtitle_seek(move |direction| {
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
        }
        if let Err(error) = subtitle_player.send(Command::SubtitleSeek(direction))
            && let Some(app) = weak.upgrade()
        {
            app.set_status(error.into());
        }
    });
    connect!(on_playlist_next, Command::PlaylistNext);
    connect!(on_playlist_previous, Command::PlaylistPrevious);
    connect!(on_reset_speed, Command::Speed(1.0));
    let speed_slider_player = player.command_sender();
    let weak = app.as_weak();
    app.on_set_speed_slider(move |position| {
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
            let result = riina_core::speed_controls::from_slider(position.into())
                .map_err(str::to_owned)
                .and_then(|command| speed_slider_player.send(command));
            if let Err(error) = result {
                app.set_status(error.into());
            }
        }
    });
    let speed_text_player = player.command_sender();
    let weak = app.as_weak();
    app.on_set_speed_text(move |text| {
        let result = riina_core::speed_controls::from_text(text.as_str())
            .map_err(str::to_owned)
            .and_then(|command| speed_text_player.send(command));
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
            if let Err(error) = result {
                app.set_status(error.into());
                return false;
            }
        }
        true
    });
    let aspect_player = player.command_sender();
    let weak = app.as_weak();
    app.on_set_video_aspect(move |text| {
        if text.is_empty() {
            return;
        }
        let aspect = riina_core::video_controls::AspectRatio::from_text(text.as_str());
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
            if let Err(error) = aspect_player.send(Command::VideoAspect(aspect)) {
                app.set_status(error.into());
            }
        }
    });
    let rotation_player = player.command_sender();
    let weak = app.as_weak();
    app.on_set_menu_video_aspect(move |index| {
        let Some(app) = weak.upgrade() else { return };
        refresh_video_menu(&app);
        if !app.get_loaded()
            || !app.get_video_aspect_known()
            || app.get_crop_editing()
            || app.get_dialog_open()
        {
            return;
        }
        if let Some(label) = usize::try_from(index)
            .ok()
            .and_then(|index| riina_core::video_controls::ASPECT_MENU_PRESETS.get(index))
        {
            app.invoke_set_video_aspect((*label).into());
        }
    });
    let crop_player = player.command_sender();
    let crop_ui = app.as_weak();
    app.on_set_video_crop(move |index| {
        if index == 6 {
            if let Some(app) = crop_ui.upgrade() {
                app.invoke_begin_custom_crop();
            }
            return;
        }
        let preset = if index == 0 {
            None
        } else if let Some(preset) = usize::try_from(index - 1)
            .ok()
            .and_then(|index| riina_core::crop::Preset::ALL.get(index))
            .copied()
        {
            Some(preset)
        } else {
            return;
        };
        if let Some(app) = crop_ui.upgrade() {
            app.invoke_activity();
            if let Err(error) = crop_player.send(Command::VideoCrop(preset)) {
                app.set_status(error.into());
            }
        }
    });
    let menu_crop_player = player.command_sender();
    let weak = app.as_weak();
    app.on_set_menu_video_crop(move |index| {
        let Some(app) = weak.upgrade() else { return };
        refresh_video_menu(&app);
        if !app.get_loaded()
            || !app.get_video_crop_known()
            || app.get_crop_editing()
            || app.get_dialog_open()
        {
            return;
        }
        if index == riina_core::crop::MENU_CUSTOM_INDEX as i32 {
            app.invoke_begin_custom_crop();
            return;
        }
        let preset = if index == 0 {
            None
        } else if let Some(preset) = usize::try_from(index - 1)
            .ok()
            .and_then(|index| riina_core::crop::Preset::MENU_ALL.get(index))
            .copied()
        {
            Some(preset)
        } else {
            return;
        };
        app.invoke_activity();
        if let Err(error) = menu_crop_player.send(Command::VideoCrop(preset)) {
            app.set_status(error.into());
        }
    });
    let weak = app.as_weak();
    app.on_set_video_rotation(move |degrees| {
        if let Some(app) = weak.upgrade()
            && let Some(rotation) =
                riina_core::video_controls::Rotation::from_degrees(degrees.into())
        {
            app.invoke_activity();
            if let Err(error) = rotation_player.send(Command::VideoRotation(rotation)) {
                app.set_status(error.into());
            }
        }
    });
    let menu_rotation_player = player.command_sender();
    let weak = app.as_weak();
    app.on_set_menu_video_rotation(move |degrees| {
        let Some(app) = weak.upgrade() else { return };
        refresh_video_menu(&app);
        if !app.get_loaded()
            || !app.get_video_rotation_known()
            || app.get_crop_editing()
            || app.get_dialog_open()
        {
            return;
        }
        let Some(rotation) = riina_core::video_controls::Rotation::from_degrees(degrees.into())
        else {
            return;
        };
        app.invoke_activity();
        if let Err(error) = menu_rotation_player.send(Command::VideoRotation(rotation)) {
            app.set_status(error.into());
        }
    });
    let hardware_player = player.command_sender();
    let weak = app.as_weak();
    app.on_set_hardware_decoding(move |enabled| {
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
            let mode = if enabled {
                crate::codec_preferences::hardware_decoder(app.get_hardware_decoder_index())
                    .unwrap_or(riina_core::video_controls::HardwareDecoder::Auto)
            } else {
                riina_core::video_controls::HardwareDecoder::Disabled
            };
            if let Err(error) = hardware_player.send(Command::HardwareDecoding(mode)) {
                app.set_status(error.into());
            }
        }
    });
    connect!(on_reset_subtitle_delay, Command::SubtitleDelay(0.0));
    let equalizer_player = player.command_sender();
    let weak = app.as_weak();
    app.on_set_video_equalizer(move |index, value| {
        let option = usize::try_from(index).ok().and_then(|index| {
            riina_core::video_controls::Equalizer::ALL
                .get(index)
                .copied()
        });
        if let Some(option) = option
            && value.is_finite()
            && (-100.0..=100.0).contains(&value)
            && let Some(app) = weak.upgrade()
        {
            app.invoke_activity();
            // NSSlider's integer action uses whole units. The backend validates
            // the range again so every caller obeys the same portable contract.
            if let Err(error) = equalizer_player.send(Command::VideoEqualizer {
                option,
                value: value as i64,
            }) {
                app.set_status(error.into());
            }
        }
    });
    let scale_player = player.command_sender();
    let absolute_scale_player = player.command_sender();
    let font_size_player = player.command_sender();
    crate::font_picker::connect(app);
    let outline_size_player = player.command_sender();
    let color_player = player.command_sender();
    let weak = app.as_weak();
    app.on_set_subtitle_color(move |target, value| {
        if let Some(app) = weak.upgrade() {
            let rgba = [value.red(), value.green(), value.blue(), value.alpha()]
                .map(|channel| f64::from(channel) / 255.0);
            let Ok(color) = riina_core::subtitle_color::SubtitleColor::from_rgba(rgba) else {
                return;
            };
            let command = match target {
                1 => Command::SubtitleTextColor(color),
                2 => Command::SubtitleShadowColor(color),
                3 => Command::SubtitleBorderColor(color),
                _ => return,
            };
            app.invoke_activity();
            if let Err(error) = color_player.send(command) {
                app.set_status(error.into());
            } else {
                match target {
                    1 => app.set_subtitle_text_preference_color(value),
                    2 => {
                        app.set_subtitle_shadow_preference_color(value);
                        app.set_subtitle_background_preference_color(value);
                    }
                    3 => app.set_subtitle_outline_preference_color(value),
                    _ => unreachable!("target was validated above"),
                }
            }
        }
    });
    let weak = app.as_weak();
    app.on_set_subtitle_outline_size(move |value| {
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
            if let Err(error) = outline_size_player.send(Command::SubtitleOutlineSize(value as f64))
            {
                app.set_status(error.into());
            } else {
                app.set_subtitle_outline_size_preference(value);
            }
        }
    });
    let weak = app.as_weak();
    app.on_set_subtitle_font_size(move |value| {
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
            if let Err(error) = font_size_player.send(Command::SubtitleFontSize(value as f64)) {
                app.set_status(error.into());
            } else {
                app.set_subtitle_font_size_preference(value);
            }
        }
    });
    let weak = app.as_weak();
    app.on_set_subtitle_scale_slider(move |value| {
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
            // Each side advances in 0.05 scale units; the negative side is reciprocal.
            let magnitude = ((1.0 + f64::from(value).abs()) * 20.0).round() / 20.0;
            let scale = if value < 0.0 {
                magnitude.recip()
            } else {
                magnitude
            };
            if let Err(error) = absolute_scale_player.send(Command::SubtitleScale(scale)) {
                app.set_status(error.into());
            }
        }
    });
    let weak = app.as_weak();
    app.on_adjust_subtitle_scale(move |delta| {
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
        }
        if let Err(error) = scale_player.send(Command::AdjustSubtitleScale(delta as f64))
            && let Some(app) = weak.upgrade()
        {
            app.set_status(error.into());
        }
    });
    let position_player = player.command_sender();
    let weak = app.as_weak();
    app.on_adjust_subtitle_position(move |delta| {
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
        }
        if let Err(error) = position_player.send(Command::AdjustSubtitlePosition(delta as f64))
            && let Some(app) = weak.upgrade()
        {
            app.set_status(error.into());
        }
    });
    connect!(on_reset_audio_delay, Command::AudioDelay(0.0));
    let audio_delay_player = player.command_sender();
    let weak = app.as_weak();
    app.on_adjust_audio_delay(move |delta| {
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
        }
        if let Err(error) = audio_delay_player.send(Command::AdjustAudioDelay(delta as f64))
            && let Some(app) = weak.upgrade()
        {
            app.set_status(error.into());
        }
    });
    let delay_player = player.command_sender();
    let text_delay_player = player.command_sender();
    let weak = app.as_weak();
    app.on_set_delay_text(move |category, text| {
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
            use riina_core::subtitle_controls::SubtitleLane;
            let lane = match category {
                2 => Ok(SubtitleLane::Primary),
                5 => Ok(SubtitleLane::Secondary),
                _ => Err("Unknown subtitle delay lane"),
            };
            let command = lane
                .and_then(|lane| lane.delay_from_text(&text))
                .map_err(str::to_owned);
            if let Err(error) = command.and_then(|command| text_delay_player.send(command)) {
                app.set_status(error.into());
                return false;
            }
            return true;
        }
        false
    });
    let secondary_position_player = player.command_sender();
    let weak = app.as_weak();
    app.on_set_subtitle_position(move |secondary, value| {
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
            if let Err(error) = secondary_position_player.send(if secondary {
                Command::SecondarySubtitlePosition(value as f64)
            } else {
                Command::SubtitlePosition(value as f64)
            }) {
                app.set_status(error.into());
            }
        }
    });
    let weak = app.as_weak();
    app.on_adjust_subtitle_delay(move |delta| {
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
        }
        if let Err(error) = delay_player.send(Command::AdjustSubtitleDelay(delta as f64))
            && let Some(app) = weak.upgrade()
        {
            app.set_status(error.into());
        }
    });
    connect!(on_cycle_ab_loop, Command::CycleAbLoop);
    let ab_loop_player = player.command_sender();
    let ab_loop_app = app.as_weak();
    app.on_set_ab_loop_point(move |point, value| {
        let Some(app) = ab_loop_app.upgrade() else {
            return;
        };
        if !app.get_loaded() || !value.is_finite() {
            return;
        }
        let point = match point {
            0 => riina_core::AbLoopPoint::A,
            1 => riina_core::AbLoopPoint::B,
            _ => return,
        };
        app.invoke_activity();
        if let Err(error) = ab_loop_player.send(Command::SetAbLoopPoint {
            point,
            value: Some(f64::from(value)),
        }) {
            app.set_status(error.into());
        }
    });
    connect!(on_toggle_file_loop, Command::ToggleFileLoop);
    connect!(
        on_toggle_secondary_subtitle_visibility,
        Command::ToggleSecondarySubtitleVisibility
    );
    connect!(on_cycle_audio_track, Command::CycleTrack(TrackKind::Audio));
    connect!(on_cycle_subtitle_track, Command::CycleTrack(TrackKind::Sub));
    connect!(on_cycle_video_track, Command::CycleTrack(TrackKind::Video));
    connect!(
        on_toggle_subtitle_visibility,
        Command::ToggleSubtitleVisibility
    );
    let selection_player = player.command_sender();
    let weak = app.as_weak();
    app.on_select_item(move |category, token| {
        // IDs stay strings in the UI: Slint's float numbers cannot preserve every i64.
        let command = match category {
            5 => token
                .as_str()
                .parse::<i64>()
                .map(Some)
                .or_else(|error| if token == "no" { Ok(None) } else { Err(error) })
                .map(Command::SelectSecondarySubtitle)
                .map_err(|error| error.to_string()),
            1 | 2 | 6 => token
                .as_str()
                .parse::<i64>()
                .map(Some)
                .or_else(|error| if token == "no" { Ok(None) } else { Err(error) })
                .map(|id| Command::SelectTrack {
                    kind: match category {
                        1 => TrackKind::Audio,
                        6 => TrackKind::Video,
                        _ => TrackKind::Sub,
                    },
                    id,
                })
                .map_err(|error| error.to_string()),
            3 => token
                .parse::<usize>()
                .map(Command::SelectChapter)
                .map_err(|error| error.to_string()),
            4 => token
                .parse::<i64>()
                .map(Command::SelectPlaylistId)
                .map_err(|error| error.to_string()),
            _ => Err("Unknown selection category".into()),
        };
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
            if let Err(error) = command.and_then(|command| selection_player.send(command)) {
                app.set_status(error.into());
            }
        }
    });
    let delay_player = player.command_sender();
    let weak = app.as_weak();
    app.on_set_delay(move |category, value| {
        let command = match category {
            1 => Some(Command::AudioDelay(value as f64)),
            2 => Some(Command::SubtitleDelay(value as f64)),
            5 => Some(Command::SecondarySubtitleDelay(value as f64)),
            _ => None,
        };
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
            if let Some(command) = command
                && let Err(error) = delay_player.send(command)
            {
                app.set_status(error.into());
            }
        }
    });
    let playlist_player = player.command_sender();
    let weak = app.as_weak();
    app.on_playlist_action(move |action, token| {
        if matches!(action, 6..=8) {
            let Some(app) = weak.upgrade() else {
                return;
            };
            app.invoke_activity();
            let rows = app.get_playlist();
            let selected_index = (0..rows.row_count()).find(|index| {
                rows.row_data(*index)
                    .is_some_and(|row| row.token.as_str() == token.as_str())
            });
            let result = match action {
                6 | 7 => selected_index
                    .and_then(|index| rows.row_data(index))
                    .ok_or_else(|| "Select a playlist item first".to_owned())
                    .and_then(|row| crate::platform::clipboard::copy_text(row.path.as_str()))
                    .and_then(|()| {
                        if action == 7 {
                            token
                                .parse::<i64>()
                                .map(Command::RemovePlaylistId)
                                .map_err(|error| error.to_string())
                                .and_then(|command| playlist_player.send(command))
                        } else {
                            Ok(())
                        }
                    }),
                8 => crate::platform::clipboard::read_text().and_then(|text| {
                    let paths = riina_core::playlist::clipboard_references(&text);
                    if paths.is_empty() {
                        return Err("Clipboard contains no playlist references".into());
                    }
                    let index = selected_index.unwrap_or(0);
                    playlist_player.send(Command::InsertPlaylistFiles { paths, index })
                }),
                9 => selected_index
                    .and_then(|index| rows.row_data(index))
                    .ok_or_else(|| "Select a playlist item first".to_owned())
                    .and_then(|row| crate::media_open::open_in_new_player(row.path.as_str())),
                _ => unreachable!(),
            };
            if let Err(error) = result {
                app.set_status(error.into());
            } else if action == 7 {
                app.set_selected_playlist_token("".into());
            }
            return;
        }
        let command = match action {
            1 => token
                .parse()
                .map(Command::RemovePlaylistId)
                .map_err(|error: std::num::ParseIntError| error.to_string()),
            2 | 3 => token
                .parse()
                .map(|id| Command::MovePlaylistId {
                    id,
                    direction: if action == 2 { -1 } else { 1 },
                })
                .map_err(|error: std::num::ParseIntError| error.to_string()),
            4 => Ok(Command::ClearPlaylist),
            5 => Ok(Command::CycleLoop),
            _ => Err("Unknown playlist action".into()),
        };
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
            if let Err(error) = command.and_then(|command| playlist_player.send(command)) {
                app.set_status(error.into());
            }
        }
    });
    let shuffle_player = player.command_sender();
    let weak = app.as_weak();
    app.on_toggle_playlist_shuffle(move || {
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
            if let Err(error) = shuffle_player.send(Command::TogglePlaylistShuffle) {
                app.set_status(error.into());
            }
        }
    });
    let sort_player = player.command_sender();
    let weak = app.as_weak();
    app.on_sort_playlist(move |sort| {
        let sort = match sort {
            0 => Some(riina_core::playlist::PlaylistSort::FilenameAscending),
            1 => Some(riina_core::playlist::PlaylistSort::FilenameDescending),
            2 => Some(riina_core::playlist::PlaylistSort::PathAscending),
            3 => Some(riina_core::playlist::PlaylistSort::PathDescending),
            _ => None,
        };
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
            let result = sort.map_or_else(
                || Err("Unknown playlist sort order".to_owned()),
                |sort| sort_player.send(Command::SortPlaylist(sort)),
            );
            if let Err(error) = result {
                app.set_status(error.into());
            }
        }
    });
    let browse_player = player.command_sender();
    let weak = app.as_weak();
    let browse_host = native.as_ref().map(platform::NativePresentation::host);
    app.on_browse_files(move |category| {
        let purpose = match category {
            1 => Purpose::Audio,
            2 => Purpose::Subtitle,
            4 => Purpose::Playlist,
            _ => return,
        };
        if let Some(app) = weak.upgrade() {
            ui_dialogs::open(
                &app,
                browse_host.clone(),
                purpose,
                browse_player.clone(),
                None,
            );
        }
    });
    for absolute in [true, false] {
        let player = player.command_sender();
        let weak = app.as_weak();
        let seek = move |seconds: f32| {
            if let Some(app) = weak.upgrade() {
                app.invoke_activity();
            }
            if let Err(error) = player.send(Command::Seek {
                seconds: seconds as f64,
                absolute,
            }) && let Some(app) = weak.upgrade()
            {
                app.set_status(error.into());
            }
        };
        if absolute {
            app.on_seek(seek);
        } else {
            app.on_seek_relative(seek);
        }
    }
    let boost_player = player.command_sender();
    let weak = app.as_weak();
    app.on_set_volume_boost(move |enabled| {
        if let Some(app) = weak.upgrade() {
            let volume = app.get_volume().min(if enabled { 200.0 } else { 100.0 });
            match boost_player.send(Command::SetVolumeBoost(enabled)) {
                Ok(()) => {
                    app.set_volume_boost(enabled);
                    app.set_volume(volume);
                }
                Err(error) => app.set_status(error.into()),
            }
            app.invoke_activity();
        }
    });
    let volume_player = player.command_sender();
    let weak = app.as_weak();
    app.on_set_volume(move |volume| {
        let Some(app) = weak.upgrade() else { return };
        app.invoke_activity();
        let limit = if app.get_volume_boost() { 200.0 } else { 100.0 };
        if let Err(error) = volume_player.send(Command::Volume((volume as f64).min(limit)))
            && let Some(app) = weak.upgrade()
        {
            app.set_status(error.into());
        }
    });
    let speed_player = player.command_sender();
    let weak = app.as_weak();
    app.on_speed(move |factor| {
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
        }
        if let Err(error) = speed_player.send(Command::MultiplySpeed(factor as f64))
            && let Some(app) = weak.upgrade()
        {
            app.set_status(error.into());
        }
    });
    let adjustment_player = player.command_sender();
    let weak = app.as_weak();
    app.on_adjust_volume(move |delta| {
        let Some(app) = weak.upgrade() else { return };
        app.invoke_activity();
        if let Err(error) = adjustment_player.send(Command::AdjustVolume {
            delta: delta as f64,
            boost: app.get_volume_boost(),
        }) {
            app.set_status(error.into());
        }
    });
    let weak = app.as_weak();
    let fullscreen_host = native.as_ref().map(platform::NativePresentation::host);
    app.on_toggle_fullscreen(move || {
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
            let fullscreen = !app.get_fullscreen();
            if let Some(host) = fullscreen_host.as_ref().and_then(slint::Weak::upgrade) {
                host.window().set_fullscreen(fullscreen);
            } else {
                app.window().set_fullscreen(fullscreen);
            }
            app.set_fullscreen(fullscreen);
        }
    });
    let open_player = player.command_sender();
    let weak = app.as_weak();
    let open_host = native.as_ref().map(platform::NativePresentation::host);
    let welcome_window = welcome.map(|welcome| welcome.as_weak());
    let pending_welcome_open = Rc::new(RefCell::new(None));
    let ready_pending = pending_welcome_open.clone();
    let ready_app = app.as_weak();
    let ready_player = player.command_sender();
    app.on_renderer_ready(move || {
        if let Some(request) = ready_pending.borrow_mut().take()
            && let Some(app) = ready_app.upgrade()
        {
            dispatch_welcome_open(&app, request, &ready_player);
        }
    });
    let file_welcome = welcome_window.clone();
    let file_open_host = open_host.clone();
    let file_dialog_host = open_host.clone();
    app.on_open_file(move || {
        if let Some(app) = weak.upgrade() {
            let welcome_is_visible = file_welcome
                .as_ref()
                .and_then(slint::Weak::upgrade)
                .is_some_and(|welcome| welcome.window().is_visible());
            if welcome_is_visible {
                set_native_video_host_visible(file_open_host.as_ref(), true);
                if let Err(error) = app.show() {
                    app.set_status(error.to_string().into());
                    return;
                }
                if let Some(welcome) = file_welcome.as_ref().and_then(slint::Weak::upgrade) {
                    let _ = welcome.hide();
                }
            }
            ui_dialogs::open(
                &app,
                file_dialog_host.clone(),
                Purpose::Media,
                open_player.clone(),
                welcome_is_visible.then(|| file_welcome.clone()).flatten(),
            );
        }
    });
    let open_url = player.command_sender();
    let weak = app.as_weak();
    let open_url_host = open_host.clone();
    let url_welcome = welcome_window.clone();
    let url_pending = pending_welcome_open.clone();
    app.on_open_url(move |value| {
        if let Some(app) = weak.upgrade() {
            let from_welcome = url_welcome
                .as_ref()
                .and_then(slint::Weak::upgrade)
                .is_some_and(|welcome| welcome.window().is_visible());
            if from_welcome {
                if let Err(error) = media_open::validate_url(value.as_str()) {
                    if let Some(welcome) = url_welcome.as_ref().and_then(slint::Weak::upgrade) {
                        welcome.set_url_error(error.into());
                    }
                    return;
                }
                set_native_video_host_visible(open_url_host.as_ref(), true);
                if let Err(error) = app.show() {
                    app.set_status(error.to_string().into());
                    return;
                }
                if let Some(welcome) = url_welcome.as_ref().and_then(slint::Weak::upgrade) {
                    welcome.set_url_error("".into());
                    let _ = welcome.hide();
                }
                let request = PendingWelcomeOpen::Url(value.to_string());
                if app.get_engine_ready() {
                    dispatch_welcome_open(&app, request, &open_url);
                } else {
                    *url_pending.borrow_mut() = Some(request);
                }
            } else {
                media_open::url(&app, value.as_str(), open_url.clone());
            }
        }
    });
    let recent_player = player.command_sender();
    let recent_host = open_host.clone();
    let recent_media_for_open = recent_media.clone();
    let weak = app.as_weak();
    let recent_welcome = welcome_window.clone();
    let recent_pending = pending_welcome_open.clone();
    app.on_open_recent(move |index| {
        let Ok(index) = usize::try_from(index) else {
            return;
        };
        let media = recent_media_for_open.borrow().get(index).cloned();
        if let (Some(app), Some(media)) = (weak.upgrade(), media) {
            let from_welcome = recent_welcome
                .as_ref()
                .and_then(slint::Weak::upgrade)
                .is_some_and(|welcome| welcome.window().is_visible());
            if from_welcome {
                set_native_video_host_visible(recent_host.as_ref(), true);
                if let Err(error) = app.show() {
                    app.set_status(error.to_string().into());
                    return;
                }
                if let Some(welcome) = recent_welcome.as_ref().and_then(slint::Weak::upgrade) {
                    let _ = welcome.hide();
                }
                let request = PendingWelcomeOpen::Recent(media, app.get_resume_last_position());
                if app.get_engine_ready() {
                    dispatch_welcome_open(&app, request, &recent_player);
                } else {
                    *recent_pending.borrow_mut() = Some(request);
                }
            } else {
                media_open::open_recent(
                    &app,
                    media,
                    app.get_resume_last_position(),
                    recent_player.clone(),
                );
            }
        }
    });
    let opened_history = history_cleared.clone();
    let opened_welcome = welcome_window.clone();
    app.on_media_opened(move || {
        opened_history.set(false);
        if let Some(welcome) = opened_welcome.as_ref().and_then(slint::Weak::upgrade) {
            let _ = welcome.hide();
        }
    });
    let weak = app.as_weak();
    app.window().on_close_requested(move || {
        if weak.upgrade().is_some_and(|app| app.get_dialog_open()) {
            slint::CloseRequestResponse::KeepWindowShown
        } else {
            if let Some(app) = weak.upgrade() {
                app.invoke_dismiss_font_picker();
                // Remove the borrowed texture before hide suspends its context.
                // Teardown runs inside the backend's window borrow and cannot
                // safely dirty a property that schedules another redraw.
                app.set_video_frame(slint::Image::default());
                app.set_engine_ready(false);
            }
            slint::CloseRequestResponse::HideWindow
        }
    });

    if let Some(welcome) = welcome {
        let weak = app.as_weak();
        welcome.on_open_file(move || {
            if let Some(app) = weak.upgrade() {
                app.invoke_open_file();
            }
        });
        let weak = app.as_weak();
        welcome.on_open_url(move |value| {
            if let Some(app) = weak.upgrade() {
                app.invoke_open_url(value);
            }
        });
        let weak = app.as_weak();
        welcome.on_open_recent(move |index| {
            if let Some(app) = weak.upgrade() {
                app.invoke_open_recent(index);
            }
        });
    }

    player
        .send(Command::SetVolumeBoost(prefs.volume_boost))
        .map_err(std::io::Error::other)?;
    player
        .send(Command::SetPauseWhenOpen(prefs.pause_when_open))
        .map_err(std::io::Error::other)?;
    player
        .send(Command::HardwareDecoding(prefs.hardware_decoder))
        .map_err(std::io::Error::other)?;
    player
        .send(Command::SubtitleAssOverride {
            lane: riina_core::subtitle_controls::SubtitleLane::Primary,
            level: prefs.subtitle_ass_override,
        })
        .map_err(std::io::Error::other)?;
    player
        .send(Command::SubtitleAssOverride {
            lane: riina_core::subtitle_controls::SubtitleLane::Secondary,
            level: prefs.secondary_subtitle_ass_override,
        })
        .map_err(std::io::Error::other)?;
    player
        .send(Command::SubtitleFont(prefs.subtitle_font.clone()))
        .map_err(std::io::Error::other)?;
    player
        .send(Command::SubtitleFontSize(prefs.subtitle_font_size))
        .map_err(std::io::Error::other)?;
    for command in crate::subtitle_preferences::commands(prefs.subtitle_text_style) {
        player.send(command).map_err(std::io::Error::other)?;
    }
    for command in prefs.subtitle_position.commands() {
        player.send(command).map_err(std::io::Error::other)?;
    }
    for command in prefs.subtitle_other.commands() {
        player.send(command).map_err(std::io::Error::other)?;
    }
    crate::fullscreen_info::connect(app);
    if native.is_some() {
        app.set_engine_ready(true);
        player
            .send(Command::Volume(prefs.volume))
            .map_err(std::io::Error::other)?;
        if let Some(path) = media {
            media_open::startup(app, path, player.command_sender());
        }
        return Ok(UiSession {
            player: Some(player),
            _presentation: native,
            _hdr_display_timer: hdr_display_timer,
            _live_text_timer: live_text_timer,
            _appearance_timer: appearance_timer,
            prefs_path,
            prefs,
            recent_media,
            history_cleared,
            latest_playback,
        });
    }
    let pending_media = Rc::new(RefCell::new(media));
    let render_weak = app.as_weak();
    let render_player = player.clone();
    let mut surface: Option<VideoSurface> = None;
    let render_generation = Rc::new(Cell::new(0_u64));
    let mut started = false;
    let repaint_pending = Arc::new(AtomicBool::new(false));
    let volume = prefs.volume;
    app.window()
        .set_rendering_notifier(move |state, graphics_api| {
            match state {
                slint::RenderingState::RenderingSetup => {
                    render_generation.set(render_generation.get().wrapping_add(1));
                    if let slint::GraphicsAPI::NativeOpenGL { get_proc_address } = graphics_api {
                        let weak = render_weak.clone();
                        let pending = repaint_pending.clone();
                        // SAFETY: notifier guarantees that this GL context is current.
                        match unsafe {
                            VideoSurface::new(
                                render_player.clone(),
                                &get_proc_address,
                                Box::new(move || {
                                    if pending.swap(true, Ordering::AcqRel) {
                                        return;
                                    }
                                    let weak = weak.clone();
                                    let pending = pending.clone();
                                    let _ = slint::invoke_from_event_loop(move || {
                                        pending.store(false, Ordering::Release);
                                        if let Some(app) = weak.upgrade() {
                                            app.window().request_redraw();
                                        }
                                    });
                                }),
                            )
                        } {
                            Ok(video) => {
                                surface = Some(video);
                                if let Some(app) = render_weak.upgrade() {
                                    app.set_engine_ready(true);
                                }
                            }
                            Err(error) => {
                                if let Some(app) = render_weak.upgrade() {
                                    app.set_status(
                                        format!("Video initialization failed: {error}").into(),
                                    );
                                }
                            }
                        }
                    }
                }
                slint::RenderingState::BeforeRendering => {
                    if let (Some(video), Some(app)) = (surface.as_mut(), render_weak.upgrade()) {
                        let scale = app.window().scale_factor();
                        let width = (app.get_render_width() * scale).clamp(1.0, 8192.0) as u32;
                        let height = (app.get_render_height() * scale).clamp(1.0, 8192.0) as u32;
                        // SAFETY: the notifier uses the same context throughout its lifetime.
                        match unsafe { video.render(width, height) } {
                            Ok(()) => {
                                let image = unsafe {
                                    slint::BorrowedOpenGLTextureBuilder::new_gl_2d_rgba_texture(
                                        video.texture_id(),
                                        (width, height).into(),
                                    )
                                    .build()
                                };
                                app.set_video_frame(image);
                                if !started {
                                    started = true;
                                    let _ = render_player.send(Command::Volume(volume));
                                    if let Some(path) = pending_media.borrow_mut().take() {
                                        media_open::startup(
                                            &app,
                                            path,
                                            render_player.command_sender(),
                                        );
                                    }
                                }
                            }
                            Err(error) => {
                                app.set_status(format!("Video rendering failed: {error}").into())
                            }
                        }
                    }
                }
                slint::RenderingState::RenderingTeardown => {
                    drop(surface.take());
                    // Context loss also needs UI invalidation, but only after
                    // the backend releases its mutable window borrow. A later
                    // setup must not have its new frame cleared by stale work.
                    let retired_generation = render_generation.get();
                    let generation = render_generation.clone();
                    let weak = render_weak.clone();
                    slint::Timer::single_shot(Duration::ZERO, move || {
                        if generation.get() == retired_generation
                            && let Some(app) = weak.upgrade()
                        {
                            app.set_video_frame(slint::Image::default());
                            app.set_engine_ready(false);
                        }
                    });
                }
                _ => {}
            }
        })?;
    Ok(UiSession {
        player: Some(player),
        _presentation: None,
        _hdr_display_timer: None,
        _live_text_timer: live_text_timer,
        _appearance_timer: appearance_timer,
        prefs_path,
        prefs,
        recent_media,
        history_cleared,
        latest_playback,
    })
}

#[cfg(test)]
mod welcome_tests {
    use super::{
        recent_document_rows, recent_media_rows, resume_engine_options, resume_media_rows,
    };
    use riina_core::RecentMedia;

    #[test]
    fn resume_item_is_not_repeated_in_recent_documents() {
        let history = [
            RecentMedia {
                path: "resume.mkv".into(),
                title: "Resume item".into(),
                position: 12.0,
            },
            RecentMedia {
                path: "older.mkv".into(),
                title: "Older item".into(),
                position: 3.0,
            },
        ];

        let resume = recent_media_rows(&history);
        let resume_action = resume_media_rows(&history, true);
        let documents = recent_document_rows(&history, true);

        assert_eq!(resume[0].title.as_str(), "Resume item");
        assert_eq!(resume_action[0].title.as_str(), "Resume item");
        assert_eq!(documents.len(), 1);
        assert_eq!(documents[0].title.as_str(), "Older item");

        assert!(resume_media_rows(&history, false).is_empty());
        let all_documents = recent_document_rows(&history, false);
        assert_eq!(all_documents.len(), 2);
        assert_eq!(all_documents[0].title.as_str(), "Resume item");
    }

    #[test]
    fn startup_options_configure_resume_and_subtitle_autoload() {
        let directory = std::path::Path::new("profile/watch-later");
        let enabled = resume_engine_options(
            directory,
            true,
            &riina_core::subtitle_controls::AutoLoadPreferences::default(),
            &riina_core::network_preferences::Connection::default(),
            &riina_core::network_preferences::Cache::default(),
            &riina_core::network_preferences::OnlineVideos::default(),
        )
        .unwrap();
        assert_eq!(enabled[0].0, "watch-later-directory");
        assert_eq!(enabled[0].1, directory.to_string_lossy());
        assert_eq!(enabled[1], ("save-position-on-quit".into(), "yes".into()));
        assert_eq!(enabled[2], ("resume-playback".into(), "yes".into()));
        assert_eq!(enabled[3], ("sub-auto".into(), "exact".into()));
        assert_eq!(enabled[4], ("sub-file-paths".into(), "".into()));
        assert_eq!(enabled[5], ("rtsp-transport".into(), "tcp".into()));
        assert_eq!(enabled[6], ("cache".into(), "yes".into()));
        assert_eq!(enabled[9], ("cache-secs".into(), "36000".into()));
        assert_eq!(enabled[11], ("ytdl".into(), "yes".into()));
        assert_eq!(enabled[12], ("ytdl-raw-options".into(), "".into()));

        let disabled = resume_engine_options(
            directory,
            false,
            &riina_core::subtitle_controls::AutoLoadPreferences::default(),
            &riina_core::network_preferences::Connection::default(),
            &riina_core::network_preferences::Cache::default(),
            &riina_core::network_preferences::OnlineVideos::default(),
        )
        .unwrap();
        assert_eq!(disabled[1].1, "no");
        assert_eq!(disabled[2].1, "no");
    }
}
