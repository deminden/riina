use super::{AbLoopPoint, Command, CommandPlan};
use crate::{
    LoopMode, Phase, Playback, PlaylistEntry, TrackKind, audio_equalizer, crop, delogo,
    hdr_preferences, key_bindings, playlist, screenshot, subtitle_controls, valid_path,
    video_filters, video_viewport,
};

impl Command {
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
                let value = if *enabled { "yes" } else { "no" };
                vec![
                    vec!["set".into(), "sub-use-margins".into(), value.into()],
                    vec!["set".into(), "sub-ass-force-margins".into(), value.into()],
                ]
            }
            Self::SubtitleEncoding(encoding) => {
                if !subtitle_controls::SUBTITLE_ENCODING_CHOICES
                    .iter()
                    .any(|(_, code)| *code == encoding)
                {
                    return Err("Unsupported subtitle encoding".into());
                }
                let mut commands =
                    vec![vec!["set".into(), "sub-codepage".into(), encoding.clone()]];
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
                let mut commands = vec![vec![
                    "set".into(),
                    "sub-auto".into(),
                    preferences.mpv_matching_mode().into(),
                ]];
                commands.push(vec![
                    "set".into(),
                    "sub-file-paths".into(),
                    preferences.mpv_search_paths().unwrap_or_default(),
                ]);
                commands
            }
            Self::TogglePause if state.phase == Phase::Ended => {
                // mpv stays at EOF after toggling its pause flag. Treat Play on
                // the ended state as replay so the visible Play control works.
                vec![
                    vec!["seek".into(), "0".into(), "absolute+exact".into()],
                    vec!["set".into(), "pause".into(), "no".into()],
                ]
            }
            Self::SetPaused(false) if state.phase == Phase::Ended => {
                // Play from EOF must seek first; setting pause=no alone does not replay.
                vec![
                    vec!["seek".into(), "0".into(), "absolute+exact".into()],
                    vec!["set".into(), "pause".into(), "no".into()],
                ]
            }
            Self::NetworkCache(preferences) => preferences.commands()?,
            Self::NetworkOnlineVideos(preferences) => preferences
                .engine_options()?
                .into_iter()
                .map(|(name, value)| vec!["set".into(), name, value])
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
                    vec![
                        "set".into(),
                        "screenshot-format".into(),
                        encoding.format.native_value().into(),
                    ],
                    vec![
                        "set".into(),
                        "screenshot-template".into(),
                        encoding.template.clone(),
                    ],
                    // Match the reference's JXL software path without changing
                    // screenshot conversion policy for the other formats.
                    vec![
                        "set".into(),
                        "screenshot-sw".into(),
                        if encoding.format == screenshot::Format::Jxl {
                            "yes"
                        } else {
                            "no"
                        }
                        .into(),
                    ],
                    // Keep the native trailing separator: mixed '/' in a
                    // verbatim Windows path is rejected by screenshot writing.
                    vec![
                        "set".into(),
                        "screenshot-dir".into(),
                        std::path::Path::new(directory)
                            .join("")
                            .to_string_lossy()
                            .into_owned(),
                    ],
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
            Self::ToggleVideoFilter(filter) => {
                if !video_filters::valid_filter_string(filter) {
                    return Err("The saved video filter string is invalid".into());
                }
                let filters = state
                    .video_filters
                    .as_ref()
                    .ok_or("Video filters are unavailable")?;
                if let Some(index) = filters.iter().position(|candidate| {
                    video_filters::is_active(filter, std::slice::from_ref(candidate))
                }) {
                    vec![vec!["vf".into(), "del".into(), index.to_string()]]
                } else {
                    vec![vec!["vf".into(), "add".into(), filter.clone()]]
                }
            }
            Self::AddVideoFilter(filter) => {
                if !video_filters::valid_filter_string(filter) {
                    return Err("The video filter string is invalid".into());
                }
                vec![vec!["vf".into(), "add".into(), filter.clone()]]
            }
            Self::RemoveVideoFilter(filter) => {
                let filters = state
                    .video_filters
                    .as_ref()
                    .ok_or("Video filters are unavailable")?;
                let index = filters
                    .iter()
                    .position(|candidate| candidate == filter)
                    .ok_or("The selected video filter is no longer active")?;
                vec![vec!["vf".into(), "del".into(), index.to_string()]]
            }
            Self::SetVideoFilterAt { index, filter } => {
                if !video_filters::valid_filter_string(filter) {
                    return Err("The video filter string is invalid".into());
                }
                let filters = state
                    .video_filters
                    .as_ref()
                    .ok_or("Video filters are unavailable")?;
                if *index >= filters.len() {
                    return Err("The selected video filter is no longer active".into());
                }
                let mut chain = filters
                    .iter()
                    .map(video_filters::format_filter)
                    .collect::<Vec<_>>();
                chain[*index] = filter.clone();
                vec![vec!["vf".into(), "set".into(), chain.join(",")]]
            }
            Self::ToggleAudioFilter(filter) => {
                if !video_filters::valid_filter_string(filter) {
                    return Err("The saved audio filter string is invalid".into());
                }
                let filters = state
                    .audio_filters
                    .as_ref()
                    .ok_or("Audio filters are unavailable")?;
                if let Some(index) = filters.iter().position(|candidate| {
                    video_filters::is_active(filter, std::slice::from_ref(candidate))
                }) {
                    vec![vec!["af".into(), "del".into(), index.to_string()]]
                } else {
                    vec![vec!["af".into(), "add".into(), filter.clone()]]
                }
            }
            Self::AddAudioFilter(filter) => {
                if !video_filters::valid_filter_string(filter) {
                    return Err("The audio filter string is invalid".into());
                }
                vec![vec!["af".into(), "add".into(), filter.clone()]]
            }
            Self::RemoveAudioFilter(filter) => {
                let filters = state
                    .audio_filters
                    .as_ref()
                    .ok_or("Audio filters are unavailable")?;
                let index = filters
                    .iter()
                    .position(|candidate| candidate == filter)
                    .ok_or("The selected audio filter is no longer active")?;
                vec![vec!["af".into(), "del".into(), index.to_string()]]
            }
            Self::SetAudioFilterAt { index, filter } => {
                if !video_filters::valid_filter_string(filter) {
                    return Err("The audio filter string is invalid".into());
                }
                let filters = state
                    .audio_filters
                    .as_ref()
                    .ok_or("Audio filters are unavailable")?;
                if *index >= filters.len() {
                    return Err("The selected audio filter is no longer active".into());
                }
                let mut chain = filters
                    .iter()
                    .map(video_filters::format_filter)
                    .collect::<Vec<_>>();
                chain[*index] = filter.clone();
                vec![vec!["af".into(), "set".into(), chain.join(",")]]
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
                    commands.push(vec!["write-watch-later-config".into()]);
                }
                commands.push(vec!["stop".into()]);
                commands
            }
            Self::SetResumeLastPosition(enabled) => {
                let value = if *enabled { "yes" } else { "no" };
                vec![
                    vec!["set".into(), "save-position-on-quit".into(), value.into()],
                    vec!["set".into(), "resume-playback".into(), value.into()],
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
                vec![
                    vec!["set".into(), "ab-loop-a".into(), a],
                    vec!["set".into(), "ab-loop-b".into(), b],
                ]
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
                vec![vec!["set".into(), name.into(), value]]
            }
            Self::SetToneMapping(settings) => settings.engine_commands()?,
            Self::SetIccProfileAuto(enabled) => vec![vec![
                "set".into(),
                "icc-profile-auto".into(),
                if *enabled { "yes" } else { "no" }.into(),
            ]],
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
            Self::VideoThreads(threads) if *threads <= i32::MAX as u32 => vec![vec![
                "set".into(),
                "vd-lavc-threads".into(),
                threads.to_string(),
            ]],
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
                    commands.insert(
                        0,
                        vec![
                            "set".into(),
                            "pause".into(),
                            if state.pause_when_open { "yes" } else { "no" }.into(),
                        ],
                    );
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
                commands.extend([
                    vec![
                        "set".into(),
                        "pause".into(),
                        if state.pause_when_open { "yes" } else { "no" }.into(),
                    ],
                    self.mpv_args()?,
                ]);
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
                    vec!["set".into(), "pause".into(), "no".into()],
                ]
            }
            Self::SelectPlaylistEntry(index) => {
                if *index >= state.playlist.len() {
                    return Err("The playlist entry is no longer available".into());
                }
                resume_on_load = true;
                vec![
                    vec!["set".into(), "pause".into(), "yes".into()],
                    self.mpv_args()?,
                ]
            }
            Self::PlaylistPrevious if state.playlist.first().is_some_and(|entry| entry.current) => {
                vec![vec!["seek".into(), "0".into(), "absolute".into()]]
            }
            Self::PlaylistNext | Self::PlaylistPrevious => {
                resume_on_load = true;
                vec![
                    vec!["set".into(), "pause".into(), "yes".into()],
                    self.mpv_args()?,
                ]
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
fn set_loop_mode_commands(mode: LoopMode) -> Vec<Vec<String>> {
    match mode {
        LoopMode::Off => vec![
            vec!["set".into(), "loop-playlist".into(), "no".into()],
            vec!["set".into(), "loop-file".into(), "no".into()],
        ],
        LoopMode::File => vec![vec!["set".into(), "loop-file".into(), "inf".into()]],
        LoopMode::Playlist => vec![
            vec!["set".into(), "loop-playlist".into(), "inf".into()],
            vec!["set".into(), "loop-file".into(), "no".into()],
        ],
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
        vec![
            "set".into(),
            "volume-max".into(),
            if boost { "200" } else { "100" }.into(),
        ],
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
