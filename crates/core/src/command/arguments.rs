use super::{Command, cmd, flag, set};
use crate::{
    control_preferences, key_bindings, playlist_end_policy, subtitle_controls, valid_path,
};

impl Command {
    #[expect(
        clippy::too_many_lines,
        reason = "decomposition tracked in docs/review"
    )]
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
            Self::SetPaused(paused) => set("pause", flag(*paused)),
            Self::TogglePause => cmd(["cycle", "pause"]),
            Self::SetPlaybackEndPolicy {
                keep_open_on_file_end,
                playlist_auto_play_next,
            } => set(
                "keep-open",
                playlist_end_policy(*keep_open_on_file_end, *playlist_auto_play_next),
            ),
            Self::MpvInputAction(action) => key_bindings::mpv_action_args(action)?,
            Self::Stop => cmd(["stop"]),
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
                set("volume", value.to_string())
            }
            Self::AudioDevice(name) if !name.is_empty() && !name.contains('\0') => {
                set("audio-device", name.clone())
            }
            Self::AudioDevice(_) => {
                return Err("The audio device name is empty or contains a NUL byte".into());
            }
            // The reference's custom field has no upper bound. Submit finite
            // requests above the engine limit so native rejection remains an
            // observed failure instead of inventing a narrower input contract.
            Self::Speed(value) if value.is_finite() && *value >= 0.01 => {
                set("speed", value.to_string())
            }
            Self::MultiplySpeed(factor) if factor.is_finite() && *factor > 0.0 => {
                vec!["multiply".into(), "speed".into(), factor.to_string()]
            }
            Self::MultiplySubtitleScale(factor) if factor.is_finite() && *factor > 0.0 => {
                vec!["multiply".into(), "sub-scale".into(), factor.to_string()]
            }
            Self::ToggleMute => cmd(["cycle", "mute"]),
            Self::SetMuted(muted) => set("mute", flag(*muted)),
            Self::FrameStep => cmd(["frame-step"]),
            Self::FrameBackStep => cmd(["frame-back-step"]),
            Self::CaptureFrame => cmd(["screenshot-raw", "video", "rgba"]),
            Self::CaptureTransitionFrame => cmd(["screenshot-raw", "video", "rgba"]),
            Self::CaptureIncomingTransitionFrame => cmd(["screenshot-raw", "video", "rgba"]),
            Self::CaptureMaterialFrame => cmd(["screenshot-raw", "video", "rgba"]),
            Self::ObserveMpvProperty(_) => {
                return Err("mpv property observation is handled by the player worker".into());
            }
            Self::ScreenshotToFile { path, content } if valid_path(path) => vec![
                "screenshot-to-file".into(),
                path.clone(),
                content.native_value().into(),
            ],
            Self::PlaylistNext => cmd(["playlist-next", "weak"]),
            Self::PlaylistPrevious => cmd(["playlist-prev", "weak"]),
            Self::SelectTrack { kind, id } if id.is_none_or(|id| id > 0) => set(
                kind.property(),
                id.map_or_else(|| "no".into(), |id| id.to_string()),
            ),
            Self::CycleTrack(kind) => {
                // Native cycling owns track ordering and the disabled sentinel;
                // a catalog snapshot can lag external-track insertion/removal.
                vec!["cycle".into(), kind.property().into()]
            }
            Self::VideoAspect(aspect) => set("video-aspect-override", aspect.native_value()),
            // Read the native chain at execution; rapid activations must not
            // invert the same stale observed state twice.
            Self::ToggleVideoReflection(axis) => vec!["vf".into(), "toggle".into(), axis.filter()],
            Self::VideoRotation(rotation) => set("video-rotate", rotation.degrees().to_string()),
            Self::Deinterlace(enabled) => set("deinterlace", flag(*enabled)),
            // Limit the native cycle to the reference's two states; newer mpv
            // also offers auto, which is not a third state in IINA's checkbox.
            Self::ToggleDeinterlace => cmd(["cycle-values", "deinterlace", "no", "yes"]),
            Self::HardwareDecoding(mode) => set("hwdec", mode.native_value()),
            Self::SubtitleAssOverride { lane, level } => set(
                match lane {
                    subtitle_controls::SubtitleLane::Primary => "sub-ass-override",
                    subtitle_controls::SubtitleLane::Secondary => "secondary-sub-ass-override",
                },
                level.mpv_value(),
            ),
            Self::VideoEqualizer { option, value } if (-100..=100).contains(value) => {
                set(option.property(), value.to_string())
            }
            Self::SelectSecondarySubtitle(id) if id.is_none_or(|id| id > 0) => set(
                "secondary-sid",
                id.map_or_else(|| "no".into(), |id| id.to_string()),
            ),
            Self::SelectChapter(index) => set("chapter", index.to_string()),
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
            Self::ClearPlaylist => cmd(["playlist-clear"]),
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
            Self::AudioDelay(value) | Self::SubtitleDelay(value) if value.is_finite() => set(
                if matches!(self, Self::AudioDelay(_)) {
                    "audio-delay"
                } else {
                    "sub-delay"
                },
                value.to_string(),
            ),
            Self::ToggleSubtitleVisibility => cmd(["cycle", "sub-visibility"]),
            Self::SubtitleFont(font) if subtitle_controls::valid_font_name(font) => {
                set("sub-font", font.clone())
            }
            Self::SubtitlePreferredLanguages(languages)
                if languages.len() <= 256 && !languages.chars().any(char::is_control) =>
            {
                set("slang", languages.clone())
            }
            Self::SubtitleBold(value) => set("sub-bold", flag(*value)),
            Self::SubtitleItalic(value) => set("sub-italic", flag(*value)),
            Self::SubtitleBlur(value) if value.is_finite() && (0.0..=20.0).contains(value) => {
                set("sub-blur", value.to_string())
            }
            Self::SubtitleSpacing(value) if value.is_finite() => {
                set("sub-spacing", value.to_string())
            }
            Self::SubtitleShadowOffset(value) if value.is_finite() && *value >= 0.0 => {
                set("sub-shadow-offset", value.to_string())
            }
            Self::SubtitleTextColor(color)
            | Self::SubtitleBorderColor(color)
            | Self::SubtitleShadowColor(color) => {
                let option = match self {
                    Self::SubtitleTextColor(_) => "sub-color",
                    Self::SubtitleBorderColor(_) => "sub-outline-color",
                    _ => "sub-back-color",
                };
                set(option, color.mpv_value())
            }
            Self::SubtitleOutlineSize(value) if value.is_finite() && *value >= 0.0 => {
                set("sub-outline-size", value.to_string())
            }
            Self::SubtitleFontSize(value) if value.is_finite() && *value > 0.0 => {
                set("sub-font-size", value.to_string())
            }
            Self::SubtitleScale(value) if value.is_finite() && (0.0..=100.0).contains(value) => {
                set("sub-scale", value.to_string())
            }
            Self::SecondarySubtitleDelay(value) if value.is_finite() => {
                set("secondary-sub-delay", value.to_string())
            }
            Self::SubtitlePosition(value) | Self::SecondarySubtitlePosition(value)
                if value.is_finite() && (0.0..=150.0).contains(value) =>
            {
                set(
                    if matches!(self, Self::SubtitlePosition(_)) {
                        "sub-pos"
                    } else {
                        "secondary-sub-pos"
                    },
                    value.to_string(),
                )
            }
            Self::SubtitleAlignX(alignment) => set("sub-align-x", alignment.mpv_value()),
            Self::SubtitleAlignY(alignment) => set("sub-align-y", alignment.mpv_value()),
            Self::SubtitleScaleWithWindow(enabled) => set("sub-scale-by-window", flag(*enabled)),
            Self::SubtitleMarginX(value) | Self::SubtitleMarginY(value)
                if value.is_finite() && (0.0..=2_000_000_000.0).contains(value) =>
            {
                set(
                    if matches!(self, Self::SubtitleMarginX(_)) {
                        "sub-margin-x"
                    } else {
                        "sub-margin-y"
                    },
                    value.to_string(),
                )
            }
            Self::ToggleSecondarySubtitleVisibility => cmd(["cycle", "secondary-sub-visibility"]),
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
            Self::CycleAbLoop => cmd(["ab-loop"]),
            Self::ToggleFileLoop => {
                // This shortcut changes only file repeat; a playlist repeat
                // setting survives while file repeat temporarily takes priority.
                cmd(["cycle-values", "loop-file", "inf", "no"])
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
