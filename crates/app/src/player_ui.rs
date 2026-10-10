use crate::seek_osd::{self, Change, SeekOsd};
use crate::{
    CatalogRow, PlayerWindow, media_open, platform,
    ui_dialogs::{self, Purpose},
};
use riina_core::{Command, LoopMode, Phase, Playback, Preferences, TrackKind, time_label};
use riina_mpv::{Player, video::VideoSurface};
use slint::ComponentHandle;
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
            language: "".into(),
            selected: !tracks.iter().any(|track| track.is_primary_selected()),
        }];
        rows.extend(tracks.into_iter().map(|track| CatalogRow {
            token: track.id.to_string().into(),
            label: track.display_name().into(),
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
        language: "".into(),
        selected: !subtitle_tracks.iter().any(secondary_selected),
    }];
    secondary_rows.extend(subtitle_tracks.into_iter().map(|track| CatalogRow {
        token: track.id.to_string().into(),
        label: track.display_name().into(),
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
    app.set_playlist(slint::ModelRc::new(slint::VecModel::from(
        state
            .playlist
            .iter()
            .map(|entry| CatalogRow {
                token: entry.id.to_string().into(),
                language: "".into(),
                label: entry
                    .title
                    .clone()
                    .unwrap_or_else(|| {
                        PathBuf::from(&entry.filename).file_name().map_or_else(
                            || entry.filename.clone(),
                            |name| name.to_string_lossy().into_owned(),
                        )
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
    match last_seek.observe(&state) {
        Change::Hide => app.set_seek_osd_visible(false),
        Change::Show => app.invoke_show_seek_osd(),
        Change::Keep => {}
    }
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
                        app.set_screenshot_preview_path(path.into());
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
    let first_loaded = !app.get_loaded() && state.phase == Phase::Playing;
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
        app.invoke_activity();
    }
    app.set_seekable(state.seekable && state.duration.is_some());
    app.set_position(state.position.unwrap_or(0.0) as f32);
    app.set_duration(state.duration.unwrap_or(0.0) as f32);
    app.set_volume(state.volume as f32);
    app.set_muted(state.muted);
    app.set_speed_value(state.speed as f32);
    app.set_speed_slider_position(riina_core::speed_controls::slider_position(state.speed) as f32);
    app.set_speed_edit_value(riina_core::speed_controls::text(state.speed).into());
    app.set_video_aspect_known(state.video_aspect_override.is_some());
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
    app.set_subtitle_font(state.subtitle_font.clone().unwrap_or_default().into());
    app.set_subtitle_font_size(state.subtitle_font_size.unwrap_or(0.0) as f32);
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
    app.set_media_title(state.title.into());
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
    prefs_path: PathBuf,
    prefs: Preferences,
}

impl UiSession {
    pub(crate) fn save(&self, app: &PlayerWindow) -> Result<(), Box<dyn std::error::Error>> {
        let prefs = Preferences {
            volume: (app.get_volume() as f64).min(if app.get_volume_boost() {
                200.0
            } else {
                100.0
            }),
            volume_boost: app.get_volume_boost(),
            screenshot: crate::screenshot_preferences::current(app)
                .map_err(std::io::Error::other)?,
            ..self.prefs.clone()
        };
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

pub(crate) fn configure_player(
    app: &PlayerWindow,
    native: Option<platform::NativePresentation>,
    media: Option<std::ffi::OsString>,
) -> Result<UiSession, Box<dyn std::error::Error>> {
    let prefs_path = platform::preferences_path()?;
    let prefs = match std::fs::read_to_string(&prefs_path) {
        Ok(text) => Preferences::parse(&text).map_err(std::io::Error::other)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Preferences::default(),
        Err(error) => return Err(error.into()),
    };
    app.set_volume_boost(prefs.volume_boost);
    app.set_video_aspect_presets(slint::ModelRc::new(slint::VecModel::from(
        riina_core::video_controls::ASPECT_PRESETS
            .map(slint::SharedString::from)
            .to_vec(),
    )));
    app.set_osd_enabled(prefs.enable_osd);
    crate::screenshot_preferences::install(app, &prefs.screenshot);
    refresh_video_menu(app);
    app.set_video_crop_presets(slint::ModelRc::new(slint::VecModel::from(
        riina_core::crop::LABELS
            .map(slint::SharedString::from)
            .to_vec(),
    )));
    app.set_primary_shortcut_is_meta(cfg!(target_os = "macos"));
    let osd_timeout = Duration::try_from_secs_f64(prefs.osd_timeout_secs)?;
    let app_weak = app.as_weak();
    // Only one pending state snapshot is retained; a decoder can produce updates
    // faster than the UI consumes them without growing the application event queue.
    let latest = Arc::new(std::sync::Mutex::new(None));
    let queued = Arc::new(AtomicBool::new(false));
    let displayed = Arc::new(std::sync::Mutex::new(None));
    let crop_editor = crate::crop_editor::Editor::default();
    let observed_crop_editor = crop_editor.clone();
    let last_seek = Arc::new(std::sync::Mutex::new(SeekOsd::default()));
    let last_video = Arc::new(std::sync::Mutex::new(crate::video_osd::VideoOsd::new(
        prefs.disable_osd_speed,
    )));
    let last_screenshot = Arc::new(std::sync::Mutex::new(
        crate::screenshot_osd::ScreenshotOsd::default(),
    ));
    let notify = move |state| {
        if let Ok(mut latest) = latest.lock() {
            *latest = Some(state);
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
        let _ = slint::invoke_from_event_loop(move || {
            queued.store(false, Ordering::Release);
            if let (Some(app), Ok(mut latest)) = (weak.upgrade(), latest.lock())
                && let Some(state) = latest.take()
                && let Ok(mut displayed) = displayed.lock()
                && let Ok(mut last_seek) = last_seek.lock()
                && let Ok(mut last_video) = last_video.lock()
                && let Ok(mut last_screenshot) = last_screenshot.lock()
            {
                crop_editor.observe(&app, &state);
                update_ui(
                    &app,
                    state,
                    &mut displayed,
                    &mut last_seek,
                    &mut last_video,
                    &mut last_screenshot,
                );
            }
        });
    };
    let player = if let Some(presentation) = &native {
        presentation.start_engine(notify)
    } else {
        Player::start(&platform::playback_library(), false, notify)
    };
    let player = match player {
        Ok(player) => player,
        Err(error) => {
            app.set_status(error.clone().into());
            return Err(std::io::Error::other(error).into());
        }
    };
    let idle_timer = Rc::new(slint::Timer::default());
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
            platform::screenshot_directory(screenshot_settings.directory.as_deref())
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
    let disable_seek_osd = prefs.disable_osd_seek;
    let weak = app.as_weak();
    app.on_show_seek_osd(move || {
        if let Some(app) = weak.upgrade() {
            if !app.get_osd_enabled() || disable_seek_osd {
                return;
            }
            app.set_seek_osd_visible(true);
            clear_screenshot_preview(&app);
            app.set_video_osd_visible(false);
        }
        let weak = weak.clone();
        seek_timer.start(slint::TimerMode::SingleShot, osd_timeout, move || {
            if let Some(app) = weak.upgrade() {
                app.set_seek_osd_visible(false);
            }
        });
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
        if let Some(app) = weak.upgrade() {
            if !app.get_osd_enabled() {
                return;
            }
            app.set_seek_osd_visible(false);
            clear_screenshot_preview(&app);
            app.set_video_osd_visible(true);
        }
        let weak = weak.clone();
        video_timer.start(slint::TimerMode::SingleShot, osd_timeout, move || {
            if let Some(app) = weak.upgrade() {
                app.set_video_osd_visible(false);
            }
        });
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
    connect!(on_toggle_pause, Command::TogglePause);
    connect!(on_toggle_mute, Command::ToggleMute);
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
    let preferred_decoder = prefs.hardware_decoder;
    let weak = app.as_weak();
    app.on_set_hardware_decoding(move |enabled| {
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
            let mode = if enabled {
                preferred_decoder
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
    crate::font_picker::connect(app, player.command_sender());
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
            }
        }
    });
    let weak = app.as_weak();
    app.on_set_subtitle_font_size(move |value| {
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
            if let Err(error) = font_size_player.send(Command::SubtitleFontSize(value as f64)) {
                app.set_status(error.into());
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
            ui_dialogs::open(&app, browse_host.clone(), purpose, browse_player.clone());
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
    app.on_open_file(move || {
        if let Some(app) = weak.upgrade() {
            ui_dialogs::open(&app, open_host.clone(), Purpose::Media, open_player.clone());
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

    player
        .send(Command::SetVolumeBoost(prefs.volume_boost))
        .map_err(std::io::Error::other)?;
    player
        .send(Command::HardwareDecoding(prefs.hardware_decoder))
        .map_err(std::io::Error::other)?;
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
            prefs_path,
            prefs,
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
        prefs_path,
        prefs,
    })
}
