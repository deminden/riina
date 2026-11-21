use super::filters::FilterChain;
use super::{AbLoopPoint, Command, CommandPlan, cmd, flag, set};
use crate::{
    LoopMode, Phase, Playback, PlaylistEntry, TrackKind, audio_equalizer, crop, delogo,
    hdr_preferences, key_bindings, playlist, screenshot, subtitle_controls, valid_path,
    video_viewport,
};

impl Command {
    #[expect(
        clippy::too_many_lines,
        reason = "decomposition tracked in docs/review"
    )]
    pub fn plan(&self, state: &Playback) -> Result<CommandPlan, String> {
        let mut resume_on_load = false;
        if matches!(self, Self::CaptureFrame) && state.phase != Phase::Paused {
            return Err("Text recognition is available only while video is paused".into());
        }
        if matches!(
            self,
            Self::CaptureTransitionFrame
                | Self::CaptureIncomingTransitionFrame
                | Self::CaptureMaterialFrame
        ) && (state.video.is_none()
            || matches!(state.phase, Phase::Loading | Phase::Stopped | Phase::Failed))
        {
            return Err("A transition image requires an active video".into());
        }
        let commands = match self {
            Self::ObserveMpvProperty(property) if valid_mpv_property_name(property) => Vec::new(),
            Self::ObserveMpvProperty(_) => {
                return Err("Invalid mpv property name".into());
            }
            Self::MpvInputAction(action) => key_bindings::mpv_action_commands(action)?,
            Self::AudioEqualizer(gains) => {
                let filter = audio_equalizer::lavfi_filter(gains).map_err(str::to_owned)?;
                let mut commands = Vec::new();
                if state
                    .audio_equalizer
                    .is_some_and(|current| current.iter().any(|gain| *gain != 0.0))
                {
                    commands.push(vec![
                        "af".into(),
                        "remove".into(),
                        format!("@{}", audio_equalizer::FILTER_LABEL),
                    ]);
                }
                if let Some(filter) = filter {
                    commands.push(vec!["af".into(), "add".into(), filter]);
                }
                commands
            }
            Self::SubtitleLetterboxMargins(enabled) => {
                let value = flag(*enabled);
                vec![
                    set("sub-use-margins", value),
                    set("sub-ass-force-margins", value),
                ]
            }
            Self::SubtitleEncoding(encoding) => {
                if !subtitle_controls::SUBTITLE_ENCODING_CHOICES
                    .iter()
                    .any(|(_, code)| *code == encoding)
                {
                    return Err("Unsupported subtitle encoding".into());
                }
                let mut commands = vec![set("sub-codepage", encoding.clone())];
                commands.extend(
                    state
                        .tracks
                        .iter()
                        .filter(|track| track.kind == TrackKind::Sub)
                        .map(|track| vec!["sub-reload".into(), track.id.to_string()]),
                );
                commands
            }
            Self::SubtitleAutoLoad(preferences) => {
                preferences.validate().map_err(str::to_owned)?;
                let mut commands = vec![set("sub-auto", preferences.mpv_matching_mode())];
                commands.push(set(
                    "sub-file-paths",
                    preferences.mpv_search_paths().unwrap_or_default(),
                ));
                commands
            }
            Self::TogglePause | Self::SetPaused(false) if state.phase == Phase::Ended => {
                replay_from_eof()
            }
            Self::NetworkCache(preferences) => preferences.commands()?,
            Self::NetworkOnlineVideos(preferences) => preferences
                .engine_options()?
                .into_iter()
                .map(|(name, value)| set(&name, value))
                .collect(),
            Self::Screenshot {
                directory,
                content,
                encoding,
            } => {
                if !valid_path(directory) || encoding.template.contains('\0') {
                    return Err(
                        "Screenshot directory is empty or settings contain a NUL byte".into(),
                    );
                }
                vec![
                    set("screenshot-format", encoding.format.native_value()),
                    set("screenshot-template", encoding.template.clone()),
                    // Match the reference's JXL software path without changing
                    // screenshot conversion policy for the other formats.
                    set(
                        "screenshot-sw",
                        if encoding.format == screenshot::Format::Jxl {
                            "yes"
                        } else {
                            "no"
                        },
                    ),
                    // Keep the native trailing separator: mixed '/' in a
                    // verbatim Windows path is rejected by screenshot writing.
                    set(
                        "screenshot-dir",
                        std::path::Path::new(directory)
                            .join("")
                            .to_string_lossy()
                            .into_owned(),
                    ),
                    vec!["screenshot".into(), content.native_value().into()],
                ]
            }
            Self::VideoViewport(viewport) => video_viewport::Viewport::commands(*viewport),
            Self::VideoReflection { axis, enabled } => axis.commands(*enabled, state)?,
            Self::EnterCrop(session) => session.enter_commands(state)?,
            Self::ExitCrop { session, rectangle } => session.exit_commands(state, *rectangle)?,
            Self::RemoveDelogo => {
                let filters = state
                    .video_filters
                    .as_ref()
                    .ok_or("Video filters are unavailable")?;
                if filters
                    .iter()
                    .any(|filter| filter.label.as_deref() == Some(delogo::FILTER_LABEL))
                {
                    delogo::remove_filter()
                } else {
                    Vec::new()
                }
            }
            Self::ToggleVideoFilter(filter) => FilterChain::Video.toggle(state, filter)?,
            Self::AddVideoFilter(filter) => FilterChain::Video.add(filter)?,
            Self::RemoveVideoFilter(filter) => FilterChain::Video.remove(state, filter)?,
            Self::SetVideoFilterAt { index, filter } => {
                FilterChain::Video.replace(state, *index, filter)?
            }
            Self::ToggleAudioFilter(filter) => FilterChain::Audio.toggle(state, filter)?,
            Self::AddAudioFilter(filter) => FilterChain::Audio.add(filter)?,
            Self::RemoveAudioFilter(filter) => FilterChain::Audio.remove(state, filter)?,
            Self::SetAudioFilterAt { index, filter } => {
                FilterChain::Audio.replace(state, *index, filter)?
            }
            Self::VideoCropRectangle(rectangle) => {
                let source = state
                    .video
                    .as_ref()
                    .ok_or("Source video dimensions are unavailable")?;
                if rectangle.x == 0
                    && rectangle.y == 0
                    && rectangle.width != 0
                    && rectangle.height != 0
                    && rectangle.width == source.w
                    && rectangle.height == source.h
                {
                    return Self::VideoCrop(None).plan(state);
                }
                vec![vec![
                    "vf".into(),
                    "add".into(),
                    rectangle.editor_filter(source.w, source.h)?,
                ]]
            }
            Self::VideoCrop(preset) => match preset {
                Some(preset) => {
                    let source = state
                        .video
                        .as_ref()
                        .ok_or("Source video dimensions are unavailable")?;
                    vec![vec![
                        "vf".into(),
                        "add".into(),
                        preset.filter(source.w, source.h)?,
                    ]]
                }
                None => {
                    let filters = state
                        .video_filters
                        .as_ref()
                        .ok_or("Video filters are unavailable")?;
                    if filters
                        .iter()
                        .any(|filter| filter.label.as_deref() == Some(crop::FILTER_LABEL))
                    {
                        vec![vec![
                            "vf".into(),
                            "remove".into(),
                            format!("@{}", crop::FILTER_LABEL),
                        ]]
                    } else {
                        Vec::new()
                    }
                }
            },
            Self::AdjustVolume { delta, boost } => {
                if !delta.is_finite() {
                    return Err("The volume adjustment must be finite".into());
                }
                volume_adjustment(*delta, *boost)
            }
            Self::SetVolumeBoost(boost) => volume_adjustment(0.0, *boost),
            Self::SetPauseWhenOpen(_) => Vec::new(),
            Self::Stop => {
                let active = matches!(
                    state.phase,
                    Phase::Playing | Phase::Paused | Phase::Buffering | Phase::Seeking
                );
                let mut commands = Vec::with_capacity(2);
                if active && state.save_position_on_quit == Some(true) {
                    commands.push(cmd(["write-watch-later-config"]));
                }
                commands.push(cmd(["stop"]));
                commands
            }
            Self::SetResumeLastPosition(enabled) => {
                let value = flag(*enabled);
                vec![
                    set("save-position-on-quit", value),
                    set("resume-playback", value),
                ]
            }
            Self::SetAbLoop { a, b } => {
                let point = |value: Option<f64>| -> Result<String, String> {
                    match value {
                        None => Ok("no".into()),
                        Some(value) if value.is_finite() && value >= 0.0 => {
                            // The pinned IINA contract preserves a point at the
                            // beginning using its minimum nonzero loop timestamp.
                            Ok(value.max(0.000001).to_string())
                        }
                        _ => Err("Invalid A–B loop point".into()),
                    }
                };
                let (a, b) = (point(*a)?, point(*b)?);
                vec![set("ab-loop-a", a), set("ab-loop-b", b)]
            }
            Self::SetAbLoopPoint { point, value } => {
                let name = match point {
                    AbLoopPoint::A => "ab-loop-a",
                    AbLoopPoint::B => "ab-loop-b",
                };
                let value = match value {
                    None => "no".to_owned(),
                    Some(value) if value.is_finite() && *value >= 0.0 => {
                        value.max(0.000001).to_string()
                    }
                    _ => return Err("Invalid A–B loop point".into()),
                };
                vec![set(name, value)]
            }
            Self::SetToneMapping(settings) => settings.engine_commands()?,
            Self::SetIccProfileAuto(enabled) => vec![set("icc-profile-auto", flag(*enabled))],
            Self::SetHdrEnabled(enabled) => {
                let primaries = if *enabled {
                    let source = state
                        .video
                        .as_ref()
                        .or(state.video_target.as_ref())
                        .or(state.video_output.as_ref())
                        .filter(|video| video.is_hdr())
                        .ok_or("HDR output is available only for a recognized HDR source")?;
                    source.primaries.as_deref()
                } else {
                    None
                };
                hdr_preferences::output_commands(*enabled, primaries)?
            }
            Self::SetHdrEnabledWithIcc {
                enabled,
                load_icc_profile,
            } => {
                let primaries = if *enabled {
                    let source = state
                        .video
                        .as_ref()
                        .or(state.video_target.as_ref())
                        .or(state.video_output.as_ref())
                        .filter(|video| video.is_hdr())
                        .ok_or("HDR output is available only for a recognized HDR source")?;
                    source.primaries.as_deref()
                } else {
                    None
                };
                hdr_preferences::output_commands_with_icc(*enabled, primaries, *load_icc_profile)?
            }
            Self::VideoThreads(threads) if *threads <= i32::MAX as u32 => {
                vec![set("vd-lavc-threads", threads.to_string())]
            }
            Self::VideoThreads(_) => {
                return Err("Video decoder threads must be between 0 and 2,147,483,647".into());
            }
            Self::TogglePlaylistShuffle => vec![vec![
                if state.playlist_shuffled {
                    "playlist-unshuffle"
                } else {
                    "playlist-shuffle"
                }
                .into(),
            ]],
            Self::SortPlaylist(sort) => playlist_reorder_commands(&state.playlist, *sort)?,
            Self::SelectPlaylistId(id)
            | Self::RemovePlaylistId(id)
            | Self::MovePlaylistId { id, .. } => {
                // Stable native IDs survive inserts/reorders while a UI intent is queued.
                let index = state
                    .playlist
                    .iter()
                    .position(|entry| entry.id == *id)
                    .ok_or("The playlist entry is no longer available")?;
                let command = match self {
                    Self::SelectPlaylistId(_) => Self::SelectPlaylistEntry(index),
                    Self::RemovePlaylistId(_) => Self::RemovePlaylistEntry(index),
                    Self::MovePlaylistId { direction: -1, .. } if index > 0 => {
                        Self::MovePlaylistEntry {
                            from: index,
                            to: index - 1,
                        }
                    }
                    Self::MovePlaylistId { direction: 1, .. }
                        if index + 1 < state.playlist.len() =>
                    {
                        Self::MovePlaylistEntry {
                            from: index,
                            to: index + 2,
                        }
                    }
                    Self::MovePlaylistId { .. } => {
                        return Err("The playlist entry cannot move in that direction".into());
                    }
                    _ => unreachable!(),
                };
                return command.plan(state);
            }
            Self::RemovePlaylistIds(ids) => {
                let mut indices = Vec::with_capacity(ids.len());
                for id in ids {
                    let Some(index) = state.playlist.iter().position(|entry| entry.id == *id)
                    else {
                        return Err("A selected playlist entry is no longer available".into());
                    };
                    indices.push(index);
                }
                indices.sort_unstable();
                if indices.windows(2).any(|pair| pair[0] == pair[1]) {
                    return Err("A playlist entry was selected more than once".into());
                }
                // IINA removes ascending index sets while accounting for each prior
                // removal; stable IDs are resolved before any native row can shift.
                indices
                    .into_iter()
                    .enumerate()
                    .map(|(removed, index)| {
                        vec!["playlist-remove".into(), (index - removed).to_string()]
                    })
                    .collect()
            }
            Self::OpenFiles(paths)
            | Self::AppendFiles(paths)
            | Self::OpenFilesWithSubtitles { paths, .. } => {
                if paths.is_empty() || paths.iter().any(|path| !valid_path(path)) {
                    return Err("No valid files were supplied".into());
                }
                if let Self::OpenFilesWithSubtitles { subtitles, .. } = self
                    && (subtitles.is_empty()
                        || subtitles.len() > 64
                        || subtitles.iter().any(|path| !valid_path(path)))
                {
                    return Err("Invalid dropped subtitle files were supplied".into());
                }
                let mut commands: Vec<Vec<String>> = paths
                    .iter()
                    .map(|path| vec!["loadfile".into(), path.clone(), "append".into()])
                    .collect();
                if matches!(
                    self,
                    Self::OpenFiles(_) | Self::OpenFilesWithSubtitles { .. }
                ) {
                    // PlayerCore.openURLs opens the first playable item, then appends
                    // the remainder. Validate everything before replacing old media.
                    commands[0][2] = "replace".into();
                    commands.insert(0, set("pause", flag(state.pause_when_open)));
                    // HDR overrides target the current source's primaries. Clear
                    // them before replacement so the next source inherits auto/ICC.
                    let hdr_reset = hdr_preferences::output_commands(false, None)?;
                    commands.splice(0..0, hdr_reset);
                }
                commands
            }
            Self::InsertPlaylistFiles { paths, index } => {
                if paths.is_empty()
                    || paths.iter().any(|path| !valid_path(path))
                    || *index > state.playlist.len()
                {
                    return Err("Playlist items or insertion position are invalid".into());
                }
                let first_new = state.playlist.len();
                let mut commands = Vec::with_capacity(paths.len() * 2);
                for (offset, path) in paths.iter().enumerate() {
                    commands.push(vec!["loadfile".into(), path.clone(), "append".into()]);
                    let appended_index = first_new + offset;
                    let destination = index + offset;
                    if appended_index != destination {
                        commands.push(vec![
                            "playlist-move".into(),
                            appended_index.to_string(),
                            destination.to_string(),
                        ]);
                    }
                }
                commands
            }
            Self::InsertPlaylistFilesAroundCurrent { before, after } => {
                if before.is_empty() && after.is_empty()
                    || before.iter().chain(after).any(|path| !valid_path(path))
                {
                    return Err("No valid folder media files were supplied".into());
                }
                let current_index = state
                    .playlist
                    .iter()
                    .position(|entry| entry.current)
                    .ok_or_else(|| "The current playlist item is no longer available".to_owned())?;
                let first_new = state.playlist.len();
                let mut commands = Vec::with_capacity(before.len() * 2 + after.len());
                for (offset, path) in before.iter().enumerate() {
                    commands.push(vec!["loadfile".into(), path.clone(), "append".into()]);
                    let appended_index = first_new + offset;
                    let destination = current_index + offset;
                    if appended_index != destination {
                        commands.push(vec![
                            "playlist-move".into(),
                            appended_index.to_string(),
                            destination.to_string(),
                        ]);
                    }
                }
                commands.extend(
                    after
                        .iter()
                        .map(|path| vec!["loadfile".into(), path.clone(), "append".into()]),
                );
                commands
            }
            Self::Load(_) | Self::LoadAtPosition { .. } => {
                let mut commands = hdr_preferences::output_commands(false, None)?;
                commands.extend([set("pause", flag(state.pause_when_open)), self.mpv_args()?]);
                commands
            }
            Self::SelectChapter(index) => {
                let chapter = state
                    .chapters
                    .get(*index)
                    .ok_or("The chapter is no longer available")?;
                if !chapter.time.is_finite() || chapter.time < 0.0 {
                    return Err("Invalid chapter time".into());
                }
                // PlayerCore.playChapter seeks to the observed time and always resumes.
                vec![
                    vec!["seek".into(), chapter.time.to_string(), "absolute".into()],
                    set("pause", "no"),
                ]
            }
            Self::SelectPlaylistEntry(index) => {
                if *index >= state.playlist.len() {
                    return Err("The playlist entry is no longer available".into());
                }
                resume_on_load = true;
                vec![set("pause", "yes"), self.mpv_args()?]
            }
            Self::PlaylistPrevious if state.playlist.first().is_some_and(|entry| entry.current) => {
                vec![cmd(["seek", "0", "absolute"])]
            }
            Self::PlaylistNext | Self::PlaylistPrevious => {
                resume_on_load = true;
                vec![set("pause", "yes"), self.mpv_args()?]
            }
            Self::RemovePlaylistEntry(index) if *index >= state.playlist.len() => {
                return Err("The playlist entry is no longer available".into());
            }
            Self::MovePlaylistEntry { from, to }
                if *from >= state.playlist.len() || *to > state.playlist.len() =>
            {
                return Err("The playlist entry is no longer available".into());
            }
            Self::AddSubtitle(path) => {
                if !valid_path(path) {
                    return Err("Invalid subtitle path".into());
                }
                // Reload an existing external file to avoid duplicate tracks, as IINA does.
                if let Some(track) = state.tracks.iter().find(|track| {
                    track.kind == TrackKind::Sub
                        && track
                            .external_filename
                            .as_deref()
                            .is_some_and(|existing| same_external_path(existing, path))
                }) {
                    vec![vec!["sub-reload".into(), track.id.to_string()]]
                } else {
                    vec![self.mpv_args()?]
                }
            }
            Self::AddSubtitlesWithoutSelection(paths) => {
                const MAX_SUBTITLES_PER_SCAN: usize = 64;
                if paths.len() > MAX_SUBTITLES_PER_SCAN {
                    return Err("Subtitle search returned too many matching files".into());
                }
                let mut added = Vec::<&str>::new();
                let mut commands = Vec::new();
                for path in paths {
                    if !valid_path(path) {
                        return Err("Invalid subtitle path".into());
                    }
                    let exists = state.tracks.iter().any(|track| {
                        track.kind == TrackKind::Sub
                            && track
                                .external_filename
                                .as_deref()
                                .is_some_and(|existing| same_external_path(existing, path))
                    }) || added
                        .iter()
                        .any(|existing| same_external_path(existing, path));
                    if !exists {
                        added.push(path);
                        commands.push(vec!["sub-add".into(), path.clone(), "auto".into()]);
                    }
                }
                commands
            }
            Self::CycleLoop => set_loop_mode_commands(state.loop_mode().next()),
            Self::SetLoopMode(mode) => set_loop_mode_commands(*mode),
            _ => vec![self.mpv_args()?],
        };
        Ok(CommandPlan {
            commands,
            resume_on_load,
        })
    }
}
fn replay_from_eof() -> Vec<Vec<String>> {
    // Changing the pause flag at EOF leaves mpv on the last frame.
    vec![cmd(["seek", "0", "absolute+exact"]), set("pause", "no")]
}

