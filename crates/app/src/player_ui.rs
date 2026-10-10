use crate::seek_osd::{self, Change, SeekOsd};
use crate::{
    CatalogRow, PlayerWindow, media_open, platform,
    ui_dialogs::{self, Purpose},
};
use riina_core::{Command, LoopMode, Phase, Playback, Preferences, TrackKind, time_label};
use riina_mpv::{Player, video::VideoSurface};
use slint::ComponentHandle;
use std::{
    cell::RefCell,
    path::PathBuf,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

fn update_catalog(app: &PlayerWindow, state: &Playback) {
    for kind in [TrackKind::Audio, TrackKind::Sub] {
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
        rows.extend(tracks.into_iter().map(|track| {
            CatalogRow {
                token: track.id.to_string().into(),
                label: track.display_name().into(),
                language: track
                    .lang
                    .as_deref()
                    .filter(|lang| *lang != "und")
                    .unwrap_or("")
                    .into(),
                selected: track.is_primary_selected(),
            }
        }));
        let model = slint::ModelRc::new(slint::VecModel::from(rows));
        if kind == TrackKind::Audio {
            app.set_audio_tracks(model);
        } else {
            app.set_subtitle_tracks(model);
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
    secondary_rows.extend(subtitle_tracks.into_iter().map(|track| {
        CatalogRow {
            token: track.id.to_string().into(),
            label: track.display_name().into(),
            language: track
                .lang
                .as_deref()
                .filter(|lang| *lang != "und")
                .unwrap_or("")
                .into(),
            selected: track.selected && track.main_selection == Some(1),
        }
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
) {
    match last_seek.observe(&state) {
        Change::Hide => app.set_seek_osd_visible(false),
        Change::Show => app.invoke_show_seek_osd(),
        Change::Keep => {}
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
    app.set_audio_delay(state.audio_delay as f32);
    app.set_subtitle_delay(state.subtitle_delay as f32);
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
    app.set_osd_enabled(prefs.enable_osd);
    app.set_primary_shortcut_is_meta(cfg!(target_os = "macos"));
    let osd_timeout = Duration::try_from_secs_f64(prefs.osd_timeout_secs)?;
    let app_weak = app.as_weak();
    // Only one pending state snapshot is retained; a decoder can produce updates
    // faster than the UI consumes them without growing the application event queue.
    let latest = Arc::new(std::sync::Mutex::new(None));
    let queued = Arc::new(AtomicBool::new(false));
    let displayed = Arc::new(std::sync::Mutex::new(None));
    let last_seek = Arc::new(std::sync::Mutex::new(SeekOsd::default()));
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
        let _ = slint::invoke_from_event_loop(move || {
            queued.store(false, Ordering::Release);
            if let (Some(app), Ok(mut latest)) = (weak.upgrade(), latest.lock())
                && let Some(state) = latest.take()
                && let Ok(mut displayed) = displayed.lock()
                && let Ok(mut last_seek) = last_seek.lock()
            {
                update_ui(&app, state, &mut displayed, &mut last_seek);
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
    let seek_timer = slint::Timer::default();
    let weak = app.as_weak();
    app.on_show_seek_osd(move || {
        if let Some(app) = weak.upgrade() {
            if !app.get_osd_enabled() {
                return;
            }
            app.set_seek_osd_visible(true);
        }
        let weak = weak.clone();
        seek_timer.start(slint::TimerMode::SingleShot, osd_timeout, move || {
            if let Some(app) = weak.upgrade() {
                app.set_seek_osd_visible(false);
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
    connect!(on_reset_subtitle_delay, Command::SubtitleDelay(0.0));
    let scale_player = player.command_sender();
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
    let secondary_position_player = player.command_sender();
    let weak = app.as_weak();
    app.on_set_secondary_subtitle_position(move |value| {
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
            if let Err(error) =
                secondary_position_player.send(Command::SecondarySubtitlePosition(value as f64))
            {
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
            1 | 2 => token
                .as_str()
                .parse::<i64>()
                .map(Some)
                .or_else(|error| if token == "no" { Ok(None) } else { Err(error) })
                .map(|id| Command::SelectTrack {
                    kind: if category == 1 {
                        TrackKind::Audio
                    } else {
                        TrackKind::Sub
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
            slint::CloseRequestResponse::HideWindow
        }
    });

    player
        .send(Command::SetVolumeBoost(prefs.volume_boost))
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
    let mut started = false;
    let repaint_pending = Arc::new(AtomicBool::new(false));
    let volume = prefs.volume;
    app.window()
        .set_rendering_notifier(move |state, graphics_api| {
            match state {
                slint::RenderingState::RenderingSetup => {
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
                        let width = (app.get_video_width() * scale).clamp(1.0, 8192.0) as u32;
                        let height = (app.get_video_height() * scale).clamp(1.0, 8192.0) as u32;
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
                    if let Some(app) = render_weak.upgrade() {
                        app.set_video_frame(slint::Image::default());
                        app.set_engine_ready(false);
                    }
                    drop(surface.take());
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
