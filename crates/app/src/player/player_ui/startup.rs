//! Build initial engine options and issue the saved player commands in startup order.
use super::engine_start::EngineOptions;
use riina_core::{Command, Preferences};
use riina_mpv::CommandSender;
use std::path::Path;

pub(super) struct OptionsFlags {
    pub(super) ytdl_plugin_active: bool,
    pub(super) platform_manages_icc: bool,
}

pub(super) fn engine_options(
    prefs: &Preferences,
    watch_later_directory: &Path,
    flags: OptionsFlags,
    advanced_options: Result<EngineOptions, String>,
) -> Result<EngineOptions, String> {
    let mut network_online_videos = prefs.network_online_videos.clone();
    if flags.ytdl_plugin_active {
        network_online_videos.enabled = false;
    }
    let mut engine_options = resume_engine_options(
        watch_later_directory,
        prefs.resume_last_position,
        &prefs.subtitle_auto_load,
        &prefs.network,
        &prefs.network_cache,
        &network_online_videos,
    )?;
    engine_options.push((
        "keep-open".into(),
        riina_core::playlist_end_policy(prefs.keep_open_on_file_end, prefs.playlist_auto_play_next)
            .into(),
    ));
    engine_options.extend(prefs.audio.engine_options()?);
    engine_options.push(("vd-lavc-threads".into(), prefs.video_threads.to_string()));
    engine_options.extend(prefs.hdr_tone_mapping.engine_options()?);
    if !prefs.hdr_enabled {
        engine_options.extend(riina_core::hdr_preferences::disabled_output_options());
    }
    if flags.platform_manages_icc {
        engine_options.retain(|(name, _)| name != "icc-profile-auto");
        engine_options.push(("icc-profile-auto".into(), "no".into()));
    }
    engine_options.extend(advanced_options?);
    Ok(engine_options)
}

pub(super) fn resume_engine_options(
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

pub(super) fn apply_player_preferences(
    command_sender: &CommandSender,
    prefs: &Preferences,
) -> std::io::Result<()> {
    if prefs
        .audio_equalizer
        .gains_db
        .iter()
        .any(|gain| *gain != 0.0)
    {
        command_sender
            .send(Command::AudioEqualizer(prefs.audio_equalizer.gains_db))
            .map_err(std::io::Error::other)?;
    }
    command_sender
        .send(Command::SetVolumeBoost(prefs.volume_boost))
        .map_err(std::io::Error::other)?;
    command_sender
        .send(Command::SetPauseWhenOpen(prefs.pause_when_open))
        .map_err(std::io::Error::other)?;
    command_sender
        .send(Command::HardwareDecoding(prefs.hardware_decoder))
        .map_err(std::io::Error::other)?;
    command_sender
        .send(Command::SubtitleAssOverride {
            lane: riina_core::subtitle_controls::SubtitleLane::Primary,
            level: prefs.subtitle_ass_override,
        })
        .map_err(std::io::Error::other)?;
    command_sender
        .send(Command::SubtitleAssOverride {
            lane: riina_core::subtitle_controls::SubtitleLane::Secondary,
            level: prefs.secondary_subtitle_ass_override,
        })
        .map_err(std::io::Error::other)?;
    command_sender
        .send(Command::SubtitleFont(prefs.subtitle_font.clone()))
        .map_err(std::io::Error::other)?;
    command_sender
        .send(Command::SubtitleFontSize(prefs.subtitle_font_size))
        .map_err(std::io::Error::other)?;
    for command in crate::subtitle_preferences::commands(prefs.subtitle_text_style) {
        command_sender
            .send(command)
            .map_err(std::io::Error::other)?;
    }
    for command in prefs.subtitle_position.commands() {
        command_sender
            .send(command)
            .map_err(std::io::Error::other)?;
    }
    for command in prefs.subtitle_other.commands() {
        command_sender
            .send(command)
            .map_err(std::io::Error::other)?;
    }
    Ok(())
}