fn set_loop_mode_commands(mode: LoopMode) -> Vec<Vec<String>> {
    match mode {
        LoopMode::Off => vec![set("loop-playlist", "no"), set("loop-file", "no")],
        LoopMode::File => vec![set("loop-file", "inf")],
        LoopMode::Playlist => vec![set("loop-playlist", "inf"), set("loop-file", "no")],
    }
}

fn valid_mpv_property_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 256
        && !name.contains('.')
        && !name.chars().any(char::is_control)
}

fn volume_adjustment(delta: f64, boost: bool) -> Vec<Vec<String>> {
    // Native add clamps against the active range and uses the engine's current
    // volume. A zero adjustment also clamps when the user turns boost off.
    vec![
        set("volume-max", if boost { "200" } else { "100" }),
        vec!["add".into(), "volume".into(), delta.to_string()],
    ]
}

fn same_external_path(left: &str, right: &str) -> bool {
    #[cfg(target_os = "windows")]
    {
        fn identity(path: &str) -> (bool, &str) {
            if let Some(path) = path.strip_prefix(r"\\?\UNC\") {
                (true, path)
            } else if let Some(path) = path.strip_prefix(r"\\?\") {
                (false, path)
            } else if let Some(path) = path.strip_prefix(r"\\") {
                (true, path)
            } else {
                (false, path)
            }
        }
        // mpv may omit the verbatim prefix returned by Windows canonicalize.
        // Compare that spelling equivalently without filesystem I/O or treating
        // case-sensitive names, aliases or different volumes as the same file.
        identity(left) == identity(right)
    }
    #[cfg(not(target_os = "windows"))]
    {
        left == right
    }
}

fn playlist_reorder_commands(
    entries: &[PlaylistEntry],
    sort: playlist::PlaylistSort,
) -> Result<Vec<Vec<String>>, String> {
    let desired = playlist::sorted_ids(entries, sort);
    let mut current = entries.iter().map(|entry| entry.id).collect::<Vec<_>>();
    let mut commands = Vec::new();
    for (target, id) in desired.into_iter().enumerate() {
        let from = current
            .iter()
            .position(|candidate| *candidate == id)
            .ok_or("The playlist changed before sorting")?;
        if from == target {
            continue;
        }
        // mpv's destination is an insertion position before removal; increment
        // downward moves to leave the item at its requested final index.
        let destination = if target > from { target + 1 } else { target };
        commands.push(vec![
            "playlist-move".into(),
            from.to_string(),
            destination.to_string(),
        ]);
        let moved = current.remove(from);
        current.insert(target, moved);
    }
    Ok(commands)
}
