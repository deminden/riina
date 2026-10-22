use super::Command;
use crate::{
    control_preferences, key_bindings, playlist_end_policy, subtitle_controls, valid_path,
};

impl Command {
    pub fn mpv_args(&self) -> Result<Vec<String>, String> {
        let args: Vec<String> = match self {
            Self::Load(path) if !path.is_empty() && !path.contains('\0') => {
                vec!["loadfile".into(), path.clone(), "replace".into()]
            }
            Self::Load(_) => return Err("The media path is empty or contains a NUL byte".into()),
            Self::LoadAtPosition { path, position }
                if valid_path(path) && position.is_finite() && *position >= 0.0 =>
            {
                vec![
                    "loadfile".into(),
                    path.clone(),
                    "replace".into(),
                    "-1".into(),
                    format!("start={position}"),
                ]
            }
            Self::LoadAtPosition { .. } => {
                return Err("The resume file or position is invalid".into());
            }
            Self::SetPaused(paused) => vec![
                "set".into(),
                "pause".into(),
                if *paused { "yes" } else { "no" }.into(),
            ],
            Self::TogglePause => vec!["cycle".into(), "pause".into()],
            Self::SetPlaybackEndPolicy {
                keep_open_on_file_end,
                playlist_auto_play_next,
            } => vec![
                "set".into(),
                "keep-open".into(),
                playlist_end_policy(*keep_open_on_file_end, *playlist_auto_play_next).into(),
            ],
            Self::MpvInputAction(action) => key_bindings::mpv_action_args(action)?,
            Self::Stop => vec!["stop".into()],
            Self::Seek { seconds, absolute } if seconds.is_finite() => vec![
                "seek".into(),
                seconds.to_string(),
                if *absolute {
                    "absolute+exact"
                } else {
                    "relative"
                }
                .into(),
            ],
            Self::SeekWithType { seconds, seek_type } if seconds.is_finite() => vec![
                "seek".into(),
                seconds.to_string(),
                match seek_type {
                    control_preferences::SeekType::Keyframe
                    | control_preferences::SeekType::Auto => "relative",
                    control_preferences::SeekType::Exact => "relative+exact",
                }
                .into(),
            ],
            Self::Volume(value) if value.is_finite() && (0.0..=200.0).contains(value) => {
                vec!["set".into(), "volume".into(), value.to_string()]
            }
            Self::AudioDevice(name) if !name.is_empty() && !name.contains('\0') => {
                vec!["set".into(), "audio-device".into(), name.clone()]
            }
            Self::AudioDevice(_) => {
                return Err("The audio device name is empty or contains a NUL byte".into());
            }
            // The reference's custom field has no upper bound. Submit finite
            // requests above the engine limit so native rejection remains an
            // observed failure instead of inventing a narrower input contract.
            Self::Speed(value) if value.is_finite() && *value >= 0.01 => {
                vec!["set".into(), "speed".into(), value.to_string()]
            }
            Self::MultiplySpeed(factor) if factor.is_finite() && *factor > 0.0 => {
                vec!["multiply".into(), "speed".into(), factor.to_string()]
            }
            Self::MultiplySubtitleScale(factor) if factor.is_finite() && *factor > 0.0 => {
                vec!["multiply".into(), "sub-scale".into(), factor.to_string()]
            }
            Self::ToggleMute => vec!["cycle".into(), "mute".into()],
            Self::SetMuted(muted) => vec![
                "set".into(),
                "mute".into(),
                if *muted { "yes" } else { "no" }.into(),
            ],
            Self::FrameStep => vec!["frame-step".into()],
            Self::FrameBackStep => vec!["frame-back-step".into()],
            Self::CaptureFrame => vec!["screenshot-raw".into(), "video".into(), "rgba".into()],
            Self::CaptureTransitionFrame => {
                vec!["screenshot-raw".into(), "video".into(), "rgba".into()]
            }
            Self::CaptureIncomingTransitionFrame => {
                vec!["screenshot-raw".into(), "video".into(), "rgba".into()]
            }
            Self::CaptureMaterialFrame => {
                vec!["screenshot-raw".into(), "video".into(), "rgba".into()]
            }
            Self::ObserveMpvProperty(_) => {
                return Err("mpv property observation is handled by the player worker".into());
            }
            Self::ScreenshotToFile { path, content } if valid_path(path) => vec![
                "screenshot-to-file".into(),
                path.clone(),
                content.native_value().into(),
            ],
            Self::PlaylistNext => vec!["playlist-next".into(), "weak".into()],
            Self::PlaylistPrevious => vec!["playlist-prev".into(), "weak".into()],
            Self::SelectTrack { kind, id } if id.is_none_or(|id| id > 0) => vec![
                "set".into(),
                kind.property().into(),
                id.map_or_else(|| "no".into(), |id| id.to_string()),
            ],
            Self::CycleTrack(kind) => {
                // Native cycling owns track ordering and the disabled sentinel;
                // a catalog snapshot can lag external-track insertion/removal.
                vec!["cycle".into(), kind.property().into()]
            }
            Self::VideoAspect(aspect) => vec![
                "set".into(),
                "video-aspect-override".into(),
                aspect.native_value().into(),
            ],
            // Read the native chain at execution; rapid activations must not
            // invert the same stale observed state twice.
            Self::ToggleVideoReflection(axis) => vec!["vf".into(), "toggle".into(), axis.filter()],
            Self::VideoRotation(rotation) => vec![
                "set".into(),
                "video-rotate".into(),
                rotation.degrees().to_string(),
            ],
            Self::Deinterlace(enabled) => vec![
                "set".into(),
                "deinterlace".into(),
                if *enabled { "yes" } else { "no" }.into(),
            ],
            // Limit the native cycle to the reference's two states; newer mpv
            // also offers auto, which is not a third state in IINA's checkbox.
            Self::ToggleDeinterlace => vec![
                "cycle-values".into(),
                "deinterlace".into(),
                "no".into(),
                "yes".into(),
            ],
            Self::HardwareDecoding(mode) => {
                vec!["set".into(), "hwdec".into(), mode.native_value().into()]
            }
            Self::SubtitleAssOverride { lane, level } => vec![
                "set".into(),
                match lane {
                    subtitle_controls::SubtitleLane::Primary => "sub-ass-override",
                    subtitle_controls::SubtitleLane::Secondary => "secondary-sub-ass-override",
                }
                .into(),
                level.mpv_value().into(),
            ],
            Self::VideoEqualizer { option, value } if (-100..=100).contains(value) => {
                vec!["set".into(), option.property().into(), value.to_string()]
            }
            Self::SelectSecondarySubtitle(id) if id.is_none_or(|id| id > 0) => vec![
                "set".into(),
                "secondary-sid".into(),
                id.map_or_else(|| "no".into(), |id| id.to_string()),
            ],
            Self::SelectChapter(index) => vec!["set".into(), "chapter".into(), index.to_string()],
            Self::AdjustChapter(delta) => {
                // Relative navigation keeps native restart-threshold and boundary
                // semantics; selecting a sidebar chapter intentionally resumes.
                vec!["add".into(), "chapter".into(), delta.to_string()]
            }
            Self::SelectPlaylistEntry(index) => {
                // Setting playlist-pos does not reload an already selected entry.
                vec!["playlist-play-index".into(), index.to_string()]
            }
            Self::RemovePlaylistEntry(index) => vec!["playlist-remove".into(), index.to_string()],
            Self::MovePlaylistEntry { from, to } => {
                vec!["playlist-move".into(), from.to_string(), to.to_string()]
            }
            Self::ClearPlaylist => vec!["playlist-clear".into()],
            Self::AddSubtitle(path) | Self::AddAudio(path) | Self::AddVideo(path)
                if valid_path(path) =>
            {
                let args = vec![
                    match self {
                        Self::AddAudio(_) => "audio-add",
                        Self::AddVideo(_) => "video-add",
                        _ => "sub-add",
                    }
                    .into(),
                    path.clone(),
                ];
                args
            }
            Self::AudioDelay(value) | Self::SubtitleDelay(value) if value.is_finite() => vec![
                "set".into(),
                if matches!(self, Self::AudioDelay(_)) {
                    "audio-delay"
                } else {
                    "sub-delay"
                }
                .into(),
                value.to_string(),
            ],
            Self::ToggleSubtitleVisibility => vec!["cycle".into(), "sub-visibility".into()],
            Self::SubtitleFont(font) if subtitle_controls::valid_font_name(font) => {
                vec!["set".into(), "sub-font".into(), font.clone()]
            }
            Self::SubtitlePreferredLanguages(languages)
                if languages.len() <= 256 && !languages.chars().any(char::is_control) =>
            {
                vec!["set".into(), "slang".into(), languages.clone()]
            }
            Self::SubtitleBold(value) => vec![
                "set".into(),
                "sub-bold".into(),
                if *value { "yes" } else { "no" }.into(),
            ],
            Self::SubtitleItalic(value) => vec![
                "set".into(),
                "sub-italic".into(),
                if *value { "yes" } else { "no" }.into(),
            ],
            Self::SubtitleBlur(value) if value.is_finite() && (0.0..=20.0).contains(value) => {
                vec!["set".into(), "sub-blur".into(), value.to_string()]
            }
            Self::SubtitleSpacing(value) if value.is_finite() => {
                vec!["set".into(), "sub-spacing".into(), value.to_string()]
            }
            Self::SubtitleShadowOffset(value) if value.is_finite() && *value >= 0.0 => {
                vec!["set".into(), "sub-shadow-offset".into(), value.to_string()]
            }
            Self::SubtitleTextColor(color)
            | Self::SubtitleBorderColor(color)
            | Self::SubtitleShadowColor(color) => {
                let option = match self {
                    Self::SubtitleTextColor(_) => "sub-color",
                    Self::SubtitleBorderColor(_) => "sub-outline-color",
                    _ => "sub-back-color",
                };
                vec!["set".into(), option.into(), color.mpv_value()]
            }
            Self::SubtitleOutlineSize(value) if value.is_finite() && *value >= 0.0 => {
                vec!["set".into(), "sub-outline-size".into(), value.to_string()]
            }
            Self::SubtitleFontSize(value) if value.is_finite() && *value > 0.0 => {
                vec!["set".into(), "sub-font-size".into(), value.to_string()]
            }
            Self::SubtitleScale(value) if value.is_finite() && (0.0..=100.0).contains(value) => {
                vec!["set".into(), "sub-scale".into(), value.to_string()]
            }
            Self::SecondarySubtitleDelay(value) if value.is_finite() => vec![
                "set".into(),
                "secondary-sub-delay".into(),
                value.to_string(),
            ],
            Self::SubtitlePosition(value) | Self::SecondarySubtitlePosition(value)
                if value.is_finite() && (0.0..=150.0).contains(value) =>
            {
                vec![
                    "set".into(),
                    if matches!(self, Self::SubtitlePosition(_)) {
                        "sub-pos"
                    } else {
                        "secondary-sub-pos"
                    }
                    .into(),
                    value.to_string(),
                ]
            }
            Self::SubtitleAlignX(alignment) => vec![
                "set".into(),
                "sub-align-x".into(),
                alignment.mpv_value().into(),
            ],
            Self::SubtitleAlignY(alignment) => vec![
                "set".into(),
                "sub-align-y".into(),
                alignment.mpv_value().into(),
            ],
            Self::SubtitleScaleWithWindow(enabled) => vec![
                "set".into(),
                "sub-scale-by-window".into(),
                if *enabled { "yes" } else { "no" }.into(),
            ],
            Self::SubtitleMarginX(value) | Self::SubtitleMarginY(value)
                if value.is_finite() && (0.0..=2_000_000_000.0).contains(value) =>
            {
                vec![
                    "set".into(),
                    if matches!(self, Self::SubtitleMarginX(_)) {
                        "sub-margin-x"
                    } else {
                        "sub-margin-y"
                    }
                    .into(),
                    value.to_string(),
                ]
            }
            Self::ToggleSecondarySubtitleVisibility => {
                vec!["cycle".into(), "secondary-sub-visibility".into()]
            }
            Self::AdjustAudioDelay(delta)
            | Self::AdjustSubtitleDelay(delta)
            | Self::AdjustSubtitleScale(delta)
            | Self::AdjustSubtitlePosition(delta)
                if delta.is_finite() =>
            {
                // Apply on the decoder worker's current value; rapid key presses
                // must not reuse a stale value from a coalesced UI snapshot.
                vec![
                    "add".into(),
                    match self {
                        Self::AdjustAudioDelay(_) => "audio-delay",
                        Self::AdjustSubtitleDelay(_) => "sub-delay",
                        Self::AdjustSubtitleScale(_) => "sub-scale",
                        Self::AdjustSubtitlePosition(_) => "sub-pos",
                        _ => unreachable!(),
                    }
                    .into(),
                    delta.to_string(),
                ]
            }
            Self::SubtitleSeek(skip) => {
                // Let the engine resolve selected-track cues and subtitle delay;
                // a UI timestamp search would bypass both timing semantics.
                vec!["sub-seek".into(), skip.to_string()]
            }
            Self::CycleAbLoop => vec!["ab-loop".into()],
            Self::ToggleFileLoop => {
                // This shortcut changes only file repeat; a playlist repeat
                // setting survives while file repeat temporarily takes priority.
                vec![
                    "cycle-values".into(),
                    "loop-file".into(),
                    "inf".into(),
                    "no".into(),
                ]
            }
            Self::OpenFiles(_)
            | Self::OpenFilesWithSubtitles { .. }
            | Self::SetPauseWhenOpen(_)
            | Self::SetResumeLastPosition(_)
            | Self::Screenshot { .. }
            | Self::VideoCrop(_)
            | Self::VideoCropRectangle(_)
            | Self::RemoveDelogo
            | Self::ToggleVideoFilter(_)
            | Self::AddVideoFilter(_)
            | Self::RemoveVideoFilter(_)
            | Self::SetVideoFilterAt { .. }
            | Self::ToggleAudioFilter(_)
            | Self::AddAudioFilter(_)
            | Self::RemoveAudioFilter(_)
            | Self::SetAudioFilterAt { .. }
            | Self::VideoViewport(_)
            | Self::AudioEqualizer(_)
            | Self::VideoReflection { .. }
            | Self::EnterCrop(_)
            | Self::ExitCrop { .. }
            | Self::AdjustVolume { .. }
            | Self::SetVolumeBoost(_)
            | Self::AppendFiles(_)
            | Self::AddSubtitlesWithoutSelection(_)
            | Self::InsertPlaylistFiles { .. }
            | Self::SubtitleLetterboxMargins(_)
            | Self::SubtitleEncoding(_)
            | Self::SubtitleAutoLoad(_)
            | Self::CycleLoop
            | Self::SetLoopMode(_)
            | Self::SetAbLoop { .. }
            | Self::SetAbLoopPoint { .. }
            | Self::SetToneMapping(_)
            | Self::SetIccProfileAuto(_)
            | Self::SetHdrEnabled(_)
            | Self::SetHdrEnabledWithIcc { .. }
            | Self::VideoThreads(_)
            | Self::SelectPlaylistId(_)
            | Self::RemovePlaylistId(_)
            | Self::RemovePlaylistIds(_)
            | Self::MovePlaylistId { .. } => {
                return Err("This intent requires a command plan".into());
            }
            Self::Shutdown => return Err("Shutdown is a lifecycle command".into()),
            _ => return Err("The requested value is out of range or non-finite".into()),
        };
        Ok(args)
    }
}
